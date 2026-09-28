//! Campaign payout coverage for `milestones.rs` (#1303).
//!
//! Full payout with the default fee, payout with a per-campaign fee
//! override, and the two rejection paths (unverified campaign, zero-balance
//! escrow) that guard `withdraw_funds`.

use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::String;

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
