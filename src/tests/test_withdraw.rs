use super::helpers::*;
use crate::{storage, Category, DataKey, Error};
use soroban_sdk::String;

#[test]
fn test_withdraw_before_deadline_goal_not_met_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Early Withdraw"),
        String::from_str(&env, "Desc"), 10_000, 30,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);
}

#[test]
fn test_withdraw_after_deadline_goal_not_met_returns_typed_error() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Late Withdraw"),
        String::from_str(&env, "Desc"), 10_000, 1,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);
}

#[test]
fn test_withdraw_funds_requires_verified_campaign() {
    let (env, _admin, creator, _contributor1, _, _token, token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Unverified Campaign"),
        String::from_str(&env, "Description"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));

    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    token_admin.mint(&contract_id, &1500);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.amount_raised = 1500;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let result = client.try_withdraw_funds(&campaign_id);
    assert_eq!(result.unwrap_err().unwrap(), Error::CampaignNotVerified);
}

#[test]
fn test_withdraw_funds_succeeds_when_verified() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Verified Campaign"),
        String::from_str(&env, "Description"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1500);

    assert!(client.try_withdraw_funds(&campaign_id).is_ok());
}

#[test]
fn test_claim_refund_removes_contribution_storage_key() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Refund storage cleanup"),
        String::from_str(&env, "Contribution key should be removed"), 5_000, 30,
        Category::Learner, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1_000);
    client.cancel_campaign(&campaign_id);

    env.as_contract(&client.address, || {
        assert!(env.storage().persistent()
            .has(&DataKey::Contribution(campaign_id, contributor1.clone())));
    });

    client.claim_refund(&campaign_id, &contributor1);

    env.as_contract(&client.address, || {
        assert!(!env.storage().persistent()
            .has(&DataKey::Contribution(campaign_id, contributor1.clone())));
    });
}

#[test]
fn test_view_function_get_campaign_not_found() {
    let (_env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let res = client.try_get_campaign(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}
