//! Regression tests for #1037, #1041, #1052, #1054, and issues 1-4.
#![allow(deprecated)]
use super::helpers::*;
use crate::{voting, CompactString, Error};
use soroban_sdk::{Address, BytesN, String};

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

// ── Issue 1: Vote delegation ──────────────────────────────────────────────────

#[test]
fn vote_delegation_happy_path() {
    let (env, _admin, creator, c1, c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &5_000);
    token_admin.mint(&c2, &5_000);

    let id = make_campaign(&env, &client, &creator, 1_000, "Delegation Test");
    assert_eq!(client.get_delegate(&c1), None);

    // c1 delegates to c2
    client.delegate_votes(&c1, &c2);
    assert_eq!(client.get_delegate(&c1), Some(c2.clone()));

    // c2 casts delegated vote on behalf of c1
    client.cast_delegated_vote(&id, &c2, &c1, &true);

    // c1 cannot vote again directly
    assert_eq!(
        client.try_vote_on_campaign(&id, &c1, &true),
        Err(Ok(Error::AlreadyVoted))
    );

    // c2 cannot cast vote on behalf of c1 again
    assert_eq!(
        client.try_cast_delegated_vote(&id, &c2, &c1, &true),
        Err(Ok(Error::AlreadyVoted))
    );
}

#[test]
fn vote_delegation_self_delegation_rejected() {
    let (_env, _admin, _creator, c1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &5_000);

    assert_eq!(
        client.try_delegate_votes(&c1, &c1),
        Err(Ok(Error::ValidationFailed))
    );
}

#[test]
fn vote_delegation_non_token_holder_rejected() {
    let (_env, _admin, _creator, c1, c2, _token, _token_admin, client) = setup_env();
    // c1 has 0 tokens
    assert_eq!(
        client.try_delegate_votes(&c1, &c2),
        Err(Ok(Error::NotTokenHolder))
    );
}

#[test]
fn vote_delegation_unauthorized_delegate_rejected() {
    let (env, _admin, creator, c1, c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &5_000);
    let id = make_campaign(&env, &client, &creator, 1_000, "Unauth Delegate Test");

    // c2 tries to vote on behalf of c1 without delegation
    assert_eq!(
        client.try_cast_delegated_vote(&id, &c2, &c1, &true),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn vote_delegation_undelegate_flow() {
    let (_env, _admin, _creator, c1, c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&c1, &5_000);

    client.delegate_votes(&c1, &c2);
    assert_eq!(client.get_delegate(&c1), Some(c2.clone()));

    client.undelegate_votes(&c1);
    assert_eq!(client.get_delegate(&c1), None);

    // Undelegating again fails
    assert_eq!(
        client.try_undelegate_votes(&c1),
        Err(Ok(Error::ValidationFailed))
    );
}

// ── Issue 2: CompactString (BytesN<32>) ────────────────────────────────────────

#[test]
fn compact_string_roundtrip() {
    let env = Env::default();
    let text = "Proof of Heart";
    let compact = CompactString::from_str(&env, text).unwrap();

    assert_eq!(compact.len(), 14);
    assert!(!compact.is_empty());

    let recovered = compact.to_string(&env);
    assert_eq!(recovered, String::from_str(&env, text));
}

#[test]
fn compact_string_from_string() {
    let env = Env::default();
    let original = String::from_str(&env, "Web3Launchpad");
    let compact = CompactString::from_string(&env, &original).unwrap();

    assert_eq!(compact.to_string(&env), original);
    assert_eq!(compact.len(), 13);
}

#[test]
fn compact_string_exact_32_bytes() {
    let env = Env::default();
    let text32 = "12345678901234567890123456789012"; // 32 bytes
    let compact = CompactString::from_str(&env, text32).unwrap();

    assert_eq!(compact.len(), 32);
    assert_eq!(compact.to_string(&env), String::from_str(&env, text32));
}

#[test]
fn compact_string_exceeding_32_bytes_fails() {
    let env = Env::default();
    let text33 = "123456789012345678901234567890123"; // 33 bytes
    assert_eq!(
        CompactString::from_str(&env, text33),
        Err(Error::ValidationFailed)
    );
}

#[test]
fn compact_string_empty() {
    let env = Env::default();
    let compact = CompactString::from_str(&env, "").unwrap();
    assert_eq!(compact.len(), 0);
    assert!(compact.is_empty());
    assert_eq!(compact.to_string(&env), String::from_str(&env, ""));
}

// ── Issue 3: Cryptographic Proof-of-Impact ─────────────────────────────────────

#[test]
fn proof_of_impact_recorded_success() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let id = make_campaign(&env, &client, &creator, 1_000, "Proof Campaign");

    assert_eq!(client.get_proof_of_impact(&id), None);

    let dummy_hash = BytesN::from_array(&env, &[42u8; 32]);
    client.record_proof_of_impact(&id, &admin, &dummy_hash);

    let proof = client.get_proof_of_impact(&id).unwrap();
    assert_eq!(proof.verifier, admin);
    assert_eq!(proof.proof_hash, dummy_hash);
    assert_eq!(proof.timestamp, 1_700_000_000);
}

#[test]
fn proof_of_impact_rejects_non_admin() {
    let (env, _admin, creator, c1, _, _, _, client) = setup_env();
    let id = make_campaign(&env, &client, &creator, 1_000, "Proof Non-admin");

    let dummy_hash = BytesN::from_array(&env, &[7u8; 32]);
    assert_eq!(
        client.try_record_proof_of_impact(&id, &c1, &dummy_hash),
        Err(Ok(Error::NotAuthorized))
    );
}

#[test]
fn proof_of_impact_rejects_nonexistent_campaign() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let dummy_hash = BytesN::from_array(&env, &[9u8; 32]);
    assert_eq!(
        client.try_record_proof_of_impact(&999, &admin, &dummy_hash),
        Err(Ok(Error::CampaignNotFound))
    );
}

// ── Issue 4: Creator bucket balance isolation ─────────────────────────────────

fn make_revenue_campaign(
    env: &Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    goal: i128,
    title: &str,
    rev_share_bps: u32,
) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "D"),
        goal,
        30,
        Category::EducationalStartup,
        true,
        rev_share_bps,
        0i128,
    ))
}

#[test]
fn creator_bucket_isolation_on_deposit() {
    let (env, _admin, creator, c1, _, _token, token_admin, client) = setup_env();
    // 2000 bps = 20% to contributors, 80% to creator bucket
    let id = make_revenue_campaign(&env, &client, &creator, 1_000, "Startup Rev 1", 2000);
    client.verify_campaign(&id);

    token_admin.mint(&c1, &2_000);
    client.contribute(&id, &c1, &1_000);

    advance_past_deadline(&env, &client, id);
    client.withdraw_funds(&id);

    // Initial bucket balance is 0
    assert_eq!(client.get_creator_bucket_balance(&id, &creator), 0);

    // Creator deposits 1000 revenue
    token_admin.mint(&creator, &1_000);
    client.deposit_revenue(&id, &1_000);

    // Creator's 80% is isolated in CreatorRevenueBucket
    assert_eq!(client.get_creator_bucket_balance(&id, &creator), 800);
}

#[test]
fn creator_bucket_claim_depletes_bucket() {
    let (env, _admin, creator, c1, _, token, token_admin, client) = setup_env();
    let id = make_revenue_campaign(&env, &client, &creator, 1_000, "Startup Rev 2", 2000);
    client.verify_campaign(&id);

    token_admin.mint(&c1, &2_000);
    client.contribute(&id, &c1, &1_000);

    advance_past_deadline(&env, &client, id);
    client.withdraw_funds(&id);

    token_admin.mint(&creator, &1_000);
    client.deposit_revenue(&id, &1_000);
    assert_eq!(client.get_creator_bucket_balance(&id, &creator), 800);

    let creator_bal_before = token.balance(&creator);
    client.claim_creator_revenue(&id);

    assert_eq!(token.balance(&creator), creator_bal_before + 800);
    assert_eq!(client.get_creator_bucket_balance(&id, &creator), 0);

    // Second claim fails with NoFundsToWithdraw
    assert_eq!(
        client.try_claim_creator_revenue(&id),
        Err(Ok(Error::NoFundsToWithdraw))
    );
}

#[test]
fn creator_bucket_isolation_multiple_campaigns() {
    let (env, _admin, creator, c1, _, _token, token_admin, client) = setup_env();
    let id1 = make_revenue_campaign(&env, &client, &creator, 1_000, "Startup Rev A", 2000);
    let id2 = make_revenue_campaign(&env, &client, &creator, 1_000, "Startup Rev B", 3000);
    client.verify_campaign(&id1);
    client.verify_campaign(&id2);

    token_admin.mint(&c1, &5_000);
    client.contribute(&id1, &c1, &1_000);
    client.contribute(&id2, &c1, &1_000);

    advance_past_deadline(&env, &client, id1);
    advance_past_deadline(&env, &client, id2);
    client.withdraw_funds(&id1);
    client.withdraw_funds(&id2);

    token_admin.mint(&creator, &2_000);
    client.deposit_revenue(&id1, &1_000);

    // Only id1 bucket has balance (80% of 1000 = 800); id2 bucket is untouched (0)
    assert_eq!(client.get_creator_bucket_balance(&id1, &creator), 800);
    assert_eq!(client.get_creator_bucket_balance(&id2, &creator), 0);

    // Now deposit into id2 (70% to creator = 700)
    client.deposit_revenue(&id2, &1_000);
    assert_eq!(client.get_creator_bucket_balance(&id1, &creator), 800);
    assert_eq!(client.get_creator_bucket_balance(&id2, &creator), 700);
}
