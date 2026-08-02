use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::String;

#[test]
fn test_save_and_get_saved_campaigns() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    assert_eq!(
        client.get_saved_campaigns(&contributor1),
        soroban_sdk::vec![&env]
    );

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved.get(0).unwrap(), id1);
    assert_eq!(saved.get(1).unwrap(), id2);
}

#[test]
fn test_save_campaign_nonexistent_fails() {
    let (_env, _admin, _creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let result = client.try_save_campaign(&contributor1, &999);
    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));
}

#[test]
fn test_save_campaign_duplicate_fails() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
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

    client.save_campaign(&contributor1, &id);
    let result = client.try_save_campaign(&contributor1, &id);
    assert_eq!(result, Err(Ok(Error::CampaignAlreadyBookmarked)));
}

#[test]
fn test_remove_saved_campaign() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);

    client.remove_saved_campaign(&contributor1, &id1);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved.get(0).unwrap(), id2);
}

#[test]
fn test_remove_saved_campaign_not_bookmarked_fails() {
    let (_env, _admin, _creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let result = client.try_remove_saved_campaign(&contributor1, &1);
    assert_eq!(result, Err(Ok(Error::CampaignNotBookmarked)));
}

#[test]
fn test_saved_campaigns_are_per_wallet() {
    let (env, _admin, creator, contributor1, contributor2, _token, _token_admin, client) =
        setup_env();

    let id = client.create_campaign(&make_params(
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

    client.save_campaign(&contributor1, &id);

    assert_eq!(client.get_saved_campaigns(&contributor1).len(), 1);
    assert_eq!(client.get_saved_campaigns(&contributor2).len(), 0);
}

#[test]
fn test_save_campaign_then_cancel() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
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

    // Contributor bookmarks the campaign
    client.save_campaign(&contributor1, &id);
    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved.get(0).unwrap(), id);

    // Creator cancels the campaign
    client.cancel_campaign(&id);

    // Bookmarks still persist after cancellation (documented gap #667)
    // Frontend/clients should filter cancelled campaigns from the UI
    let saved_after_cancel = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_cancel.len(), 1);
    assert_eq!(saved_after_cancel.get(0).unwrap(), id);

    // Campaign is cancelled
    let campaign = client.get_campaign(&id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);
}

#[test]
fn test_get_saved_returns_insertion_order_after_interleaved_add_remove_add() {
    // Verifies that get_saved returns campaign ids in the order they were saved,
    // even after a mid-list removal. The doc comment promises "in the order they
    // were saved", which should hold after remove operations.
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    // Create three campaigns
    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id3 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 3"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Save all three in order: [id1, id2, id3]
    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);
    client.save_campaign(&contributor1, &id3);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 3);
    assert_eq!(saved.get(0).unwrap(), id1);
    assert_eq!(saved.get(1).unwrap(), id2);
    assert_eq!(saved.get(2).unwrap(), id3);

    // Remove the middle campaign (id2)
    client.remove_saved_campaign(&contributor1, &id2);

    let saved_after_remove = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_remove.len(), 2);
    assert_eq!(saved_after_remove.get(0).unwrap(), id1);
    assert_eq!(saved_after_remove.get(1).unwrap(), id3);

    // Re-add id2 - it should be appended at the end, not inserted back in its original position
    client.save_campaign(&contributor1, &id2);

    let saved_after_readd = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_readd.len(), 3);
    // Order should reflect insertion order: id1, id3 (from before), then id2 (re-added)
    assert_eq!(saved_after_readd.get(0).unwrap(), id1);
    assert_eq!(saved_after_readd.get(1).unwrap(), id3);
    assert_eq!(saved_after_readd.get(2).unwrap(), id2);
}
