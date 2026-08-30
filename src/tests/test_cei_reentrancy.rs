//! CEI ordering guards for `cancel_campaign` (#795).
//!
//! # What the investigation found
//!
//! `cancel_campaign` used to refund the revenue pool *before* zeroing it and
//! before persisting the cancellation flags — a textbook checks-effects-
//! interactions violation, since `transfer` hands control to a contract whose
//! address the platform does not vet (it is chosen at `init` and replaceable
//! via `propose_token_update` / `accept_token_update`).
//!
//! The drain that ordering would allow on an EVM-style host is **not
//! reachable here**: the Soroban host refuses re-entry into a contract already
//! on the call stack. Any call back into the campaign contract from inside
//! `transfer`, even a read-only one, aborts the whole invocation at the host
//! level rather than returning a catchable error. There is no test that can
//! demonstrate the drain, because the platform makes it impossible.
//!
//! The reorder is still worth having, and is kept for three reasons:
//!
//! 1. Host re-entry protection is a property of the current protocol, not of
//!    this contract. Relying on it means the contract is only correct as long
//!    as that stays true.
//! 2. The token remains an untrusted callee, and a token replaced by a wrapper
//!    with its own call graph reopens the question of what state a third party
//!    observes mid-cancellation.
//! 3. Every other transfer site in the contract already writes state first,
//!    several with explicit `(#557)` CEI comments. `cancel_campaign` was the
//!    lone exception, and an inconsistent convention is what lets the next one
//!    slip in.
//!
//! So these tests pin the ordering directly — behaviourally where possible,
//! and structurally where the host prevents an end-to-end exploit.

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, TryFromVal};

#[contracttype]
enum RecorderKey {
    /// How many times `transfer` was invoked.
    Transfers,
    /// The amount passed to the last `transfer`.
    LastAmount,
}

/// A stand-in token that records calls instead of moving value.
///
/// It implements only `transfer`, the sole token entry point
/// `cancel_campaign` touches. It deliberately does *not* call back into the
/// campaign contract: the host aborts re-entry (see the module docs), so a
/// callback would kill the test process rather than exercise anything.
#[contract]
pub struct RecordingToken;

#[contractimpl]
impl RecordingToken {
    pub fn transfer(env: Env, _from: Address, _to: Address, amount: i128) {
        let transfers: u32 = env
            .storage()
            .instance()
            .get(&RecorderKey::Transfers)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&RecorderKey::Transfers, &(transfers + 1));
        env.storage()
            .instance()
            .set(&RecorderKey::LastAmount, &amount);
    }

    pub fn transfers(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&RecorderKey::Transfers)
            .unwrap_or(0)
    }

    pub fn last_amount(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&RecorderKey::LastAmount)
            .unwrap_or(0)
    }
}

/// Build a cancellable campaign carrying a revenue pool, then point the
/// contract at a token that only records calls.
fn campaign_with_revenue_pool_and_recording_token<'a>(
    env: &Env,
    creator: &Address,
    contributor: &Address,
    token_admin: &TokenAdminClient<'a>,
    client: &ProofOfHeartClient<'a>,
    revenue: i128,
) -> (u32, RecordingTokenClient<'a>) {
    token_admin.mint(contributor, &2000);
    token_admin.mint(creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "Revenue Pool Campaign"),
        description: String::from_str(env, "Cancelled while holding a revenue pool"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, contributor, &1000);

    // `deposit_revenue` requires funds to have been withdrawn; flip the flag
    // around the deposit so the campaign is still cancellable afterwards.
    // (Same shape as `test_cancel_campaign_refunds_revenue_pool`.)
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(env, campaign_id, &campaign);
    });

    // Swap in the recording token by writing storage directly rather than
    // through `accept_token_update`, which refuses to swap while a campaign
    // still holds escrowed funds (#407). That guard narrows the window for a
    // hostile token but does not close it — the token passed to `init` was
    // never vetted in the first place.
    let recorder_id = env.register_contract(None, RecordingToken);
    env.as_contract(&client.address, || {
        storage::set_token(env, &recorder_id);
    });

    (campaign_id, RecordingTokenClient::new(env, &recorder_id))
}

/// The refund reaches the token exactly once, for the full pool, and the
/// campaign is terminal afterwards.
///
/// The single-transfer assertion is the part that would have caught a repeat
/// refund had the host permitted one.
#[test]
fn test_cancel_campaign_refunds_pool_exactly_once() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let (campaign_id, recorder) = campaign_with_revenue_pool_and_recording_token(
        &env,
        &creator,
        &contributor1,
        &token_admin,
        &client,
        3000,
    );

    client.cancel_campaign(&campaign_id);

    assert_eq!(
        recorder.transfers(),
        1,
        "revenue pool refunded more than once"
    );
    assert_eq!(recorder.last_amount(), 3000);

    // Post-conditions a re-entrant caller would have needed to violate.
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);

    // A second cancel is refused, so the refund cannot be replayed by an
    // ordinary caller either.
    let res = client.try_cancel_campaign(&campaign_id);
    assert!(res.is_err());
    assert_eq!(recorder.transfers(), 1);
}

/// With no revenue pool the token is never called at all — the interaction is
/// conditional on the effect having something to undo.
#[test]
fn test_cancel_campaign_without_pool_never_calls_token() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "No Revenue"),
        description: String::from_str(&env, "Cancelled with an empty revenue pool"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    let recorder_id = env.register_contract(None, RecordingToken);
    env.as_contract(&client.address, || {
        storage::set_token(&env, &recorder_id);
    });
    let recorder = RecordingTokenClient::new(&env, &recorder_id);

    client.cancel_campaign(&campaign_id);

    assert_eq!(recorder.transfers(), 0);
    assert!(client.get_campaign(&campaign_id).is_cancelled);
}

/// The cancellation event still lands when a refund happens, so the refund
/// path does not swallow the campaign's own bookkeeping.
#[test]
fn test_cancel_campaign_emits_both_refund_and_cancellation() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let (campaign_id, _recorder) = campaign_with_revenue_pool_and_recording_token(
        &env,
        &creator,
        &contributor1,
        &token_admin,
        &client,
        2500,
    );

    client.cancel_campaign(&campaign_id);

    for topic in ["revenue_pool_refunded", "campaign_cancelled"] {
        let expected = String::from_str(&env, topic);
        assert!(
            env.events().all().iter().any(|(_, topics, _)| {
                topics
                    .get(0)
                    .and_then(|v| String::try_from_val(&env, &v).ok())
                    .map(|s| s == expected)
                    .unwrap_or(false)
            }),
            "missing event: {}",
            topic
        );
    }
}

/// Structural guard on the ordering itself.
///
/// The host's re-entry ban means no behavioural test can distinguish
/// state-then-transfer from transfer-then-state — both produce identical
/// ledgers. This reads the source and asserts the ordering directly, which is
/// blunt but is the only thing that actually fails if the CEI fix is reverted.
///
/// Anchored on the call shapes rather than on line numbers, so unrelated edits
/// to the function do not break it.
#[test]
fn test_cancel_campaign_source_orders_effects_before_interaction() {
    let source = include_str!("../campaigns/cancel.rs");

    // Scope the check to `cancel_campaign`; `admin_cancel_campaign` follows it
    // in the same file and deliberately does not refund.
    let start = source
        .find("pub(crate) fn cancel_campaign")
        .expect("cancel_campaign not found");
    let end = source[start..]
        .find("pub(crate) fn admin_cancel_campaign")
        .map(|i| start + i)
        .unwrap_or(source.len());
    let body = &source[start..end];

    let zero_pool = body
        .find("set_revenue_pool(env, campaign_id, 0)")
        .expect("revenue pool is never zeroed");
    let persist = body
        .find("set_campaign(env, campaign_id, &campaign)")
        .expect("cancellation is never persisted");
    let transfer = body
        .find("client.transfer(")
        .expect("no token transfer in cancel_campaign");

    assert!(
        zero_pool < transfer,
        "CEI (#795): the revenue pool must be zeroed before the token transfer"
    );
    assert!(
        persist < transfer,
        "CEI (#795): the cancelled campaign must be persisted before the token transfer"
    );
}
