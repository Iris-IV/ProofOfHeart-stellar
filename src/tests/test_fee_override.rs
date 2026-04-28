use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_campaign_fee_override_zero_percent() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Charity"),
        String::from_str(&env, "0% fee campaign"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.set_campaign_fee_override(&admin, &campaign_id, &0);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 0);
    assert_eq!(token.balance(&creator), 1000);
}

#[test]
fn test_campaign_fee_override_custom_percent() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Reduced Fee"),
        String::from_str(&env, "1% fee"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.set_campaign_fee_override(&admin, &campaign_id, &100);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 10);
    assert_eq!(token.balance(&creator), 990);
}

#[test]
fn test_campaign_fee_override_default_unchanged() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Default Fee"),
        String::from_str(&env, "Global fee applies"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);
}

#[test]
fn test_campaign_fee_override_above_max_rejected() {
    let (env, admin2, creator2, _c1, _c2, _token2, _token_admin2, client2) = setup_env();
    let id = client2.create_campaign(&make_params(
        creator2.clone(), String::from_str(&env, "X"),
        String::from_str(&env, "X"), 1, 1,
        Category::Learner, false, 0, 0i128,
    ));
    let res = client2.try_set_campaign_fee_override(&admin2, &id, &1001);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_campaign_fee_override_non_admin_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "X"),
        String::from_str(&env, "X"), 1, 1,
        Category::Learner, false, 0, 0i128,
    ));

    let impostor = Address::generate(&env);
    let res = client.try_set_campaign_fee_override(&impostor, &id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}
