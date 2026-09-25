#![no_std]
#![allow(unexpected_cfgs)]

/// Current contract version. Increment this on each breaking upgrade.
const CONTRACT_VERSION: u32 = 1;

// Validation limit constants
pub(crate) const CAMPAIGN_TITLE_MIN_LEN: u32 = 1;
pub(crate) const CAMPAIGN_TITLE_MAX_LEN: u32 = 100;
pub(crate) const CAMPAIGN_DESCRIPTION_MIN_LEN: u32 = 1;
pub(crate) const CAMPAIGN_DESCRIPTION_MAX_LEN: u32 = 1000;
/// Bounds for a single campaign tag (#798).
pub(crate) const CAMPAIGN_TAG_MIN_LEN: u32 = 1;
pub(crate) const CAMPAIGN_TAG_MAX_LEN: u32 = 32;
/// Maximum number of tags a campaign may carry (#798). Bounds the work
/// `add_campaign_tag` and tag queries can be asked to do per campaign.
pub(crate) const MAX_TAGS_PER_CAMPAIGN: u32 = 10;
pub(crate) const CAMPAIGN_DURATION_MIN_DAYS: u64 = 1;
pub(crate) const CAMPAIGN_DURATION_MAX_DAYS: u64 = 365;
pub(crate) const CAMPAIGN_EXTENSION_MAX_DAYS: u64 = 365;
pub(crate) const CAMPAIGN_FUNDING_GOAL_MIN: i128 = 100_000;
pub(crate) const CAMPAIGN_FUNDING_GOAL_MAX: i128 = 1_000_000_000_000_000; // 10^15
pub(crate) const PLATFORM_FEE_MAX_BPS: u32 = 1000; // 10%
pub(crate) const PLATFORM_FEE_ABSOLUTE_MAX_BPS: u32 = BPS_DENOMINATOR; // 100% — hard limit, basis-point formula requires fee <= BPS_DENOMINATOR
pub(crate) const REVENUE_SHARE_MAX_BPS: u32 = 5000; // 50%
pub(crate) const AUTO_PAUSE_SINGLE_CONTRIBUTION_BPS_THRESHOLD: i128 = 20000;
pub(crate) const AUTO_PAUSE_BURST_THRESHOLD: u32 = 10;
/// #535: burst detection (the per-block contribution-count read/write) only
/// runs once a campaign has raised at least this fraction of its funding
/// goal, in basis points (5000 = 50%). Skips a wasted ledger read on the
/// happy path for new/low-activity campaigns, which can't plausibly be
/// mid-burst yet.
pub(crate) const AUTO_PAUSE_BURST_CHECK_MIN_RAISED_BPS: i128 = 5000;
pub(crate) const LIST_MAX_LIMIT: u32 = 50;
/// #518: max number of `(campaign_id, amount)` pairs accepted by
/// `batch_contribute` in a single call. Each item pays the full per-item cost
/// of `contribute` (cap checks, burst guard, two persistent writes), so this
/// is kept well under `verify_campaigns`' read-mostly batch size of 50.
pub(crate) const MAX_BATCH_CONTRIBUTE_SIZE: u32 = 20;

mod admin;
mod bookmarks;
mod campaigns;
mod comments;
mod constants;
mod contributions;
mod errors;
mod lifecycle;
mod milestones;
mod queries;
mod revenue;
mod storage;
mod tags;
mod types;
mod voting;

pub(crate) use constants::{
    BPS_CEIL_OFFSET, BPS_DENOMINATOR, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS, MAX_EXTENSION_DAYS,
    MAX_TOKEN_UPDATE_DELAY_SECS, SECONDS_PER_DAY, TOKEN_UPDATE_DELAY_SECS, TRANSFER_EXPIRY_SECS,
};
pub use errors::Error;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};
use storage::*;
pub use storage::{AdminKey, CampaignKey, ContributionKey, RevenueKey, StorageKey, VotingKey};
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

    /// Initializes the contract with its `admin`, the platform `token` and the
    /// `platform_fee` (basis points). Callable exactly once; requires `admin`'s
    /// authorization.
    ///
    /// # Errors
    /// * `AlreadyInitialized` — `init` has already been called.
    /// * `InvalidPlatformFee` — `platform_fee` exceeds the allowed maximum.
    pub fn init(env: Env, admin: Address, token: Address, platform_fee: u32) -> Result<(), Error> {
        admin::init(&env, admin, token, platform_fee)
    }

    // ── Campaign creation ─────────────────────────────────────────────────────

    /// Creates a campaign denominated in the platform token and returns its new
    /// id. The campaign creator in `params` must authorize the call.
    ///
    /// Reverts if the contract is paused, campaign creation is disabled, or any
    /// field in `params` fails validation (title/description length, funding-goal
    /// bounds, duration and category caps).
    #[allow(clippy::too_many_arguments)]
    pub fn create_campaign(env: Env, params: CreateCampaignParams) -> Result<u32, Error> {
        let id = campaigns::create::create_campaign(&env, params)?;
        increment_active_campaign_count(&env);
        Ok(id)
    }

    /// Creates a campaign denominated in `token` rather than the platform
    /// token (#784).
    ///
    /// The currency is fixed for the campaign's lifetime — contributions,
    /// refunds, withdrawals, milestone payouts and revenue all move in it —
    /// and must be on the admin allowlist. `create_campaign` remains the
    /// platform-token shorthand.
    pub fn create_campaign_with_token(
        env: Env,
        params: CreateCampaignParams,
        token: Address,
    ) -> Result<u32, Error> {
        let id = campaigns::create::create_campaign_with_token(&env, params, token)?;
        increment_active_campaign_count(&env);
        Ok(id)
    }

    /// The currency a campaign accepts. Falls back to the platform token for
    /// campaigns created before per-campaign currencies existed.
    pub fn get_campaign_token(env: Env, campaign_id: u32) -> Address {
        get_campaign_token(&env, campaign_id)
    }

    /// Admin: allow or disallow a token as a campaign currency (#784).
    ///
    /// Disallowing affects only future campaigns; existing ones keep the
    /// currency they were created with.
    pub fn set_token_allowed(env: Env, token: Address, allowed: bool) -> Result<(), Error> {
        admin::set_token_allowed_fn(&env, token, allowed)
    }

    /// Whether a token may be chosen as a campaign currency. The platform
    /// token always is.
    pub fn is_token_allowed(env: Env, token: Address) -> bool {
        is_token_allowed(&env, &token)
    }

    // ── Contributions ─────────────────────────────────────────────────────────

    /// Contributes `amount` of the campaign's currency from `contributor` to
    /// `campaign_id`. Requires `contributor`'s authorization.
    ///
    /// Reverts if the contract is paused, the campaign is not active or past its
    /// deadline, or `amount` breaks a global, campaign or personal cap.
    pub fn contribute(
        env: Env,
        campaign_id: u32,
        contributor: Address,
        amount: i128,
    ) -> Result<(), Error> {
        contributions::contribute(&env, campaign_id, contributor, amount)
    }

    /// Refunds `contributor`'s full contribution to `campaign_id` when the
    /// campaign is cancelled or ended without meeting its goal. Requires
    /// `contributor`'s authorization.
    pub fn claim_refund(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        contributions::claim_refund(&env, campaign_id, contributor)
    }

    /// Contributes to multiple campaigns in a single transaction, moving the
    /// combined token amount in one transfer instead of one per campaign
    /// (#518). Every item is validated with the same rules as `contribute`;
    /// if any item fails, the whole batch reverts atomically.
    pub fn batch_contribute(
        env: Env,
        contributor: Address,
        contributions: soroban_sdk::Vec<(u32, i128)>,
    ) -> Result<(), Error> {
        contributions::batch_contribute(&env, contributor, contributions)
    }

    // ── Withdrawals ───────────────────────────────────────────────────────────

    /// Pays out a verified campaign that met its goal to its creator, net of
    /// the platform fee (and of any vesting reserve). Creator only; reverts if the
    /// contract is paused, the campaign is unverified or funds were already
    /// withdrawn.
    pub fn withdraw_funds(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::withdraw::withdraw_funds(&env, campaign_id)
    }

    /// Releases a campaign's held-back vesting reserve (see
    /// `set_vesting_params`) once its vesting delay has elapsed. Reverts if the
    /// contract is paused or the reserve was already released.
    pub fn withdraw_reserve(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::withdraw::withdraw_reserve(&env, campaign_id)
    }

    /// Admin: sets the vesting policy applied at withdrawal — `reserve_bps` of
    /// the payout is held back and becomes withdrawable via `withdraw_reserve`
    /// after `delay_days`.
    pub fn set_vesting_params(
        env: Env,
        admin: Address,
        delay_days: u64,
        reserve_bps: u32,
    ) -> Result<(), Error> {
        campaigns::withdraw::set_vesting_params(&env, admin, delay_days, reserve_bps)
    }

    // ── Emergency withdrawal — admin last-resort recovery (#802) ───────────────

    /// **Last resort.** Proposes recovering the escrowed funds of a campaign
    /// that met its goal but can no longer pay out — e.g. the creator address
    /// is a dead/locked account — sending them to `recipient`.
    ///
    /// Moves no funds. Starts a mandatory 7-day timelock; the transfer only
    /// happens when `execute_emergency_withdrawal` is called afterwards. Emits
    /// `emergency_withdrawal_proposed`. Admin only.
    pub fn emergency_withdraw(
        env: Env,
        admin: Address,
        campaign_id: u32,
        recipient: Address,
    ) -> Result<(), Error> {
        campaigns::emergency::emergency_withdraw(&env, admin, campaign_id, recipient)
    }

    /// Cancels a pending emergency withdrawal before it is executed (#802).
    /// Emits `emergency_withdrawal_cancelled`. Admin only.
    pub fn cancel_emergency_withdrawal(
        env: Env,
        admin: Address,
        campaign_id: u32,
    ) -> Result<(), Error> {
        campaigns::emergency::cancel_emergency_withdrawal(&env, admin, campaign_id)
    }

    /// Executes a pending emergency withdrawal once its 7-day timelock has
    /// elapsed, transferring the campaign's escrowed principal to the recipient
    /// recorded at proposal time (#802). Emits `emergency_withdrawal_executed`.
    /// Admin only.
    pub fn execute_emergency_withdrawal(
        env: Env,
        admin: Address,
        campaign_id: u32,
    ) -> Result<(), Error> {
        campaigns::emergency::execute_emergency_withdrawal(&env, admin, campaign_id)
    }

    /// The pending emergency withdrawal for a campaign (recipient and the
    /// timestamp it becomes executable), or `None` (#802).
    pub fn get_emergency_withdrawal(env: Env, campaign_id: u32) -> Option<EmergencyWithdrawal> {
        storage::get_emergency_withdrawal(&env, campaign_id)
    }

    // ── Milestone-based withdrawals (#783) ─────────────────────────────────────

    /// Creator: defines the milestone schedule for `campaign_id`, splitting its
    /// payout into separately verified tranches (#783). Reverts if the contract
    /// is paused, the campaign is inactive, or the milestones fail validation.
    pub fn set_milestones(
        env: Env,
        campaign_id: u32,
        milestones: soroban_sdk::Vec<Milestone>,
    ) -> Result<(), Error> {
        milestones::set_milestones(&env, campaign_id, milestones)
    }

    /// Admin: marks milestone `milestone_id` of `campaign_id` as verified,
    /// making it claimable by the creator via `claim_milestone`.
    pub fn verify_milestone(
        env: Env,
        admin: Address,
        campaign_id: u32,
        milestone_id: u32,
    ) -> Result<(), Error> {
        milestones::verify_milestone(&env, admin, campaign_id, milestone_id)
    }

    /// Creator: withdraws the tranche for a verified milestone. Reverts if the
    /// contract is paused, the campaign is inactive or unverified, or the
    /// milestone was already claimed.
    pub fn claim_milestone(env: Env, campaign_id: u32, milestone_id: u32) -> Result<(), Error> {
        milestones::claim_milestone(&env, campaign_id, milestone_id)
    }

    /// Returns the milestone schedule configured for `campaign_id` (empty if
    /// none).
    pub fn get_milestones(env: Env, campaign_id: u32) -> soroban_sdk::Vec<Milestone> {
        storage::get_campaign_milestones(&env, campaign_id)
    }

    /// Whether milestone `milestone_id` of `campaign_id` has already been
    /// paid out.
    pub fn is_milestone_claimed(env: Env, campaign_id: u32, milestone_id: u32) -> bool {
        storage::is_milestone_claimed(&env, campaign_id, milestone_id)
    }

    // ── Campaign lifecycle ────────────────────────────────────────────────────

    /// Creator: cancels an active campaign so contributors can claim refunds.
    /// Reverts once the campaign has met its goal (anti-rug-pull guard) — use
    /// `admin_cancel_campaign` for fraud response instead.
    pub fn cancel_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::cancel::cancel_campaign(&env, campaign_id)
    }

    /// Admin-only targeted fraud response: cancels a single campaign without
    /// pausing the entire platform (#508). Unlike `cancel_campaign`, this
    /// bypasses the goal-met anti-rug-pull guard so admins can stop verified
    /// fraudulent campaigns even after they've hit their funding goal.
    pub fn admin_cancel_campaign(
        env: Env,
        admin: Address,
        campaign_id: u32,
        reason: String,
    ) -> Result<(), Error> {
        campaigns::cancel::admin_cancel_campaign(&env, admin, campaign_id, reason)
    }

    /// Creator: replaces the title and description of a campaign that has not
    /// been verified yet. Reverts if paused, already verified, or the new text
    /// fails length validation.
    pub fn update_campaign(
        env: Env,
        campaign_id: u32,
        title: String,
        description: String,
    ) -> Result<(), Error> {
        campaigns::update::update_campaign(&env, campaign_id, title, description)
    }

    /// Creator: replaces only the description of a campaign. Reverts if paused
    /// or the description fails length validation.
    pub fn update_campaign_description(
        env: Env,
        campaign_id: u32,
        description: String,
    ) -> Result<(), Error> {
        campaigns::update::update_campaign_description(&env, campaign_id, description)
    }

    /// Creator: pushes an active campaign's deadline out by `additional_days`.
    /// A campaign may be extended at most once and only before its deadline
    /// passes.
    ///
    /// # Errors
    /// * `DeadlineAlreadyExtended` — The deadline was already extended.
    /// * `DeadlinePassed` — The current deadline is already in the past.
    pub fn extend_campaign_deadline(
        env: Env,
        campaign_id: u32,
        additional_days: u64,
    ) -> Result<(), Error> {
        campaigns::update::extend_campaign_deadline(&env, campaign_id, additional_days)
    }

    // ── Campaign ownership transfer ───────────────────────────────────────────

    /// Starts a two-step ownership transfer of `campaign_id` to `new_creator`.
    /// Both the current creator and `new_creator` must authorize. The nomination
    /// expires after `TRANSFER_EXPIRY_SECS`.
    ///
    /// # Errors
    /// * `InvalidNewOwner` — `new_creator` is already the creator.
    /// * `TransferAlreadyPending` — An unexpired nomination already exists.
    pub fn initiate_campaign_transfer(
        env: Env,
        campaign_id: u32,
        new_creator: Address,
    ) -> Result<(), Error> {
        campaigns::transfer::initiate_campaign_transfer(&env, campaign_id, new_creator)
    }

    /// Completes a pending campaign transfer. The nominated creator must
    /// authorize; ownership and the creator index move to them.
    ///
    /// # Errors
    /// * `NoTransferPending` — Nothing is pending, or the nomination expired.
    pub fn accept_campaign_transfer(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::transfer::accept_campaign_transfer(&env, campaign_id)
    }

    /// Creator: withdraws a pending ownership nomination.
    ///
    /// # Errors
    /// * `NoTransferPending` — No transfer is pending.
    pub fn cancel_campaign_transfer(env: Env, campaign_id: u32) -> Result<(), Error> {
        campaigns::transfer::cancel_campaign_transfer(&env, campaign_id)
    }

    /// Admin recovery path for a pending transfer whose nominee is
    /// unreachable (#869): lets the admin clear `pending_creator` without
    /// waiting for `TRANSFER_EXPIRY_SECS` to elapse.
    pub fn admin_cancel_campaign_transfer(
        env: Env,
        admin: Address,
        campaign_id: u32,
    ) -> Result<(), Error> {
        campaigns::transfer::admin_cancel_campaign_transfer(&env, admin, campaign_id)
    }

    // ── Revenue sharing ───────────────────────────────────────────────────────

    /// Creator: deposits `amount` of post-campaign revenue into the campaign's
    /// revenue pool for pro-rata distribution to contributors. Requires revenue
    /// sharing to be enabled on the campaign.
    pub fn deposit_revenue(env: Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
        revenue::deposit_revenue(&env, campaign_id, amount)
    }

    /// Claims `contributor`'s pro-rata share of the campaign's revenue pool.
    /// Requires `contributor`'s authorization.
    pub fn claim_revenue(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        revenue::claim_revenue(&env, campaign_id, contributor)
    }

    /// Creator: claims the creator's retained share of the campaign's revenue
    /// pool. Requires revenue sharing to be enabled.
    pub fn claim_creator_revenue(env: Env, campaign_id: u32) -> Result<(), Error> {
        revenue::claim_creator_revenue(&env, campaign_id)
    }

    // ── Voting & verification ─────────────────────────────────────────────────

    /// Casts `voter`'s approve/reject vote on an unverified campaign. Requires
    /// `voter`'s authorization and the minimum voting balance; each address may
    /// vote once. Reverts if the contract is paused.
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

    /// Admin: verifies `campaign_id` directly, bypassing community voting.
    /// Reverts if the contract is paused.
    pub fn verify_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let admin = get_admin(&env);
        assert_admin(&env, &admin)?;
        lifecycle::require_not_paused(&env)?;
        bump_instance_ttl(&env);
        voting::admin_verify(&env, campaign_id)
    }

    /// Batch-verifies up to 50 campaigns in one admin call (#442).
    ///
    /// Returns `Ok((verified_ids, failed_ids))` covering every id it
    /// processed. Successful verifications are committed even when other ids
    /// fail, so callers can distinguish partial success from total failure and
    /// retry only the failed ids — the previous behaviour collapsed the whole
    /// batch to `Err(first_error)`. Per-campaign failures are collected in
    /// `failed_ids` and never abort the batch; only hard errors (not admin,
    /// paused) return `Err`. The voting-state TTL is extended for every
    /// processed id, success or failure.
    ///
    /// # Errors
    /// * `NotAuthorized` — Caller is not the stored admin.
    /// * `ContractPaused` — The contract is paused.
    pub fn verify_campaigns(
        env: Env,
        campaign_ids: soroban_sdk::Vec<u32>,
    ) -> Result<(soroban_sdk::Vec<u32>, soroban_sdk::Vec<u32>), Error> {
        let admin = get_admin(&env);
        assert_admin(&env, &admin)?;
        lifecycle::require_not_paused(&env)?;

        const MAX_BATCH_SIZE: u32 = 50;
        let batch_size = campaign_ids.len().min(MAX_BATCH_SIZE);

        let mut verified_ids: soroban_sdk::Vec<u32> = soroban_sdk::Vec::new(&env);
        let mut failed_ids: soroban_sdk::Vec<u32> = soroban_sdk::Vec::new(&env);

        bump_instance_ttl(&env);

        for idx in 0..batch_size {
            if let Some(campaign_id) = campaign_ids.get(idx) {
                storage::extend_voting_state_ttl(&env, campaign_id);
                match voting::admin_verify(&env, campaign_id) {
                    Ok(()) => verified_ids.push_back(campaign_id),
                    Err(_) => failed_ids.push_back(campaign_id),
                }
            }
        }

        env.events().publish(
            ("campaigns_bulk_verified",),
            (verified_ids.len(), failed_ids.clone()),
        );

        Ok((verified_ids, failed_ids))
    }

    /// Verifies `campaign_id` once its community vote meets quorum and the
    /// applicable approval threshold. Callable by anyone; reverts if paused,
    /// the campaign is inactive or past its deadline, or the vote has not
    /// passed.
    pub fn verify_campaign_with_votes(env: Env, campaign_id: u32) -> Result<(), Error> {
        lifecycle::require_not_paused(&env)?;
        bump_instance_ttl(&env);
        voting::verify_with_votes(&env, campaign_id)
    }

    /// Clears an automatic (anomaly-triggered) pause for `campaign_id`. `caller`
    /// must authorize and be either the campaign creator or the admin.
    ///
    /// # Errors
    /// * `ValidationFailed` — The contract is not auto-paused.
    /// * `NotAuthorized` — `caller` is neither the creator nor the admin.
    pub fn resume_campaign(env: Env, campaign_id: u32, caller: Address) -> Result<(), Error> {
        admin::resume_campaign(&env, campaign_id, caller)
    }

    /// Admin: deletes per-voter records for `voters` on `campaign_id` to reclaim
    /// storage; `finalize_aggregate` also clears the campaign's vote tallies.
    pub fn purge_voting_state(
        env: Env,
        campaign_id: u32,
        voters: soroban_sdk::Vec<Address>,
        finalize_aggregate: bool,
    ) -> Result<(), Error> {
        admin::purge_voting_state(&env, campaign_id, voters, finalize_aggregate)
    }

    // ── Admin: pause / creation gate ─────────────────────────────────────────

    /// Admin: pauses all state-changing user operations.
    pub fn pause(env: Env) -> Result<(), Error> {
        admin::pause(&env)
    }

    /// Admin: lifts a manual or automatic pause.
    pub fn unpause(env: Env) -> Result<(), Error> {
        admin::unpause(&env)
    }

    /// Pauses the contract from a registered emergency signer, so an incident
    /// can be contained without the admin key. `caller` must authorize.
    ///
    /// # Errors
    /// * `NotAuthorized` — `caller` is not a registered emergency signer.
    pub fn emergency_pause(env: Env, caller: Address) -> Result<(), Error> {
        admin::emergency_pause(&env, caller)
    }

    /// Admin: replaces the set of addresses allowed to call `emergency_pause`.
    pub fn set_emergency_pause_signers(
        env: Env,
        admin: Address,
        signers: soroban_sdk::Vec<Address>,
    ) -> Result<(), Error> {
        admin::set_emergency_pause_signers(&env, admin, signers)
    }

    /// Returns the addresses allowed to call `emergency_pause`.
    pub fn get_emergency_pause_signers(env: Env) -> soroban_sdk::Vec<Address> {
        storage::get_emergency_pause_signers(&env)
    }

    /// Whether the contract is paused, either manually (`pause` /
    /// `emergency_pause`) or automatically by anomaly detection.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&AdminKey::Paused)
            .unwrap_or(false)
            || env
                .storage()
                .instance()
                .get(&AdminKey::AutoPaused)
                .unwrap_or(false)
    }

    /// Admin: turns new campaign creation off (`true`) or back on. Works while
    /// paused.
    pub fn set_creation_disabled(env: Env, disabled: bool) -> Result<(), Error> {
        admin::set_creation_disabled_fn(&env, disabled)
    }

    /// Whether new campaign creation is currently disabled.
    pub fn is_creation_disabled(env: Env) -> bool {
        get_creation_disabled(&env)
    }

    // ── Admin: fees & config ──────────────────────────────────────────────────

    /// Admin: sets the platform fee in basis points. Works while paused.
    ///
    /// # Errors
    /// * `InvalidPlatformFee` — `new_fee` exceeds `PLATFORM_FEE_MAX_BPS`.
    pub fn update_platform_fee(env: Env, new_fee: u32) -> Result<(), Error> {
        admin::update_platform_fee(&env, new_fee)
    }

    /// Sets the global per-transaction contribution cap. `0` disables the cap.
    pub fn set_max_contribution_per_tx(
        env: Env,
        admin: Address,
        amount: i128,
    ) -> Result<(), Error> {
        admin::set_max_contribution_per_transaction(&env, admin, amount)
    }

    /// Admin: sets a campaign-specific platform fee (`fee_bps`) that replaces
    /// the global fee for `campaign_id`.
    pub fn set_campaign_fee_override(
        env: Env,
        campaign_id: u32,
        admin: Address,
        fee_bps: u32,
    ) -> Result<(), Error> {
        admin::set_campaign_fee_override(&env, campaign_id, admin, fee_bps)
    }

    /// Admin: caps the duration (in days) of new campaigns in `category`.
    pub fn set_category_duration_cap(
        env: Env,
        admin: Address,
        category: Category,
        max_days: u64,
    ) -> Result<(), Error> {
        admin::set_category_duration_cap(&env, admin, category, max_days)
    }

    /// Admin: removes the duration cap for `category`, restoring the global
    /// maximum.
    pub fn remove_category_duration_cap(
        env: Env,
        admin: Address,
        category: Category,
    ) -> Result<(), Error> {
        admin::remove_category_duration_cap(&env, admin, category)
    }

    /// Admin: sets the minimum funding goal a new campaign may request.
    pub fn set_min_campaign_funding_goal(
        env: Env,
        admin: Address,
        min_goal: i128,
    ) -> Result<(), Error> {
        admin::set_min_campaign_funding_goal_fn(&env, admin, min_goal)
    }

    /// Admin: sets the maximum funding goal a new campaign may request.
    pub fn set_max_campaign_funding_goal(
        env: Env,
        admin: Address,
        max_goal: i128,
    ) -> Result<(), Error> {
        admin::set_max_campaign_funding_goal_fn(&env, admin, max_goal)
    }

    // ── Admin: voting params ──────────────────────────────────────────────────

    /// Admin: sets the global voting quorum (`min_votes_quorum`) and approval
    /// threshold (`approval_threshold_bps`) used by `verify_campaign_with_votes`.
    pub fn set_voting_params(
        env: Env,
        admin: Address,
        min_votes_quorum: u32,
        approval_threshold_bps: u32,
    ) -> Result<(), Error> {
        admin::set_voting_params(&env, admin, min_votes_quorum, approval_threshold_bps)
    }

    /// Admin: sets the minimum token balance an address needs to vote.
    pub fn set_min_voting_balance(
        env: Env,
        admin: Address,
        min_balance: i128,
    ) -> Result<(), Error> {
        admin::set_min_voting_balance_fn(&env, admin, min_balance)
    }

    /// Admin: overrides the approval threshold (basis points) for `category`
    /// (#536).
    pub fn set_category_voting_threshold(
        env: Env,
        admin: Address,
        category: Category,
        threshold_bps: u32,
    ) -> Result<(), Error> {
        admin::set_category_voting_threshold(&env, admin, category, threshold_bps)
    }

    /// Admin: removes `category`'s approval-threshold override, falling back to
    /// the global default.
    pub fn remove_category_voting_threshold(
        env: Env,
        admin: Address,
        category: Category,
    ) -> Result<(), Error> {
        admin::remove_category_voting_threshold(&env, admin, category)
    }

    /// Returns the approval threshold (in basis points) that actually applies
    /// to `category`: its per-category override if one is set, otherwise the
    /// global default (#536).
    pub fn get_category_voting_threshold(env: Env, category: Category) -> u32 {
        voting::effective_approval_threshold_bps(&env, category)
    }

    // ── Admin: token migration ────────────────────────────────────────────────

    /// Admin: proposes replacing the platform token with `new_token`. Takes
    /// effect only via `accept_token_update` after the timelock (see
    /// `get_token_update_delay_secs`).
    pub fn propose_token_update(env: Env, admin: Address, new_token: Address) -> Result<(), Error> {
        admin::propose_token_update(&env, admin, new_token)
    }

    /// Admin: applies a proposed token update once its timelock has elapsed.
    ///
    /// # Errors
    /// * `ValidationFailed` — Nothing is pending or the timelock has not elapsed.
    pub fn accept_token_update(env: Env, admin: Address) -> Result<(), Error> {
        admin::accept_token_update(&env, admin)
    }

    /// Admin: discards a pending token update proposal.
    pub fn cancel_token_update(env: Env, admin: Address) -> Result<(), Error> {
        admin::cancel_token_update(&env, admin)
    }

    /// Overrides the timelock delay `propose_token_update` enforces before a
    /// pending token update can be accepted (default: 7 days), so platforms
    /// that want a longer or shorter timelock don't need a code change and
    /// redeploy (#650). Must be in `(0, 365 days]`.
    pub fn set_token_update_delay_secs(
        env: Env,
        admin: Address,
        delay_secs: u64,
    ) -> Result<(), Error> {
        admin::set_token_update_delay_secs_fn(&env, admin, delay_secs)
    }

    // ── Admin: admin transfer ─────────────────────────────────────────────────

    /// Admin: nominates `new_admin` as the next admin (two-step transfer).
    /// Works while paused.
    ///
    /// # Errors
    /// * `InvalidNewOwner` — `new_admin` is already the admin.
    pub fn initiate_admin_transfer(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), Error> {
        admin::initiate_admin_transfer(&env, admin, new_admin)
    }

    /// Completes a pending admin transfer. The nominated admin must authorize.
    /// Works while paused.
    ///
    /// # Errors
    /// * `NoTransferPending` — No admin transfer is pending.
    pub fn accept_admin_transfer(env: Env) -> Result<(), Error> {
        admin::accept_admin_transfer(&env)
    }

    /// Admin: withdraws a pending admin nomination.
    pub fn cancel_admin_transfer(env: Env, admin: Address) -> Result<(), Error> {
        admin::cancel_admin_transfer(&env, admin)
    }

    /// Legacy alias for `initiate_admin_transfer` using the stored admin. Starts
    /// a two-step transfer; `new_admin` must still call `accept_admin_transfer`.
    pub fn update_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let admin = get_admin(&env);
        admin::initiate_admin_transfer(&env, admin, new_admin)
    }

    // ── Admin: migrate ────────────────────────────────────────────────────────

    /// Admin: runs storage migrations after an upgrade. `expected_old_version`
    /// must match the stored version, guarding against running a migration twice
    /// or on the wrong build.
    pub fn migrate(env: Env, admin: Address, expected_old_version: u32) -> Result<(), Error> {
        admin::migrate(&env, admin, expected_old_version)
    }

    // ── Contributor cap ───────────────────────────────────────────────────────

    /// Sets a self-imposed cap on how much `contributor` may contribute to
    /// `campaign_id` in total (#503). Requires `contributor`'s authorization.
    pub fn set_personal_cap(
        env: Env,
        campaign_id: u32,
        contributor: Address,
        amount: i128,
    ) -> Result<(), Error> {
        contributions::set_personal_cap_fn(&env, campaign_id, contributor, amount)
    }

    /// Removes the contributor's personal contribution cap for a campaign,
    /// restoring the campaign-wide `max_contribution_per_user` as the only
    /// bound on their contributions (#503). Requires `contributor`'s auth.
    pub fn remove_personal_cap(
        env: Env,
        campaign_id: u32,
        contributor: Address,
    ) -> Result<(), Error> {
        contributions::remove_personal_cap_fn(&env, campaign_id, contributor)
    }

    // ── Read-only queries ─────────────────────────────────────────────────────

    /// Returns campaign `campaign_id`.
    ///
    /// # Errors
    /// * `CampaignNotFound` — No campaign with that id.
    pub fn get_campaign(env: Env, campaign_id: u32) -> Result<Campaign, Error> {
        get_campaign_or_error(&env, campaign_id)
    }

    /// Returns campaign `campaign_id`, or `None` if it does not exist.
    pub fn get_campaign_optional(env: Env, campaign_id: u32) -> Option<Campaign> {
        get_campaign(&env, campaign_id)
    }

    /// Returns the total number of campaigns ever created.
    pub fn get_campaign_count(env: Env) -> u32 {
        get_campaign_count(&env)
    }

    /// Returns the total amount raised across all campaigns.
    pub fn get_total_raised_global(env: Env) -> i128 {
        get_total_raised_global(&env)
    }

    /// Returns the number of distinct contributors to `campaign_id`.
    pub fn get_total_contributors_count(env: Env, campaign_id: u32) -> u32 {
        get_contributor_count(&env, campaign_id)
    }

    /// Returns `contributor`'s current (unrefunded) contribution to
    /// `campaign_id`.
    pub fn get_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_contribution(&env, campaign_id, &contributor)
    }

    /// Returns everything `contributor` has ever contributed to `campaign_id`,
    /// including amounts later refunded.
    pub fn get_lifetime_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_lifetime_contribution(&env, campaign_id, &contributor)
    }

    /// Returns the revenue deposited into `campaign_id`'s pool.
    pub fn get_revenue_pool(env: Env, campaign_id: u32) -> i128 {
        get_revenue_pool(&env, campaign_id)
    }

    /// Returns how much revenue `contributor` has already claimed from
    /// `campaign_id`.
    pub fn get_revenue_claimed(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_revenue_claimed(&env, campaign_id, &contributor)
    }

    /// Returns the contract version recorded in storage (updated by `migrate`).
    pub fn get_version(env: Env) -> u32 {
        get_version(&env)
    }

    /// Returns the compiled-in contract version. Unlike `get_version`, this
    /// reads a constant baked into the WASM at build time rather than
    /// instance storage, so it can be called on a freshly deployed contract
    /// before `init` has ever been invoked (#523).
    pub fn contract_version(_env: Env) -> u32 {
        CONTRACT_VERSION
    }

    /// Returns the current admin address.
    pub fn get_admin(env: Env) -> Address {
        get_admin(&env)
    }

    /// Returns the nominated admin of a pending transfer, if any.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        get_pending_admin(&env)
    }

    /// Returns the platform token address.
    pub fn get_token(env: Env) -> Address {
        get_token(&env)
    }

    /// Returns the global platform fee in basis points.
    pub fn get_platform_fee(env: Env) -> u32 {
        get_platform_fee(&env)
    }

    /// Returns the global per-transaction contribution cap; `0` means unlimited.
    pub fn get_max_contribution_per_tx(env: Env) -> i128 {
        get_max_contribution_per_transaction(&env)
    }

    /// Returns the basis-point denominator (10_000 == 100%) that fee and
    /// threshold values are expressed against, so off-chain code can read it
    /// from the deployed contract instead of hardcoding it (#652).
    pub fn get_bps_denominator(_env: Env) -> u32 {
        BPS_DENOMINATOR
    }

    /// Returns the timelock delay (seconds) currently enforced by
    /// `propose_token_update`: the admin override if one has been set via
    /// `set_token_update_delay_secs`, otherwise the compiled-in
    /// `TOKEN_UPDATE_DELAY_SECS` default (#650, #652).
    pub fn get_token_update_delay_secs(env: Env) -> u64 {
        get_token_update_delay_secs(&env, TOKEN_UPDATE_DELAY_SECS)
    }

    /// Returns the minimum funding goal for new campaigns (admin override or
    /// `CAMPAIGN_FUNDING_GOAL_MIN`).
    pub fn get_min_campaign_funding_goal(env: Env) -> i128 {
        get_min_campaign_funding_goal(&env, CAMPAIGN_FUNDING_GOAL_MIN)
    }

    /// Returns the maximum funding goal for new campaigns (admin override or
    /// `CAMPAIGN_FUNDING_GOAL_MAX`).
    pub fn get_max_campaign_funding_goal(env: Env) -> i128 {
        get_max_campaign_funding_goal(&env, CAMPAIGN_FUNDING_GOAL_MAX)
    }

    /// Returns the minimum token balance required to vote.
    pub fn get_min_voting_balance(env: Env) -> i128 {
        get_min_voting_balance(&env)
    }

    /// Returns the number of approve votes on `campaign_id`.
    pub fn get_approve_votes(env: Env, campaign_id: u32) -> u32 {
        get_approve_votes(&env, campaign_id)
    }

    /// Returns the number of reject votes on `campaign_id`.
    pub fn get_reject_votes(env: Env, campaign_id: u32) -> u32 {
        get_reject_votes(&env, campaign_id)
    }

    /// Whether `voter` has already voted on `campaign_id`.
    pub fn has_voted(env: Env, campaign_id: u32, voter: Address) -> bool {
        get_has_voted(&env, campaign_id, &voter)
    }

    /// Returns the global voting quorum (admin override or the default).
    pub fn get_min_votes_quorum(env: Env) -> u32 {
        get_min_votes_quorum(&env, voting::DEFAULT_MIN_VOTES_QUORUM)
    }

    /// Returns the global approval threshold in basis points (admin override
    /// or the default).
    pub fn get_approval_threshold_bps(env: Env) -> u32 {
        get_approval_threshold_bps(&env, voting::DEFAULT_APPROVAL_THRESHOLD_BPS)
    }

    /// Returns `contributor`'s personal cap for `campaign_id`; `0` means no
    /// personal cap is set.
    pub fn get_personal_cap(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_personal_cap(&env, campaign_id, &contributor).unwrap_or(0)
    }

    /// Returns the vesting reserve held back for `campaign_id`, if any.
    pub fn get_campaign_reserve(env: Env, campaign_id: u32) -> Option<CampaignReserve> {
        storage::get_campaign_reserve(&env, campaign_id)
    }

    /// Returns the payout marker stored for `campaign_id`, or `None` if none
    /// has been recorded.
    pub fn get_campaign_payout_marker(env: Env, campaign_id: u32) -> Option<u32> {
        storage::get_campaign_payout_marker(&env, campaign_id)
    }

    /// Whether `campaign_id` has a pending ownership nomination. Returns
    /// `false` for unknown campaigns.
    pub fn has_pending_campaign_transfer(env: Env, campaign_id: u32) -> bool {
        get_campaign(&env, campaign_id).is_some_and(|c| c.pending_creator.is_some())
    }

    /// Checks whether `creator` owns `campaign_id` in O(1) via the creator reverse
    /// index, without scanning the creator's campaign bucket (#478).
    pub fn is_campaign_creator(env: Env, campaign_id: u32, creator: Address) -> bool {
        storage::is_campaign_creator(&env, campaign_id, &creator)
    }

    // ── Listing & pagination ──────────────────────────────────────────────────

    /// Returns up to `limit` campaigns (capped at `LIST_MAX_LIMIT`) with ids
    /// after the exclusive cursor `start`.
    pub fn list_campaigns(env: Env, start: u32, limit: u32) -> soroban_sdk::Vec<Campaign> {
        queries::list_campaigns(&env, start, limit)
    }

    /// Returns up to `limit` active campaigns with ids after `start`, plus the
    /// next cursor to pass as `start`; `0` means there are no more pages.
    pub fn list_active_campaigns(
        env: Env,
        start: u32,
        limit: u32,
    ) -> (soroban_sdk::Vec<Campaign>, u32) {
        queries::list_active_campaigns(&env, start, limit)
    }

    /// `offset` is a **zero-based positional index** into this category's
    /// campaign list, not a campaign ID — unlike `list_campaigns`'s `start`,
    /// which is an exclusive campaign-ID cursor. Page by advancing
    /// `offset += page.len()`; the two cursor styles are not interchangeable
    /// (#845).
    pub fn get_campaigns_by_category(
        env: Env,
        category: Category,
        offset: u32,
        limit: u32,
    ) -> (soroban_sdk::Vec<Campaign>, u32) {
        queries::get_campaigns_by_category(&env, category, offset, limit)
    }

    /// `start` is a **zero-based positional index** into this creator's
    /// campaign list, not a campaign ID — unlike `list_campaigns`'s `start`,
    /// which is an exclusive campaign-ID cursor. Page by advancing
    /// `start += page.len()`; the two cursor styles are not interchangeable
    /// (#845).
    pub fn get_creator_campaigns(
        env: Env,
        creator: Address,
        start: u32,
        limit: u32,
    ) -> (soroban_sdk::Vec<Campaign>, u32) {
        queries::get_creator_campaigns(&env, creator, start, limit)
    }

    /// Returns platform-wide aggregate statistics.
    pub fn get_platform_stats(env: Env) -> PlatformStats {
        queries::get_platform_stats(&env)
    }

    /// Returns the platform report (aggregate figures for dashboards).
    pub fn get_platform_report(env: Env) -> PlatformReport {
        queries::get_platform_report(&env)
    }

    /// Returns aggregate statistics for campaigns owned by `creator`.
    pub fn get_creator_stats(env: Env, creator: Address) -> CreatorStats {
        queries::get_creator_stats(&env, creator)
    }

    /// Returns aggregate statistics for `campaign_id`.
    pub fn get_campaign_stats(env: Env, campaign_id: u32) -> CampaignStats {
        queries::get_campaign_stats(&env, campaign_id)
    }

    /// Returns a page of `contributor`'s positions as
    /// `(campaign_id, amount, status, refundable)` tuples, where `status` is one
    /// of `active`, `verified`, `inactive`, `withdrawn` or `cancelled`. The
    /// returned `u32` is the next cursor to pass as `start`; `0` means done.
    pub fn get_contributor_portfolio(
        env: Env,
        contributor: Address,
        start: u32,
        limit: u32,
    ) -> (soroban_sdk::Vec<(u32, i128, String, bool)>, u32) {
        queries::get_contributor_portfolio(&env, contributor, start, limit)
    }

    // ── Bookmarks / saved campaigns ───────────────────────────────────────────

    /// Saves `campaign_id` to `user`'s on-chain bookmark list. Requires
    /// `user`'s authorization.
    pub fn save_campaign(env: Env, user: Address, campaign_id: u32) -> Result<(), Error> {
        bookmarks::save_campaign(&env, user, campaign_id)
    }

    /// Saves multiple `campaign_ids` to `user`'s on-chain bookmark list in a
    /// single transaction. Requires `user`'s authorization. The batch is
    /// atomic — any invalid id reverts the whole call.
    pub fn batch_save_campaigns(
        env: Env,
        user: Address,
        campaign_ids: soroban_sdk::Vec<u32>,
    ) -> Result<(), Error> {
        bookmarks::batch_save_campaigns(&env, user, campaign_ids)
    }

    /// Removes `campaign_id` from `user`'s on-chain bookmark list. Requires
    /// `user`'s authorization.
    pub fn remove_saved_campaign(env: Env, user: Address, campaign_id: u32) -> Result<(), Error> {
        bookmarks::remove_saved_campaign(&env, user, campaign_id)
    }

    /// Removes every bookmark from `user`'s on-chain bookmark list in a single
    /// transaction. Requires `user`'s authorization.
    pub fn clear_saved_campaigns(env: Env, user: Address) -> Result<(), Error> {
        bookmarks::clear_saved_campaigns(&env, user)
    }

    /// Returns the list of campaign ids `user` has bookmarked, in the order
    /// they were saved, excluding those bookmarked to campaigns that have
    /// since been cancelled.
    pub fn get_saved_campaigns(env: Env, user: Address) -> soroban_sdk::Vec<u32> {
        bookmarks::get_saved(&env, user)
    }

    /// Returns the number of `user`'s live (non-cancelled) bookmarks.
    pub fn get_saved_campaigns_count(env: Env, user: Address) -> u32 {
        bookmarks::get_saved_count(&env, user)
    }

    // ── Comment moderation transparency (#797) ──────────────────────────────

    /// Record that an off-chain comment was removed. Admin only.
    ///
    /// Comments themselves stay off-chain; what lands here is the immutable
    /// evidence that one was suppressed, so moderation cannot be silent.
    /// Idempotent — re-censuring the same hash is a no-op and emits no second
    /// event.
    pub fn censure_comment(
        env: Env,
        campaign_id: u32,
        comment_hash: BytesN<32>,
        reason: String,
    ) -> Result<(), Error> {
        let admin = get_admin(&env);
        comments::censure_comment(&env, admin, campaign_id, comment_hash, reason)
    }

    /// Lift a censure, restoring a comment to displayable. Admin only.
    ///
    /// Emits its own event so a censure-then-revert cannot be used to hide
    /// that the suppression ever happened.
    pub fn uncensure_comment(
        env: Env,
        campaign_id: u32,
        comment_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let admin = get_admin(&env);
        comments::uncensure_comment(&env, admin, campaign_id, comment_hash)
    }

    /// Whether a comment is currently censured — the check a frontend makes
    /// before rendering. Total: an unknown hash reads as not censured.
    pub fn is_comment_censured(env: Env, campaign_id: u32, comment_hash: BytesN<32>) -> bool {
        comments::comment_is_censured(&env, campaign_id, comment_hash)
    }

    /// The censure record (reason, timestamp, acting admin), or `None`.
    pub fn get_comment_censure(
        env: Env,
        campaign_id: u32,
        comment_hash: BytesN<32>,
    ) -> Option<CommentCensure> {
        comments::comment_censure_record(&env, campaign_id, comment_hash)
    }

    /// How many comments have been censured on a campaign.
    pub fn get_censured_comment_count(env: Env, campaign_id: u32) -> u32 {
        comments::campaign_censured_comment_count(&env, campaign_id)
    }
}

#[cfg(test)]
mod tests;
