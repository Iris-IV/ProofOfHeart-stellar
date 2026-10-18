use soroban_sdk::{Env, String};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_start_time_or_error, get_creator_campaign, require_active_campaign,
    require_not_paused, require_unverified_campaign,
};
use crate::storage::{
    bump_instance_ttl, creator_has_title, get_category_duration_cap, hash_text,
    remove_creator_title_index, set_campaign, set_creator_title_index,
};

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

    if campaign.title == title && campaign.description == description {
        return Ok(());
    }

    // Keep the per-creator title-uniqueness index in sync with a rename, so a
    // creator cannot rename into a title they already use and the old title is
    // released for reuse. Same error as the duplicate check in create.
    let title_hashes = if campaign.title != title {
        let new_hash = hash_text(env, &title);
        if creator_has_title(env, &campaign.creator, &new_hash) {
            return Err(Error::ValidationFailed);
        }
        Some((hash_text(env, &campaign.title), new_hash))
    } else {
        None
    };

    bump_instance_ttl(env);
    let old_title = core::mem::replace(&mut campaign.title, title.clone());
    let old_description = core::mem::replace(&mut campaign.description, description.clone());

    set_campaign(env, campaign_id, &campaign);
    if let Some((old_hash, new_hash)) = title_hashes {
        remove_creator_title_index(env, &campaign.creator, &old_hash);
        set_creator_title_index(env, &campaign.creator, &new_hash, campaign_id);
    }

    env.events().publish(
        ("campaign_metadata_updated", campaign_id),
        (old_title, old_description, title, description),
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

    // Freeze verified metadata: once a campaign is verified, its description
    // is part of the attested content contributors rely on. Allowing edits
    // after verification creates a bait-and-switch path where a creator gets
    // approval on one description and then silently rewrites it. Reject the
    // edit entirely — the creator must cancel and recreate if the content
    // needs to change after verification.
    require_unverified_campaign(&campaign)?;

    require_active_campaign(&campaign)?;
    if description == campaign.description {
        return Ok(());
    }
    if description.len() < crate::CAMPAIGN_DESCRIPTION_MIN_LEN
        || description.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN
    {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);
    let old_description = core::mem::replace(&mut campaign.description, description.clone());

    set_campaign(env, campaign_id, &campaign);

    // Title is unaffected by this function — publish it unchanged in both
    // old/new slots so `campaign_metadata_updated` has one consistent shape
    // for indexers regardless of which entry point emitted it (#510).
    env.events().publish(
        ("campaign_metadata_updated", campaign_id),
        (
            campaign.title.clone(),
            old_description,
            campaign.title.clone(),
            description,
        ),
    );

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
    if additional_days == 0 || additional_days > crate::MAX_EXTENSION_DAYS {
        return Err(Error::ExtensionTooLong);
    }

    let new_deadline = campaign
        .deadline
        .checked_add(additional_days * crate::SECONDS_PER_DAY)
        .ok_or(Error::Overflow)?;

    let start_time = campaign_start_time_or_error(env, campaign_id)?;
    let category_cap = get_category_duration_cap(env, campaign.category)
        .unwrap_or(crate::CAMPAIGN_DURATION_MAX_DAYS);

    // Compute the total elapsed seconds between campaign start and the
    // proposed new deadline.  Do NOT convert to days via integer division
    // before comparing against the caps: floor division would silently accept
    // a deadline that is `cap * SECONDS_PER_DAY + 1` seconds after start
    // (which rounds down to exactly `cap` days), letting the campaign run 1-N
    // seconds past the policy boundary (#868).
    //
    // Instead, compare seconds directly against `cap * SECONDS_PER_DAY`.
    // The multiplications cannot overflow: both caps are at most 365 and
    // SECONDS_PER_DAY is 86_400, so the product is at most 365 * 86_400 =
    // 31_536_000, well within u64::MAX.
    let total_duration_seconds = new_deadline
        .checked_sub(start_time)
        .ok_or(Error::Overflow)?;

    if total_duration_seconds > category_cap * crate::SECONDS_PER_DAY {
        return Err(Error::InvalidDuration);
    }
    if total_duration_seconds > crate::CAMPAIGN_EXTENSION_MAX_DAYS * crate::SECONDS_PER_DAY {
        return Err(Error::InvalidDuration);
    }

    bump_instance_ttl(env);
    let old_deadline = campaign.deadline;
    campaign.deadline = new_deadline;
    campaign.deadline_extended = true;
    set_campaign(env, campaign_id, &campaign);

    env.events().publish(
        ("campaign_deadline_extended", campaign_id),
        (
            old_deadline,
            new_deadline,
            additional_days,
            total_duration_seconds,
        ),
    );
    Ok(())
}

/// Bitmask flags identifying which campaign fields changed in an update revision (#1025).
pub(crate) const CHANGE_KIND_TITLE: u32 = 1 << 0;
pub(crate) const CHANGE_KIND_DESCRIPTION: u32 = 1 << 1;
pub(crate) const CHANGE_KIND_DEADLINE: u32 = 1 << 2;

/// Computes the bitmask of changed campaign fields for on-chain update tracking (#1025).
pub(crate) const fn classify_metadata_change_mask(
    title_changed: bool,
    description_changed: bool,
    deadline_changed: bool,
) -> u32 {
    let mut mask = 0u32;
    if title_changed {
        mask |= CHANGE_KIND_TITLE;
    }
    if description_changed {
        mask |= CHANGE_KIND_DESCRIPTION;
    }
    if deadline_changed {
        mask |= CHANGE_KIND_DEADLINE;
    }
    mask
}

/// Increments a campaign's monotonic change-log revision counter with overflow protection (#1025).
pub(crate) const fn next_change_revision(current_revision: u32) -> Result<u32, Error> {
    match current_revision.checked_add(1) {
        Some(next) => Ok(next),
        None => Err(Error::Overflow),
    }
}

const _: () = {
    assert!(
        classify_metadata_change_mask(true, false, true)
            == (CHANGE_KIND_TITLE | CHANGE_KIND_DEADLINE)
    );
    assert!(next_change_revision(0).is_ok());
};

/// Computes a deterministic SHA-256 audit digest binding `campaign_id`,
/// `revision`, `change_mask`, `title` hash, `description` hash, and `deadline`
/// for verifiable campaign update history logs (#1025).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_campaign_change_digest(
    env: &Env,
    campaign_id: u32,
    revision: u32,
    change_mask: u32,
    title: &String,
    description: &String,
    deadline: u64,
) -> soroban_sdk::BytesN<32> {
    let mut payload = soroban_sdk::Bytes::from_slice(env, b"poh:campaign-change:v1:");
    payload.extend_from_array(&campaign_id.to_be_bytes());
    payload.extend_from_array(&revision.to_be_bytes());
    payload.extend_from_array(&change_mask.to_be_bytes());
    payload.extend_from_array(&hash_text(env, title).to_array());
    payload.extend_from_array(&hash_text(env, description).to_array());
    payload.extend_from_array(&deadline.to_be_bytes());
    env.crypto().sha256(&payload).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_update_change_tracking_mask_and_digest_are_deterministic() {
        let env = Env::default();
        let mask = classify_metadata_change_mask(true, true, false);
        assert_eq!(mask, CHANGE_KIND_TITLE | CHANGE_KIND_DESCRIPTION);
        assert_eq!(next_change_revision(4), Ok(5));
        assert_eq!(next_change_revision(u32::MAX), Err(Error::Overflow));

        let title_v1 = String::from_str(&env, "Initial Title");
        let title_v2 = String::from_str(&env, "Updated Title");
        let desc = String::from_str(&env, "Campaign Description");

        let d1 = build_campaign_change_digest(&env, 1, 1, mask, &title_v1, &desc, 1_800_000_000);
        let d1_repeat =
            build_campaign_change_digest(&env, 1, 1, mask, &title_v1, &desc, 1_800_000_000);
        let d2 = build_campaign_change_digest(&env, 1, 2, mask, &title_v2, &desc, 1_800_000_000);

        assert_eq!(d1, d1_repeat);
        assert_ne!(d1, d2);
    }
}
