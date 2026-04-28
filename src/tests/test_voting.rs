use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_community_voting_verification_success() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let voter3 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Community Verified"),
        String::from_str(&env, "Verify by voting"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    assert_eq!(client.get_approve_votes(&campaign_id), 2);
    assert_eq!(client.get_reject_votes(&campaign_id), 1);
    assert!(client.has_voted(&campaign_id, &contributor1));

    client.verify_campaign_with_votes(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);

    let res = client.try_verify_campaign_with_votes(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CommunityVerificationConflict);
}

#[test]
fn test_vote_prevents_double_voting_and_requires_token_holder() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let non_holder = Address::generate(&env);

    token_admin.mint(&contributor1, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Vote Safety"),
        String::from_str(&env, "No duplicate votes"), 500, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &false);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyVoted);

    let res = client.try_vote_on_campaign(&campaign_id, &non_holder, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);
}

#[test]
fn test_verify_campaign_quorum_and_threshold_edges() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let voter3 = Address::generate(&env);
    let voter4 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);
    token_admin.mint(&voter4, &100);

    client.set_voting_params(&admin, &4, &7500);
    assert_eq!(client.get_min_votes_quorum(), 4);
    assert_eq!(client.get_approval_threshold_bps(), 7500);

    let campaign_id_1 = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Quorum Campaign"),
        String::from_str(&env, "Needs 4 votes"), 700, 30,
        Category::Publisher, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id_1, &contributor1, &true);
    client.vote_on_campaign(&campaign_id_1, &contributor2, &true);
    client.vote_on_campaign(&campaign_id_1, &voter3, &true);

    let res = client.try_verify_campaign_with_votes(&campaign_id_1);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingQuorumNotMet);

    client.vote_on_campaign(&campaign_id_1, &voter4, &false);
    client.verify_campaign(&campaign_id_1);
    assert!(client.get_campaign(&campaign_id_1).is_verified);

    let campaign_id_2 = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Threshold Campaign"),
        String::from_str(&env, "Fails threshold"), 700, 30,
        Category::Publisher, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id_2, &contributor1, &true);
    client.vote_on_campaign(&campaign_id_2, &contributor2, &true);
    client.vote_on_campaign(&campaign_id_2, &voter3, &false);
    client.vote_on_campaign(&campaign_id_2, &voter4, &false);

    let res = client.try_verify_campaign_with_votes(&campaign_id_2);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingThresholdNotMet);
}

#[test]
fn test_set_voting_params_rejects_threshold_over_10000() {
    let (_env, admin, _, _, _, _, _, client) = setup_env();

    let res = client.try_set_voting_params(&admin, &3, &10000);
    assert!(res.is_ok());

    let res = client.try_set_voting_params(&admin, &3, &10001);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    let res = client.try_set_voting_params(&admin, &3, &u32::MAX);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_set_voting_params_rejects_non_admin() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let res = client.try_set_voting_params(&creator, &5, &7000);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);

    let random = Address::generate(&env);
    let res = client.try_set_voting_params(&random, &5, &7000);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_set_voting_params_emits_event() {
    let (env, admin, _, _, _, _, _, client) = setup_env();

    client.set_voting_params(&admin, &5, &7000);

    let events = env.events().all();
    let last_event = events.last().unwrap();

    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);

    let data: (u32, u32, u32, u32) = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data, (3, 5, 6000, 7000));
}

#[test]
fn test_vote_on_campaign_basic_flow() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Voting Test"),
        String::from_str(&env, "Test voting"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(client.get_approve_votes(&campaign_id), 1);
    assert_eq!(client.get_reject_votes(&campaign_id), 0);
    assert!(client.has_voted(&campaign_id, &contributor1));

    client.vote_on_campaign(&campaign_id, &contributor2, &false);
    assert_eq!(client.get_approve_votes(&campaign_id), 1);
    assert_eq!(client.get_reject_votes(&campaign_id), 1);
    assert!(client.has_voted(&campaign_id, &contributor2));
}

#[test]
fn test_vote_on_campaign_double_vote_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Double Vote Test"),
        String::from_str(&env, "Test double voting"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &false);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyVoted);
}

#[test]
fn test_vote_on_campaign_no_tokens_fails() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "No Token Vote Test"),
        String::from_str(&env, "Test voting without tokens"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);
}

#[test]
fn test_vote_on_campaign_below_minimum_balance_fails() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &100);
    client.set_min_voting_balance(&admin, &500);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Min Balance Vote Test"),
        String::from_str(&env, "Test voting with insufficient balance"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);
}

#[test]
fn test_vote_on_verified_campaign_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Already Verified"),
        String::from_str(&env, "Test voting on verified campaign"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.verify_campaign(&campaign_id);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
}
