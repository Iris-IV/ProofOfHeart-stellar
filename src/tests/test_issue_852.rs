//! Regression coverage for issue #852 — reserve payouts must be auditable from
//! the event stream alone.
//!
//! The reserve lifecycle spans two events:
//!
//! * `reserve_withheld` — emitted by `withdraw_funds` when part of the escrow
//!   is set aside as the creator's vested reserve.
//! * `reserve_released` — emitted by `withdraw_reserve` when that reserve is
//!   finally paid out to the creator.
//!
//! Both must name the campaign id *and* the creator in their topics and carry
//! the exact reserve amount as data, so off-chain accounting can attribute a
//! reserve payout without diffing token balances.

use super::helpers::*;
use crate::{Category, SECONDS_PER_DAY};
use soroban_sdk::testutils::{Events, Ledger};
use soroban_sdk::{Address, FromVal, String, TryFromVal};

/// Vesting configuration used by these tests: 20% reserve, released 7 days
/// after `withdraw_funds`.
const RESERVE_BPS: u32 = 2000;
const DELAY_DAYS: u64 = 7;

/// Contribution used by these tests. With the 300 bps platform fee set by
/// `setup_env`, `withdraw_funds` splits this into:
///   fee = 30, reserve = 194 (20% of 970), immediate payout = 776.
const CONTRIBUTION: i128 = 1000;
const EXPECTED_RESERVE: i128 = 194;

/// Finds the first emitted event whose leading topic is `name`.
fn find_event<'a>(
    env: &'a Env,
    name: &str,
) -> (
    Address,
    soroban_sdk::Vec<soroban_sdk::Val>,
    soroban_sdk::Val,
) {
    let expected = String::from_str(env, name);
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(env, &v).ok())
                .map(|topic| topic == expected)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("{} event must be emitted", name))
}

/// Creates a verified campaign with vesting enabled and funds it with
/// `CONTRIBUTION`, returning the campaign id. The caller advances the ledger
/// to the deadline before withdrawing.
fn funded_campaign(
    env: &Env,
    admin: &Address,
    creator: &Address,
    contributor: &Address,
    token_admin: &TokenAdminClient,
    client: &ProofOfHeartClient,
) -> u32 {
    client.set_vesting_params(admin, &DELAY_DAYS, &RESERVE_BPS);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Reserve Audit Campaign"),
        String::from_str(env, "Reserve lifecycle events must be self-describing"),
        CONTRIBUTION,
        30,
        Category::EducationalStartup,
        false,
        0,
        0,
    ));
    client.verify_campaign(&campaign_id);

    token_admin.mint(contributor, &CONTRIBUTION);
    client.contribute(&campaign_id, contributor, &CONTRIBUTION);

    campaign_id
}

#[test]
fn test_reserve_withheld_event_carries_campaign_creator_and_amount() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    let campaign_id = funded_campaign(&env, &admin, &creator, &contributor, &token_admin, &client);

    let now = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = now + 31 * SECONDS_PER_DAY;
    });
    client.withdraw_funds(&campaign_id);

    let withheld = find_event(&env, "reserve_withheld");
    let topics = &withheld.1;

    assert_eq!(
        topics.len(),
        3,
        "reserve_withheld must name the campaign id and the creator"
    );

    let topic_campaign_id: u32 = FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(topic_campaign_id, campaign_id);

    let topic_creator: Address = FromVal::from_val(&env, &topics.get(2).unwrap());
    assert_eq!(topic_creator, creator);

    let amount: i128 = FromVal::from_val(&env, &withheld.2);
    assert_eq!(amount, EXPECTED_RESERVE);
}

#[test]
fn test_reserve_released_event_carries_campaign_creator_and_amount() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();

    let campaign_id = funded_campaign(&env, &admin, &creator, &contributor, &token_admin, &client);

    let now = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = now + 31 * SECONDS_PER_DAY;
    });
    client.withdraw_funds(&campaign_id);

    // Move past the vesting delay so the reserve becomes releasable.
    let now = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = now + (DELAY_DAYS + 1) * SECONDS_PER_DAY;
    });

    let balance_before = token.balance(&creator);
    client.withdraw_reserve(&campaign_id);
    let balance_after = token.balance(&creator);

    let released = find_event(&env, "reserve_released");
    let topics = &released.1;

    assert_eq!(
        topics.len(),
        3,
        "reserve_released must name the campaign id and the creator"
    );

    let topic_campaign_id: u32 = FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(topic_campaign_id, campaign_id);

    let topic_creator: Address = FromVal::from_val(&env, &topics.get(2).unwrap());
    assert_eq!(topic_creator, creator);

    let amount: i128 = FromVal::from_val(&env, &released.2);
    assert_eq!(amount, EXPECTED_RESERVE);

    // The event amount must equal the balance actually credited, so indexers
    // can trust the payload for accounting without a balance diff.
    assert_eq!(balance_after - balance_before, amount);
    assert_eq!(balance_after, 970);
}
