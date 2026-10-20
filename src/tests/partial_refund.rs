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
fn test_partial_refund_when_contribution_exceeds_goal() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token);

    // Create campaign with 1000 token goal
    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(
        &creator,
        &soroban_sdk::String::from_str(&env, "Test Campaign"),
        &soroban_sdk::String::from_str(&env, "Description"),
        &funding_goal,
        &30u32, // 30 days
        &crate::types::Category::Learner,
        &false,
        &0u32,
        &0i128, // no per-user cap
    );

    // Verify campaign
    contract.verify_campaign(&campaign_id);

    // First contributor adds 700 tokens (remaining: 300)
    let contributor1 = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor1, &700i128);

    let campaign = contract.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, 700);

    // Second contributor tries to add 500 tokens
    // Should accept 300 (remaining capacity) and refund 200
    let contributor2 = Address::generate(&env);

    // This should succeed with partial contribution
    contract.contribute(&campaign_id, &contributor2, &500i128);

    // Verify campaign is now at exactly the goal
    let campaign = contract.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, funding_goal);

    // Verify contributor2's contribution is 300 (not 500)
    let contribution = contract.get_contribution(&campaign_id, &contributor2);
    assert_eq!(contribution, 300);

    // Note: In a real test, we would verify the token transfers
    // (700 from contributor1, 500 from contributor2, 200 refund to contributor2)
    // This requires mocking the token contract's transfer events
}

#[test]
fn test_reject_contribution_when_goal_already_met() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token);

    // Create campaign with 1000 token goal
    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(
        &creator,
        &soroban_sdk::String::from_str(&env, "Test Campaign"),
        &soroban_sdk::String::from_str(&env, "Description"),
        &funding_goal,
        &30u32,
        &crate::types::Category::Learner,
        &false,
        &0u32,
        &0i128,
    );

    contract.verify_campaign(&campaign_id);

    // Fill campaign to exactly the goal
    let contributor1 = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor1, &1000i128);

    let campaign = contract.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, funding_goal);

    // Try to contribute again - should fail with FundingGoalReached
    let contributor2 = Address::generate(&env);
    let result = contract.try_contribute(&campaign_id, &contributor2, &100i128);

    assert_eq!(result, Err(Ok(Error::FundingGoalReached)));
}

#[test]
fn test_full_contribution_accepted_when_within_capacity() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(
        &creator,
        &soroban_sdk::String::from_str(&env, "Test Campaign"),
        &soroban_sdk::String::from_str(&env, "Description"),
        &funding_goal,
        &30u32,
        &crate::types::Category::Learner,
        &false,
        &0u32,
        &0i128,
    );

    contract.verify_campaign(&campaign_id);

    // Contribute amount that fits completely
    let contributor = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor, &800i128);

    let campaign = contract.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, 800);

    let contribution = contract.get_contribution(&campaign_id, &contributor);
    assert_eq!(contribution, 800);

    // Should still have room for 200 more
    let remaining = funding_goal - campaign.amount_raised;
    assert_eq!(remaining, 200);
}

#[test]
fn test_exact_goal_contribution_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = create_test_token(&env);
    let contract = create_contract(&env);

    contract.init(&admin, &token);

    let creator = Address::generate(&env);
    let funding_goal = 1000i128;

    let campaign_id = contract.create_campaign(
        &creator,
        &soroban_sdk::String::from_str(&env, "Test Campaign"),
        &soroban_sdk::String::from_str(&env, "Description"),
        &funding_goal,
        &30u32,
        &crate::types::Category::Learner,
        &false,
        &0u32,
        &0i128,
    );

    contract.verify_campaign(&campaign_id);

    // Contribute exactly the goal amount
    let contributor = Address::generate(&env);
    contract.contribute(&campaign_id, &contributor, &funding_goal);

    let campaign = contract.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, funding_goal);

    // No refund should have occurred (full amount accepted)
    let contribution = contract.get_contribution(&campaign_id, &contributor);
    assert_eq!(contribution, funding_goal);
}
