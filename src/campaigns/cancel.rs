use soroban_sdk::{token, Address, Env, String};

use crate::errors::Error;
use crate::lifecycle::{
    assert_admin, get_campaign_or_error, get_creator_campaign, require_active_campaign,
    require_not_paused, transition, CampaignState,
};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_revenue_pool, get_token,
    increment_cancelled_campaign_count, remove_voting_state, set_campaign, set_revenue_pool,
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

    let revenue_pool = get_revenue_pool(env, campaign_id);
    if revenue_pool > 0 {
        let token_addr = get_token(env);
        let client = token::Client::new(env, &token_addr);
        client.transfer(
            &env.current_contract_address(),
            &campaign.creator,
            &revenue_pool,
        );
        set_revenue_pool(env, campaign_id, 0);
        env.events()
            .publish(("revenue_pool_refunded", campaign_id), revenue_pool);
    }

    campaign.is_cancelled = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    decrement_active_campaign_count(env);
    increment_cancelled_campaign_count(env);

    env.events().publish(
        ("campaign_cancelled", campaign_id, campaign.creator.clone()),
        campaign.amount_raised,
    );

    Ok(())
}

/// Admin-initiated cancellation for fraud response (#508). Unlike
/// `cancel_campaign`, this is not restricted to the creator and does not
/// apply the goal-met anti-rug-pull guard — an admin must be able to stop a
/// verified fraudulent campaign even after it has hit its funding goal,
/// without pausing the entire platform. It also deliberately does not
/// auto-refund any revenue pool balance to the (presumed fraudulent)
/// creator, unlike creator self-cancel; that balance is left in the
/// contract with no other exit path — a known follow-up, not solved here.
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

    campaign.is_cancelled = true;
    campaign.is_active = false;
    set_campaign(env, campaign_id, &campaign);
    remove_voting_state(env, campaign_id);
    decrement_active_campaign_count(env);
    increment_cancelled_campaign_count(env);

    env.events().publish(
        ("campaign_admin_cancelled", campaign_id, admin),
        (campaign.creator.clone(), reason),
    );

    Ok(())
}
