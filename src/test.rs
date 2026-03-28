#![cfg(test)]

use super::*;
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger},
    Address, Env, IntoVal, String, Symbol,
};

fn setup_env<'a>() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'a>,
    TokenAdminClient<'a>,
    ProofOfHeartClient<'a>,
) {
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

    (
        env,
        admin,
        creator,
        contributor1,
        contributor2,
        token,
        token_admin,
        client,
    )
}

#[test]
fn test_create_and_validation() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Science Book");
    let desc = String::from_str(&env, "Teaching science to kids");

    // Test goal validation
    let res = client.try_create_campaign(
        &creator,
        &title,
        &desc,
        &0,
        &30,
        &Category::Publisher,
        &false,
        &0,
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalMustBePositive);

    // Test duration validation
    let res = client.try_create_campaign(
        &creator,
        &title,
        &desc,
        &500,
        &0,
        &Category::Publisher,
        &false,
        &0,
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(
        &creator,
        &title,
        &desc,
        &500,
        &400,
        &Category::Publisher,
        &false,
        &0,
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let res = client.try_create_campaign(
        &creator,
        &title,
        &desc,
        &500,
        &30,
        &Category::Educator,
        &true,
        &1000,
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueShareOnlyForStartup);

    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &2000,
        &30,
        &Category::EducationalStartup,
        &true,
        &1500,
    );
    assert_eq!(campaign_id, 1);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.id, 1);
    assert_eq!(campaign.funding_goal, 2000);
    assert!(campaign.is_active);
    assert!(!campaign.is_verified);
}

#[test]
fn test_contribute_and_withdraw_success() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Code Camp");
    let desc = String::from_str(&env, "Learn Rust");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );

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
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let title = String::from_str(&env, "Self Funding Block");
    let desc = String::from_str(&env, "Creator should not contribute");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );

    let res = client.try_contribute(&campaign_id, &creator, &100);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_cancel_and_refund() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&contributor2, &1000);

    let title = String::from_str(&env, "Failed Idea");
    let desc = String::from_str(&env, "Desc");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &5000,
        &10,
        &Category::Learner,
        &false,
        &0,
    );

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &500);

    client.cancel_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);

    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);

    assert_eq!(token.balance(&contributor1), 2000);
    assert_eq!(token.balance(&contributor2), 1000);
    assert_eq!(token.balance(&client.address), 0);
}

#[test]
fn test_claim_refund_requires_contributor_auth() {
    let (env, _admin, creator, contributor1, _contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);

    let title = String::from_str(&env, "Auth Refund");
    let desc = String::from_str(&env, "Only contributor can claim");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &5000,
        &10,
        &Category::Learner,
        &false,
        &0,
    );

    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);

    client.claim_refund(&campaign_id, &contributor1);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let (auth_addr, invocation) = &auths[0];
    assert_eq!(auth_addr, &contributor1);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "claim_refund"),
                (campaign_id, contributor1.clone()).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );

    assert_eq!(token.balance(&contributor1), 2000);
}

#[test]
fn test_pull_based_revenue_distribution() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    token_admin.mint(&creator, &10000);

    let title = String::from_str(&env, "Next Gen AI");
    let desc = String::from_str(&env, "Build AI");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &2000,
        &30,
        &Category::EducationalStartup,
        &true,
        &2000,
    );

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);

    client.withdraw_funds(&campaign_id);

    // Deposit revenue
    client.deposit_revenue(&campaign_id, &5000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 5000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2500);
    assert_eq!(
        client.get_revenue_claimed(&campaign_id, &contributor1),
        2500
    );

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
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &duration_days,
        &Category::Educator,
        &false,
        &0,
    );

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.contribute(&campaign_id, &contributor1, &500);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (duration_days * 86450),
        protocol_version: 20,
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
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);

    // After failure refund successful
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

    let c1_title = String::from_str(&env, "Campaign 1");
    let c1_desc = String::from_str(&env, "Educator campaign");
    let campaign_1 = client.create_campaign(
        &creator1,
        &c1_title,
        &c1_desc,
fn test_get_version() {
    let (_env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    // init stores CONTRACT_VERSION (1) in instance storage
    assert_eq!(client.get_version(), 1u32);
}

#[test]
fn test_admin_verify_campaign_success() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let title = String::from_str(&env, "Admin Verification");
    let desc = String::from_str(&env, "Admin verifies campaign");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );

    let c2_title = String::from_str(&env, "Campaign 2");
    let c2_desc = String::from_str(&env, "Learner campaign");
    let campaign_2 = client.create_campaign(
        &creator2,
        &c2_title,
        &c2_desc,
        &1500,
        &30,
        &Category::Learner,
    client.verify_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.is_verified, true);
}

#[test]
fn test_admin_verify_campaign_duplicate_attempt() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let title = String::from_str(&env, "Duplicate Verification");
    let desc = String::from_str(&env, "Cannot verify twice");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Publisher,
        &false,
        &0,
    );

    client.verify_campaign(&campaign_id);
    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
}

#[test]
fn test_community_voting_verification_success() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let voter3 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);

    let title = String::from_str(&env, "Community Verified");
    let desc = String::from_str(&env, "Verify by voting");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );

    let c3_title = String::from_str(&env, "Campaign 3");
    let c3_desc = String::from_str(&env, "Startup campaign");
    let campaign_3 = client.create_campaign(
        &creator3,
        &c3_title,
        &c3_desc,
        &2000,
        &30,
        &Category::EducationalStartup,
        &true,
        &1500,
    );

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

    let c1_after_withdraw = client.get_campaign(&campaign_1);
    let c2_after_withdraw = client.get_campaign(&campaign_2);
    let c3_after_withdraw = client.get_campaign(&campaign_3);

    assert_eq!(c1_after_withdraw.funds_withdrawn, true);
    assert_eq!(c1_after_withdraw.is_active, false);

    assert_eq!(c2_after_withdraw.amount_raised, 900);
    assert_eq!(c2_after_withdraw.funds_withdrawn, false);
    assert_eq!(c2_after_withdraw.is_active, true);
    assert_eq!(c2_after_withdraw.is_cancelled, false);

    assert_eq!(c3_after_withdraw.amount_raised, 2000);
    assert_eq!(c3_after_withdraw.funds_withdrawn, false);
    assert_eq!(c3_after_withdraw.is_active, true);
    assert_eq!(c3_after_withdraw.is_cancelled, false);

    client.cancel_campaign(&campaign_2);

    let c1_after_cancel = client.get_campaign(&campaign_1);
    let c2_after_cancel = client.get_campaign(&campaign_2);
    let c3_after_cancel = client.get_campaign(&campaign_3);

    assert_eq!(c2_after_cancel.is_cancelled, true);
    assert_eq!(c2_after_cancel.is_active, false);

    assert_eq!(c1_after_cancel.funds_withdrawn, true);
    assert_eq!(c1_after_cancel.is_cancelled, false);
    assert_eq!(c3_after_cancel.is_active, true);
    assert_eq!(c3_after_cancel.is_cancelled, false);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);

    client.deposit_revenue(&campaign_3, &3000);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);
    assert_eq!(client.get_revenue_pool(&campaign_3), 3000);

    // Balance checks to ensure campaign operations remained isolated.
    assert_eq!(token.balance(&client.address), 5900);
    assert_eq!(token.balance(&creator3), 7000);
}

#[test]
fn test_double_refund_prevention() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &2000);

    let title = String::from_str(&env, "Double Refund");
    let desc = String::from_str(&env, "Test double refund");
    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &false);

    assert_eq!(client.get_approve_votes(&campaign_id), 2);
    assert_eq!(client.get_reject_votes(&campaign_id), 1);
    assert!(client.has_voted(&campaign_id, &contributor1));

    client.verify_campaign_with_votes(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);
}

#[test]
fn test_vote_prevents_double_voting_and_requires_token_holder() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let non_holder = Address::generate(&env);

    token_admin.mint(&contributor1, &100);

    let title = String::from_str(&env, "Vote Safety");
    let desc = String::from_str(&env, "No duplicate votes");
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &500,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    client.vote_on_campaign(&campaign_id, &contributor1, &true);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &false);
    assert_eq!(res.unwrap_err().unwrap(), Error::AlreadyVoted);

    let res = client.try_vote_on_campaign(&campaign_id, &non_holder, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);
}

#[test]
fn test_verify_campaign_quorum_and_threshold_edges() {
    let (env, admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let voter3 = Address::generate(&env);
    let voter4 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);
    token_admin.mint(&voter4, &100);

    client.set_voting_params(&admin, &4, &7500);
    assert_eq!(client.get_min_votes_quorum(), 4);
    assert_eq!(client.get_approval_threshold_bps(), 7500);

    let title1 = String::from_str(&env, "Quorum Campaign");
    let desc1 = String::from_str(&env, "Needs 4 votes");
    let campaign_id_1 = client.create_campaign(
        &creator,
        &title1,
        &desc1,
        &700,
        &30,
        &Category::Publisher,
        &false,
        &0,
    );

    client.vote_on_campaign(&campaign_id_1, &contributor1, &true);
    client.vote_on_campaign(&campaign_id_1, &contributor2, &true);
    client.vote_on_campaign(&campaign_id_1, &voter3, &true);

    let res = client.try_verify_campaign_with_votes(&campaign_id_1);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingQuorumNotMet);

    client.vote_on_campaign(&campaign_id_1, &voter4, &false);
    client.verify_campaign(&campaign_id_1);
    assert!(client.get_campaign(&campaign_id_1).is_verified);

    let title2 = String::from_str(&env, "Threshold Campaign");
    let desc2 = String::from_str(&env, "Fails threshold");
    let campaign_id_2 = client.create_campaign(
        &creator,
        &title2,
        &desc2,
        &700,
        &30,
        &Category::Publisher,
        &false,
        &0,
    );

    client.vote_on_campaign(&campaign_id_2, &contributor1, &true);
    client.vote_on_campaign(&campaign_id_2, &contributor2, &true);
    client.vote_on_campaign(&campaign_id_2, &voter3, &false);
    client.vote_on_campaign(&campaign_id_2, &voter4, &false);

    let res = client.try_verify_campaign_with_votes(&campaign_id_2);
    assert_eq!(res.unwrap_err().unwrap(), Error::VotingThresholdNotMet);
}

#[test]
fn test_update_platform_fee() {
    let (_env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let result = client.try_update_platform_fee(&500);
    assert!(
        result.is_ok(),
        "Admin should be able to update platform fee"
    );

    let result = client.try_update_platform_fee(&5000);
    assert!(result.is_ok(), "Fee update should succeed even when capped");
}

#[test]
fn test_get_campaign_not_found() {
    let (_env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    // Attempting to get a Campaign with a non-existent ID should return CampaignNotFound
    let res = client.try_get_campaign(&999);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_deadline_boundary() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Boundary Test");
    let desc = String::from_str(&env, "Testing exact deadline boundary");
    let duration_days = 2;
    let funding_goal = 1000;

    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &5000,
        &10,
        &Category::Learner,
        &funding_goal,
        &duration_days,
        &Category::Educator,
        &false,
        &0,
    );

    client.contribute(&campaign_id, &contributor1, &1000);
    client.cancel_campaign(&campaign_id);

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2000);

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
    assert_eq!(token.balance(&contributor1), 2000);
    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;

    // Fast forward to exactly the deadline
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

    // Should succeed exactly at the deadline
    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    // Fast forward to exactly 1 second past the deadline
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

    // Should fail past the deadline
    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

#[test]
fn test_reinit_prevention() {
    let (env, admin, _, _, _, token, _, client) = setup_env();

    let attacker = Address::generate(&env);
    let fake_token = Address::generate(&env);

    // Attempt re-initialization with different values — must be rejected
    let res = client.try_init(&attacker, &fake_token, &0);
    assert!(res.is_err()); // Should fail with AlreadyInitialized

    // Verify original values remain unchanged after rejected re-init
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
}

#[test]
fn test_initialization_getters() {
    let (_, admin, _, _, _, token, _, client) = setup_env();

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_token(), token.address);
    assert_eq!(client.get_platform_fee(), 300);
    assert_eq!(client.get_campaign_count(), 0);
}

#[test]
fn test_revenue_sharing_edge_cases() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();

    // 1. Non-revenue campaign: check ValidationFailed
    let title_nr = String::from_str(&env, "No Revenue");
    let desc_nr = String::from_str(&env, "Non-revenue campaign");
    let campaign_nr = client.create_campaign(
        &creator, &title_nr, &desc_nr, &1000, &30, &Category::Educator, &false, &0
    );
    let res = client.try_claim_revenue(&campaign_nr, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    token_admin.mint(&contributor1, &10);
    token_admin.mint(&contributor2, &10);
    token_admin.mint(&creator, &100);

    let title = String::from_str(&env, "Rounding Test");
    let desc = String::from_str(&env, "Test rounding and pool edge cases");
    // 100% revenue share (10000 bps)
    let campaign_id = client.create_campaign(
        &creator, &title, &desc, &3, &30, &Category::EducationalStartup, &true, &10000
    );

    client.contribute(&campaign_id, &contributor1, &1);
    client.contribute(&campaign_id, &contributor2, &2);
    client.withdraw_funds(&campaign_id);

    // 2. Zero pool: should fail NoFundsToWithdraw
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    // 3. Rounding: 10 revenue / 3 contribution units (1 vs 2)
    client.deposit_revenue(&campaign_id, &10); 
    client.claim_revenue(&campaign_id, &contributor1); // (1 * 10) / 3 = 3
    assert_eq!(token.balance(&contributor1), 12); // Initial 10 - 1 contribution + 3 claimed

    client.claim_revenue(&campaign_id, &contributor2); // (2 * 10) / 3 = 6
    assert_eq!(token.balance(&contributor2), 14); // Initial 10 - 2 contribution + 6 claimed

    // 4. Double claim: NoFundsToWithdraw
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}
