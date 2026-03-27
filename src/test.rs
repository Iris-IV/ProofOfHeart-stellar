#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;

fn setup_env<'a>() -> (Env, Address, Address, Address, Address, TokenClient<'a>, TokenAdminClient<'a>, ProofOfHeartClient<'a>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor1 = Address::generate(&env);
    let contributor2 = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token = TokenClient::new(&env, &token_address);
    let token_admin = TokenAdminClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token.address, &300);

    (env, admin, creator, contributor1, contributor2, token, token_admin, client)
}

#[test]
fn test_init_only_once() {
    let (_env, admin, _creator, _c1, _c2, token, _token_admin, client) = setup_env();
    let res = client.try_init(&admin, &token.address, &300);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyInitialized);
}

#[test]
fn test_create_and_validation() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Science Book");
    let desc = String::from_str(&env, "Teaching science to kids");

    // Test goal validation
    let res = client.try_create_campaign(&creator, &title, &desc, &0, &30, &Category::Publisher, &false, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);

    // Test duration validation
    let res = client.try_create_campaign(&creator, &title, &desc, &500, &0, &Category::Publisher, &false, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&creator, &title, &desc, &500, &400, &Category::Publisher, &false, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(&creator, &title, &desc, &500, &30, &Category::Educator, &true, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueShareOnlyForStartup);

    let campaign_id = client.create_campaign(&creator, &title, &desc, &2000, &30, &Category::EducationalStartup, &true, &1500);
    assert_eq!(campaign_id, 1);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.id, 1);
    assert_eq!(campaign.funding_goal, 2000);
    assert_eq!(campaign.is_active, true);
    assert_eq!(campaign.is_verified, false);
}

#[test]
fn test_contribute_and_withdraw_success() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Code Camp");
    let desc = String::from_str(&env, "Learn Rust");
    let campaign_id = client.create_campaign(&creator, &title, &desc, &1000, &30, &Category::Educator, &false, &0);

    client.contribute(&campaign_id, &contributor1, &1000);

    assert_eq!(token.balance(&contributor1), 4000);
    assert_eq!(token.balance(&client.address), 1000);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1000);

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.is_active, false);
    assert_eq!(campaign.funds_withdrawn, true);
}

#[test]
fn test_cancel_and_refund() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&contributor2, &1000);

    let title = String::from_str(&env, "Failed Idea");
    let desc = String::from_str(&env, "Desc");
    let campaign_id = client.create_campaign(&creator, &title, &desc, &5000, &10, &Category::Learner, &false, &0);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &500);

    client.cancel_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.is_cancelled, true);

    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);

    assert_eq!(token.balance(&contributor1), 2000);
    assert_eq!(token.balance(&contributor2), 1000);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_pull_based_revenue_distribution() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    token_admin.mint(&creator, &10000);

    let title = String::from_str(&env, "Next Gen AI");
    let desc = String::from_str(&env, "Build AI");
    let campaign_id = client.create_campaign(&creator, &title, &desc, &2000, &30, &Category::EducationalStartup, &true, &2000); 

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);

    client.withdraw_funds(&campaign_id);
    
    // Deposit revenue
    client.deposit_revenue(&campaign_id, &5000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 5000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2500);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 2500);

    client.deposit_revenue(&campaign_id, &1000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 6000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 3000); 

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 3000);
}

#[test]
fn test_failure_states() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);
    
    let title = String::from_str(&env, "Deadline Test");
    let desc = String::from_str(&env, "Desc");
    let duration_days = 2;
    let campaign_id = client.create_campaign(&creator, &title, &desc, &1000, &duration_days, &Category::Educator, &false, &0);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.contribute(&campaign_id, &contributor1, &500);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    env.ledger().set(soroban_sdk::testutils::LedgerInfo { timestamp: env.ledger().timestamp() + (duration_days * 86450), protocol_version: 20, sequence_number: env.ledger().sequence(), network_id: [0; 32], base_reserve: 10, min_temp_entry_ttl: 10, min_persistent_entry_ttl: 10, max_entry_ttl: 10 }); 

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);

    // After failure refund successful
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
}
