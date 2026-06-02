use soroban_sdk::{token, Address, Env};

use crate::errors::Error;
use crate::storage::{get_admin, get_campaign, get_campaign_start_time, get_token, DataKey};
use crate::types::Campaign;

pub(crate) fn get_campaign_or_error(env: &Env, campaign_id: u32) -> Result<Campaign, Error> {
    get_campaign(env, campaign_id).ok_or(Error::CampaignNotFound)
}

pub(crate) fn get_creator_campaign(env: &Env, campaign_id: u32) -> Result<Campaign, Error> {
    let campaign = get_campaign_or_error(env, campaign_id)?;
    assert_creator(&campaign)?;
    Ok(campaign)
}

pub(crate) fn assert_creator(campaign: &Campaign) -> Result<(), Error> {
    campaign.creator.require_auth();
    Ok(())
}

pub(crate) fn assert_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin = get_admin(env);
    if *caller != admin {
        return Err(Error::NotAuthorized);
    }
    caller.require_auth();
    Ok(())
}

pub(crate) fn require_active_campaign(campaign: &Campaign) -> Result<(), Error> {
    if campaign.is_cancelled || !campaign.is_active {
        return Err(Error::CampaignNotActive);
    }
    Ok(())
}

pub(crate) fn require_unverified_campaign(campaign: &Campaign) -> Result<(), Error> {
    if campaign.is_verified {
        return Err(Error::CampaignAlreadyVerified);
    }
    Ok(())
}

pub(crate) fn require_revenue_sharing(campaign: &Campaign, error: Error) -> Result<(), Error> {
    if !campaign.has_revenue_sharing {
        return Err(error);
    }
    Ok(())
}

pub(crate) fn calculate_deadline(current_time: u64, duration_days: u64) -> Result<u64, Error> {
    let seconds_in_duration = duration_days
        .checked_mul(86400)
        .ok_or(Error::ValidationFailed)?;
    current_time
        .checked_add(seconds_in_duration)
        .ok_or(Error::ValidationFailed)
}

pub(crate) fn require_not_paused(env: &Env) -> Result<(), Error> {
    if env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
        || env
            .storage()
            .instance()
            .get(&DataKey::AutoPaused)
            .unwrap_or(false)
    {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

pub(crate) fn token_client(env: &Env) -> token::Client<'_> {
    token::Client::new(env, &get_token(env))
}

pub(crate) fn campaign_start_time_or_error(env: &Env, campaign_id: u32) -> Result<u64, Error> {
    get_campaign_start_time(env, campaign_id).ok_or(Error::InvalidDuration)
}
