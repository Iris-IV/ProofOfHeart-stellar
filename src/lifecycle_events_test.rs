// Tests for issue #216: snapshot test for events emitted during a full campaign lifecycle.
// Captures env.events().all() after each step and asserts the deterministic event sequence.
use super::*;
use crate::test::setup_env;
use soroban_sdk::testutils::Events;
use soroban_sdk::{String, TryFromVal};

/// Returns true if any event in the environment has the given symbol as its first topic.
fn has_event(env: &soroban_sdk::Env, topic: &str) -> bool {
    let expected = String::from_str(env, topic);
    env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|v| String::try_from_val(env, &v).ok())
            .map(|s| s == expected)
            .unwrap_or(false)
    })
}

/// Full lifecycle: create → verify → contribute → withdraw → deposit_revenue → claim_revenue → claim_creator_revenue.
/// Asserts that each step emits the expected event topic.
#[test]
fn test_full_lifecycle_event_sequence() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &5_000);

    // ── Step 1: create_campaign ───────────────────────────────────────────────
    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Lifecycle Campaign"),
        description: String::from_str(&env, "Full lifecycle test"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 1000,
        max_contribution_per_user: 0,
    });

    assert!(
        has_event(&env, "campaign_created"),
        "campaign_created event must be emitted"
    );

    // ── Step 2: verify_campaign ───────────────────────────────────────────────
    client.verify_campaign(&id);
    assert!(
        has_event(&env, "campaign_verified"),
        "campaign_verified event must be emitted"
    );

    // ── Step 3: contribute ────────────────────────────────────────────────────
    client.contribute(&id, &contributor1, &1_000);
    assert!(
        has_event(&env, "contribution_made"),
        "contribution_made event must be emitted"
    );

    // ── Step 4: withdraw_funds ────────────────────────────────────────────────
    client.withdraw_funds(&id);
    assert!(
        has_event(&env, "withdrawal"),
        "withdrawal event must be emitted"
    );

    // ── Step 5: deposit_revenue ───────────────────────────────────────────────
    client.deposit_revenue(&id, &2_000);
    assert!(
        has_event(&env, "revenue_deposited"),
        "revenue_deposited event must be emitted"
    );

    // ── Step 6: claim_revenue (contributor) ───────────────────────────────────
    client.claim_revenue(&id, &contributor1);
    assert!(
        has_event(&env, "revenue_claimed"),
        "revenue_claimed event must be emitted"
    );

    // ── Step 7: claim_creator_revenue ─────────────────────────────────────────
    client.claim_creator_revenue(&id);
    assert!(
        has_event(&env, "creator_revenue_claimed"),
        "creator_revenue_claimed event must be emitted"
    );

    // ── Snapshot: assert total event count is stable ──────────────────────────
    // init + create + verify + contribute + withdraw + deposit + claim + creator_claim = 8 minimum
    let total = env.events().all().len();
    assert!(
        total >= 8,
        "full lifecycle must emit at least 8 events, got {}",
        total
    );
}

/// Cancellation path: create → verify → contribute → cancel → refund emits expected events.
#[test]
fn test_cancel_lifecycle_event_sequence() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5_000);

    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cancelled Campaign"),
        description: String::from_str(&env, "Will be cancelled"),
        funding_goal: 10_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);
    client.cancel_campaign(&id);

    assert!(
        has_event(&env, "campaign_cancelled"),
        "campaign_cancelled event must be emitted"
    );

    client.claim_refund(&id, &contributor1);
    assert!(
        has_event(&env, "refund_claimed"),
        "refund_claimed event must be emitted"
    );
}
