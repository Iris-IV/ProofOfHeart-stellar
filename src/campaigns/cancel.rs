use soroban_sdk::{Address, Env, String};

use crate::bookmarks::prune_bookmarks_for_campaign;
use crate::errors::Error;
use crate::lifecycle::{
    assert_admin, campaign_token_client, get_campaign_or_error, get_creator_campaign,
    require_active_campaign, require_not_paused, transition, CampaignState,
};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_cancellation_penalty_bps,
    get_revenue_pool, get_total_raised_global, hash_text, increment_cancelled_campaign_count,
    remove_creator_title_index, remove_emergency_withdrawal, remove_top_contributor,
    remove_voting_state, set_campaign, set_revenue_pool, set_total_raised_global,
};
use crate::types::MaybePendingCreator;

/// Upper bound for the voluntary-cancellation penalty (500 bps = 5%, #1052).
pub const MAX_CANCELLATION_PENALTY_BPS: u32 = 500;

/// Validates a cancellation penalty value in basis points (#1052).
pub fn validate_penalty_bps(penalty_bps: u32) -> Result<(), Error> {
    if penalty_bps > MAX_CANCELLATION_PENALTY_BPS {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

/// Computes the penalty owed on voluntary cancellation (#1052).
///
/// Returns 0 when nothing was raised or the penalty is disabled.
pub fn calculate_cancellation_penalty(amount_raised: i128, penalty_bps: u32) -> i128 {
    if amount_raised <= 0 || penalty_bps == 0 {
        return 0;
    }
    amount_raised
        .checked_mul(penalty_bps as i128)
        .and_then(|n| n.checked_div(crate::BPS_DENOMINATOR as i128))
        .unwrap_or(0)
        .max(0)
}

pub(crate) fn cancel_campaign(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    require_active_campaign(&campaign)?;
    if campaign.funds_withdrawn() {
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
    campaign.pending_creator = MaybePendingCreator::None;
    campaign.pending_creator_expiry = 0;
    campaign.set_cancelled(true);
    campaign.set_active(false);
    let title_hash = hash_text(env, &campaign.title);
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    // #863: a cancelled campaign is terminal and every contribution to it is
    // refundable, so there is no winner to report. Clear the marker here —
    // alongside the other per-campaign side state retired at cancellation —
    // so it cannot pin the address in storage or leak into
    // `get_campaign_stats` for a campaign whose refunds are never claimed.
    remove_top_contributor(env, campaign_id);
    // #989: Clean up obsolete per-campaign persistent keys to reclaim storage rent.
    remove_emergency_withdrawal(env, campaign_id);
    remove_creator_title_index(env, &campaign.creator, &title_hash);
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

    // #1052: voluntary-cancellation penalty, charged creator -> admin to
    // reimburse platform costs. Defaults to 0 (disabled) so existing
    // cancel/refund flows are unaffected unless the admin configures it.
    // State above is already committed (CEI); contributor refunds are untouched.
    let penalty =
        calculate_cancellation_penalty(campaign.amount_raised, get_cancellation_penalty_bps(env));
    if penalty > 0 {
        let client = campaign_token_client(env, campaign_id);
        client.transfer(&campaign.creator, &crate::storage::get_admin(env), &penalty);
        env.events()
            .publish(("cancellation_penalty_collected", campaign_id), penalty);
    }

    env.events().publish(
        ("campaign_cancelled", campaign_id, campaign.creator.clone()),
        campaign.amount_raised,
    );

    Ok(())
}

/// Admin-initiated cancellation for fraud response (#508). Unlike
// `cancel_campaign`, this is not restricted to the creator and does not
// apply the goal-met anti-rug-pull guard — an admin must be able to stop a
// verified fraudulent campaign even after it has hit its funding goal,
// without pausing the entire platform. It also deliberately does not
// auto-refund any revenue pool balance to the (presumed fraudulent)
// creator, unlike creator self-cancel; that balance is left in the
// contract with no other exit path — a known follow-up, not solved here.
// Contributors reclaim their own funds via the existing `claim_refund`,
// which already treats any `is_cancelled` campaign as refund-eligible.
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
    if campaign.funds_withdrawn() {
        return Err(Error::CancellationNotAllowed);
    }

    if reason.is_empty() || reason.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN {
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
            total
                .checked_sub(campaign.amount_raised)
                .ok_or(Error::Overflow)?,
        );
    }

    campaign.effective_amount_raised = 0;
    campaign.pending_creator = MaybePendingCreator::None;
    campaign.pending_creator_expiry = 0;
    campaign.set_cancelled(true);
    campaign.set_active(false);
    let title_hash = hash_text(env, &campaign.title);
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    // #863: same terminal-state cleanup as creator cancel — an admin-cancelled
    // campaign has no winner either.
    remove_top_contributor(env, campaign_id);
    remove_emergency_withdrawal(env, campaign_id);
    remove_creator_title_index(env, &campaign.creator, &title_hash);
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

#[cfg(test)]
mod tests {
    use crate::storage::get_emergency_withdrawal;
    use crate::tests::helpers::*;
    use crate::Category;
    use soroban_sdk::{Address, String};

    #[test]
    fn test_cancel_campaign_cleans_up_pending_transfer_emergency_and_title_keys() {
        let (env, admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
        let nominee = Address::generate(&env);
        let rescue_recipient = Address::generate(&env);

        token_admin.mint(&c1, &2_000);

        let title = String::from_str(&env, "Cleanup State Keys");
        let id = client.create_campaign(&make_params(
            creator.clone(),
            title.clone(),
            String::from_str(&env, "Verify cancelled campaign state cleanup"),
            1_000,
            30,
            Category::Educator,
            false,
            0,
            0,
        ));
        client.verify_campaign(&id);
        client.contribute(&id, &c1, &200);
        client.initiate_campaign_transfer(&id, &nominee);

        client.cancel_campaign(&id);

        let cancelled = client.get_campaign(&id);
        assert!(cancelled.is_cancelled());
        assert!(cancelled.pending_creator.is_none());
        assert_eq!(cancelled.pending_creator_expiry, 0);

        // Creator can reuse the cleaned-up title for a fresh campaign after cancellation.
        let new_id = client.create_campaign(&make_params(
            creator,
            title,
            String::from_str(&env, "Recreated with freed title index"),
            1_000,
            30,
            Category::Educator,
            false,
            0,
            0,
        ));
        assert_eq!(new_id, 2);
        client.verify_campaign(&new_id);
        client.contribute(&new_id, &c1, &1_000);
        let new_campaign = client.get_campaign(&new_id);
        env.ledger().set_timestamp(new_campaign.deadline + 1);
        client.emergency_withdraw(&admin, &new_id, &rescue_recipient);
        env.as_contract(&client.address, || {
            assert!(get_emergency_withdrawal(&env, new_id).is_some());
        });

        client.admin_cancel_campaign(
            &admin,
            &new_id,
            &String::from_str(&env, "Cancel after emergency proposal"),
        );
        env.as_contract(&client.address, || {
            assert!(get_emergency_withdrawal(&env, new_id).is_none());
        });
    }
}
