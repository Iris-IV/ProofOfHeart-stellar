#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_env<'a>() -> (Env, Address, Address, ProofOfHeartClient<'a>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token_address, &300);

    (env, admin, creator, client)
}

#[test]
fn test_update_admin_success() {
    let (env, admin, _creator, client) = setup_env();
    let new_admin = Address::generate(&env);

    let res = client.try_update_admin(&admin, &new_admin);
    assert!(res.is_ok());
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_update_admin_rejects_non_admin() {
    let (env, _admin, creator, client) = setup_env();
    let new_admin = Address::generate(&env);

    let res = client.try_update_admin(&creator, &new_admin);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}
