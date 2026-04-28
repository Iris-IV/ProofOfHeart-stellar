use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_contribute_and_withdraw_success() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Code Camp"),
        String::from_str(&env, "Learn Rust"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    assert_eq!(token.balance(&contributor1), 4000);
    assert_eq!(token.balance(&client.address), 1000);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1000);

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);

    let campaign = client.get_campaign(&campaign_id);
    assert!(!campaign.is_active);
    assert!(campaign.funds_withdrawn);
}

#[test]
fn test_creator_cannot_contribute_to_own_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Self Funding Block"),
        String::from_str(&env, "Creator should not contribute"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_contribute(&campaign_id, &creator, &100);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_failure_states() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let duration_days = 2;
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Deadline Test"),
        String::from_str(&env, "Desc"), 1000, duration_days,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.contribute(&campaign_id, &contributor1, &500);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (duration_days * 86450),
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
}

#[test]
fn test_multiple_concurrent_campaigns_are_isolated() {
    let (env, _admin, creator1, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    let creator2 = Address::generate(&env);
    let creator3 = Address::generate(&env);

    token_admin.mint(&contributor1, &10000);
    token_admin.mint(&contributor2, &10000);
    token_admin.mint(&creator3, &10000);

    let campaign_1 = client.create_campaign(&make_params(
        creator1.clone(), String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Educator campaign"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_1);

    let campaign_2 = client.create_campaign(&make_params(
        creator2.clone(), String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Learner campaign"), 1500, 30,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_2);

    let campaign_3 = client.create_campaign(&make_params(
        creator3.clone(), String::from_str(&env, "Campaign 3"),
        String::from_str(&env, "Startup campaign"), 2000, 30,
        Category::EducationalStartup, true, 1500, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_3);

    assert_eq!(campaign_1, 1);
    assert_eq!(campaign_2, 2);
    assert_eq!(campaign_3, 3);
    assert_eq!(client.get_campaign_count(), 3);

    client.contribute(&campaign_1, &contributor1, &1000);
    client.contribute(&campaign_2, &contributor1, &400);
    client.contribute(&campaign_2, &contributor2, &500);
    client.contribute(&campaign_3, &contributor1, &1200);
    client.contribute(&campaign_3, &contributor2, &800);

    assert_eq!(client.get_contribution(&campaign_1, &contributor1), 1000);
    assert_eq!(client.get_contribution(&campaign_1, &contributor2), 0);
    assert_eq!(client.get_contribution(&campaign_2, &contributor1), 400);
    assert_eq!(client.get_contribution(&campaign_2, &contributor2), 500);
    assert_eq!(client.get_contribution(&campaign_3, &contributor1), 1200);
    assert_eq!(client.get_contribution(&campaign_3, &contributor2), 800);

    client.withdraw_funds(&campaign_1);

    assert!(client.get_campaign(&campaign_1).funds_withdrawn);
    assert!(!client.get_campaign(&campaign_1).is_active);
    assert_eq!(client.get_campaign(&campaign_2).amount_raised, 900);
    assert!(!client.get_campaign(&campaign_2).funds_withdrawn);
    assert_eq!(client.get_campaign(&campaign_3).amount_raised, 2000);

    client.cancel_campaign(&campaign_2);
    assert!(client.get_campaign(&campaign_2).is_cancelled);
    assert!(client.get_campaign(&campaign_3).is_active);

    client.deposit_revenue(&campaign_3, &3000);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);
    assert_eq!(client.get_revenue_pool(&campaign_3), 3000);

    assert_eq!(token.balance(&client.address), 5900);
    assert_eq!(token.balance(&creator3), 7000);
}

#[test]
fn test_deadline_boundary() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Boundary Test"),
        String::from_str(&env, "Testing exact deadline boundary"), 1000, 2,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let deadline = client.get_campaign(&campaign_id).deadline;

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

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

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_contribution_accounting_invariant() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    let contributor3 = Address::generate(&env);

    token_admin.mint(&contributor1, &3000);
    token_admin.mint(&contributor2, &3000);
    token_admin.mint(&contributor3, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Invariant Campaign"),
        String::from_str(&env, "Accounting invariant check"), 5000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &500);
    client.contribute(&campaign_id, &contributor2, &750);
    client.contribute(&campaign_id, &contributor3, &250);
    client.contribute(&campaign_id, &contributor1, &300);
    client.contribute(&campaign_id, &contributor2, &200);

    let c1 = client.get_contribution(&campaign_id, &contributor1);
    let c2 = client.get_contribution(&campaign_id, &contributor2);
    let c3 = client.get_contribution(&campaign_id, &contributor3);

    assert_eq!(c1, 800);
    assert_eq!(c2, 950);
    assert_eq!(c3, 250);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(c1 + c2 + c3, campaign.amount_raised);
}

#[test]
fn test_view_functions_error_handling() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "View Test"),
        String::from_str(&env, "Testing view functions"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let stranger = Address::generate(&env);
    let invalid_id = 999u32;

    let res = client.try_get_campaign(&invalid_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);

    assert_eq!(client.get_contribution(&campaign_id, &stranger), 0);
    assert_eq!(client.get_contribution(&invalid_id, &contributor1), 0);
    assert_eq!(client.get_revenue_pool(&invalid_id), 0);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &stranger), 0);
    assert_eq!(client.get_revenue_claimed(&invalid_id, &contributor1), 0);
}

#[test]
fn test_contribute_one_second_before_deadline() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Almost Deadline"),
        String::from_str(&env, "Desc"), 1000, 1,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let deadline = client.get_campaign(&campaign_id).deadline;

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: deadline - 1,
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);
}
