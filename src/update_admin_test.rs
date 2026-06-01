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

    let res = client.try_update_admin(&new_admin);
    assert!(res.is_ok());
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    let accept_res = client.try_accept_admin_transfer();
    assert!(accept_res.is_ok());
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn test_update_admin_requires_stored_admin_auth() {
    let (env, admin, _creator, client) = setup_env();
    let new_admin = Address::generate(&env);

    // update_admin no longer accepts a caller-supplied admin address;
    // the contract fetches the stored admin and requires auth from it.
    let res = client.try_update_admin(&new_admin);
    assert!(res.is_ok());

    // Verify that the stored admin's address was the authorized signer.
    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| addr == &admin),
        "stored admin must be the authorized address"
    );
}

#[test]
fn test_cancel_admin_transfer() {
    let (env, admin, _creator, client) = setup_env();
    let new_admin = Address::generate(&env);

    client.update_admin(&new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin));

    let cancel_res = client.try_cancel_admin_transfer(&admin);
    assert!(cancel_res.is_ok());
    assert_eq!(client.get_pending_admin(), None);
    assert_eq!(client.get_admin(), admin);
}
