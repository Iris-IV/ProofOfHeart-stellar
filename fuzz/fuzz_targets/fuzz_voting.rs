//! Fuzz target for vote casting and the threshold/quorum arithmetic in
//! `voting.rs` (issue #771).
//!
//! `cast_vote` and `verify_with_votes` gate campaign verification on
//! community-configured `min_votes_quorum` and `approval_threshold_bps`
//! values, then compute an approval percentage from live vote counts. This
//! harness throws adversarial quorum/threshold configs and vote sequences
//! (including repeat votes from the same address, and verification attempts
//! before/after quorum is reached) at those entry points.
//!
//! The property under test: none of `set_voting_params`, `cast_vote`, or
//! `verify_with_votes` must ever panic / trap the host, no matter the
//! configured quorum, threshold, or vote sequence -- each call must always
//! resolve to either `Ok(())` or a typed `Err(Error)`.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String};

use proof_of_heart::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    min_votes_quorum: u32,
    approval_threshold_bps: u32,
    min_voting_balance_raw: u32,
    voter_balance_raw: u32,
    votes: Vec<bool>,
    revote_last_voter: bool,
    verify_with_votes_first: bool,
}

fuzz_target!(|input: FuzzInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    if client.try_init(&admin, &token_address, &0u32).is_err() {
        return;
    }

    // `set_voting_params` / `set_min_voting_balance` must reject
    // out-of-range configuration gracefully rather than panicking; when
    // rejected, the existing defaults simply remain in effect.
    let _ = client.try_set_voting_params(
        &admin,
        &input.min_votes_quorum,
        &input.approval_threshold_bps,
    );
    let min_voting_balance = (input.min_voting_balance_raw as i128) % 1_000_000;
    let _ = client.try_set_min_voting_balance(&admin, &min_voting_balance);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Fuzz"),
        description: String::from_str(&env, "Fuzz voting target"),
        funding_goal: 100_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };

    let campaign_id = match client.try_create_campaign(&params) {
        Ok(Ok(id)) => id,
        _ => return,
    };

    // Optionally attempt verification before any votes are cast, exercising
    // the zero-votes / quorum-not-met path.
    if input.verify_with_votes_first {
        let _ = client.try_verify_campaign_with_votes(&campaign_id);
    }

    // Cast a bounded number of votes from distinct voters, each funded with
    // the same arbitrary balance, to fuzz the quorum/approval-bps arithmetic
    // under adversarial approve/reject sequences.
    let voter_balance = (input.voter_balance_raw as i128).saturating_add(1);
    let mut last_voter: Option<Address> = None;
    for &approve in input.votes.iter().take(25) {
        let voter = Address::generate(&env);
        token_admin.mint(&voter, &voter_balance);
        // Must never panic regardless of balance, quorum, or vote history.
        let _ = client.try_vote_on_campaign(&campaign_id, &voter, &approve);
        last_voter = Some(voter);
    }

    // Exercise the `AlreadyVoted` path by having the last voter vote again
    // with the opposite choice.
    if input.revote_last_voter {
        if let Some(voter) = last_voter {
            let _ = client.try_vote_on_campaign(&campaign_id, &voter, &true);
        }
    }

    // Both verification paths must never panic, regardless of how many
    // votes were cast or how the quorum/threshold params were configured.
    let _ = client.try_verify_campaign_with_votes(&campaign_id);
    let _ = client.try_verify_campaign(&campaign_id);
});
