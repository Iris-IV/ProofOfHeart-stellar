use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, String};

#[test]
fn test_create_and_validation() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Science Book");
    let desc = String::from_str(&env, "Teaching science to kids");

    let res = client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), desc.clone(), 0, 30, Category::Publisher, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);

    let res = client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), desc.clone(), 500, 0, Category::Publisher, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), desc.clone(), 500, 400, Category::Publisher, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), desc.clone(), 500, 30, Category::Educator, true, 1000, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueShareOnlyForStartup);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), title.clone(), desc.clone(), 2000, 30,
        Category::EducationalStartup, true, 1500, 0i128,
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
        creator.clone(), String::from_str(&env, "Admin Verification"),
        String::from_str(&env, "Admin verifies campaign"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);
}

#[test]
fn test_admin_verify_campaign_duplicate_attempt() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Duplicate Verification"),
        String::from_str(&env, "Cannot verify twice"), 1000, 30,
        Category::Publisher, false, 0, 0i128,
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
        creator.clone(), title.clone(), String::from_str(&env, ""), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    assert!(client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), String::from_str(&env, "a"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    )).is_ok());

    let desc_1000 = "a".repeat(1000);
    assert!(client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), String::from_str(&env, &desc_1000), 1000, 30,
        Category::Educator, false, 0, 0i128,
    )).is_ok());

    let desc_1001 = "a".repeat(1001);
    let res = client.try_create_campaign(&make_params(
        creator.clone(), title.clone(), String::from_str(&env, &desc_1001), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_title_length_boundaries() {
    extern crate std;
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let desc = String::from_str(&env, "Description");

    let res = client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, ""), desc.clone(), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    assert!(client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "a"), desc.clone(), 1000, 30,
        Category::Educator, false, 0, 0i128,
    )).is_ok());

    let title_100 = "a".repeat(100);
    assert!(client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, &title_100), desc.clone(), 1000, 30,
        Category::Educator, false, 0, 0i128,
    )).is_ok());

    let title_101 = "a".repeat(101);
    let res = client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, &title_101), desc.clone(), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_revenue_share_percentage_normalised_to_zero_when_disabled() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "No Revenue"),
        String::from_str(&env, "Desc"), 1000, 30, Category::Educator, false, 12345, 0i128,
    ));
    let campaign = client.get_campaign(&id);
    assert_eq!(campaign.revenue_share_percentage, 0);
    assert!(!campaign.has_revenue_sharing);
}

#[test]
fn test_revenue_share_above_max_rejected_even_without_flag() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Bad Revenue"),
        String::from_str(&env, "Desc"), 1000, 30, Category::Educator, false, 9999, 0i128,
    ));
    assert_eq!(client.get_campaign(&id).revenue_share_percentage, 0);
}

#[test]
fn test_revenue_share_with_flag_true_above_max_rejected() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let res = client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Too High"),
        String::from_str(&env, "Desc"), 1000, 30,
        Category::EducationalStartup, true, 5001, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidRevenueShare);
}

#[test]
fn test_campaign_count_cannot_reset_after_deployment() {
    let (env, _admin, creator, _, _, token, _, client) = setup_env();

    assert_eq!(client.get_campaign_count(), 0);
    for i in 1u32..=3 {
        let id = client.create_campaign(&make_params(
            creator.clone(), String::from_str(&env, "Campaign"),
            String::from_str(&env, "Desc"), 1000, 30, Category::Educator, false, 0, 0i128,
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
