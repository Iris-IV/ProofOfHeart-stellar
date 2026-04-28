use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{String};

#[test]
fn test_update_platform_fee() {
    let (env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let result = client.try_update_platform_fee(&500);
    assert!(result.is_ok());

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let expected_topics = (String::from_str(&env, "fee_updated"),).into_val(&env);
    assert_eq!(last_event.1, expected_topics);

    let data_vec: soroban_sdk::Vec<u32> = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data_vec.get(0).unwrap(), 300);
    assert_eq!(data_vec.get(1).unwrap(), 500);

    let result = client.try_update_platform_fee(&5000);
    assert!(result.is_ok());
}

#[test]
fn test_pause_and_unpause() {
    let (_env, _admin, _creator, _contributor1, _, _token, _token_admin, client) = setup_env();

    assert!(!client.is_paused());

    client.pause();
    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn test_pause_blocks_state_changing_operations() {
    let (env, _admin, creator, contributor1, _contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Paused Test"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.pause();
    assert!(client.is_paused());

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "New Campaign"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_update_platform_fee(&400);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, String::from_str(&env, "Paused Test"));

    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    let _ = token;
}
