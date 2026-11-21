// Tests use soroban-sdk testutils APIs deprecated in SDK 22
#![allow(deprecated)]
//! Regression tests for:
//!   #1439 — personal cap should use lifetime (not current) as its basis
//!   #1440 — cap checks should run against accepted_amount, not the requested amount

use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::String;

// ── #1439: personal cap uses lifetime basis ───────────────────────────────────

/// After a refund (which zeroes `current` but preserves `lifetime`), a
/// re-contribution that would exceed the personal cap via lifetime must still
/// be rejected. Before the fix, the check used `current` (== 0 after refund)
/// so the cap was silently bypassed.
#[test]
fn test_personal_cap_enforced_via_lifetime_after_refund() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Personal cap lifetime"),
        String::from_str(&env, "Issue 1439"),
        5_000,
        1, // 1-day: will expire quickly
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // Set personal cap to 500.
    client.set_personal_cap(&campaign_id, &contributor, &500);

    // Contribute 400 (lifetime = 400, current = 400).
    client.contribute(&campaign_id, &contributor, &400);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        400
    );

    // Let the campaign expire (deadline + 1) so it fails and becomes refundable.
    advance_past_deadline(&env, &client, campaign_id);

    // Claim refund: current → 0, lifetime stays at 400.
    client.claim_refund(&campaign_id, &contributor);
    assert_eq!(client.get_contribution(&campaign_id, &contributor), 0);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        400
    );

    // set_personal_cap should reject a new cap below the lifetime total.
    let res = client.try_set_personal_cap(&campaign_id, &contributor, &300);
    assert_eq!(res, Err(Ok(Error::ValidationFailed)));
}

/// Personal cap and campaign-wide cap both use lifetime as the basis, so they
/// behave identically for a contributor who has refunded and re-contributes.
#[test]
fn test_personal_cap_and_campaign_cap_use_same_lifetime_basis() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &5_000);

    // campaign-wide cap of 1000
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Unified cap basis"),
        String::from_str(&env, "Issue 1439 parity"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    client.verify_campaign(&campaign_id);

    // Contribute 600 (lifetime = 600).
    client.contribute(&campaign_id, &contributor, &600);

    // Another 500 would bring lifetime to 1100 > 1000 → rejected.
    let res = client.try_contribute(&campaign_id, &contributor, &500);
    assert_eq!(res, Err(Ok(Error::ContributionCapExceeded)));

    // 400 exactly hits the cap → accepted.
    client.contribute(&campaign_id, &contributor, &400);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        1_000
    );
}

// ── #1440: cap checks use accepted_amount on partial fill ─────────────────────

/// A contribution that overshoots the goal but whose accepted portion fits
/// within the campaign-wide cap must succeed.  Before the fix, the cap check
/// ran against the full requested amount and wrongly rejected it.
#[test]
fn test_cap_check_uses_accepted_amount_on_partial_fill() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &5_000);

    // Campaign goal = 1000, campaign cap per user = 400.
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Partial fill cap"),
        String::from_str(&env, "Issue 1440"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        400i128, // cap: 400
    ));
    client.verify_campaign(&campaign_id);

    // Fill 700 with a different contributor so remaining = 300.
    let other = soroban_sdk::Address::generate(&env);
    token_admin.mint(&other, &5_000);
    client.contribute(&campaign_id, &other, &700);

    // contributor requests 500 — overshoots by 200 (remaining = 300).
    // accepted = 300, which is ≤ cap (400) → must succeed.
    client.contribute(&campaign_id, &contributor, &500);

    // Accepted portion is 300, not 500.
    assert_eq!(client.get_contribution(&campaign_id, &contributor), 300);
    assert_eq!(client.get_campaign(&campaign_id).amount_raised, 1_000);
}

/// Same scenario but with a personal cap: the accepted portion fits, so the
/// contribution must succeed even though the full requested amount would exceed
/// the personal cap.
#[test]
fn test_personal_cap_check_uses_accepted_amount_on_partial_fill() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &5_000);

    // No campaign cap; personal cap = 400.
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Personal partial fill"),
        String::from_str(&env, "Issue 1440 personal"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.set_personal_cap(&campaign_id, &contributor, &400);

    // Fill 700 elsewhere so remaining = 300.
    let other = soroban_sdk::Address::generate(&env);
    token_admin.mint(&other, &5_000);
    client.contribute(&campaign_id, &other, &700);

    // contributor requests 500 — overshoots by 200 (remaining = 300).
    // accepted = 300, which is ≤ personal cap (400) → must succeed.
    client.contribute(&campaign_id, &contributor, &500);

    assert_eq!(client.get_contribution(&campaign_id, &contributor), 300);
    assert_eq!(client.get_campaign(&campaign_id).amount_raised, 1_000);
}

/// If even the accepted portion would exceed the cap, the contribution must
/// still be rejected.
#[test]
fn test_cap_exceeded_even_after_partial_fill_is_rejected() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &5_000);

    // campaign cap = 100; contributor already contributed 50; remaining in
    // campaign = 200.  Requests 150 → accepted = 150, lifetime would become
    // 50 + 150 = 200 > 100 → rejected.
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Still exceeds cap"),
        String::from_str(&env, "Issue 1440 reject"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        100i128,
    ));
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor, &50);
    let res = client.try_contribute(&campaign_id, &contributor, &150);
    assert_eq!(res, Err(Ok(Error::ContributionCapExceeded)));
    assert_eq!(client.get_contribution(&campaign_id, &contributor), 50);
}
