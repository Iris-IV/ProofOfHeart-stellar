#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::errors::Error;
use crate::{ProofOfHeart, ProofOfHeartClient};

fn create_test_token(env: &Env) -> Address {
    env.register_contract(None, crate::tests::token::Token)
}

fn create_contract(env: &Env) -> ProofOfHeartClient {
    ProofOfHeartClient::new(env, &env.register_contract(None, ProofOfHeart))
}

#[test]
fn test_settlement_status_before_deadline() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(&crate::CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Test Campaign"),
        description: soroban_sdk::String::from_str(&env, "Description"),
        funding_goal,
        duration_days: 30,
        category: crate::types::Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    contract.verify_campaign(&campaign_id);

    // Check status before deadline passes
    let (deadline_passed, goal_met, ready_for_withdrawal) =
        contract.check_settlement_status(&campaign_id).unwrap();

    assert_eq!(deadline_passed, false, "Deadline should not have passed");
    assert_eq!(goal_met, false, "Goal should not be met (no contributions)");
    assert_eq!(ready_for_withdrawal, false, "Not ready for withdrawal");
}

#[test]
fn test_settlement_status_goal_met_deadline_passed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    // Create campaign with 1-day duration
    let campaign_id = contract.create_campaign(&crate::CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Test Campaign"),
        description: soroban_sdk::String::from_str(&env, "Description"),
        funding_goal,
        duration_days: 1,
        category: crate::types::Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    contract.verify_campaign(&campaign_id);

    // Contribute to meet goal
    let contributor = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor, &1000i128);

    // Advance time past deadline (1 day + buffer)
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + (2 * 24 * 60 * 60);
    });

    // Check status after deadline with goal met
    let (deadline_passed, goal_met, ready_for_withdrawal) =
        contract.check_settlement_status(&campaign_id).unwrap();

    assert_eq!(deadline_passed, true, "Deadline should have passed");
    assert_eq!(goal_met, true, "Goal should be met");
    assert_eq!(ready_for_withdrawal, true, "Should be ready for withdrawal");
}

#[test]
fn test_settlement_status_goal_not_met_deadline_passed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(&crate::CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Test Campaign"),
        description: soroban_sdk::String::from_str(&env, "Description"),
        funding_goal,
        duration_days: 1,
        category: crate::types::Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    contract.verify_campaign(&campaign_id);

    // Contribute but don't meet goal
    let contributor = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor, &500i128); // Only half

    // Advance time past deadline
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + (2 * 24 * 60 * 60);
    });

    // Check status: deadline passed but goal not met
    let (deadline_passed, goal_met, ready_for_withdrawal) =
        contract.check_settlement_status(&campaign_id).unwrap();

    assert_eq!(deadline_passed, true, "Deadline should have passed");
    assert_eq!(goal_met, false, "Goal should not be met (only 500/1000)");
    assert_eq!(
        ready_for_withdrawal, false,
        "Not ready for withdrawal (goal not met)"
    );
}

#[test]
fn test_settlement_status_already_withdrawn() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(&crate::CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Test Campaign"),
        description: soroban_sdk::String::from_str(&env, "Description"),
        funding_goal,
        duration_days: 1,
        category: crate::types::Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    contract.verify_campaign(&campaign_id);

    let contributor = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor, &1000i128);

    // Advance past deadline
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + (2 * 24 * 60 * 60);
    });

    // Withdraw funds
    contract.withdraw_funds(&campaign_id).unwrap();

    // Check status after withdrawal
    let (deadline_passed, goal_met, ready_for_withdrawal) =
        contract.check_settlement_status(&campaign_id).unwrap();

    assert_eq!(deadline_passed, true, "Deadline should have passed");
    assert_eq!(goal_met, true, "Goal should be met");
    assert_eq!(
        ready_for_withdrawal, false,
        "Not ready (already withdrawn)"
    );
}

#[test]
fn test_settlement_status_campaign_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    // Try to check status for non-existent campaign
    let result = contract.try_check_settlement_status(&9999);

    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));
}

#[test]
fn test_settlement_status_cancelled_campaign() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token, &0);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(&crate::CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Test Campaign"),
        description: soroban_sdk::String::from_str(&env, "Description"),
        funding_goal,
        duration_days: 30,
        category: crate::types::Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    contract.verify_campaign(&campaign_id);

    // Cancel campaign
    contract.cancel_campaign(&campaign_id).unwrap();

    // Check status of cancelled campaign
    let (deadline_passed, goal_met, ready_for_withdrawal) =
        contract.check_settlement_status(&campaign_id).unwrap();

    // Cancelled campaigns return false for ready_for_withdrawal
    assert_eq!(
        ready_for_withdrawal, false,
        "Cancelled campaigns not ready for withdrawal"
    );
}
