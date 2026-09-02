use soroban_sdk::Env;

use crate::errors::Error;
use crate::lifecycle::{
    campaign_token_client, get_campaign_or_error, get_creator_campaign, require_not_paused,
    transition, CampaignState,
};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_admin, get_campaign_fee_recipient,
    get_campaign_milestones, get_campaign_reserve, get_campaign_vesting, get_platform_fee,
    get_total_raised_global, get_withdraw_release_delay_days, get_withdraw_reserve_percentage,
    set_campaign, set_campaign_reserve, set_total_raised_global, set_withdraw_release_delay_days,
    set_withdraw_reserve_percentage,
};
use crate::types::CampaignReserve;

pub(crate) fn withdraw_funds(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;
    // Milestone campaigns must use claim_milestone proportional flow (#783).
    if !get_campaign_milestones(env, campaign_id).is_empty() {
        return Err(Error::ValidationFailed);
    }

    // Defense-in-depth: re-check verification even though `contribute`
    // already requires it, in case a future code path seeds an unverified
    // campaign directly (admin grant, migration, etc.).
    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }

    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    // Withdrawal is only allowed after the campaign deadline has passed.
    // A verified campaign can otherwise be withdrawn immediately,
    // bypassing the intended funding window (#854).
    if env.ledger().timestamp() <= campaign.deadline {
        return Err(Error::DeadlineNotPassed);
    }
    if campaign.funds_withdrawn {
        return Err(Error::FundsAlreadyWithdrawn);
    }
    if campaign.amount_raised == 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    if campaign.amount_raised < campaign.funding_goal {
        return Err(Error::FundingGoalNotReached);
    }

    transition(CampaignState::of(&campaign), CampaignState::Withdrawn)?;

    bump_instance_ttl(env);
    let platform_fee = campaign
        .fee_override
        .unwrap_or_else(|| get_platform_fee(env));
    // Refunds reduce `effective_amount_raised` but retain `amount_raised` as an
    // audit total. Fees and payouts must use the remaining escrowed amount.
    // Ceiling division uses checked arithmetic to avoid an overflow panic (#408).
    let fee_amount = campaign
        .effective_amount_raised
        .checked_mul(platform_fee as i128)
        .and_then(|n| n.checked_add(crate::BPS_CEIL_OFFSET))
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;
    let total_after_fee = campaign.effective_amount_raised - fee_amount;

    // Use per-campaign vesting params snapshotted at creation, falling back
    // to the global defaults for campaigns created before the snapshot was
    // introduced (#466).
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

    // Update state before the token transfer (CEI pattern) so that a
    // malicious token contract cannot re-enter and double-claim (#557).
    campaign.funds_withdrawn = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    decrement_active_campaign_count(env);

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

    // Token transfers happen after all state updates (CEI pattern).
    //
    // The platform fee goes to the recipient snapshotted on the campaign's
    // first contribution (#800), not to whoever is admin right now — an admin
    // transfer between contribution and withdrawal must not redirect a fee
    // that was earned under the previous admin. Campaigns funded before this
    // snapshot existed have no key and fall back to the current admin.
    let fee_recipient =
        get_campaign_fee_recipient(env, campaign_id).unwrap_or_else(|| get_admin(env));
    let client = campaign_token_client(env, campaign_id);

    client.transfer(&env.current_contract_address(), &fee_recipient, &fee_amount);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &creator_amount,
    );

    env.events().publish(
        ("withdrawal", campaign_id, campaign.creator.clone()),
        (
            campaign.effective_amount_raised,
            fee_amount,
            reserve_amount,
            creator_amount,
        ),
    );

    // Emitted separately from `withdrawal` so its shape stays stable: the fee
    // recipient is the snapshot from the first contribution (#800), which is
    // not necessarily the current admin.
    env.events().publish(
        ("withdrawal_fee_paid", campaign_id),
        (fee_recipient, fee_amount),
    );

    let payout_marker = env.ledger().sequence();
    crate::storage::set_campaign_payout_marker(env, campaign_id, payout_marker);
    env.events().publish(("payout_marker", campaign_id), payout_marker);

    if reserve_amount > 0 {
        env.events()
            .publish(("reserve_withheld", campaign_id), reserve_amount);
    }

    Ok(())
}

pub(crate) fn withdraw_reserve(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut reserve = get_campaign_reserve(env, campaign_id).ok_or(Error::NoFundsToWithdraw)?;
    require_not_paused(env)?;
    if reserve.released {
        return Err(Error::FundsAlreadyWithdrawn);
    }
    if env.ledger().timestamp() < reserve.release_timestamp {
        return Err(Error::ValidationFailed);
    }

    let campaign = get_campaign_or_error(env, campaign_id)?;

    // Defense-in-depth: only release reserve on campaigns that have
    // actually withdrawn funds. A migration-planted reserve on a
    // non-withdrawn campaign must not be drainable.
    //
    // Uses ValidationFailed (same error as the release-timestamp check above)
    // because the existing error variants NoFundsToWithdraw (no reserve at
    // all) and FundingGoalNotReached (goal not met) describe different
    // failure modes — this guard is about the campaign never having called
    // withdraw_funds, which is a distinct invariant violation.
    if !campaign.funds_withdrawn {
        return Err(Error::ValidationFailed);
    }

    campaign.creator.require_auth();

    // Update state before the token transfer (CEI pattern) so that a
    // malicious token contract cannot re-enter and double-claim (#557).
    reserve.released = true;
    set_campaign_reserve(env, campaign_id, &reserve);

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised
            .checked_sub(reserve.amount)
            .ok_or(Error::Overflow)?,
    );

    // Token transfer happens after all state updates (CEI pattern).
    let client = campaign_token_client(env, campaign_id);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &reserve.amount,
    );

    env.events().publish(
        ("reserve_released", campaign_id, campaign.creator),
        reserve.amount,
    );

    Ok(())
}

pub(crate) fn set_vesting_params(
    env: &Env,
    admin: soroban_sdk::Address,
    delay_days: u64,
    reserve_bps: u32,
) -> Result<(), Error> {
    crate::lifecycle::assert_admin(env, &admin)?;
    require_not_paused(env)?;
    if reserve_bps > crate::BPS_DENOMINATOR || delay_days > 365 {
        return Err(Error::ValidationFailed);
    }
    if delay_days == 0 && reserve_bps > 0 {
        return Err(Error::InvalidVestingDelay);
    }

    let old_delay_days = get_withdraw_release_delay_days(env);
    let old_reserve_bps = get_withdraw_reserve_percentage(env);

    set_withdraw_release_delay_days(env, delay_days);
    set_withdraw_reserve_percentage(env, reserve_bps);

    if delay_days == 0 && reserve_bps == 0 {
        env.events().publish(
            ("vesting_disabled", admin),
            (old_delay_days, delay_days, old_reserve_bps, reserve_bps),
        );
    } else {
        env.events().publish(
            ("vesting_params_updated", admin),
            (old_delay_days, delay_days, old_reserve_bps, reserve_bps),
        );
    }

    Ok(())
}
