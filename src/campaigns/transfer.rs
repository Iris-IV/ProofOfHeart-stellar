use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, get_creator_campaign, require_active_campaign, require_not_paused,
};
use crate::storage::{bump_instance_ttl, set_campaign, transfer_creator_campaign};
use crate::types::MaybePendingCreator;

pub(crate) fn initiate_campaign_transfer(
    env: &Env,
    campaign_id: u32,
    new_creator: Address,
) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;
    require_active_campaign(&campaign)?;

    if campaign.funds_withdrawn {
        return Err(Error::CampaignNotActive);
    }

    if new_creator == campaign.creator {
        return Err(Error::InvalidNewOwner);
    }

    if campaign.pending_creator.is_some() {
        return Err(Error::TransferAlreadyPending);
    }

    bump_instance_ttl(env);
    campaign.pending_creator = MaybePendingCreator::from(new_creator.clone());
    set_campaign(env, campaign_id, &campaign);

    env.events().publish(
        (
            "campaign_transfer_initiated",
            campaign_id,
            campaign.creator.clone(),
        ),
        new_creator,
    );

    Ok(())
}

pub(crate) fn accept_campaign_transfer(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;

    let pending = match campaign.pending_creator.clone() {
        MaybePendingCreator::Some(addr) => addr,
        MaybePendingCreator::None => return Err(Error::NoTransferPending),
    };
    pending.require_auth();

    require_not_paused(env)?;

    bump_instance_ttl(env);
    let old_creator = campaign.creator.clone();

    // Fix #464: atomic bucket transfer — reads both buckets before any write.
    transfer_creator_campaign(env, campaign_id, &old_creator, &pending)?;

    campaign.creator = pending.clone();
    campaign.pending_creator = MaybePendingCreator::None;

    set_campaign(env, campaign_id, &campaign);

    env.events().publish(
        ("campaign_transfer_completed", campaign_id),
        (old_creator, pending),
    );

    Ok(())
}

pub(crate) fn cancel_campaign_transfer(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    let pending_address = match campaign.pending_creator.clone() {
        MaybePendingCreator::Some(addr) => addr,
        MaybePendingCreator::None => return Err(Error::NoTransferPending),
    };

    bump_instance_ttl(env);
    campaign.pending_creator = MaybePendingCreator::None;
    set_campaign(env, campaign_id, &campaign);

    env.events().publish(
        ("campaign_transfer_cancelled", campaign_id),
        pending_address,
    );

    Ok(())
}
