use super::helpers::*;
use crate::{storage::set_min_campaign_funding_goal, Category, Error, CAMPAIGN_FUNDING_GOAL_MIN};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_init_only_once() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();
    let res = client.try_init(&admin, &token.address, &300);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);
}

#[test]
fn test_platform_fee_cap_enforcement() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token = soroban_sdk::token::Client::new(&env, &token_address);
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    let client = crate::ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token_address, &5000);
    env.as_contract(&client.address, || set_min_campaign_funding_goal(&env, 1));
    assert_eq!(client.get_platform_fee(), 1000);

    token_admin.mint(&contributor, &2000);

    let title = String::from_str(&env, "Fee Cap Test");
    let desc = String::from_str(&env, "Testing platform fee cap enforcement");
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title,
        desc,
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &1000);

    assert_eq!(token.balance(&contributor), 1000);
    assert_eq!(token.balance(&client.address), 1000);

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 100);
    assert_eq!(token.balance(&creator), 900);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_platform_fee_exact_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(admin.clone());
    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    let client = crate::ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token_address, &1000);
    assert_eq!(client.get_platform_fee(), 1000);
}

#[test]
fn test_reinit_prevention() {
    let (env, admin, _, _, _, token, _, client) = setup_env();

    let attacker = Address::generate(&env);
    let fake_token = Address::generate(&env);

    let res = client.try_init(&attacker, &fake_token, &0);
    assert!(res.is_err());

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
}

#[test]
fn test_initialization_getters() {
    let (_, admin, _, _, _, token, _, client) = setup_env();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
    assert_eq!(client.get_campaign_count(), 0);
}

#[test]
fn test_init_returns_already_initialized_error() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();
    let err = client
        .try_init(&admin, &token.address, &300)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_init_preserves_all_config_state() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();

    let _ = client.try_init(&admin, &token.address, &999);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
    assert_eq!(client.get_campaign_count(), 0);
    assert_eq!(client.get_version(), 1);
    assert_eq!(
        client.get_min_votes_quorum(),
        crate::voting::DEFAULT_MIN_VOTES_QUORUM
    );
    assert_eq!(
        client.get_approval_threshold_bps(),
        crate::voting::DEFAULT_APPROVAL_THRESHOLD_BPS
    );
}

#[test]
fn test_init_rejects_every_subsequent_call() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();

    for _ in 0..3 {
        let res = client.try_init(&admin, &token.address, &300);
        assert_eq!(
            res.unwrap_err().unwrap(),
            Error::AlreadyInitialized,
            "expected AlreadyInitialized on every repeated call"
        );
    }
}

#[test]
fn test_init_cannot_overwrite_after_campaign_created() {
    let (env, admin, creator, _c1, _c2, token, _token_admin, client) = setup_env();

    let _ = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Test Campaign"),
        String::from_str(&env, "Testing init idempotency after state change"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(client.get_campaign_count(), 1);

    let res = client.try_init(&admin, &token.address, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);

    assert_eq!(client.get_campaign_count(), 1);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
}

#[test]
fn test_min_campaign_funding_goal_boundary_and_admin_update() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) =
        setup_env_with_default_min();

    assert_eq!(
        client.get_min_campaign_funding_goal(),
        CAMPAIGN_FUNDING_GOAL_MIN
    );

    let title = String::from_str(&env, "Minimum Goal");
    let desc = String::from_str(&env, "Checks funding goal floor");

    let below_min = CAMPAIGN_FUNDING_GOAL_MIN - 1;
    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        below_min,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooLow);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        CAMPAIGN_FUNDING_GOAL_MIN,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(campaign_id, 1);

    client.set_min_campaign_funding_goal(&admin, &500);
    assert_eq!(client.get_min_campaign_funding_goal(), 500);

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        title.clone(),
        desc.clone(),
        499,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalTooLow);
}
