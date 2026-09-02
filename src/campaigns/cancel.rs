use soroban_sdk::{Address, Env, String};

use crate::bookmarks::prune_bookmarks_for_campaign;
use crate::errors::Error;
use crate::lifecycle::{
    assert_admin, campaign_token_client, get_campaign_or_error, get_creator_campaign,
    require_active_campaign, require_not_paused, transition, CampaignState,
};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_revenue_pool, get_total_raised_global,
    increment_cancelled_campaign_count, remove_voting_state, set_campaign, set_revenue_pool,
    set_total_raised_global,
};

pub(crate) fn cancel_campaign(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    require_active_campaign(&campaign)?;
    if campaign.funds_withdrawn {
        return Err(Error::CancellationNotAllowed);
    }
    // Prevent rug-pull: reject cancellation after the funding goal has been met but
    // funds have not yet been withdrawn.
    if campaign.amount_raised >= campaign.funding_goal {
        return Err(Error::GoalMetCancellationNotAllowed);
    }

    transition(CampaignState::of(&campaign), CampaignState::Cancelled)?;

    bump_instance_ttl(env);

    // CEI (#795): every state write happens before the token transfer below.
    //
    // Previously the revenue-pool refund was transferred first and the pool
    // zeroed afterwards, with the cancellation flags written after that. A
    // token contract swapped for a malicious one via `accept_token_update`
    // could re-enter `cancel_campaign` during that transfer and find the
    // campaign still active with the pool still non-zero, draining it once
    // per re-entry.
    let revenue_pool = get_revenue_pool(env, campaign_id);
    if revenue_pool > 0 {
        set_revenue_pool(env, campaign_id, 0);
    }

    // #818: Decrement total_raised_global by the full amount_raised upfront so
    // that the global stat reflects the cancellation immediately — not after
    // every contributor individually calls claim_refund. Without this,
    // total_raised_global is overstated until all refunds are claimed, which
    // blocks accept_token_update (which requires total_raised_global == 0)
    // forever if any dust refund is never claimed.
    //
    // claim_refund skips the total_raised_global decrement for cancelled
    // campaigns to avoid double-counting.
    if campaign.amount_raised > 0 {
        let total = get_total_raised_global(env);
        set_total_raised_global(
            env,
            total
                .checked_sub(campaign.amount_raised)
                .ok_or(Error::Overflow)?,
        );
    }

    // #819: Zero effective_amount_raised so indexers and dashboards report 0
    // live contributions on a dead campaign immediately, even before refunds.
    campaign.effective_amount_raised = 0;
    campaign.is_cancelled = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    prune_bookmarks_for_campaign(env, campaign_id);
    decrement_active_campaign_count(env);
    increment_cancelled_campaign_count(env);

    // Interaction last. A re-entrant call now fails `require_active_campaign`,
    // and even if it did not, the pool reads as zero.
    if revenue_pool > 0 {
        let client = campaign_token_client(env, campaign_id);
        client.transfer(
            &env.current_contract_address(),
            &campaign.creator,
            &revenue_pool,
        );
        env.events()
            .publish(("revenue_pool_refunded", campaign_id), revenue_pool);
    }

    env.events().publish(
        ("campaign_cancelled", campaign_id, campaign.creator.clone()),
        campaign.amount_raised,
    );

    Ok(())
}

/// Admin-initiated cancellation for fraud response (#508, #858). Unlike
/// `cancel_campaign`, this is not restricted to the creator and does not
/// apply the goal-met anti-rug-pull guard — an admin must be able to stop a
/// verified fraudulent campaign even after it has hit its funding goal,
/// without pausing the entire platform.
///
/// Follows CEI pattern: refunds any creator revenue_pool deposit back to the
/// creator and zeroes the pool before emitting cancellation events.
/// Contributors reclaim their own funds via the existing `claim_refund`,
/// which already treats any `is_cancelled` campaign as refund-eligible.
pub(crate) fn admin_cancel_campaign(
    env: &Env,
    admin: Address,
    campaign_id: u32,
    reason: String,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    require_not_paused(env)?;

    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;
    if campaign.funds_withdrawn {
        return Err(Error::CancellationNotAllowed);
    }

    if reason.len() == 0 || reason.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN {
        return Err(Error::ValidationFailed);
    }

    transition(CampaignState::of(&campaign), CampaignState::Cancelled)?;

    bump_instance_ttl(env);

    let revenue_pool = get_revenue_pool(env, campaign_id);
    if revenue_pool > 0 {
        set_revenue_pool(env, campaign_id, 0);
    }

    // #818: Same upfront decrement as creator cancel — global stat must not be
    // overstated while unclaimed refunds exist.
    if campaign.amount_raised > 0 {
        let total = get_total_raised_global(env);
        set_total_raised_global(
            env,
            total.checked_sub(campaign.amount_raised).ok_or(Error::Overflow)?,
        );
    }

    // #819: Zero effective_amount_raised so indexers and dashboards report 0
    // live contributions on a dead campaign immediately, even before refunds.
    campaign.effective_amount_raised = 0;
    campaign.is_cancelled = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    prune_bookmarks_for_campaign(env, campaign_id);
    decrement_active_campaign_count(env);
    increment_cancelled_campaign_count(env);

    if revenue_pool > 0 {
        let client = campaign_token_client(env, campaign_id);
        client.transfer(
            &env.current_contract_address(),
            &campaign.creator,
            &revenue_pool,
        );
        env.events()
            .publish(("revenue_pool_refunded", campaign_id), revenue_pool);
    }

    env.events().publish(
        ("campaign_admin_cancelled", campaign_id, admin),
        (
            campaign.creator.clone(),
            reason,
            campaign.effective_amount_raised,
            revenue_pool,
        ),
    );

    Ok(())
}
