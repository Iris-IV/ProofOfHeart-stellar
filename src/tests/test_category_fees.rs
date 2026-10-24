//! Tests for category-specific platform fee calculation (#1019).

#![cfg(test)]

use crate::constants::{
    get_category_fee_bps, EDUCATIONAL_STARTUP_FEE_BPS, EDUCATOR_FEE_BPS, LEARNER_FEE_BPS,
    PUBLISHER_FEE_BPS,
};

#[test]
fn test_learner_category_fee() {
    let fee = get_category_fee_bps(0); // Category::Learner = 0
    assert_eq!(fee, LEARNER_FEE_BPS);
    assert_eq!(fee, 150); // 1.5%
}

#[test]
fn test_educational_startup_category_fee() {
    let fee = get_category_fee_bps(1); // Category::EducationalStartup = 1
    assert_eq!(fee, EDUCATIONAL_STARTUP_FEE_BPS);
    assert_eq!(fee, 300); // 3%
}

#[test]
fn test_educator_category_fee() {
    let fee = get_category_fee_bps(2); // Category::Educator = 2
    assert_eq!(fee, EDUCATOR_FEE_BPS);
    assert_eq!(fee, 200); // 2%
}

#[test]
fn test_publisher_category_fee() {
    let fee = get_category_fee_bps(3); // Category::Publisher = 3
    assert_eq!(fee, PUBLISHER_FEE_BPS);
    assert_eq!(fee, 350); // 3.5%
}

#[test]
fn test_unknown_category_falls_back_to_startup_fee() {
    let fee = get_category_fee_bps(99); // Unknown category
    assert_eq!(fee, EDUCATIONAL_STARTUP_FEE_BPS);
    assert_eq!(fee, 300); // Fallback to standard 3%
}

#[test]
fn test_category_fees_are_reasonable() {
    // All category fees should be between 1% and 5% (100-500 bps)
    assert!(LEARNER_FEE_BPS >= 100 && LEARNER_FEE_BPS <= 500);
    assert!(EDUCATIONAL_STARTUP_FEE_BPS >= 100 && EDUCATIONAL_STARTUP_FEE_BPS <= 500);
    assert!(EDUCATOR_FEE_BPS >= 100 && EDUCATOR_FEE_BPS <= 500);
    assert!(PUBLISHER_FEE_BPS >= 100 && PUBLISHER_FEE_BPS <= 500);
}

#[test]
fn test_category_fees_are_ordered_by_risk_profile() {
    // Learners (individual, low-risk) should have the lowest fee
    assert!(LEARNER_FEE_BPS < EDUCATOR_FEE_BPS);
    assert!(LEARNER_FEE_BPS < EDUCATIONAL_STARTUP_FEE_BPS);
    assert!(LEARNER_FEE_BPS < PUBLISHER_FEE_BPS);

    // Publishers (commercial, high-risk) should have the highest fee
    assert!(PUBLISHER_FEE_BPS > LEARNER_FEE_BPS);
    assert!(PUBLISHER_FEE_BPS > EDUCATOR_FEE_BPS);
    assert!(PUBLISHER_FEE_BPS >= EDUCATIONAL_STARTUP_FEE_BPS);
}

#[test]
fn test_get_category_fee_bps_is_const_fn() {
    // This test verifies that get_category_fee_bps can be used in const contexts.
    // If it compiles, the function is properly marked as const fn.
    const LEARNER_FEE: u32 = get_category_fee_bps(0);
    assert_eq!(LEARNER_FEE, 150);
    assert_eq!(get_category_fee_bps(0), 150);
}
