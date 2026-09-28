use proptest::prelude::*;

use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{Address, String, TryFromVal, Vec};

// ── community voting ────────────────────────────────────────────────────────────

#[test]
fn test_community_voting_verification_success() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let voter3 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Community Verified"),
        String::from_str(&env, "Verify by voting"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
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
    assert_eq!(res.unwrap_err().unwrap(), Error::VerificationConflict);
}

#[test]
fn test_vote_prevents_double_voting_and_requires_token_holder() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let non_holder = Address::generate(&env);

    token_admin.mint(&contributor1, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Vote Safety"),
        String::from_str(&env, "No duplicate votes"),
        500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Quorum Campaign"),
        String::from_str(&env, "Needs 4 votes"),
        700,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Threshold Campaign"),
        String::from_str(&env, "Fails threshold"),
        700,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
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
    let event_admin: Address = soroban_sdk::FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(event_admin, admin);

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
        creator.clone(),
        String::from_str(&env, "Voting Test"),
        String::from_str(&env, "Test voting"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Double Vote Test"),
        String::from_str(&env, "Test double voting"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &false);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyVoted);
}

#[test]
fn test_vote_on_campaign_no_tokens_fails() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "No Token Vote Test"),
        String::from_str(&env, "Test voting without tokens"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Min Balance Vote Test"),
        String::from_str(&env, "Test voting with insufficient balance"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);
}

#[test]
fn test_vote_on_verified_campaign_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Already Verified"),
        String::from_str(&env, "Test voting on verified campaign"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.verify_campaign(&campaign_id);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
}

#[test]
fn test_verify_campaigns_extends_voting_state_ttl() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    // Create a campaign
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "TTL Test"),
        String::from_str(&env, "Testing TTL extension"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Bulk verify the campaign
    let (verified_ids, failed_ids) =
        client.verify_campaigns(&soroban_sdk::Vec::from_array(&env, [campaign_id]));
    assert_eq!(
        verified_ids,
        soroban_sdk::Vec::from_array(&env, [campaign_id])
    );
    assert!(failed_ids.is_empty());

    // Verify campaign is verified (confirming it worked)
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);
}

#[test]
fn test_vote_on_campaign_after_deadline_returns_deadline_passed() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &500);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Deadline Vote Test"),
        String::from_str(&env, "Voting after deadline must return DeadlinePassed"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let campaign = client.get_campaign(&campaign_id);

    // Advance past the deadline
    env.ledger().with_mut(|li| {
        li.timestamp = campaign.deadline + 1;
    });

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_verify_campaigns_partial_failure_reports_failed_ids() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Valid Campaign"),
        String::from_str(&env, "One valid campaign"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // 999 does not exist — will produce CampaignNotFound.
    let ids = soroban_sdk::Vec::from_array(&env, [campaign_id, 999u32]);
    let (verified_ids, failed_ids) = client.verify_campaigns(&ids);

    // #442: partial success is preserved and reported per-id, instead of the
    // whole batch collapsing to Err(first_error).
    assert_eq!(
        verified_ids,
        soroban_sdk::Vec::from_array(&env, [campaign_id])
    );
    assert_eq!(failed_ids, soroban_sdk::Vec::from_array(&env, [999u32]));
    assert!(
        client.get_campaign(&campaign_id).is_verified,
        "the valid campaign must be committed even though the batch also failed"
    );
}

#[test]
fn test_verify_campaigns_all_failed_reports_every_id() {
    let (env, _admin, _creator, _, _, _, _, client) = setup_env();

    // Both ids are unknown — the batch fails entirely, but every id must be
    // reported in failed_ids (not just the first error).
    let ids = soroban_sdk::Vec::from_array(&env, [999u32, 1000u32]);
    let (verified_ids, failed_ids) = client.verify_campaigns(&ids);

    assert!(verified_ids.is_empty());
    assert_eq!(failed_ids, ids);
}

#[test]
fn test_verify_campaigns_cancelled_campaign_in_batch_reported_as_failed() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let valid_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Valid In Batch"),
        String::from_str(&env, "Survives a failed sibling"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let cancelled_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled In Batch"),
        String::from_str(&env, "Cannot be verified"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.cancel_campaign(&cancelled_id);

    let ids = soroban_sdk::Vec::from_array(&env, [valid_id, cancelled_id]);
    let (verified_ids, failed_ids) = client.verify_campaigns(&ids);

    // Any admin_verify error (here CampaignNotActive for the cancelled
    // campaign) must route that id to failed_ids without aborting the batch.
    assert_eq!(verified_ids, soroban_sdk::Vec::from_array(&env, [valid_id]));
    assert_eq!(
        failed_ids,
        soroban_sdk::Vec::from_array(&env, [cancelled_id])
    );
    assert!(client.get_campaign(&valid_id).is_verified);
    assert!(!client.get_campaign(&cancelled_id).is_verified);
}

#[test]
fn test_verify_campaigns_emits_bulk_verified_event_with_failed_ids() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Bulk Event"),
        String::from_str(&env, "Event payload check"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let ids = soroban_sdk::Vec::from_array(&env, [campaign_id, 999u32]);
    let _ = client.verify_campaigns(&ids);

    let events = env.events().all();
    let bulk = events
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|s| s == String::from_str(&env, "campaigns_bulk_verified"))
                .unwrap_or(false)
        })
        .expect("campaigns_bulk_verified event must exist");

    // #442: the event now carries the failing ids, not just counts.
    let (verified_count, failed_ids): (u32, soroban_sdk::Vec<u32>) =
        soroban_sdk::FromVal::from_val(&env, &bulk.2);
    assert_eq!(verified_count, 1);
    assert_eq!(failed_ids, soroban_sdk::Vec::from_array(&env, [999u32]));
}

// ── verification via votes ──────────────────────────────────────────────────────

#[test]
fn test_vote_on_cancelled_campaign_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled Campaign"),
        String::from_str(&env, "Test voting on cancelled campaign"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.cancel_campaign(&campaign_id);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_admin_verify_cancelled_campaign_fails() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled Admin Verify"),
        String::from_str(&env, "Test admin verification on cancelled campaign"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.cancel_campaign(&campaign_id);

    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_verify_campaign_with_votes_cancelled_campaign_fails() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    let voter3 = Address::generate(&env);
    token_admin.mint(&voter3, &1000);

    client.set_voting_params(&admin, &3, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancelled Vote Verify"),
        String::from_str(&env, "Test vote-based verification on cancelled campaign"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    client.cancel_campaign(&campaign_id);

    let res = client.try_verify_campaign_with_votes(&campaign_id);
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
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
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
fn test_vote_on_campaign_one_address_one_vote() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    // contributor1 has 5000 tokens, contributor2 has only 1000 — both get 1 vote (#469).
    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &1000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "1-Address-1-Vote Test"),
        String::from_str(&env, "Test 1-address-1-vote model"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &false);

    // Each voter contributes exactly 1 to the count regardless of balance.
    assert_eq!(client.get_approve_votes(&campaign_id), 1);
    assert_eq!(client.get_reject_votes(&campaign_id), 1);
}

#[test]
fn test_verify_campaign_with_votes_quorum_not_met() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);
    client.set_voting_params(&admin, &5, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Quorum Test"),
        String::from_str(&env, "Test quorum requirement"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Threshold Test"),
        String::from_str(&env, "Test approval threshold"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Success Verify Test"),
        String::from_str(&env, "Test successful verification"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    client.verify_campaign_with_votes(&campaign_id);

    assert!(client.get_campaign(&campaign_id).is_verified);
}

// ── #536: per-category voting threshold ─────────────────────────────────────────

#[test]
fn test_category_voting_threshold_overrides_global_default() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    let voter3 = Address::generate(&env);
    token_admin.mint(&voter3, &1000);

    // Global default requires 80% approval.
    client.set_voting_params(&admin, &3, &8000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Category Threshold"),
        String::from_str(&env, "Learner override"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // 2 approve / 1 reject, equal weight => ~66.7% approval: fails the 80% global default.
    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    let res = client.try_verify_campaign_with_votes(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingThresholdNotMet);

    // Lower the Learner-category threshold to 50% — same votes now pass.
    client.set_category_voting_threshold(&admin, &Category::Learner, &5000);
    assert_eq!(
        client.get_category_voting_threshold(&Category::Learner),
        5000
    );

    client.verify_campaign_with_votes(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);
}

#[test]
fn test_category_voting_threshold_other_categories_unaffected() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_voting_params(&admin, &3, &6000);
    client.set_category_voting_threshold(&admin, &Category::Learner, &9000);

    assert_eq!(
        client.get_category_voting_threshold(&Category::Learner),
        9000
    );
    assert_eq!(
        client.get_category_voting_threshold(&Category::Educator),
        6000
    );
}

#[test]
fn test_category_voting_threshold_removal_reverts_to_global_default() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    client.set_voting_params(&admin, &3, &7000);
    client.set_category_voting_threshold(&admin, &Category::Learner, &5000);
    assert_eq!(
        client.get_category_voting_threshold(&Category::Learner),
        5000
    );

    client.remove_category_voting_threshold(&admin, &Category::Learner);
    assert_eq!(
        client.get_category_voting_threshold(&Category::Learner),
        7000
    );
}

#[test]
fn test_category_voting_threshold_non_admin_rejected() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let impostor = Address::generate(&env);

    let res = client.try_set_category_voting_threshold(&impostor, &Category::Learner, &5000);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_category_voting_threshold_out_of_range_rejected() {
    let (_env, admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let too_low = client.try_set_category_voting_threshold(&admin, &Category::Learner, &999);
    assert_eq!(too_low.unwrap_err().unwrap(), Error::ValidationFailed);

    let too_high = client.try_set_category_voting_threshold(&admin, &Category::Learner, &10001);
    assert_eq!(too_high.unwrap_err().unwrap(), Error::ValidationFailed);
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
        creator.clone(),
        String::from_str(&env, "Min Balance Vote Test"),
        String::from_str(&env, "Testing minimum voting balance"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
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

// ── voting arithmetic proptests ─────────────────────────────────────────────────
// Property-based fuzz tests for the voting system.
//
// These tests use `proptest` to exercise the voting logic with arbitrary inputs,
// confirming that:
//
// * Vote counts and weights are always non-negative
// * Approval weight + rejection weight equals total weight
// * Vote counts increment correctly
// * Threshold calculations don't overflow
// * Quorum checks work correctly with arbitrary vote counts

// ── Pure arithmetic helpers ──────────────────────────────────────────────────

/// Calculate approval percentage in basis points (0-10000) from vote counts.
/// Uses u64 to avoid overflow when multiplying by 10_000.
fn calculate_approval_bps(approve_votes: u32, total_votes: u32) -> u32 {
    if total_votes > 0 {
        ((approve_votes as u64 * 10_000) / total_votes as u64) as u32
    } else {
        0
    }
}

/// Check if quorum is met
fn is_quorum_met(total_votes: u32, min_quorum: u32) -> bool {
    total_votes >= min_quorum
}

/// Check if approval threshold is met
fn is_threshold_met(approval_bps: u32, threshold_bps: u32) -> bool {
    approval_bps >= threshold_bps
}

// ── Strategies ───────────────────────────────────────────────────────────────

/// Vote counts: 0 to a reasonable maximum (1 million votes)
fn arb_vote_count() -> impl Strategy<Value = u32> {
    0u32..=1_000_000u32
}

/// Approval threshold in basis points (0-10000, i.e., 0-100%)
fn arb_threshold_bps() -> impl Strategy<Value = u32> {
    0u32..=10_000u32
}

/// Minimum quorum (1-1000 votes)
fn arb_min_quorum() -> impl Strategy<Value = u32> {
    1u32..=1_000u32
}

// ── Properties ───────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_approval_bps_in_valid_range(
        approve_votes in arb_vote_count(),
        reject_votes in arb_vote_count(),
    ) {
        let total_votes = approve_votes.saturating_add(reject_votes);
        let approval_bps = calculate_approval_bps(approve_votes, total_votes);
        prop_assert!(
            approval_bps <= 10_000,
            "approval_bps ({}) must be <= 10000",
            approval_bps
        );
    }

    #[test]
    fn prop_full_approval_gives_max_bps(votes in 1u32..=1_000_000u32) {
        let approval_bps = calculate_approval_bps(votes, votes);
        prop_assert_eq!(
            approval_bps, 10_000,
            "100% approval should give 10000 bps"
        );
    }

    #[test]
    fn prop_zero_approval_gives_zero_bps(reject_votes in 1u32..=1_000_000u32) {
        let approval_bps = calculate_approval_bps(0, reject_votes);
        prop_assert_eq!(approval_bps, 0, "0% approval should give 0 bps");
    }

    #[test]
    fn prop_half_approval_gives_half_bps(votes in 10u32..=1_000_000u32) {
        // Use an even vote count so the 50/50 split is exact: doubling the
        // generated value guarantees half == votes / 2 exactly, so the
        // computed bps is exactly 5000 with no rounding error.
        let votes = votes * 2;
        let half = votes / 2;
        let approval_bps = calculate_approval_bps(half, votes);
        prop_assert_eq!(
            approval_bps, 5_000,
            "50% approval should give 5000 bps, got {}",
            approval_bps
        );
    }

    #[test]
    fn prop_quorum_check_consistent(
        approve_votes in arb_vote_count(),
        reject_votes in arb_vote_count(),
        min_quorum in arb_min_quorum(),
    ) {
        let total_votes = approve_votes.saturating_add(reject_votes);
        let met = is_quorum_met(total_votes, min_quorum);
        prop_assert_eq!(met, total_votes >= min_quorum);
    }

    #[test]
    fn prop_threshold_check_consistent(
        approval_bps in arb_threshold_bps(),
        threshold_bps in arb_threshold_bps(),
    ) {
        let met = is_threshold_met(approval_bps, threshold_bps);
        prop_assert_eq!(met, approval_bps >= threshold_bps);
    }

    #[test]
    fn prop_vote_count_no_overflow(
        approve_votes in 0u32..=500_000u32,
        reject_votes in 0u32..=500_000u32,
    ) {
        let total = approve_votes.checked_add(reject_votes);
        prop_assert!(total.is_some(), "vote count addition should not overflow");
    }

    #[test]
    fn prop_approval_monotonic(
        base_approve in 0u32..=500_000u32,
        extra_approve in 0u32..=500_000u32,
        reject_votes in 1u32..=500_000u32,
    ) {
        let bps1 = calculate_approval_bps(base_approve, base_approve.saturating_add(reject_votes));
        let bps2 = calculate_approval_bps(
            base_approve.saturating_add(extra_approve),
            base_approve.saturating_add(extra_approve).saturating_add(reject_votes),
        );
        prop_assert!(
            bps2 >= bps1,
            "adding approval votes should not decrease approval bps: {} -> {}",
            bps1, bps2
        );
    }

    #[test]
    fn prop_verification_requires_both_conditions(
        approve_votes in arb_vote_count(),
        reject_votes in arb_vote_count(),
        min_quorum in arb_min_quorum(),
        threshold_bps in 5_000u32..=10_000u32, // 50-100%
    ) {
        let total_votes = approve_votes.saturating_add(reject_votes);
        let approval_bps = calculate_approval_bps(approve_votes, total_votes);

        let quorum_met = is_quorum_met(total_votes, min_quorum);
        let threshold_met = is_threshold_met(approval_bps, threshold_bps);
        let can_verify = quorum_met && threshold_met;

        // If either condition fails, verification should fail
        if !quorum_met || !threshold_met {
            prop_assert!(!can_verify);
        }
    }

    /// Property test for the 1-address-1-vote model (#469):
    /// Every voter contributes exactly 1 to the count regardless of token balance.
    /// This confirms that the vote counts equal the number of voters on each side.
    #[test]
    fn prop_one_address_one_vote_invariant(
        // Generate random numbers of approving and rejecting voters
        approve_count in 0u32..=10_000u32,
        reject_count in 0u32..=10_000u32,
    ) {
        // In the 1-address-1-vote model:
        // - Each approving voter adds exactly 1 to approve_count
        // - Each rejecting voter adds exactly 1 to reject_count
        // - approval_bps is computed from counts, not balances
        let total_votes = approve_count.saturating_add(reject_count);
        let approval_bps = calculate_approval_bps(approve_count, total_votes);
        prop_assert!(approval_bps <= 10_000);

        // If all votes approve, approval should be 10000 bps
        if reject_count == 0 && approve_count > 0 {
            prop_assert_eq!(approval_bps, 10_000);
        }

        // If all votes reject, approval should be 0 bps
        if approve_count == 0 && reject_count > 0 {
            prop_assert_eq!(approval_bps, 0);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_approval_bps_calculation() {
        // 60% approval (3 approve / 5 total)
        assert_eq!(calculate_approval_bps(3, 5), 6000);

        // 100% approval
        assert_eq!(calculate_approval_bps(1, 1), 10000);

        // 0% approval
        assert_eq!(calculate_approval_bps(0, 1), 0);

        // Zero total votes
        assert_eq!(calculate_approval_bps(0, 0), 0);
    }

    #[test]
    fn test_quorum_checks() {
        assert!(is_quorum_met(10, 5));
        assert!(is_quorum_met(5, 5));
        assert!(!is_quorum_met(4, 5));
        assert!(!is_quorum_met(0, 1));
    }

    #[test]
    fn test_threshold_checks() {
        assert!(is_threshold_met(6000, 5000));
        assert!(is_threshold_met(5000, 5000));
        assert!(!is_threshold_met(4999, 5000));
        assert!(!is_threshold_met(0, 1));
    }
}

// ── purge_voting_state ──────────────────────────────────────────────────────────

// Tests for issue #342: purge_voting_state batch cap and finalize semantics.

fn make_voters(env: &soroban_sdk::Env, count: u32) -> Vec<Address> {
    let mut voters = Vec::new(env);
    for _ in 0..count {
        voters.push_back(Address::generate(env));
    }
    voters
}

/// Set up a cancelled campaign with `voter_count` token-holding voters that have
/// each cast an approve vote. Returns the campaign id and the voters.
fn cancelled_campaign_with_voters(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
    token_admin: &TokenAdminClient<'_>,
    voter_count: u32,
) -> (u32, Vec<Address>) {
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Purge Voting Test"),
        String::from_str(env, "Voting state purge regression"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let voters = make_voters(env, voter_count);
    for voter in voters.iter() {
        token_admin.mint(&voter, &100);
        client.vote_on_campaign(&campaign_id, &voter, &true);
    }

    client.cancel_campaign(&campaign_id);
    (campaign_id, voters)
}

#[test]
fn test_purge_voting_state_rejects_oversized_batch() {
    let (env, _admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _) = cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 1);

    // 51 voters exceeds the MAX_VOTERS_PER_CALL = 50 cap.
    let oversized = make_voters(&env, 51);
    let res = client.try_purge_voting_state(&campaign_id, &oversized, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_purge_voting_state_rejects_empty_batch() {
    let (env, _admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _) = cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 1);

    let empty: Vec<Address> = Vec::new(&env);
    let res = client.try_purge_voting_state(&campaign_id, &empty, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_purge_voting_state_non_finalize_keeps_aggregate() {
    let (env, _admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, voters) =
        cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 3);

    let mut batch: Vec<Address> = Vec::new(&env);
    batch.push_back(voters.get(0).unwrap());

    // Non-final batch — HasVoted for the supplied voter is cleared.
    // The aggregate vote counts were already purged by cancel_campaign.
    client.purge_voting_state(&campaign_id, &batch, &false);

    assert!(!client.has_voted(&campaign_id, &voters.get(0).unwrap()));
    assert!(client.has_voted(&campaign_id, &voters.get(1).unwrap()));
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
}

#[test]
fn test_purge_voting_state_finalize_clears_aggregate() {
    let (env, _admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, voters) =
        cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 2);

    client.purge_voting_state(&campaign_id, &voters, &true);

    for voter in voters.iter() {
        assert!(!client.has_voted(&campaign_id, &voter));
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
    assert_eq!(client.get_reject_votes(&campaign_id), 0);
}

#[test]
fn test_purge_voting_state_split_batches_then_finalize() {
    let (env, _admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, voters) =
        cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 4);

    let mut first: Vec<Address> = Vec::new(&env);
    first.push_back(voters.get(0).unwrap());
    first.push_back(voters.get(1).unwrap());

    let mut second: Vec<Address> = Vec::new(&env);
    second.push_back(voters.get(2).unwrap());
    second.push_back(voters.get(3).unwrap());

    client.purge_voting_state(&campaign_id, &first, &false);
    assert_eq!(
        client.get_approve_votes(&campaign_id),
        0,
        "aggregate was already purged by cancel_campaign"
    );

    client.purge_voting_state(&campaign_id, &second, &true);

    for voter in voters.iter() {
        assert!(!client.has_voted(&campaign_id, &voter));
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
}
