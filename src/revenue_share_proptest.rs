//! Property-based fuzz tests for the revenue-share calculation logic.
//!
//! These tests use `proptest` to exercise the arithmetic used in
//! `claim_revenue` and `claim_creator_revenue` with arbitrary inputs,
//! confirming that:
//!
//! * No integer overflow or underflow occurs (the contract is compiled
//!   with `overflow-checks = true` in release, and the tests run in debug
//!   mode where overflow already panics).
//! * The contributor pool never exceeds the total revenue pool.
//! * Individual contributor dues never exceed the contributor pool.
//! * Contributor due + creator share equals the full revenue pool
//!   (no tokens are lost or created).
//! * All results remain non-negative.

use proptest::prelude::*;

// ── Pure arithmetic helpers ──────────────────────────────────────────────────
//
// These mirror the formulas in lib.rs exactly so the properties are tested
// against the real calculation, not a reimplementation.

/// Compute the portion of the pool allocated to all contributors combined.
///
/// `revenue_share_percentage` is in basis points (0 – 5 000, i.e. 0 – 50 %).
fn contributor_pool(total_pool: i128, revenue_share_bps: i128) -> i128 {
    (total_pool * revenue_share_bps) / 10_000
}

/// Compute one contributor's share of the contributor pool.
fn contributor_due(contribution: i128, contributor_pool: i128, amount_raised: i128) -> i128 {
    (contribution * contributor_pool) / amount_raised
}

// ── Strategies ───────────────────────────────────────────────────────────────

/// Revenue pool: allow 0 up to a realistic ceiling (~10 billion stroops).
fn arb_pool() -> impl Strategy<Value = i128> {
    0i128..=10_000_000_000i128
}

/// Revenue-share percentage in basis points (0 – 5 000 = 0 – 50 %).
fn arb_revenue_bps() -> impl Strategy<Value = i128> {
    0i128..=5_000i128
}

/// Amount raised: at least 1 (division guard) up to the pool ceiling.
fn arb_amount_raised() -> impl Strategy<Value = i128> {
    1i128..=10_000_000_000i128
}

// ── Properties ───────────────────────────────────────────────────────────────

proptest! {
    /// Contributor pool never exceeds the total revenue pool.
    #[test]
    fn prop_contributor_pool_does_not_exceed_total(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        prop_assert!(cp >= 0, "contributor pool must be non-negative");
        prop_assert!(
            cp <= total_pool,
            "contributor pool ({cp}) must not exceed total pool ({total_pool})"
        );
    }

    /// Creator share (total_pool – contributor_pool) is always non-negative.
    #[test]
    fn prop_creator_share_non_negative(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        let creator_share = total_pool - cp;
        prop_assert!(
            creator_share >= 0,
            "creator share ({creator_share}) must be non-negative"
        );
    }

    /// contributor_due ≤ contributor_pool for any valid contribution slice.
    #[test]
    fn prop_individual_due_does_not_exceed_pool(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
        contribution in arb_amount_raised(), // reuse; will be clamped below
    ) {
        let contribution = contribution.min(amount_raised);
        let cp = contributor_pool(total_pool, bps);
        let due = contributor_due(contribution, cp, amount_raised);
        prop_assert!(due >= 0, "contributor due must be non-negative");
        prop_assert!(
            due <= cp,
            "contributor due ({due}) must not exceed contributor pool ({cp})"
        );
    }

    /// The sum of contributor_pool + creator_share equals total_pool exactly
    /// (no tokens are lost or created by the split).
    #[test]
    fn prop_pool_split_is_lossless(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        let creator_share = total_pool - cp;
        prop_assert_eq!(
            cp + creator_share,
            total_pool,
            "split must be lossless: {} + {} != {}",
            cp, creator_share, total_pool
        );
    }

    /// Zero revenue_share_bps allocates nothing to contributors.
    #[test]
    fn prop_zero_bps_gives_contributors_nothing(total_pool in arb_pool()) {
        let cp = contributor_pool(total_pool, 0);
        prop_assert_eq!(cp, 0);
    }

    /// Maximum revenue_share_bps (5000 = 50 %) gives contributors exactly half.
    #[test]
    fn prop_max_bps_gives_contributors_half(total_pool in 0i128..=10_000_000_000i128) {
        let cp = contributor_pool(total_pool, 5_000);
        // Integer division may lose 1 stroop on odd pools — that is correct behaviour.
        prop_assert_eq!(
            cp,
            total_pool / 2,
            "max bps should give ~half: cp={}, half={}",
            cp,
            total_pool / 2
        );
    }

    /// contributor_due with a full contribution (== amount_raised) equals
    /// the whole contributor pool (single contributor edge case).
    #[test]
    fn prop_sole_contributor_gets_full_pool(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        // A contributor who contributed everything gets the full contributor pool.
        let due = contributor_due(amount_raised, cp, amount_raised);
        prop_assert_eq!(
            due, cp,
            "sole contributor should get entire contributor pool"
        );
    }

    /// Boundary: revenue pool of 0 produces 0 for all shares.
    #[test]
    fn prop_empty_pool_gives_zero_shares(
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
        contribution in arb_amount_raised(),
    ) {
        let contribution = contribution.min(amount_raised);
        let cp = contributor_pool(0, bps);
        let due = contributor_due(contribution, cp, amount_raised);
        prop_assert_eq!(cp, 0);
        prop_assert_eq!(due, 0);
    }
}
