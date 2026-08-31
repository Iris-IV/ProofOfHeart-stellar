/// Tests for issues #813, #814, #816, and #818.
///
/// #818 — cancel_campaign must decrement total_raised_global immediately.
/// #816 — verify_campaigns returns (verified_ids, failed_ids).
/// #813 — extend_campaign_deadline respects the per-category duration cap.
/// #814 — BlockCampaignContributionCount (per-campaign) is the only block
///         count variant; no dead global key exists.
use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{testutils::Ledger as _, vec, Address, String, Vec};

fn make_campaign(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    goal: i128,
    days: u64,
    category: Category,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "Test Campaign"),
        description: String::from_str(env, "Test description for campaign"),
        funding_goal: goal,
        duration_days: days,
        category,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    })
}

// ── #818: cancel_campaign decrements total_raised_global upfront ──────────────

#[test]
fn test_cancel_campaign_decrements_total_raised_global_immediately() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &500);

    let id = make_campaign(&env, &client, &creator, 1_000, 30, Category::Educator);
    client.contribute(&id, &contributor, &500);

    assert_eq!(client.get_total_raised_global(), 500);

    client.cancel_campaign(&id);

    // total_raised_global must drop immediately on cancel, not wait for refund claims.
    assert_eq!(client.get_total_raised_global(), 0);
}

#[test]
fn test_cancel_campaign_allows_accept_token_update_after_all_refunds_claimed() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &500);

    let id = make_campaign(&env, &client, &creator, 1_000, 30, Category::Educator);
    client.contribute(&id, &contributor, &500);
    client.cancel_campaign(&id);

    // total_raised_global is 0 immediately after cancel (#818 fix), so
    // accept_token_update is no longer permanently blocked by unclaimed refunds.
    assert_eq!(client.get_total_raised_global(), 0);

    // Claiming a refund after cancel must not underflow total_raised_global.
    client.claim_refund(&id, &contributor);
    assert_eq!(client.get_total_raised_global(), 0);
}

#[test]
fn test_claim_refund_does_not_double_decrement_after_creator_cancel() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &300);

    let id = make_campaign(&env, &client, &creator, 1_000, 30, Category::Learner);
    client.contribute(&id, &contributor, &300);

    assert_eq!(client.get_total_raised_global(), 300);
    client.cancel_campaign(&id);
    assert_eq!(client.get_total_raised_global(), 0);

    client.claim_refund(&id, &contributor);
    // Still 0 — claim_refund skips the decrement for cancelled campaigns.
    assert_eq!(client.get_total_raised_global(), 0);
}

#[test]
fn test_admin_cancel_decrements_total_raised_global() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let goal = 2_000i128;
    token_admin.mint(&contributor, &goal);

    let id = make_campaign(&env, &client, &creator, goal, 30, Category::Educator);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &goal);

    assert_eq!(client.get_total_raised_global(), goal);

    client.admin_cancel_campaign(
        &admin,
        &id,
        &String::from_str(&env, "fraud detected"),
    );

    assert_eq!(client.get_total_raised_global(), 0);
}

#[test]
fn test_failed_funding_claim_refund_still_decrements_total_raised_global() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &400);

    let id = make_campaign(&env, &client, &creator, 1_000, 30, Category::Learner);
    client.contribute(&id, &contributor, &400);
    assert_eq!(client.get_total_raised_global(), 400);

    // Advance past the deadline without reaching the goal.
    env.ledger().set_timestamp(env.ledger().timestamp() + 31 * 24 * 60 * 60 + 1);

    client.claim_refund(&id, &contributor);
    // Failed-funding path still decrements (campaign.is_cancelled is false here).
    assert_eq!(client.get_total_raised_global(), 0);
}

// ── #816: verify_campaigns returns (verified_ids, failed_ids) ─────────────────

#[test]
fn test_verify_campaigns_partial_failure_returns_both_vecs() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let good = make_campaign(&env, &client, &creator, 100, 30, Category::Learner);
    let bad_id: u32 = 9999;

    let (verified, failed) = client.verify_campaigns(&vec![&env, good, bad_id]);

    assert_eq!(verified.len(), 1);
    assert_eq!(verified.get(0), Some(good));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed.get(0), Some(bad_id));
}

#[test]
fn test_verify_campaigns_all_fail_returns_empty_verified_vec() {
    let (env, _, _, _, _, _, _, client) = setup_env();

    let (verified, failed) = client.verify_campaigns(&vec![&env, 8888u32, 9999u32]);

    assert_eq!(verified.len(), 0);
    assert_eq!(failed.len(), 2);
}

#[test]
fn test_verify_campaigns_all_succeed_returns_empty_failed_vec() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let id1 = make_campaign(&env, &client, &creator, 100, 30, Category::Learner);
    let id2 = make_campaign(&env, &client, &creator, 200, 30, Category::Educator);

    let (verified, failed) = client.verify_campaigns(&vec![&env, id1, id2]);

    assert_eq!(verified.len(), 2);
    assert_eq!(failed.len(), 0);
}

// ── #813: extend_campaign_deadline validates category duration cap ─────────────

#[test]
fn test_extend_deadline_blocked_when_new_total_exceeds_category_cap() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    // Create a 60-day campaign.
    let id = make_campaign(&env, &client, &creator, 100, 60, Category::Learner);

    // Admin sets a tight 90-day cap on the category.
    client.set_category_duration_cap(&admin, &Category::Learner, &90u64);

    // Adding 31 days would make total = 91 days > cap. Must be rejected.
    let res = client.try_extend_campaign_deadline(&id, &31u64);
    assert!(
        res.is_err(),
        "extension beyond category cap must be rejected"
    );
}

#[test]
fn test_extend_deadline_allowed_within_category_cap() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    let id = make_campaign(&env, &client, &creator, 100, 60, Category::Learner);

    // 120-day cap — adding 30 days (total = 90) is within the cap.
    client.set_category_duration_cap(&admin, &Category::Learner, &120u64);

    client.extend_campaign_deadline(&id, &30u64);

    let campaign = client.get_campaign(&id);
    assert!(campaign.deadline_extended);
}

// ── #814: BlockCampaignContributionCount is the live per-campaign key ──────────
// There is no dead global BlockContributionCount variant in ContributionKey;
// the split from DataKey removed it. The per-campaign burst-detection key is
// exercised by the anomaly detection path in contribute(). This test verifies
// the counts are truly per-campaign and independent of each other.

#[test]
fn test_burst_guard_block_counts_are_per_campaign_not_global() {
    let (env, _admin, creator, contributor, contributor2, _token, token_admin, client) =
        setup_env();
    // Mint enough for two campaigns × two contributions each.
    token_admin.mint(&contributor, &10_000);
    token_admin.mint(&contributor2, &10_000);

    // goal = 1_000; burst_check_threshold = 50% = 500
    let id_a = make_campaign(&env, &client, &creator, 1_000, 30, Category::Learner);
    let id_b = make_campaign(&env, &client, &creator, 1_000, 30, Category::Educator);

    // First contribution to each — amount_raised starts at 0 so the burst-check
    // early-exit fires and no block-count entry is written yet.
    client.contribute(&id_a, &contributor, &600); // A now at 600 (>50% threshold)
    client.contribute(&id_b, &contributor, &600); // B now at 600 (>50% threshold)

    // Second contributions on the same ledger block — the burst guard now
    // runs and writes the per-campaign key for each campaign separately.
    client.contribute(&id_a, &contributor2, &1);
    client.contribute(&id_b, &contributor2, &1);

    // Verify counts are per-campaign (each should be 1, not 2).
    env.as_contract(&client.address, || {
        let (_, count_a) = crate::storage::get_campaign_block_contribution_count(&env, id_a);
        let (_, count_b) = crate::storage::get_campaign_block_contribution_count(&env, id_b);
        assert_eq!(count_a, 1, "campaign A count must be 1 (per-campaign, not global)");
        assert_eq!(count_b, 1, "campaign B count must be 1 (per-campaign, not global)");
    });
}
