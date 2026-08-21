use soroban_sdk::{Address, Env, Vec};

use crate::errors::Error;
use crate::lifecycle::{assert_admin, get_campaign_or_error, require_active_campaign};
use crate::storage::{
    self, bump_instance_ttl, decrement_active_campaign_count, get_active_campaign_count, get_admin,
    get_approval_threshold_bps, get_campaign_vesting, get_emergency_withdrawal_proposal,
    get_max_campaign_funding_goal, get_min_campaign_funding_goal, get_min_votes_quorum,
    get_pending_admin, get_pending_token, get_pending_token_release, get_platform_fee, get_token,
    get_token_update_delay_secs, get_total_raised_global, get_version,
    get_withdraw_release_delay_days, get_withdraw_reserve_percentage, is_initialized,
    remove_emergency_withdrawal_proposal, remove_has_voted, remove_pending_admin,
    remove_pending_token, remove_voting_state, set_admin, set_approval_threshold_bps, set_campaign,
    set_campaign_count, set_campaign_reserve, set_creation_disabled,
    set_emergency_withdrawal_proposal, set_initialized, set_max_campaign_funding_goal,
    set_min_campaign_funding_goal, set_min_votes_quorum, set_min_voting_balance, set_pending_admin,
    set_pending_token, set_pending_token_release, set_platform_fee, set_token,
    set_token_update_delay_secs, set_total_raised_global, set_version,
    set_withdraw_release_delay_days, set_withdraw_reserve_percentage, AdminKey,
};
use crate::types::CampaignReserve;
use crate::voting;

pub(crate) fn init(
    env: &Env,
    admin: Address,
    token: Address,
    platform_fee: u32,
) -> Result<(), Error> {
    if is_initialized(env) {
        return Err(Error::AlreadyInitialized);
    }
    admin.require_auth();

    if platform_fee > crate::PLATFORM_FEE_ABSOLUTE_MAX_BPS {
        return Err(Error::InvalidPlatformFee);
    }
    if platform_fee > crate::PLATFORM_FEE_MAX_BPS {
        return Err(Error::InvalidPlatformFee);
    }

    // Validate that the address is a real SEP-41 token contract.
    env.try_invoke_contract::<u32, Error>(
        &token,
        &soroban_sdk::Symbol::new(env, "decimals"),
        soroban_sdk::Vec::new(env),
    )
    .map_err(|_| Error::InvalidTokenContract)?
    .map_err(|_| Error::InvalidTokenContract)?;

    bump_instance_ttl(env);
    set_admin(env, &admin);
    remove_pending_admin(env);
    set_token(env, &token);
    set_initialized(env);

    set_platform_fee(env, platform_fee);
    set_campaign_count(env, 0);
    set_total_raised_global(env, 0);
    set_version(env, crate::CONTRACT_VERSION);
    set_min_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MIN);
    set_min_votes_quorum(env, voting::DEFAULT_MIN_VOTES_QUORUM);
    set_approval_threshold_bps(env, voting::DEFAULT_APPROVAL_THRESHOLD_BPS);
    set_withdraw_release_delay_days(env, 0);
    set_withdraw_reserve_percentage(env, 0);

    env.events().publish(
        ("initialized", admin.clone()),
        (
            token.clone(),
            platform_fee,
            voting::DEFAULT_MIN_VOTES_QUORUM,
            voting::DEFAULT_APPROVAL_THRESHOLD_BPS,
            crate::CONTRACT_VERSION,
        ),
    );
    Ok(())
}

pub(crate) fn pause(env: &Env) -> Result<(), Error> {
    let admin = get_admin(env);
    assert_admin(env, &admin)?;
    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::Paused, &true);
    env.events().publish(("contract_paused", admin), ());
    Ok(())
}

pub(crate) fn unpause(env: &Env) -> Result<(), Error> {
    let admin = get_admin(env);
    assert_admin(env, &admin)?;
    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::Paused, &false);
    env.storage().instance().set(&AdminKey::AutoPaused, &false);
    env.events().publish(("contract_unpaused", admin), ());
    Ok(())
}

pub(crate) fn set_creation_disabled_fn(env: &Env, disabled: bool) -> Result<(), Error> {
    let admin = get_admin(env);
    assert_admin(env, &admin)?;
    // No require_not_paused: admin must be able to gate campaign creation even during pause (#388).
    bump_instance_ttl(env);
    set_creation_disabled(env, disabled);
    env.events()
        .publish(("creation_disabled_updated", admin), disabled);
    Ok(())
}

pub(crate) fn set_voting_params(
    env: &Env,
    admin: Address,
    min_votes_quorum: u32,
    approval_threshold_bps: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: admin must be able to adjust voting parameters during pause (#388).
    bump_instance_ttl(env);
    let old_quorum = get_min_votes_quorum(env, voting::DEFAULT_MIN_VOTES_QUORUM);
    let old_threshold = get_approval_threshold_bps(env, voting::DEFAULT_APPROVAL_THRESHOLD_BPS);
    let caller = admin.clone();
    voting::set_params(env, min_votes_quorum, approval_threshold_bps)?;
    env.events().publish(
        (
            soroban_sdk::Symbol::new(env, "voting_params_updated"),
            caller,
        ),
        (
            old_quorum,
            min_votes_quorum,
            old_threshold,
            approval_threshold_bps,
        ),
    );
    Ok(())
}

pub(crate) fn set_min_voting_balance_fn(
    env: &Env,
    admin: Address,
    min_balance: i128,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if min_balance < 0 {
        return Err(Error::ValidationFailed);
    }

    if min_balance > 1_000_000_000_000_000 {
        env.events()
            .publish(("warning_high_voting_balance",), min_balance);
    }

    bump_instance_ttl(env);
    let old_balance = storage::get_min_voting_balance(env);
    set_min_voting_balance(env, min_balance);
    env.events().publish(
        (
            soroban_sdk::Symbol::new(env, "min_voting_balance_updated"),
            admin,
        ),
        (old_balance, min_balance),
    );
    Ok(())
}

pub(crate) fn update_platform_fee(env: &Env, new_fee: u32) -> Result<(), Error> {
    let admin = get_admin(env);
    assert_admin(env, &admin)?;
    // No require_not_paused: admin must be able to adjust fees during an emergency pause (#388).
    if new_fee > crate::PLATFORM_FEE_ABSOLUTE_MAX_BPS {
        return Err(Error::InvalidPlatformFee);
    }
    if new_fee > crate::PLATFORM_FEE_MAX_BPS {
        return Err(Error::InvalidPlatformFee);
    }
    let old_fee = get_platform_fee(env);
    bump_instance_ttl(env);
    set_platform_fee(env, new_fee);
    env.events().publish(("fee_updated",), (old_fee, new_fee));
    Ok(())
}

pub(crate) fn set_campaign_fee_override(
    env: &Env,
    campaign_id: u32,
    admin: Address,
    fee_bps: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: per-campaign fee overrides are admin governance (#388).
    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    if fee_bps > crate::PLATFORM_FEE_ABSOLUTE_MAX_BPS {
        return Err(Error::ValidationFailed);
    }
    if fee_bps > crate::PLATFORM_FEE_MAX_BPS {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    campaign.fee_override = Some(fee_bps);
    storage::set_campaign(env, campaign_id, &campaign);
    env.events()
        .publish(("campaign_fee_override_set", campaign_id), fee_bps);
    Ok(())
}

pub(crate) fn set_category_duration_cap(
    env: &Env,
    admin: Address,
    category: crate::types::Category,
    max_days: u64,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if !(crate::CAMPAIGN_DURATION_MIN_DAYS..=crate::CAMPAIGN_DURATION_MAX_DAYS).contains(&max_days)
    {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    storage::set_category_duration_cap(env, category, max_days);
    env.events()
        .publish(("category_duration_cap_set", category as u32), max_days);
    Ok(())
}

pub(crate) fn remove_category_duration_cap(
    env: &Env,
    admin: Address,
    category: crate::types::Category,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    bump_instance_ttl(env);
    storage::remove_category_duration_cap(env, category);
    env.events()
        .publish(("category_duration_cap_removed", category as u32), ());
    Ok(())
}

pub(crate) fn set_category_voting_threshold(
    env: &Env,
    admin: Address,
    category: crate::types::Category,
    threshold_bps: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if !(voting::MIN_APPROVAL_THRESHOLD_BPS..=crate::BPS_DENOMINATOR).contains(&threshold_bps) {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    storage::set_category_voting_threshold_bps(env, category, threshold_bps);
    env.events().publish(
        ("category_voting_threshold_set", category as u32),
        threshold_bps,
    );
    Ok(())
}

pub(crate) fn remove_category_voting_threshold(
    env: &Env,
    admin: Address,
    category: crate::types::Category,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    bump_instance_ttl(env);
    storage::remove_category_voting_threshold_bps(env, category);
    env.events()
        .publish(("category_voting_threshold_removed", category as u32), ());
    Ok(())
}

pub(crate) fn set_min_campaign_funding_goal_fn(
    env: &Env,
    admin: Address,
    min_goal: i128,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: funding goal limits are admin governance (#388).
    if min_goal <= 0 {
        return Err(Error::FundingGoalMustBePositive);
    }
    let old_min_goal = get_min_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MIN);
    bump_instance_ttl(env);
    set_min_campaign_funding_goal(env, min_goal);
    env.events().publish(
        ("min_campaign_funding_goal_updated",),
        (old_min_goal, min_goal),
    );
    Ok(())
}

pub(crate) fn set_max_campaign_funding_goal_fn(
    env: &Env,
    admin: Address,
    max_goal: i128,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: funding goal limits are admin governance (#388).
    if max_goal <= 0 {
        return Err(Error::FundingGoalMustBePositive);
    }
    if max_goal < get_min_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MIN) {
        return Err(Error::ValidationFailed);
    }
    let old_max_goal = get_max_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MAX);
    bump_instance_ttl(env);
    set_max_campaign_funding_goal(env, max_goal);
    env.events().publish(
        ("max_campaign_funding_goal_updated",),
        (old_max_goal, max_goal),
    );
    Ok(())
}

pub(crate) fn migrate(env: &Env, admin: Address, expected_old_version: u32) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    let current = get_version(env);
    if current != expected_old_version {
        return Err(Error::ValidationFailed);
    }
    set_version(env, crate::CONTRACT_VERSION);
    env.events().publish(
        ("migrated",),
        (expected_old_version, crate::CONTRACT_VERSION),
    );
    Ok(())
}

pub(crate) fn propose_token_update(
    env: &Env,
    admin: Address,
    new_token: Address,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    env.try_invoke_contract::<u32, Error>(
        &new_token,
        &soroban_sdk::Symbol::new(env, "decimals"),
        soroban_sdk::Vec::new(env),
    )
    .map_err(|_| Error::InvalidTokenContract)?
    .map_err(|_| Error::InvalidTokenContract)?;

    let delay_secs = get_token_update_delay_secs(env, crate::TOKEN_UPDATE_DELAY_SECS);
    let release_after = env
        .ledger()
        .timestamp()
        .checked_add(delay_secs)
        .ok_or(Error::ValidationFailed)?;

    bump_instance_ttl(env);
    set_pending_token(env, &new_token);
    set_pending_token_release(env, release_after);
    env.events()
        .publish(("token_update_proposed",), (new_token, release_after));
    Ok(())
}

/// Fix #407: refuse the token swap while any campaign still has escrowed funds.
/// All existing campaigns must reach a terminal state (withdrawn or cancelled)
/// before the token address can change, preventing stranded balances.
pub(crate) fn accept_token_update(env: &Env, admin: Address) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    let new_token = get_pending_token(env).ok_or(Error::ValidationFailed)?;
    let release_after = get_pending_token_release(env).ok_or(Error::ValidationFailed)?;
    if env.ledger().timestamp() < release_after {
        return Err(Error::ValidationFailed);
    }

    // Block the swap while any campaign is still active OR any contributor
    // principal/reserve remains escrowed in the old token (issue #407).
    //
    // The active-campaign count alone is insufficient: `cancel_campaign` drops
    // that count immediately, but contributor refunds stay escrowed until each
    // contributor calls `claim_refund` — which pays out in the *current* token.
    // Gating on the outstanding balance closes that window. Vesting reserves are
    // likewise tracked in `total_raised_global` until released.
    if get_active_campaign_count(env) > 0 || get_total_raised_global(env) != 0 {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);
    let old_token = get_token(env);
    set_token(env, &new_token);
    remove_pending_token(env);
    env.events()
        .publish(("token_update_accepted",), (old_token, new_token));
    Ok(())
}

pub(crate) fn cancel_token_update(env: &Env, admin: Address) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if get_pending_token(env).is_none() {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    remove_pending_token(env);
    env.events().publish(("token_update_cancelled",), ());
    Ok(())
}

/// Propose an emergency withdrawal for a campaign. The admin can trigger
/// a withdrawal that bypasses the normal goal/deadline requirements, but
/// must wait for the timelock (EMERGENCY_WITHDRAW_TIMELOCK_SECS) before
/// execution. This provides a recovery path for campaigns where the creator
/// is unresponsive or funds are at risk, while giving contributors a window
/// to challenge the withdrawal.
pub(crate) fn propose_emergency_withdrawal(
    env: &Env,
    admin: Address,
    campaign_id: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: emergency withdrawal is a critical recovery path.

    let campaign = get_campaign_or_error(env, campaign_id)?;

    // Cannot propose if already proposed
    if get_emergency_withdrawal_proposal(env).is_some() {
        return Err(Error::ValidationFailed);
    }

    // Must have funds to withdraw
    if campaign.amount_raised == 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    // Must be in an active state (not already withdrawn or cancelled)
    if campaign.funds_withdrawn || campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }

    // Must be verified
    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }

    // Use effective_amount_raised for consistency with normal withdraw
    // (which deducts fee/reserve from effective_amount_raised, not amount_raised).
    // The emergency withdrawal computes fee + reserve and transfers the rest.
    let amount = campaign.effective_amount_raised;

    let release_after = env
        .ledger()
        .timestamp()
        .checked_add(crate::EMERGENCY_WITHDRAW_TIMELOCK_SECS)
        .ok_or(Error::Overflow)?;

    bump_instance_ttl(env);
    set_emergency_withdrawal_proposal(env, campaign_id, &campaign.creator, amount, release_after);
    env.events().publish(
        ("emergency_withdrawal_proposed", admin),
        (campaign_id, campaign.creator.clone(), amount, release_after),
    );

    Ok(())
}

/// Execute an emergency withdrawal after the timelock has elapsed.
/// The withdrawal computes fee + reserve from effective_amount_raised
/// and transfers the remainder to the creator. The reserve is set with
/// the campaign's vesting params (or global defaults). Decrements the
/// active campaign count if the campaign was active.
pub(crate) fn execute_emergency_withdrawal(
    env: &Env,
    admin: Address,
    campaign_id: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;

    let proposal =
        get_emergency_withdrawal_proposal(env).ok_or(Error::EmergencyWithdrawalNotProposed)?;
    let (prop_campaign_id, creator, _amount, release_after) = proposal;

    if prop_campaign_id != campaign_id {
        return Err(Error::EmergencyWithdrawalNotProposed);
    }

    if env.ledger().timestamp() < release_after {
        return Err(Error::EmergencyWithdrawalTimelockNotMet);
    }

    let mut campaign = get_campaign_or_error(env, campaign_id)?;

    // Re-validate state
    if campaign.funds_withdrawn || campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }
    if campaign.effective_amount_raised == 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    // Compute fee and reserve using effective_amount_raised (consistent with withdraw_funds)
    let platform_fee = campaign
        .fee_override
        .unwrap_or_else(|| get_platform_fee(env));
    let fee_amount = campaign
        .effective_amount_raised
        .checked_mul(platform_fee as i128)
        .and_then(|n| n.checked_add(crate::BPS_CEIL_OFFSET))
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;
    let total_after_fee = campaign.effective_amount_raised - fee_amount;

    let (delay_days, reserve_bps) = get_campaign_vesting(env, campaign_id).unwrap_or_else(|| {
        (
            get_withdraw_release_delay_days(env),
            get_withdraw_reserve_percentage(env),
        )
    });
    let reserve_amount = total_after_fee
        .checked_mul(reserve_bps as i128)
        .and_then(|n| n.checked_add(crate::BPS_CEIL_OFFSET))
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;
    let creator_amount = total_after_fee - reserve_amount;

    // Transition state (CEI pattern)
    let was_active = campaign.is_active;
    campaign.funds_withdrawn = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);

    if was_active {
        decrement_active_campaign_count(env);
    }

    if reserve_amount > 0 {
        let release_timestamp = env
            .ledger()
            .timestamp()
            .checked_add(delay_days * crate::SECONDS_PER_DAY)
            .ok_or(Error::Overflow)?;

        let reserve = CampaignReserve {
            amount: reserve_amount,
            release_timestamp,
            released: false,
        };
        set_campaign_reserve(env, campaign_id, &reserve);
    }

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised
            .checked_sub(campaign.effective_amount_raised - reserve_amount)
            .ok_or(Error::Overflow)?,
    );

    // Token transfers after state updates (CEI pattern)
    let admin_addr = get_admin(env);
    let client = crate::lifecycle::token_client(env);

    client.transfer(&env.current_contract_address(), &admin_addr, &fee_amount);
    client.transfer(&env.current_contract_address(), &creator, &creator_amount);

    bump_instance_ttl(env);
    remove_emergency_withdrawal_proposal(env);

    env.events().publish(
        ("emergency_withdrawal_executed", admin),
        (campaign_id, fee_amount, creator_amount, reserve_amount),
    );

    if reserve_amount > 0 {
        env.events()
            .publish(("emergency_reserve_withheld", campaign_id), reserve_amount);
    }

    Ok(())
}

/// Cancel a pending emergency withdrawal proposal.
pub(crate) fn cancel_emergency_withdrawal(env: &Env, admin: Address) -> Result<(), Error> {
    assert_admin(env, &admin)?;

    if get_emergency_withdrawal_proposal(env).is_none() {
        return Err(Error::EmergencyWithdrawalNotProposed);
    }

    bump_instance_ttl(env);
    remove_emergency_withdrawal_proposal(env);
    env.events()
        .publish(("emergency_withdrawal_cancelled", admin), ());

    Ok(())
}

/// Lets the admin override the timelock delay that `propose_token_update`
/// enforces before a pending token update can be accepted, instead of it
/// being fixed at the compiled-in `TOKEN_UPDATE_DELAY_SECS` default (#650).
/// Does not affect a token update that is already pending: that keeps the
/// release timestamp computed with the delay in effect at proposal time.
pub(crate) fn set_token_update_delay_secs_fn(
    env: &Env,
    admin: Address,
    delay_secs: u64,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if delay_secs == 0 || delay_secs > crate::MAX_TOKEN_UPDATE_DELAY_SECS {
        return Err(Error::ValidationFailed);
    }
    let old_delay = get_token_update_delay_secs(env, crate::TOKEN_UPDATE_DELAY_SECS);
    bump_instance_ttl(env);
    set_token_update_delay_secs(env, delay_secs);
    env.events()
        .publish(("token_update_delay_updated",), (old_delay, delay_secs));
    Ok(())
}

pub(crate) fn initiate_admin_transfer(
    env: &Env,
    admin: Address,
    new_admin: Address,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: admin transfer is the critical recovery path during an emergency (#388).

    let current_admin = get_admin(env);
    if new_admin == current_admin {
        return Err(Error::InvalidNewOwner);
    }

    if let Some(old_pending) = get_pending_admin(env) {
        env.events()
            .publish(("admin_transfer_cancelled",), old_pending);
    }

    bump_instance_ttl(env);
    set_pending_admin(env, &new_admin);
    env.events()
        .publish(("admin_transfer_initiated",), (current_admin, new_admin));

    Ok(())
}

pub(crate) fn accept_admin_transfer(env: &Env) -> Result<(), Error> {
    // No require_not_paused: accepting an admin transfer is part of the emergency recovery path (#388).
    let pending_admin = get_pending_admin(env).ok_or(Error::NoTransferPending)?;
    pending_admin.require_auth();

    bump_instance_ttl(env);
    let old_admin = get_admin(env);
    set_admin(env, &pending_admin);
    remove_pending_admin(env);
    env.events()
        .publish(("admin_updated", old_admin), pending_admin);

    Ok(())
}

pub(crate) fn cancel_admin_transfer(env: &Env, admin: Address) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    // No require_not_paused: cancelling an admin transfer must be available during pause (#388).

    if get_pending_admin(env).is_none() {
        return Err(Error::NoTransferPending);
    }

    bump_instance_ttl(env);
    remove_pending_admin(env);
    env.events().publish(("admin_transfer_cancelled",), admin);

    Ok(())
}

pub(crate) fn purge_voting_state(
    env: &Env,
    campaign_id: u32,
    voters: Vec<Address>,
    finalize_aggregate: bool,
) -> Result<(), Error> {
    let admin = get_admin(env);
    assert_admin(env, &admin)?;

    let campaign = get_campaign_or_error(env, campaign_id)?;
    if !campaign.funds_withdrawn && !campaign.is_cancelled {
        return Err(Error::ValidationFailed);
    }

    if voters.is_empty() {
        return Err(Error::ValidationFailed);
    }

    const MAX_VOTERS_PER_CALL: u32 = 50;
    if voters.len() > MAX_VOTERS_PER_CALL {
        return Err(Error::ValidationFailed);
    }

    for voter in voters.iter() {
        remove_has_voted(env, campaign_id, &voter);
    }

    if finalize_aggregate {
        remove_voting_state(env, campaign_id);
        env.events()
            .publish(("voting_state_purged", campaign_id), ());
    }

    Ok(())
}

pub(crate) fn resume_campaign(env: &Env, campaign_id: u32, caller: Address) -> Result<(), Error> {
    caller.require_auth();

    // Check auto-pause FIRST: if the contract isn't auto-paused, bail early
    // without touching campaign storage. Also ensures the admin can still
    // clear the global flag via unpause() even if the triggering campaign
    // has since become inactive (fix #436).
    let auto_paused: bool = env
        .storage()
        .instance()
        .get(&AdminKey::AutoPaused)
        .unwrap_or(false);
    if !auto_paused {
        return Err(Error::ValidationFailed);
    }

    let campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;

    let admin = get_admin(env);
    if caller != campaign.creator && caller != admin {
        return Err(Error::NotAuthorized);
    }

    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::AutoPaused, &false);

    env.events()
        .publish(("campaign_resumed", campaign_id, caller), ());

    Ok(())
}
