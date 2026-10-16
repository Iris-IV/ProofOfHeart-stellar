//! #1045: refunds for a failed campaign unlock only after a configurable grace
//! period following the deadline (48 hours by default).

use super::helpers::*;
use crate::{Error, MAX_REFUND_GRACE_PERIOD_SECS, REFUND_GRACE_PERIOD_SECS, SECONDS_PER_DAY};

/// Creates a campaign with a 1_000 goal and has `contributor` put in 400, so
/// the campaign fails once its deadline passes. Returns `(campaign_id, deadline)`.
fn failing_campaign(
    env: &Env,
    creator: &Address,
    contributor: &Address,
    token_admin: &TokenAdminClient,
    client: &ProofOfHeartClient,
) -> (u32, u64) {
    token_admin.mint(contributor, &1_000);
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Grace period campaign"),
        String::from_str(env, "Misses its goal on purpose"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, contributor, &400);
    (id, client.get_campaign(&id).deadline)
}

fn set_time(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|l| l.timestamp = timestamp);
}

#[test]
fn test_default_grace_period_is_48_hours() {
    let (_env, _admin, _, _, _, _, _, client) = setup_env();
    assert_eq!(REFUND_GRACE_PERIOD_SECS, 2 * SECONDS_PER_DAY);
    assert_eq!(
        client.get_refund_grace_period_secs(),
        REFUND_GRACE_PERIOD_SECS
    );
}

#[test]
fn test_refund_locked_during_grace_period() {
    let (env, _admin, creator, contributor, _, token, token_admin, client) = setup_env();
    let (id, deadline) = failing_campaign(&env, &creator, &contributor, &token_admin, &client);

    assert_eq!(
        client.get_refund_unlock_time(&id),
        deadline + REFUND_GRACE_PERIOD_SECS
    );

    // Just after the deadline: the campaign has failed, but refunds are locked.
    set_time(&env, deadline + 1);
    let res = client.try_claim_refund(&id, &contributor);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Last second of the grace period: still locked.
    set_time(&env, deadline + REFUND_GRACE_PERIOD_SECS);
    let res = client.try_claim_refund(&id, &contributor);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(client.get_contribution(&id, &contributor), 400);

    // One second later the refund goes through.
    set_time(&env, deadline + REFUND_GRACE_PERIOD_SECS + 1);
    client.claim_refund(&id, &contributor);
    assert_eq!(client.get_contribution(&id, &contributor), 0);
    assert_eq!(token.balance(&contributor), 1_000);
}

#[test]
fn test_portfolio_refundable_flag_respects_grace_period() {
    let (env, _admin, creator, contributor, _, _, token_admin, client) = setup_env();
    let (_id, deadline) = failing_campaign(&env, &creator, &contributor, &token_admin, &client);

    set_time(&env, deadline + 1);
    let (page, _) = client.get_contributor_portfolio(&contributor, &0, &10);
    assert!(!page.get(0).unwrap().3);

    set_time(&env, deadline + REFUND_GRACE_PERIOD_SECS + 1);
    let (page, _) = client.get_contributor_portfolio(&contributor, &0, &10);
    assert!(page.get(0).unwrap().3);
}

#[test]
fn test_cancelled_campaign_refunds_skip_grace_period() {
    let (env, _admin, creator, contributor, _, token, token_admin, client) = setup_env();
    let (id, _deadline) = failing_campaign(&env, &creator, &contributor, &token_admin, &client);

    // Cancelled before the deadline: the refund is available straight away.
    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor);
    assert_eq!(token.balance(&contributor), 1_000);
}

#[test]
fn test_admin_can_configure_grace_period() {
    let (env, admin, creator, contributor, _, _, token_admin, client) = setup_env();
    let (id, deadline) = failing_campaign(&env, &creator, &contributor, &token_admin, &client);

    client.set_refund_grace_period_secs(&admin, &(7 * SECONDS_PER_DAY));
    assert_eq!(client.get_refund_grace_period_secs(), 7 * SECONDS_PER_DAY);
    assert_eq!(
        client.get_refund_unlock_time(&id),
        deadline + 7 * SECONDS_PER_DAY
    );

    // The old 48h window is no longer enough.
    set_time(&env, deadline + REFUND_GRACE_PERIOD_SECS + 1);
    assert!(client.try_claim_refund(&id, &contributor).is_err());

    set_time(&env, deadline + 7 * SECONDS_PER_DAY + 1);
    client.claim_refund(&id, &contributor);
}

#[test]
fn test_zero_grace_period_restores_immediate_refunds() {
    let (env, admin, creator, contributor, _, _, token_admin, client) = setup_env();
    let (id, deadline) = failing_campaign(&env, &creator, &contributor, &token_admin, &client);

    client.set_refund_grace_period_secs(&admin, &0);
    set_time(&env, deadline + 1);
    client.claim_refund(&id, &contributor);
}

#[test]
fn test_grace_period_bounds_and_auth() {
    let (_env, admin, creator, _, _, _, _, client) = setup_env();

    client.set_refund_grace_period_secs(&admin, &MAX_REFUND_GRACE_PERIOD_SECS);
    let res = client.try_set_refund_grace_period_secs(&admin, &(MAX_REFUND_GRACE_PERIOD_SECS + 1));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    let res = client.try_set_refund_grace_period_secs(&creator, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
    assert_eq!(
        client.get_refund_grace_period_secs(),
        MAX_REFUND_GRACE_PERIOD_SECS
    );
}

#[test]
fn test_refund_unlock_time_for_unknown_campaign() {
    let (_env, _admin, _, _, _, _, _, client) = setup_env();
    let res = client.try_get_refund_unlock_time(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}
