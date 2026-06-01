#![no_std]
#![allow(unexpected_cfgs)]

/// Current contract version. Increment this on each breaking upgrade.
const CONTRACT_VERSION: u32 = 1;

// Validation limit constants
pub(crate) const CAMPAIGN_TITLE_MIN_LEN: u32 = 1;
pub(crate) const CAMPAIGN_TITLE_MAX_LEN: u32 = 100;
pub(crate) const CAMPAIGN_DESCRIPTION_MIN_LEN: u32 = 1;
pub(crate) const CAMPAIGN_DESCRIPTION_MAX_LEN: u32 = 1000;
pub(crate) const CAMPAIGN_DURATION_MIN_DAYS: u64 = 1;
pub(crate) const CAMPAIGN_DURATION_MAX_DAYS: u64 = 365;
pub(crate) const CAMPAIGN_FUNDING_GOAL_MIN: i128 = 100_000;
pub(crate) const CAMPAIGN_FUNDING_GOAL_MAX: i128 = 1_000_000_000_000_000; // 10^15
pub(crate) const PLATFORM_FEE_MAX_BPS: u32 = 1000; // 10%
pub(crate) const REVENUE_SHARE_MAX_BPS: u32 = 5000; // 50%
pub(crate) const AUTO_PAUSE_SINGLE_CONTRIBUTION_BPS_THRESHOLD: i128 = 20000;
pub(crate) const AUTO_PAUSE_BURST_THRESHOLD: u32 = 10;
pub(crate) const LIST_MAX_LIMIT: u32 = 50;

mod admin;
mod campaigns;
mod contributions;
mod errors;
mod lifecycle;
mod queries;
mod revenue;
mod storage;
mod types;
mod voting;

pub use errors::Error;
use soroban_sdk::{contract, contractimpl, Address, Env, String};
pub use storage::DataKey;
use storage::*;
pub use types::*;

// Re-export lifecycle helpers so voting.rs can continue using `crate::` paths.
pub(crate) use lifecycle::{
    assert_admin, get_campaign_or_error, require_active_campaign, require_unverified_campaign,
};

#[contract]
pub struct ProofOfHeart;

#[contractimpl]
impl ProofOfHeart {
    // ── Initialisation ────────────────────────────────────────────────────────

    pub fn init(env: Env, admin: Address, token: Address, platform_fee: u32) -> Result<(), Error> {
        admin::init(&env, admin, token, platform_fee)
    }

    // ── Campaign creation ─────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn create_campaign(env: Env, params: CreateCampaignParams) -> Result<u32, Error> {
        let id = campaigns::create::create_campaign(&env, params)?;
        increment_active_campaign_count(&env);
        Ok(id)
    }

    // ── Contributions ─────────────────────────────────────────────────────────

    pub fn contribute(
        env: Env,
        campaign_id: u32,
        contributor: Address,
        amount: i128,
    ) -> Result<(), Error> {
        contributions::contribute(&env, campaign_id, contributor, amount)
    }

    pub fn claim_refund(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        contributions::claim_refund(&env, campaign_id, contributor)
    }

    // ── Withdrawals ───────────────────────────────────────────────────────────

    pub fn withdraw_funds(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::withdraw::withdraw_funds(&env, campaign_id)
    }

    pub fn withdraw_reserve(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::withdraw::withdraw_reserve(&env, campaign_id)
    }

    pub fn set_vesting_params(
        env: Env,
        admin: Address,
        delay_days: u64,
        reserve_bps: u32,
    ) -> Result<(), Error> {
        campaigns::withdraw::set_vesting_params(&env, admin, delay_days, reserve_bps)
    }

    // ── Campaign lifecycle ────────────────────────────────────────────────────

    pub fn cancel_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::cancel::cancel_campaign(&env, campaign_id)
    }

    pub fn update_campaign(
        env: Env,
        campaign_id: u32,
        title: String,
        description: String,
    ) -> Result<(), Error> {
        campaigns::update::update_campaign(&env, campaign_id, title, description)
    }

    pub fn update_campaign_description(
        env: Env,
        campaign_id: u32,
        description: String,
    ) -> Result<(), Error> {
        campaigns::update::update_campaign_description(&env, campaign_id, description)
    }

    pub fn extend_campaign_deadline(
        env: Env,
        campaign_id: u32,
        additional_days: u64,
    ) -> Result<(), Error> {
        campaigns::update::extend_campaign_deadline(&env, campaign_id, additional_days)
    }

    // ── Campaign ownership transfer ───────────────────────────────────────────

    pub fn initiate_campaign_transfer(
        env: Env,
        campaign_id: u32,
        new_creator: Address,
    ) -> Result<(), Error> {
        campaigns::transfer::initiate_campaign_transfer(&env, campaign_id, new_creator)
    }

    pub fn accept_campaign_transfer(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::transfer::accept_campaign_transfer(&env, campaign_id)
    }

    pub fn cancel_campaign_transfer(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::transfer::cancel_campaign_transfer(&env, campaign_id)
    }

    // ── Revenue sharing ───────────────────────────────────────────────────────

    pub fn deposit_revenue(env: Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
        revenue::deposit_revenue(&env, campaign_id, amount)
    }

    pub fn claim_revenue(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        revenue::claim_revenue(&env, campaign_id, contributor)
    }

    pub fn claim_creator_revenue(env: Env, campaign_id: u32) -> Result<(), Error> {
        revenue::claim_creator_revenue(&env, campaign_id)
    }

    // ── Voting & verification ─────────────────────────────────────────────────

    pub fn vote_on_campaign(
        env: Env,
        campaign_id: u32,
        voter: Address,
        approve: bool,
    ) -> Result<(), Error> {
        lifecycle::require_not_paused(&env)?;
        bump_instance_ttl(&env);
        voting::cast_vote(&env, campaign_id, voter, approve)
    }

    pub fn verify_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let admin = get_admin(&env);
        assert_admin(&env, &admin)?;
        lifecycle::require_not_paused(&env)?;
        bump_instance_ttl(&env);
        voting::admin_verify(&env, campaign_id)
    }

    pub fn verify_campaigns(env: Env, campaign_ids: soroban_sdk::Vec<u32>) -> Result<u32, Error> {
        let admin = get_admin(&env);
        assert_admin(&env, &admin)?;
        lifecycle::require_not_paused(&env)?;

        const MAX_BATCH_SIZE: u32 = 50;
        let batch_size = campaign_ids.len().min(MAX_BATCH_SIZE);

        let mut verified_count = 0u32;
        let mut first_error: Option<Error> = None;

        bump_instance_ttl(&env);

        for idx in 0..batch_size {
            if let Some(campaign_id) = campaign_ids.get(idx) {
                match voting::admin_verify(&env, campaign_id) {
                    Ok(()) => {
                        verified_count += 1;
                        storage::extend_voting_state_ttl(&env, campaign_id);
                    }
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                }
            }
        }

        env.events().publish(
            ("campaigns_bulk_verified",),
            (verified_count, campaign_ids.len()),
        );

        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(verified_count)
        }
    }

    pub fn verify_campaign_with_votes(env: Env, campaign_id: u32) -> Result<(), Error> {
        lifecycle::require_not_paused(&env)?;
        bump_instance_ttl(&env);
        voting::verify_with_votes(&env, campaign_id)
    }

    pub fn resume_campaign(env: Env, campaign_id: u32, caller: Address) -> Result<(), Error> {
        admin::resume_campaign(&env, campaign_id, caller)
    }

    pub fn purge_voting_state(
        env: Env,
        campaign_id: u32,
        voters: soroban_sdk::Vec<Address>,
        finalize_aggregate: bool,
    ) -> Result<(), Error> {
        admin::purge_voting_state(&env, campaign_id, voters, finalize_aggregate)
    }

    // ── Admin: pause / creation gate ─────────────────────────────────────────

    pub fn pause(env: Env) -> Result<(), Error> {
        admin::pause(&env)
    }

    pub fn unpause(env: Env) -> Result<(), Error> {
        admin::unpause(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
            || env
                .storage()
                .instance()
                .get(&DataKey::AutoPaused)
                .unwrap_or(false)
    }

    pub fn set_creation_disabled(env: Env, disabled: bool) -> Result<(), Error> {
        admin::set_creation_disabled_fn(&env, disabled)
    }

    pub fn is_creation_disabled(env: Env) -> bool {
        get_creation_disabled(&env)
    }

    // ── Admin: fees & config ──────────────────────────────────────────────────

    pub fn update_platform_fee(env: Env, new_fee: u32) -> Result<(), Error> {
        admin::update_platform_fee(&env, new_fee)
    }

    pub fn set_campaign_fee_override(
        env: Env,
        admin: Address,
        campaign_id: u32,
        fee_bps: u32,
    ) -> Result<(), Error> {
        admin::set_campaign_fee_override(&env, admin, campaign_id, fee_bps)
    }

    pub fn set_category_duration_cap(
        env: Env,
        admin: Address,
        category: Category,
        max_days: u64,
    ) -> Result<(), Error> {
        admin::set_category_duration_cap(&env, admin, category, max_days)
    }

    pub fn remove_category_duration_cap(
        env: Env,
        admin: Address,
        category: Category,
    ) -> Result<(), Error> {
        admin::remove_category_duration_cap(&env, admin, category)
    }

    pub fn set_min_campaign_funding_goal(
        env: Env,
        admin: Address,
        min_goal: i128,
    ) -> Result<(), Error> {
        admin::set_min_campaign_funding_goal_fn(&env, admin, min_goal)
    }

    pub fn set_max_campaign_funding_goal(
        env: Env,
        admin: Address,
        max_goal: i128,
    ) -> Result<(), Error> {
        admin::set_max_campaign_funding_goal_fn(&env, admin, max_goal)
    }

    // ── Admin: voting params ──────────────────────────────────────────────────

    pub fn set_voting_params(
        env: Env,
        admin: Address,
        min_votes_quorum: u32,
        approval_threshold_bps: u32,
    ) -> Result<(), Error> {
        admin::set_voting_params(&env, admin, min_votes_quorum, approval_threshold_bps)
    }

    pub fn set_min_voting_balance(
        env: Env,
        admin: Address,
        min_balance: i128,
    ) -> Result<(), Error> {
        admin::set_min_voting_balance_fn(&env, admin, min_balance)
    }

    // ── Admin: token migration ────────────────────────────────────────────────

    pub fn propose_token_update(env: Env, admin: Address, new_token: Address) -> Result<(), Error> {
        admin::propose_token_update(&env, admin, new_token)
    }

    pub fn accept_token_update(env: Env, admin: Address) -> Result<(), Error> {
        admin::accept_token_update(&env, admin)
    }

    pub fn cancel_token_update(env: Env, admin: Address) -> Result<(), Error> {
        admin::cancel_token_update(&env, admin)
    }

    // ── Admin: admin transfer ─────────────────────────────────────────────────

    pub fn initiate_admin_transfer(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), Error> {
        admin::initiate_admin_transfer(&env, admin, new_admin)
    }

    pub fn accept_admin_transfer(env: Env) -> Result<(), Error> {
        admin::accept_admin_transfer(&env)
    }

    pub fn cancel_admin_transfer(env: Env, admin: Address) -> Result<(), Error> {
        admin::cancel_admin_transfer(&env, admin)
    }

    pub fn update_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let admin = get_admin(&env);
        admin::initiate_admin_transfer(&env, admin, new_admin)
    }

    // ── Admin: migrate ────────────────────────────────────────────────────────

    pub fn migrate(env: Env, admin: Address, expected_old_version: u32) -> Result<(), Error> {
        admin::migrate(&env, admin, expected_old_version)
    }

    // ── Contributor cap ───────────────────────────────────────────────────────

    pub fn set_personal_cap(
        env: Env,
        campaign_id: u32,
        contributor: Address,
        amount: i128,
    ) -> Result<(), Error> {
        contributions::set_personal_cap_fn(&env, campaign_id, contributor, amount)
    }

    // ── Read-only queries ─────────────────────────────────────────────────────

    pub fn get_campaign(env: Env, campaign_id: u32) -> Result<Campaign, Error> {
        get_campaign_or_error(&env, campaign_id)
    }

    pub fn get_campaign_optional(env: Env, campaign_id: u32) -> Option<Campaign> {
        get_campaign(&env, campaign_id)
    }

    pub fn get_campaign_count(env: Env) -> u32 {
        get_campaign_count(&env)
    }

    pub fn get_total_raised_global(env: Env) -> i128 {
        get_total_raised_global(&env)
    }

    pub fn get_total_contributors_count(env: Env, campaign_id: u32) -> u32 {
        get_contributor_count(&env, campaign_id)
    }

    pub fn get_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_contribution(&env, campaign_id, &contributor)
    }

    pub fn get_lifetime_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_lifetime_contribution(&env, campaign_id, &contributor)
    }

    pub fn get_revenue_pool(env: Env, campaign_id: u32) -> i128 {
        get_revenue_pool(&env, campaign_id)
    }

    pub fn get_revenue_claimed(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_revenue_claimed(&env, campaign_id, &contributor)
    }

    pub fn get_version(env: Env) -> u32 {
        get_version(&env)
    }

    pub fn get_admin(env: Env) -> Address {
        get_admin(&env)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        get_pending_admin(&env)
    }

    pub fn get_token(env: Env) -> Address {
        get_token(&env)
    }

    pub fn get_platform_fee(env: Env) -> u32 {
        get_platform_fee(&env)
    }

    pub fn get_min_campaign_funding_goal(env: Env) -> i128 {
        get_min_campaign_funding_goal(&env, CAMPAIGN_FUNDING_GOAL_MIN)
    }

    pub fn get_max_campaign_funding_goal(env: Env) -> i128 {
        get_max_campaign_funding_goal(&env, CAMPAIGN_FUNDING_GOAL_MAX)
    }

    pub fn get_min_voting_balance(env: Env) -> i128 {
        get_min_voting_balance(&env)
    }

    pub fn get_approve_votes(env: Env, campaign_id: u32) -> u32 {
        get_approve_votes(&env, campaign_id)
    }

    pub fn get_reject_votes(env: Env, campaign_id: u32) -> u32 {
        get_reject_votes(&env, campaign_id)
    }

    pub fn has_voted(env: Env, campaign_id: u32, voter: Address) -> bool {
        get_has_voted(&env, campaign_id, &voter)
    }

    pub fn get_min_votes_quorum(env: Env) -> u32 {
        get_min_votes_quorum(&env, voting::DEFAULT_MIN_VOTES_QUORUM)
    }

    pub fn get_approval_threshold_bps(env: Env) -> u32 {
        get_approval_threshold_bps(&env, voting::DEFAULT_APPROVAL_THRESHOLD_BPS)
    }

    pub fn get_personal_cap(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_personal_cap(&env, campaign_id, &contributor).unwrap_or(0)
    }

    pub fn get_campaign_reserve(env: Env, campaign_id: u32) -> Option<CampaignReserve> {
        storage::get_campaign_reserve(&env, campaign_id)
    }

    pub fn has_pending_campaign_transfer(env: Env, campaign_id: u32) -> bool {
        match get_campaign(&env, campaign_id) {
            Some(c) => c.pending_creator != MaybePendingCreator::None,
            None => false,
        }
    }

    // ── Listing & pagination ──────────────────────────────────────────────────

    pub fn list_campaigns(env: Env, start: u32, limit: u32) -> soroban_sdk::Vec<Campaign> {
        queries::list_campaigns(&env, start, limit)
    }

    pub fn list_active_campaigns(
        env: Env,
        start: u32,
        limit: u32,
    ) -> (soroban_sdk::Vec<Campaign>, u32) {
        queries::list_active_campaigns(&env, start, limit)
    }

    pub fn get_campaigns_by_category(
        env: Env,
        category: Category,
        offset: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<Campaign> {
        queries::get_campaigns_by_category(&env, category, offset, limit)
    }

    pub fn get_creator_campaigns(
        env: Env,
        creator: Address,
        start: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<Campaign> {
        queries::get_creator_campaigns(&env, creator, start, limit)
    }

    pub fn get_platform_stats(env: Env) -> PlatformStats {
        queries::get_platform_stats(&env)
    }
}

#[cfg(test)]
mod admin_transfer_test;
#[cfg(test)]
mod benchmark_test;
#[cfg(test)]
mod campaign_transfer_test;
#[cfg(test)]
mod create_campaign_proptest;
#[cfg(test)]
mod issues_test;
#[cfg(test)]
mod lifecycle_events_test;
#[cfg(test)]
mod pagination_test;
#[cfg(test)]
mod revenue_share_proptest;
#[cfg(test)]
mod storage_cleanup_test;
#[cfg(test)]
mod test;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_admin_test;
#[cfg(test)]
mod vesting_test;
#[cfg(test)]
mod voting_proptest;
