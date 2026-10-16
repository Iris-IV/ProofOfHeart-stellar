//! Regression tests for #1037, #1041, #1052, #1054.
#![allow(deprecated)]
use super::helpers::*;
use crate::{voting, Error};
use soroban_sdk::{Address, String};

fn make_campaign(
    env: &Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    goal: i128,
    title: &str,
) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "D"),
        goal,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ))
}

// ── #1054 dynamic voting weight ─────────────────────────────────────────────

#[test]
fn dynamic_weight_pure_adds_history() {
    assert_eq!(voting::calculate_dynamic_weight(100, 0), 100);
    assert_eq!(voting::calculate_dynamic_weight(100, 50), 150);
    assert_eq!(voting::calculate_dynamic_weight(100, -10), 100);
}

#[test]
fn dynamic_weight_reflects_donor_history() {
    let (env, _admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &10_000);
    let id = make_campaign(&env, &client, &creator, 1_000, "T1");
    client.verify_campaign(&id);
    assert_eq!(client.get_dynamic_voting_weight(&id, &c1), 10_000);
    client.contribute(&id, &c1, &400);
    assert_eq!(client.get_donor_total_contributed(&c1), 400);
    // balance 9600 + history 400 = 10_000.
    assert_eq!(client.get_dynamic_voting_weight(&id, &c1), 10_000);
    // History carries to a second campaign.
    let id2 = make_campaign(&env, &client, &creator, 1_000, "T2");
    assert_eq!(client.get_dynamic_voting_weight(&id2, &c1), 10_000);
}

// ── #1052 cancellation penalty ──────────────────────────────────────────────

#[test]
fn cancellation_penalty_defaults_to_zero_and_validates() {
    let (_e, admin, _c, _c1, _c2, _t, _ta, client) = setup_env();
    assert_eq!(client.get_cancellation_penalty_bps(), 0);
    assert!(client
        .try_set_cancellation_penalty_bps(&admin, &500)
        .is_ok());
    assert_eq!(client.get_cancellation_penalty_bps(), 500);
    assert_eq!(
        client.try_set_cancellation_penalty_bps(&admin, &501),
        Err(Ok(Error::ValidationFailed))
    );
}

#[test]
fn cancellation_penalty_rejects_non_admin() {
    let (env, _admin, _creator, _, _, _, _, client) = setup_env();
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_set_cancellation_penalty_bps(&stranger, &100),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn cancel_without_penalty_preserves_full_refund() {
    let (env, _admin, creator, c1, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &2_000);
    let id = make_campaign(&env, &client, &creator, 5_000, "T");
    client.verify_campaign(&id);
    client.contribute(&id, &c1, &1_000);
    client.cancel_campaign(&id);
    client.claim_refund(&id, &c1);
    assert_eq!(token.balance(&c1), 2_000);
}

#[test]
fn cancel_with_penalty_charges_creator_to_admin() {
    let (env, admin, creator, c1, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &5_000);
    token_admin.mint(&creator, &5_000);
    let id = make_campaign(&env, &client, &creator, 5_000, "T");
    client.verify_campaign(&id);
    client.contribute(&id, &c1, &2_000);
    client.set_cancellation_penalty_bps(&admin, &100); // 1%
    let admin_before = token.balance(&admin);
    client.cancel_campaign(&id);
    // penalty = 1% of 2000 = 20 from creator to admin.
    assert_eq!(token.balance(&admin), admin_before + 20);
    client.claim_refund(&id, &c1);
    assert_eq!(token.balance(&c1), 5_000);
}

// ── #1037 emergency rescue ───────────────────────────────────────────────────

#[test]
fn rescue_rejects_native_token_and_bad_amount() {
    let (env, admin, _creator, c1, _, token, _, client) = setup_env();
    assert_eq!(
        client.try_rescue_stranded_tokens(&admin, &token.address, &c1, &10),
        Err(Ok(Error::ValidationFailed))
    );
    let foreign = env.register_stellar_asset_contract(Address::generate(&env));
    assert_eq!(
        client.try_rescue_stranded_tokens(&admin, &foreign, &c1, &0),
        Err(Ok(Error::ValidationFailed))
    );
}

#[test]
fn rescue_rejects_non_admin() {
    let (env, _admin, _creator, c1, _, _, _, client) = setup_env();
    let foreign = env.register_stellar_asset_contract(Address::generate(&env));
    assert_eq!(
        client.try_rescue_stranded_tokens(&c1, &foreign, &c1, &10),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn rescue_moves_stranded_foreign_tokens() {
    let (env, admin, _creator, c1, _, _, _, client) = setup_env();
    let foreign_addr = env.register_stellar_asset_contract(admin.clone());
    let foreign_admin = TokenAdminClient::new(&env, &foreign_addr);
    let foreign = TokenClient::new(&env, &foreign_addr);
    foreign_admin.mint(&client.address, &777);
    assert_eq!(foreign.balance(&client.address), 777);
    client.rescue_stranded_tokens(&admin, &foreign_addr, &c1, &277);
    assert_eq!(foreign.balance(&c1), 277);
    assert_eq!(foreign.balance(&client.address), 500);
}
