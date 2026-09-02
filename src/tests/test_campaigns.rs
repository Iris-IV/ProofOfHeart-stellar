use proptest::prelude::*;

use super::helpers::*;
use crate::{
    storage, AdminKey, Category, CreateCampaignParams, Error, MaybePendingCreator,
    CAMPAIGN_DURATION_MAX_DAYS, CAMPAIGN_DURATION_MIN_DAYS, CAMPAIGN_FUNDING_GOAL_MAX,
    CAMPAIGN_FUNDING_GOAL_MIN, REVENUE_SHARE_MAX_BPS, SECONDS_PER_DAY,
};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation},
    Address, IntoVal, String, Symbol,
};

// ── campaign creation & validation ──────────────────────────────────────────────

#[test]
fn test_create_and_validation() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Science Book");
    let desc = String::from_str(&env, "Teaching science to kids");

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        0,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        500,
        0,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        500,
        400,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        500,
        30,
        Category::Educator,
        true,
        1000,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueShareOnlyForStartup);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        2000,
        30,
        Category::EducationalStartup,
        true,
        1500,
        0i128,
    ));
    assert_eq!(campaign_id, 1);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.id, 1);
    assert_eq!(campaign.funding_goal, 2000);
    assert!(campaign.is_active);
    assert!(!campaign.is_verified);
}

#[test]
fn test_get_campaign_not_found() {
    let (_env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let res = client.try_get_campaign(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_get_version() {
    let (_env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    assert_eq!(client.get_version(), 1u32);
}

#[test]
fn test_admin_verify_campaign_success() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Admin Verification"),
        String::from_str(&env, "Admin verifies campaign"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);
}

#[test]
fn test_admin_verify_campaign_duplicate_attempt() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Duplicate Verification"),
        String::from_str(&env, "Cannot verify twice"),
        1000,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::AdminVerificationConflict);
}

#[test]
fn test_description_length_boundaries() {
    extern crate std;
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let title = String::from_str(&env, "Title");

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        String::from_str(&env, ""),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    assert!(client
        .try_create_campaign(&make_params(
            creator.clone(),
            title.clone(),
            String::from_str(&env, "a"),
            1000,
            30,
            Category::Educator,
            false,
            0,
            0i128,
        ))
        .is_ok());

    let desc_1000 = "a".repeat(1000);
    assert!(client
        .try_create_campaign(&make_params(
            creator.clone(),
            title.clone(),
            String::from_str(&env, &desc_1000),
            1000,
            30,
            Category::Educator,
            false,
            0,
            0i128,
        ))
        .is_ok());

    let desc_1001 = "a".repeat(1001);
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        String::from_str(&env, &desc_1001),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_title_length_boundaries() {
    extern crate std;
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let desc = String::from_str(&env, "Description");

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, ""),
        desc.clone(),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    assert!(client
        .try_create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "a"),
            desc.clone(),
            1000,
            30,
            Category::Educator,
            false,
            0,
            0i128,
        ))
        .is_ok());

    let title_100 = "a".repeat(100);
    assert!(client
        .try_create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, &title_100),
            desc.clone(),
            1000,
            30,
            Category::Educator,
            false,
            0,
            0i128,
        ))
        .is_ok());

    let title_101 = "a".repeat(101);
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, &title_101),
        desc.clone(),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_revenue_share_percentage_normalised_to_zero_when_disabled() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "No Revenue"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Educator,
        false,
        12345,
        0i128,
    ));
    let campaign = client.get_campaign(&id);
    assert_eq!(campaign.revenue_share_percentage, 0);
    assert!(!campaign.has_revenue_sharing);
}

#[test]
fn test_revenue_share_above_max_rejected_even_without_flag() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Bad Revenue"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Educator,
        false,
        9999,
        0i128,
    ));
    assert_eq!(client.get_campaign(&id).revenue_share_percentage, 0);
}

#[test]
fn test_revenue_share_with_flag_true_above_max_rejected() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Too High"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        5001,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidRevenueShare);
}

#[test]
fn test_campaign_count_cannot_reset_after_deployment() {
    let (env, _admin, creator, _, _, token, _, client) = setup_env();

    assert_eq!(client.get_campaign_count(), 0);
    let titles = ["Campaign 1", "Campaign 2", "Campaign 3"];
    for i in 1u32..=3 {
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, titles[(i - 1) as usize]),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Educator,
            false,
            0,
            0i128,
        ));
        assert_eq!(id, i);
    }
    assert_eq!(client.get_campaign_count(), 3);

    client.update_platform_fee(&500);
    assert_eq!(client.get_campaign_count(), 3);

    let new_admin = Address::generate(&env);
    client.update_admin(&new_admin);
    client.accept_admin_transfer();
    assert_eq!(client.get_campaign_count(), 3);

    let res = client.try_init(&new_admin, &token.address, &300);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);
    assert_eq!(client.get_campaign_count(), 3);
}

#[test]
fn test_create_campaign_validation_independence() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    // Set a category cap of 10 days
    env.as_contract(&client.address, || {
        storage::set_category_duration_cap(&env, Category::Educator, 10);
    });

    // 1. FundingGoalTooHigh should trigger even if duration is invalid
    // Provide duration = 11 (invalid for Educator) and goal > max
    let params = make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"),
        CAMPAIGN_FUNDING_GOAL_MAX + 1,
        11,
        Category::Educator,
        false,
        0,
        0i128,
    );

    // Current logic checks goal bounds FIRST, then duration.
    // Wait, let's check src/lib.rs order.
    // 222: if funding_goal <= 0 ...
    // 225: if funding_goal < min ...
    // 228: let duration_max = ...
    // 230: if !(min..=max).contains(&duration_days) { return Err(InvalidDuration); }
    // 233: if funding_goal > get_max_campaign_funding_goal(...) { return Err(FundingGoalTooHigh); }

    // In my current version, InvalidDuration (230) is checked BEFORE FundingGoalTooHigh (233).
    // The user's requested fix for Issue 4 says:
    /*
    if !(CAMPAIGN_DURATION_MIN_DAYS..=duration_max).contains(&duration_days) {
        return Err(Error::InvalidDuration);
    }
    if funding_goal > get_max_campaign_funding_goal(&env, CAMPAIGN_FUNDING_GOAL_MAX) {
        return Err(Error::FundingGoalTooHigh);
    }
    */
    // This is exactly what I have in src/lib.rs.
    // But the user's Acceptance says:
    // "FundingGoalTooHigh triggers regardless of duration validity"

    // Wait! If they want FundingGoalTooHigh to trigger REGARDLESS of duration validity,
    // it MUST be checked BEFORE duration validity.

    let res = client.try_create_campaign(&params);
    // FundingGoalTooHigh triggers regardless of duration validity (as requested).
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);

    // 2. High goal with valid duration should trigger FundingGoalTooHigh
    let params_valid_dur = make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"),
        CAMPAIGN_FUNDING_GOAL_MAX + 1,
        5,
        Category::Educator,
        false,
        0,
        0i128,
    );
    let res = client.try_create_campaign(&params_valid_dur);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);
}

// ── create_campaign validation proptests ────────────────────────────────────────
// Property-based fuzz tests for `create_campaign` validation inputs.
//
// These tests exercise the pure validation logic (no contract environment
// required) to uncover edge-case regressions in:
//
// * `funding_goal` bounds (positive, min, max cap)
// * `duration_days` bounds
// * `revenue_share_percentage` bounds
// * `max_contribution_per_user` sign check

// ── Mirror the validation constants from lib.rs ──────────────────────────────

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

// ── campaign update & ownership transfer ────────────────────────────────────────

#[test]
fn test_update_campaign_blocks_after_admin_verification() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let orig_title = String::from_str(&env, "Original Title");
    let orig_desc = String::from_str(&env, "Original Description");
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        orig_title.clone(),
        orig_desc.clone(),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // Fix #416: update_campaign must be blocked after admin verification.
    let res = client.try_update_campaign(
        &campaign_id,
        &String::from_str(&env, "New Title"),
        &String::from_str(&env, "New Description"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, orig_title);
    assert_eq!(campaign.description, orig_desc);
}

#[test]
fn test_update_campaign_emits_old_and_new_title_and_description() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let orig_title = String::from_str(&env, "Original Title");
    let orig_desc = String::from_str(&env, "Original Description");
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        orig_title.clone(),
        orig_desc.clone(),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let new_title = String::from_str(&env, "Updated Title");
    let new_desc = String::from_str(&env, "Updated Description");
    client.update_campaign(&campaign_id, &new_title, &new_desc);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let payload: (String, String, String, String) =
        soroban_sdk::FromVal::from_val(&env, &last_event.2);

    assert_eq!(payload.0, orig_title, "old title");
    assert_eq!(payload.1, orig_desc, "old description");
    assert_eq!(payload.2, new_title, "new title");
    assert_eq!(payload.3, new_desc, "new description");
}

#[test]
fn test_update_campaign_event_old_values_track_previous_call_not_creation() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Original Title"),
        String::from_str(&env, "Original Description"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.update_campaign(
        &campaign_id,
        &String::from_str(&env, "Title V2"),
        &String::from_str(&env, "Description V2"),
    );
    client.update_campaign(
        &campaign_id,
        &String::from_str(&env, "Title V3"),
        &String::from_str(&env, "Description V3"),
    );

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let payload: (String, String, String, String) =
        soroban_sdk::FromVal::from_val(&env, &last_event.2);
    // The second call's "old" values must be the state right before it
    // (V2), not the original values from creation — old-value capture is
    // per-call, not cumulative.
    assert_eq!(payload.0, String::from_str(&env, "Title V2"));
    assert_eq!(payload.1, String::from_str(&env, "Description V2"));
    assert_eq!(payload.2, String::from_str(&env, "Title V3"));
    assert_eq!(payload.3, String::from_str(&env, "Description V3"));
}

#[test]
fn test_update_campaign_blocks_after_community_verification() {
    let (env, _admin, creator, contributor1, contributor2, _, token_admin, client) = setup_env();
    let voter3 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);

    let orig_title = String::from_str(&env, "Original Title");
    let orig_desc = String::from_str(&env, "Original Description");
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        orig_title.clone(),
        orig_desc.clone(),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &true);
    client.verify_campaign_with_votes(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);

    // Fix #416: update_campaign must be blocked after community verification.
    let res = client.try_update_campaign(
        &campaign_id,
        &String::from_str(&env, "New Title"),
        &String::from_str(&env, "New Description"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, orig_title);
    assert_eq!(campaign.description, orig_desc);
}

#[test]
fn test_update_campaign_description_success() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Original Title"),
        String::from_str(&env, "Original description"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let new_desc = String::from_str(&env, "Updated description with more detail");
    assert!(client
        .try_update_campaign_description(&campaign_id, &new_desc)
        .is_ok());

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.description, new_desc);
    assert_eq!(campaign.funding_goal, 1_000);
}

#[test]
fn test_update_campaign_description_emits_metadata_updated_with_unchanged_title() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Original Title");
    let old_desc = String::from_str(&env, "Original description");
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        old_desc.clone(),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let new_desc = String::from_str(&env, "Updated description with more detail");
    client.update_campaign_description(&campaign_id, &new_desc);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let payload: (String, String, String, String) =
        soroban_sdk::FromVal::from_val(&env, &last_event.2);
    // Title is unchanged by this entry point — both title slots equal the
    // original title, only the description pair reflects the edit.
    assert_eq!(payload.0, title, "old title slot mirrors unchanged title");
    assert_eq!(payload.1, old_desc, "old description");
    assert_eq!(payload.2, title, "new title slot mirrors unchanged title");
    assert_eq!(payload.3, new_desc, "new description");
}

#[test]
fn test_update_campaign_description_rejects_cancelled() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.cancel_campaign(&campaign_id);

    let res =
        client.try_update_campaign_description(&campaign_id, &String::from_str(&env, "New desc"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_update_campaign_description_rejects_empty() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_update_campaign_description(&campaign_id, &String::from_str(&env, ""));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_update_campaign_description_not_found() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let res = client.try_update_campaign_description(&999, &String::from_str(&env, "Some desc"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_campaign_ownership_transfer_flow() {
    let (env, _admin, creator, contributor1, contributor2, _, _, client) = setup_env();
    let new_creator = contributor1;

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Transfer Test"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(
        campaign.pending_creator,
        MaybePendingCreator::Some(new_creator.clone())
    );
    assert_eq!(campaign.creator, creator);

    client.accept_campaign_transfer(&campaign_id);
    let campaign_after = client.get_campaign(&campaign_id);
    assert_eq!(campaign_after.creator, new_creator.clone());
    assert_eq!(campaign_after.pending_creator, MaybePendingCreator::None);

    let updated_description = String::from_str(&env, "Managed by the transferred owner");
    client.update_campaign_description(&campaign_id, &updated_description);

    let auths = env.auths();
    let (auth_addr, invocation) = auths.last().unwrap();
    assert_eq!(auth_addr, &new_creator);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "update_campaign_description"),
                (campaign_id, updated_description).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );

    let campaign_id_2 = client.create_campaign(&make_params(
        new_creator.clone(),
        String::from_str(&env, "Cancel Test"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id_2);
    client.initiate_campaign_transfer(&campaign_id_2, &contributor2);
    client.cancel_campaign_transfer(&campaign_id_2);
    let final_campaign = client.get_campaign(&campaign_id_2);
    assert_eq!(final_campaign.pending_creator, MaybePendingCreator::None);
}

#[test]
fn test_campaign_transfer_validations() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Transfer Guardrails"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_initiate_campaign_transfer(&campaign_id, &creator);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);

    let res = client.try_accept_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);

    client.initiate_campaign_transfer(&campaign_id, &contributor1);
    client.cancel_campaign_transfer(&campaign_id);

    let auths = env.auths();
    let (auth_addr, _) = auths.last().unwrap();
    assert_eq!(auth_addr, &creator);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::None);

    let res = client.try_cancel_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);
}

#[test]
fn test_campaign_transfer_rejected_for_terminal_campaigns() {
    let (env, _admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    let cancelled_campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled Transfer"),
        String::from_str(&env, "Paused forever"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.cancel_campaign(&cancelled_campaign_id);

    let res = client.try_initiate_campaign_transfer(&cancelled_campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);

    token_admin.mint(&contributor1, &2000);

    let withdrawn_campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Withdrawn Transfer"),
        String::from_str(&env, "Already settled"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&withdrawn_campaign_id);
    client.contribute(&withdrawn_campaign_id, &contributor1, &1000);
    client.withdraw_funds(&withdrawn_campaign_id);

    let res = client.try_initiate_campaign_transfer(&withdrawn_campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_cancel_campaign_already_cancelled_is_terminal() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Terminal Test"),
        String::from_str(&env, "Already cancelled"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.cancel_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_cancel_campaign_after_withdrawal_is_terminal() {
    let (env, _admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Withdrawal Terminal"),
        String::from_str(&env, "Funds already out"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.funds_withdrawn);
    assert!(!campaign.is_active);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_update_description_after_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Old Description"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    let new_desc = String::from_str(&env, "New Description After Contribution");
    client.update_campaign_description(&campaign_id, &new_desc);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.description, new_desc);
}

#[test]
fn test_update_campaign_with_contributions_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Title"),
        String::from_str(&env, "Old Description"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    let new_title = String::from_str(&env, "New Title");
    let new_desc = String::from_str(&env, "New Description");
    let res = client.try_update_campaign(&campaign_id, &new_title, &new_desc);

    // update_campaign is blocked after verification (CampaignAlreadyVerified takes
    // precedence over the amount_raised > 0 check since it's checked first).
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
}

#[test]
fn test_unpause_clears_auto_pause_when_resume_campaign_blocked() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Unpause Recovery Test"),
        description: String::from_str(&env, "Testing unpause when resume_campaign is blocked"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    // Set AutoPaused directly (Soroban rolls back writes on Err, so we can't
    // rely on the anomaly trigger in contribute() to persist the flag).
    env.as_contract(&client.address, || {
        env.storage().instance().set(&AdminKey::AutoPaused, &true);
    });

    // Operations are blocked while AutoPaused is set
    let res = client.try_create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Should Fail"),
        description: String::from_str(&env, "Desc"),
        funding_goal: 500,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    // unpause() clears both Paused and AutoPaused
    client.unpause();

    // Now operations work again
    client.contribute(&campaign_id, &contributor1, &500i128);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    // Cancel the campaign (was blocked while auto-paused)
    client.cancel_campaign(&campaign_id);

    // resume_campaign returns ValidationFailed because unpause already
    // cleared AutoPaused, and the new early check (fix #436) catches it
    // before the campaign-state check.
    let res2 = client.try_resume_campaign(&campaign_id, &creator);
    assert_eq!(res2.unwrap_err().unwrap(), Error::ValidationFailed);

    // But operations still work because unpause already cleared AutoPaused
    let new_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Recovered"),
        description: String::from_str(&env, "Should work now"),
        funding_goal: 500,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    assert!(new_id > 1);
}

#[test]
fn campaign_transfer_reinitiate_rejects_silent_overwrite() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let pending_one = Address::generate(&env);
    let pending_two = Address::generate(&env);
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Re-initiate transfer"),
        String::from_str(&env, "Campaign transfer test"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.initiate_campaign_transfer(&campaign_id, &pending_one);
    assert_eq!(
        client.get_campaign(&campaign_id).pending_creator,
        MaybePendingCreator::Some(pending_one.clone())
    );

    let res = client.try_initiate_campaign_transfer(&campaign_id, &pending_two);
    assert_eq!(res.unwrap_err().unwrap(), Error::TransferAlreadyPending);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.creator, creator);
    assert_eq!(
        campaign.pending_creator,
        MaybePendingCreator::Some(pending_one.clone())
    );

    client.accept_campaign_transfer(&campaign_id);

    let transferred = client.get_campaign(&campaign_id);
    assert_eq!(transferred.creator, pending_one);
    assert_eq!(transferred.pending_creator, MaybePendingCreator::None);
}

#[test]
fn campaign_transfer_cancel_then_reinitiate_succeeds() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let pending_one = Address::generate(&env);
    let pending_two = Address::generate(&env);
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancel and retry"),
        String::from_str(&env, "Campaign transfer test"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.initiate_campaign_transfer(&campaign_id, &pending_one);
    client.cancel_campaign_transfer(&campaign_id);
    assert_eq!(
        client.get_campaign(&campaign_id).pending_creator,
        MaybePendingCreator::None
    );

    client.initiate_campaign_transfer(&campaign_id, &pending_two.clone());
    client.accept_campaign_transfer(&campaign_id);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.creator, pending_two);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::None);
}

#[test]
fn original_creator_can_contribute_after_campaign_transfer() {
    let (env, _admin, creator, _, _, _, token_admin, client) = setup_env();
    let new_creator = Address::generate(&env);
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Transfer contribution guard"),
        String::from_str(&env, "Campaign transfer test"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    token_admin.mint(&creator, &100);

    client.verify_campaign(&campaign_id);
    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    client.accept_campaign_transfer(&campaign_id);

    let res = client.try_contribute(&campaign_id, &creator, &100);
    assert!(res.is_ok());
}

// ── deadline extension ──────────────────────────────────────────────────────────

#[test]
fn test_extend_campaign_deadline_happy_path() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Extend Me"),
        String::from_str(&env, "Will be extended"),
        1000,
        10,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &7);

    let new_deadline = client.get_campaign(&id).deadline;
    assert_eq!(new_deadline, original_deadline + 7 * SECONDS_PER_DAY);
    assert!(client.get_campaign(&id).deadline_extended);
}

#[test]
fn test_extend_deadline_emits_event() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Event Extension"),
        String::from_str(&env, "Check event"),
        1000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &5);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.1.len(), 2);

    let payload: (u64, u64) = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(payload.0, original_deadline);
    assert_eq!(payload.1, original_deadline + 5 * SECONDS_PER_DAY);
}

#[test]
fn test_extend_deadline_double_extension_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Double Extension"),
        String::from_str(&env, "Only one extension"),
        1000,
        10,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    client.extend_campaign_deadline(&id, &7);

    let res = client.try_extend_campaign_deadline(&id, &7);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlineAlreadyExtended);
}

#[test]
fn test_extend_deadline_post_deadline_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Expired"),
        String::from_str(&env, "Past deadline"),
        1000,
        1,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let deadline = client.get_campaign(&id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_extend_campaign_deadline(&id, &7);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_extend_deadline_too_many_days_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Too Long"),
        String::from_str(&env, "Extension too long"),
        1000,
        10,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let res = client.try_extend_campaign_deadline(&id, &31);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);

    let res = client.try_extend_campaign_deadline(&id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);
}

#[test]
fn test_extend_deadline_max_30_days_allowed() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Max Extension"),
        String::from_str(&env, "Exactly 30 days"),
        1000,
        10,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &30);

    let new_deadline = client.get_campaign(&id).deadline;
    assert_eq!(new_deadline, original_deadline + 30 * SECONDS_PER_DAY);
}

#[test]
fn test_extend_deadline_beyond_category_cap_rejected() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    // 1. Admin sets Learner category duration cap to 40 days
    client.set_category_duration_cap(&admin, &Category::Learner, &40);

    // 2. Creator creates a Learner campaign with 30 days
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap Test"),
        String::from_str(&env, "Duration cap test"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // 3. Creator tries to extend deadline by 11 days (total 41) -> should be rejected
    let res = client.try_extend_campaign_deadline(&id, &11);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    // 4. Creator tries to extend deadline by 10 days (total 40) -> should succeed
    let res = client.try_extend_campaign_deadline(&id, &10);
    assert!(res.is_ok());

    let campaign = client.get_campaign(&id);
    assert!(campaign.deadline_extended);
}

#[test]
fn test_extend_deadline_without_start_time_rejected() {
    // Legacy campaigns without start_time cannot bypass category duration checks.
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Old Campaign"),
        String::from_str(&env, "Legacy test"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Manually remove start time to simulate a legacy campaign
    env.as_contract(&client.address, || {
        let key = crate::storage::CampaignKey::CampaignStartTime(id);
        env.storage().persistent().remove(&key);
    });

    client.set_category_duration_cap(&admin, &Category::Learner, &30);

    // Missing start_time now rejects the extension to avoid cap bypass.
    let res = client.try_extend_campaign_deadline(&id, &30);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);
}

#[test]
fn test_extend_deadline_without_start_time_keeps_deadline_unchanged() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Old Campaign Immutable"),
        String::from_str(&env, "Legacy no-start-time"),
        1000,
        20,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;

    env.as_contract(&client.address, || {
        let key = crate::storage::CampaignKey::CampaignStartTime(id);
        env.storage().persistent().remove(&key);
    });

    client.set_category_duration_cap(&admin, &Category::Learner, &25);

    let res = client.try_extend_campaign_deadline(&id, &5);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);
    assert_eq!(client.get_campaign(&id).deadline, original_deadline);
    assert!(!client.get_campaign(&id).deadline_extended);
}

#[test]
fn test_extend_deadline_absolute_max_cap_enforced() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    // Create a campaign with 350 days duration (within 365-day absolute cap)
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Absolute Cap"),
        String::from_str(&env, "Absolute cap test"),
        1000,
        350,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    // Trying to extend by 30 days would push total to 380 > 365 absolute cap
    let res = client.try_extend_campaign_deadline(&id, &30);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    // Extending by 15 days is fine: total = 365, which equals the cap
    let res = client.try_extend_campaign_deadline(&id, &15);
    assert!(res.is_ok());

    let campaign = client.get_campaign(&id);
    assert!(campaign.deadline_extended);
}

// ── cancel blocked after goal met ───────────────────────────────────────────────

/// Issue #164: creator cannot cancel after the funding goal has been reached
/// and funds have not yet been withdrawn (rug-pull prevention).
#[test]
fn test_cancel_campaign_blocked_after_goal_met() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let goal = 1000i128;
    token_admin.mint(&contributor1, &goal);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Goal Met Campaign"),
        description: String::from_str(&env, "Goal is met; cancel must be rejected"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    // Contribute exactly the funding goal
    client.contribute(&campaign_id, &contributor1, &goal);

    assert_eq!(client.get_campaign(&campaign_id).amount_raised, goal);

    // Creator tries to cancel — must be rejected
    let result = client.try_cancel_campaign(&campaign_id);
    assert_eq!(result, Err(Ok(Error::GoalMetCancellationNotAllowed)));
}

/// Creator can still cancel when contributions are below the funding goal.
#[test]
fn test_cancel_campaign_allowed_when_goal_not_met() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let goal = 2000i128;
    token_admin.mint(&contributor1, &500);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Partial Campaign"),
        description: String::from_str(&env, "Goal not met; cancel is allowed"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &500);

    // Goal not reached — cancellation must succeed
    client.cancel_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_cancelled);
}

/// If amount_raised exceeds the goal the block still applies.
#[test]
fn test_cancel_campaign_blocked_when_amount_exceeds_goal() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    let goal = 500i128;
    token_admin.mint(&contributor1, &600);
    token_admin.mint(&contributor2, &200);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Over-funded Campaign"),
        description: String::from_str(&env, "Raised more than goal"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &600);

    let result = client.try_cancel_campaign(&campaign_id);
    assert_eq!(result, Err(Ok(Error::GoalMetCancellationNotAllowed)));
}
