use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{
    get_campaign_or_error, get_creator_campaign, require_active_campaign, require_not_paused,
};
use crate::storage::{
    bump_instance_ttl, get_creator_campaign_bucket, get_creator_campaign_count,
    get_creator_campaign_position, remove_creator_campaign_position, set_campaign,
    set_campaign_creator_index, set_creator_campaign_bucket, set_creator_campaign_count,
    set_creator_campaign_position, CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
use crate::types::MaybePendingCreator;

fn get_creator_campaign_position_or_legacy_scan(
    env: &Env,
    creator: &Address,
    campaign_id: u32,
    campaign_count: u32,
) -> Option<(u32, u32)> {
    if let Some(position) = get_creator_campaign_position(env, creator, campaign_id) {
        return Some(position);
    }

    // Campaigns created before #808 do not have a position entry. Preserve
    // their ability to transfer while all newly created campaigns take the
    // O(1) path above.
    let bucket_count = campaign_count.div_ceil(CREATOR_CAMPAIGNS_BUCKET_SIZE);
    for bucket_idx in 0..bucket_count {
        let bucket = get_creator_campaign_bucket(env, creator, bucket_idx);
        if let Some(slot_idx) = bucket.first_index_of(campaign_id) {
            return Some((bucket_idx, slot_idx));
        }
    }
    None
}

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
    require_not_paused(env)?;

    let pending = match campaign.pending_creator.clone() {
        MaybePendingCreator::Some(addr) => addr,
        MaybePendingCreator::None => return Err(Error::NoTransferPending),
    };
    pending.require_auth();

    // Defence in depth (#790). `initiate_campaign_transfer` already rejects a
    // nomination equal to the current creator, and `campaign.creator` has no
    // other writer, so this is unreachable today. It is checked here anyway
    // because the bucket rewrite below is not idempotent: removing the
    // campaign from the creator's bucket and re-adding it to the same bucket
    // would leave the count decremented and then incremented around a list
    // that never changed, and any future path that reassigns `creator` would
    // silently corrupt the index rather than fail.
    if pending == campaign.creator {
        return Err(Error::InvalidNewOwner);
    }

    bump_instance_ttl(env);
    let old_creator = campaign.creator.clone();

    let old_count = get_creator_campaign_count(env, &old_creator);
    let (old_bucket_idx, old_slot_idx) =
        get_creator_campaign_position_or_legacy_scan(env, &old_creator, campaign_id, old_count)
            .ok_or(Error::ValidationFailed)?;
    let mut old_bucket = get_creator_campaign_bucket(env, &old_creator, old_bucket_idx);
    if old_bucket.is_empty() {
        return Err(Error::ValidationFailed);
    }
    if old_bucket.get(old_slot_idx) != Some(campaign_id) {
        return Err(Error::ValidationFailed);
    }

    // Swap removal avoids shifting every later slot and lets us update at most
    // one position record. The position lookup itself replaces the old scan of
    // every creator bucket (#808).
    let last_slot_idx = old_bucket.len() - 1;
    let last_campaign_id = old_bucket
        .get(last_slot_idx)
        .ok_or(Error::ValidationFailed)?;
    if old_slot_idx != last_slot_idx {
        old_bucket.set(old_slot_idx, last_campaign_id);
        set_creator_campaign_position(
            env,
            &old_creator,
            last_campaign_id,
            old_bucket_idx,
            old_slot_idx,
        );
    }
    old_bucket.pop_back();
    set_creator_campaign_bucket(env, &old_creator, old_bucket_idx, &old_bucket);
    remove_creator_campaign_position(env, &old_creator, campaign_id);
    set_creator_campaign_count(env, &old_creator, old_count.saturating_sub(1));

    let new_count = get_creator_campaign_count(env, &pending);
    let new_bucket_idx = new_count / CREATOR_CAMPAIGNS_BUCKET_SIZE;
    let mut new_bucket = get_creator_campaign_bucket(env, &pending, new_bucket_idx);
    new_bucket.push_back(campaign_id);
    set_creator_campaign_bucket(env, &pending, new_bucket_idx, &new_bucket);
    set_creator_campaign_position(
        env,
        &pending,
        campaign_id,
        new_bucket_idx,
        new_bucket.len() - 1,
    );
    let new_count = new_count
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    set_creator_campaign_count(env, &pending, new_count);
    set_campaign_creator_index(env, campaign_id, &pending);

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
