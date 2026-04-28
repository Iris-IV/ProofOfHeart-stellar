use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::String;

#[test]
fn test_deposit_revenue_negative_amount() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"), 1000, 30,
        Category::EducationalStartup, true, 2000, 0i128,
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
        creator.clone(), String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"), 1000, 30,
        Category::EducationalStartup, true, 2000, 0i128,
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
        creator.clone(), String::from_str(&env, "Educator Campaign"),
        String::from_str(&env, "No revenue sharing"), 1000, 30,
        Category::Educator, false, 0, 0i128,
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
        creator.clone(), String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"), 1000, 30,
        Category::EducationalStartup, true, 2000, 0i128,
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
