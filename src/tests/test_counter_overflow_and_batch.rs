//! Regression tests for counter overflow in `create_campaign` (#837, #838) and
//! for `batch_contribute` keeping `last_contribution_time` current (#836).

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams, Error};
use soroban_sdk::Vec;

fn params(env: &Env, creator: &Address, title: &str, category: Category) -> CreateCampaignParams {
    make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "Counter boundary"),
        1_000,
        30,
        category,
        false,
        0,
        0,
    )
}

// ── #837: category counter ───────────────────────────────────────────────────

#[test]
fn test_category_counter_at_max_returns_overflow() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_category_campaign_count(&env, Category::Learner, u32::MAX);
    });

    let res = client.try_create_campaign(&params(&env, &creator, "At max", Category::Learner));

    assert_eq!(res, Err(Ok(Error::Overflow)));
    // The failed call reverted as a whole: no campaign was stored.
    assert!(client.try_get_campaign(&1).is_err());
    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_category_campaign_count(&env, Category::Learner),
            u32::MAX
        );
    });
}

#[test]
fn test_category_counter_one_below_max_still_succeeds() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_category_campaign_count(&env, Category::Learner, u32::MAX - 1);
    });

    let id = client.create_campaign(&params(&env, &creator, "Last slot", Category::Learner));

    assert_eq!(id, 1);
    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_category_campaign_count(&env, Category::Learner),
            u32::MAX
        );
    });
    // The next creation in the same category is refused rather than wrapping.
    let res =
        client.try_create_campaign(&params(&env, &creator, "One too many", Category::Learner));
    assert_eq!(res, Err(Ok(Error::Overflow)));
}

#[test]
fn test_category_counter_overflow_is_scoped_to_its_category() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_category_campaign_count(&env, Category::Learner, u32::MAX);
    });

    // A different category is unaffected.
    let id = client.create_campaign(&params(&env, &creator, "Other", Category::Educator));
    assert_eq!(id, 1);
}

// ── #838: creator counter ────────────────────────────────────────────────────

#[test]
fn test_creator_counter_at_max_returns_overflow() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_creator_campaign_count(&env, &creator, u32::MAX);
    });

    let res = client.try_create_campaign(&params(&env, &creator, "At max", Category::Learner));

    assert_eq!(res, Err(Ok(Error::Overflow)));
    assert!(client.try_get_campaign(&1).is_err());
    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_creator_campaign_count(&env, &creator),
            u32::MAX
        );
    });
}

#[test]
fn test_creator_counter_one_below_max_still_succeeds() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_creator_campaign_count(&env, &creator, u32::MAX - 1);
    });

    let id = client.create_campaign(&params(&env, &creator, "Last slot", Category::Learner));

    assert_eq!(id, 1);
    env.as_contract(&client.address, || {
        assert_eq!(
            storage::get_creator_campaign_count(&env, &creator),
            u32::MAX
        );
    });
    let res =
        client.try_create_campaign(&params(&env, &creator, "One too many", Category::Educator));
    assert_eq!(res, Err(Ok(Error::Overflow)));
}

#[test]
fn test_creator_counter_overflow_is_scoped_to_its_creator() {
    let (env, _admin, creator, contributor, _c2, _token, _token_admin, client) = setup_env();
    env.as_contract(&client.address, || {
        storage::set_creator_campaign_count(&env, &creator, u32::MAX);
    });

    let id = client.create_campaign(&params(
        &env,
        &contributor,
        "Someone else",
        Category::Learner,
    ));
    assert_eq!(id, 1);
}

// ── #836: batch_contribute and last_contribution_time ────────────────────────

fn verified_campaign(
    env: &Env,
    creator: &Address,
    client: &ProofOfHeartClient,
    title: &str,
) -> u32 {
    let id = client.create_campaign(&params(env, creator, title, Category::Learner));
    client.verify_campaign(&id);
    id
}

#[test]
fn test_batch_contribute_updates_last_contribution_time_for_every_campaign() {
    let (env, _admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let a = verified_campaign(&env, &creator, &client, "Campaign A");
    let b = verified_campaign(&env, &creator, &client, "Campaign B");

    env.ledger().with_mut(|l| l.timestamp += 1_000);
    let now = env.ledger().timestamp();
    assert_ne!(client.get_campaign_stats(&a).last_contribution_time, now);

    let mut items: Vec<(u32, i128)> = Vec::new(&env);
    items.push_back((a, 100));
    items.push_back((b, 200));
    client.batch_contribute(&contributor, &items);

    assert_eq!(client.get_campaign_stats(&a).last_contribution_time, now);
    assert_eq!(client.get_campaign_stats(&b).last_contribution_time, now);
}

#[test]
fn test_batch_and_single_contribute_report_the_same_timestamp() {
    let (env, _admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let single = verified_campaign(&env, &creator, &client, "Single");
    let batched = verified_campaign(&env, &creator, &client, "Batched");

    env.ledger().with_mut(|l| l.timestamp += 500);
    client.contribute(&single, &contributor, &100);
    let mut items: Vec<(u32, i128)> = Vec::new(&env);
    items.push_back((batched, 100));
    client.batch_contribute(&contributor, &items);

    assert_eq!(
        client.get_campaign_stats(&single).last_contribution_time,
        client.get_campaign_stats(&batched).last_contribution_time
    );
}

#[test]
fn test_failed_batch_leaves_last_contribution_time_untouched() {
    let (env, _admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let a = verified_campaign(&env, &creator, &client, "Good");
    let before = client.get_campaign_stats(&a).last_contribution_time;

    env.ledger().with_mut(|l| l.timestamp += 1_000);
    let mut items: Vec<(u32, i128)> = Vec::new(&env);
    items.push_back((a, 100));
    items.push_back((999, 100)); // unknown campaign: the whole batch reverts
    assert!(client.try_batch_contribute(&contributor, &items).is_err());

    assert_eq!(client.get_campaign_stats(&a).last_contribution_time, before);
}
