use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::String;

#[test]
fn test_claim_refund_state_mutation_order() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Refund Order Test"),
        String::from_str(&env, "Testing state mutation order"), 10000, 10,
        Category::Learner, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1000);
    assert_eq!(token.balance(&contributor1), 4000);
    assert_eq!(token.balance(&client.address), 1000);

    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(token.balance(&client.address), 0);

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_claim_refund_multiple_contributors_isolation() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Multi Refund Test"),
        String::from_str(&env, "Testing multiple refunds"), 10000, 10,
        Category::Learner, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &2000);
    client.contribute(&campaign_id, &contributor2, &1500);
    client.cancel_campaign(&campaign_id);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);

    assert_eq!(client.get_contribution(&campaign_id, &contributor2), 1500);
    assert_eq!(token.balance(&contributor2), 1500);

    client.claim_refund(&campaign_id, &contributor2);
    assert_eq!(client.get_contribution(&campaign_id, &contributor2), 0);
    assert_eq!(token.balance(&contributor2), 3000);
}

#[test]
fn test_claim_refund_expired_campaign() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let duration_days = 2;
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Expired Campaign"),
        String::from_str(&env, "Will expire"), 10000, duration_days,
        Category::Learner, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

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

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

#[test]
fn test_claim_refund_clears_existing_revenue_claimed_key() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Refund Cleans Revenue Claim"),
        description: String::from_str(&env, "Ensure RevenueClaimed key is removed"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.deposit_revenue(&campaign_id, &1000);
    client.claim_revenue(&campaign_id, &contributor1);

    let claimed_before_refund = client.get_revenue_claimed(&campaign_id, &contributor1);
    assert!(claimed_before_refund > 0);

    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}
