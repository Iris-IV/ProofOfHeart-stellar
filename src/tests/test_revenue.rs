use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_pull_based_revenue_distribution() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Next Gen AI"),
        String::from_str(&env, "Build AI"), 2000, 30,
        Category::EducationalStartup, true, 2000, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);
    client.withdraw_funds(&campaign_id);

    token_admin.mint(&creator, &5000);
    client.deposit_revenue(&campaign_id, &5000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 5000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 500);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 500);

    client.deposit_revenue(&campaign_id, &1000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 6000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 600);

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 600);
}

#[test]
fn test_revenue_sharing_edge_cases() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    let campaign_nr = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "No Revenue"),
        String::from_str(&env, "Non-revenue campaign"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_nr);
    let res = client.try_claim_revenue(&campaign_nr, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    token_admin.mint(&contributor1, &10);
    token_admin.mint(&contributor2, &10);
    token_admin.mint(&creator, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Rounding Test"),
        String::from_str(&env, "Test rounding and pool edge cases"), 3, 30,
        Category::EducationalStartup, true, 5000, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1);
    client.contribute(&campaign_id, &contributor2, &2);
    client.withdraw_funds(&campaign_id);

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.deposit_revenue(&campaign_id, &10);
    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 10);

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 11);

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_claim_revenue_requires_contributor_auth() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Revenue Claim Auth"),
        String::from_str(&env, "Testing claim revenue auth"), 1000, 10,
        Category::EducationalStartup, true, 1000, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    token_admin.mint(&creator, &5000);
    client.deposit_revenue(&campaign_id, &5000);

    env.mock_all_auths();
    client.claim_revenue(&campaign_id, &contributor1);

    let auths = env.auths();
    let found = auths.iter().any(|(addr, inv)| {
        *addr == contributor1
            && match &inv.function {
                soroban_sdk::testutils::AuthorizedFunction::Contract((contract, function, _)) => {
                    contract == &client.address
                        && function == &soroban_sdk::Symbol::new(&env, "claim_revenue")
                }
                _ => false,
            }
    });
    assert!(found);
}

#[test]
fn test_revenue_lifecycle_e2e() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Revenue Sharing Campaign"),
        String::from_str(&env, "Full lifecycle test: create, fund, withdraw, deposit revenue, claim"),
        6000, 30, Category::EducationalStartup, true, 2000, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &4000);
    client.contribute(&campaign_id, &contributor2, &2500);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, 6500);
    assert!(campaign.amount_raised >= campaign.funding_goal);

    client.withdraw_funds(&campaign_id);

    let campaign_after_withdrawal = client.get_campaign(&campaign_id);
    assert!(campaign_after_withdrawal.funds_withdrawn);
    assert!(!campaign_after_withdrawal.is_active);

    token_admin.mint(&creator, &10000);
    client.deposit_revenue(&campaign_id, &10000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 10000);

    let contrib1_claimed_before = client.get_revenue_claimed(&campaign_id, &contributor1);
    client.claim_revenue(&campaign_id, &contributor1);
    let contrib1_claimed_after = client.get_revenue_claimed(&campaign_id, &contributor1);
    assert!(contrib1_claimed_after > contrib1_claimed_before);

    let contrib2_claimed_before = client.get_revenue_claimed(&campaign_id, &contributor2);
    client.claim_revenue(&campaign_id, &contributor2);
    let contrib2_claimed_after = client.get_revenue_claimed(&campaign_id, &contributor2);
    assert!(contrib2_claimed_after > contrib2_claimed_before);

    client.claim_creator_revenue(&campaign_id);

    assert!(client.try_claim_revenue(&campaign_id, &contributor1).is_err());
    assert!(client.try_claim_revenue(&campaign_id, &contributor2).is_err());

    assert!(!env.events().all().is_empty());
}
