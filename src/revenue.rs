use soroban_sdk::{token, Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, get_creator_campaign, require_not_paused, require_revenue_sharing,
    token_client,
};
use crate::storage::{
    bump_instance_ttl, get_contribution, get_creator_revenue_claimed, get_revenue_claimed,
    get_revenue_pool, get_token, set_creator_revenue_claimed, set_revenue_claimed,
    set_revenue_pool,
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
    let token_addr = get_token(env);
    let client = token::Client::new(env, &token_addr);
    client.transfer(&campaign.creator, &env.current_contract_address(), &amount);

    let current_pool = get_revenue_pool(env, campaign_id);
    set_revenue_pool(env, campaign_id, current_pool + amount);

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
        .and_then(|n| n.checked_div(10000))
        .ok_or(Error::Overflow)?;
    let already_claimed = get_revenue_claimed(env, campaign_id, &contributor);
    let claimable = total_due - already_claimed;

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // Transfer tokens BEFORE updating state to prevent balance wipe on failed transfer
    let client = token_client(env);
    client.transfer(&env.current_contract_address(), &contributor, &claimable);

    // Update state only after successful external interaction
    set_revenue_claimed(env, campaign_id, &contributor, already_claimed + claimable);

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

    if campaign.revenue_share_percentage > 10000 {
        return Err(Error::ValidationFailed);
    }

    let total_pool = get_revenue_pool(env, campaign_id);
    // Compute creator entitlement directly instead of as a residual from the
    // contributor pool. This avoids biasing creator payouts upward when
    // contributor-side division truncates (#386).
    let creator_share_bps = 10000i128 - campaign.revenue_share_percentage as i128;
    let creator_share_total = total_pool
        .checked_mul(creator_share_bps)
        .and_then(|n| n.checked_div(10000))
        .ok_or(Error::Overflow)?;

    let already_claimed = get_creator_revenue_claimed(env, campaign_id);
    let claimable = creator_share_total - already_claimed;

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // Transfer tokens BEFORE updating state to prevent balance wipe on failed transfer
    let client = token_client(env);
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &claimable,
    );

    set_creator_revenue_claimed(env, campaign_id, already_claimed + claimable);

    env.events().publish(
        ("creator_revenue_claimed", campaign_id, campaign.creator),
        claimable,
    );

    Ok(())
}
