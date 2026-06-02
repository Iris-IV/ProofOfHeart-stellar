use soroban_sdk::{Env, String};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_start_time_or_error, get_creator_campaign, require_active_campaign,
    require_not_paused, require_unverified_campaign,
};
use crate::storage::{bump_instance_ttl, get_category_duration_cap, set_campaign};

/// Updates the title and description of a campaign.
///
/// Blocked after verification (issue #416: verified content must match published content)
/// and blocked if contributions have already been received.
pub(crate) fn update_campaign(
    env: &Env,
    campaign_id: u32,
    title: String,
    description: String,
) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    // Fix #416: verification freezes title and description.
    require_unverified_campaign(&campaign)?;

    if campaign.amount_raised > 0 {
        return Err(Error::ValidationFailed);
    }

    require_active_campaign(&campaign)?;

    if title.len() < crate::CAMPAIGN_TITLE_MIN_LEN || title.len() > crate::CAMPAIGN_TITLE_MAX_LEN {
        return Err(Error::ValidationFailed);
    }
    if description.len() < crate::CAMPAIGN_DESCRIPTION_MIN_LEN
        || description.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN
    {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);
    let event_description = description.clone();
    campaign.title = title.clone();
    campaign.description = description;

    set_campaign(env, campaign_id, &campaign);

    env.events().publish(
        ("campaign_updated", campaign_id),
        (title, event_description),
    );

    Ok(())
}

pub(crate) fn update_campaign_description(
    env: &Env,
    campaign_id: u32,
    description: String,
) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;

    require_active_campaign(&campaign)?;
    if description.len() < crate::CAMPAIGN_DESCRIPTION_MIN_LEN
        || description.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN
    {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);
    let event_desc = description.clone();
    campaign.description = description;
    set_campaign(env, campaign_id, &campaign);

    env.events()
        .publish(("campaign_description_updated", campaign_id), event_desc);

    Ok(())
}

pub(crate) fn extend_campaign_deadline(
    env: &Env,
    campaign_id: u32,
    additional_days: u64,
) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;
    require_active_campaign(&campaign)?;

    if campaign.deadline_extended {
        return Err(Error::DeadlineAlreadyExtended);
    }
    if env.ledger().timestamp() >= campaign.deadline {
        return Err(Error::DeadlinePassed);
    }
    if additional_days == 0 || additional_days > 30 {
        return Err(Error::ExtensionTooLong);
    }

    let new_deadline = campaign
        .deadline
        .checked_add(additional_days * 86400)
        .ok_or(Error::Overflow)?;

    let start_time = campaign_start_time_or_error(env, campaign_id)?;
    let category_cap = get_category_duration_cap(env, campaign.category)
        .unwrap_or(crate::CAMPAIGN_DURATION_MAX_DAYS);

    let total_duration_seconds = new_deadline
        .checked_sub(start_time)
        .ok_or(Error::Overflow)?;
    let total_duration_days = total_duration_seconds / 86400;

    if total_duration_days > category_cap {
        return Err(Error::InvalidDuration);
    }

    bump_instance_ttl(env);
    campaign.deadline = new_deadline;
    campaign.deadline_extended = true;
    set_campaign(env, campaign_id, &campaign);

    env.events()
        .publish(("campaign_deadline_extended", campaign_id), additional_days);
    Ok(())
}
