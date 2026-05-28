// Tests for issue #342: purge_voting_state batch cap and finalize semantics.
use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, String, Vec};

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

    // Non-final batch — HasVoted for the supplied voter is cleared but the
    // aggregate vote counts remain so the cleanup can continue across calls.
    client.purge_voting_state(&campaign_id, &batch, &false);

    assert!(!client.has_voted(&campaign_id, &voters.get(0).unwrap()));
    assert!(client.has_voted(&campaign_id, &voters.get(1).unwrap()));
    assert_eq!(client.get_approve_votes(&campaign_id), 3);
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
        4,
        "aggregate must survive the non-final batch"
    );

    client.purge_voting_state(&campaign_id, &second, &true);

    for voter in voters.iter() {
        assert!(!client.has_voted(&campaign_id, &voter));
    }
    assert_eq!(client.get_approve_votes(&campaign_id), 0);
}
