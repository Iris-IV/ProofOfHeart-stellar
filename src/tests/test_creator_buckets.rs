use super::helpers::*;
use crate::{Category, LIST_MAX_LIMIT};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn create_campaign(env: &Env, client: &ProofOfHeartClient<'_>, creator: &Address, idx: u32) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Campaign"),
        String::from_str(env, "Bucket test"),
        1000 + idx as i128,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ))
}

/// Returns all campaign IDs for a creator by paginating.
fn all_creator_ids(env: &Env, client: &ProofOfHeartClient<'_>, creator: &Address) -> soroban_sdk::Vec<u32> {
    let mut ids = soroban_sdk::Vec::new(env);
    let mut start = 0u32;
    loop {
        let page = client.get_creator_campaigns(creator, &start, &LIST_MAX_LIMIT);
        let len = page.len();
        if len == 0 {
            break;
        }
        for i in 0..len {
            ids.push_back(page.get(i).unwrap().id);
        }
        start += len;
        if len < LIST_MAX_LIMIT {
            break;
        }
    }
    ids
}

#[test]
fn test_creator_buckets_100_campaigns() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let total_campaigns = 100u32;
    for idx in 0..total_campaigns {
        let id = create_campaign(&env, &client, &creator, idx);
        assert_eq!(id, idx + 1);
    }

    // Collect all IDs by paginating
    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), total_campaigns);

    for i in 0..total_campaigns {
        assert_eq!(ids.get(i).unwrap(), i + 1);
    }

    // LIST_MAX_LIMIT cap
    let big_page = client.get_creator_campaigns(&creator, &0, &u32::MAX);
    assert_eq!(big_page.len(), LIST_MAX_LIMIT as u32);
}

#[test]
fn test_creator_buckets_pagination_boundaries() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let total = 100u32;
    for idx in 0..total {
        create_campaign(&env, &client, &creator, idx);
    }

    let last_page = client.get_creator_campaigns(&creator, &95, &10);
    assert_eq!(last_page.len(), 5);
    assert_eq!(last_page.get(0).unwrap().id, 96);
    assert_eq!(last_page.get(4).unwrap().id, 100);

    let empty = client.get_creator_campaigns(&creator, &total, &10);
    assert_eq!(empty.len(), 0);

    let empty2 = client.get_creator_campaigns(&creator, &(total + 10), &10);
    assert_eq!(empty2.len(), 0);

    let zero = client.get_creator_campaigns(&creator, &0, &0);
    assert_eq!(zero.len(), 0);
}

#[test]
fn test_creator_buckets_transfer_single() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let receiver = Address::generate(&env);

    // Create 15 campaigns
    for idx in 0..15 {
        create_campaign(&env, &client, &creator, idx);
    }

    // Transfer the first campaign
    client.initiate_campaign_transfer(&1, &receiver);
    client.accept_campaign_transfer(&1);

    // Old creator should have 14 campaigns, without id 1
    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), 14);
    assert!(verify_missing(&env, &client, &creator, 1));

    // Receiver should have 1 campaign
    let ids = all_creator_ids(&env, &client, &receiver);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids.get(0).unwrap(), 1);
}

#[test]
fn test_creator_buckets_transfer_multiple() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let receiver = Address::generate(&env);

    // Create 15 campaigns
    for idx in 0..15 {
        create_campaign(&env, &client, &creator, idx);
    }

    // Transfer first, last, and middle
    client.initiate_campaign_transfer(&1, &receiver);
    client.accept_campaign_transfer(&1);
    client.initiate_campaign_transfer(&15, &receiver);
    client.accept_campaign_transfer(&15);
    client.initiate_campaign_transfer(&7, &receiver);
    client.accept_campaign_transfer(&7);

    assert!(verify_missing(&env, &client, &creator, 1));
    assert!(verify_missing(&env, &client, &creator, 15));
    assert!(verify_missing(&env, &client, &creator, 7));
    assert_eq!(all_creator_ids(&env, &client, &creator).len(), 12);

    let receiver_ids = all_creator_ids(&env, &client, &receiver);
    assert_eq!(receiver_ids.len(), 3);
    assert_eq!(receiver_ids.get(0).unwrap(), 1);
    assert_eq!(receiver_ids.get(1).unwrap(), 15);
    assert_eq!(receiver_ids.get(2).unwrap(), 7);
}

fn verify_missing(env: &Env, client: &ProofOfHeartClient<'_>, creator: &Address, missing_id: u32) -> bool {
    let ids = all_creator_ids(env, client, creator);
    for i in 0..ids.len() {
        if ids.get(i).unwrap() == missing_id {
            return false;
        }
    }
    true
}

#[test]
fn test_creator_buckets_multiple_creators() {
    let (env, _admin, creator1, _c1, _c2, _token, _token_admin, client) = setup_env();
    let creator2 = Address::generate(&env);

    for idx in 0..30 {
        create_campaign(&env, &client, &creator1, idx);
    }
    for idx in 0..20 {
        client.create_campaign(&make_params(
            creator2.clone(),
            String::from_str(&env, "Creator2"),
            String::from_str(&env, "Test"),
            1000 + idx as i128,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    let ids1 = all_creator_ids(&env, &client, &creator1);
    assert_eq!(ids1.len(), 30);
    let ids2 = all_creator_ids(&env, &client, &creator2);
    assert_eq!(ids2.len(), 20);
}

#[test]
fn test_creator_buckets_internal_state() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    for idx in 0..50 {
        create_campaign(&env, &client, &creator, idx);
    }

    // Check count via the contract
    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), 50);

    // Transfer one
    let receiver = Address::generate(&env);
    client.initiate_campaign_transfer(&1, &receiver);
    client.accept_campaign_transfer(&1);

    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), 49);
    assert!(verify_missing(&env, &client, &creator, 1));

    let ids = all_creator_ids(&env, &client, &receiver);
    assert_eq!(ids.len(), 1);
}
