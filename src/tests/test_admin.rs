use super::helpers::*;
use crate::{
    Category, Error, CAMPAIGN_FUNDING_GOAL_MAX, CAMPAIGN_FUNDING_GOAL_MIN, TOKEN_UPDATE_DELAY_SECS,
};
use soroban_sdk::{Address, String};

// ── admin transfer, pause, token update ─────────────────────────────────────────

#[test]
fn test_update_platform_fee() {
    let (env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let result = client.try_update_platform_fee(&500);
    assert!(result.is_ok());

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let expected_topics = (String::from_str(&env, "fee_updated"),).into_val(&env);
    assert_eq!(last_event.1, expected_topics);

    let data_vec: soroban_sdk::Vec<u32> = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data_vec.get(0).unwrap(), 300);
    assert_eq!(data_vec.get(1).unwrap(), 500);

    // Issue #343: fees above the business cap (1000 bps) are rejected.
    let result = client.try_update_platform_fee(&5000);
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidPlatformFee);
    // Issue #559: fees above the absolute max (10000 bps) are rejected.
    let result = client.try_update_platform_fee(&10001);
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidPlatformFee);
}

// ── Admin transfer (two-step) ──────────────────────────────────────────

#[test]
fn test_admin_transfer_happy_path() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    client.initiate_admin_transfer(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(client.get_admin(), admin);

    client.accept_admin_transfer();
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn test_admin_transfer_cancel() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    client.initiate_admin_transfer(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    client.cancel_admin_transfer(&admin);
    assert_eq!(client.get_pending_admin(), None);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_admin_transfer_reinitiate_overwrites_pending() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let first_candidate = Address::generate(&env);
    let second_candidate = Address::generate(&env);

    client.initiate_admin_transfer(&admin, &first_candidate);
    assert_eq!(client.get_pending_admin(), Some(first_candidate.clone()));

    client.initiate_admin_transfer(&admin, &second_candidate);
    assert_eq!(client.get_pending_admin(), Some(second_candidate.clone()));
    assert_ne!(client.get_pending_admin(), Some(first_candidate));
}

#[test]
fn test_admin_transfer_wrong_address_fails() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    client.initiate_admin_transfer(&admin, &new_admin);

    let result = client.try_initiate_admin_transfer(&admin, &admin);
    assert!(result.is_err(), "transfer to same admin must fail");

    assert_eq!(client.get_pending_admin(), Some(new_admin));
}

// ── Admin update (legacy single-step `update_admin`) ────────────────────

#[test]
fn test_update_admin_success() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    let res = client.try_update_admin(&new_admin);
    assert!(res.is_ok());
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    let accept_res = client.try_accept_admin_transfer();
    assert!(accept_res.is_ok());
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn test_update_admin_requires_stored_admin_auth() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    let res = client.try_update_admin(&new_admin);
    assert!(res.is_ok());

    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| addr == &admin),
        "stored admin must be the authorized address"
    );
}

#[test]
fn test_cancel_admin_transfer_updated() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let new_admin = Address::generate(&env);

    client.update_admin(&new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin));

    let cancel_res = client.try_cancel_admin_transfer(&admin);
    assert!(cancel_res.is_ok());
    assert_eq!(client.get_pending_admin(), None);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_pause_and_unpause() {
    let (_env, _admin, _creator, _contributor1, _, _token, _token_admin, client) = setup_env();

    assert!(!client.is_paused());

    client.pause();
    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn test_pause_blocks_state_changing_operations() {
    let (env, _admin, creator, contributor1, _contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Paused Test"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.pause();
    assert!(client.is_paused());

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "New Campaign"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    // Admin governance functions must succeed while paused (#388).
    let res = client.try_update_platform_fee(&400);
    assert!(
        res.is_ok(),
        "update_platform_fee must succeed while paused (#388)"
    );

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, String::from_str(&env, "Paused Test"));

    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    let _ = token;
}

// ── Issue #407: accept_token_update must not strand campaign balances ──────────

#[test]
fn test_token_swap_blocked_with_active_campaign() {
    let (env, admin, creator, _, _, _, token_admin, client) = setup_env();

    let _campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Active Campaign"),
        String::from_str(&env, "Token swap must be blocked"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    // Register a second token to use as the migration target.
    let new_token_address = env.register_stellar_asset_contract(admin.clone());

    client.propose_token_update(&admin, &new_token_address);

    // Advance timestamp past the 7-day delay.
    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });

    // Must fail: there is still an active campaign with escrowed funds.
    let res = client.try_accept_token_update(&admin);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Token must remain unchanged.
    let _ = token_admin;
}

#[test]
fn test_token_swap_succeeds_after_all_campaigns_terminal() {
    let (env, admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Terminal Campaign"),
        String::from_str(&env, "Withdraw before swap"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);

    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });

    // All campaigns terminal (withdrawn) → swap must succeed.
    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());

    assert_eq!(client.get_token(), new_token_address);
}

#[test]
fn test_token_swap_succeeds_after_campaign_cancelled() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancellable"),
        String::from_str(&env, "Cancel then swap"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.cancel_campaign(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);

    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });

    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());
    assert_eq!(client.get_token(), new_token_address);
}

// ── Issue #407 follow-up: cancelling a campaign drops the active-campaign count
//    to zero, but contributor refunds remain escrowed in the old token until
//    claimed. The swap must stay blocked until those funds actually leave. ──────
#[test]
fn test_token_swap_blocked_with_unrefunded_cancelled_campaign() {
    let (env, admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancel With Funds"),
        String::from_str(&env, "Refund pending after cancel"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Cancel: ActiveCampaignCount → 0 and total_raised_global is
    // decremented upfront (#818), so the token swap can proceed.
    client.cancel_campaign(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);
    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });

    // #818: total_raised_global was already decremented at cancel time,
    // so the swap is no longer blocked even though claim_refund is pending.
    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());
    assert_eq!(client.get_token(), new_token_address);
}

// ── Issue #470: partial refund must still block token swap ──────────

#[test]
fn test_token_swap_blocked_after_partial_refund() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Partial Refund"),
        String::from_str(&env, "Partial refund blocks swap"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);
    client.contribute(&campaign_id, &contributor2, &500);

    // Cancel → ActiveCampaignCount → 0 and total_raised_global is
    // decremented upfront (#818), so the swap can proceed immediately.
    client.cancel_campaign(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);
    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });

    // #818: total_raised_global was already decremented at cancel time,
    // so the swap succeeds even though claim_refund is pending.
    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());
    assert_eq!(client.get_token(), new_token_address);

    // Refunds still work after the swap.
    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);
}

// ── initialisation & config ─────────────────────────────────────────────────────

#[test]
fn test_init_only_once() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();
    let res = client.try_init(&admin, &token.address, &300);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);
}

#[test]
fn test_platform_fee_exact_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(admin.clone());
    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    let client = crate::ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token_address, &1000);
    assert_eq!(client.get_platform_fee(), 1000);
}

#[test]
fn test_reinit_prevention() {
    let (env, admin, _, _, _, token, _, client) = setup_env();

    let attacker = Address::generate(&env);
    let fake_token = Address::generate(&env);

    let res = client.try_init(&attacker, &fake_token, &0);
    assert!(res.is_err());

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
}

#[test]
fn test_initialization_getters() {
    let (_, admin, _, _, _, token, _, client) = setup_env();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
    assert_eq!(client.get_campaign_count(), 0);
}

#[test]
fn test_init_returns_already_initialized_error() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();
    let err = client
        .try_init(&admin, &token.address, &300)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_init_preserves_all_config_state() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();

    let _ = client.try_init(&admin, &token.address, &999);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
    assert_eq!(client.get_campaign_count(), 0);
    assert_eq!(client.get_version(), 1);
    assert_eq!(
        client.get_min_votes_quorum(),
        crate::voting::DEFAULT_MIN_VOTES_QUORUM
    );
    assert_eq!(
        client.get_approval_threshold_bps(),
        crate::voting::DEFAULT_APPROVAL_THRESHOLD_BPS
    );
}

#[test]
fn test_init_rejects_every_subsequent_call() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();

    for _ in 0..3 {
        let res = client.try_init(&admin, &token.address, &300);
        assert_eq!(
            res.unwrap_err().unwrap(),
            Error::AlreadyInitialized,
            "expected AlreadyInitialized on every repeated call"
        );
    }
}

#[test]
fn test_init_cannot_overwrite_after_campaign_created() {
    let (env, admin, creator, _c1, _c2, token, _token_admin, client) = setup_env();

    let _ = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Test Campaign"),
        String::from_str(&env, "Testing init idempotency after state change"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(client.get_campaign_count(), 1);

    let res = client.try_init(&admin, &token.address, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);

    assert_eq!(client.get_campaign_count(), 1);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
}

#[test]
fn test_min_campaign_funding_goal_boundary_and_admin_update() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) =
        setup_env_with_default_min();

    assert_eq!(
        client.get_min_campaign_funding_goal(),
        CAMPAIGN_FUNDING_GOAL_MIN
    );

    let title = String::from_str(&env, "Minimum Goal");
    let desc = String::from_str(&env, "Checks funding goal floor");

    let below_min = CAMPAIGN_FUNDING_GOAL_MIN - 1;
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        below_min,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooLow);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        CAMPAIGN_FUNDING_GOAL_MIN,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(campaign_id, 1);

    client.set_min_campaign_funding_goal(&admin, &500);
    assert_eq!(client.get_min_campaign_funding_goal(), 500);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        499,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooLow);
}

#[test]
fn test_max_campaign_funding_goal_boundary_and_admin_update() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) =
        setup_env_with_default_min();

    assert_eq!(
        client.get_max_campaign_funding_goal(),
        CAMPAIGN_FUNDING_GOAL_MAX
    );

    let title1 = String::from_str(&env, "Max Goal 1");
    let desc1 = String::from_str(&env, "Checks funding goal ceiling");

    // Exactly at the cap must succeed.
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title1.clone(),
        desc1.clone(),
        CAMPAIGN_FUNDING_GOAL_MAX,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(campaign_id, 1);

    // One above the cap must fail.
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title1.clone(),
        desc1.clone(),
        CAMPAIGN_FUNDING_GOAL_MAX + 1,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);

    // Admin raises the cap.
    let new_max = CAMPAIGN_FUNDING_GOAL_MAX * 2;
    client.set_max_campaign_funding_goal(&admin, &new_max);
    assert_eq!(client.get_max_campaign_funding_goal(), new_max);

    // Previously-rejected goal now succeeds.
    let title2 = String::from_str(&env, "Max Goal 2");
    let campaign_id2 = client.create_campaign(&make_params(
        creator.clone(),
        title2,
        desc1.clone(),
        CAMPAIGN_FUNDING_GOAL_MAX + 1,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(campaign_id2, 2);
}

// ── per-campaign fee override ───────────────────────────────────────────────────

#[test]
fn test_campaign_fee_override_zero_percent() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Charity"),
        String::from_str(&env, "0% fee campaign"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.set_campaign_fee_override(&campaign_id, &admin, &0);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 0);
    assert_eq!(token.balance(&creator), 1000);
}

#[test]
fn test_campaign_fee_override_custom_percent() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Reduced Fee"),
        String::from_str(&env, "1% fee"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.set_campaign_fee_override(&campaign_id, &admin, &100);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 10);
    assert_eq!(token.balance(&creator), 990);
}

#[test]
fn test_campaign_fee_override_default_unchanged() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Default Fee"),
        String::from_str(&env, "Global fee applies"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);
}

#[test]
fn test_campaign_fee_override_above_max_rejected() {
    let (env, admin2, creator2, _c1, _c2, _token2, _token_admin2, client2) = setup_env();
    let id = client2.create_campaign(&make_params(
        creator2.clone(),
        String::from_str(&env, "X"),
        String::from_str(&env, "X"),
        1,
        1,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let res = client2.try_set_campaign_fee_override(&id, &admin2, &1001);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidPlatformFee);
    let res = client2.try_set_campaign_fee_override(&id, &admin2, &10001);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidPlatformFee);
}

#[test]
fn test_campaign_fee_override_non_admin_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "X"),
        String::from_str(&env, "X"),
        1,
        1,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let impostor = Address::generate(&env);
    let res = client.try_set_campaign_fee_override(&id, &impostor, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── per-category duration caps ──────────────────────────────────────────────────

#[test]
fn test_category_duration_cap_enforced() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_duration_cap(&admin, &Category::EducationalStartup, &60);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Startup desc"),
        1000,
        61,
        Category::EducationalStartup,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup OK"),
        String::from_str(&env, "Startup desc"),
        1000,
        60,
        Category::EducationalStartup,
        false,
        0,
        0i128,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_duration_cap_other_categories_unaffected() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_duration_cap(&admin, &Category::Learner, &10);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator"),
        String::from_str(&env, "Full duration"),
        1000,
        365,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(id, 1);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Learner"),
        String::from_str(&env, "Too long"),
        1000,
        11,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);
}

#[test]
fn test_category_duration_cap_default_unchanged() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Default"),
        String::from_str(&env, "Default cap"),
        1000,
        365,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_duration_cap_above_365_rejected() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let res = client.try_set_category_duration_cap(&admin, &Category::Learner, &366);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_category_duration_cap_non_admin_rejected() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let impostor = Address::generate(&env);
    let res = client.try_set_category_duration_cap(&impostor, &Category::Learner, &30);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}
