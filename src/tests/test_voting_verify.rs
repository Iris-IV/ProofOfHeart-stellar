use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_vote_on_cancelled_campaign_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Cancelled Campaign"),
        String::from_str(&env, "Test voting on cancelled campaign"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.cancel_campaign(&campaign_id);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_vote_on_campaign_past_deadline_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Deadline Vote"),
        description: String::from_str(&env, "Voting deadline gate"),
        funding_goal: 1000,
        duration_days: 1,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });

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

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_vote_on_campaign_after_withdraw_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Withdrawn Vote"),
        description: String::from_str(&env, "Voting withdrawn gate"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_vote_on_campaign_token_weighted() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Weighted Vote Test"),
        String::from_str(&env, "Test token-weighted voting"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &false);

    assert_eq!(client.get_approve_votes(&campaign_id), 1);
    assert_eq!(client.get_reject_votes(&campaign_id), 1);
}

#[test]
fn test_verify_campaign_with_votes_quorum_not_met() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);
    client.set_voting_params(&admin, &5, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Quorum Test"),
        String::from_str(&env, "Test quorum requirement"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);

    let res = client.try_verify_campaign_with_votes(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingQuorumNotMet);
}

#[test]
fn test_verify_campaign_with_votes_threshold_not_met() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    let voter3 = Address::generate(&env);
    token_admin.mint(&voter3, &1000);

    client.set_voting_params(&admin, &3, &8000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Threshold Test"),
        String::from_str(&env, "Test approval threshold"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    let res = client.try_verify_campaign_with_votes(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingThresholdNotMet);
}

#[test]
fn test_verify_campaign_with_votes_success() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    let voter3 = Address::generate(&env);
    token_admin.mint(&voter3, &1000);

    client.set_voting_params(&admin, &3, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Success Verify Test"),
        String::from_str(&env, "Test successful verification"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    client.verify_campaign_with_votes(&campaign_id);

    assert!(client.get_campaign(&campaign_id).is_verified);
}

#[test]
fn test_vote_on_nonexistent_campaign() {
    let (_env, _admin, _creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let res = client.try_vote_on_campaign(&999, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_min_voting_balance_threshold_enforcement() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &50);
    token_admin.mint(&contributor2, &200);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Min Balance Vote Test"),
        String::from_str(&env, "Testing minimum voting balance"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));

    client.set_min_voting_balance(&admin, &100);
    assert_eq!(client.get_min_voting_balance(), 100);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);

    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    assert!(client.has_voted(&campaign_id, &contributor2));
    assert_eq!(client.get_approve_votes(&campaign_id), 1);

    client.set_min_voting_balance(&admin, &0);
    assert_eq!(client.get_min_voting_balance(), 0);

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    assert!(client.has_voted(&campaign_id, &contributor1));
    assert_eq!(client.get_approve_votes(&campaign_id), 2);
}
