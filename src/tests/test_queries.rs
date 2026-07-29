use super::helpers::*;
use crate::{Campaign, Category, MaybePendingCreator};
use soroban_sdk::{Address, String};

#[test]
fn test_list_campaigns_exclusive_cursor_semantics() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    for i in 0..3 {
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
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

    for _ in 0..4 {
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
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
    assert_eq!(learner_page_1.len(), 1);
    assert_eq!(learner_page_1.get(0).unwrap().id, id1);

    let learner_page_2 = client.get_campaigns_by_category(&Category::Learner, &1, &1);
    assert_eq!(learner_page_2.len(), 1);
    assert_eq!(learner_page_2.get(0).unwrap().id, id3);

    let publisher = client.get_campaigns_by_category(&Category::Publisher, &0, &10);
    assert_eq!(publisher.len(), 1);
    assert_eq!(publisher.get(0).unwrap().category, Category::Publisher);
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
    assert_eq!(stats.total_amount_raised, 700);
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
    assert_eq!(stats.total_raised, 700);
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
    assert_eq!(list1.len(), 2);
    assert_eq!(list1.get(0).unwrap().id, id1);
    assert_eq!(list1.get(1).unwrap().id, id2);

    let paginated1 = client.get_creator_campaigns(&creator1, &0, &1);
    assert_eq!(paginated1.len(), 1);
    assert_eq!(paginated1.get(0).unwrap().id, id1);

    let paginated2 = client.get_creator_campaigns(&creator1, &1, &1);
    assert_eq!(paginated2.len(), 1);
    assert_eq!(paginated2.get(0).unwrap().id, id2);

    let list2 = client.get_creator_campaigns(&creator2, &0, &10);
    assert_eq!(list2.len(), 0);

    client.initiate_campaign_transfer(&id1, &creator2);
    client.accept_campaign_transfer(&id1);

    let list1_after = client.get_creator_campaigns(&creator1, &0, &10);
    assert_eq!(list1_after.len(), 1);
    assert_eq!(list1_after.get(0).unwrap().id, id2);

    let list2_after = client.get_creator_campaigns(&creator2, &0, &10);
    assert_eq!(list2_after.len(), 1);
    assert_eq!(list2_after.get(0).unwrap().id, id1);
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

    for idx in 0..3 {
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
            String::from_str(&env, "Pagination test"),
            1_000 + idx as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
        assert_eq!(id, idx + 1);
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

    for idx in 0..5 {
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
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
    assert_eq!(page.len(), extra + 2);
    assert_eq!(page.get(0).unwrap().id, bucket_size - 1);
    assert_eq!(page.get(1).unwrap().id, bucket_size);
    assert_eq!(page.get(2).unwrap().id, bucket_size + 1);
    assert_eq!(page.get(6).unwrap().id, bucket_size + 5);

    // Pagination entirely within bucket 1.
    let tail = client.get_creator_campaigns(&creator, &bucket_size, &10);
    assert_eq!(tail.len(), extra);
    assert_eq!(tail.get(0).unwrap().id, bucket_size + 1);
    assert_eq!(tail.get(extra - 1).unwrap().id, total);
}
