use super::helpers::*;
use crate::Error;
use soroban_sdk::testutils::Ledger;

#[test]
fn test_full_campaign_lifecycle_active_to_completed() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &5000);

    // 1. Create campaign
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Lifecycle Campaign"),
        String::from_str(&env, "Full lifecycle test"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_active);
    assert!(!campaign.is_verified);
    assert!(!campaign.funds_withdrawn);

    // 2. Verify campaign
    client.verify_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);

    // 3. Contribute tokens
    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, 2000);

    // 4. Fast-forward past deadline
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    // 5. Withdraw funds (goal met)
    client.withdraw_funds(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.funds_withdrawn);
    assert!(!campaign.is_active);
}

#[test]
fn test_lifecycle_campaign_fails_goal_not_reached() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Failing Campaign"),
        String::from_str(&env, "Won't reach goal"),
        5000,
        2,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    // Cannot withdraw — goal not reached
    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    // Contributor can claim refund after deadline
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
}

#[test]
fn test_lifecycle_cancel_and_refund() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled Campaign"),
        String::from_str(&env, "Will be cancelled"),
        3000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1500);

    // Creator cancels
    client.cancel_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);

    // Contributor refunds
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
#![cfg(test)]

use crate::tests::helpers::{setup_contract, setup_token};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_multi_step_sequence() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = setup_token(&env, &admin);
    let client = setup_contract(&env, &admin, &token.address);

    let creator = Address::generate(&env);
    let params = crate::types::CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Test"),
        description: String::from_str(&env, "Desc"),
        funding_goal: 100_000,
        duration_days: 10,
        category: crate::types::Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };

    let id = client.create_campaign(&params);
    client.cancel_campaign(&id);
}
