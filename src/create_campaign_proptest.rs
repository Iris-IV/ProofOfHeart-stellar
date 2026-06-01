//! Property-based fuzz tests for `create_campaign` validation inputs.
//!
//! These tests exercise the pure validation logic (no contract environment
//! required) to uncover edge-case regressions in:
//!
//! * `funding_goal` bounds (positive, min, max cap)
//! * `duration_days` bounds
//! * `revenue_share_percentage` bounds
//! * `max_contribution_per_user` sign check

use proptest::prelude::*;

// ── Mirror the validation constants from lib.rs ──────────────────────────────

const CAMPAIGN_FUNDING_GOAL_MIN: i128 = 100_000;
const CAMPAIGN_FUNDING_GOAL_MAX: i128 = 1_000_000_000_000_000; // 10^15
const CAMPAIGN_DURATION_MIN_DAYS: u64 = 1;
const CAMPAIGN_DURATION_MAX_DAYS: u64 = 365;
const REVENUE_SHARE_MAX_BPS: u32 = 5000;

// ── Pure validation helpers (mirror lib.rs logic) ────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum ValidationError {
    FundingGoalMustBePositive,
    FundingGoalTooLow,
    FundingGoalTooHigh,
    InvalidDuration,
    InvalidRevenueShare,
    NegativeContributionCap,
}

fn validate_funding_goal(goal: i128, min: i128, max: i128) -> Result<(), ValidationError> {
    if goal <= 0 {
        return Err(ValidationError::FundingGoalMustBePositive);
    }
    if goal < min {
        return Err(ValidationError::FundingGoalTooLow);
    }
    if goal > max {
        return Err(ValidationError::FundingGoalTooHigh);
    }
    Ok(())
}

fn validate_duration(days: u64) -> Result<(), ValidationError> {
    if !(CAMPAIGN_DURATION_MIN_DAYS..=CAMPAIGN_DURATION_MAX_DAYS).contains(&days) {
        return Err(ValidationError::InvalidDuration);
    }
    Ok(())
}

fn validate_revenue_share(has_revenue_sharing: bool, bps: u32) -> Result<(), ValidationError> {
    if bps > REVENUE_SHARE_MAX_BPS {
        return Err(ValidationError::InvalidRevenueShare);
    }
    if has_revenue_sharing && bps == 0 {
        return Err(ValidationError::InvalidRevenueShare);
    }
    Ok(())
}

fn validate_max_contribution(cap: i128) -> Result<(), ValidationError> {
    if cap < 0 {
        return Err(ValidationError::NegativeContributionCap);
    }
    Ok(())
}

// ── Properties ───────────────────────────────────────────────────────────────

proptest! {
    /// Any goal in [min, max] must pass.
    #[test]
    fn prop_funding_goal_valid_range_always_passes(
        goal in CAMPAIGN_FUNDING_GOAL_MIN..=CAMPAIGN_FUNDING_GOAL_MAX,
    ) {
        prop_assert!(
            validate_funding_goal(goal, CAMPAIGN_FUNDING_GOAL_MIN, CAMPAIGN_FUNDING_GOAL_MAX).is_ok(),
            "goal {goal} in valid range should pass"
        );
    }

    /// Zero or negative goals must always be rejected.
    #[test]
    fn prop_non_positive_funding_goal_rejected(goal in i128::MIN..=0i128) {
        let err = validate_funding_goal(goal, CAMPAIGN_FUNDING_GOAL_MIN, CAMPAIGN_FUNDING_GOAL_MAX)
            .unwrap_err();
        prop_assert_eq!(err, ValidationError::FundingGoalMustBePositive);
    }

    /// Goals below min (but positive) must return TooLow.
    #[test]
    fn prop_funding_goal_below_min_rejected(goal in 1i128..CAMPAIGN_FUNDING_GOAL_MIN) {
        let err = validate_funding_goal(goal, CAMPAIGN_FUNDING_GOAL_MIN, CAMPAIGN_FUNDING_GOAL_MAX)
            .unwrap_err();
        prop_assert_eq!(err, ValidationError::FundingGoalTooLow);
    }

    /// Goals above max must return TooHigh.
    #[test]
    fn prop_funding_goal_above_max_rejected(
        goal in (CAMPAIGN_FUNDING_GOAL_MAX + 1)..=i128::MAX,
    ) {
        let err = validate_funding_goal(goal, CAMPAIGN_FUNDING_GOAL_MIN, CAMPAIGN_FUNDING_GOAL_MAX)
            .unwrap_err();
        prop_assert_eq!(err, ValidationError::FundingGoalTooHigh);
    }

    /// Duration in [1, 365] must pass.
    #[test]
    fn prop_valid_duration_passes(days in CAMPAIGN_DURATION_MIN_DAYS..=CAMPAIGN_DURATION_MAX_DAYS) {
        prop_assert!(validate_duration(days).is_ok());
    }

    /// Duration > 365 must fail.
    #[test]
    fn prop_duration_above_max_rejected(days in (CAMPAIGN_DURATION_MAX_DAYS + 1)..=u64::MAX) {
        prop_assert_eq!(validate_duration(days).unwrap_err(), ValidationError::InvalidDuration);
    }

    /// Revenue share bps in (0, 5000] with flag=true must pass.
    #[test]
    fn prop_valid_revenue_share_passes(bps in 1u32..=REVENUE_SHARE_MAX_BPS) {
        prop_assert!(validate_revenue_share(true, bps).is_ok());
    }

    /// bps > 5000 must always fail regardless of flag.
    #[test]
    fn prop_revenue_share_above_max_rejected(bps in (REVENUE_SHARE_MAX_BPS + 1)..=u32::MAX) {
        prop_assert_eq!(
            validate_revenue_share(true, bps).unwrap_err(),
            ValidationError::InvalidRevenueShare
        );
        prop_assert_eq!(
            validate_revenue_share(false, bps).unwrap_err(),
            ValidationError::InvalidRevenueShare
        );
    }

    /// Revenue sharing disabled with any bps in [0, 5000] must pass.
    #[test]
    fn prop_revenue_share_disabled_any_valid_bps_passes(bps in 0u32..=REVENUE_SHARE_MAX_BPS) {
        prop_assert!(validate_revenue_share(false, bps).is_ok());
    }

    /// Non-negative contribution cap must pass.
    #[test]
    fn prop_non_negative_contribution_cap_passes(cap in 0i128..=i128::MAX) {
        prop_assert!(validate_max_contribution(cap).is_ok());
    }

    /// Negative contribution cap must fail.
    #[test]
    fn prop_negative_contribution_cap_rejected(cap in i128::MIN..=-1i128) {
        prop_assert_eq!(
            validate_max_contribution(cap).unwrap_err(),
            ValidationError::NegativeContributionCap
        );
    }

    /// Admin-raised cap: a goal previously above default max is valid under the new cap.
    #[test]
    fn prop_admin_raised_cap_allows_higher_goals(
        extra in 1i128..=CAMPAIGN_FUNDING_GOAL_MAX,
    ) {
        let goal = CAMPAIGN_FUNDING_GOAL_MAX + extra;
        let raised_max = goal; // admin sets cap exactly to this goal
        prop_assert!(
            validate_funding_goal(goal, CAMPAIGN_FUNDING_GOAL_MIN, raised_max).is_ok(),
            "goal {goal} should pass under raised cap {raised_max}"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_funding_goal_boundary_exact_min() {
        assert!(validate_funding_goal(
            CAMPAIGN_FUNDING_GOAL_MIN,
            CAMPAIGN_FUNDING_GOAL_MIN,
            CAMPAIGN_FUNDING_GOAL_MAX
        )
        .is_ok());
    }

    #[test]
    fn test_funding_goal_boundary_exact_max() {
        assert!(validate_funding_goal(
            CAMPAIGN_FUNDING_GOAL_MAX,
            CAMPAIGN_FUNDING_GOAL_MIN,
            CAMPAIGN_FUNDING_GOAL_MAX
        )
        .is_ok());
    }

    #[test]
    fn test_funding_goal_one_above_max() {
        assert_eq!(
            validate_funding_goal(
                CAMPAIGN_FUNDING_GOAL_MAX + 1,
                CAMPAIGN_FUNDING_GOAL_MIN,
                CAMPAIGN_FUNDING_GOAL_MAX
            )
            .unwrap_err(),
            ValidationError::FundingGoalTooHigh
        );
    }

    #[test]
    fn test_funding_goal_one_below_min() {
        assert_eq!(
            validate_funding_goal(
                CAMPAIGN_FUNDING_GOAL_MIN - 1,
                CAMPAIGN_FUNDING_GOAL_MIN,
                CAMPAIGN_FUNDING_GOAL_MAX
            )
            .unwrap_err(),
            ValidationError::FundingGoalTooLow
        );
    }

    #[test]
    fn test_admin_can_raise_cap() {
        let raised_max = CAMPAIGN_FUNDING_GOAL_MAX * 2;
        assert!(validate_funding_goal(
            CAMPAIGN_FUNDING_GOAL_MAX + 1,
            CAMPAIGN_FUNDING_GOAL_MIN,
            raised_max
        )
        .is_ok());
    }

    #[test]
    fn test_duration_zero_rejected() {
        assert_eq!(
            validate_duration(0).unwrap_err(),
            ValidationError::InvalidDuration
        );
    }

    #[test]
    fn test_revenue_share_enabled_zero_bps_rejected() {
        assert_eq!(
            validate_revenue_share(true, 0).unwrap_err(),
            ValidationError::InvalidRevenueShare
        );
    }
}
