//! #1056: a creator can change the funding goal until the first contribution.

use super::helpers::*;
use crate::{Error, CAMPAIGN_FUNDING_GOAL_MAX, SECONDS_PER_DAY};

fn campaign(env: &Env, creator: &Address, client: &ProofOfHeartClient) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Adjustable goal"),
        String::from_str(env, "Creator may resize the target"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0,
    ))
}

#[test]
fn test_creator_can_raise_and_lower_goal_before_first_contribution() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);

    client.update_campaign_goal(&id, &5_000);
    assert_eq!(client.get_campaign(&id).funding_goal, 5_000);

    client.update_campaign_goal(&id, &200);
    assert_eq!(client.get_campaign(&id).funding_goal, 200);

    // Same value is a no-op, not an error.
    client.update_campaign_goal(&id, &200);
    assert_eq!(client.get_campaign(&id).funding_goal, 200);
}

#[test]
fn test_goal_frozen_once_contributions_flow() {
    let (env, _admin, creator, contributor, _, _, token_admin, client) = setup_env();
    let id = campaign(&env, &creator, &client);
    client.verify_campaign(&id);

    token_admin.mint(&contributor, &100);
    client.contribute(&id, &contributor, &10);

    assert!(client.try_update_campaign_goal(&id, &2_000).is_err());
    assert_eq!(client.get_campaign(&id).funding_goal, 1_000);
}

#[test]
fn test_goal_frozen_once_funded_even_if_unverified() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);

    // Simulate the state a first contribution leaves behind without routing
    // through `contribute`, which requires verification.
    env.as_contract(&client.address, || {
        let mut c = crate::storage::get_campaign(&env, id).unwrap();
        c.amount_raised = 10;
        crate::storage::set_campaign(&env, id, &c);
    });
    let res = client.try_update_campaign_goal(&id, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // The fee-recipient snapshot alone (first contribution later refunded)
    // also freezes the goal.
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Second campaign"),
        String::from_str(&env, "Creator may resize the target"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    env.as_contract(&client.address, || {
        let admin = crate::storage::get_admin(&env);
        crate::storage::set_campaign_fee_recipient(&env, id2, &admin);
    });
    let res = client.try_update_campaign_goal(&id2, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_goal_frozen_after_verification() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);
    client.verify_campaign(&id);

    let res = client.try_update_campaign_goal(&id, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
}

#[test]
fn test_goal_frozen_after_deadline_and_cancel() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);

    env.ledger()
        .with_mut(|l| l.timestamp += 30 * SECONDS_PER_DAY);
    let res = client.try_update_campaign_goal(&id, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);

    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);
    client.cancel_campaign(&id);
    let res = client.try_update_campaign_goal(&id, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_new_goal_respects_platform_bounds() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);

    let res = client.try_update_campaign_goal(&id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);
    let res = client.try_update_campaign_goal(&id, &-5);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);
    let res = client.try_update_campaign_goal(&id, &(CAMPAIGN_FUNDING_GOAL_MAX + 1));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);

    client.set_min_campaign_funding_goal(&admin, &500);
    let res = client.try_update_campaign_goal(&id, &499);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooLow);

    assert_eq!(client.get_campaign(&id).funding_goal, 1_000);
}

#[test]
fn test_goal_update_requires_creator_auth() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);

    client.update_campaign_goal(&id, &2_000);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, creator);
}

#[test]
fn test_goal_update_on_missing_campaign() {
    let (_env, _admin, _, _, _, _, _, client) = setup_env();
    let res = client.try_update_campaign_goal(&42, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_paused_contract_rejects_goal_update() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client);
    client.pause();
    let res = client.try_update_campaign_goal(&id, &2_000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}
