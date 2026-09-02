use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_token_client, get_campaign_or_error, require_active_campaign, require_not_paused,
};
use crate::storage::{
    bump_instance_ttl, decrement_contributor_count, get_admin,
    get_campaign_block_contribution_count, get_campaign_token, get_contribution,
    get_lifetime_contribution, get_max_contribution_per_transaction, get_personal_cap,
    get_top_contributor, get_total_raised_global, increment_contributor_count, remove_contribution,
    remove_personal_cap, remove_revenue_claimed, set_campaign,
    set_campaign_block_contribution_count, set_campaign_fee_recipient, set_contribution,
    set_last_contribution_time, set_lifetime_contribution, set_personal_cap, set_top_contributor,
    set_total_raised_global, AdminKey,
};
use crate::types::Campaign;

/// `max_contribution_per_user == 0` is an explicit "no cap" sentinel, not
/// "0 tokens allowed" — see the doc comment on `Campaign::max_contribution_per_user`
/// (#530). `create_campaign` rejects negative values, so `0` and positive
/// values are the only inputs the cap check needs to handle here.
fn check_contribution_caps(
    campaign: &Campaign,
    current_lifetime_contribution: i128,
    amount: i128,
) -> Result<(), Error> {
    if campaign.max_contribution_per_user > 0
        && current_lifetime_contribution
            .checked_add(amount)
            .ok_or(Error::Overflow)?
            > campaign.max_contribution_per_user
    {
        return Err(Error::ContributionCapExceeded);
    }
    Ok(())
}

fn check_transaction_contribution_cap(env: &Env, amount: i128) -> Result<(), Error> {
    let cap = get_max_contribution_per_transaction(env);
    if cap > 0 && amount > cap {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

/// Fix #408: use checked arithmetic to avoid panic on overflow.
/// A huge contribution (> 200% of goal) triggers an auto-pause.
fn check_burst_guard(
    env: &Env,
    campaign_id: u32,
    campaign: &Campaign,
    amount: i128,
) -> Result<(), Error> {
    let amount_bps = amount
        .checked_mul(crate::BPS_DENOMINATOR as i128)
        .ok_or(Error::Overflow)?;
    let threshold = campaign
        .funding_goal
        .checked_mul(crate::AUTO_PAUSE_SINGLE_CONTRIBUTION_BPS_THRESHOLD)
        .ok_or(Error::Overflow)?;
    if amount_bps > threshold {
        env.storage().instance().set(&AdminKey::AutoPaused, &true);
        env.events()
            .publish(("auto_paused",), ("huge_contribution", amount));
        return Err(Error::ContractPaused);
    }

    // #535: skip the burst-count ledger read/write entirely for campaigns
    // that haven't raised a meaningful share of their goal yet — a burst
    // isn't possible to meaningfully detect (or worth guarding against) on a
    // campaign that's still near-empty, so this is a wasted read on the
    // common happy path.
    let raised_bps = campaign
        .amount_raised
        .checked_mul(crate::BPS_DENOMINATOR as i128)
        .ok_or(Error::Overflow)?;
    let burst_check_threshold = campaign
        .funding_goal
        .checked_mul(crate::AUTO_PAUSE_BURST_CHECK_MIN_RAISED_BPS)
        .ok_or(Error::Overflow)?;
    if raised_bps <= burst_check_threshold {
        return Ok(());
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
        env.storage().instance().set(&AdminKey::AutoPaused, &true);
        env.events()
            .publish(("auto_paused",), ("burst", block_count));
        return Err(Error::ContractPaused);
    }

    Ok(())
}

fn update_contribution_accounting(
    env: &Env,
    campaign_id: u32,
    contributor: &Address,
    campaign: &mut Campaign,
    current: i128,
    lifetime: i128,
    amount: i128,
) -> Result<(), Error> {
    // Snapshot the platform-fee recipient on the first contribution (#800).
    //
    // The fee is only moved at `withdraw_funds` time, which can be long after
    // the money came in. If an admin transfer completes in that window, the
    // fee would otherwise land with whoever is admin at withdrawal rather than
    // the admin who was in place when contributors funded the campaign.
    // Pinning it here — on the first contribution, when `amount_raised` is
    // still zero — ties the fee to the admin the campaign was funded under.
    // `withdraw_funds` falls back to the current admin when this key is absent
    // (campaigns created before this change, or never contributed to).
    if campaign.amount_raised == 0 {
        set_campaign_fee_recipient(env, campaign_id, &get_admin(env));
    }

    campaign.amount_raised = campaign
        .amount_raised
        .checked_add(amount)
        .ok_or(Error::Overflow)?;
    campaign.effective_amount_raised = campaign
        .effective_amount_raised
        .checked_add(amount)
        .ok_or(Error::Overflow)?;
    set_campaign(env, campaign_id, campaign);
    set_contribution(
        env,
        campaign_id,
        contributor,
        current.checked_add(amount).ok_or(Error::Overflow)?,
    );
    set_lifetime_contribution(
        env,
        campaign_id,
        contributor,
        lifetime.checked_add(amount).ok_or(Error::Overflow)?,
    );

    if lifetime == 0 {
        increment_contributor_count(env, campaign_id);
    }

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised.checked_add(amount).ok_or(Error::Overflow)?,
    );

    Ok(())
}

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

    check_transaction_contribution_cap(env, amount)?;
    check_contribution_caps(&campaign, lifetime, amount)?;

    if let Some(cap) = get_personal_cap(env, campaign_id, &contributor) {
        if current.checked_add(amount).ok_or(Error::Overflow)? > cap {
            return Err(Error::ContributionCapExceeded);
        }
    }

    check_burst_guard(env, campaign_id, &campaign, amount)?;

    bump_instance_ttl(env);
    crate::storage::extend_contributor_ttl(env, campaign_id, &contributor);
    update_contribution_accounting(
        env,
        campaign_id,
        &contributor,
        &mut campaign,
        current,
        lifetime,
        amount,
    )?;

    let new_total = current.checked_add(amount).ok_or(Error::Overflow)?;
    let is_new_top = match get_top_contributor(env, campaign_id) {
        Some(top_addr) if top_addr != contributor => {
            new_total > get_contribution(env, campaign_id, &top_addr)
        }
        _ => true,
    };
    if is_new_top {
        set_top_contributor(env, campaign_id, &contributor);
    }
    set_last_contribution_time(env, campaign_id, env.ledger().timestamp());

    let client = campaign_token_client(env, campaign_id);
    client.transfer(&contributor, &env.current_contract_address(), &amount);

    env.events()
        .publish(("contribution_made", campaign_id, contributor), amount);

    Ok(())
}

/// Contributes to multiple campaigns in one call, moving the combined amount
/// in a single token transfer (#518). Auth and pause are checked once up
/// front; each `(campaign_id, amount)` item is then validated with the same
/// rules `contribute` uses, and its accounting is applied immediately so a
/// campaign repeated later in the same batch sees the earlier item's updated
/// totals. The aggregate transfer happens last — if any item fails, the
/// whole call reverts atomically and no accounting persists.
pub(crate) fn batch_contribute(
    env: &Env,
    contributor: Address,
    contributions: soroban_sdk::Vec<(u32, i128)>,
) -> Result<(), Error> {
    contributor.require_auth();
    require_not_paused(env)?;

    if contributions.is_empty() || contributions.len() > crate::MAX_BATCH_CONTRIBUTE_SIZE {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);

    // Amount owed per token. A batch may span campaigns denominated in
    // different currencies (#784), so the aggregate transfer is per token
    // rather than a single lump sum. Grouping keeps one transfer per distinct
    // currency instead of one per contribution.
    let mut owed: soroban_sdk::Map<Address, i128> = soroban_sdk::Map::new(env);
    let mut total: i128 = 0;
    let mut seen: soroban_sdk::Map<u32, bool> = soroban_sdk::Map::new(env);
    for (campaign_id, amount) in contributions.iter() {
        if seen.get(campaign_id).is_some() {
            return Err(Error::ValidationFailed);
        }
        seen.set(*campaign_id, true);

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

        check_transaction_contribution_cap(env, amount)?;
        check_contribution_caps(&campaign, lifetime, amount)?;

        if let Some(cap) = get_personal_cap(env, campaign_id, &contributor) {
            if current.checked_add(amount).ok_or(Error::Overflow)? > cap {
                return Err(Error::ContributionCapExceeded);
            }
        }

        check_burst_guard(env, campaign_id, &campaign, amount)?;

        crate::storage::extend_contributor_ttl(env, campaign_id, &contributor);
        update_contribution_accounting(
            env,
            campaign_id,
            &contributor,
            &mut campaign,
            current,
            lifetime,
            amount,
        )?;

        total = total.checked_add(amount).ok_or(Error::Overflow)?;

        let token = get_campaign_token(env, campaign_id);
        let running = owed.get(token.clone()).unwrap_or(0);
        owed.set(token, running.checked_add(amount).ok_or(Error::Overflow)?);

    }

    // Transfers happen here, after accounting and before any events are
    // published, so a failed transfer leaves no partial event stream.
    for (token, amount) in owed.iter() {
        soroban_sdk::token::Client::new(env, &token).transfer(
            &contributor,
            &env.current_contract_address(),
            &amount,
        );
    }

    // Publish per-contribution events only after the aggregate transfer has
    // succeeded, so a failed transfer leaves no partial event stream.
    for (campaign_id, amount) in contributions.iter() {
        env.events().publish(
            ("contribution_made", campaign_id, contributor.clone()),
            amount,
        );
    }

    env.events().publish(
        ("batch_contribution_made", contributor),
        (contributions.len(), total),
    );

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
    remove_revenue_claimed(env, campaign_id, &contributor);
    remove_personal_cap(env, campaign_id, &contributor);

    decrement_contributor_count(env, campaign_id)?;

    // #819: For cancelled campaigns effective_amount_raised was already zeroed
    // at cancel time. Only decrement here for the failed-funding path
    // (deadline passed, goal not met).
    if !campaign.is_cancelled {
        campaign.effective_amount_raised = campaign
            .effective_amount_raised
            .checked_sub(amount)
            .ok_or(Error::Overflow)?;
        set_campaign(env, campaign_id, &campaign);
    }

    if !campaign.is_cancelled {
        let total_raised = get_total_raised_global(env);
        set_total_raised_global(
            env,
            total_raised.checked_sub(amount).ok_or(Error::Overflow)?,
        );
    }

    let client = campaign_token_client(env, campaign_id);
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
    let lifetime = get_lifetime_contribution(env, campaign_id, &contributor);
    if amount < lifetime {
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

/// Removes the contributor's personal contribution cap for a campaign (#503).
/// Mirrors `set_personal_cap_fn`'s guards: the caller must authorize and the
/// campaign must still be active. Removing a cap that is not set is an error
/// rather than a silent no-op, so indexers can rely on `personal_cap_removed`
/// meaning a cap actually existed.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignNotActive` - The campaign is cancelled, withdrawn, or otherwise inactive.
/// * `PersonalCapNotFound` - The contributor has no personal cap set on this campaign.
pub(crate) fn remove_personal_cap_fn(
    env: &Env,
    campaign_id: u32,
    contributor: Address,
) -> Result<(), Error> {
    contributor.require_auth();
    let campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;
    if !has_personal_cap(env, campaign_id, &contributor) {
        return Err(Error::PersonalCapNotFound);
    }
    bump_instance_ttl(env);
    remove_personal_cap(env, campaign_id, &contributor);
    env.events().publish(
        ("personal_cap_removed", campaign_id, contributor.clone()),
        (),
    );
    Ok(())
}
