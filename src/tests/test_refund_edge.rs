use super::helpers::*;
use crate::storage;
use crate::{Category, DataKey, Error};
use soroban_sdk::String;

#[test]
fn test_claim_refund_state_mutation_order() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Refund Order Test"),
        String::from_str(&env, "Testing state mutation order"),
        10000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Multi Refund Test"),
        String::from_str(&env, "Testing multiple refunds"),
        10000,
        10,
        Category::Learner,
        false,
        0,
        0i128,
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
        creator.clone(),
        String::from_str(&env, "Expired Campaign"),
        String::from_str(&env, "Will expire"),
        10000,
        duration_days,
        Category::Learner,
        false,
        0,
        0i128,
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
    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Refund Cleans Revenue Claim"),
        description: String::from_str(&env, "Ensure RevenueClaimed key is removed"),
        funding_goal: 5000,
        duration_days: 2,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    // Artificially mark funds as withdrawn so deposit/claim_revenue bypass the guard.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    client.deposit_revenue(&campaign_id, &1000);
    client.claim_revenue(&campaign_id, &contributor1);

    let claimed_before_refund = client.get_revenue_claimed(&campaign_id, &contributor1);
    assert!(claimed_before_refund > 0);

    // Advance past deadline so claim_refund accepts failed_due_to_goal
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (2 * 86450),
        protocol_version: 22,
        sequence_number: env.ledger().sequence(),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    // claim_refund with deadline passed and goal not met clears revenue_claimed.
    client.claim_refund(&campaign_id, &contributor1);

    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

#[test]
fn test_claim_revenue_after_single_refund_uses_live_raised() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue Refund Denominator"),
        String::from_str(
            &env,
            "Remaining contributor receives full share after refund",
        ),
        2000,
        10,
        Category::EducationalStartup,
        true,
        5000,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);

    // Withdraw funds so funds_withdrawn = true (required for deposit/claim revenue).
    client.withdraw_funds(&campaign_id);
    client.deposit_revenue(&campaign_id, &1000);

    // Simulate a refund for contributor1 via storage: zero out their contribution
    // and reduce effective_amount_raised — without cancelling the campaign.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.effective_amount_raised = 1000;
        storage::set_campaign(&env, campaign_id, &campaign);
        storage::remove_contribution(&env, campaign_id, &contributor1);
    });

    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    // Contributor2 can still claim their full revenue share
    // (1000 / 1000 * 5000 / 10000 * 1000 = 500).
    client.claim_revenue(&campaign_id, &contributor2);

    assert_eq!(token.balance(&contributor2), 4500);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor2), 500);
}
fn has_persistent_key(env: &Env, client: &ProofOfHeartClient<'_>, key: DataKey) -> bool {
    env.as_contract(&client.address, || env.storage().persistent().has(&key))
}

#[test]
fn test_storage_cleaned_after_claim_refund_on_cancel() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cleanup Cancel Refund"),
        String::from_str(&env, "Storage cleanup test"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund"
    );
    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::RevenueClaimed(id, contributor1.clone())
        ),
        "RevenueClaimed key must not exist after refund"
    );
}

#[test]
fn test_storage_cleaned_after_claim_refund_on_failed_campaign() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cleanup Failed"),
        String::from_str(&env, "Failed campaign refund"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);

    env.ledger().with_mut(|li| {
        li.timestamp += 31 * 86400;
    });

    client.claim_refund(&id, &contributor1);

    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund on failed campaign"
    );
}

// Issue #341: claim_revenue is gated on funds_withdrawn. The prior "claim
// then cancel then refund" flow this test exercised is now structurally
// impossible (cancel is blocked once funds are withdrawn). Covered by
// test::test_claim_revenue_blocked_before_funds_withdrawn.

#[test]
fn test_claim_revenue_amount_raised_zero_guard() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Zero Raised Guard"),
        String::from_str(&env, "Directly test AmountRaisedIsZero guard"),
        1_000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Artificially zero out amount_raised and effective_amount_raised while keeping
    // the contribution in storage. Also force funds_withdrawn=true so we bypass the
    // funds_withdrawn guard and exercise the AmountRaisedIsZero guard specifically.
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.amount_raised = 0;
        campaign.effective_amount_raised = 0;
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::AmountRaisedIsZero);
}

#[test]
fn test_claim_refund_clears_lifetime_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "LT Cleanup"),
        String::from_str(&env, "Test lifetime cleanup"),
        2000,
        1,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &900);

    // Cancel and refund
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);

    // LifetimeContribution should be cleared after full refund
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        0,
        "LifetimeContribution should be 0 after full refund"
    );
}
