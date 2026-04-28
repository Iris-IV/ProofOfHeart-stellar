use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::String;

#[test]
fn test_extend_campaign_deadline_happy_path() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Extend Me"),
        String::from_str(&env, "Will be extended"), 1000, 10,
        Category::Educator, false, 0, 0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &7);

    let new_deadline = client.get_campaign(&id).deadline;
    assert_eq!(new_deadline, original_deadline + 7 * 86400);
    assert!(client.get_campaign(&id).deadline_extended);
}

#[test]
fn test_extend_deadline_emits_event() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Event Extension"),
        String::from_str(&env, "Check event"), 1000, 10,
        Category::Learner, false, 0, 0i128,
    ));

    client.extend_campaign_deadline(&id, &5);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.1.len(), 2);
}

#[test]
fn test_extend_deadline_double_extension_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Double Extension"),
        String::from_str(&env, "Only one extension"), 1000, 10,
        Category::Educator, false, 0, 0i128,
    ));

    client.extend_campaign_deadline(&id, &7);

    let res = client.try_extend_campaign_deadline(&id, &7);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlineAlreadyExtended);
}

#[test]
fn test_extend_deadline_post_deadline_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Expired"),
        String::from_str(&env, "Past deadline"), 1000, 1,
        Category::Educator, false, 0, 0i128,
    ));

    let deadline = client.get_campaign(&id).deadline;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline + 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_extend_campaign_deadline(&id, &7);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_extend_deadline_too_many_days_rejected() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Too Long"),
        String::from_str(&env, "Extension too long"), 1000, 10,
        Category::Educator, false, 0, 0i128,
    ));

    let res = client.try_extend_campaign_deadline(&id, &31);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);

    let res = client.try_extend_campaign_deadline(&id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);
}

#[test]
fn test_extend_deadline_max_30_days_allowed() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Max Extension"),
        String::from_str(&env, "Exactly 30 days"), 1000, 10,
        Category::Educator, false, 0, 0i128,
    ));

    let original_deadline = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &30);

    let new_deadline = client.get_campaign(&id).deadline;
    assert_eq!(new_deadline, original_deadline + 30 * 86400);
}
