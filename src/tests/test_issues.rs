use super::helpers::*;
use crate::{Campaign, DataKey, Error, MaybePendingCreator};
use soroban_sdk::{Address, Env, String};

// ── #266 migrate ──────────────────────────────────────────────────────────────

#[test]
fn test_migrate_success() {
    let (_env, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_migrate(&admin, &1u32);
    assert!(result.is_ok());
    assert_eq!(client.get_version(), 1u32);
}

#[test]
fn test_migrate_wrong_version_fails() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_migrate(&admin, &99u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_migrate_double_run_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::Version, &0u32);
    });
    client.migrate(&admin, &0u32);
    let result = client.try_migrate(&admin, &0u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_migrate_non_admin_fails() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let stranger = Address::generate(&env);
    let result = client.try_migrate(&stranger, &1u32);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #267 two-step token update ────────────────────────────────────────────────

fn setup_second_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract(admin.clone())
}

#[test]
fn test_propose_token_update_stores_pending() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    assert_ne!(client.get_token(), new_token);
}

#[test]
fn test_accept_token_update_before_delay_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_accept_token_update_after_delay_succeeds() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);

    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    client.accept_token_update(&admin);
    assert_eq!(client.get_token(), new_token);
}

#[test]
fn test_cancel_token_update_clears_pending() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    client.propose_token_update(&admin, &new_token);
    client.cancel_token_update(&admin);

    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_cancel_token_update_no_pending_fails() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_cancel_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_propose_token_update_non_admin_fails() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);
    let stranger = Address::generate(&env);
    let result = client.try_propose_token_update(&stranger, &new_token);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #268 O(1) platform stats ──────────────────────────────────────────────────

fn make_campaign_params_simple(env: &Env, creator: &Address) -> CreateCampaignParams {
    CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "T"),
        description: String::from_str(env, "D"),
        funding_goal: 1,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    }
}

#[test]
fn test_platform_stats_after_create() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator));
    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 0);
    assert_eq!(stats.verified_campaigns, 0);
}

#[test]
fn test_platform_stats_after_cancel() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.cancel_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.cancelled_campaigns, 1);
}

#[test]
fn test_platform_stats_after_verify() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.verify_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.verified_campaigns, 1);
    assert_eq!(stats.active_campaigns, 1);
}

#[test]
fn test_platform_stats_after_withdraw() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    client.verify_campaign(&id);

    token_admin.mint(&contributor, &1000);
    client.contribute(&id, &contributor, &1);

    env.ledger().with_mut(|l| {
        l.timestamp += 31 * 86400;
    });

    client.withdraw_funds(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
}

// ── #269 category list limit cap ─────────────────────────────────────────────

#[test]
fn test_get_campaigns_by_category_capped_at_list_max_limit() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    for _ in 0..60 {
        client.create_campaign(&make_campaign_params_simple(&env, &creator));
    }

    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &1000u32);
    assert!(result.len() <= 50);
    assert_eq!(result.len(), 50);
}

#[test]
fn test_get_campaigns_by_category_small_limit_respected() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    for _ in 0..10 {
        client.create_campaign(&make_campaign_params_simple(&env, &creator));
    }
    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &5u32);
    assert_eq!(result.len(), 5);
}

// ── #348 resume_campaign spurious events/state writes ─────────────────────────

#[test]
fn test_resume_campaign_rejects_when_contract_not_paused() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    let events_before = env.events().all().len();
    let result = client.try_resume_campaign(&campaign_id, &creator);
    let events_after = env.events().all().len();

    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(events_before, events_after);
}

#[test]
fn test_resume_campaign_clears_auto_pause_when_active() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::AutoPaused, &true);
    });

    assert!(client.is_paused());
    client.resume_campaign(&campaign_id, &creator);
    assert!(!client.is_paused());
}

// ── #353 / #388 pause checks ──
// Updated for #388: admin governance functions must succeed even while paused so the
// admin can adjust parameters and recover ownership during an emergency pause.
#[test]
fn test_paused_admin_parameter_setting_functions_succeed() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    client.pause();

    let result_fee = client.try_set_campaign_fee_override(&admin, &campaign_id, &100u32);
    assert!(
        result_fee.is_ok(),
        "set_campaign_fee_override must succeed while paused"
    );

    let result_disabled = client.try_set_creation_disabled(&true);
    assert!(
        result_disabled.is_ok(),
        "set_creation_disabled must succeed while paused"
    );
    let _ = campaign_id;
}

// ── #355 set_personal_cap limits check ──
#[test]
fn test_set_personal_cap_cannot_exceed_max_contribution_per_user() {
    let (env, _, creator, contributor, _, _, _, client) = setup_env();

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "T"),
        description: String::from_str(&env, "D"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 500,
    };
    let campaign_id = client.create_campaign(&params);

    let res1 = client.try_set_personal_cap(&campaign_id, &contributor, &500);
    assert!(res1.is_ok());

    let res2 = client.try_set_personal_cap(&campaign_id, &contributor, &501);
    assert_eq!(res2.unwrap_err().unwrap(), Error::ValidationFailed);
}

// ── #354 vote weight checked addition ──
#[test]
fn test_vote_weight_overflow_fails() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    token_admin.mint(&contributor, &1000);

    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::ApproveWeight(campaign_id), &(i128::MAX - 500));
    });

    token_admin.mint(&contributor, &501);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::Overflow);
}

// ── #360 resume_campaign admin-path coverage ──────────────────────────────────

fn set_auto_paused(env: &Env, client_address: &Address, paused: bool) {
    env.as_contract(client_address, || {
        env.storage().instance().set(&DataKey::AutoPaused, &paused);
    });
}

#[test]
fn test_resume_by_admin() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    set_auto_paused(&env, &client.address, true);
    assert!(client.is_paused());

    client.resume_campaign(&campaign_id, &admin);
    assert!(!client.is_paused());
}

#[test]
fn test_resume_unauthorized_fails() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));
    let stranger = Address::generate(&env);

    set_auto_paused(&env, &client.address, true);

    let result = client.try_resume_campaign(&campaign_id, &stranger);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_resume_after_campaign_transfer_uses_new_creator() {
    let (env, _admin, original_creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &original_creator));

    let new_creator = Address::generate(&env);
    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    client.accept_campaign_transfer(&campaign_id);

    set_auto_paused(&env, &client.address, true);
    assert!(client.is_paused());

    client.resume_campaign(&campaign_id, &new_creator);
    assert!(!client.is_paused());

    set_auto_paused(&env, &client.address, true);
    let result = client.try_resume_campaign(&campaign_id, &original_creator);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #409/#410 MaybePendingCreator round-trip (binary compat) ─────────────

#[test]
fn test_pending_creator_none_round_trip() {
    let env = Env::default();
    let contract_id = Address::generate(&env);
    env.register_contract(&contract_id, crate::ProofOfHeart);
    let addr = Address::generate(&env);
    let campaign = Campaign {
        id: 1,
        creator: addr.clone(),
        first_creator: addr,
        pending_creator: MaybePendingCreator::None,
        title: String::from_str(&env, "test"),
        description: String::from_str(&env, "desc"),
        funding_goal: 1000,
        deadline: 1000000,
        amount_raised: 0,
        is_active: true,
        funds_withdrawn: false,
        is_cancelled: false,
        is_verified: false,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
        fee_override: None,
        deadline_extended: false,
        effective_amount_raised: 0,
    };

    env.as_contract(&contract_id, || {
        env.storage().instance().extend_ttl(100, 100);
        env.storage()
            .instance()
            .set(&DataKey::Campaign(1), &campaign);
        let read: Campaign = env.storage().instance().get(&DataKey::Campaign(1)).unwrap();
        assert!(read.pending_creator.is_none());
    });
}

#[test]
fn test_pending_creator_some_round_trip() {
    let env = Env::default();
    let contract_id = Address::generate(&env);
    let _ = env.register_contract(&contract_id, crate::ProofOfHeart);
    let addr = Address::generate(&env);
    let pending = Address::generate(&env);
    let campaign = Campaign {
        id: 1,
        creator: addr.clone(),
        first_creator: addr,
        pending_creator: MaybePendingCreator::Some(pending.clone()),
        title: String::from_str(&env, "test"),
        description: String::from_str(&env, "desc"),
        funding_goal: 1000,
        deadline: 1000000,
        amount_raised: 0,
        is_active: true,
        funds_withdrawn: false,
        is_cancelled: false,
        is_verified: false,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
        fee_override: None,
        deadline_extended: false,
        effective_amount_raised: 0,
    };

    env.as_contract(&contract_id, || {
        env.storage().instance().extend_ttl(100, 100);
        env.storage()
            .instance()
            .set(&DataKey::Campaign(1), &campaign);
        let read: Campaign = env.storage().instance().get(&DataKey::Campaign(1)).unwrap();
        assert_eq!(read.pending_creator, MaybePendingCreator::Some(pending));
    });
}

// ── #388 admin governance unblocked during pause ──────────────────────────────

fn pause_contract(client: &ProofOfHeartClient) {
    client.pause();
    assert!(client.is_paused());
}

/// Issue #388 — admin can update the platform fee while the contract is paused.
#[test]
fn test_update_platform_fee_while_paused() {
    let (_, _admin, _, _, _, _, _, client) = setup_env();
    pause_contract(&client);
    let result = client.try_update_platform_fee(&100u32);
    assert!(
        result.is_ok(),
        "admin must be able to update fee while paused"
    );
    assert_eq!(client.get_platform_fee(), 100);
}

/// Issue #388 — admin can initiate an ownership transfer while the contract is paused
/// (critical recovery path: compromised key → transfer to safe address while paused).
#[test]
fn test_initiate_admin_transfer_while_paused() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    pause_contract(&client);
    let new_admin = Address::generate(&env);
    let result = client.try_initiate_admin_transfer(&admin, &new_admin);
    assert!(
        result.is_ok(),
        "admin transfer must be initiable while paused"
    );
    assert_eq!(client.get_pending_admin(), Some(new_admin));
}

/// Issue #388 — pending admin can accept the transfer while paused.
#[test]
fn test_accept_admin_transfer_while_paused() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_admin = Address::generate(&env);
    client.initiate_admin_transfer(&admin, &new_admin);
    pause_contract(&client);
    let result = client.try_accept_admin_transfer();
    assert!(
        result.is_ok(),
        "pending admin must be able to accept while paused"
    );
    assert_eq!(client.get_admin(), new_admin);
}

/// Issue #388 — admin can cancel a pending admin transfer while paused.
#[test]
fn test_cancel_admin_transfer_while_paused() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_admin = Address::generate(&env);
    client.initiate_admin_transfer(&admin, &new_admin);
    pause_contract(&client);
    let result = client.try_cancel_admin_transfer(&admin);
    assert!(
        result.is_ok(),
        "admin must be able to cancel transfer while paused"
    );
    assert_eq!(client.get_pending_admin(), None);
}

/// Issue #388 — admin can adjust voting parameters while paused.
#[test]
fn test_set_voting_params_while_paused() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    pause_contract(&client);
    let result = client.try_set_voting_params(&admin, &5u32, &6000u32);
    assert!(
        result.is_ok(),
        "admin must be able to set voting params while paused"
    );
}

// ── #411 get_platform_stats O(1) counter reads ────────────────────────────────

/// Issue #411 — stats counters match actual campaign lifecycle transitions.
#[test]
fn test_platform_stats_counters_track_lifecycle() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    // Baseline: no campaigns yet.
    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 0);
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.cancelled_campaigns, 0);
    assert_eq!(stats.verified_campaigns, 0);
    assert!(!stats.stats_are_partial);

    // Create two campaigns.
    let p1 = make_campaign_params_simple(&env, &creator);
    let p2 = make_campaign_params_simple(&env, &creator);
    let id1 = client.create_campaign(&p1);
    let id2 = client.create_campaign(&p2);

    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 2);
    assert_eq!(stats.active_campaigns, 2);

    // Cancel one — active count drops, cancelled count rises.
    client.cancel_campaign(&id1);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 1);

    // Verify the remaining active campaign.
    client.verify_campaign(&id2);
    let stats = client.get_platform_stats();
    assert_eq!(stats.verified_campaigns, 1);

    // stats_are_partial must always be false after the O(1) refactor.
    assert!(!stats.stats_are_partial);
    assert_eq!(stats.scanned_up_to, stats.total_campaigns);
    let _ = (id1, id2, admin);
}

/// Issue #411 — stats_are_partial is always false regardless of campaign count.
#[test]
fn test_platform_stats_never_partial() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    for _ in 0..5 {
        client.create_campaign(&make_campaign_params_simple(&env, &creator));
    }

    let stats = client.get_platform_stats();
    assert!(!stats.stats_are_partial);
    assert_eq!(stats.active_campaigns, 5);
}

// ── #386 creator-claim precision bias ─────────────────────────────────────────

/// Issue #386 — creator claim must not take the contributor-side truncation dust.
#[test]
fn test_creator_claim_does_not_absorb_contributor_rounding() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &20_000);
    token_admin.mint(&creator, &20_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Issue 386"),
        description: String::from_str(&env, "Creator claim precision regression"),
        funding_goal: 10_001,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 5000, // 50%
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &10_001);
    client.withdraw_funds(&campaign_id);

    client.deposit_revenue(&campaign_id, &10_001);

    let creator_before = token.balance(&creator);
    client.claim_creator_revenue(&campaign_id);
    let creator_after = token.balance(&creator);

    // Previous residual math paid 5001 here; direct creator-side math must pay 5000.
    assert_eq!(creator_after - creator_before, 5_000);
}
