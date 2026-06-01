use soroban_sdk::Env;

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, get_creator_campaign, require_not_paused, token_client,
};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_admin, get_campaign_reserve,
    get_platform_fee, get_total_raised_global, get_withdraw_release_delay_days,
    get_withdraw_reserve_percentage, set_campaign, set_campaign_reserve, set_total_raised_global,
    set_withdraw_release_delay_days, set_withdraw_reserve_percentage,
};
use crate::types::CampaignReserve;

pub(crate) fn withdraw_funds(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    // Defense-in-depth: re-check verification even though `contribute`
    // already requires it, in case a future code path seeds an unverified
    // campaign directly (admin grant, migration, etc.).
    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }

    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
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

    bump_instance_ttl(env);
    let platform_fee = campaign
        .fee_override
        .unwrap_or_else(|| get_platform_fee(env));
    // Ceiling division: ceil(a / b) = (a + b - 1) / b
    let fee_amount = (campaign.amount_raised * (platform_fee as i128) + 9999) / 10000;
    let total_after_fee = campaign.amount_raised - fee_amount;

    let reserve_bps = get_withdraw_reserve_percentage(env);
    let reserve_amount = (total_after_fee * (reserve_bps as i128) + 9999) / 10000;
    let creator_amount = total_after_fee - reserve_amount;

    // Execute token transfers BEFORE marking campaign as withdrawn to prevent stuck state
    let admin_addr = get_admin(env);
    let client = token_client(env);

    client.transfer(&env.current_contract_address(), &admin_addr, &fee_amount);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &creator_amount,
    );

    // Update state only after successful external interactions
    campaign.funds_withdrawn = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    decrement_active_campaign_count(env);

    if reserve_amount > 0 {
        let delay_days = get_withdraw_release_delay_days(env);
        let release_timestamp = env
            .ledger()
            .timestamp()
            .checked_add(delay_days * 86400)
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
            .checked_sub(campaign.amount_raised - reserve_amount)
            .ok_or(Error::Overflow)?,
    );

    env.events().publish(
        ("withdrawal", campaign_id, campaign.creator.clone()),
        creator_amount,
    );

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
    campaign.creator.require_auth();

    // Transfer funds BEFORE marking reserve as released to prevent stuck state
    let client = token_client(env);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &reserve.amount,
    );

    // Update state only after successful external interaction
    reserve.released = true;
    set_campaign_reserve(env, campaign_id, &reserve);

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised
            .checked_sub(reserve.amount)
            .ok_or(Error::Overflow)?,
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
    if reserve_bps > 10000 || delay_days > 365 {
        return Err(Error::ValidationFailed);
    }

    set_withdraw_release_delay_days(env, delay_days);
    set_withdraw_reserve_percentage(env, reserve_bps);

    env.events()
        .publish(("vesting_params_updated", admin), (delay_days, reserve_bps));

    Ok(())
}
