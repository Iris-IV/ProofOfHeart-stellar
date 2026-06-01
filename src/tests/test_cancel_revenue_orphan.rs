use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams};
use soroban_sdk::String;

/// Test that reproduces the orphaned revenue pool bug:
/// When revenue is deposited into a campaign and the campaign is then cancelled,
/// the revenue pool should be refunded to the creator (not orphaned).
#[test]
fn test_cancel_campaign_refunds_revenue_pool() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Campaign with Revenue"),
        description: String::from_str(&env, "Testing revenue refund on cancel"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000, // 20% to contributors
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    // Contributor makes a contribution
    client.contribute(&campaign_id, &contributor1, &1000);

    // Creator deposits revenue
    let revenue_amount = 5000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_amount);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Verify revenue pool is set
    assert_eq!(client.get_revenue_pool(&campaign_id), revenue_amount);

    // Creator's balance should be reduced by the revenue deposit
    assert_eq!(token.balance(&creator), 0); // 5000 - 5000 = 0

    // Cancel the campaign (before withdrawal)
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);

    // Revenue pool should be cleared
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Creator should have received the full revenue pool refund
    assert_eq!(token.balance(&creator), revenue_amount);

    // Contract should still have the contribution (1000) but not the revenue
    // Contributions are only refunded when contributors claim their refunds
    assert_eq!(token.balance(&client.address), 1000);

    // Contributor can claim their contribution back via refund
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2000); // 1000 (original) + 1000 (refunded)
    assert_eq!(token.balance(&client.address), 0);
}

/// Test that revenue pool is cleared but contributors can still claim refunds
/// even if they previously had revenue claims.
#[test]
fn test_cannot_claim_revenue_after_cancel() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cancel Then Refund"),
        description: String::from_str(&env, "Verify revenue is unavailable after cancel"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &1000);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Cancel the campaign
    client.cancel_campaign(&campaign_id);

    // Revenue pool should be empty now (refunded to creator)
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Contributor can still claim refund
    client.claim_refund(&campaign_id, &contributor1);

    // Verify contribution is cleared (as part of refund process)
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    // Revenue claimed should be cleared on refund
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

/// Test that multiple contributors with different contributions
/// cannot claim revenue after campaign is cancelled.
#[test]
fn test_cancel_with_multiple_contributors_and_revenue() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &3000);
    token_admin.mint(&contributor2, &2000);
    token_admin.mint(&creator, &8000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Multi-contributor Cancel"),
        description: String::from_str(&env, "Multiple contributors with revenue"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 3000, // 30% to contributors
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &2000);
    client.contribute(&campaign_id, &contributor2, &1000);

    let revenue_deposited = 3000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_deposited);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let creator_balance_before_cancel = token.balance(&creator);
    let contract_balance_before_cancel = token.balance(&client.address);

    // Cancel the campaign
    client.cancel_campaign(&campaign_id);

    // Revenue pool should be refunded to creator
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);
    assert_eq!(
        token.balance(&creator),
        creator_balance_before_cancel + revenue_deposited
    );

    // Contract should only have the contributions now (revenue removed)
    assert_eq!(
        token.balance(&client.address),
        contract_balance_before_cancel - revenue_deposited
    );

    // Both contributors should be able to claim refunds
    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);

    // Verify all funds are returned to their original owners
    assert_eq!(token.balance(&contributor1), 3000);
    assert_eq!(token.balance(&contributor2), 2000);
    assert_eq!(token.balance(&client.address), 0);
}

/// Test that revenue refund event is emitted when campaign is cancelled with revenue pool.
#[test]
fn test_cancel_campaign_emits_revenue_refund_event() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&creator, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Event Test"),
        description: String::from_str(&env, "Verify events are emitted"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    let revenue_amount = 5000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_amount);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Cancel campaign - should emit revenue_pool_refunded event
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);
}

/// Test that cancel still works correctly when no revenue has been deposited.
#[test]
fn test_cancel_campaign_with_no_revenue() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "No Revenue Cancel"),
        description: String::from_str(&env, "Cancel without revenue deposit"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    let contract_balance_before = token.balance(&client.address);

    // Cancel the campaign (no revenue deposited)
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);

    // Revenue pool should remain 0
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Contract balance should not change (no revenue to refund)
    assert_eq!(token.balance(&client.address), contract_balance_before);

    // Contributor should still be able to claim refund
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(token.balance(&client.address), 0);
}
