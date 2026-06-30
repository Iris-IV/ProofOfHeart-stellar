use super::helpers::*;
use crate::{Category, Error, CAMPAIGN_FUNDING_GOAL_MAX};
use soroban_sdk::{Address, String};

#[test]
fn test_category_funding_goal_cap_enforced() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_funding_goal_cap(&admin, &Category::Educator, &500);
    assert_eq!(
        client.get_category_funding_goal_cap(&Category::Educator),
        500
    );

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator high"),
        String::from_str(&env, "Too much"),
        501,
        30,
        Category::Educator,
        false,
        0,
        0,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator OK"),
        String::from_str(&env, "Within cap"),
        500,
        30,
        Category::Educator,
        false,
        0,
        0,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_funding_goal_cap_other_categories_unaffected() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_funding_goal_cap(&admin, &Category::Learner, &500);

    let educator_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator"),
        String::from_str(&env, "Global cap still applies"),
        1_000,
        30,
        Category::Educator,
        false,
        0,
        0,
    ));
    assert_eq!(educator_id, 1);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Learner"),
        String::from_str(&env, "Too much"),
        501,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);
}

#[test]
fn test_remove_category_funding_goal_cap_reverts_to_global_max() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_funding_goal_cap(&admin, &Category::Publisher, &500);
    client.remove_category_funding_goal_cap(&admin, &Category::Publisher);
    assert_eq!(
        client.get_category_funding_goal_cap(&Category::Publisher),
        CAMPAIGN_FUNDING_GOAL_MAX
    );

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Publisher"),
        String::from_str(&env, "Back to global max"),
        1_000,
        30,
        Category::Publisher,
        false,
        0,
        0,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_funding_goal_cap_respects_lower_global_max() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_funding_goal_cap(&admin, &Category::Learner, &1_000);
    client.set_max_campaign_funding_goal(&admin, &800);
    assert_eq!(
        client.get_category_funding_goal_cap(&Category::Learner),
        800
    );

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Learner high"),
        String::from_str(&env, "Above global"),
        801,
        30,
        Category::Learner,
        false,
        0,
        0,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooHigh);
}

#[test]
fn test_category_funding_goal_cap_validation() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let zero = client.try_set_category_funding_goal_cap(&admin, &Category::Learner, &0);
    assert_eq!(zero.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);

    client.set_min_campaign_funding_goal(&admin, &100);
    let below_min = client.try_set_category_funding_goal_cap(&admin, &Category::Learner, &99);
    assert_eq!(below_min.unwrap_err().unwrap(), Error::ValidationFailed);

    client.set_max_campaign_funding_goal(&admin, &800);
    let above_global = client.try_set_category_funding_goal_cap(&admin, &Category::Learner, &801);
    assert_eq!(above_global.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_category_funding_goal_cap_non_admin_rejected() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let impostor = Address::generate(&env);
    let res = client.try_set_category_funding_goal_cap(&impostor, &Category::Learner, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}
