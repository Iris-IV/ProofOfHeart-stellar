use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_token_client, get_campaign_or_error, require_active_campaign, require_not_paused,
};
use crate::storage::{
    bump_instance_ttl, decrement_contributor_count, get_admin,
    get_campaign_block_contribution_count, get_contribution, get_lifetime_contribution,
    get_max_contribution_per_transaction, get_personal_cap, get_pinned_campaign_token, get_token,
    get_top_contributor, get_total_raised_by_token, get_total_raised_global, has_personal_cap,
    increment_contributor_count, remove_contribution, remove_personal_cap, remove_revenue_claimed,
    set_campaign, set_campaign_block_contribution_count, set_campaign_fee_recipient,
    set_contribution, set_last_contribution_time, set_lifetime_contribution, set_personal_cap,
    set_top_contributor, set_total_raised_by_token, set_total_raised_global,
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

/// Rejects contributions that look like an attack: a single contribution above
/// `AUTO_PAUSE_SINGLE_CONTRIBUTION_BPS_THRESHOLD` of the goal, or more than
/// `AUTO_PAUSE_BURST_THRESHOLD` contributions to one campaign in a single
/// ledger. The offending transaction is rejected with `Error::ContractPaused`.
///
/// This does not pause the contract. A rejected transaction reverts its whole
/// storage footprint in Soroban, so any `AutoPaused` flag written here would be
/// rolled back with it; the only durable effect is the rejection itself. See
/// the module history for the removed (inert) auto-pause writes.
///
/// Fix #408: use checked arithmetic to avoid panic on overflow.
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
        // No event: this call returns Err, and Soroban rolls back events
        // emitted by a failed invocation along with its storage writes.
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
    let (last_ledger, block_count) = get_campaign_block_contribution_count(env, campaign_id);
    let block_count = if current_ledger == last_ledger {
        // #966: never wrap the per-campaign burst counter. A single ledger
        // cannot reach u32::MAX contributions in practice, but checked math
        // keeps the invariant explicit and returns a typed error instead of
        // panicking if state is ever forged or migrated.
        block_count.checked_add(1).ok_or(Error::Overflow)?
    } else {
        1
    };
    set_campaign_block_contribution_count(env, campaign_id, current_ledger, block_count);

    if block_count > crate::AUTO_PAUSE_BURST_THRESHOLD {
        // No event / no pause flag: this call returns Err, and Soroban rolls
        // back both events and storage writes from a failed invocation. The
        // persisted `block_count` above (written on the Ok path of prior
        // contributions) is what keeps rejecting further contributions in this
        // ledger; it resets on the next ledger.
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
    // #1054: track cumulative donor history for dynamic voting weight.
    crate::storage::add_donor_total_contributed(env, contributor, amount);

    if lifetime == 0 {
        increment_contributor_count(env, campaign_id);
    }

    // #1438: update both the platform-token global (for the token-update gate
    // and legacy callers) and the per-token total (so multi-token platforms
    // can read per-currency aggregates without mixing XLM into USDC).
    let campaign_token = crate::storage::get_campaign_token(env, campaign_id);
    let platform_token = get_token(env);
    if campaign_token == platform_token {
        // Platform-token campaign: update the legacy global counter so the
        // accept_token_update gate (which reads get_total_raised_global) still
        // works correctly.
        let total_raised = get_total_raised_global(env);
        set_total_raised_global(
            env,
            total_raised.checked_add(amount).ok_or(Error::Overflow)?,
        );
    }
    // Always update the per-token total, regardless of which token this is.
    let token_total = get_total_raised_by_token(env, &campaign_token);
    set_total_raised_by_token(
        env,
        &campaign_token,
        token_total.checked_add(amount).ok_or(Error::Overflow)?,
    );

    Ok(())
}

/// Shared accounting helper for remaining campaign capacity (#1010).
/// Returns `(accepted_amount, refund_amount)`.
/// Returns `FundingGoalReached` if the campaign is already fully funded.
fn calculate_contribution_capacity(
    campaign: &Campaign,
    amount: i128,
) -> Result<(i128, i128), Error> {
    let remaining_capacity = campaign
        .funding_goal
        .checked_sub(campaign.amount_raised)
        .ok_or(Error::Overflow)?;

    if remaining_capacity <= 0 {
        return Err(Error::FundingGoalReached);
    }

    if amount > remaining_capacity {
        let refund = amount
            .checked_sub(remaining_capacity)
            .ok_or(Error::Overflow)?;
        Ok((remaining_capacity, refund))
    } else {
        Ok((amount, 0))
    }
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

    if !campaign.is_verified() {
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

    // Burst guard and overflow detection on incoming amount (rejects attacks > 200% goal or overflow).
    check_burst_guard(env, campaign_id, &campaign, amount)?;

    // Issue 2 & Issue 4: Compute accepted_amount first before running caps.
    let (accepted_amount, refund_amount) = calculate_contribution_capacity(&campaign, amount)?;

    // Caps run against accepted_amount (the portion that will actually
    // be recorded), not the full requested amount (#1440, Issue 4).
    check_transaction_contribution_cap(env, accepted_amount)?;
    check_contribution_caps(&campaign, lifetime, accepted_amount)?;

    // Issue 3: Enforce personal cap against lifetime contributions.
    if let Some(cap) = get_personal_cap(env, campaign_id, &contributor) {
        if lifetime
            .checked_add(accepted_amount)
            .ok_or(Error::Overflow)?
            > cap
        {
            return Err(Error::ContributionCapExceeded);
        }
    }

    bump_instance_ttl(env);
    crate::storage::extend_contributor_ttl(env, campaign_id, &contributor);
    update_contribution_accounting(
        env,
        campaign_id,
        &contributor,
        &mut campaign,
        current,
        lifetime,
        accepted_amount,
    )?;

    let new_total = current
        .checked_add(accepted_amount)
        .ok_or(Error::Overflow)?;
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
    // Transfer the full amount from contributor to contract first
    client.transfer(&contributor, &env.current_contract_address(), &amount);

    // If there's a refund, send it back immediately
    if refund_amount > 0 {
        client.transfer(
            &env.current_contract_address(),
            &contributor,
            &refund_amount,
        );
        env.events().publish(
            ("contribution_refunded", campaign_id, contributor.clone()),
            refund_amount,
        );
    }

    env.events().publish(
        ("contribution_made", campaign_id, contributor),
        accepted_amount,
    );

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
    // Loop invariants hoisted out of the per-item path (#1172): the ledger
    // timestamp and the platform-token fallback are the same for every item,
    // so each is resolved at most once per call.
    let now = env.ledger().timestamp();
    let mut platform_token: Option<Address> = None;
    let mut seen: soroban_sdk::Map<u32, bool> = soroban_sdk::Map::new(env);
    let mut accepted_contributions: soroban_sdk::Vec<(u32, i128)> = soroban_sdk::Vec::new(env);
    for (campaign_id, amount) in contributions.iter() {
        if seen.get(campaign_id).is_some() {
            return Err(Error::ValidationFailed);
        }
        seen.set(campaign_id, true);
        let (token, contribution_total) = apply_batch_contribution(
            env,
            campaign_id,
            contributor.clone(),
            amount,
            now,
            &mut platform_token,
        )?;

        accepted_contributions.push_back((campaign_id, contribution_total));

        total = total
            .checked_add(contribution_total)
            .ok_or(Error::Overflow)?;

        let running: i128 = owed.get(token.clone()).unwrap_or(0);
        owed.set(
            token,
            running
                .checked_add(contribution_total)
                .ok_or(Error::Overflow)?,
        );
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
    for (campaign_id, accepted_amount) in accepted_contributions.iter() {
        env.events().publish(
            ("contribution_made", campaign_id, contributor.clone()),
            accepted_amount,
        );
    }

    env.events().publish(
        ("batch_contribution_made", contributor),
        (contributions.len(), total),
    );

    Ok(())
}

/// Applies one batch item in its own call frame. Keeping the campaign
/// validation and accounting locals out of `batch_contribute` reduces the
/// peak Wasm stack frame for multi-step batches without changing their atomic
/// semantics (#1187).
fn apply_batch_contribution(
    env: &Env,
    campaign_id: u32,
    contributor: Address,
    amount: i128,
    now: u64,
    platform_token: &mut Option<Address>,
) -> Result<(Address, i128), Error> {
    if amount <= 0 {
        return Err(Error::ContributionMustBePositive);
    }

    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    if !campaign.is_verified() {
        return Err(Error::CampaignNotVerified);
    }
    require_active_campaign(&campaign)?;
    if contributor == campaign.creator {
        return Err(Error::NotAuthorized);
    }
    if now > campaign.deadline {
        return Err(Error::DeadlinePassed);
    }

    let current = get_contribution(env, campaign_id, &contributor);
    let lifetime = get_lifetime_contribution(env, campaign_id, &contributor);

    // Issue 2 & Issue 4: Compute accepted_amount first before running caps.
    // Caps partial fill to remaining capacity or rejects with FundingGoalReached.
    let (accepted_amount, _refund_amount) = calculate_contribution_capacity(&campaign, amount)?;

    check_transaction_contribution_cap(env, accepted_amount)?;
    check_contribution_caps(&campaign, lifetime, accepted_amount)?;

    // Issue 3: Enforce personal cap against lifetime contributions.
    if let Some(cap) = get_personal_cap(env, campaign_id, &contributor) {
        if lifetime
            .checked_add(accepted_amount)
            .ok_or(Error::Overflow)?
            > cap
        {
            return Err(Error::ContributionCapExceeded);
        }
    }

    check_burst_guard(env, campaign_id, &campaign, accepted_amount)?;
    crate::storage::extend_contributor_ttl(env, campaign_id, &contributor);
    update_contribution_accounting(
        env,
        campaign_id,
        &contributor,
        &mut campaign,
        current,
        lifetime,
        accepted_amount,
    )?;

    let new_total = current
        .checked_add(accepted_amount)
        .ok_or(Error::Overflow)?;
    let is_new_top = match get_top_contributor(env, campaign_id) {
        Some(top_addr) if top_addr != contributor => {
            new_total > get_contribution(env, campaign_id, &top_addr)
        }
        _ => true,
    };
    if is_new_top {
        set_top_contributor(env, campaign_id, &contributor);
    }
    set_last_contribution_time(env, campaign_id, now);

    // Same answer as `get_campaign_token`, but the platform-token fallback is
    // read once per batch and reused across campaigns that inherit it (#1172).
    let token = match get_pinned_campaign_token(env, campaign_id) {
        Some(token) => token,
        None => platform_token.get_or_insert_with(|| get_token(env)).clone(),
    };

    Ok((token, accepted_amount))
}

pub(crate) fn claim_refund(env: &Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
    contributor.require_auth();
    require_not_paused(env)?;

    let mut campaign = get_campaign_or_error(env, campaign_id)?;

    let failed_due_to_goal = env.ledger().timestamp() > campaign.deadline
        && campaign.amount_raised < campaign.funding_goal;

    if !(campaign.is_cancelled() || failed_due_to_goal) {
        return Err(Error::ValidationFailed);
    }
    // #1478: defence in depth — never refund a campaign whose escrow has
    // already been partially paid out through milestone claims. Those funds
    // left the contract, so a full refund would be paid from other
    // campaigns' balances.
    if crate::milestones::has_claimed_milestone(env, campaign_id) {
        return Err(Error::RefundNotAvailable);
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
    if !campaign.is_cancelled() {
        campaign.effective_amount_raised = campaign
            .effective_amount_raised
            .checked_sub(amount)
            .ok_or(Error::Overflow)?;
        set_campaign(env, campaign_id, &campaign);
    }

    if !campaign.is_cancelled() {
        let campaign_token = crate::storage::get_campaign_token(env, campaign_id);
        let platform_token = get_token(env);
        if campaign_token == platform_token {
            let total_raised = get_total_raised_global(env);
            set_total_raised_global(
                env,
                total_raised.checked_sub(amount).ok_or(Error::Overflow)?,
            );
        }

        // #1438: also decrement the per-token total on refund.
        let token_total = get_total_raised_by_token(env, &campaign_token);
        set_total_raised_by_token(
            env,
            &campaign_token,
            token_total.checked_sub(amount).ok_or(Error::Overflow)?,
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

#[cfg(test)]
mod overflow_tests {
    use crate::tests::helpers::*;
    use crate::{storage, Category, Error};
    use soroban_sdk::{Address, String};

    fn verified_campaign(
        env: &soroban_sdk::Env,
        client: &ProofOfHeartClient,
        creator: &Address,
        max_contribution_per_user: i128,
    ) -> u32 {
        let campaign_id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(env, "Overflow Guard"),
            String::from_str(env, "Checked contribution arithmetic"),
            1_000,
            30,
            Category::Learner,
            false,
            0,
            max_contribution_per_user,
        ));
        client.verify_campaign(&campaign_id);
        campaign_id
    }

    /// `amount * BPS_DENOMINATOR` in the burst guard must surface as a typed
    /// error, never an overflow panic (#966).
    #[test]
    fn test_contribution_amount_multiplication_overflow_returns_error() {
        let (env, _admin, creator, contributor, _, _token, _token_admin, client) = setup_env();
        let campaign_id = verified_campaign(&env, &client, &creator, 0);

        let res = client.try_contribute(&campaign_id, &contributor, &i128::MAX);
        assert_eq!(res, Err(Ok(Error::Overflow)));
    }

    /// `lifetime + amount` in the cap check must use checked arithmetic even
    /// when stored state is forged or migrated (#966).
    #[test]
    fn test_lifetime_contribution_addition_overflow_returns_error() {
        let (env, _admin, creator, contributor, _, _token, _token_admin, client) = setup_env();
        let campaign_id = verified_campaign(&env, &client, &creator, 1_000);

        env.as_contract(&client.address, || {
            storage::set_lifetime_contribution(&env, campaign_id, &contributor, i128::MAX);
        });

        let res = client.try_contribute(&campaign_id, &contributor, &1);
        assert_eq!(res, Err(Ok(Error::Overflow)));
    }

    /// The per-ledger burst counter must not wrap past `u32::MAX`; the next
    /// contribution fails closed with `Overflow` instead of panicking (#966).
    #[test]
    fn test_burst_counter_overflow_returns_error() {
        let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();
        let campaign_id = verified_campaign(&env, &client, &creator, 1_000);
        token_admin.mint(&contributor, &2_000);
        client.contribute(&campaign_id, &contributor, &600);

        env.as_contract(&client.address, || {
            storage::set_campaign_block_contribution_count(
                &env,
                campaign_id,
                env.ledger().sequence(),
                u32::MAX,
            );
        });

        let res = client.try_contribute(&campaign_id, &contributor, &1);
        assert_eq!(res, Err(Ok(Error::Overflow)));
    }
}
