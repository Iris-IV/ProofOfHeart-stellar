//! # Revenue Sharing Engine
//!
//! This module implements the revenue sharing mechanism for ProofOfHeart campaigns.
//! It provides functionality for depositing revenue, claiming contributor shares, and creator payouts.
//!
//! ## Overview
//!
//! The revenue sharing system allows creators to share a percentage of post-campaign revenue
//! with contributors. Once a campaign's funding goal is met and funds are withdrawn, creators
//! can deposit revenue, which is then split between contributors (based on their contribution
//! amount) and the creator themselves (based on the revenue share percentage).
//!
//! ## Revenue Allocation Mathematics
//!
//! ### Total Pool Distribution
//!
//! When revenue is deposited into a campaign with revenue sharing enabled:
//!
//! ```text
//! Total Revenue Pool = deposit_amount
//!
//! Contributor Share (BPS) = revenue_share_percentage
//! Creator Share (BPS) = 10_000 - revenue_share_percentage
//!
//! Contributor Pool = Total Pool * Contributor Share / 10_000
//! Creator Pool = Total Pool * Creator Share / 10_000
//! ```
//!
//! ### Per-Contributor Allocation
//!
//! Each contributor's claimable amount from the contributor pool is calculated as:
//!
//! ```text
//! Contributor's Claimable = (contribution_amount / effective_amount_raised) * Contributor Pool
//!
//! Where:
//! - contribution_amount = the contributor's individual contribution to the campaign
//! - effective_amount_raised = total raised minus any refunds claimed (prevents race conditions)
//! - Contributor Pool = total revenue pool * revenue_share_percentage / 10_000 (basis points)
//! ```
//!
//! ### Creator Allocation
//!
//! The creator receives their share directly computed from the total pool:
//!
//! ```text
//! Creator's Claimable = Total Pool * Creator Share / 10_000
//!
//! Where Creator Share = 10_000 - revenue_share_percentage
//! ```
//!
//! ## Critical Invariants
//!
//! 1. **No Claims Before Withdrawal**: Contributors cannot claim until `funds_withdrawn` is true.
//!    This prevents race conditions where `amount_raised` could grow while claims are being
//!    calculated against a moving denominator.
//!
//! 2. **Dust Handling**: Integer division truncates each contributor's share. The final claimant
//!    receives the full remaining pool balance to ensure complete distribution (#526).
//!
//! 3. **CEI Pattern**: State updates happen before token transfers to prevent re-entrancy attacks
//!    from malicious token contracts (#557).
//!
//! 4. **Precise Division Order**: Multiplication happens before division to avoid intermediate
//!    truncation to zero when the pool is small relative to basis points (#375).
//!
//! ## Example Scenario
//!
//! Given:
//! - Campaign funding goal: 1000 USDC
//! - Revenue share percentage: 2000 BPS (20% to contributors, 80% to creator)
//! - Contributor A contributed: 300 USDC
//! - Contributor B contributed: 700 USDC
//! - Revenue deposited: 5000 USDC
//!
//! Results:
//! - Contributor Pool: 5000 * 2000 / 10_000 = 1000 USDC
//! - Creator Pool: 5000 * 8000 / 10_000 = 4000 USDC
//! - Contributor A's share: (300 / 1000) * 1000 = 300 USDC
//! - Contributor B's share: (700 / 1000) * 1000 = 700 USDC
//! - Creator's share: 4000 USDC

use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_token_client, get_campaign_or_error, get_creator_campaign, require_not_paused,
    require_revenue_sharing,
};
use crate::storage::{
    bump_instance_ttl, get_contribution, get_contributor_count, get_contributor_revenue_claimants,
    get_contributor_revenue_distributed, get_creator_revenue_claimed, get_revenue_claimed,
    get_revenue_pool, set_contributor_revenue_claimants, set_contributor_revenue_distributed,
    set_creator_revenue_claimed, set_revenue_claimed, set_revenue_pool,
};

pub(crate) fn deposit_revenue(env: &Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
    let campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    if amount <= 0 {
        return Err(Error::ValidationFailed);
    }
    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    if !campaign.funds_withdrawn {
        return Err(Error::ValidationFailed);
    }
    require_revenue_sharing(&campaign, Error::RevenueSharingNotEnabled)?;

    if campaign.amount_raised == 0 {
        return Err(Error::AmountRaisedIsZero);
    }

    bump_instance_ttl(env);

    let current_pool = get_revenue_pool(env, campaign_id);
    set_revenue_pool(
        env,
        campaign_id,
        current_pool.checked_add(amount).ok_or(Error::Overflow)?,
    );

    let client = campaign_token_client(env, campaign_id);
    client.transfer(&campaign.creator, &env.current_contract_address(), &amount);

    env.events()
        .publish(("revenue_deposited", campaign_id, campaign.creator), amount);

    Ok(())
}

pub(crate) fn claim_revenue(
    env: &Env,
    campaign_id: u32,
    contributor: Address,
) -> Result<(), Error> {
    contributor.require_auth();
    require_not_paused(env)?;
    let campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    require_revenue_sharing(&campaign, Error::ValidationFailed)?;

    // Block claims until the creator has withdrawn funds. Until then,
    // `amount_raised` (the share denominator) can still grow as new
    // contributions arrive, which would let early claimers compute their
    // share against a smaller denominator than late claimers and create a
    // race condition.
    if !campaign.funds_withdrawn {
        return Err(Error::ValidationFailed);
    }

    let contribution = get_contribution(env, campaign_id, &contributor);
    if contribution == 0 {
        return Err(Error::ValidationFailed);
    }
    if campaign.effective_amount_raised == 0 {
        return Err(Error::AmountRaisedIsZero);
    }

    let total_pool = get_revenue_pool(env, campaign_id);
    // Defer all division to the last step to avoid intermediate truncation to zero
    // when total_pool is small relative to 10_000 / revenue_share_percentage (#375).
    let total_due = contribution
        .checked_mul(total_pool)
        .and_then(|n| n.checked_mul(campaign.revenue_share_percentage as i128))
        .and_then(|n| n.checked_div(campaign.effective_amount_raised))
        .and_then(|n| n.checked_div(crate::BPS_DENOMINATOR as i128))
        .ok_or(Error::Overflow)?;
    let already_claimed = get_revenue_claimed(env, campaign_id, &contributor);
    let mut claimable = total_due - already_claimed;

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    // #526: integer division truncates each contributor's individual share,
    // so the sum of every contributor's `claimable` can fall short of the
    // full contributor-side pool, leaving dust in the contract until the
    // final eligible contributor claims.
    // Once every contributor entitled to this campaign has claimed at least
    // once, give the last one to claim whatever remains of the
    // contributor-side pool instead of their individually-truncated share,
    // so the full allocation is paid out exactly.
    //
    // #675: a pull-based claim cannot safely infer that a contributor will
    // never claim: transferring that contributor's entitlement (or the dust)
    // early would make a later valid claim underfunded. Resolving dust when a
    // contributor is permanently inactive requires an explicit claim deadline
    // and finalization flow, neither of which exists in this API.
    let is_first_claim = already_claimed == 0;
    let claimants_so_far = get_contributor_revenue_claimants(env, campaign_id);
    let total_contributors = get_contributor_count(env, campaign_id);
    let is_last_claimant = is_first_claim
        && total_contributors > 0
        && claimants_so_far.checked_add(1).ok_or(Error::Overflow)? >= total_contributors;

    let distributed_so_far = get_contributor_revenue_distributed(env, campaign_id);
    if is_last_claimant {
        let creator_share_bps =
            crate::BPS_DENOMINATOR as i128 - campaign.revenue_share_percentage as i128;
        let creator_share_total = total_pool
            .checked_mul(creator_share_bps)
            .and_then(|n| n.checked_div(crate::BPS_DENOMINATOR as i128))
            .ok_or(Error::Overflow)?;
        let contributor_pool_total = total_pool - creator_share_total;
        let pool_remaining = contributor_pool_total - distributed_so_far;
        if pool_remaining > claimable {
            claimable = pool_remaining;
        }
    }

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // Update state before the token transfer (CEI pattern) so that a
    // malicious token contract cannot re-enter and double-claim (#557).
    set_revenue_claimed(
        env,
        campaign_id,
        &contributor,
        already_claimed
            .checked_add(claimable)
            .ok_or(Error::Overflow)?,
    );

    // Track the running sum paid out to contributors, and the count of
    // distinct contributors who have claimed at least once, so future claims
    // can detect the last claimant (#526).
    set_contributor_revenue_distributed(
        env,
        campaign_id,
        distributed_so_far
            .checked_add(claimable)
            .ok_or(Error::Overflow)?,
    );
    if is_first_claim {
        set_contributor_revenue_claimants(
            env,
            campaign_id,
            claimants_so_far.checked_add(1).ok_or(Error::Overflow)?,
        );
    }

    // Token transfer happens after all state updates (CEI pattern).
    let client = campaign_token_client(env, campaign_id);
    client.transfer(&env.current_contract_address(), &contributor, &claimable);

    env.events().publish(
        ("revenue_claimed", campaign_id, contributor.clone()),
        claimable,
    );

    Ok(())
}

pub(crate) fn claim_creator_revenue(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    require_revenue_sharing(&campaign, Error::ValidationFailed)?;

    if campaign.revenue_share_percentage > crate::BPS_DENOMINATOR {
        return Err(Error::ValidationFailed);
    }

    let total_pool = get_revenue_pool(env, campaign_id);
    // Compute creator entitlement directly instead of as a residual from the
    // contributor pool. This avoids biasing creator payouts upward when
    // contributor-side division truncates (#386).
    let creator_share_bps =
        crate::BPS_DENOMINATOR as i128 - campaign.revenue_share_percentage as i128;
    let creator_share_total = total_pool
        .checked_mul(creator_share_bps)
        .and_then(|n| n.checked_div(crate::BPS_DENOMINATOR as i128))
        .ok_or(Error::Overflow)?;

    let already_claimed = get_creator_revenue_claimed(env, campaign_id);
    let claimable = creator_share_total - already_claimed;

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // Update state before the token transfer (CEI pattern) so that a
    // malicious token contract cannot re-enter and double-claim (#557).
    set_creator_revenue_claimed(
        env,
        campaign_id,
        already_claimed
            .checked_add(claimable)
            .ok_or(Error::Overflow)?,
    );

    // Token transfer happens after all state updates (CEI pattern).
    let client = campaign_token_client(env, campaign_id);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &claimable,
    );

    env.events().publish(
        ("creator_revenue_claimed", campaign_id, campaign.creator),
        claimable,
    );

    Ok(())
}
