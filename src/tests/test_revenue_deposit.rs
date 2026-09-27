use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{Address, FromVal, String, TryFromVal};

#[test]
fn test_deposit_revenue_negative_amount() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &-100);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_zero_amount() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_without_revenue_sharing() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator Campaign"),
        String::from_str(&env, "No revenue sharing"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueSharingNotEnabled);
}

#[test]
fn test_deposit_revenue_when_paused() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    client.pause();

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}

#[test]
fn test_deposit_revenue_non_existent_campaign() {
    let (_env, _admin, _creator, _, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&_admin, &10000);

    let res = client.try_deposit_revenue(&999, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_deposit_revenue_repeated_calls_accumulate_and_emit_events() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Repeated Deposits"),
        description: String::from_str(&env, "Deposit idempotency"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let events_before = env.events().all().len();
    for _ in 0..10 {
        client.deposit_revenue(&campaign_id, &100);
    }
    let events_after = env.events().all().len();
    assert_eq!(client.get_revenue_pool(&campaign_id), 1000);
    assert_eq!(events_after - events_before, 20);
}

#[test]
fn test_deposit_revenue_requires_funds_withdrawn() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue pre-withdraw blocked"),
        String::from_str(&env, "Deposit requires successful withdrawal"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_cancelled_campaign() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // Cancel the campaign (before withdrawal — cancellation after withdrawal is disallowed)
    client.cancel_campaign(&campaign_id);

    // Depositing revenue into a cancelled campaign should fail
    let res = client.try_deposit_revenue(&campaign_id, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_deposit_revenue_event_includes_creator() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Event Topic Test"),
        description: String::from_str(&env, "Verify revenue_deposited event topics"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    token_admin.mint(&creator, &2000);
    client.deposit_revenue(&campaign_id, &2000);

    let events = env.events().all();
    let deposit_event = events
        .iter()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|topic| topic == String::from_str(&env, "revenue_deposited"))
                .unwrap_or(false)
        })
        .expect("revenue_deposited event must exist");

    let topics = &deposit_event.1;
    assert_eq!(topics.len(), 3, "revenue_deposited must have 3 topics");

    let topic_campaign_id: u32 = FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(topic_campaign_id, campaign_id);

    let topic_creator: Address = FromVal::from_val(&env, &topics.get(2).unwrap());
    assert_eq!(topic_creator, creator);

    let amount: i128 = FromVal::from_val(&env, &deposit_event.2);
    assert_eq!(amount, 2000);
}
