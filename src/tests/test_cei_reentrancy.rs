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
    let recorder_id = env.register(RecordingToken, ());
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

    let recorder_id = env.register(RecordingToken, ());
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

// ─────────────────────────────────────────────────────────────────────────────
// Re-entrancy attack simulation harness (#1131)
//
// The tests above pin *ordering*. This section actually attacks: a malicious
// token whose `transfer` calls back into the campaign contract, trying to
// withdraw, claim or cancel a second time while the first call is still on the
// stack. Each scenario asserts the two things a real drain would have to break:
//
// 1. the re-entrant call never succeeds, and
// 2. no more value than the campaign holds ever leaves the contract.
//
// Whether the host surfaces the refused re-entry to the token as a catchable
// error or aborts the outer invocation is host behaviour, not contract
// behaviour, so the scenarios assert the outcomes that matter under either
// (no successful re-entry, no double payout, consistent post-state) instead of
// a specific error shape.
// ─────────────────────────────────────────────────────────────────────────────

/// Namespaced because `#[contractimpl]` emits module-level symbols named after
/// each function, and `RecordingToken` above already owns `transfer`.
mod attacker {
    use super::*;

    /// The entrypoint the malicious token tries to re-enter from inside `transfer`.
    #[contracttype]
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum Attack {
        WithdrawFunds,
        ClaimRefund,
        CancelCampaign,
        ClaimMilestone,
    }

    #[contracttype]
    enum AttackKey {
        Target,
        CampaignId,
        Victim,
        MilestoneId,
        Action,
        /// Times `transfer` was entered.
        Transfers,
        /// Sum of all amounts passed to `transfer`.
        Moved,
        /// Re-entry attempts that came back `Ok` — a successful attack.
        ReentrySucceeded,
    }

    fn get_u32(env: &Env, key: &AttackKey) -> u32 {
        env.storage().instance().get(key).unwrap_or(0)
    }

    /// A token that fights back: every `transfer` it receives from the campaign
    /// contract triggers a re-entry attempt against the same contract.
    ///
    /// It also keeps honest books (`transfers`, `moved`) so tests can prove how
    /// much value actually left the contract.
    #[contract]
    pub struct ReentrantToken;

    #[contractimpl]
    impl ReentrantToken {
        /// Point the token at the contract and the entrypoint to attack.
        pub fn arm(
            env: Env,
            target: Address,
            campaign_id: u32,
            victim: Address,
            milestone_id: u32,
            action: Attack,
        ) {
            let s = env.storage().instance();
            s.set(&AttackKey::Target, &target);
            s.set(&AttackKey::CampaignId, &campaign_id);
            s.set(&AttackKey::Victim, &victim);
            s.set(&AttackKey::MilestoneId, &milestone_id);
            s.set(&AttackKey::Action, &action);
        }

        pub fn transfer(env: Env, _from: Address, _to: Address, amount: i128) {
            let s = env.storage().instance();
            s.set(
                &AttackKey::Transfers,
                &(get_u32(&env, &AttackKey::Transfers) + 1),
            );
            let moved: i128 = s.get(&AttackKey::Moved).unwrap_or(0);
            s.set(&AttackKey::Moved, &(moved + amount));

            let Some(target) = s.get::<_, Address>(&AttackKey::Target) else {
                return;
            };
            let campaign_id = get_u32(&env, &AttackKey::CampaignId);
            let milestone_id = get_u32(&env, &AttackKey::MilestoneId);
            let victim: Address = s.get(&AttackKey::Victim).unwrap();
            let action: Attack = s.get(&AttackKey::Action).unwrap();

            let contract = ProofOfHeartClient::new(&env, &target);
            let succeeded = match action {
                Attack::WithdrawFunds => {
                    matches!(contract.try_withdraw_funds(&campaign_id), Ok(Ok(_)))
                }
                Attack::ClaimRefund => {
                    matches!(contract.try_claim_refund(&campaign_id, &victim), Ok(Ok(_)))
                }
                Attack::CancelCampaign => {
                    matches!(contract.try_cancel_campaign(&campaign_id), Ok(Ok(_)))
                }
                Attack::ClaimMilestone => {
                    matches!(
                        contract.try_claim_milestone(&campaign_id, &milestone_id),
                        Ok(Ok(_))
                    )
                }
            };
            if succeeded {
                s.set(
                    &AttackKey::ReentrySucceeded,
                    &(get_u32(&env, &AttackKey::ReentrySucceeded) + 1),
                );
            }
        }

        pub fn transfers(env: Env) -> u32 {
            get_u32(&env, &AttackKey::Transfers)
        }

        pub fn moved(env: Env) -> i128 {
            env.storage().instance().get(&AttackKey::Moved).unwrap_or(0)
        }

        pub fn reentry_succeeded(env: Env) -> u32 {
            get_u32(&env, &AttackKey::ReentrySucceeded)
        }
    }
}

use attacker::{Attack, ReentrantToken, ReentrantTokenClient};

const GOAL: i128 = 1000;

/// A verified campaign that has met its goal, with `GOAL` escrowed in the real
/// token, whose currency is then swapped for a [`ReentrantToken`].
///
/// Funding happens with the real token first so `contribute` behaves normally;
/// only the payout path talks to the hostile token. Pass `None` for a control
/// run: the token is swapped in but never fights back, which proves the
/// scenario's preconditions are met and any failure in an armed run is down to
/// the attack, not to the setup.
fn hostile_campaign<'a>(
    env: &Env,
    creator: &Address,
    contributor: &Address,
    token_admin: &TokenAdminClient<'a>,
    client: &ProofOfHeartClient<'a>,
    action: Option<Attack>,
) -> (u32, ReentrantTokenClient<'a>) {
    token_admin.mint(contributor, &(GOAL * 2));

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "Attacked Campaign"),
        description: String::from_str(env, "Paid out through a hostile token"),
        funding_goal: GOAL,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, contributor, &GOAL);

    let attacker_id = env.register(ReentrantToken, ());
    env.as_contract(&client.address, || {
        storage::set_campaign_token(env, campaign_id, &attacker_id);
    });
    let attacker = ReentrantTokenClient::new(env, &attacker_id);
    if let Some(action) = action {
        attacker.arm(&client.address, &campaign_id, contributor, &1, &action);
    }

    (campaign_id, attacker)
}

/// `withdraw_funds` is only allowed once the funding window has closed.
fn pass_deadline(env: &Env, client: &ProofOfHeartClient, campaign_id: u32) {
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().with_mut(|l| l.timestamp = deadline + 1);
}

/// The invariant every armed run must satisfy, whichever way the host handles
/// the refused re-entry:
///
/// * it never succeeded, and
/// * if the outer call failed, nothing was left half-applied — the whole
///   invocation rolled back, so no value moved and `committed` state is intact;
/// * if the outer call succeeded, the token really was entered (the attack ran)
///   and the total that moved stays within `max_moved`.
fn assert_reentry_refused<T, E>(
    outcome: &Result<T, E>,
    attacker: &ReentrantTokenClient,
    max_moved: i128,
) {
    assert_eq!(
        attacker.reentry_succeeded(),
        0,
        "a re-entrant call succeeded"
    );
    assert!(
        attacker.moved() <= max_moved,
        "{} left the contract, more than the {} it may release",
        attacker.moved(),
        max_moved
    );
    match outcome {
        Ok(_) => assert!(
            attacker.transfers() >= 1,
            "the payout never reached the hostile token, so nothing was attacked"
        ),
        Err(_) => assert_eq!(attacker.moved(), 0, "failed call still moved funds"),
    }
}

/// Control: with the token swapped but not armed, the payout succeeds and
/// releases the full escrow. Every armed test below is measured against this.
#[test]
fn test_control_unarmed_withdraw_pays_out_once() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, token) =
        hostile_campaign(&env, &creator, &contributor, &token_admin, &client, None);
    pass_deadline(&env, &client, campaign_id);

    client.withdraw_funds(&campaign_id);

    assert!(client.get_campaign(&campaign_id).funds_withdrawn);
    assert!(token.transfers() >= 1);
    assert!(token.moved() > 0 && token.moved() <= GOAL);
    assert_eq!(
        client.try_withdraw_funds(&campaign_id),
        Err(Ok(crate::Error::FundsAlreadyWithdrawn)),
        "a completed withdrawal must not be repeatable"
    );
}

/// Re-entering `withdraw_funds` from inside its own payout must not pay twice.
#[test]
fn test_withdraw_funds_reentry_cannot_double_withdraw() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, attacker) = hostile_campaign(
        &env,
        &creator,
        &contributor,
        &token_admin,
        &client,
        Some(Attack::WithdrawFunds),
    );
    pass_deadline(&env, &client, campaign_id);

    let outcome = client.try_withdraw_funds(&campaign_id);

    assert_reentry_refused(&outcome, &attacker, GOAL);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(
        campaign.funds_withdrawn,
        outcome.is_ok(),
        "withdrawal state disagrees with the call outcome"
    );
}

/// Trying to cancel a campaign mid-payout (to open refunds to contributors
/// while the creator is being paid) must not succeed.
#[test]
fn test_withdraw_funds_reentry_cannot_cancel_mid_payout() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, attacker) = hostile_campaign(
        &env,
        &creator,
        &contributor,
        &token_admin,
        &client,
        Some(Attack::CancelCampaign),
    );
    pass_deadline(&env, &client, campaign_id);

    let outcome = client.try_withdraw_funds(&campaign_id);

    assert_reentry_refused(&outcome, &attacker, GOAL);
    let campaign = client.get_campaign(&campaign_id);
    assert!(
        !(campaign.is_cancelled && campaign.funds_withdrawn),
        "campaign is both cancelled (refundable) and withdrawn (paid out)"
    );
}

/// Claiming a refund from inside the creator's payout would pay one pot of
/// money to both sides.
#[test]
fn test_withdraw_funds_reentry_cannot_claim_refund_mid_payout() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, attacker) = hostile_campaign(
        &env,
        &creator,
        &contributor,
        &token_admin,
        &client,
        Some(Attack::ClaimRefund),
    );
    pass_deadline(&env, &client, campaign_id);

    let outcome = client.try_withdraw_funds(&campaign_id);

    assert_reentry_refused(&outcome, &attacker, GOAL);
}

/// Sets up two 50% milestones, both verified, on an already-funded campaign.
fn verified_milestones(env: &Env, admin: &Address, client: &ProofOfHeartClient, campaign_id: u32) {
    let mut milestones = soroban_sdk::Vec::new(env);
    for id in 1..=2u32 {
        milestones.push_back(crate::Milestone {
            id,
            description: String::from_str(env, "half"),
            payout_bps: 5000,
            verified: false,
        });
    }
    client.set_milestones(&campaign_id, &milestones);
    client.verify_milestone(admin, &campaign_id, &1);
    client.verify_milestone(admin, &campaign_id, &2);
}

/// Control for the milestone flow: milestone 1 pays out, and only its share.
#[test]
fn test_control_unarmed_milestone_claim_pays_its_share() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, token) =
        hostile_campaign(&env, &creator, &contributor, &token_admin, &client, None);
    verified_milestones(&env, &admin, &client, campaign_id);

    client.claim_milestone(&campaign_id, &1);

    assert!(token.transfers() >= 1);
    assert!(token.moved() > 0 && token.moved() <= GOAL / 2);
    assert_eq!(
        client.try_claim_milestone(&campaign_id, &1),
        Err(Ok(crate::Error::MilestoneAlreadyClaimed))
    );
}

/// Milestone payouts: re-entering `claim_milestone` for the same milestone
/// must not release its share twice.
#[test]
fn test_claim_milestone_reentry_cannot_double_claim() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, attacker) = hostile_campaign(
        &env,
        &creator,
        &contributor,
        &token_admin,
        &client,
        Some(Attack::ClaimMilestone),
    );
    verified_milestones(&env, &admin, &client, campaign_id);

    let outcome = client.try_claim_milestone(&campaign_id, &1);

    // Milestone 1 is worth half the escrow; a double claim would move more.
    assert_reentry_refused(&outcome, &attacker, GOAL / 2);
}

/// Re-entering `withdraw_funds` from a milestone payout would bypass the
/// proportional release entirely (and is refused for milestone campaigns
/// anyway).
#[test]
fn test_claim_milestone_reentry_cannot_withdraw_everything() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let (campaign_id, attacker) = hostile_campaign(
        &env,
        &creator,
        &contributor,
        &token_admin,
        &client,
        Some(Attack::WithdrawFunds),
    );
    verified_milestones(&env, &admin, &client, campaign_id);
    pass_deadline(&env, &client, campaign_id);

    let outcome = client.try_claim_milestone(&campaign_id, &1);

    assert_reentry_refused(&outcome, &attacker, GOAL / 2);
}
