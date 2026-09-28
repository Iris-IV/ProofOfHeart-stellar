/// Tests for issue #863: `cancel_campaign` does not clear `top_contributor`.
///
/// A cancelled campaign is terminal and every contribution to it is
/// refundable, so there is no winner to report. Two layers keep that promise:
///
/// 1. `cancel_campaign` / `admin_cancel_campaign` remove the stored
///    top-contributor marker as part of the cancellation write, so a never-
///    claimed refund cannot leave the address pinned in ledger storage.
/// 2. `get_campaign_stats` ignores the marker for a cancelled campaign, which
///    also covers campaigns cancelled before the write path existed.
use super::helpers::*;
use crate::storage::{get_top_contributor, set_top_contributor};
use crate::types::MaybePendingCreator;
use crate::{Category, CreateCampaignParams};
use soroban_sdk::{Address, Env, String};

fn make_campaign(
    env: &Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    title: &str,
    goal: i128,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "Test description for campaign"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    })
}

/// Reads the raw stored top-contributor marker, bypassing `get_campaign_stats`.
fn stored_top_contributor(
    env: &Env,
    client: &ProofOfHeartClient,
    campaign_id: u32,
) -> Option<Address> {
    env.as_contract(&client.address, || get_top_contributor(env, campaign_id))
}

/// Writes the marker directly, reproducing the ledger state an older contract
/// version left behind on a cancelled campaign.
fn force_stored_top_contributor(
    env: &Env,
    client: &ProofOfHeartClient,
    campaign_id: u32,
    contributor: &Address,
) {
    env.as_contract(&client.address, || {
        set_top_contributor(env, campaign_id, contributor)
    });
}

#[test]
fn test_cancel_campaign_clears_top_contributor() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let campaign_id = make_campaign(&env, &client, &creator, "Cancel Clears Winner", 1_000);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &400);
    client.contribute(&campaign_id, &contributor2, &100);

    assert_eq!(
        client.get_campaign_stats(&campaign_id).top_contributor,
        MaybePendingCreator::Some(contributor1.clone())
    );
    assert_eq!(
        stored_top_contributor(&env, &client, campaign_id),
        Some(contributor1.clone())
    );

    client.cancel_campaign(&campaign_id);

    // The marker is gone from storage, not merely hidden from the query.
    assert_eq!(
        stored_top_contributor(&env, &client, campaign_id),
        None,
        "cancellation must clear the stored top-contributor marker"
    );
    assert_eq!(
        client.get_campaign_stats(&campaign_id).top_contributor,
        MaybePendingCreator::None
    );
    assert!(client
        .get_campaign_stats(&campaign_id)
        .top_contributor
        .is_none());

    // Claiming a refund must not resurrect a winner for a terminal campaign.
    client.claim_refund(&campaign_id, &contributor1);
    assert!(client
        .get_campaign_stats(&campaign_id)
        .top_contributor
        .is_none());
    assert_eq!(stored_top_contributor(&env, &client, campaign_id), None);
}

#[test]
fn test_admin_cancel_campaign_clears_top_contributor() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let campaign_id = make_campaign(&env, &client, &creator, "Admin Cancel Clears Winner", 1_000);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &100);
    client.contribute(&campaign_id, &contributor2, &700);
    assert_eq!(
        client.get_campaign_stats(&campaign_id).top_contributor,
        MaybePendingCreator::Some(contributor2.clone())
    );

    client.admin_cancel_campaign(
        &admin,
        &campaign_id,
        &String::from_str(&env, "fraud detected"),
    );

    assert_eq!(stored_top_contributor(&env, &client, campaign_id), None);
    assert!(client
        .get_campaign_stats(&campaign_id)
        .top_contributor
        .is_none());
}

#[test]
fn test_get_campaign_stats_ignores_marker_left_by_older_version() {
    let (env, _admin, creator, contributor, _contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor, &2_000);

    let campaign_id = make_campaign(&env, &client, &creator, "Legacy Stale Marker", 1_000);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &250);

    client.cancel_campaign(&campaign_id);

    // Simulate a campaign cancelled before the write path existed: the marker
    // is still in storage but the campaign is terminal.
    force_stored_top_contributor(&env, &client, campaign_id, &contributor);
    assert_eq!(
        stored_top_contributor(&env, &client, campaign_id),
        Some(contributor.clone())
    );

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(stats.top_contributor, MaybePendingCreator::None);
    assert!(stats.top_contributor.is_none());
}

#[test]
fn test_cancel_campaign_without_contributions_reports_no_winner() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let campaign_id = make_campaign(&env, &client, &creator, "Cancel No Contributions", 1_000);
    client.cancel_campaign(&campaign_id);

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(stats.contributor_count, 0);
    assert!(stats.top_contributor.is_none());
    assert_eq!(stats.avg_contribution, 0);
    assert_eq!(stored_top_contributor(&env, &client, campaign_id), None);
}
