use super::helpers::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

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
fn test_creator_campaigns_pagination_within_bucket() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    // Create 50 campaigns — all within a single bucket
    for i in 0..50 {
        let _ = client.create_campaign(&make_params(
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
    }

    let page0 = client.get_creator_campaigns(&creator, &0, &20);
    assert_eq!(page0.len(), 20);
    assert_eq!(page0.get(0).unwrap().id, 1);
    assert_eq!(page0.get(19).unwrap().id, 20);

    let page1 = client.get_creator_campaigns(&creator, &20, &30);
    assert_eq!(page1.len(), 30);
    assert_eq!(page1.get(0).unwrap().id, 21);

    // Beyond bounds
    let out = client.get_creator_campaigns(&creator, &50, &10);
    assert_eq!(out.len(), 0);
}

#[test]
fn test_creator_campaigns_limit_capped_at_list_max_limit() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    // Create 60 campaigns (> LIST_MAX_LIMIT)
    for i in 0..60 {
        let _ = client.create_campaign(&make_params(
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
    }

    // Request 100, but should be capped at LIST_MAX_LIMIT (50)
    let result = client.get_creator_campaigns(&creator, &0, &100);
    assert_eq!(result.len(), 50);
    assert_eq!(result.get(0).unwrap().id, 1);
    assert_eq!(result.get(49).unwrap().id, 50);
}

#[test]
fn test_creator_campaigns_transfer_within_bucket() {
    let (env, _admin, creator1, _c1, _c2, _token, _token_admin, client) = setup_env();
    let creator2 = Address::generate(&env);

    for i in 0..10 {
        let _ = client.create_campaign(&make_params(
            creator1.clone(),
            String::from_str(&env, "Campaign"),
            String::from_str(&env, "Desc"),
            1000 + i as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    assert_eq!(client.get_creator_campaigns(&creator1, &0, &20).len(), 10);

    // Transfer campaign 1
    client.initiate_campaign_transfer(&1, &creator2);
    client.accept_campaign_transfer(&1);

    // creator1 now has 9
    let list_after = client.get_creator_campaigns(&creator1, &0, &20);
    assert_eq!(list_after.len(), 9);

    // creator2 has 1
    let list_new = client.get_creator_campaigns(&creator2, &0, &10);
    assert_eq!(list_new.len(), 1);
    assert_eq!(list_new.get(0).unwrap().id, 1);
}

#[test]
fn test_creator_campaigns_bucket_logic() {
    let env = Env::default();
    env.mock_all_auths();

    let creator = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(Address::generate(&env));
    let contract_id = env.register_contract(None, crate::ProofOfHeart);
    let client = crate::ProofOfHeartClient::new(&env, &contract_id);
    client.init(&Address::generate(&env), &token_address, &300);
    env.as_contract(&client.address, || set_min_campaign_funding_goal(&env, 1));

    // Create campaigns via the normal contract path, staying within the test
    // env's storage budget (~60 entries).
    let count = 10u32;
    for i in 0..count {
        let title = if i == 0 {
            soroban_sdk::String::from_str(&env, "C0")
        } else {
            soroban_sdk::String::from_str(&env, "C1")
        };
        let _ = client.create_campaign(&make_params(
            creator.clone(),
            title,
            soroban_sdk::String::from_str(&env, "D"),
            1000 + i as i128,
            30,
            crate::types::Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    // Verify bucket 0 has exactly `count` entries
    env.as_contract(&client.address, || {
        let b0 = crate::storage::get_creator_campaigns_bucket(&env, &creator, 0);
        assert_eq!(b0.len(), count);

        let count_stored = crate::storage::get_creator_campaign_count(&env, &creator);
        assert_eq!(count_stored, count);
    });

    // Paginate from start
    let page0 = client.get_creator_campaigns(&creator, &0, &5);
    assert_eq!(page0.len(), 5);
    assert_eq!(page0.get(0).unwrap().id, 1);

    // Remaining 5
    let page1 = client.get_creator_campaigns(&creator, &5, &10);
    assert_eq!(page1.len(), 5);
    assert_eq!(page1.get(0).unwrap().id, 6);

    // Verify push_creator_campaign_id properly increments count
    env.as_contract(&client.address, || {
        crate::storage::push_creator_campaign_id(&env, &creator, count + 1);
        let count_stored = crate::storage::get_creator_campaign_count(&env, &creator);
        assert_eq!(count_stored, count + 1);
        let b0 = crate::storage::get_creator_campaigns_bucket(&env, &creator, 0);
        assert_eq!(b0.len(), count + 1);
    });

    // Verify remove_creator_campaign_id
    env.as_contract(&client.address, || {
        let removed = crate::storage::remove_creator_campaign_id(&env, &creator, 5);
        assert!(removed);
        let count_stored = crate::storage::get_creator_campaign_count(&env, &creator);
        assert_eq!(count_stored, count);
        let b0 = crate::storage::get_creator_campaigns_bucket(&env, &creator, 0);
        assert_eq!(b0.len(), count);
    });
}
