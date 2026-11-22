//! Campaign payout coverage for `milestones.rs` (#1303).
//!
//! Full payout with the default fee, payout with a per-campaign fee
//! override, and the two rejection paths (unverified campaign, zero-balance
//! escrow) that guard `withdraw_funds`.

use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error, Milestone};
use soroban_sdk::{testutils::Address as _, Address, String, Vec};

#[test]
fn test_campaign_payout_full_amount() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Payout Campaign");
    let desc = String::from_str(&env, "Full payout test");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);
}

#[test]
fn test_campaign_payout_with_fee_override() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Override Fee Campaign");
    let desc = String::from_str(&env, "Fee override payout");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    client.set_campaign_fee_override(&campaign_id, &admin, &1000);

    client.contribute(&campaign_id, &contributor1, &1000);

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 100);
    assert_eq!(token.balance(&creator), 900);
}

#[test]
fn test_campaign_payout_rejects_unverified() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Unverified Payout");
    let desc = String::from_str(&env, "Cannot withdraw unverified");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotVerified);
}

#[test]
fn test_campaign_payout_rejects_zero_balance() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Empty Payout");
    let desc = String::from_str(&env, "No funds to withdraw");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    // Withdrawal requires the deadline to have passed (#854).
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_admin_cancel_partial_milestone_payout_prorates_refunds() {
    let (env, admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();
    let contributor3 = Address::generate(&env);

    token_admin.mint(&contributor1, &600);
    token_admin.mint(&contributor2, &400);
    token_admin.mint(&contributor3, &1_000);

    // Campaign A: milestone-based campaign (goal 1_000)
    let campaign_a = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Milestone Campaign A"),
        description: String::from_str(&env, "Partial payout before admin cancel"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_a);

    // Campaign B: independent campaign holding 1_000 in shared contract escrow
    let campaign_b = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Escrow Campaign B"),
        description: String::from_str(&env, "Unrelated campaign whose escrow must stay intact"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_b);

    client.contribute(&campaign_a, &contributor1, &600);
    client.contribute(&campaign_a, &contributor2, &400);
    client.contribute(&campaign_b, &contributor3, &1_000);

    assert_eq!(client.get_total_raised_global(), 2_000);
    assert_eq!(token.balance(&client.address), 2_000);

    let mut milestones = Vec::new(&env);
    milestones.push_back(Milestone {
        id: 1,
        description: String::from_str(&env, "Phase 1"),
        payout_bps: 5_000,
        verified: false,
    });
    milestones.push_back(Milestone {
        id: 2,
        description: String::from_str(&env, "Phase 2"),
        payout_bps: 5_000,
        verified: false,
    });
    client.set_milestones(&campaign_a, &milestones);

    // Verify and claim Milestone 1 (50% = 485 to creator + 15 fee to admin = 500 paid out)
    client.verify_milestone(&admin, &campaign_a, &1);
    client.claim_milestone(&campaign_a, &1);

    assert_eq!(token.balance(&creator), 485);
    assert_eq!(token.balance(&admin), 15);
    assert_eq!(token.balance(&client.address), 1_500);
    assert_eq!(client.get_total_raised_global(), 1_500);

    // Admin cancels Campaign A after Milestone 1 was already paid out
    client.admin_cancel_campaign(
        &admin,
        &campaign_a,
        &String::from_str(&env, "fraud after M1"),
    );

    // Global total raised must reflect only Campaign B's 1_000 (not double-subtract Milestone 1)
    assert_eq!(client.get_total_raised_global(), 1_000);

    // Contributors of Campaign A receive pro-rata refunds of the 500 unpaid escrow remainder
    client.claim_refund(&campaign_a, &contributor1);
    client.claim_refund(&campaign_a, &contributor2);

    assert_eq!(token.balance(&contributor1), 300);
    assert_eq!(token.balance(&contributor2), 200);

    // Campaign B's 1_000 escrow in the contract is 100% untouched
    assert_eq!(token.balance(&client.address), 1_000);

    withdraw_after_deadline(&env, &client, campaign_b);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_admin_cancel_odd_milestone_payout_absorbs_rounding_dust() {
    let (env, admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();
    let contributor3 = Address::generate(&env);

    token_admin.mint(&contributor1, &600);
    token_admin.mint(&contributor2, &400);
    token_admin.mint(&contributor3, &1_000);

    let campaign_a = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Odd Split Milestone A"),
        description: String::from_str(&env, "3333/3333/3334 split"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_a);

    let campaign_b = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Safe Campaign B"),
        description: String::from_str(&env, "Unrelated escrow"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_b);

    client.contribute(&campaign_a, &contributor1, &600);
    client.contribute(&campaign_a, &contributor2, &400);
    client.contribute(&campaign_b, &contributor3, &1_000);

    let mut milestones = Vec::new(&env);
    for (id, bps) in [(1u32, 3_333u32), (2, 3_333), (3, 3_334)] {
        milestones.push_back(Milestone {
            id,
            description: String::from_str(&env, "M"),
            payout_bps: bps,
            verified: false,
        });
    }
    client.set_milestones(&campaign_a, &milestones);

    // Milestone 1 pays 323 to creator + 9 fee = 332 total paid out -> 668 unpaid remainder
    client.verify_milestone(&admin, &campaign_a, &1);
    client.claim_milestone(&campaign_a, &1);
    assert_eq!(token.balance(&client.address), 1_668);

    client.admin_cancel_campaign(
        &admin,
        &campaign_a,
        &String::from_str(&env, "cancelled after M1"),
    );

    // contributor1 (600/1000 of 668) = 400; contributor2 (last contributor) absorbs 668 - 400 = 268
    client.claim_refund(&campaign_a, &contributor1);
    client.claim_refund(&campaign_a, &contributor2);

    assert_eq!(token.balance(&contributor1), 400);
    assert_eq!(token.balance(&contributor2), 268);
    assert_eq!(token.balance(&client.address), 1_000);
}
