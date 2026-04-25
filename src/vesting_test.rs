#![cfg(test)]
use crate::test::setup_env;
use crate::types::{Category, CreateCampaignParams};
use soroban_sdk::testutils::{Ledger, Address as _};
use soroban_sdk::{Address, Env, IntoVal, Symbol};

#[test]
fn test_withdrawal_vesting_full_flow() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();
    
    // 1. Setup vesting params: 7 days delay, 20% reserve (2000 bps)
    client.set_vesting_params(&admin, &7, &2000);
    
    // 2. Create and verify campaign
    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Vesting Campaign"),
        description: soroban_sdk::String::from_str(&env, "Test vesting"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };
    let campaign_id = client.create_campaign(&params);
    client.verify_campaign(&campaign_id);
    
    // 3. Contribute to meet goal
    token_admin.mint(&contributor, &1000);
    client.contribute(&campaign_id, &contributor, &1000);
    
    // 4. Fast forward to deadline
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * 86400;
    });
    
    // 5. Withdraw funds
    // Goal: 1000. Fee (3% default): 30. Remaining: 970.
    // Reserve (20% of 970): 194. Immediate: 776.
    client.withdraw_funds(&campaign_id);
    
    assert_eq!(token.balance(&creator), 776);
    assert_eq!(token.balance(&admin), 30); // Platform fee
    
    // 6. Try to withdraw reserve before delay - should fail
    let res = client.try_withdraw_reserve(&campaign_id);
    assert!(res.is_err());
    
    // 7. Fast forward past delay (7 days)
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * 86400;
    });
    
    // 8. Withdraw reserve
    client.withdraw_reserve(&campaign_id);
    assert_eq!(token.balance(&creator), 970); // 776 + 194
}

#[test]
fn test_set_vesting_params_authorization() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let non_admin = Address::generate(&env);
    
    let res = client.try_set_vesting_params(&non_admin, &7, &2000);
    assert!(res.is_err());
}
