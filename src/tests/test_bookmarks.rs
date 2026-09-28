use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_create_and_get_campaign() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Test Campaign"),
        String::from_str(&env, "A test campaign"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.creator, creator);
    assert_eq!(campaign.is_active, true);
    assert_eq!(campaign.amount_raised, 0);
}

#[test]
fn test_list_campaigns_pagination() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    for _i in 0..3 {
        client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
            String::from_str(&env, "Description"),
            1_000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
    }

    let campaigns = client.list_campaigns(&0, &10);
    assert_eq!(campaigns.len(), 3);
}

#[test]
fn test_get_nonexistent_campaign() {
    let (_env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let res = client.try_get_campaign(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}
