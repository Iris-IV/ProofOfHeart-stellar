extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::{
    storage, AdminKey, Campaign, CampaignKey, Category, CreateCampaignParams, Error,
    MaybePendingCreator, VotingKey, SECONDS_PER_DAY, TOKEN_UPDATE_DELAY_SECS,
};
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
        env.storage().instance().set(&AdminKey::Version, &0u32);
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
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
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

fn make_campaign_params_simple(env: &Env, creator: &Address, seq: u32) -> CreateCampaignParams {
    extern crate std;
    CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, &std::format!("T{}", seq)),
        description: String::from_str(env, "D"),
        funding_goal: 1,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    }
}

/// Like `make_campaign_params_simple` but with a caller-chosen title, for
/// tests that create several campaigns under one creator (title uniqueness
/// is enforced since the campaign-integrity hardening).
fn make_campaign_params_titled(env: &Env, creator: &Address, title: &str) -> CreateCampaignParams {
    CreateCampaignParams {
        title: String::from_str(env, title),
        ..make_campaign_params_simple(env, creator)
    }
}

#[test]
fn test_platform_stats_after_create() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    let stats = client.get_platform_stats();
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 0);
    assert_eq!(stats.verified_campaigns, 0);
}

#[test]
fn test_platform_stats_after_cancel() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    client.cancel_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
    assert_eq!(stats.cancelled_campaigns, 1);
}

#[test]
fn test_platform_stats_after_verify() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    client.verify_campaign(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.verified_campaigns, 1);
    assert_eq!(stats.active_campaigns, 1);
}

#[test]
fn test_platform_stats_after_withdraw() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    client.verify_campaign(&id);

    token_admin.mint(&contributor, &100_000);
    client.contribute(&id, &contributor, &10_000);

    env.ledger().with_mut(|l| {
        l.timestamp += 31 * SECONDS_PER_DAY;
    });

    client.withdraw_funds(&id);
    let stats = client.get_platform_stats();
    assert_eq!(stats.active_campaigns, 0);
}

// ── #269 category list limit cap ─────────────────────────────────────────────

#[test]
fn test_get_campaigns_by_category_capped_at_list_max_limit() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    env.budget().reset_unlimited();

    let campaign = crate::types::Campaign {
        id: 1,
        creator: creator.clone(),
        first_creator: creator.clone(),
        pending_creator: crate::types::MaybePendingCreator::None,
        title: String::from_str(&env, "T"),
        description: String::from_str(&env, "D"),
        funding_goal: 1,
        deadline: 100000,
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
    env.as_contract(&client.address, || {
        let mut bucket = soroban_sdk::Vec::new(&env);
        for i in 1..=60 {
            let mut c = campaign.clone();
            c.id = i;
            crate::storage::set_campaign(&env, i, &c);
            bucket.push_back(i);
        }
        crate::storage::set_category_campaign_count(&env, Category::Learner, 60);
        crate::storage::set_category_campaign_bucket(&env, Category::Learner, 0, &bucket);
    });

    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &1000u32);
    assert_eq!(result.len(), 20);
}

#[test]
fn test_get_campaigns_by_category_small_limit_respected() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    for i in 0..10 {
        let title_str = format!("T{}", i);
        let mut p = make_campaign_params_simple(&env, &creator);
        p.title = String::from_str(&env, &title_str);
        client.create_campaign(&p);
    }
    let result = client.get_campaigns_by_category(&Category::Learner, &0u32, &5u32);
    assert_eq!(result.0.len(), 5);
}

// ── #348 resume_campaign spurious events/state writes ─────────────────────────

#[test]
fn test_resume_campaign_rejects_when_contract_not_paused() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    let events_before = env.events().all().len();
    let result = client.try_resume_campaign(&campaign_id, &creator);
    let events_after = env.events().all().len();

    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(events_before, events_after);
}

#[test]
fn test_resume_campaign_clears_auto_pause_when_active() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    env.as_contract(&client.address, || {
        env.storage().instance().set(&AdminKey::AutoPaused, &true);
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
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    client.pause();

    let result_fee = client.try_set_campaign_fee_override(&campaign_id, &admin, &100u32);
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

// ── #441 set_personal_cap below lifetime_contribution rejected ──
#[test]
fn test_set_personal_cap_below_lifetime_contribution_rejected() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "T"),
        description: String::from_str(&env, "D"),
        funding_goal: 10_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    token_admin.mint(&contributor, &10_000);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &1000);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        1000
    );

    let res = client.try_set_personal_cap(&campaign_id, &contributor, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    let res = client.try_set_personal_cap(&campaign_id, &contributor, &1000);
    assert!(res.is_ok());

    let res = client.try_set_personal_cap(&campaign_id, &contributor, &2000);
    assert!(res.is_ok());
}

// ── #441 boundary: cap == lifetime blocks future contributions ──
#[test]
fn test_set_personal_cap_equal_lifetime_blocks_further_contributions() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "T"),
        description: String::from_str(&env, "D"),
        funding_goal: 10_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    token_admin.mint(&contributor, &10_000);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &1000);

    let res = client.try_set_personal_cap(&campaign_id, &contributor, &1000);
    assert!(res.is_ok());

    let res = client.try_contribute(&campaign_id, &contributor, &1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
}

// ── #354 vote weight checked addition ──
#[test]
fn test_vote_weight_does_not_overflow_with_1_address_1_vote() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    token_admin.mint(&contributor, &1000);

    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&VotingKey::ApproveWeight(campaign_id), &(i128::MAX - 500));
    });

    token_admin.mint(&contributor, &501);

    // With 1-address-1-vote, the weight only increments by 1, so no overflow occurs
    // even when ApproveWeight is near i128::MAX (#469).
    client.vote_on_campaign(&campaign_id, &contributor, &true);
    assert_eq!(client.get_approve_votes(&campaign_id), 1);
}

// ── #360 resume_campaign admin-path coverage ──────────────────────────────────

fn set_auto_paused(env: &Env, client_address: &Address, paused: bool) {
    env.as_contract(client_address, || {
        env.storage().instance().set(&AdminKey::AutoPaused, &paused);
    });
}

#[test]
fn test_resume_by_admin() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    set_auto_paused(&env, &client.address, true);
    assert!(client.is_paused());

    client.resume_campaign(&campaign_id, &admin);
    assert!(!client.is_paused());
}

#[test]
fn test_resume_unauthorized_fails() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    let stranger = Address::generate(&env);

    set_auto_paused(&env, &client.address, true);

    let result = client.try_resume_campaign(&campaign_id, &stranger);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_resume_after_campaign_transfer_uses_new_creator() {
    let (env, _admin, original_creator, _, _, _, _, client) = setup_env();
    let campaign_id =
        client.create_campaign(&make_campaign_params_simple(&env, &original_creator, 0));

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
        pending_creator_expiry: 0,
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
            .set(&CampaignKey::Campaign(1), &campaign);
        let read: Campaign = env
            .storage()
            .instance()
            .get(&CampaignKey::Campaign(1))
            .unwrap();
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
        pending_creator_expiry: 1_000_000,
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
            .set(&CampaignKey::Campaign(1), &campaign);
        let read: Campaign = env
            .storage()
            .instance()
            .get(&CampaignKey::Campaign(1))
            .unwrap();
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
    let mut p2 = make_campaign_params_simple(&env, &creator);
    p2.title = String::from_str(&env, "T2");
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
    env.budget().reset_unlimited();

    for i in 0..5 {
        let title_str = format!("T{}", i);
        let mut p = make_campaign_params_simple(&env, &creator);
        p.title = String::from_str(&env, &title_str);
        client.create_campaign(&p);
    }

    let stats = client.get_platform_stats();
    assert!(!stats.stats_are_partial);
    assert_eq!(stats.active_campaigns, 5);
}

// ── Counter consistency invariants (partial migrations / failed writes) ───────

/// The active/verified/cancelled counters and `total_campaigns` are separate
/// instance-storage keys. A partial migration or failed legacy write can leave
/// them mutually inconsistent; `get_platform_stats` must flag that instead of
/// exposing impossible totals with `stats_are_partial = false`.
#[test]
fn test_platform_stats_flags_active_counter_exceeding_total() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator));

    // Simulate a partial migration: the active counter was written but the
    // campaign-count key was rolled back / never written.
    env.as_contract(&client.address, || {
        crate::storage::set_active_campaign_count(&env, 5);
    });

    let stats = client.get_platform_stats();

    // The impossible aggregate is flagged rather than silently trusted.
    assert!(stats.stats_are_partial);
    // Raw stored values are surfaced for auditability.
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.active_campaigns, 5);
    // `scanned_up_to` remains the authoritative pagination bound.
    assert_eq!(stats.scanned_up_to, 1);

    // An audit event is published so indexers/admin can notice the corruption.
    let events = env.events().all();
    let last_event = events.last().unwrap();
    let expected_topics = (String::from_str(&env, "platform_stats_inconsistent"),).into_val(&env);
    assert_eq!(last_event.1, expected_topics);
    let data: soroban_sdk::Vec<u32> = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data.get(0).unwrap(), 1); // total_campaigns
    assert_eq!(data.get(1).unwrap(), 5); // active_campaigns
    assert_eq!(data.get(2).unwrap(), 0); // verified_campaigns
    assert_eq!(data.get(3).unwrap(), 0); // cancelled_campaigns
}

#[test]
fn test_platform_stats_flags_cancelled_counter_exceeding_total() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator));

    env.as_contract(&client.address, || {
        crate::storage::set_cancelled_campaign_count(&env, 7);
    });

    let stats = client.get_platform_stats();
    assert!(stats.stats_are_partial);
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 7);
}

#[test]
fn test_platform_stats_flags_verified_counter_exceeding_total() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    client.create_campaign(&make_campaign_params_simple(&env, &creator));

    env.as_contract(&client.address, || {
        crate::storage::set_verified_campaign_count(&env, 3);
    });

    let stats = client.get_platform_stats();
    assert!(stats.stats_are_partial);
    assert_eq!(stats.total_campaigns, 1);
    assert_eq!(stats.verified_campaigns, 3);
}

/// Each counter can individually be ≤ total while their combination is still
/// impossible: active + cancelled ≤ total must hold too (a campaign can never
/// be counted in both buckets).
#[test]
fn test_platform_stats_flags_active_plus_cancelled_exceeding_total() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let _ = client.create_campaign(&make_campaign_params_titled(&env, &creator, "S1"));
    let _ = client.create_campaign(&make_campaign_params_titled(&env, &creator, "S2"));

    // total = 2, active = 2, cancelled = 1 → 3 campaigns accounted for but
    // only 2 exist.
    env.as_contract(&client.address, || {
        crate::storage::set_active_campaign_count(&env, 2);
        crate::storage::set_cancelled_campaign_count(&env, 1);
    });

    let stats = client.get_platform_stats();
    assert!(stats.stats_are_partial);
    assert_eq!(stats.total_campaigns, 2);
    assert_eq!(stats.active_campaigns, 2);
    assert_eq!(stats.cancelled_campaigns, 1);
}

/// Healthy state must keep `stats_are_partial = false` even when the counts
/// are non-zero and `active + cancelled` sits exactly at `total` — the
/// invariants must not false-positive at the boundary.
#[test]
fn test_platform_stats_consistent_through_full_lifecycle() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    let id1 = client.create_campaign(&make_campaign_params_titled(&env, &creator, "L1"));
    let id2 = client.create_campaign(&make_campaign_params_titled(&env, &creator, "L2"));
    client.verify_campaign(&id1);
    client.cancel_campaign(&id2);

    let stats = client.get_platform_stats();
    assert!(!stats.stats_are_partial);
    assert_eq!(stats.total_campaigns, 2);
    assert_eq!(stats.active_campaigns, 1);
    assert_eq!(stats.verified_campaigns, 1);
    assert_eq!(stats.cancelled_campaigns, 1);
    assert_eq!(stats.scanned_up_to, 2);

    // No inconsistency event may be published in the healthy path.
    let events = env.events().all();
    let expected_topics = (String::from_str(&env, "platform_stats_inconsistent"),).into_val(&env);
    for event in events.iter() {
        assert_ne!(event.1, expected_topics);
    }
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

// ── #526 last-claimant revenue dust ────────────────────────────────────────────

/// Issue #526 — per-contributor integer division truncates each individual
/// share, so the sum of every contributor's claim can fall short of the full
/// contributor-side pool. The last contributor to claim must absorb that
/// remainder instead of receiving their own individually-truncated share, so
/// no revenue is permanently stuck in the contract.
#[test]
fn test_last_revenue_claimant_absorbs_rounding_dust() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10);
    token_admin.mint(&contributor2, &10);
    token_admin.mint(&creator, &100);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Issue 526"),
        description: String::from_str(&env, "Revenue dust regression"),
        funding_goal: 3,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 5000, // 50%
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1);
    client.contribute(&campaign_id, &contributor2, &2);
    client.withdraw_funds(&campaign_id);
    client.deposit_revenue(&campaign_id, &10);

    // contributor_pool_total = 10 * 5000 / 10000 = 5.
    // Naive per-contributor math: contributor1 = 1*10*5000/3/10000 = 1,
    // contributor2 = 2*10*5000/3/10000 = 3 -> sum = 4, leaving 1 stuck.
    let before1 = token.balance(&contributor1);
    client.claim_revenue(&campaign_id, &contributor1);
    let claimed1 = token.balance(&contributor1) - before1;
    assert_eq!(claimed1, 1);

    let before2 = token.balance(&contributor2);
    client.claim_revenue(&campaign_id, &contributor2);
    let claimed2 = token.balance(&contributor2) - before2;

    // The last claimant (contributor2) must absorb the rounding dust, so the
    // two contributor claims sum to exactly the contributor-side pool (5),
    // not the naively-truncated 4.
    assert_eq!(claimed1 + claimed2, 5);
    assert_eq!(claimed2, 4);
}

// ── #528 corrupted campaign storage key ────────────────────────────────────────

/// Issue #528 — if a campaign's persistent storage entry can't be
/// deserialized into `Campaign`, `get_campaign` must return `None` (and
/// therefore callers like `get_campaign_or_error` must surface
/// `Error::CampaignNotFound`) instead of panicking / aborting the host.
#[test]
fn test_get_campaign_survives_corrupted_storage_entry() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    assert!(client.get_campaign(&campaign_id).id == campaign_id);

    // Corrupt the persistent entry backing this campaign by overwriting it
    // with a value of a totally different (incompatible) shape.
    env.as_contract(&client.address, || {
        let key = CampaignKey::Campaign(campaign_id);
        env.storage().persistent().set(&key, &"not a campaign");
    });

    let result = client.try_get_campaign(&campaign_id);
    assert_eq!(result.unwrap_err().unwrap(), Error::CampaignNotFound);
}

// ── #478 O(1) creator-ownership check ─────────────────────────────────────────

#[test]
fn test_is_campaign_creator_true_for_owner() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    assert!(client.is_campaign_creator(&campaign_id, &creator));
}

#[test]
fn test_is_campaign_creator_false_for_non_owner() {
    let (env, _, creator, contributor, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    assert!(!client.is_campaign_creator(&campaign_id, &contributor));
}

#[test]
fn test_is_campaign_creator_false_for_nonexistent_campaign() {
    let (_env, _, creator, _, _, _, _, client) = setup_env();
    assert!(!client.is_campaign_creator(&999, &creator));
}

#[test]
fn test_is_campaign_creator_updates_after_transfer() {
    let (env, _, creator, _, _, _, _, client) = setup_env();
    let receiver = Address::generate(&env);
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator, 0));

    client.initiate_campaign_transfer(&campaign_id, &receiver);
    client.accept_campaign_transfer(&campaign_id);

    assert!(!client.is_campaign_creator(&campaign_id, &creator));
    assert!(client.is_campaign_creator(&campaign_id, &receiver));
}

// ── #650 admin-configurable token update delay ────────────────────────────────

/// Issue #650 — with no override set, the effective delay is the compiled-in
/// default and proposing a token update still enforces it.
#[test]
fn test_token_update_delay_defaults_to_constant() {
    let (_, _admin, _, _, _, _, _, client) = setup_env();
    assert_eq!(
        client.get_token_update_delay_secs(),
        TOKEN_UPDATE_DELAY_SECS
    );
}

/// Issue #650 — the admin can shorten the timelock, and the new delay (not
/// the compiled-in default) is what `accept_token_update` enforces.
#[test]
fn test_set_token_update_delay_secs_shortens_timelock() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);

    let one_day = SECONDS_PER_DAY;
    client.set_token_update_delay_secs(&admin, &one_day);
    assert_eq!(client.get_token_update_delay_secs(), one_day);

    client.propose_token_update(&admin, &new_token);

    // Halfway through the shortened delay: still too early.
    env.ledger().with_mut(|l| {
        l.timestamp += one_day / 2;
    });
    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);

    // Past the shortened (1-day) delay, well before the old 7-day default.
    env.ledger().with_mut(|l| {
        l.timestamp += one_day;
    });
    client.accept_token_update(&admin);
    assert_eq!(client.get_token(), new_token);
}

/// Issue #650 — the admin can lengthen the timelock beyond the 7-day default.
#[test]
fn test_set_token_update_delay_secs_lengthens_timelock() {
    let (env, admin, _, _, _, _, _, client) = setup_env();
    let new_token = setup_second_token(&env, &admin);

    let thirty_days = 30 * SECONDS_PER_DAY;
    client.set_token_update_delay_secs(&admin, &thirty_days);
    client.propose_token_update(&admin, &new_token);

    // Past the old 7-day default, but not the new 30-day delay.
    env.ledger().with_mut(|l| {
        l.timestamp += TOKEN_UPDATE_DELAY_SECS + 1;
    });
    let result = client.try_accept_token_update(&admin);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);

    env.ledger().with_mut(|l| {
        l.timestamp += thirty_days;
    });
    client.accept_token_update(&admin);
    assert_eq!(client.get_token(), new_token);
}

#[test]
fn test_set_token_update_delay_secs_rejects_zero() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let result = client.try_set_token_update_delay_secs(&admin, &0u64);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_set_token_update_delay_secs_rejects_above_max() {
    let (_, admin, _, _, _, _, _, client) = setup_env();
    let too_long = 365 * SECONDS_PER_DAY + 1;
    let result = client.try_set_token_update_delay_secs(&admin, &too_long);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_set_token_update_delay_secs_non_admin_fails() {
    let (env, _admin, _, _, _, _, _, client) = setup_env();
    let stranger = Address::generate(&env);
    let result = client.try_set_token_update_delay_secs(&stranger, &SECONDS_PER_DAY);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
}

// ── #652 public views for constants.rs values ─────────────────────────────────

#[test]
fn test_get_bps_denominator_matches_constant() {
    let (_, _, _, _, _, _, _, client) = setup_env();
    assert_eq!(client.get_bps_denominator(), 10_000u32);
}

// ── #653 bookmark Error discriminant lock ─────────────────────────────────────

/// Issue #653 — `CampaignAlreadyBookmarked` and `CampaignNotBookmarked` are
/// the newest `Error` variants. Locks their exact discriminant values so a
/// careless future edit to the enum (e.g. inserting a variant above them)
/// fails this test instead of silently renumbering them, which would change
/// the on-the-wire error codes existing clients match against.
#[test]
fn test_bookmark_error_discriminants_are_locked() {
    assert_eq!(Error::CampaignAlreadyBookmarked as u32, 44);
    assert_eq!(Error::CampaignNotBookmarked as u32, 45);
}

// ── #475 list_active_campaigns scan window ────────────────────────────────────

#[test]
fn test_list_active_campaigns_reaches_campaigns_beyond_old_200_scan_window() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    // Reduced from 40 to 20 campaigns to avoid Soroban testutils stack overflow.
    // Cancel the first 15, leaving 5 active campaigns at the tail.
    let mut last_id = 0u32;
    for i in 0..20 {
        let title_str = format!("T{}", i);
        let mut p = make_campaign_params_simple(&env, &creator);
        p.title = String::from_str(&env, &title_str);
        last_id = client.create_campaign(&p);
    }
    for id in 1..=15 {
        client.cancel_campaign(&id);
    }

    let (active, next_cursor) = client.list_active_campaigns(&0, &50);
    assert_eq!(active.len(), 5);
    assert_eq!(active.get(0).unwrap().id, 16);
    assert_eq!(active.get(4).unwrap().id, last_id);
    assert_eq!(next_cursor, 0);
}

// ── #815 campaign_count checked_add overflow guard ────────────────────────────

/// Regression: `create_campaign` used `count += 1` (unchecked). At u32::MAX
/// the increment wraps to 0, assigning `campaign_id = 0` and overwriting the
/// storage slot of the very first campaign. The fix uses `checked_add(1)` and
/// returns `Error::Overflow` instead of corrupting state.
#[test]
fn test_create_campaign_at_u32_max_returns_overflow() {
    let (env, _, creator, _, _, _, _, client) = setup_env();

    // Forge the campaign counter to u32::MAX so the next create would wrap.
    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&CampaignKey::CampaignCount, &u32::MAX);
    });

    let result = client.try_create_campaign(&make_campaign_params_simple(&env, &creator, 0));
    assert_eq!(result.unwrap_err().unwrap(), Error::Overflow);
}

// ── #811 claim_refund double-claim guard (CEI ordering) ───────────────────────

/// Regression guard: `claim_refund` must zero the storage slot before the
/// token transfer (Checks–Effects–Interactions). A second invocation with the
/// same contributor must see 0 and return `NoFundsToWithdraw`, not transfer
/// again.
#[test]
fn test_claim_refund_double_claim_rejected() {
    let (env, _, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let mut params = make_campaign_params_simple(&env, &creator);
    params.funding_goal = 1_000;
    let campaign_id = client.create_campaign(&params);
    client.verify_campaign(&campaign_id);

    // Fund contributor and make a contribution.
    token_admin.mint(&contributor1, &500);
    client.contribute(&campaign_id, &contributor1, &500);

    // Let deadline pass without reaching goal so a refund is valid.
    env.ledger().with_mut(|l| {
        l.timestamp += 31 * SECONDS_PER_DAY;
    });

    // First refund must succeed.
    let result = client.try_claim_refund(&campaign_id, &contributor1);
    assert!(result.is_ok());

    // Second refund on the same (now-zeroed) slot must fail.
    let result2 = client.try_claim_refund(&campaign_id, &contributor1);
    assert_eq!(result2.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

// ── #855 withdraw_funds unchecked day-to-second multiplication ────────────────

/// Issue #855 — `delay_days * SECONDS_PER_DAY` in `withdraw_funds` was
/// unchecked. An extreme per-campaign vesting delay (e.g. via migration or
/// storage corruption) could overflow the multiplication, panicking and
/// permanently locking the reserve. The fix uses `checked_mul` so the
/// operation returns `Error::Overflow` instead.
#[test]
fn test_withdraw_funds_overflow_day_to_seconds_returns_error() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Overflow Vesting"),
        description: String::from_str(&env, "Test day-to-second overflow"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    // Forge the per-campaign vesting delay to an extreme value that would
    // overflow when multiplied by SECONDS_PER_DAY (86_400).
    let huge_delay = u64::MAX / 2;
    env.as_contract(&client.address, || {
        storage::set_campaign_vesting(&env, campaign_id, huge_delay, 1000);
    });

    env.ledger().with_mut(|l| {
        l.timestamp += 31 * SECONDS_PER_DAY;
    });

    // Must return Overflow, not panic.
    let result = client.try_withdraw_funds(&campaign_id);
    assert_eq!(result.unwrap_err().unwrap(), Error::Overflow);
}
