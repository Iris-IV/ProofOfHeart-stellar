//! #1039: campaign categories are validated against the taxonomy at creation.

extern crate alloc;

use super::helpers::*;
use crate::Error;

fn params(env: &Env, creator: &Address, title: &str, category: Category) -> CreateCampaignParams {
    make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "A description long enough to pass"),
        1_000,
        30,
        category,
        false,
        0,
        0,
    )
}

#[test]
fn test_category_taxonomy_is_complete() {
    for (i, category) in Category::ALL.iter().enumerate() {
        assert_eq!(*category as u32, i as u32);
        assert_eq!(Category::from_u32(i as u32), Some(*category));
    }
    assert_eq!(Category::from_u32(Category::ALL.len() as u32), None);
    assert_eq!(Category::from_u32(u32::MAX), None);
}

#[test]
fn test_every_category_is_enabled_by_default() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    for (i, category) in Category::ALL.iter().enumerate() {
        assert!(client.is_category_enabled(category));
        let title = alloc::format!("Campaign number {i}");
        client.create_campaign(&params(&env, &creator, &title, *category));
    }
}

#[test]
fn test_disabled_category_rejects_new_campaigns() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    client.set_category_enabled(&admin, &Category::Publisher, &false);
    assert!(!client.is_category_enabled(&Category::Publisher));
    assert!(client.is_category_enabled(&Category::Learner));

    let res = client.try_create_campaign(&params(
        &env,
        &creator,
        "Closed category",
        Category::Publisher,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Other categories are unaffected.
    client.create_campaign(&params(&env, &creator, "Open category", Category::Learner));

    // Re-enabling restores creation.
    client.set_category_enabled(&admin, &Category::Publisher, &true);
    assert!(client.is_category_enabled(&Category::Publisher));
    client.create_campaign(&params(
        &env,
        &creator,
        "Closed category",
        Category::Publisher,
    ));
}

#[test]
fn test_disabling_a_category_leaves_existing_campaigns_alone() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&params(
        &env,
        &creator,
        "Before closing",
        Category::Educator,
    ));

    client.set_category_enabled(&admin, &Category::Educator, &false);

    let campaign = client.get_campaign(&id);
    assert_eq!(campaign.category, Category::Educator);
    assert!(campaign.is_active());
}

#[test]
fn test_only_admin_can_change_category_state() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let res = client.try_set_category_enabled(&creator, &Category::Learner, &false);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
    assert!(client.is_category_enabled(&Category::Learner));
    let _ = env;
}
