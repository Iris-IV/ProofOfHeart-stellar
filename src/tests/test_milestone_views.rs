//! Unit tests for milestone and payout view entrypoints (#1459):
//! - `get_milestones(campaign_id)`
//! - `is_milestone_claimed(campaign_id, milestone_id)`
//! - `get_campaign_payout_marker(campaign_id)`

use super::helpers::*;
use crate::{Category, Milestone};
use soroban_sdk::{testutils::Ledger as _, Address, Env, String, Vec};

fn create_test_campaign(
    env: &Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    title: &str,
    goal: i128,
) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "Milestone view entrypoint test campaign"),
        goal,
        30,
        Category::Educator,
        false,
        0,
        0,
    ))
}

#[test]
fn test_milestone_views_empty_campaign() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let campaign_id = create_test_campaign(&env, &client, &creator, "Empty Milestones", 1_000);

    // Newly created campaign has no milestones, no claimed milestones, and no payout marker.
    assert!(client.get_milestones(&campaign_id).is_empty());
    assert!(!client.is_milestone_claimed(&campaign_id, &1));
    assert!(!client.is_milestone_claimed(&campaign_id, &2));
    assert_eq!(client.get_campaign_payout_marker(&campaign_id), None);

    // Non-existent campaign IDs also return empty / false / None safely.
    assert!(client.get_milestones(&999).is_empty());
    assert!(!client.is_milestone_claimed(&999, &1));
    assert_eq!(client.get_campaign_payout_marker(&999), None);
}

#[test]
fn test_milestone_views_after_set_milestones() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let campaign_id = create_test_campaign(&env, &client, &creator, "Planned Milestones", 1_000);

    let mut schedule = Vec::new(&env);
    schedule.push_back(Milestone {
        id: 1,
        description: String::from_str(&env, "Curriculum design"),
        payout_bps: 2_500,
        verified: true, // set_milestones normalizes verified to false
    });
    schedule.push_back(Milestone {
        id: 2,
        description: String::from_str(&env, "Pilot workshop"),
        payout_bps: 2_500,
        verified: false,
    });
    schedule.push_back(Milestone {
        id: 3,
        description: String::from_str(&env, "Final delivery"),
        payout_bps: 5_000,
        verified: false,
    });

    client.set_milestones(&campaign_id, &schedule);

    let stored = client.get_milestones(&campaign_id);
    assert_eq!(stored.len(), 3);

    let m1 = stored.get(0).unwrap();
    assert_eq!(m1.id, 1);
    assert_eq!(m1.description, String::from_str(&env, "Curriculum design"));
    assert_eq!(m1.payout_bps, 2_500);
    assert!(!m1.verified);

    let m2 = stored.get(1).unwrap();
    assert_eq!(m2.id, 2);
    assert_eq!(m2.description, String::from_str(&env, "Pilot workshop"));
    assert_eq!(m2.payout_bps, 2_500);
    assert!(!m2.verified);

    let m3 = stored.get(2).unwrap();
    assert_eq!(m3.id, 3);
    assert_eq!(m3.description, String::from_str(&env, "Final delivery"));
    assert_eq!(m3.payout_bps, 5_000);
    assert!(!m3.verified);

    for id in 1..=3u32 {
        assert!(!client.is_milestone_claimed(&campaign_id, &id));
    }
    assert_eq!(client.get_campaign_payout_marker(&campaign_id), None);
}

#[test]
fn test_milestone_views_after_verify_and_batch_verify() {
    let (env, admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let campaign_id = create_test_campaign(&env, &client, &creator, "Verify Milestones", 1_000);

    let mut schedule = Vec::new(&env);
    for (id, bps) in [(1u32, 3_000u32), (2, 3_000), (3, 4_000)] {
        schedule.push_back(Milestone {
            id,
            description: String::from_str(&env, "Tranche"),
            payout_bps: bps,
            verified: false,
        });
    }
    client.set_milestones(&campaign_id, &schedule);

    // Verify only milestone 1
    client.verify_milestone(&admin, &campaign_id, &1);

    let after_single = client.get_milestones(&campaign_id);
    assert!(after_single.get(0).unwrap().verified);
    assert!(!after_single.get(1).unwrap().verified);
    assert!(!after_single.get(2).unwrap().verified);

    // Verification alone must not mark the milestone as claimed
    assert!(!client.is_milestone_claimed(&campaign_id, &1));
    assert!(!client.is_milestone_claimed(&campaign_id, &2));
    assert!(!client.is_milestone_claimed(&campaign_id, &3));

    // Batch-verify remaining milestones (2 and 3)
    let newly_verified = client.batch_verify_milestones(&admin, &campaign_id);
    assert_eq!(newly_verified, 2);

    let after_batch = client.get_milestones(&campaign_id);
    assert!(after_batch.get(0).unwrap().verified);
    assert!(after_batch.get(1).unwrap().verified);
    assert!(after_batch.get(2).unwrap().verified);

    for id in 1..=3u32 {
        assert!(!client.is_milestone_claimed(&campaign_id, &id));
    }
}

#[test]
fn test_milestone_views_after_partial_and_full_claim() {
    let (env, admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    let campaign_id = create_test_campaign(&env, &client, &creator, "Claim Milestones", 1_000);
    client.verify_campaign(&campaign_id);

    token_admin.mint(&contributor1, &1_000);
    client.contribute(&campaign_id, &contributor1, &1_000);

    let mut schedule = Vec::new(&env);
    schedule.push_back(Milestone {
        id: 10,
        description: String::from_str(&env, "First tranche"),
        payout_bps: 4_000,
        verified: false,
    });
    schedule.push_back(Milestone {
        id: 20,
        description: String::from_str(&env, "Second tranche"),
        payout_bps: 6_000,
        verified: false,
    });
    client.set_milestones(&campaign_id, &schedule);

    // Verify and claim milestone 10 (partial payout)
    client.verify_milestone(&admin, &campaign_id, &10);
    client.claim_milestone(&campaign_id, &10);

    assert!(client.is_milestone_claimed(&campaign_id, &10));
    assert!(!client.is_milestone_claimed(&campaign_id, &20));
    assert!(!client.is_milestone_claimed(&campaign_id, &99));

    let after_partial = client.get_milestones(&campaign_id);
    assert_eq!(after_partial.len(), 2);
    assert!(after_partial.get(0).unwrap().verified);
    assert!(!after_partial.get(1).unwrap().verified);
    assert!(!client.get_campaign(&campaign_id).funds_withdrawn());

    // Verify and claim milestone 20 (full payout)
    client.verify_milestone(&admin, &campaign_id, &20);
    client.claim_milestone(&campaign_id, &20);

    assert!(client.is_milestone_claimed(&campaign_id, &10));
    assert!(client.is_milestone_claimed(&campaign_id, &20));

    let after_full = client.get_milestones(&campaign_id);
    assert!(after_full.get(0).unwrap().verified);
    assert!(after_full.get(1).unwrap().verified);
    assert!(client.get_campaign(&campaign_id).funds_withdrawn());
}

#[test]
fn test_get_campaign_payout_marker_before_and_after_withdrawal() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1_000);
    token_admin.mint(&contributor2, &1_000);

    let campaign_1 = create_test_campaign(&env, &client, &creator, "Payout Marker 1", 1_000);
    let campaign_2 = create_test_campaign(&env, &client, &creator, "Payout Marker 2", 1_000);

    // Before verification: None
    assert_eq!(client.get_campaign_payout_marker(&campaign_1), None);
    assert_eq!(client.get_campaign_payout_marker(&campaign_2), None);

    client.verify_campaign(&campaign_1);
    client.verify_campaign(&campaign_2);

    // After verification and funding (before withdrawal): still None
    client.contribute(&campaign_1, &contributor1, &1_000);
    client.contribute(&campaign_2, &contributor2, &1_000);
    assert_eq!(client.get_campaign_payout_marker(&campaign_1), None);
    assert_eq!(client.get_campaign_payout_marker(&campaign_2), None);

    // Advance past deadline at ledger sequence 77 and withdraw campaign_1
    let deadline_1 = client.get_campaign(&campaign_1).deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline_1 + 1;
        li.sequence_number = 77;
    });
    client.withdraw_funds(&campaign_1);

    assert_eq!(client.get_campaign_payout_marker(&campaign_1), Some(77));
    assert_eq!(client.get_campaign_payout_marker(&campaign_2), None);

    // Advance to ledger sequence 88 and withdraw campaign_2
    let deadline_2 = client.get_campaign(&campaign_2).deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline_2 + 10;
        li.sequence_number = 88;
    });
    client.withdraw_funds(&campaign_2);

    assert_eq!(client.get_campaign_payout_marker(&campaign_1), Some(77));
    assert_eq!(client.get_campaign_payout_marker(&campaign_2), Some(88));
}
