//! Guards for #791 (cumulative contribution cap), #793 (platform-fee bounds),
//! #792 (heavy-struct copying in queries) and #794 (deadline extension cap).
//!
//! # What these four issues turned out to be
//!
//! All four describe conditions the contract already satisfies. The value here
//! is therefore not a fix but a lock: each property is asserted directly, in
//! the exact scenario the issue describes, so it cannot regress silently. Two
//! genuine defects did surface while writing them, both fixed and pinned
//! below — a wasted full-struct load in `get_contributor_portfolio`, and an
//! unreachable duplicate bound in the fee setters.
//!
//! Where an issue's premise did not hold, the test says so and asserts the
//! behaviour that *is* correct, rather than quietly asserting nothing.

extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams, Error};
use soroban_sdk::{Address, String};

static mut CC_COUNTER: u32 = 0;

fn capped_campaign(
    env: &soroban_sdk::Env,
    creator: &Address,
    client: &ProofOfHeartClient,
    max_per_user: i128,
    index: u32,
) -> u32 {
    let nonce = unsafe {
        CC_COUNTER += 1;
        CC_COUNTER
    };
    let title_data = [
        b'C',
        b'C',
        b'_',
        b'0' + (nonce / 10) as u8,
        b'0' + (nonce % 10) as u8,
    ];
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_bytes(env, &title_data),
        description: String::from_str(env, "Has a per-user contribution cap"),
        funding_goal: 100_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: max_per_user,
    })
}

// ── #791: the cap is cumulative, not per transaction ─────────────────────────
//
// The issue's concern is that `max_contribution_per_user` might be compared
// against a single transaction's amount, letting a user split a contribution
// into several transactions to exceed it. `check_contribution_caps` already
// compares `lifetime + amount`, so the cap holds; these tests exercise the
// multi-transaction path end to end rather than the helper in isolation.

/// A user cannot exceed the cap by splitting the contribution across calls.
#[test]
fn test_contribution_cap_holds_across_multiple_transactions() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &400);
    client.contribute(&id, &contributor1, &400);

    // 800 so far; a third 400 would reach 1200, past the 1000 cap.
    let res = client.try_contribute(&id, &contributor1, &400);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    // The rejected call changed nothing.
    assert_eq!(client.get_contribution(&id, &contributor1), 800);
}

/// The cap is a ceiling the user may reach exactly, in any number of steps.
#[test]
fn test_contribution_cap_can_be_reached_exactly_in_steps() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &600);
    client.contribute(&id, &contributor1, &400);
    assert_eq!(client.get_contribution(&id, &contributor1), 1000);

    // One stroop more is refused.
    let res = client.try_contribute(&id, &contributor1, &1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
}

/// A single oversized contribution is refused too — the cumulative check must
/// not have replaced the per-transaction one.
#[test]
fn test_contribution_cap_rejects_a_single_oversized_transfer() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);

    let res = client.try_contribute(&id, &contributor1, &1001);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
    assert_eq!(client.get_contribution(&id, &contributor1), 0);
}

/// The cap is per user, not per campaign: one contributor hitting it does not
/// restrict anyone else.
#[test]
fn test_contribution_cap_is_per_user() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &100_000);
    token_admin.mint(&contributor2, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &1000);
    assert!(client.try_contribute(&id, &contributor1, &1).is_err());

    client.contribute(&id, &contributor2, &1000);
    assert_eq!(client.get_contribution(&id, &contributor2), 1000);
}

/// A batch cannot do what a sequence of calls cannot.
///
/// The batch path applies each item's accounting immediately, so a campaign
/// repeated later in the same batch sees the earlier item's updated total.
#[test]
fn test_contribution_cap_holds_within_a_single_batch() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);

    // 600 + 600 = 1200 against a 1000 cap, split across two batch items.
    let batch = soroban_sdk::Vec::from_array(&env, [(id, 600i128), (id, 600i128)]);
    let res = client.try_batch_contribute(&contributor1, &batch);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    // The batch is atomic: the first item did not stick either.
    assert_eq!(client.get_contribution(&id, &contributor1), 0);
}

/// A refund does not reset the cap.
///
/// `claim_refund` clears the refundable balance but deliberately leaves the
/// lifetime total in place, so refund-and-recontribute is not a way around
/// the ceiling.
#[test]
fn test_refund_does_not_reset_the_lifetime_cap() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 1000, 0);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    // The refundable balance is gone...
    assert_eq!(client.get_contribution(&id, &contributor1), 0);
    // ...but the lifetime total, which the cap is measured against, is not.
    assert_eq!(client.get_lifetime_contribution(&id, &contributor1), 1000);
}

/// A `max_contribution_per_user` of zero is the documented "no cap" sentinel,
/// not "no contributions allowed".
#[test]
fn test_zero_cap_means_unlimited() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 0, 0);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &50_000);
    client.contribute(&id, &contributor1, &20_000);
    assert_eq!(client.get_contribution(&id, &contributor1), 70_000);
}

// ── #793: the platform fee is bounded ────────────────────────────────────────
//
// The issue asks that `update_platform_fee` be limited to 10000 bps so the
// basis-point arithmetic cannot overflow. The policy ceiling is already far
// tighter (1000 bps = 10%), and the arithmetic itself uses checked operations
// (#408). Both bounds are pinned here, along with the property the issue is
// ultimately about: a large withdrawal does not overflow.

/// The policy ceiling rejects anything above 10%.
#[test]
fn test_platform_fee_is_capped_at_the_policy_ceiling() {
    let (_env, _admin, _creator, _, _, _token, _token_admin, client) = setup_env();

    client.update_platform_fee(&crate::PLATFORM_FEE_MAX_BPS);
    assert_eq!(client.get_platform_fee(), crate::PLATFORM_FEE_MAX_BPS);

    let res = client.try_update_platform_fee(&(crate::PLATFORM_FEE_MAX_BPS + 1));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidPlatformFee);

    // The rejected value did not take effect.
    assert_eq!(client.get_platform_fee(), crate::PLATFORM_FEE_MAX_BPS);
}

/// Nothing at or beyond the basis-point denominator is accepted — the bound
/// the issue names, and the one the fee arithmetic depends on.
#[test]
fn test_platform_fee_never_accepts_a_fee_at_or_above_100_percent() {
    let (_env, _admin, _creator, _, _, _token, _token_admin, client) = setup_env();

    for bps in [
        crate::BPS_DENOMINATOR,
        crate::BPS_DENOMINATOR + 1,
        u32::MAX / 2,
        u32::MAX,
    ] {
        let res = client.try_update_platform_fee(&bps);
        assert_eq!(
            res.unwrap_err().unwrap(),
            Error::InvalidPlatformFee,
            "fee of {} bps must be rejected",
            bps
        );
    }
}

/// A per-campaign override is subject to the same ceiling, so it cannot be
/// used to route around `update_platform_fee`.
#[test]
fn test_campaign_fee_override_obeys_the_same_ceiling() {
    let (env, admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let id = capped_campaign(&env, &creator, &client, 0, 0);

    client.set_campaign_fee_override(&id, &admin, &crate::PLATFORM_FEE_MAX_BPS);
    assert_eq!(
        client.get_campaign(&id).fee_override,
        Some(crate::PLATFORM_FEE_MAX_BPS)
    );

    let res = client.try_set_campaign_fee_override(&id, &admin, &crate::BPS_DENOMINATOR);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidPlatformFee);
}

/// The scenario behind the issue: a withdrawal at the maximum fee on a large
/// raise settles without overflow, and the creator is never short-changed.
#[test]
fn test_maximum_fee_withdrawal_does_not_overflow() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    client.update_platform_fee(&crate::PLATFORM_FEE_MAX_BPS);

    let goal = 1_000_000_000i128;
    token_admin.mint(&contributor1, &(goal * 2));

    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Large Raise"),
        description: String::from_str(&env, "Exercises the fee arithmetic at scale"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &goal);

    client.withdraw_funds(&id);

    // The creator received something, and strictly less than the full raise.
    let received = token.balance(&creator);
    assert!(received > 0);
    assert!(received < goal);
}

// ── #792: query paths do not copy campaigns they discard ─────────────────────
//
// The issue asks that `Campaign` be passed by reference in query and
// validation functions. It already is everywhere — `check_contribution_caps`,
// `check_burst_guard`, `require_active_campaign`, `require_unverified_campaign`
// and `CampaignState::of` all take `&Campaign`, and no function in the crate
// takes one by value. The real copying was elsewhere: a query that loaded
// every campaign and then threw most of them away.

/// `get_contributor_portfolio` returns only campaigns the caller funded.
///
/// The filter now runs before the campaign is loaded, so this also pins that
/// the reorder did not change what the query returns.
#[test]
fn test_contributor_portfolio_returns_only_funded_campaigns() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let funded = capped_campaign(&env, &creator, &client, 0, 0);
    let other_a = capped_campaign(&env, &creator, &client, 0, 1);
    let other_b = capped_campaign(&env, &creator, &client, 0, 2);
    client.verify_campaign(&funded);

    client.contribute(&funded, &contributor1, &2500);

    let (portfolio, _) = client.get_contributor_portfolio(&contributor1, &0, &u32::MAX);
    assert_eq!(portfolio.len(), 1);

    let (id, amount, _status, _refundable) = portfolio.get(0).unwrap();
    assert_eq!(id, funded);
    assert_eq!(amount, 2500);

    let _ = (other_a, other_b);
}

/// A contributor with no contributions gets an empty portfolio, without the
/// query loading a single campaign.
#[test]
fn test_contributor_portfolio_is_empty_for_a_non_contributor() {
    let (env, _admin, creator, _, contributor2, _token, _token_admin, client) = setup_env();

    for i in 0..5 {
        capped_campaign(&env, &creator, &client, 0, i);
    }

    assert_eq!(client.get_contributor_portfolio(&contributor2, &0, &u32::MAX).0.len(), 0);
}

/// The portfolio still reports status and refundability from the campaign, so
/// the reorder did not drop the fields that require loading it.
#[test]
fn test_contributor_portfolio_still_reports_campaign_state() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let id = capped_campaign(&env, &creator, &client, 0, 0);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    let (before, _) = client.get_contributor_portfolio(&contributor1, &0, &u32::MAX);
    let (_, _, status, refundable) = before.get(0).unwrap();
    assert_eq!(status, String::from_str(&env, "verified"));
    assert!(!refundable);

    client.cancel_campaign(&id);

    let (after, _) = client.get_contributor_portfolio(&contributor1, &0, &u32::MAX);
    let (_, _, status, refundable) = after.get(0).unwrap();
    assert_eq!(status, String::from_str(&env, "cancelled"));
    assert!(refundable);
}

/// Validation helpers take `&Campaign`, so calling one leaves the caller's
/// campaign usable — a by-value signature would have moved it.
///
/// This compiles only while the borrow-based signatures hold, which is the
/// property #792 asks for; the assertions merely give it a runtime home.
#[test]
fn test_validation_helpers_borrow_rather_than_consume_the_campaign() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let id = capped_campaign(&env, &creator, &client, 0, 0);

    env.as_contract(&client.address, || {
        let campaign = storage::get_campaign(&env, id).unwrap();

        // Each of these borrows; the campaign is still owned afterwards.
        assert!(crate::lifecycle::require_active_campaign(&campaign).is_ok());
        assert!(crate::lifecycle::require_unverified_campaign(&campaign).is_ok());

        assert_eq!(campaign.id, id);
        assert_eq!(campaign.creator, creator);
    });
}

// ── #794: a deadline cannot be pushed indefinitely ───────────────────────────
//
// Duplicate of #788, already implemented: `MAX_EXTENSION_DAYS` caps one
// extension, `deadline_extended` makes it one-shot, and the resulting span is
// bounded by the category cap and `CAMPAIGN_EXTENSION_MAX_DAYS`. Covered in
// `test_deadline_and_reverification.rs`; what is added here is the end-to-end
// property those layers exist to produce.

/// However a creator combines the levers, the deadline never lands more than
/// `CAMPAIGN_DURATION_MAX_DAYS` after the campaign started — which is what
/// "funds cannot be locked indefinitely" means concretely.
#[test]
fn test_deadline_can_never_exceed_the_absolute_horizon() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let start = env.ledger().timestamp();
    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Long Campaign"),
        description: String::from_str(&env, "Started at the maximum duration"),
        funding_goal: 100_000,
        duration_days: crate::CAMPAIGN_DURATION_MAX_DAYS,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });

    // Every extension the per-call cap permits is refused, because the span is
    // already at the ceiling.
    for days in [1u64, 15, crate::MAX_EXTENSION_DAYS] {
        assert!(
            client.try_extend_campaign_deadline(&id, &days).is_err(),
            "extending an already-maximal campaign by {} days must fail",
            days
        );
    }

    let horizon = start + crate::CAMPAIGN_DURATION_MAX_DAYS * crate::SECONDS_PER_DAY;
    assert!(client.get_campaign(&id).deadline <= horizon);
}

/// A shorter campaign may extend, and the result still sits inside the
/// horizon.
#[test]
fn test_extension_stays_within_the_absolute_horizon() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let start = env.ledger().timestamp();
    let id = capped_campaign(&env, &creator, &client, 0, 0);

    client.extend_campaign_deadline(&id, &crate::MAX_EXTENSION_DAYS);

    let horizon = start + crate::CAMPAIGN_DURATION_MAX_DAYS * crate::SECONDS_PER_DAY;
    let deadline = client.get_campaign(&id).deadline;
    assert!(deadline > start);
    assert!(deadline <= horizon);

    // And it is genuinely one-shot.
    assert!(client.try_extend_campaign_deadline(&id, &1).is_err());
}

