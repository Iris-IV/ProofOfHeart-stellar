/// Tests for issue #819: cancel_campaign does not zero effective_amount_raised.
///
/// When a campaign is cancelled (by creator or admin), its `effective_amount_raised`
/// must immediately drop to 0 so indexers and dashboards report 0 live contributions,
/// regardless of whether contributors claim refunds.
/// Subsequent `claim_refund` calls must not underflow or decrement below 0.
use super::helpers::*;
use crate::{Category, CreateCampaignParams};
use soroban_sdk::{Address, String};

fn make_campaign(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    title: &str,
    goal: i128,
    days: u64,
    category: Category,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "Test description for campaign"),
        funding_goal: goal,
        duration_days: days,
        category,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    })
}

#[test]
fn test_creator_cancel_campaign_zeros_effective_amount_raised_immediately() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &500);

    let id = make_campaign(
        &env,
        &client,
        &creator,
        "Cancel Zero Effective",
        1_000,
        30,
        Category::Educator,
    );
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &500);

    let campaign_before = client.get_campaign(&id);
    assert_eq!(campaign_before.effective_amount_raised, 500);
    assert_eq!(campaign_before.amount_raised, 500);

    client.cancel_campaign(&id);

    // effective_amount_raised must be 0 immediately upon cancellation.
    let campaign_after_cancel = client.get_campaign(&id);
    assert_eq!(campaign_after_cancel.effective_amount_raised, 0);
    assert_eq!(campaign_after_cancel.amount_raised, 500); // Historical metric preserved

    // Contributor claims refund afterwards; effective_amount_raised stays 0 without underflow.
    client.claim_refund(&id, &contributor);
    let campaign_after_refund = client.get_campaign(&id);
    assert_eq!(campaign_after_refund.effective_amount_raised, 0);
}

#[test]
fn test_admin_cancel_campaign_zeros_effective_amount_raised_immediately() {
    let (env, admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &800);

    let id = make_campaign(
        &env,
        &client,
        &creator,
        "Admin Cancel Zero Effective",
        1_000,
        30,
        Category::Learner,
    );
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &800);

    let campaign_before = client.get_campaign(&id);
    assert_eq!(campaign_before.effective_amount_raised, 800);

    client.admin_cancel_campaign(
        &admin,
        &id,
        &String::from_str(&env, "Fraudulent activity detected"),
    );

    // effective_amount_raised must be 0 immediately upon admin cancellation.
    let campaign_after_cancel = client.get_campaign(&id);
    assert_eq!(campaign_after_cancel.effective_amount_raised, 0);

    // Contributor claims refund afterwards.
    client.claim_refund(&id, &contributor);
    let campaign_after_refund = client.get_campaign(&id);
    assert_eq!(campaign_after_refund.effective_amount_raised, 0);
}

#[test]
fn test_unclaimed_refunds_do_not_leave_stale_effective_amount_raised() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &300);
    token_admin.mint(&contributor2, &400);

    let id = make_campaign(
        &env,
        &client,
        &creator,
        "Multi Contributor Cancel",
        1_000,
        30,
        Category::Educator,
    );
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &300);
    client.contribute(&id, &contributor2, &400);

    let campaign_before = client.get_campaign(&id);
    assert_eq!(campaign_before.effective_amount_raised, 700);

    client.cancel_campaign(&id);

    // Both contributors haven't claimed yet; effective_amount_raised is already 0.
    let campaign_cancelled = client.get_campaign(&id);
    assert_eq!(campaign_cancelled.effective_amount_raised, 0);

    // Only contributor1 claims refund; contributor2 leaves refund unclaimed.
    client.claim_refund(&id, &contributor1);

    let campaign_partial = client.get_campaign(&id);
    assert_eq!(
        campaign_partial.effective_amount_raised, 0,
        "effective_amount_raised must remain 0 even when some refunds remain unclaimed"
    );
}

#[test]
fn test_failed_funding_deadline_passed_decrements_effective_amount_raised() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &400);

    let id = make_campaign(
        &env,
        &client,
        &creator,
        "Failed Funding Refund",
        1_000,
        30,
        Category::Learner,
    );
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &400);

    assert_eq!(client.get_campaign(&id).effective_amount_raised, 400);

    // Advance past deadline without reaching goal.
    env.ledger().with_mut(|li| {
        li.timestamp += 31 * 24 * 60 * 60 + 1;
    });

    // On failed (non-cancelled) campaign, effective_amount_raised is decremented upon refund claim.
    client.claim_refund(&id, &contributor);
    assert_eq!(client.get_campaign(&id).effective_amount_raised, 0);
}
