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
