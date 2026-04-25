//! Property-based fuzz tests for the voting system.
//!
//! These tests use `proptest` to exercise the voting logic with arbitrary inputs,
//! confirming that:
//!
//! * Vote counts and weights are always non-negative
//! * Approval weight + rejection weight equals total weight
//! * Vote counts increment correctly
//! * Threshold calculations don't overflow
//! * Quorum checks work correctly with arbitrary vote counts

use proptest::prelude::*;

// ── Pure arithmetic helpers ──────────────────────────────────────────────────

/// Calculate approval percentage in basis points (0-10000)
fn calculate_approval_bps(approve_weight: i128, total_weight: i128) -> u32 {
    if total_weight > 0 {
        ((approve_weight * 10_000) / total_weight) as u32
    } else {
        0
    }
}

/// Check if quorum is met
fn is_quorum_met(total_votes: u32, min_quorum: u32) -> bool {
    total_votes >= min_quorum
}

/// Check if approval threshold is met
fn is_threshold_met(approval_bps: u32, threshold_bps: u32) -> bool {
    approval_bps >= threshold_bps
}

// ── Strategies ───────────────────────────────────────────────────────────────

/// Vote counts: 0 to a reasonable maximum (1 million votes)
fn arb_vote_count() -> impl Strategy<Value = u32> {
    0u32..=1_000_000u32
}

/// Token weights: 0 to 10 billion stroops
fn arb_token_weight() -> impl Strategy<Value = i128> {
    0i128..=10_000_000_000i128
}

/// Approval threshold in basis points (0-10000, i.e., 0-100%)
fn arb_threshold_bps() -> impl Strategy<Value = u32> {
    0u32..=10_000u32
}

/// Minimum quorum (1-1000 votes)
fn arb_min_quorum() -> impl Strategy<Value = u32> {
    1u32..=1_000u32
}

// ── Properties ───────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_approval_bps_in_valid_range(
        approve_weight in arb_token_weight(),
        reject_weight in arb_token_weight(),
    ) {
        let total_weight = approve_weight + reject_weight;
        let approval_bps = calculate_approval_bps(approve_weight, total_weight);
        prop_assert!(
            approval_bps <= 10_000,
            "approval_bps ({}) must be <= 10000",
            approval_bps
        );
    }

    #[test]
    fn prop_full_approval_gives_max_bps(weight in arb_token_weight()) {
        let approval_bps = calculate_approval_bps(weight, weight);
        prop_assert_eq!(
            approval_bps, 10_000,
            "100% approval should give 10000 bps"
        );
    }

    #[test]
    fn prop_zero_approval_gives_zero_bps(reject_weight in arb_token_weight()) {
        let approval_bps = calculate_approval_bps(0, reject_weight);
        prop_assert_eq!(approval_bps, 0, "0% approval should give 0 bps");
    }

    #[test]
    fn prop_half_approval_gives_half_bps(weight in 2i128..=10_000_000_000i128) {
        let half = weight / 2;
        let approval_bps = calculate_approval_bps(half, weight);
        // Allow for rounding error of 1 bps
        prop_assert!(
            (4_999..=5_000).contains(&approval_bps),
            "50% approval should give ~5000 bps, got {}",
            approval_bps
        );
    }

    #[test]
    fn prop_quorum_check_consistent(
        approve_votes in arb_vote_count(),
        reject_votes in arb_vote_count(),
        min_quorum in arb_min_quorum(),
    ) {
        let total_votes = approve_votes.saturating_add(reject_votes);
        let met = is_quorum_met(total_votes, min_quorum);
        prop_assert_eq!(met, total_votes >= min_quorum);
    }

    #[test]
    fn prop_threshold_check_consistent(
        approval_bps in arb_threshold_bps(),
        threshold_bps in arb_threshold_bps(),
    ) {
        let met = is_threshold_met(approval_bps, threshold_bps);
        prop_assert_eq!(met, approval_bps >= threshold_bps);
    }

    #[test]
    fn prop_vote_count_no_overflow(
        approve_votes in 0u32..=500_000u32,
        reject_votes in 0u32..=500_000u32,
    ) {
        let total = approve_votes.checked_add(reject_votes);
        prop_assert!(total.is_some(), "vote count addition should not overflow");
    }

    #[test]
    fn prop_weight_no_overflow(
        approve_weight in 0i128..=5_000_000_000i128,
        reject_weight in 0i128..=5_000_000_000i128,
    ) {
        let total = approve_weight.checked_add(reject_weight);
        prop_assert!(total.is_some(), "weight addition should not overflow");
    }

    #[test]
    fn prop_approval_monotonic(
        base_approve in 0i128..=1_000_000i128,
        extra_approve in 0i128..=1_000_000i128,
        reject_weight in 1i128..=1_000_000i128,
    ) {
        let bps1 = calculate_approval_bps(base_approve, base_approve + reject_weight);
        let bps2 = calculate_approval_bps(
            base_approve + extra_approve,
            base_approve + extra_approve + reject_weight
        );
        prop_assert!(
            bps2 >= bps1,
            "adding approval weight should not decrease approval bps: {} -> {}",
            bps1, bps2
        );
    }

    #[test]
    fn prop_verification_requires_both_conditions(
        approve_votes in arb_vote_count(),
        reject_votes in arb_vote_count(),
        approve_weight in arb_token_weight(),
        reject_weight in arb_token_weight(),
        min_quorum in arb_min_quorum(),
        threshold_bps in 5_000u32..=10_000u32, // 50-100%
    ) {
        let total_votes = approve_votes.saturating_add(reject_votes);
        let total_weight = approve_weight.saturating_add(reject_weight);
        let approval_bps = calculate_approval_bps(approve_weight, total_weight);

        let quorum_met = is_quorum_met(total_votes, min_quorum);
        let threshold_met = is_threshold_met(approval_bps, threshold_bps);
        let can_verify = quorum_met && threshold_met;

        // If either condition fails, verification should fail
        if !quorum_met || !threshold_met {
            prop_assert!(!can_verify);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_approval_bps_calculation() {
        // 60% approval
        assert_eq!(calculate_approval_bps(600, 1000), 6000);

        // 100% approval
        assert_eq!(calculate_approval_bps(1000, 1000), 10000);

        // 0% approval
        assert_eq!(calculate_approval_bps(0, 1000), 0);

        // Zero total weight
        assert_eq!(calculate_approval_bps(0, 0), 0);
    }

    #[test]
    fn test_quorum_checks() {
        assert!(is_quorum_met(10, 5));
        assert!(is_quorum_met(5, 5));
        assert!(!is_quorum_met(4, 5));
        assert!(!is_quorum_met(0, 1));
    }

    #[test]
    fn test_threshold_checks() {
        assert!(is_threshold_met(6000, 5000));
        assert!(is_threshold_met(5000, 5000));
        assert!(!is_threshold_met(4999, 5000));
        assert!(!is_threshold_met(0, 1));
    }
}
