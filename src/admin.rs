//! # Admin trust model (#810)
//!
// A single admin key controls sensitive operations: `update_platform_fee`,
// `set_campaign_fee_override`, `verify_campaign`, `pause`, `propose_token_update`,
// `migrate`, and `purge_voting_state`. A compromised key can force-verify
// fraudulent campaigns, set fees to 0, or pause the contract.
//
// **Short-term mitigation**: guard the admin key with a hardware wallet or
// multisig signer. The `propose_admin` / `accept_admin` two-step transfer
// prevents accidental key rotation but does not add a timelock.
//
// **Long-term**: replace the single-key admin with a Soroban-native multisig
// or timelock contract so that high-impact actions (fee override to 0,
// force-verify, token migration) require multiple signers or a mandatory
// delay before taking effect.

use soroban_sdk::{Address, Env, Vec};

use crate::errors::Error;
use crate::lifecycle::{emit_event, ensure_admin, get_campaign_or_error, require_active_campaign};
use crate::storage::{
    self, bump_instance_ttl, get_active_campaign_count, get_admin, get_approval_threshold_bps,
    get_max_campaign_funding_goal, get_max_contribution_per_transaction,
    get_min_campaign_funding_goal, get_min_votes_quorum, get_pending_admin, get_pending_token,
    get_pending_token_release, get_platform_fee, get_token, get_token_update_delay_secs,
    get_total_raised_global, get_version, is_initialized, remove_has_voted, remove_pending_admin,
    remove_pending_token, remove_voting_state, set_admin, set_approval_threshold_bps,
    set_campaign_count, set_creation_disabled, set_initialized, set_max_campaign_funding_goal,
    set_max_contribution_per_transaction as set_max_contribution_per_transaction_value,
    set_min_campaign_funding_goal, set_min_votes_quorum, set_min_voting_balance, set_pending_admin,
    set_pending_token, set_pending_token_release, set_platform_fee, set_token,
    set_token_update_delay_secs, set_total_raised_global, set_version,
    set_withdraw_release_delay_days, set_withdraw_reserve_percentage, AdminKey,
};
use crate::voting;

// ---------------------------------------------------------------------------
// Security validation helpers (#993)
//
// These functions encapsulate access control and parameter validation checks
// that must hold across all admin operations. Centralizing them here ensures
// security invariants are enforced consistently and auditable in one place.
// ---------------------------------------------------------------------------

/// Validates that an address list contains no duplicates.
///
/// Duplicate addresses in emergency pause signers or governance committees
/// would allow a single party to meet quorum requirements alone, defeating
/// the purpose of multi-party authorization.
///
/// Returns `Error::ValidationFailed` when duplicates are found.
#[inline]
fn validate_unique_addresses(addresses: &Vec<Address>) -> Result<(), Error> {
    for i in 0..addresses.len() {
        for j in (i + 1)..addresses.len() {
            if addresses.get_unchecked(i) == addresses.get_unchecked(j) {
                return Err(Error::ValidationFailed);
            }
        }
    }
    Ok(())
}

/// Validates that a quorum value is within acceptable bounds.
///
/// A quorum of 0 would allow actions with no votes, defeating governance.
/// A quorum above a reasonable maximum (e.g., 1000) could make the system
/// ungovernable by setting an unachievable threshold.
///
/// Returns `Error::ValidationFailed` for out-of-bounds quorums.
#[inline]
fn validate_quorum_bounds(quorum: u32) -> Result<(), Error> {
    const MAX_REASONABLE_QUORUM: u32 = 1000;
    if quorum == 0 || quorum > MAX_REASONABLE_QUORUM {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

/// Validates that a percentage (in basis points) does not exceed 100%.
///
/// Many operations express percentages as basis points (1 bps = 0.01%).
/// A value above 10,000 bps (100%) would represent an invalid percentage
/// and could cause arithmetic overflow or underflow in downstream calculations.
///
/// Returns `Error::ValidationFailed` for values > 10,000.
#[inline]
fn validate_bps_percentage(bps: u32) -> Result<(), Error> {
    if bps > 10_000 {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

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
    set_max_contribution_per_transaction_value(env, 0);

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
    let admin = ensure_admin!(env);
    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::Paused, &true);
    emit_event!(env, ("contract_paused", admin), ());
    Ok(())
}

pub(crate) fn unpause(env: &Env) -> Result<(), Error> {
    let admin = ensure_admin!(env);
    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::Paused, &false);
    env.storage().instance().set(&AdminKey::AutoPaused, &false);
    emit_event!(env, ("contract_unpaused", admin), ());
    Ok(())
}

pub(crate) fn set_emergency_pause_signers(
    env: &Env,
    admin: Address,
    signers: Vec<Address>,
) -> Result<(), Error> {
    ensure_admin!(env, &admin);
    if signers.is_empty() {
        return Err(Error::ValidationFailed);
    }
    // #993 — Prevent a single signer from appearing multiple times, which would
    // defeat multi-party authorization by allowing one address to meet quorum alone.
    validate_unique_addresses(&signers)?;
    // No require_not_paused: admin must be able to configure signers even during pause (#785).
    bump_instance_ttl(env);
    storage::set_emergency_pause_signers(env, &signers);
    env.events()
        .publish(("emergency_pause_signers_updated", admin), signers.len());
    Ok(())
}

pub(crate) fn emergency_pause(env: &Env, caller: Address) -> Result<(), Error> {
    caller.require_auth();
    let signers = storage::get_emergency_pause_signers(env);
    if signers.is_empty() {
        return Err(Error::NotAuthorized);
    }
    if !signers.iter().any(|s| s == caller) {
        return Err(Error::NotAuthorized);
    }
    bump_instance_ttl(env);
    env.storage().instance().set(&AdminKey::Paused, &true);
    emit_event!(env, ("emergency_paused", caller), ());
    Ok(())
}

pub(crate) fn set_creation_disabled_fn(env: &Env, disabled: bool) -> Result<(), Error> {
    let admin = ensure_admin!(env);
    // No require_not_paused: admin must be able to gate campaign creation even during pause (#388).
    bump_instance_ttl(env);
    set_creation_disabled(env, disabled);
    env.events()
        .publish(("creation_disabled_updated", admin), disabled);
    Ok(())
}

/// Add or remove a token from the set creators may denominate campaigns in
/// (#784).
///
/// Admin-gated because the allowlist is the only thing standing between a
/// contributor and a `transfer` into a contract the platform never reviewed.
/// Removing a token does not affect campaigns already denominated in it —
/// their currency is pinned at creation — it only stops new ones.
pub(crate) fn set_token_allowed_fn(env: &Env, token: Address, allowed: bool) -> Result<(), Error> {
    let admin = ensure_admin!(env);
    bump_instance_ttl(env);
    crate::storage::set_token_allowed(env, &token, allowed);
    env.events()
        .publish(("campaign_token_allowlisted", admin), (token, allowed));
    Ok(())
}

pub(crate) fn set_voting_params(
    env: &Env,
    admin: Address,
    min_votes_quorum: u32,
    approval_threshold_bps: u32,
) -> Result<(), Error> {
    ensure_admin!(env, &admin);
    // #993 — Validate quorum and threshold before applying them to prevent
    // ungovernable states (quorum = 0 or impossibly high) or invalid percentages.
    validate_quorum_bounds(min_votes_quorum)?;
    validate_bps_percentage(approval_threshold_bps)?;
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
    ensure_admin!(env, &admin);
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
    ensure_admin!(env);
    // No require_not_paused: admin must be able to adjust fees during an emergency pause (#388).
    // Two bounds, deliberately, though only the tighter one can currently
    // reject (#793):
    //
    //   * `PLATFORM_FEE_ABSOLUTE_MAX_BPS` (10000 = 100%) is a correctness
    //     bound. The basis-point formula in `withdraw_funds` computes
    //     `amount_raised * fee / 10000`; a fee above the denominator would
    //     make the platform's cut exceed what was raised and drive
    //     `total_after_fee` negative.
    //   * `PLATFORM_FEE_MAX_BPS` (1000 = 10%) is the policy bound — what the
    //     platform promises creators it will never charge more than.
    //
    // Policy is stricter than correctness today, so the absolute check is
    // unreachable. It is kept and checked first anyway: it is the invariant
    // the arithmetic depends on, and someone raising the policy ceiling must
    // not be able to breach it by editing one constant.
    if new_fee > crate::PLATFORM_FEE_ABSOLUTE_MAX_BPS || new_fee > crate::PLATFORM_FEE_MAX_BPS {
        return Err(Error::InvalidPlatformFee);
    }
    let old_fee = get_platform_fee(env);
    bump_instance_ttl(env);
    set_platform_fee(env, new_fee);
    env.events()
        .publish(("platform_fee_updated",), (old_fee, new_fee));
    Ok(())
}
