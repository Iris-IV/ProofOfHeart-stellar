// Regression coverage for #1478: an admin (or creator) must not be able to
// cancel a milestone campaign once part of its escrow has been paid out, and
// contributors must not be able to refund their full contribution afterwards.

use super::helpers::*;
use crate::{Category, Error, Milestone};
use soroban_sdk::{String, Vec};

fn funded_milestone_campaign(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    creator: &soroban_sdk::Address,
    contributor: &soroban_sdk::Address,
) -> u32 {
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Milestone Escrow"),
        String::from_str(env, "Two-phase milestone payout"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, contributor, &1_000);

    let mut milestones: Vec<Milestone> = Vec::new(env);
    milestones.push_back(Milestone {
        id: 1,
        description: String::from_str(env, "Phase 1"),
        payout_bps: 5_000,
        verified: false,
    });
    milestones.push_back(Milestone {
        id: 2,
        description: String::from_str(env, "Phase 2"),
        payout_bps: 5_000,
        verified: false,
    });
    client.set_milestones(&campaign_id, &milestones);
    campaign_id
}

#[test]
fn test_admin_cancel_rejected_after_partial_milestone_payout() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &1_000);
    let campaign_id = funded_milestone_campaign(&env, &client, &creator, &contributor);

    client.verify_milestone(&admin, &campaign_id, &1);
    client.claim_milestone(&campaign_id, &1);

    // Only one of two milestones claimed: funds_withdrawn is still false.
    assert!(!client.get_campaign(&campaign_id).funds_withdrawn());
    let paid_out = token.balance(&creator) + token.balance(&admin);
    assert!(paid_out > 0);

    let res =
        client.try_admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));
    assert_eq!(res, Err(Ok(Error::CancellationNotAllowed)));

    let campaign = client.get_campaign(&campaign_id);
    assert!(!campaign.is_cancelled());

    // No refund path opens up either.
    let refund = client.try_claim_refund(&campaign_id, &contributor);
    assert_eq!(refund, Err(Ok(Error::RefundNotAvailable)));
    assert_eq!(token.balance(&contributor), 0);
}

#[test]
fn test_admin_cancel_allowed_before_any_milestone_claimed() {
    let (env, admin, creator, contributor, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &1_000);
    let campaign_id = funded_milestone_campaign(&env, &client, &creator, &contributor);

    // Verified but unclaimed milestones leave the escrow intact.
    client.verify_milestone(&admin, &campaign_id, &1);

    client.admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));
    assert!(client.get_campaign(&campaign_id).is_cancelled());

    client.claim_refund(&campaign_id, &contributor);
    assert_eq!(token.balance(&contributor), 1_000);
}
