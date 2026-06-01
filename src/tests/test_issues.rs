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

// ── #353 pause checks ──
#[test]
fn test_paused_admin_parameter_setting_functions_fail() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_campaign_params_simple(&env, &creator));

    client.pause();

    let result_fee = client.try_set_campaign_fee_override(&admin, &campaign_id, &100u32);
    assert_eq!(result_fee.unwrap_err().unwrap(), Error::ContractPaused);

    let result_disabled = client.try_set_creation_disabled(&true);
    assert_eq!(result_disabled.unwrap_err().unwrap(), Error::ContractPaused);
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
