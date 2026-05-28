use super::*;
use soroban_sdk::{testutils::Ledger, Address, Env, testutils::Address as _, testutils::Events as _};

use crate::test::setup_env;

// ── #266 migrate ──────────────────────────────────────────────────────────────

#[test]
fn test_migrate_success() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    // version is 1 after init; migrate from 1 → CONTRACT_VERSION (1)
    let result = client.try_migrate(&admin, &1u32);
    assert!(result.is_ok());
    assert_eq!(client.get_version(), 1u32); // CONTRACT_VERSION = 1
}

#[test]
fn test_migrate_wrong_version_fails() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_migrate(&admin, &99u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_migrate_double_run_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::Version, &0u32);
    });
    client.migrate(&admin, &0u32);
    // version is now CONTRACT_VERSION; calling again with old version fails
    let result = client.try_migrate(&admin, &0u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_migrate_non_admin_fails() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let stranger = Address::generate(&env);
    let result = client.try_migrate(&stranger, &1u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #267 two-step token update ────────────────────────────────────────────────

fn setup_second_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract(admin.clone())
}

#[test]
fn test_propose_token_update_stores_pending() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    // pending token is set; original token unchanged
    assert_ne!(client.get_token(), new_token);
}

#[test]
fn test_accept_token_update_before_delay_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    // try to accept immediately — should fail
    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_accept_token_update_after_delay_succeeds() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);

    // advance time by 7 days + 1 second
    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    client.accept_token_update(&admin);
    assert_eq!(client.get_token(), new_token);
}

#[test]
fn test_cancel_token_update_clears_pending() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    client.cancel_token_update(&admin);

    // after cancel, accept should fail (no pending)
    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_cancel_token_update_no_pending_fails() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_cancel_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_propose_token_update_non_admin_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    let stranger = Address::generate(&env);
    let result = client.try_propose_token_update(&stranger, &new_token);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #268 O(1) platform stats ──────────────────────────────────────────────────

fn make_campaign_params_simple(env: &Env, creator: &Address) -> CreateCampaignParams {
    CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(env, "T"),
        description: soroban_sdk::String::from_str(env, "D"),
        funding_goal: 1,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    }
}

#[test]
fn test_platform_stats_after_create() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator));
    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 0);
    assert_eq!(stats.verified_campaigns, 0);
}

#[test]
fn test_platform_stats_after_cancel() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.cancel_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.cancelled_campaigns, 1);
}

#[test]
fn test_platform_stats_after_verify() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.verify_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.verified_campaigns, 1);
    // campaign is still active after verify
    assert_eq!(stats.active_campaigns, 1);
}

#[test]
fn test_platform_stats_after_withdraw() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.verify_campaign(&id);

    token_admin.mint(&contributor, &1000);
    // funding_goal is 1, contribute 1 to meet it
    client.contribute(&id, &contributor, &1);

    // advance past deadline
    env.ledger().with_mut(|l| {
        l.timestamp += 31 * 86400;
    });

    client.withdraw_funds(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
}

// ── #269 category list limit cap ─────────────────────────────────────────────

#[test]
fn test_get_campaigns_by_category_capped_at_list_max_limit() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    // Create 60 campaigns in the Learner category
    for _ in 0..60 {
        client.create_campaign(&make_campaign_params_simple(&env, &creator));
    }

    // Request 1000 — should be capped at LIST_MAX_LIMIT (50)
    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &1000u32);
    assert!(result.len() <= 50);
    assert_eq!(result.len(), 50);
}

#[test]
fn test_get_campaigns_by_category_small_limit_respected() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    for _ in 0..10 {
        client.create_campaign(&make_campaign_params_simple(&env, &creator));
    }
    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &5u32);
    assert_eq!(result.len(), 5);
}

// ── #348 resume_campaign spurious events/state writes ─────────────────────────

#[test]
fn test_resume_campaign_rejects_when_contract_not_paused() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    let events_before = env.events().all().len();
    let result = client.try_resume_campaign(&campaign_id, &creator);
    let events_after = env.events().all().len();

    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(events_before, events_after);
}

#[test]
fn test_resume_campaign_clears_auto_pause_when_active() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::AutoPaused, &true);
    });

    assert!(client.is_paused());
    client.resume_campaign(&campaign_id, &creator);
    assert!(!client.is_paused());
}

// ── #353 pause checks ──
#[test]
fn test_paused_admin_parameter_setting_functions_fail() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    client.pause();

    // set_campaign_fee_override should fail when paused
    let result_fee = client.try_set_campaign_fee_override(&admin, &campaign_id, &100u32);
    assert_eq!(result_fee.unwrap_err().unwrap(), Error::ContractPaused);

    // set_creation_disabled should fail when paused
    let result_disabled = client.try_set_creation_disabled(&true);
    assert_eq!(result_disabled.unwrap_err().unwrap(), Error::ContractPaused);
}

// ── #355 set_personal_cap limits check ──
#[test]
fn test_set_personal_cap_cannot_exceed_max_contribution_per_user() {
    let (env, _, creator, contributor, _, _, _, client) = setup_env();

    // Create a campaign with a max_contribution_per_user cap of 500
    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: soroban_sdk::String::from_str(&env, "T"),
        description: soroban_sdk::String::from_str(&env, "D"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 500,
    };
    let campaign_id = client.create_campaign(&params);

    // Setting personal cap equal to or less than 500 should succeed
    let res1 = client.try_set_personal_cap(&campaign_id, &contributor, &500);
    assert!(res1.is_ok());

    // Setting personal cap greater than 500 should fail
    let res2 = client.try_set_personal_cap(&campaign_id, &contributor, &501);
    assert_eq!(res2.unwrap_err().unwrap(), Error::ValidationFailed);
}

// ── #354 vote weight checked addition ──
#[test]
fn test_vote_weight_overflow_fails() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    // Mint contributor tokens
    token_admin.mint(&contributor, &1000);

    // Set high weight manually in storage to simulate a whale or accumulation that would overflow i128::MAX
    env.as_contract(&client.address, || {
        env.storage().persistent().set(&DataKey::ApproveWeight(campaign_id), &(i128::MAX - 500));
    });

    // Cast a vote with balance 501, which overflows i128::MAX when added to i128::MAX - 500
    token_admin.mint(&contributor, &501);
    
    // cast vote should return Overflow error
    let res = client.try_vote_on_campaign(&campaign_id, &contributor, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::Overflow);
}
