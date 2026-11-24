// Tests use soroban-sdk testutils APIs deprecated in SDK 22
#![allow(deprecated)]
//! Regression tests for #1438 — total_raised_global must not sum contributions
//! across different tokens.
//!
//! Before the fix, every contribution added to a single `TotalRaised` instance
//! key regardless of token, making XLM + USDC into a meaningless mixed number.
//! After the fix:
//!   * Platform-token campaigns continue to update `TotalRaised` (the global).
//!   * Every campaign also updates `TotalRaisedByToken(token)`.
//!   * `get_total_raised_global` returns only the platform-token total.
//!   * `get_total_raised_by_token(token)` returns the per-currency total.

use super::helpers::*;
use crate::{Category, CreateCampaignParams};
use soroban_sdk::{token::StellarAssetClient as TokenAdminClient, Address, String};

fn second_currency<'a>(
    env: &soroban_sdk::Env,
    admin: &Address,
    client: &ProofOfHeartClient<'a>,
) -> (Address, TokenClient<'a>, TokenAdminClient<'a>) {
    let addr = env.register_stellar_asset_contract(admin.clone());
    client.set_token_allowed(&addr, &true);
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        TokenAdminClient::new(env, &addr),
    )
}

fn campaign_params(env: &soroban_sdk::Env, creator: &Address, title: &str) -> CreateCampaignParams {
    CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "Per-token tracking test"),
        funding_goal: 5_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    }
}

/// A platform-token contribution updates both `get_total_raised_global` and
/// `get_total_raised_by_token(platform_token)`.
#[test]
fn test_platform_token_contribution_updates_both_totals() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &5_000);

    let platform = client.get_token();
    let id = client.create_campaign(&campaign_params(&env, &creator, "Platform"));
    client.verify_campaign(&id);

    client.contribute(&id, &contributor, &1_000);

    assert_eq!(client.get_total_raised_global(), 1_000);
    assert_eq!(client.get_total_raised_by_token(&platform), 1_000);
}

/// A non-platform-token contribution updates only
/// `get_total_raised_by_token(alt_token)`, **not** `get_total_raised_global`.
/// This is the core invariant broken by #1438.
#[test]
fn test_alt_token_contribution_does_not_pollute_global_total() {
    let (env, admin, creator, contributor, _, _token, _token_admin, client) = setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);
    usdc_admin.mint(&contributor, &5_000);

    let id = client.create_campaign_with_token(&campaign_params(&env, &creator, "USDC"), &usdc);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor, &2_000);

    // Global must stay at 0 — USDC should not be mixed into platform-token total.
    assert_eq!(client.get_total_raised_global(), 0);
    // Per-token total must reflect the USDC contribution.
    assert_eq!(client.get_total_raised_by_token(&usdc), 2_000);
}

/// Mixed-token platform: global tracks only platform token; per-token totals
/// track each currency independently.
#[test]
fn test_mixed_token_platform_tracks_per_currency_totals() {
    let (env, admin, creator, contributor1, contributor2, _platform, platform_admin, client) =
        setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    let platform = client.get_token();
    platform_admin.mint(&contributor1, &5_000);
    usdc_admin.mint(&contributor2, &5_000);

    let native_id = client.create_campaign(&campaign_params(&env, &creator, "Native"));
    let usdc_id =
        client.create_campaign_with_token(&campaign_params(&env, &creator, "USDC"), &usdc);
    client.verify_campaign(&native_id);
    client.verify_campaign(&usdc_id);

    client.contribute(&native_id, &contributor1, &1_000);
    client.contribute(&usdc_id, &contributor2, &3_000);

    // Global: only the platform-token contribution (1 000), not 1 000 + 3 000.
    assert_eq!(client.get_total_raised_global(), 1_000);

    // Per-token: each currency tracked independently.
    assert_eq!(client.get_total_raised_by_token(&platform), 1_000);
    assert_eq!(client.get_total_raised_by_token(&usdc), 3_000);
}

/// Per-token totals accumulate across multiple contributions from different
/// contributors to the same campaign.
#[test]
fn test_per_token_total_accumulates_across_contributors() {
    let (env, admin, creator, contributor1, contributor2, _platform, _platform_admin, client) =
        setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);
    usdc_admin.mint(&contributor1, &5_000);
    usdc_admin.mint(&contributor2, &5_000);

    let id = client.create_campaign_with_token(&campaign_params(&env, &creator, "Multi"), &usdc);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &1_500);
    client.contribute(&id, &contributor2, &2_000);

    assert_eq!(client.get_total_raised_by_token(&usdc), 3_500);
    // Global untouched.
    assert_eq!(client.get_total_raised_global(), 0);
}

/// A refund decrements the per-token total (failed-funding path).
#[test]
fn test_refund_decrements_per_token_total() {
    let (env, admin, creator, contributor, _, _token, _token_admin, client) = setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);
    usdc_admin.mint(&contributor, &5_000);

    let id = client.create_campaign_with_token(&campaign_params(&env, &creator, "Refund"), &usdc);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor, &2_000);
    assert_eq!(client.get_total_raised_by_token(&usdc), 2_000);

    // Let the campaign expire without meeting the goal.
    advance_past_deadline(&env, &client, id);
    client.claim_refund(&id, &contributor);

    assert_eq!(client.get_total_raised_by_token(&usdc), 0);
    assert_eq!(client.get_total_raised_global(), 0);
}

/// The `accept_token_update` gate correctly checks whether the platform-token
/// total is 0 (using `get_total_raised_global`, which only tracks the platform
/// token). A non-zero alt-token total must not block a platform-token swap.
/// We verify the invariant indirectly: global stays 0 while alt-token total
/// is non-zero, confirming the two counters are independent.
#[test]
fn test_global_stays_zero_when_only_alt_token_contributed() {
    let (env, admin, creator, contributor, _, _token, _token_admin, client) = setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);
    usdc_admin.mint(&contributor, &5_000);

    let id =
        client.create_campaign_with_token(&campaign_params(&env, &creator, "USDC only"), &usdc);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &2_000);

    // global (platform-token) must remain 0 — no platform-token was raised.
    assert_eq!(client.get_total_raised_global(), 0);
    // Per-token reflects the actual USDC raised.
    assert_eq!(client.get_total_raised_by_token(&usdc), 2_000);
    // A further platform-token contribution to a different campaign updates
    // global but does not affect the USDC per-token bucket.
    let platform = client.get_token();
    let (_, platform_admin) = {
        let pa = soroban_sdk::token::StellarAssetClient::new(&env, &platform);
        (platform.clone(), pa)
    };
    platform_admin.mint(&contributor, &1_000);
    let native_id = client.create_campaign(&campaign_params(&env, &creator, "Native"));
    client.verify_campaign(&native_id);
    client.contribute(&native_id, &contributor, &1_000);

    assert_eq!(client.get_total_raised_global(), 1_000);
    assert_eq!(client.get_total_raised_by_token(&platform), 1_000);
    assert_eq!(client.get_total_raised_by_token(&usdc), 2_000);
}
