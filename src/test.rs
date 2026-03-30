use super::*;
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
use soroban_sdk::{
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

    client.init(&admin, &token_address, &300);

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

    // contributor_pool = (5000 * 2000) / 10000 = 1000 (20% of pool to contributors)
    // contributor1 share = (1000 * 1000) / 2000 = 500
    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 500);
    assert_eq!(
        client.get_revenue_claimed(&campaign_id, &contributor1),
        500
    );

    client.deposit_revenue(&campaign_id, &1000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 6000);

    // contributor1: total_due = (1000 * 1200) / 2000 = 600, already_claimed = 500, claimable = 100
    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 600);

    // contributor2: total_due = (1000 * 1200) / 2000 = 600, claimable = 600
    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 600);
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

    assert!(c1_after_withdraw.funds_withdrawn);
    assert!(!c1_after_withdraw.is_active);

    assert_eq!(c2_after_withdraw.amount_raised, 900);
    assert!(!c2_after_withdraw.funds_withdrawn);
    assert!(c2_after_withdraw.is_active);
    assert!(!c2_after_withdraw.is_cancelled);

    assert_eq!(c3_after_withdraw.amount_raised, 2000);
    assert!(!c3_after_withdraw.funds_withdrawn);
    assert!(c3_after_withdraw.is_active);
    assert!(!c3_after_withdraw.is_cancelled);

    client.cancel_campaign(&campaign_2);

    let c1_after_cancel = client.get_campaign(&campaign_1);
    let c2_after_cancel = client.get_campaign(&campaign_2);
    let c3_after_cancel = client.get_campaign(&campaign_3);

    assert!(c2_after_cancel.is_cancelled);
    assert!(!c2_after_cancel.is_active);

    assert!(c1_after_cancel.funds_withdrawn);
    assert!(!c1_after_cancel.is_cancelled);
    assert!(c3_after_cancel.is_active);
    assert!(!c3_after_cancel.is_cancelled);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);

    client.deposit_revenue(&campaign_3, &3000);

    assert_eq!(client.get_revenue_pool(&campaign_1), 0);
    assert_eq!(client.get_revenue_pool(&campaign_2), 0);
    assert_eq!(client.get_revenue_pool(&campaign_3), 3000);

    // Balance checks to ensure campaign operations remained isolated
    assert_eq!(token.balance(&client.address), 5900);
    assert_eq!(token.balance(&creator3), 7000);
}

#[test]
fn test_double_refund_prevention() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &2000);

    let title = String::from_str(&env, "Double Refund");
    let desc = String::from_str(&env, "Test double refund");
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
    assert_eq!(token.balance(&contributor1), 2000);

    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
    assert_eq!(token.balance(&contributor1), 2000);
}

#[test]
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

    client.verify_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);
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
        &funding_goal,
        &duration_days,
        &Category::Educator,
        &false,
        &0,
    );

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
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    // 1. Non-revenue campaign: check ValidationFailed
    let title_nr = String::from_str(&env, "No Revenue");
    let desc_nr = String::from_str(&env, "Non-revenue campaign");
    let campaign_nr = client.create_campaign(
        &creator,
        &title_nr,
        &desc_nr,
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );
    let res = client.try_claim_revenue(&campaign_nr, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    token_admin.mint(&contributor1, &10);
    token_admin.mint(&contributor2, &10);
    token_admin.mint(&creator, &100);

    let title = String::from_str(&env, "Rounding Test");
    let desc = String::from_str(&env, "Test rounding and pool edge cases");
    // 50% revenue share to contributors (5000 bps = max allowed)
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &3,
        &30,
        &Category::EducationalStartup,
        &true,
        &5000,
    );

    client.contribute(&campaign_id, &contributor1, &1);
    client.contribute(&campaign_id, &contributor2, &2);
    client.withdraw_funds(&campaign_id);

    // 2. Zero pool: should fail NoFundsToWithdraw
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    // 3. Rounding: pool=10, contributor_pool = (10*5000)/10000 = 5
    // contributor1 (1 of 3): (1 * 5) / 3 = 1
    // contributor2 (2 of 3): (2 * 5) / 3 = 3
    client.deposit_revenue(&campaign_id, &10);
    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 10); // Initial 10 - 1 contribution + 1 claimed

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 11); // Initial 10 - 2 contribution + 3 claimed

    // 4. Double claim: NoFundsToWithdraw
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_view_functions_error_handling() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    // Create a valid campaign for relative testing
    let title = String::from_str(&env, "View Test");
    let desc = String::from_str(&env, "Testing view functions");
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

    let stranger = Address::generate(&env);
    let invalid_id = 999u32;

    // 1. get_campaign with invalid ID
    // Expected: Returns Error::CampaignNotFound (previously panicked before Issue #8 fix)
    let res = client.try_get_campaign(&invalid_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);

    // 2. get_contribution with valid campaign but non-existent contributor
    // Expected: Returns 0 (no panic)
    assert_eq!(client.get_contribution(&campaign_id, &stranger), 0);

    // 3. get_contribution with invalid campaign ID
    // Expected: Returns 0 (no panic)
    assert_eq!(client.get_contribution(&invalid_id, &contributor1), 0);

    // 4. get_revenue_pool with invalid campaign ID
    // Expected: Returns 0 (no panic)
    assert_eq!(client.get_revenue_pool(&invalid_id), 0);

    // 5. get_revenue_claimed with valid campaign but non-existent contributor
    // Expected: Returns 0 (no panic)
    assert_eq!(client.get_revenue_claimed(&campaign_id, &stranger), 0);

    // 6. get_revenue_claimed with invalid campaign ID
    // Expected: Returns 0 (no panic)
    assert_eq!(client.get_revenue_claimed(&invalid_id, &contributor1), 0);
}

// ── Issue #68: update_campaign_description ────────────────────────────────────

#[test]
fn test_update_campaign_description_success() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &10_000);

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Original Title"),
        &String::from_str(&env, "Original description"),
        &1_000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    // Contribute so amount_raised > 0 (update_campaign would reject this)
    token_admin.mint(&contributor1, &1_000);
    client.contribute(&campaign_id, &contributor1, &500);

    let new_desc = String::from_str(&env, "Updated description with more detail");
    let res = client.try_update_campaign_description(&campaign_id, &new_desc);
    assert!(res.is_ok());

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.description, new_desc);
    // Funding goal and deadline must remain unchanged
    assert_eq!(campaign.funding_goal, 1_000);
}

#[test]
fn test_update_campaign_description_rejects_cancelled() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "Desc"),
        &1_000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    client.cancel_campaign(&campaign_id);

    let res = client.try_update_campaign_description(
        &campaign_id,
        &String::from_str(&env, "New desc"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_update_campaign_description_rejects_empty() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "Desc"),
        &1_000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    let res = client.try_update_campaign_description(
        &campaign_id,
        &String::from_str(&env, ""),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_update_campaign_description_not_found() {
    let (env, _, _, _, _, _, _, client) = setup_env();

    let res = client.try_update_campaign_description(
        &999,
        &String::from_str(&env, "Some desc"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_campaign_ownership_transfer_flow() {
    let (env, _admin, creator, contributor1, contributor2, _, _, client) = setup_env();
    let new_creator = contributor1;

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Transfer Test"),
        &String::from_str(&env, "Desc"),
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );

    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::Some(new_creator.clone()));
    assert_eq!(campaign.creator, creator);

    client.accept_campaign_transfer(&campaign_id);

    let campaign_after = client.get_campaign(&campaign_id);
    assert_eq!(campaign_after.creator, new_creator.clone());
    assert_eq!(campaign_after.pending_creator, MaybePendingCreator::None);

    let updated_description = String::from_str(&env, "Managed by the transferred owner");
    client.update_campaign_description(&campaign_id, &updated_description);

    let auths = env.auths();
    let (auth_addr, invocation) = auths.last().unwrap();
    assert_eq!(auth_addr, &new_creator);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "update_campaign_description"),
                (campaign_id, updated_description).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );

    let campaign_id_2 = client.create_campaign(
        &new_creator,
        &String::from_str(&env, "Cancel Test"),
        &String::from_str(&env, "Desc"),
        &1000,
        &30,
        &Category::Educator,
        &false,
        &0,
    );
    client.initiate_campaign_transfer(&campaign_id_2, &contributor2);
    client.cancel_campaign_transfer(&campaign_id_2);
    let final_campaign = client.get_campaign(&campaign_id_2);
    assert_eq!(final_campaign.pending_creator, MaybePendingCreator::None);
}

#[test]
fn test_campaign_transfer_validations() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Transfer Guardrails"),
        &String::from_str(&env, "Desc"),
        &1000,
        &30,
        &Category::Publisher,
        &false,
        &0,
    );

    let res = client.try_initiate_campaign_transfer(&campaign_id, &creator);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);

    let res = client.try_accept_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);

    client.initiate_campaign_transfer(&campaign_id, &contributor1);
    client.cancel_campaign_transfer(&campaign_id);

    // Verify cancel was authorized by the creator
    let auths = env.auths();
    let (auth_addr, _) = auths.last().unwrap();
    assert_eq!(auth_addr, &creator);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::None);

    let res = client.try_cancel_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);
}

#[test]
fn test_pause_and_unpause() {
    let (_env, admin, _creator, _contributor1, _, _token, _token_admin, client) = setup_env();

    // Initially not paused
    assert!(!client.is_paused());

    // Pause
    client.pause(&admin);

    // Now paused
    assert!(client.is_paused());

    // Unpause
    client.unpause(&admin);

    // Not paused
    assert!(!client.is_paused());
}

#[test]
fn test_pause_blocks_state_changing_operations() {
    let (env, admin, creator, contributor1, _contributor2, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &10000);

    let title = String::from_str(&env, "Paused Test");
    let desc = String::from_str(&env, "Testing pause functionality");

    // Create campaign before pause
    let campaign_id = client.create_campaign(
        &creator,
        &title,
        &desc,
        &1000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    // Pause
    client.pause(&admin);
    assert!(client.is_paused());

    // Try state-changing operations, should fail
    let res = client.try_create_campaign(
        &creator,
        &String::from_str(&env, "New Campaign"),
        &desc,
        &1000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );
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

    // View functions should still work
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, title);

    let paused = client.is_paused();
    assert!(paused);

    // Unpause
    client.unpause(&admin);
    assert!(!client.is_paused());

    // Now operations should work
    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    let _ = token;
}

// ── Deadline edge-case tests (#74) ───────────────────────────────────────────

/// A contribution made 1 second before the deadline must succeed.
#[test]
fn test_contribute_one_second_before_deadline() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Almost Deadline"),
        &String::from_str(&env, "Desc"),
        &1000,
        &1,
        &Category::Learner,
        &false,
        &0,
    );

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

/// Withdrawing while the deadline has not yet passed and the goal is not met
/// returns `FundingGoalNotReached`.
#[test]
fn test_withdraw_before_deadline_goal_not_met_fails() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Early Withdraw"),
        &String::from_str(&env, "Desc"),
        &10_000,
        &30,
        &Category::Learner,
        &false,
        &0,
    );

    client.contribute(&campaign_id, &contributor1, &500);

    // Deadline has not passed; goal not met — withdraw must fail
    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundingGoalNotReached);
}

/// A contributor can claim a refund only after the deadline has passed
/// and the campaign failed to reach its goal.
#[test]
fn test_refund_requires_deadline_passed_and_goal_missed() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Failed Campaign"),
        &String::from_str(&env, "Desc"),
        &10_000,
        &1,
        &Category::Learner,
        &false,
        &0,
    );

    client.contribute(&campaign_id, &contributor1, &500);

    // Before deadline: refund must fail (campaign still active, goal not met yet)
    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    let deadline = client.get_campaign(&campaign_id).deadline;
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

    // After deadline, goal not reached: refund succeeds
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
}

/// A contributor whose campaign reached its goal cannot claim a refund.
#[test]
fn test_no_refund_when_goal_reached() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(
        &creator,
        &String::from_str(&env, "Successful Campaign"),
        &String::from_str(&env, "Desc"),
        &500,
        &1,
        &Category::Learner,
        &false,
        &0,
    );

    // Meet the funding goal exactly
    client.contribute(&campaign_id, &contributor1, &500);

    let deadline = client.get_campaign(&campaign_id).deadline;
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

    // Goal was reached — refund must be rejected
    let res = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}
