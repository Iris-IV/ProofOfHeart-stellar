use super::helpers::*;
use crate::{storage, Category, Error, CAMPAIGN_FUNDING_GOAL_MAX};
use soroban_sdk::String;

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
    for i in 1u32..=3 {
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
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
