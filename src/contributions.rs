use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, require_active_campaign, require_not_paused, token_client,
};
use crate::storage::{
    bump_instance_ttl, decrement_contributor_count, get_campaign_block_contribution_count,
    get_contribution, get_lifetime_contribution, get_personal_cap, get_total_raised_global,
    increment_contributor_count, remove_contribution, remove_lifetime_contribution,
    remove_personal_cap, remove_revenue_claimed, set_campaign,
    set_campaign_block_contribution_count, set_contribution, set_lifetime_contribution,
    set_personal_cap, set_total_raised_global, DataKey,
};

pub(crate) fn contribute(
    env: &Env,
    campaign_id: u32,
    contributor: Address,
    amount: i128,
) -> Result<(), Error> {
    contributor.require_auth();
    require_not_paused(env)?;

    if amount <= 0 {
        return Err(Error::ContributionMustBePositive);
    }

    let mut campaign = get_campaign_or_error(env, campaign_id)?;

    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }

    require_active_campaign(&campaign)?;
    if contributor == campaign.creator {
        return Err(Error::NotAuthorized);
    }
    if env.ledger().timestamp() > campaign.deadline {
        return Err(Error::DeadlinePassed);
    }

    let current = get_contribution(env, campaign_id, &contributor);
    let lifetime = get_lifetime_contribution(env, campaign_id, &contributor);

    if campaign.max_contribution_per_user > 0
        && lifetime + amount > campaign.max_contribution_per_user
    {
        return Err(Error::ContributionCapExceeded);
    }

    if let Some(cap) = get_personal_cap(env, campaign_id, &contributor) {
        if current + amount > cap {
            return Err(Error::ContributionCapExceeded);
        }
    }

    // Fix #408: use checked arithmetic to avoid panic on overflow.
    // A huge contribution (> 200% of goal) triggers an auto-pause.
    let amount_bps = amount.checked_mul(10000).ok_or(Error::Overflow)?;
    let threshold = campaign
        .funding_goal
        .checked_mul(crate::AUTO_PAUSE_SINGLE_CONTRIBUTION_BPS_THRESHOLD)
        .ok_or(Error::Overflow)?;
    if amount_bps > threshold {
        env.storage().instance().set(&DataKey::AutoPaused, &true);
        env.events()
            .publish(("auto_paused",), ("huge_contribution", amount));
        return Err(Error::ContractPaused);
    }

    // Anomaly detection: Burst (> 10 tx/block per campaign)
    let current_ledger = env.ledger().sequence();
    let (last_ledger, mut block_count) = get_campaign_block_contribution_count(env, campaign_id);
    if current_ledger == last_ledger {
        block_count += 1;
    } else {
        block_count = 1;
    }
    set_campaign_block_contribution_count(env, campaign_id, current_ledger, block_count);

    if block_count > crate::AUTO_PAUSE_BURST_THRESHOLD {
        env.storage().instance().set(&DataKey::AutoPaused, &true);
        env.events()
            .publish(("auto_paused",), ("burst", block_count));
        return Err(Error::ContractPaused);
    }

    bump_instance_ttl(env);
    let client = token_client(env);
    client.transfer(&contributor, &env.current_contract_address(), &amount);

    campaign.amount_raised += amount;
    campaign.effective_amount_raised += amount;
    set_campaign(env, campaign_id, &campaign);
    set_contribution(env, campaign_id, &contributor, current + amount);
    set_lifetime_contribution(env, campaign_id, &contributor, lifetime + amount);

    if lifetime == 0 {
        increment_contributor_count(env, campaign_id);
    }

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(env, total_raised + amount);

    env.events()
        .publish(("contribution_made", campaign_id, contributor), amount);

    Ok(())
}

pub(crate) fn claim_refund(env: &Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
    contributor.require_auth();
    require_not_paused(env)?;

    let mut campaign = get_campaign_or_error(env, campaign_id)?;

    let failed_due_to_goal = env.ledger().timestamp() > campaign.deadline
        && campaign.amount_raised < campaign.funding_goal;

    if !(campaign.is_cancelled || failed_due_to_goal) {
        return Err(Error::ValidationFailed);
    }

    let amount = get_contribution(env, campaign_id, &contributor);
    if amount == 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);
    remove_contribution(env, campaign_id, &contributor);
    remove_lifetime_contribution(env, campaign_id, &contributor);
    remove_revenue_claimed(env, campaign_id, &contributor);
    remove_personal_cap(env, campaign_id, &contributor);

    decrement_contributor_count(env, campaign_id);

    campaign.effective_amount_raised = campaign
        .effective_amount_raised
        .checked_sub(amount)
        .ok_or(Error::Overflow)?;
    set_campaign(env, campaign_id, &campaign);

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised.checked_sub(amount).ok_or(Error::Overflow)?,
    );

    let client = token_client(env);
    client.transfer(&env.current_contract_address(), &contributor, &amount);

    env.events()
        .publish(("refund_claimed", campaign_id, contributor), amount);

    Ok(())
}

pub(crate) fn set_personal_cap_fn(
    env: &Env,
    campaign_id: u32,
    contributor: Address,
    amount: i128,
) -> Result<(), Error> {
    contributor.require_auth();
    if amount < 0 {
        return Err(Error::ValidationFailed);
    }
    let campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;
    if campaign.max_contribution_per_user > 0 && amount > campaign.max_contribution_per_user {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    set_personal_cap(env, campaign_id, &contributor, amount);
    env.events().publish(
        ("personal_cap_set", campaign_id, contributor.clone()),
        amount,
    );
    Ok(())
}
