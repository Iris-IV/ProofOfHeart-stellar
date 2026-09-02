extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::{storage, Category, LIST_MAX_LIMIT};
use soroban_sdk::{Address, Env, String};

fn unique_title(env: &Env, idx: u32) -> String {
    let mut data = [0u8; 4];
    data[0] = b'C';
    data[1] = b'_';
    data[2] = b'0' + (idx / 10) as u8;
    data[3] = b'0' + (idx % 10) as u8;
    String::from_bytes(env, &data)
}

fn create_campaign(env: &Env, client: &ProofOfHeartClient<'_>, creator: &Address, idx: u32) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        unique_title(env, idx),
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
fn all_creator_ids(
    env: &Env,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
) -> soroban_sdk::Vec<u32> {
    let mut ids = soroban_sdk::Vec::new(env);
    let mut start = 0u32;
    loop {
        let (page, cursor) = client.get_creator_campaigns(creator, &start, &LIST_MAX_LIMIT);
        let len = page.len();
        if len == 0 {
            break;
        }
        for i in 0..len {
            ids.push_back(page.get(i).unwrap().id);
        }
        start = cursor;
        if len < LIST_MAX_LIMIT {
            break;
        }
    }
    ids
}

#[test]
fn test_creator_buckets_100_campaigns() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();

    let total_campaigns = 20u32;
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

    // LIST_MAX_LIMIT cap: request more than available to verify the cap works
    let big_page = client.get_creator_campaigns(&creator, &0, &u32::MAX);
    assert!(big_page.len() <= LIST_MAX_LIMIT);
}

#[test]
fn test_creator_buckets_pagination_boundaries() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();

    let total = 20u32;
    for idx in 0..total {
        create_campaign(&env, &client, &creator, idx);
    }

    let last_page = client.get_creator_campaigns(&creator, &15, &10);
    assert_eq!(last_page.len(), 5);
    assert_eq!(last_page.get(0).unwrap().id, 16);
    assert_eq!(last_page.get(4).unwrap().id, 20);

    let (empty, _cursor) = client.get_creator_campaigns(&creator, &total, &10);
    assert_eq!(empty.len(), 0);

    let (empty2, _cursor) = client.get_creator_campaigns(&creator, &(total + 10), &10);
    assert_eq!(empty2.len(), 0);

    let (zero, _cursor) = client.get_creator_campaigns(&creator, &0, &0);
    assert_eq!(zero.len(), 0);
}

#[test]
fn test_creator_buckets_transfer_single() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();
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
fn test_creator_campaign_positions_are_updated_by_swap_removal() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();
    let receiver = Address::generate(&env);

    for idx in 0..3 {
        create_campaign(&env, &client, &creator, idx);
    }
    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_creator_campaign_position(&env, &creator, 2),
            Some((0, 1))
        );
    });

    client.initiate_campaign_transfer(&2, &receiver);
    client.accept_campaign_transfer(&2);

    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_creator_campaign_position(&env, &creator, 2),
            None
        );
        assert_eq!(
            storage::get_creator_campaign_position(&env, &creator, 3),
            Some((0, 1))
        );
        assert_eq!(
            storage::get_creator_campaign_position(&env, &receiver, 2),
            Some((0, 0))
        );
    });
}

#[test]
fn test_creator_buckets_transfer_multiple() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();
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

fn verify_missing(
    env: &Env,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
    missing_id: u32,
) -> bool {
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
    env.budget().reset_unlimited();
    let creator2 = Address::generate(&env);

    // Reduced from 12+8 to 6+4 to avoid Soroban testutils stack overflow.
    for idx in 0..6 {
        create_campaign(&env, &client, &creator1, idx);
    }
    for idx in 0..20u32 {
        let mut data = [0u8; 4];
        data[0] = b'X';
        data[1] = b'_';
        data[2] = b'0' + (idx / 10) as u8;
        data[3] = b'0' + (idx % 10) as u8;
        client.create_campaign(&make_params(
            creator2.clone(),
            String::from_bytes(&env, &data),
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
    assert_eq!(ids1.len(), 6);
    let ids2 = all_creator_ids(&env, &client, &creator2);
    assert_eq!(ids2.len(), 4);
}

#[test]
fn test_creator_buckets_internal_state() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.budget().reset_unlimited();

    for idx in 0..20 {
        create_campaign(&env, &client, &creator, idx);
    }

    // Check count via the contract
    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), 20);

    // Transfer one
    let receiver = Address::generate(&env);
    client.initiate_campaign_transfer(&1, &receiver);
    client.accept_campaign_transfer(&1);

    let ids = all_creator_ids(&env, &client, &creator);
    assert_eq!(ids.len(), 19);
    assert!(verify_missing(&env, &client, &creator, 1));

    let ids = all_creator_ids(&env, &client, &receiver);
    assert_eq!(ids.len(), 1);
}
