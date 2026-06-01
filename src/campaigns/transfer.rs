use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, get_creator_campaign, require_active_campaign, require_not_paused,
};
use crate::storage::{
    bump_instance_ttl, get_creator_campaign_bucket, get_creator_campaign_count, set_campaign,
    set_creator_campaign_bucket, set_creator_campaign_count, CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
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

    let old_count = get_creator_campaign_count(env, &old_creator);
    let old_num_buckets = old_count.div_ceil(CREATOR_CAMPAIGNS_BUCKET_SIZE);
    'outer: for bucket_idx in 0..old_num_buckets {
        let mut bucket = get_creator_campaign_bucket(env, &old_creator, bucket_idx);
        if let Some(pos) = bucket.first_index_of(campaign_id) {
            bucket.remove(pos);
            set_creator_campaign_bucket(env, &old_creator, bucket_idx, &bucket);
            break 'outer;
        }
    }
    set_creator_campaign_count(env, &old_creator, old_count.saturating_sub(1));

    let new_count = get_creator_campaign_count(env, &pending);
    let new_bucket_idx = new_count / CREATOR_CAMPAIGNS_BUCKET_SIZE;
    let mut new_bucket = get_creator_campaign_bucket(env, &pending, new_bucket_idx);
    new_bucket.push_back(campaign_id);
    set_creator_campaign_bucket(env, &pending, new_bucket_idx, &new_bucket);
    set_creator_campaign_count(env, &pending, new_count + 1);

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
