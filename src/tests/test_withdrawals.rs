use super::helpers::*;
use crate::{
    storage, Category, ContributionKey, Error, BPS_CEIL_OFFSET, BPS_DENOMINATOR, SECONDS_PER_DAY,
};
use soroban_sdk::testutils::{Events, Ledger};
use soroban_sdk::{Address, String, TryFromVal};

// ── withdraw_funds ──────────────────────────────────────────────────────────────

#[test]
fn test_withdraw_before_deadline_goal_not_met_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Early Withdraw"),
        String::from_str(&env, "Desc"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Deadline guard (#854): withdrawal is rejected before the deadline.
    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlineNotPassed);
}

#[test]
fn test_withdraw_after_deadline_goal_not_met_returns_typed_error() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Late Withdraw"),
        String::from_str(&env, "Desc"),
        10_000,
        1,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);
}

#[test]
fn test_withdraw_funds_requires_verified_campaign() {
    let (env, _admin, creator, _contributor1, _, _token, token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Unverified Campaign"),
        String::from_str(&env, "Description"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    token_admin.mint(&contract_id, &1500);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.amount_raised = 1500;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let result = client.try_withdraw_funds(&campaign_id);
    assert_eq!(result.unwrap_err().unwrap(), Error::CampaignNotVerified);
}

#[test]
fn test_withdraw_funds_succeeds_when_verified() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Verified Campaign"),
        String::from_str(&env, "Description"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1500);

    // Advance the ledger past the campaign deadline (#854).
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    assert!(client.try_withdraw_funds(&campaign_id).is_ok());
}

#[test]
fn test_withdraw_funds_uses_effective_amount_after_refunds_for_fee() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &1_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Effective withdrawal"),
        String::from_str(&env, "Fees exclude refunded contributions"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &1_000);

    // Model the post-refund accounting state: historical raised total remains
    // 1_000 while only 700 is still available for withdrawal.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.effective_amount_raised = 700;
        storage::set_campaign(&env, campaign_id, &campaign);
        storage::set_total_raised_global(&env, 700);
    });

    // Advance the ledger past the campaign deadline (#854).
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 21);
    assert_eq!(token.balance(&creator), 679);
}

#[test]
fn test_claim_refund_removes_contribution_storage_key() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Refund storage cleanup"),
        String::from_str(&env, "Contribution key should be removed"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1_000);
    client.cancel_campaign(&campaign_id);

    env.as_contract(&client.address, || {
        assert!(env
            .storage()
            .persistent()
            .has(&ContributionKey::Contribution(
                campaign_id,
                contributor1.clone()
            )));
    });

    client.claim_refund(&campaign_id, &contributor1);

    env.as_contract(&client.address, || {
        assert!(!env
            .storage()
            .persistent()
            .has(&ContributionKey::Contribution(
                campaign_id,
                contributor1.clone()
            )));
    });
}

#[test]
fn test_view_function_get_campaign_not_found() {
    let (_env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let res = client.try_get_campaign(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_withdraw_funds_overflow_returns_error_not_panic() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    // 10% fee so `amount_raised * fee_bps` can overflow at extreme values.
    client.update_platform_fee(&1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Withdraw Overflow"),
        String::from_str(&env, "amount_raised * fee must not panic"),
        1_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1_000);

    // Force a pathological amount_raised that overflows the fee multiplication.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.amount_raised = i128::MAX;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::Overflow);
}

// ── withdrawal vesting & reserve ────────────────────────────────────────────────

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
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&creator), 776);
    assert_eq!(token.balance(&admin), 30);

    let res = client.try_withdraw_reserve(&campaign_id);
    assert!(res.is_err());

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * SECONDS_PER_DAY;
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
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
    });

    client.withdraw_funds(&campaign_id);

    let reserve = client
        .get_campaign_reserve(&campaign_id)
        .expect("reserve should exist after withdraw_funds");
    assert_eq!(reserve.amount, 194);
    assert!(!reserve.released);
    assert_eq!(
        reserve.release_timestamp,
        env.ledger().timestamp() + 7 * SECONDS_PER_DAY
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
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
    });
    client.withdraw_funds(&campaign_id);

    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * SECONDS_PER_DAY;
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

    // 2. Set a prior vesting config, then update it and assert the event records old/new values.
    client.set_vesting_params(&admin, &7, &2000);
    client.set_vesting_params(&admin, &14, &2500);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic_str: soroban_sdk::String =
        soroban_sdk::String::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
    assert_eq!(
        topic_str,
        soroban_sdk::String::from_str(&env, "vesting_params_updated")
    );
    let admin_in_topics: Address = soroban_sdk::FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(admin_in_topics, admin);

    let data: (u64, u64, u32, u32) = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data, (7, 14, 2000, 2500));

    // 3. Try setting both to 0 - should succeed and emit vesting_disabled event.
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

    let data: (u64, u64, u32, u32) = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data, (14, 0, 2500, 0));
}

/// Regression test for issue #466: vesting params set AFTER campaign creation
/// must NOT retroactively affect campaigns already created.
#[test]
fn test_vesting_snapshot_not_affected_by_later_changes() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();

    // No vesting set yet — default is (0, 0).
    let campaign_id_1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign Before Vesting"),
        String::from_str(&env, "Created before vesting was enabled"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id_1);

    // Verify campaign 1 has (0, 0) vesting snapshotted
    env.as_contract(&client.address, || {
        let vesting = storage::get_campaign_vesting(&env, campaign_id_1);
        assert_eq!(vesting, Some((0, 0)));
    });

    // Now enable vesting: 7 days delay, 20% reserve.
    client.set_vesting_params(&admin, &7, &2000);

    // Create campaign #2 after vesting was enabled.
    let campaign_id_2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign After Vesting"),
        String::from_str(&env, "Created after vesting was enabled"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id_2);

    // Verify campaign 2 has (7, 2000) vesting snapshotted
    env.as_contract(&client.address, || {
        let vesting_2 = storage::get_campaign_vesting(&env, campaign_id_2);
        assert_eq!(vesting_2, Some((7, 2000)));
    });

    // Fund both campaigns
    token_admin.mint(&contributor, &2000);
    client.contribute(&campaign_id_1, &contributor, &1000);
    client.contribute(&campaign_id_2, &contributor, &1000);

    // Fast forward past deadline
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
    });

    // Withdraw campaign #1 — should have NO vesting (0% reserve).
    // Goal: 1000. Fee (300 bps): 30. Remaining: 970. Reserve (0%): 0.
    // Creator gets full 970 immediately.
    client.withdraw_funds(&campaign_id_1);
    assert_eq!(token.balance(&creator), 970);

    // Withdraw campaign #2 — should have 20% vesting.
    // Goal: 1000. Fee (300 bps): 30. Remaining: 970. Reserve (20%): 194.
    // Creator gets 776 immediately, 194 later.
    client.withdraw_funds(&campaign_id_2);
    assert_eq!(token.balance(&creator), 970 + 776);

    // Campaign 1's reserve should be None (no reserve was withheld)
    assert_eq!(client.get_campaign_reserve(&campaign_id_1), None);

    // Campaign 2 should have a reserve
    let reserve_2 = client
        .get_campaign_reserve(&campaign_id_2)
        .expect("campaign 2 should have reserve");
    assert_eq!(reserve_2.amount, 194);
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
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
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

    // data payload should be a tuple (gross, fee_amount, reserve_amount, creator_amount)
    // Goal: 1000. Fee (3% default = 300 bps): 30. Remaining: 970.
    // Reserve (20% of 970): 194. Immediate: 776.
    let data: (i128, i128, i128, i128) = soroban_sdk::FromVal::from_val(&env, &withdraw_event.2);
    assert_eq!(data, (1000, 30, 194, 776));
}

// ── BPS_CEIL_OFFSET ─────────────────────────────────────────────────────────────

#[test]
fn test_bps_ceil_offset_value() {
    assert_eq!(BPS_CEIL_OFFSET, 9_999);
    // Ceiling division property: (a + BPS_CEIL_OFFSET) / BPS_DENOMINATOR == ceil(a / BPS_DENOMINATOR)
    assert_eq!(BPS_CEIL_OFFSET / BPS_DENOMINATOR as i128, 0); // ceil(0/10000) = 0
    assert_eq!((1 + BPS_CEIL_OFFSET) / BPS_DENOMINATOR as i128, 1); // ceil(1/10000) = 1
    assert_eq!((9999 + BPS_CEIL_OFFSET) / BPS_DENOMINATOR as i128, 1); // ceil(9999/10000) = 1
    assert_eq!((10000 + BPS_CEIL_OFFSET) / BPS_DENOMINATOR as i128, 1); // ceil(10000/10000) = 1
    assert_eq!((10001 + BPS_CEIL_OFFSET) / BPS_DENOMINATOR as i128, 2); // ceil(10001/10000) = 2
}
/// Regression test for #459: a migration-planted CampaignReserve on a campaign
/// with `funds_withdrawn == false` must NOT be drainable.
#[test]
fn test_withdraw_reserve_rejects_reserve_on_non_withdrawn_campaign() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    // Enable vesting so we can create a legitimate reserve later.
    client.set_vesting_params(&admin, &7, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Plant Reserve No Withdraw"),
        String::from_str(
            &env,
            "Reserve planted before withdraw leaves funds_unwithdrawn==true",
        ),
        1000,
        30,
        Category::EducationalStartup,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // Fund the campaign so it can be withdrawn.
    token_admin.mint(&contributor, &1000);
    client.contribute(&campaign_id, &contributor, &1000);

    // Fast-forward past deadline so withdraw_funds would succeed.
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 31 * SECONDS_PER_DAY;
    });

    // ── Part 1: Plant a CampaignReserve on a campaign that has NOT yet
    // ── withdrawn funds, simulating a migration artefact or bug.
    let future_release = env.ledger().timestamp() + 7 * SECONDS_PER_DAY;
    let planted_reserve = crate::types::CampaignReserve {
        amount: 194,
        release_timestamp: future_release,
        released: false,
    };
    env.as_contract(&client.address, || {
        storage::set_campaign_reserve(&env, campaign_id, &planted_reserve);
    });

    // Verify the planted reserve exists.
    assert_eq!(
        client.get_campaign_reserve(&campaign_id),
        Some(planted_reserve.clone())
    );

    // Attempting to withdraw the reserve should fail because funds_withdrawn is false.
    let res = client.try_withdraw_reserve(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // ── Part 2: The normal withdraw → reserve flow must still work.
    client.withdraw_funds(&campaign_id);

    // Now the campaign has funds_withdrawn == true and a legitimate reserve.
    // Fast-forward past the reserve release delay.
    let current_ts = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current_ts + 8 * SECONDS_PER_DAY;
    });

    // Withdraw reserve should now succeed.
    client.withdraw_reserve(&campaign_id);
    let reserve = client
        .get_campaign_reserve(&campaign_id)
        .expect("reserve should exist after withdraw_reserve");
    assert!(reserve.released);
}
