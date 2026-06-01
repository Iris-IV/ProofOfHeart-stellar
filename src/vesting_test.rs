#![cfg(test)]
use crate::test::setup_env;
use crate::types::{Category, CreateCampaignParams};
use crate::Error;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::Address;

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

    // 3. No reserve should exist before withdraw_funds.
    assert_eq!(client.get_campaign_reserve(&campaign_id), None);

    // 4. Contribute to meet goal
    token_admin.mint(&contributor, &1000);
    client.contribute(&campaign_id, &contributor, &1000);

    // 5. Fast forward to deadline
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
fn test_get_campaign_reserve_view_function() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    client.set_vesting_params(&admin, &7, &2000);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Reserve Getter Campaign"),
        description: soroban_sdk::String::from_str(&env, "Test campaign reserve getter"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };
    let campaign_id = client.create_campaign(&params);
    client.verify_campaign(&campaign_id);

    assert_eq!(client.get_campaign_reserve(&campaign_id), None);

    token_admin.mint(&contributor, &1000);
    client.contribute(&campaign_id, &contributor, &1000);

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * 86400;
    });

    client.withdraw_funds(&campaign_id);

    let reserve = client
        .get_campaign_reserve(&campaign_id)
        .expect("reserve should exist after withdraw_funds");
    assert_eq!(reserve.amount, 194);
    assert!(!reserve.released);
    assert_eq!(
        reserve.release_timestamp,
        env.ledger().timestamp() + 7 * 86400
    );
}

#[test]
fn test_set_vesting_params_authorization() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let non_admin = Address::generate(&env);

    let res = client.try_set_vesting_params(&non_admin, &7, &2000);
    assert!(res.is_err());
}

#[test]
fn test_withdraw_reserve_when_paused_fails() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    client.set_vesting_params(&admin, &7, &2000);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Paused Reserve Campaign"),
        description: soroban_sdk::String::from_str(&env, "Pause guard for reserve"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };
    let campaign_id = client.create_campaign(&params);
    client.verify_campaign(&campaign_id);

    token_admin.mint(&contributor, &1000);
    client.contribute(&campaign_id, &contributor, &1000);

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * 86400;
    });
    client.withdraw_funds(&campaign_id);

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * 86400;
    });

    client.pause();
    let res = client.try_withdraw_reserve(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}
