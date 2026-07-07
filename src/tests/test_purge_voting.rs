// Tests for issue #342: purge_voting_state batch cap and finalize semantics.
use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{Address, String, Vec};

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
    // Use 2 voters — below the default quorum of 3 — so cancel_campaign
    // still clears the aggregate (no quorum-met preservation applies).
    let (campaign_id, voters) =
        cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 2);

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
    // Use 2 voters — below the default quorum of 3 — so the finalize batch
    // is not blocked by the quorum-preservation guard.
    let (campaign_id, voters) =
        cancelled_campaign_with_voters(&env, &client, &creator, &token_admin, 2);

    let mut first: Vec<Address> = Vec::new(&env);
    first.push_back(voters.get(0).unwrap());

    let mut second: Vec<Address> = Vec::new(&env);
    second.push_back(voters.get(1).unwrap());

    client.purge_voting_state(&campaign_id, &first, &false);
    assert_eq!(
        client.get_approve_votes(&campaign_id),
        0,
        "aggregate was already purged by cancel_campaign (quorum not met)"
    );

    client.purge_voting_state(&campaign_id, &second, &true);

    for voter in voters.iter() {
        assert!(!client.has_voted(&campaign_id, &voter));
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
}

/// Build a cancelled campaign where the vote state has already met both the
/// quorum requirement and the approval threshold (i.e. `verify_with_votes`
/// would succeed).  The campaign is cancelled so `purge_voting_state` is
/// otherwise allowed — the quorum guard is the only thing that should block it.
fn cancelled_campaign_with_quorum(
    env: &soroban_sdk::Env,
    admin: &Address,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
    token_admin: &TokenAdminClient<'_>,
) -> (u32, Vec<Address>) {
    // Lower quorum to 3 so the test doesn't need many voters.
    client.set_voting_params(admin, &3, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Quorum-ready Campaign"),
        String::from_str(env, "All three voters approve"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Three voters each with 100 tokens all approve → 100 % ≥ 60 % threshold.
    let voters = make_voters(env, 3);
    for voter in voters.iter() {
        token_admin.mint(&voter, &100);
        client.vote_on_campaign(&campaign_id, &voter, &true);
    }

    // Cancel so the terminal-state guard is satisfied.
    client.cancel_campaign(&campaign_id);

    (campaign_id, voters)
}

#[test]
fn test_purge_voting_state_blocked_when_quorum_met() {
    // Requirement: admin must not be able to silently erase a community vote
    // that has already reached a verifiable outcome (quorum + threshold met).
    let (env, admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();

    let (campaign_id, voters) =
        cancelled_campaign_with_quorum(&env, &admin, &client, &creator, &token_admin);

    let res = client.try_purge_voting_state(&campaign_id, &voters, &true);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::ValidationFailed,
        "purge with finalize_aggregate=true must be blocked when quorum and threshold are met"
    );

    // The HasVoted flags and aggregate must be untouched.
    for voter in voters.iter() {
        assert!(
            client.has_voted(&campaign_id, &voter),
            "HasVoted must not be cleared when purge is blocked"
        );
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 3);
}

#[test]
fn test_purge_voting_state_non_finalize_allowed_even_when_quorum_met() {
    // Non-final batches only remove HasVoted flags; they do not erase the
    // aggregate, so they are safe even when quorum has been reached.
    let (env, admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();

    let (campaign_id, voters) =
        cancelled_campaign_with_quorum(&env, &admin, &client, &creator, &token_admin);

    let mut batch: Vec<Address> = Vec::new(&env);
    batch.push_back(voters.get(0).unwrap());

    // finalize_aggregate = false must succeed regardless of quorum state.
    client.purge_voting_state(&campaign_id, &batch, &false);
    assert!(!client.has_voted(&campaign_id, &voters.get(0).unwrap()));

    // Aggregate is still intact.
    assert_eq!(client.get_approve_votes(&campaign_id), 3);
}

#[test]
fn test_purge_voting_state_allowed_when_quorum_not_met() {
    // If the campaign was cancelled before quorum was reached, the admin
    // should still be able to clean up the partial vote state.
    let (env, admin, creator, _c1, _c2, _token, token_admin, client) = setup_env();

    // Set quorum to 5; only 2 voters → quorum not met.
    client.set_voting_params(&admin, &5, &6000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Sub-quorum Campaign"),
        String::from_str(&env, "Only two votes"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let voters = make_voters(&env, 2);
    for voter in voters.iter() {
        token_admin.mint(&voter, &100);
        client.vote_on_campaign(&campaign_id, &voter, &true);
    }
    client.cancel_campaign(&campaign_id);

    // Should succeed because quorum is not met.
    client.purge_voting_state(&campaign_id, &voters, &true);

    for voter in voters.iter() {
        assert!(!client.has_voted(&campaign_id, &voter));
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
}
