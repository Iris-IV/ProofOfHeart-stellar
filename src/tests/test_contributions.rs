use super::helpers::*;
use crate::{
    storage, Category, ContributionKey, CreateCampaignParams, Error, RevenueKey, StorageKey,
    SECONDS_PER_DAY,
};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation},
    Address, IntoVal, String, Symbol,
};

// ── contribute & basic failure states ───────────────────────────────────────────

#[test]
fn test_contribute_and_withdraw_success() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Code Camp"),
        String::from_str(&env, "Learn Rust"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    assert_eq!(token.balance(&contributor1), 4000);
    assert_eq!(token.balance(&client.address), 1000);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1000);

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);

    let campaign = client.get_campaign(&campaign_id);
    assert!(!campaign.is_active);
    assert!(campaign.funds_withdrawn);
}

#[test]
fn test_creator_cannot_contribute_to_own_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Self Funding Block"),
        String::from_str(&env, "Creator should not contribute"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_contribute(&campaign_id, &creator, &100);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_failure_states() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let duration_days = 2;
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Deadline Test"),
        String::from_str(&env, "Desc"),
        1000,
        duration_days,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.contribute(&campaign_id, &contributor1, &500);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (duration_days * 86450),
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
}

#[test]
fn test_multiple_concurrent_campaigns_are_isolated() {
    let (env, _admin, creator1, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    let creator2 = Address::generate(&env);
    let creator3 = Address::generate(&env);

    token_admin.mint(&contributor1, &10000);
    token_admin.mint(&contributor2, &10000);
    token_admin.mint(&creator3, &10000);

    let campaign_1 = client.create_campaign(&make_params(
        creator1.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Educator campaign"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_1);

    let campaign_2 = client.create_campaign(&make_params(
        creator2.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Learner campaign"),
        1500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_2);

    let campaign_3 = client.create_campaign(&make_params(
        creator3.clone(),
        String::from_str(&env, "Campaign 3"),
        String::from_str(&env, "Startup campaign"),
        2000,
        30,
        Category::EducationalStartup,
        true,
        1500,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_3);

    assert_eq!(campaign_1, 1);
    assert_eq!(campaign_2, 2);
    assert_eq!(campaign_3, 3);
    assert_eq!(client.get_campaign_count(), 3);

    client.contribute(&campaign_1, &contributor1, &1000);
    client.contribute(&campaign_2, &contributor1, &400);
    client.contribute(&campaign_2, &contributor2, &500);
    client.contribute(&campaign_3, &contributor1, &1200);
    client.contribute(&campaign_3, &contributor2, &800);

    assert_eq!(client.get_contribution(&campaign_1, &contributor1), 1000);
    assert_eq!(client.get_contribution(&campaign_1, &contributor2), 0);
    assert_eq!(client.get_contribution(&campaign_2, &contributor1), 400);
    assert_eq!(client.get_contribution(&campaign_2, &contributor2), 500);
    assert_eq!(client.get_contribution(&campaign_3, &contributor1), 1200);
    assert_eq!(client.get_contribution(&campaign_3, &contributor2), 800);

    client.withdraw_funds(&campaign_1);

    assert!(client.get_campaign(&campaign_1).funds_withdrawn);
    assert!(!client.get_campaign(&campaign_1).is_active);
    assert_eq!(client.get_campaign(&campaign_2).amount_raised, 900);
    assert!(!client.get_campaign(&campaign_2).funds_withdrawn);
    assert_eq!(client.get_campaign(&campaign_3).amount_raised, 2000);

    client.cancel_campaign(&campaign_2);
    assert!(client.get_campaign(&campaign_2).is_cancelled);
    assert!(client.get_campaign(&campaign_3).is_active);

    client.withdraw_funds(&campaign_3);
    assert!(client.get_campaign(&campaign_3).funds_withdrawn);
    assert!(!client.get_campaign(&campaign_3).is_active);

    client.deposit_revenue(&campaign_3, &3000);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);
    assert_eq!(client.get_revenue_pool(&campaign_3), 3000);

    assert_eq!(token.balance(&client.address), 3900);
    assert_eq!(token.balance(&creator3), 8940);
}

#[test]
fn test_deadline_boundary() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Boundary Test"),
        String::from_str(&env, "Testing exact deadline boundary"),
        1000,
        2,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let deadline = client.get_campaign(&campaign_id).deadline;

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

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

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_contribution_accounting_invariant() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    let contributor3 = Address::generate(&env);

    token_admin.mint(&contributor1, &3000);
    token_admin.mint(&contributor2, &3000);
    token_admin.mint(&contributor3, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Invariant Campaign"),
        String::from_str(&env, "Accounting invariant check"),
        5000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &500);
    client.contribute(&campaign_id, &contributor2, &750);
    client.contribute(&campaign_id, &contributor3, &250);
    client.contribute(&campaign_id, &contributor1, &300);
    client.contribute(&campaign_id, &contributor2, &200);

    let c1 = client.get_contribution(&campaign_id, &contributor1);
    let c2 = client.get_contribution(&campaign_id, &contributor2);
    let c3 = client.get_contribution(&campaign_id, &contributor3);

    assert_eq!(c1, 800);
    assert_eq!(c2, 950);
    assert_eq!(c3, 250);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(c1 + c2 + c3, campaign.amount_raised);
}

#[test]
fn test_view_functions_error_handling() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "View Test"),
        String::from_str(&env, "Testing view functions"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let stranger = Address::generate(&env);
    let invalid_id = 999u32;

    let res = client.try_get_campaign(&invalid_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);

    assert_eq!(client.get_contribution(&campaign_id, &stranger), 0);
    assert_eq!(client.get_contribution(&invalid_id, &contributor1), 0);
    assert_eq!(client.get_revenue_pool(&invalid_id), 0);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &stranger), 0);
    assert_eq!(client.get_revenue_claimed(&invalid_id, &contributor1), 0);
}

#[test]
fn test_contribute_one_second_before_deadline() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Almost Deadline"),
        String::from_str(&env, "Desc"),
        1000,
        1,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let deadline = client.get_campaign(&campaign_id).deadline;

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline - 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);
}

#[test]
fn test_batch_contribute_failed_transfer_emits_no_events() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Batch Atomicity"),
        String::from_str(&env, "No events on failed batch transfer"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let events_before = env.events().all().len();
    let res = client.try_batch_contribute(
        &contributor1,
        &soroban_sdk::vec![&env, (campaign_id, 800i128), (campaign_id, 700i128)],
    );
    assert!(res.is_err());
    assert_eq!(env.events().all().len(), events_before);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(client.get_campaign(&campaign_id).amount_raised, 0);
}

// ── Issue #408: checked arithmetic in anomaly detection ───────────────────────

#[test]
fn test_contribute_overflow_returns_error_not_panic() {
    let (env, _admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    // Mint a modest amount; the overflow check triggers before the token transfer.
    token_admin.mint(&contributor1, &1_000_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Overflow Test"),
        String::from_str(&env, "Checked arithmetic campaign"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // i128::MAX * 10000 overflows, so checked_mul must return Err(Overflow)
    // instead of panicking the contract.
    let res = client.try_contribute(&campaign_id, &contributor1, &i128::MAX);
    assert_eq!(res.unwrap_err().unwrap(), Error::Overflow);
}

// ── contribution caps ───────────────────────────────────────────────────────────

#[test]
fn test_contribution_cap_persists_across_refund_recontribution_cycles() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap persistence"),
        String::from_str(&env, "lifetime cap test"),
        2_000,
        1,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &900);
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        900
    );
}

#[test]
fn test_max_contribution_per_user_enforced_across_multiple_transactions() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Multi tx cap"),
        String::from_str(&env, "lifetime cap across txs"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &600);
    let res = client.try_contribute(&campaign_id, &contributor1, &600);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 600);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        600
    );
}

#[test]
fn test_max_contribution_per_transaction_is_admin_configured_and_enforced() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator,
        String::from_str(&env, "Transaction cap"),
        String::from_str(&env, "single contribution limit"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    client.verify_campaign(&campaign_id);

    assert_eq!(client.get_max_contribution_per_tx(), 0);
    client.set_max_contribution_per_tx(&admin, &750);
    assert_eq!(client.get_max_contribution_per_tx(), 750);

    client.contribute(&campaign_id, &contributor1, &750);
    let result = client.try_contribute(&campaign_id, &contributor1, &751);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);

    // The same guard applies to batch contributions, which otherwise bypass
    // the public single-contribution entry point.
    let result = client.try_batch_contribute(
        &contributor1,
        &soroban_sdk::vec![&env, (campaign_id, 751i128)],
    );
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);

    client.set_max_contribution_per_tx(&admin, &0);
    client.contribute(&campaign_id, &contributor1, &751);
}

#[test]
fn test_personal_cap_enforcement() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap Test"),
        String::from_str(&env, "Testing caps"),
        5000,
        30,
        Category::Educator,
        false,
        0,
        1000i128,
    ));
    client.verify_campaign(&campaign_id);

    client.set_personal_cap(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 500);

    client.contribute(&campaign_id, &contributor1, &400);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    let res_set = client.try_set_personal_cap(&campaign_id, &contributor1, &2000);
    assert_eq!(res_set.unwrap_err().unwrap(), Error::ValidationFailed);

    client.set_personal_cap(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor1, &500);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
}

#[test]
fn test_anomaly_auto_pause_huge_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Science Book"),
        String::from_str(&env, "Teaching science to kids"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    let res = client.try_contribute(&campaign_id, &contributor1, &4001);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
    // Rollback ensures it's NOT paused.
    assert!(!client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &100);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 100);
}

#[test]
fn test_anomaly_auto_pause_burst() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Burst Test"),
        String::from_str(&env, "Testing burst"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // #535: burst detection only engages once amount_raised crosses 50% of
    // the funding goal, so push the campaign over that line first. This
    // single contribution itself isn't burst-checked (amount_raised is still
    // 0 going into it), matching the "skip on the happy path" behavior.
    client.contribute(&campaign_id, &contributor1, &1_100);

    for _ in 0..10 {
        client.contribute(&campaign_id, &contributor1, &10);
    }
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_200);

    let res = client.try_contribute(&campaign_id, &contributor1, &10);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
    // Rollback ensures it's NOT paused.
    assert!(!client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_200);

    client.unpause();

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp(),
        protocol_version: 22,
        sequence_number: env.ledger().sequence() + 1,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &10);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_210);
}

/// #535: a campaign that stays below the 50%-of-goal activity threshold
/// never engages burst detection, so it can accept more than
/// `AUTO_PAUSE_BURST_THRESHOLD` contributions in a single ledger without
/// tripping auto-pause.
#[test]
fn test_low_activity_campaign_skips_burst_check() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Low Activity"),
        String::from_str(&env, "Testing burst skip"),
        1_000_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // 15 contributions in the same ledger, well under 50% of the huge goal —
    // would trip AUTO_PAUSE_BURST_THRESHOLD (10) if the burst check ran.
    for _ in 0..15 {
        client.contribute(&campaign_id, &contributor1, &10);
    }

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 150);
    assert!(!client.is_paused());
}

#[test]
fn test_huge_contribution_triggers_auto_pause() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Huge Contribution Test"),
        description: String::from_str(&env, "Testing auto-pause via huge contribution"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    // Anomaly detection fires (the Err rollback means AutoPaused doesn't persist
    // through contribute() itself — test the detection, not the persistence).
    let res = client.try_contribute(&campaign_id, &contributor1, &2001i128);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}

// ── refund claims ───────────────────────────────────────────────────────────────

#[test]
fn test_cancel_and_refund() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&contributor2, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Failed Idea"),
        String::from_str(&env, "Desc"),
        5000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &500);

    client.cancel_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_cancelled);

    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);

    assert_eq!(token.balance(&contributor1), 2000);
    assert_eq!(token.balance(&contributor2), 1000);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_claim_refund_requires_contributor_auth() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Auth Refund"),
        String::from_str(&env, "Only contributor can claim"),
        5000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let (auth_addr, invocation) = &auths[0];
    assert_eq!(auth_addr, &contributor1);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "claim_refund"),
                (campaign_id, contributor1.clone()).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );

    assert_eq!(token.balance(&contributor1), 2000);
}

#[test]
fn test_double_refund_prevention() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Double Refund"),
        String::from_str(&env, "Test double refund"),
        5000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2000);

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
    assert_eq!(token.balance(&contributor1), 2000);
}

#[test]
fn test_refund_requires_deadline_passed_and_goal_missed() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Failed Campaign"),
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

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

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

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
}

#[test]
fn test_no_refund_when_goal_reached() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Successful Campaign"),
        String::from_str(&env, "Desc"),
        500,
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

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

// ── refund edge cases & storage cleanup ─────────────────────────────────────────

#[test]
fn test_claim_refund_state_mutation_order() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Refund Order Test"),
        String::from_str(&env, "Testing state mutation order"),
        10000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1000);
    assert_eq!(token.balance(&contributor1), 4000);
    assert_eq!(token.balance(&client.address), 1000);

    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(token.balance(&client.address), 0);

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_claim_refund_multiple_contributors_isolation() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Multi Refund Test"),
        String::from_str(&env, "Testing multiple refunds"),
        10000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &2000);
    client.contribute(&campaign_id, &contributor2, &1500);
    client.cancel_campaign(&campaign_id);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);

    assert_eq!(client.get_contribution(&campaign_id, &contributor2), 1500);
    assert_eq!(token.balance(&contributor2), 1500);

    client.claim_refund(&campaign_id, &contributor2);
    assert_eq!(client.get_contribution(&campaign_id, &contributor2), 0);
    assert_eq!(token.balance(&contributor2), 3000);
}

#[test]
fn test_claim_refund_expired_campaign() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let duration_days = 2;
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Expired Campaign"),
        String::from_str(&env, "Will expire"),
        10000,
        duration_days,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (duration_days * 86450),
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

#[test]
fn test_claim_refund_clears_existing_revenue_claimed_key() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Refund Cleans Revenue Claim"),
        description: String::from_str(&env, "Ensure RevenueClaimed key is removed"),
        funding_goal: 5000,
        duration_days: 2,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    // Artificially mark funds as withdrawn so deposit/claim_revenue bypass the guard.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    client.deposit_revenue(&campaign_id, &1000);
    client.claim_revenue(&campaign_id, &contributor1);

    let claimed_before_refund = client.get_revenue_claimed(&campaign_id, &contributor1);
    assert!(claimed_before_refund > 0);

    // Advance past deadline so claim_refund accepts failed_due_to_goal
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (2 * 86450),
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    // claim_refund with deadline passed and goal not met clears revenue_claimed.
    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

#[test]
fn test_claim_revenue_after_single_refund_uses_live_raised() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue Refund Denominator"),
        String::from_str(
            &env,
            "Remaining contributor receives full share after refund",
        ),
        2000,
        10,
        Category::EducationalStartup,
        true,
        5000,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);

    // Withdraw funds so funds_withdrawn = true (required for deposit/claim revenue).
    client.withdraw_funds(&campaign_id);
    client.deposit_revenue(&campaign_id, &1000);

    // Simulate a refund for contributor1 via storage: zero out their contribution
    // and reduce effective_amount_raised — without cancelling the campaign.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.effective_amount_raised = 1000;
        storage::set_campaign(&env, campaign_id, &campaign);
        storage::remove_contribution(&env, campaign_id, &contributor1);
    });

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    // Contributor2 can still claim their full revenue share
    // (1000 / 1000 * 5000 / 10000 * 1000 = 500).
    client.claim_revenue(&campaign_id, &contributor2);

    assert_eq!(token.balance(&contributor2), 4500);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor2), 500);
}
fn has_persistent_key(env: &Env, client: &ProofOfHeartClient<'_>, key: impl StorageKey) -> bool {
    env.as_contract(&client.address, || env.storage().persistent().has(&key))
}

#[test]
fn test_storage_cleaned_after_claim_refund_on_cancel() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cleanup Cancel Refund"),
        String::from_str(&env, "Storage cleanup test"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    assert!(
        !has_persistent_key(
            &env,
            &client,
            ContributionKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund"
    );
    assert!(
        !has_persistent_key(
            &env,
            &client,
            RevenueKey::RevenueClaimed(id, contributor1.clone())
        ),
        "RevenueClaimed key must not exist after refund"
    );
}

#[test]
fn test_storage_cleaned_after_claim_refund_on_failed_campaign() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cleanup Failed"),
        String::from_str(&env, "Failed campaign refund"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);

    env.ledger().with_mut(|li| {
        li.timestamp += 31 * SECONDS_PER_DAY;
    });

    client.claim_refund(&id, &contributor1);

    assert!(
        !has_persistent_key(
            &env,
            &client,
            ContributionKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund on failed campaign"
    );
}

// Issue #341: claim_revenue is gated on funds_withdrawn. The prior "claim
// then cancel then refund" flow this test exercised is now structurally
// impossible (cancel is blocked once funds are withdrawn). Covered by
// test::test_claim_revenue_blocked_before_funds_withdrawn.

#[test]
fn test_claim_revenue_amount_raised_zero_guard() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Zero Raised Guard"),
        String::from_str(&env, "Directly test AmountRaisedIsZero guard"),
        1_000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Artificially zero out amount_raised and effective_amount_raised while keeping
    // the contribution in storage. Also force funds_withdrawn=true so we bypass the
    // funds_withdrawn guard and exercise the AmountRaisedIsZero guard specifically.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.amount_raised = 0;
        campaign.effective_amount_raised = 0;
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::AmountRaisedIsZero);
}

#[test]
fn test_claim_refund_preserves_lifetime_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "LT Cleanup"),
        String::from_str(&env, "Test lifetime cleanup"),
        2000,
        1,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &900);

    // Cancel and refund
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        900,
        "LifetimeContribution should persist after refund for cap enforcement"
    );
}

// ── batch_contribute (#518) ─────────────────────────────────────────────────

#[test]
fn test_batch_contribute_multiple_campaigns_single_transfer() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_a = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign A"),
        String::from_str(&env, "First"),
        1_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let campaign_b = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign B"),
        String::from_str(&env, "Second"),
        1_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_a);
    client.verify_campaign(&campaign_b);

    client.batch_contribute(
        &contributor1,
        &soroban_sdk::vec![&env, (campaign_a, 300i128), (campaign_b, 700i128)],
    );

    assert_eq!(client.get_contribution(&campaign_a, &contributor1), 300);
    assert_eq!(client.get_contribution(&campaign_b, &contributor1), 700);
    assert_eq!(token.balance(&contributor1), 5_000 - 1_000);
    assert_eq!(token.balance(&client.address), 1_000);

    let events = env.events().all();
    let summary = events.last().unwrap();
    let payload: (u32, i128) = soroban_sdk::FromVal::from_val(&env, &summary.2);
    assert_eq!(payload, (2, 1_000));
}

#[test]
fn test_batch_contribute_rejects_empty_batch() {
    let (env, _admin, _creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let res = client.try_batch_contribute(&contributor1, &soroban_sdk::vec![&env]);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_batch_contribute_rejects_oversized_batch() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Oversized"),
        String::from_str(&env, "Too many items"),
        1_000_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    let mut items = soroban_sdk::Vec::new(&env);
    for _ in 0..21 {
        items.push_back((campaign_id, 1i128));
    }

    let res = client.try_batch_contribute(&contributor1, &items);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_batch_contribute_reverts_fully_on_invalid_item() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let good_campaign = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Good"),
        String::from_str(&env, "Would succeed alone"),
        1_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&good_campaign);

    // Never created/verified — contributing to it must fail the whole batch.
    let bad_campaign_id = good_campaign + 999;

    let res = client.try_batch_contribute(
        &contributor1,
        &soroban_sdk::vec![&env, (good_campaign, 500i128), (bad_campaign_id, 100i128)],
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);

    // No partial accounting from the first (otherwise-valid) item.
    assert_eq!(client.get_contribution(&good_campaign, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5_000);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_batch_contribute_rejects_duplicate_campaign_ids() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "No Duplicates"),
        String::from_str(&env, "Duplicate campaign IDs rejected"),
        2_000,
        30,
        Category::Educator,
        false,
        0,
        1_000i128,
    ));
    client.verify_campaign(&campaign_id);

    let res = client.try_batch_contribute(
        &contributor1,
        &soroban_sdk::vec![&env, (campaign_id, 600i128), (campaign_id, 500i128)],
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
}
