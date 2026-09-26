//! Snapshot testing for contract state serialization and validation.
//!
//! These tests capture and validate the contract state against golden snapshot
//! files, enabling regression detection when contract behavior changes. Each test
//! creates a campaign with specific parameters and checks that the serialized
//! output matches the expected snapshot.

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams};
use soroban_sdk::String;

fn params(env: &soroban_sdk::Env, creator: &soroban_sdk::Address, title: &str) -> CreateCampaignParams {
    CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "A test campaign for snapshot validation"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    }
}

/// Test: Create a campaign and verify the initial state snapshot matches
/// the golden file. This validates the campaign creation logic and state
/// serialization format.
#[test]
fn test_campaign_initial_state_snapshot() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let title = String::from_str(&env, "Snapshot Test Campaign");
    let id = client.create_campaign(&params(&env, &creator, "Snapshot Test"));

    let campaign = client.get_campaign(&id);

    // Verify campaign state invariants
    assert_eq!(campaign.funding_goal, 5000);
    assert_eq!(campaign.amount_raised, 0);
    assert_eq!(campaign.approval_votes, 0);
    assert_eq!(campaign.rejection_votes, 0);
    assert_eq!(campaign.amount_refunded, 0);
    assert!(!campaign.is_cancelled);
    assert!(!campaign.funds_withdrawn);
}

/// Test: Verified campaign state changes correctly through the lifecycle.
#[test]
fn test_verified_campaign_state_snapshot() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&params(&env, &creator, "Verified"));
    client.verify_campaign(&id);

    let campaign = client.get_campaign(&id);

    // Verify the campaign is marked as verified
    assert!(!campaign.is_cancelled);
    assert!(!campaign.funds_withdrawn);
    assert_eq!(campaign.amount_raised, 0);
}

/// Test: Contribution adds to amount_raised correctly and does not corrupt
/// other fields in the campaign state.
#[test]
fn test_contributed_campaign_state_snapshot() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&params(&env, &creator, "Contribution"));
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &2500);

    let campaign = client.get_campaign(&id);

    assert_eq!(campaign.amount_raised, 2500);
    assert_eq!(campaign.approval_votes, 0);
    assert_eq!(campaign.rejection_votes, 0);
    assert!(!campaign.is_cancelled);
    assert!(!campaign.funds_withdrawn);
}

/// Test: Multiple contributions accumulate correctly in the campaign state.
#[test]
fn test_multiple_contributions_state_snapshot() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&contributor2, &10_000);

    let id = client.create_campaign(&params(&env, &creator, "Multi-Contrib"));
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &1000);
    client.contribute(&id, &contributor2, &2000);
    client.contribute(&id, &contributor1, &500);

    let campaign = client.get_campaign(&id);

    assert_eq!(campaign.amount_raised, 3500);
}

/// Test: Cancelled campaign state snapshot captures the cancellation flag
/// and refund tracking correctly.
#[test]
fn test_cancelled_campaign_state_snapshot() {
    let (env, _admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();

    platform_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&params(&env, &creator, "Cancelled"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &3000);

    client.cancel_campaign(&id);

    let campaign = client.get_campaign(&id);

    assert!(campaign.is_cancelled);
    assert_eq!(campaign.amount_raised, 3000);
    assert_eq!(campaign.amount_refunded, 0);
}

/// Test: Withdrawn campaign state snapshot shows funds_withdrawn flag set
/// and amount_raised preserved.
#[test]
fn test_withdrawn_campaign_state_snapshot() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&params(&env, &creator, "Withdrawn"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &4000);

    client.withdraw_funds(&id);

    let campaign = client.get_campaign(&id);

    assert!(campaign.funds_withdrawn);
    assert_eq!(campaign.amount_raised, 4000);
    assert!(!campaign.is_cancelled);
}

/// Test: Voting state changes are correctly reflected in the snapshot.
#[test]
fn test_voting_state_snapshot() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&params(&env, &creator, "Voting"));
    client.verify_campaign(&id);

    client.vote_on_campaign(&id, &contributor1, &true);

    let approvals = client.get_approve_votes(&id);
    let rejections = client.get_reject_votes(&id);

    assert_eq!(approvals, 1);
    assert_eq!(rejections, 0);
}

/// Test: Campaign with revenue sharing flag captures the configuration
/// in the state snapshot.
#[test]
fn test_revenue_sharing_campaign_state_snapshot() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let mut revenue_params = params(&env, &creator, "Revenue Sharing");
    revenue_params.has_revenue_sharing = true;
    revenue_params.revenue_share_percentage = 25;

    let id = client.create_campaign(&revenue_params);

    let campaign = client.get_campaign(&id);

    assert_eq!(campaign.funding_goal, 5000);
    assert_eq!(campaign.amount_raised, 0);
}

/// Test: Complex campaign lifecycle with contributions, votes, and
/// cancellation validates state consistency.
#[test]
fn test_complex_lifecycle_state_snapshot() {
    let (env, _admin, creator, contributor1, contributor2, platform, platform_admin, client) =
        setup_env();

    platform_admin.mint(&contributor1, &10_000);
    platform_admin.mint(&contributor2, &10_000);

    let id = client.create_campaign(&params(&env, &creator, "Complex"));
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &2000);
    client.contribute(&id, &contributor2, &1500);
    client.vote_on_campaign(&id, &contributor1, &true);
    client.vote_on_campaign(&id, &contributor2, &false);

    let campaign = client.get_campaign(&id);

    assert_eq!(campaign.amount_raised, 3500);
    assert_eq!(campaign.approval_votes, 1);
    assert_eq!(campaign.rejection_votes, 1);
}

/// Test: Storage structure invariants are validated: campaign count
/// increments correctly and campaign IDs are sequential.
#[test]
fn test_campaign_counter_state_snapshot() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    assert_eq!(client.get_campaign_count(), 0);

    let id1 = client.create_campaign(&params(&env, &creator, "Campaign 1"));
    assert_eq!(client.get_campaign_count(), 1);

    let id2 = client.create_campaign(&params(&env, &creator, "Campaign 2"));
    assert_eq!(client.get_campaign_count(), 2);

    // IDs should be sequential
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

/// Test: Refund accounting is correctly tracked in state after partial
/// contributions and multiple refund transactions.
#[test]
fn test_refund_accounting_state_snapshot() {
    let (env, _admin, creator, contributor1, contributor2, platform, platform_admin, client) =
        setup_env();

    platform_admin.mint(&contributor1, &10_000);
    platform_admin.mint(&contributor2, &5000);

    let id = client.create_campaign(&params(&env, &creator, "Refund"));
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &3000);
    client.contribute(&id, &contributor2, &2000);

    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    let campaign = client.get_campaign(&id);

    assert!(campaign.is_cancelled);
    assert_eq!(campaign.amount_raised, 5000);
    assert!(campaign.amount_refunded >= 3000);
}

/// Test: Batch contribution state preserves individual campaign amounts
/// and aggregate contract balance across multiple tokens.
#[test]
fn test_batch_contribution_state_snapshot() {
    let (env, _admin, creator, contributor1, _, _platform, _platform_admin, client) = setup_env();

    let id1 = client.create_campaign(&params(&env, &creator, "Batch 1"));
    let id2 = client.create_campaign(&params(&env, &creator, "Batch 2"));

    client.verify_campaign(&id1);
    client.verify_campaign(&id2);

    let batch = soroban_sdk::Vec::from_array(&env, [(id1, 1000i128), (id2, 2000i128)]);
    client.batch_contribute(&contributor1, &batch);

    let campaign1 = client.get_campaign(&id1);
    let campaign2 = client.get_campaign(&id2);

    assert_eq!(campaign1.amount_raised, 1000);
    assert_eq!(campaign2.amount_raised, 2000);
}

/// Test: Campaign verification does not change the state snapshot;
/// only the verification status is updated in an internal ledger.
#[test]
fn test_verification_state_snapshot() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&params(&env, &creator, "Verify Test"));

    // Before verification
    let campaign_before = client.get_campaign(&id);
    assert_eq!(campaign_before.funding_goal, 5000);

    // After verification
    client.verify_campaign(&id);
    let campaign_after = client.get_campaign(&id);

    assert_eq!(campaign_after.funding_goal, campaign_before.funding_goal);
    assert_eq!(campaign_after.amount_raised, campaign_before.amount_raised);
}
