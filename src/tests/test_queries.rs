extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::{Campaign, Category, MaybePendingCreator};
use soroban_sdk::{Address, String};

#[test]
fn test_list_campaigns_exclusive_cursor_semantics() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();

    let titles = ["Campaign 1", "Campaign 2", "Campaign 3"];
    for i in 0..3 {
        let title_data = [b'C', b'_', b'1' + i as u8];
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_bytes(&env, &title_data),
            String::from_str(&env, "Desc"),
            1000 + i as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
        assert_eq!(id, (i + 1) as u32);
    }

    let page1 = client.list_campaigns(&0, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap().id, 1);
    assert_eq!(page1.get(1).unwrap().id, 2);

    let page2 = client.list_campaigns(&2, &2);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2.get(0).unwrap().id, 3);
}

#[test]
fn test_list_active_campaigns_exclusive_cursor_semantics() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    for i in 0..4 {
        let title_data = [b'C', b'_', b'1' + i as u8];
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            String::from_bytes(&env, &title_data),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    client.cancel_campaign(&2);

    let active1 = client.list_active_campaigns(&0, &2);
    assert_eq!(active1.0.len(), 2);
    assert_eq!(active1.0.get(0).unwrap().id, 1);
    assert_eq!(active1.0.get(1).unwrap().id, 3);

    let active2 = client.list_active_campaigns(&3, &2);
    assert_eq!(active2.0.len(), 1);
    assert_eq!(active2.0.get(0).unwrap().id, 4);
}

#[test]
fn test_get_campaigns_by_category_with_pagination() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Learner 1"),
        String::from_str(&env, "a"),
        100,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Publisher 1"),
        String::from_str(&env, "b"),
        100,
        30,
        Category::Publisher,
        false,
        0,
        0i128,
    ));
    let id3 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Learner 2"),
        String::from_str(&env, "c"),
        100,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let learner_page_1 = client.get_campaigns_by_category(&Category::Learner, &0, &1);
    assert_eq!(learner_page_1.0.len(), 1);
    assert_eq!(learner_page_1.0.get(0).unwrap().id, id1);

    let learner_page_2 = client.get_campaigns_by_category(&Category::Learner, &1, &1);
    assert_eq!(learner_page_2.0.len(), 1);
    assert_eq!(learner_page_2.0.get(0).unwrap().id, id3);

    let publisher = client.get_campaigns_by_category(&Category::Publisher, &0, &10);
    assert_eq!(publisher.0.len(), 1);
    assert_eq!(publisher.0.get(0).unwrap().category, Category::Publisher);
}

#[test]
fn test_get_platform_stats_returns_aggregates() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let c1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Stats 1"),
        String::from_str(&env, "s1"),
        500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let c2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Stats 2"),
        String::from_str(&env, "s2"),
        500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let _ = client.try_verify_campaign(&c1);
    let _ = client.try_verify_campaign(&c2);
    client.contribute(&c1, &contributor1, &400);
    client.contribute(&c2, &contributor2, &300);
    client.cancel_campaign(&c2);

    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 2);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.verified_campaigns, 2);
    assert_eq!(stats.cancelled_campaigns, 1);
    // After cancel, total_raised_global is decremented (#818), so only the
    // remaining active campaign's contribution (400) counts.
    assert_eq!(stats.total_amount_raised, 400);
}

#[test]
fn test_get_campaign_stats_empty_before_any_contribution() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Stats Empty"),
        String::from_str(&env, "No contributions yet"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(stats.contributor_count, 0);
    assert!(stats.top_contributor.is_none());
    assert_eq!(stats.avg_contribution, 0);
    assert_eq!(stats.last_contribution_time, 0);
}

#[test]
fn test_get_campaign_stats_after_contributions() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Stats After"),
        String::from_str(&env, "Contribute then query"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let first_contribution_time = env.ledger().timestamp();
    client.contribute(&campaign_id, &contributor1, &400);

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(stats.contributor_count, 1);
    assert_eq!(
        stats.top_contributor,
        MaybePendingCreator::Some(contributor1.clone())
    );
    assert_eq!(stats.avg_contribution, 400);
    assert_eq!(stats.last_contribution_time, first_contribution_time);

    // contributor2 contributes more and should become the new top contributor
    env.ledger().with_mut(|li| {
        li.timestamp += 1;
    });
    let second_contribution_time = env.ledger().timestamp();
    client.contribute(&campaign_id, &contributor2, &900);

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(stats.contributor_count, 2);
    assert_eq!(
        stats.top_contributor,
        MaybePendingCreator::Some(contributor2.clone())
    );
    // avg_contribution = amount_raised / contributor_count = 1300 / 2 = 650
    assert_eq!(stats.avg_contribution, 650);
    assert_eq!(stats.last_contribution_time, second_contribution_time);
}

#[test]
fn test_get_campaign_stats_top_contributor_does_not_regress_on_smaller_contribution() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Stats Top Sticky"),
        String::from_str(&env, "Top contributor should not flip on a smaller add"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &900);
    client.contribute(&campaign_id, &contributor2, &100);

    let stats = client.get_campaign_stats(&campaign_id);
    assert_eq!(
        stats.top_contributor,
        MaybePendingCreator::Some(contributor1.clone())
    );
}

#[test]
fn test_get_creator_stats_returns_aggregates() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &2_000);
    token_admin.mint(&contributor2, &2_000);

    let c1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Creator Stats 1"),
        String::from_str(&env, "s1"),
        500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let c2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Creator Stats 2"),
        String::from_str(&env, "s2"),
        500,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let _ = client.try_verify_campaign(&c1);
    let _ = client.try_verify_campaign(&c2);
    client.contribute(&c1, &contributor1, &400);
    client.contribute(&c2, &contributor1, &100);
    client.contribute(&c2, &contributor2, &200);
    client.cancel_campaign(&c2);

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.total_campaigns, 2);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.total_raised, 400);
    assert_eq!(stats.total_contributors, 3);
}

#[test]
fn test_get_creator_stats_empty_for_unknown_creator() {
    let (_env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let stranger = Address::generate(&_env);

    let stats = client.get_creator_stats(&stranger);
    assert_eq!(stats.total_campaigns, 0);
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.total_raised, 0);
    assert_eq!(stats.total_contributors, 0);
}

#[test]
fn test_contract_version_readable_without_init() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    // No `init` call here — `contract_version` must not require it.
    assert_eq!(client.contract_version(), 1);
}

#[test]
fn test_total_raised_global_tracking() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &5000);

    let c1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "First"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&c1);

    let c2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Second"),
        2000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&c2);

    assert_eq!(client.get_total_raised_global(), 0);

    client.contribute(&c1, &contributor1, &500);
    assert_eq!(client.get_total_raised_global(), 500);

    client.contribute(&c2, &contributor2, &1000);
    assert_eq!(client.get_total_raised_global(), 1500);

    client.cancel_campaign(&c2);
    client.claim_refund(&c2, &contributor2);
    assert_eq!(client.get_total_raised_global(), 500);

    client.contribute(&c1, &contributor2, &500);
    assert_eq!(client.get_total_raised_global(), 1000);

    client.withdraw_funds(&c1);
    assert_eq!(client.get_total_raised_global(), 0);
}

#[test]
fn test_creator_campaigns_listing_and_transfer() {
    let (env, _admin, creator1, _c1, _c2, _token, _token_admin, client) = setup_env();
    let creator2 = Address::generate(&env);

    let id1 = client.create_campaign(&make_params(
        creator1.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "First"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    let id2 = client.create_campaign(&make_params(
        creator1.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Second"),
        2000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let list1 = client.get_creator_campaigns(&creator1, &0, &10);
    assert_eq!(list1.0.len(), 2);
    assert_eq!(list1.0.get(0).unwrap().id, id1);
    assert_eq!(list1.0.get(1).unwrap().id, id2);

    let paginated1 = client.get_creator_campaigns(&creator1, &0, &1);
    assert_eq!(paginated1.0.len(), 1);
    assert_eq!(paginated1.0.get(0).unwrap().id, id1);

    let paginated2 = client.get_creator_campaigns(&creator1, &1, &1);
    assert_eq!(paginated2.0.len(), 1);
    assert_eq!(paginated2.0.get(0).unwrap().id, id2);

    let list2 = client.get_creator_campaigns(&creator2, &0, &10);
    assert_eq!(list2.0.len(), 0);

    client.initiate_campaign_transfer(&id1, &creator2);
    client.accept_campaign_transfer(&id1);

    let list1_after = client.get_creator_campaigns(&creator1, &0, &10);
    assert_eq!(list1_after.0.len(), 1);
    assert_eq!(list1_after.0.get(0).unwrap().id, id2);

    let list2_after = client.get_creator_campaigns(&creator2, &0, &10);
    assert_eq!(list2_after.0.len(), 1);
    assert_eq!(list2_after.0.get(0).unwrap().id, id1);
}

#[test]
fn test_platform_stats_after_withdrawal() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &5000);

    // Campaign 1: fund and withdraw
    let c1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Withdrawn"),
        String::from_str(&env, "w"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&c1);
    client.contribute(&c1, &contributor1, &1000);
    client.withdraw_funds(&c1);

    // Campaign 2: still active, funded
    let c2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Active"),
        String::from_str(&env, "a"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&c2);
    client.contribute(&c2, &contributor2, &500);

    let stats = client.get_platform_stats();
    // Only currently held funds (campaign 2's 500), not the withdrawn 1000
    assert_eq!(stats.total_amount_raised, 500);
}

#[test]
fn list_campaigns_boundary_cases() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    for idx in 0..3u32 {
        let title_data = [b'C', b'_', b'1' + idx as u8];
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_bytes(&env, &title_data),
            String::from_str(&env, "Pagination test"),
            1_000 + idx as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
        assert_eq!(id, (idx + 1) as u32);
    }

    let first_page = client.list_campaigns(&0, &2);
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page.get(0).unwrap().id, 1);
    assert_eq!(first_page.get(1).unwrap().id, 2);

    let all = client.list_campaigns(&0, &u32::MAX);
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap().id, 1);
    assert_eq!(all.get(2).unwrap().id, 3);

    let total = client.get_campaign_count();
    assert_eq!(client.list_campaigns(&total, &5).len(), 0);
    assert_eq!(client.list_campaigns(&(total + 1), &5).len(), 0);
    assert_eq!(client.list_campaigns(&0, &0).len(), 0);
}

#[test]
fn list_active_campaigns_boundary_cases_and_sparse_results() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    for idx in 0..5u32 {
        let title_data = [b'C', b'_', b'1' + idx as u8];
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            String::from_bytes(&env, &title_data),
            String::from_str(&env, "Pagination test"),
            1_000 + idx as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    client.cancel_campaign(&2);
    client.cancel_campaign(&4);

    let first_page = client.list_active_campaigns(&0, &2);
    assert_eq!(first_page.0.len(), 2);
    assert_eq!(first_page.0.get(0).unwrap().id, 1);
    assert_eq!(first_page.0.get(1).unwrap().id, 3);

    let sparse_page = client.list_active_campaigns(&1, &2);
    assert_eq!(sparse_page.0.len(), 2);
    assert_eq!(sparse_page.0.get(0).unwrap().id, 3);
    assert_eq!(sparse_page.0.get(1).unwrap().id, 5);

    let all = client.list_active_campaigns(&0, &u32::MAX);
    assert_eq!(all.0.len(), 3);
    assert_eq!(all.0.get(0).unwrap().id, 1);
    assert_eq!(all.0.get(1).unwrap().id, 3);
    assert_eq!(all.0.get(2).unwrap().id, 5);

    let total = client.get_campaign_count();
    assert_eq!(client.list_active_campaigns(&total, &5).0.len(), 0);
    assert_eq!(client.list_active_campaigns(&(total + 1), &5).0.len(), 0);
    assert_eq!(client.list_active_campaigns(&0, &0).0.len(), 0);
}

fn minimal_campaign(env: &soroban_sdk::Env, id: u32, creator: &Address) -> Campaign {
    Campaign {
        id,
        creator: creator.clone(),
        first_creator: creator.clone(),
        pending_creator: MaybePendingCreator::None,
        title: String::from_str(env, "t"),
        description: String::from_str(env, "d"),
        funding_goal: 1_000,
        deadline: 0,
        amount_raised: 0,
        is_active: true,
        funds_withdrawn: false,
        is_cancelled: false,
        is_verified: false,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
        fee_override: None,
        deadline_extended: false,
        effective_amount_raised: 0,
    }
}

/// #534: `get_creator_campaigns` must jump straight to the bucket containing
/// `start` instead of walking every earlier bucket. Seeds two buckets
/// (bucket 0 full, bucket 1 partial) and campaign records directly via
/// crate-internal storage helpers — cheaper than driving
/// `CREATOR_CAMPAIGNS_BUCKET_SIZE` campaigns through the full
/// `create_campaign` flow — and pages a request that starts inside bucket 1.
#[test]
fn test_get_creator_campaigns_jumps_to_bucket_containing_start() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let bucket_size = crate::storage::CREATOR_CAMPAIGNS_BUCKET_SIZE;
    let extra = 5u32;
    let total = bucket_size + extra;

    // Seeding 500+ persistent entries directly exceeds the default test
    // budget (which models real network limits); this setup step isn't
    // what's under test, so lift the cap for it.
    env.budget().reset_unlimited();

    env.as_contract(&client.address, || {
        let mut bucket0 = soroban_sdk::Vec::new(&env);
        for id in 1..=bucket_size {
            bucket0.push_back(id);
            crate::storage::set_campaign(&env, id, &minimal_campaign(&env, id, &creator));
        }
        crate::storage::set_creator_campaign_bucket(&env, &creator, 0, &bucket0);

        let mut bucket1 = soroban_sdk::Vec::new(&env);
        for id in (bucket_size + 1)..=total {
            bucket1.push_back(id);
            crate::storage::set_campaign(&env, id, &minimal_campaign(&env, id, &creator));
        }
        crate::storage::set_creator_campaign_bucket(&env, &creator, 1, &bucket1);

        crate::storage::set_creator_campaign_count(&env, &creator, total);
    });

    env.budget().reset_default();

    // Start pagination two entries before the bucket boundary, spanning into bucket 1.
    let page = client.get_creator_campaigns(&creator, &(bucket_size - 2), &10);
    assert_eq!(page.0.len(), extra + 2);
    assert_eq!(page.0.get(0).unwrap().id, bucket_size - 1);
    assert_eq!(page.0.get(1).unwrap().id, bucket_size);
    assert_eq!(page.0.get(2).unwrap().id, bucket_size + 1);
    assert_eq!(page.0.get(6).unwrap().id, bucket_size + 5);

    // Pagination entirely within bucket 1.
    let tail = client.get_creator_campaigns(&creator, &bucket_size, &10);
    assert_eq!(tail.0.len(), extra);
    assert_eq!(tail.0.get(0).unwrap().id, bucket_size + 1);
    assert_eq!(tail.0.get(extra - 1).unwrap().id, total);
}

#[test]
fn test_list_campaigns_and_list_active_campaigns_boundary_agreement() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    for i in 0..5u32 {
        let title_data = [b'C', b'_', b'1' + i as u8];
        client.create_campaign(&make_params(
            creator.clone(),
            String::from_bytes(&env, &title_data),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    let total = client.get_campaign_count();

    // Both functions should return empty when start == total_count
    let list_at_boundary = client.list_campaigns(&total, &10);
    let active_at_boundary = client.list_active_campaigns(&total, &10);
    assert_eq!(list_at_boundary.len(), 0);
    assert_eq!(active_at_boundary.0.len(), 0);
    assert_eq!(active_at_boundary.1, 0);

    // Both should also return empty when start > total_count
    let list_beyond_boundary = client.list_campaigns(&(total + 1), &10);
    let active_beyond_boundary = client.list_active_campaigns(&(total + 1), &10);
    assert_eq!(list_beyond_boundary.len(), 0);
    assert_eq!(active_beyond_boundary.0.len(), 0);
    assert_eq!(active_beyond_boundary.1, 0);
}

#[test]
fn test_get_creator_stats_zero_campaigns() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let new_creator = Address::generate(&env);

    // Creator with no campaigns should return zeroed stats without panicking
    let stats = client.get_creator_stats(&new_creator);
    assert_eq!(stats.total_campaigns, 0);
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.total_raised, 0);
    assert_eq!(stats.total_contributors, 0);
}

#[test]
fn test_get_platform_stats_after_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract(token_admin.clone());

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token, &200);

    // Immediately after init, all counters should be zero
    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 0);
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.verified_campaigns, 0);
    assert_eq!(stats.cancelled_campaigns, 0);
    assert_eq!(stats.total_amount_raised, 0);
    assert!(!stats.stats_are_partial);
    assert_eq!(stats.scanned_up_to, 0);
}

// ── #849 get_contributor_portfolio bounded pagination ─────────────────────────

/// The portfolio cursor is exclusive over campaign IDs and pages a funded
/// contributor's contributions in campaign order.
#[test]
fn test_get_contributor_portfolio_cursor_pagination() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    for title in ["Portfolio A", "Portfolio B", "Portfolio C", "Portfolio D", "Portfolio E"] {
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, title),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
        client.verify_campaign(&id);
    }

    // Campaigns are id 1..=5; fund only ids 1, 3 and 5.
    for id in [1u32, 3, 5] {
        client.contribute(&id, &contributor1, &100);
    }

    let (page1, cursor1) = client.get_contributor_portfolio(&contributor1, &0, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap().0, 1);
    assert_eq!(page1.get(1).unwrap().0, 3);
    assert_eq!(page1.get(0).unwrap().1, 100);
    assert_eq!(cursor1, 4);

    let (page2, cursor2) = client.get_contributor_portfolio(&contributor1, &cursor1, &2);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2.get(0).unwrap().0, 5);
    assert_eq!(cursor2, 0);

    let (all, cursor_all) = client.get_contributor_portfolio(&contributor1, &0, &u32::MAX);
    assert_eq!(all.len(), 3);
    assert_eq!(cursor_all, 0);
}

/// Boundary cursors and a zero limit return an empty page with a null cursor.
#[test]
fn test_get_contributor_portfolio_boundaries() {
    let (env, _admin, creator, _c1, contributor2, _token, _token_admin, client) = setup_env();

    for title in ["Boundary A", "Boundary B", "Boundary C"] {
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, title),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    let total = client.get_campaign_count();

    // start == total and start > total both mean "nothing left".
    let (empty_at_boundary, cursor_at_boundary) =
        client.get_contributor_portfolio(&contributor2, &total, &10);
    assert_eq!(empty_at_boundary.len(), 0);
    assert_eq!(cursor_at_boundary, 0);

    let (empty_beyond, cursor_beyond) =
        client.get_contributor_portfolio(&contributor2, &(total + 1), &10);
    assert_eq!(empty_beyond.len(), 0);
    assert_eq!(cursor_beyond, 0);

    // limit 0 returns nothing, even though contributions may exist.
    let (empty_zero_limit, cursor_zero_limit) =
        client.get_contributor_portfolio(&contributor2, &0, &0);
    assert_eq!(empty_zero_limit.len(), 0);
    assert_eq!(cursor_zero_limit, 0);
}

/// A request larger than `LIST_MAX_LIMIT` is capped, and the cursor resumes
/// from where the page stopped. Storage is seeded directly (as in
/// `test_get_creator_campaigns_jumps_to_bucket_containing_start`) because
/// driving 60 campaigns through `create_campaign` exceeds the test budget.
#[test]
fn test_get_contributor_portfolio_caps_page_at_list_max_limit() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let total = crate::LIST_MAX_LIMIT + 10;
    env.budget().reset_unlimited();

    env.as_contract(&client.address, || {
        for id in 1..=total {
            crate::storage::set_campaign(&env, id, &minimal_campaign(&env, id, &creator));
            crate::storage::set_contribution(&env, id, &contributor1, 50);
        }
        crate::storage::set_campaign_count(&env, total);
    });

    env.budget().reset_default();

    let (first, cursor) = client.get_contributor_portfolio(&contributor1, &0, &u32::MAX);
    assert_eq!(first.len(), crate::LIST_MAX_LIMIT);
    assert_eq!(first.get(0).unwrap().0, 1);
    assert_eq!(first.get(crate::LIST_MAX_LIMIT - 1).unwrap().0, crate::LIST_MAX_LIMIT);
    assert_eq!(cursor, crate::LIST_MAX_LIMIT + 1);

    let (second, tail_cursor) =
        client.get_contributor_portfolio(&contributor1, &cursor, &u32::MAX);
    assert_eq!(second.len(), 9);
    assert_eq!(second.get(0).unwrap().0, crate::LIST_MAX_LIMIT + 2);
    assert_eq!(second.get(8).unwrap().0, total);
    assert_eq!(tail_cursor, 0);
}

/// A portfolio wider than `MAX_SCAN_WINDOW` campaign IDs is walked across
/// calls: the window is exhausted before the request is satisfied, so the
/// function returns the scan cursor instead of scanning the whole ledger.
#[test]
fn test_get_contributor_portfolio_scan_window_exhaustion() {
    let (env, _admin, _creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let total = crate::constants::MAX_SCAN_WINDOW * 2;
    env.budget().reset_unlimited();

    env.as_contract(&client.address, || {
        crate::storage::set_campaign_count(&env, total);
    });

    // Keep the budget unlimited: this test exercises the scan-window cursor
    // mechanics, and scanning `MAX_SCAN_WINDOW` ids per call under the default
    // host budget (which models real network limits) aborts the process.
    let (page, cursor) = client.get_contributor_portfolio(&contributor1, &0, &50);
    assert_eq!(page.len(), 0);
    assert_eq!(cursor, crate::constants::MAX_SCAN_WINDOW + 1);

    // The next call resumes from the returned cursor and finishes the scan.
    let (tail, tail_cursor) =
        client.get_contributor_portfolio(&contributor1, &cursor, &50);
    assert_eq!(tail.len(), 0);
    assert_eq!(tail_cursor, 0);
}
