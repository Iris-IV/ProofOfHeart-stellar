use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_category_duration_cap_enforced() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_duration_cap(&admin, &Category::EducationalStartup, &60);

    let res = client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Startup"),
        String::from_str(&env, "Startup desc"), 1000, 61,
        Category::EducationalStartup, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Startup OK"),
        String::from_str(&env, "Startup desc"), 1000, 60,
        Category::EducationalStartup, false, 0, 0i128,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_duration_cap_other_categories_unaffected() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_category_duration_cap(&admin, &Category::Learner, &10);

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Educator"),
        String::from_str(&env, "Full duration"), 1000, 365,
        Category::Educator, false, 0, 0i128,
    ));
    assert_eq!(id, 1);

    let res = client.try_create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Learner"),
        String::from_str(&env, "Too long"), 1000, 11,
        Category::Learner, false, 0, 0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);
}

#[test]
fn test_category_duration_cap_default_unchanged() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Default"),
        String::from_str(&env, "Default cap"), 1000, 365,
        Category::Publisher, false, 0, 0i128,
    ));
    assert_eq!(id, 1);
}

#[test]
fn test_category_duration_cap_above_365_rejected() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let res = client.try_set_category_duration_cap(&admin, &Category::Learner, &366);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_category_duration_cap_non_admin_rejected() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let impostor = Address::generate(&env);
    let res = client.try_set_category_duration_cap(&impostor, &Category::Learner, &30);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}
