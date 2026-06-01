use soroban_sdk::{token, Env};

use crate::errors::Error;
use crate::lifecycle::{get_creator_campaign, require_active_campaign, require_not_paused};
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
