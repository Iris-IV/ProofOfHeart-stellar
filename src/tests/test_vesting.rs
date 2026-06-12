use super::helpers::*;
use crate::Error;
use soroban_sdk::testutils::{Events, Ledger};
use soroban_sdk::{Address, TryFromVal};

#[test]
fn test_withdrawal_vesting_full_flow() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();

    client.set_vesting_params(&admin, &7, &2000);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Vesting Campaign"),
        description: String::from_str(&env, "Test vesting"),
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

    assert_eq!(token.balance(&creator), 776);
    assert_eq!(token.balance(&admin), 30);

    let res = client.try_withdraw_reserve(&campaign_id);
    assert!(res.is_err());

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * 86400;
    });

    client.withdraw_reserve(&campaign_id);
    assert_eq!(token.balance(&creator), 970);
}

#[test]
fn test_get_campaign_reserve_view_function() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    client.set_vesting_params(&admin, &7, &2000);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Reserve Getter Campaign"),
        description: String::from_str(&env, "Test campaign reserve getter"),
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
        title: String::from_str(&env, "Paused Reserve Campaign"),
        description: String::from_str(&env, "Pause guard for reserve"),
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

#[test]
fn test_set_vesting_params_validation_and_disabled_event() {
    let (env, admin, _, _, _, _, _, client) = setup_env();

    // 1. Try setting delay_days = 0 with reserve_bps > 0 - should fail with InvalidVestingDelay
    let res = client.try_set_vesting_params(&admin, &0, &2000);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidVestingDelay);

    // 2. Try setting both to 0 - should succeed and emit vesting_disabled event
    client.set_vesting_params(&admin, &0, &0);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic_str: soroban_sdk::String =
        soroban_sdk::String::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
    assert_eq!(
        topic_str,
        soroban_sdk::String::from_str(&env, "vesting_disabled")
    );
    let admin_in_topics: Address = soroban_sdk::FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(admin_in_topics, admin);

    let data: () = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data, ());
}

#[test]
fn test_withdraw_event_payload_tuple() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    // Setup vesting params: 7 days delay, 20% reserve (2000 bps)
    client.set_vesting_params(&admin, &7, &2000);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "Withdraw Event"),
        description: soroban_sdk::String::from_str(&env, "Test event data"),
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

    // Fast forward to deadline
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * 86400;
    });

    client.withdraw_funds(&campaign_id);

    // Filter events for "withdrawal"
    let events = env.events().all();
    let withdraw_event = events
        .iter()
        .find(|event| {
            let topics = &event.1;
            if topics.len() >= 3 {
                let topic_str =
                    soroban_sdk::String::try_from_val(&env, &topics.get(0).unwrap()).ok();
                topic_str == Some(soroban_sdk::String::from_str(&env, "withdrawal"))
            } else {
                false
            }
        })
        .expect("should find withdrawal event");

    // data payload should be a tuple (fee_amount, creator_amount, reserve_amount)
    // Goal: 1000. Fee (3% default): 30. Remaining: 970.
    // Reserve (20% of 970): 194. Immediate: 776.
    let data: (i128, i128, i128) = soroban_sdk::FromVal::from_val(&env, &withdraw_event.2);
    assert_eq!(data, (30, 776, 194));
}
