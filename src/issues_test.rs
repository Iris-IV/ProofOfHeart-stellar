use super::*;
use soroban_sdk::{testutils::Ledger, Address, Env};

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
    let (_, admin, _, _, _, _, _, client) = setup_env();
    client.migrate(&admin, &1u32);
    // version is now CONTRACT_VERSION; calling again with old version fails
    let result = client.try_migrate(&admin, &1u32);
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
