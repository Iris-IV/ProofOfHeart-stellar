//! Tag-based campaign discovery (#798).
//!
//! Campaigns carry a small set of free-text tags, applied by the creator
//! after creation with [`add_campaign_tag`]. Each tag is mirrored into an
//! inverted index — `sha256(tag)` → bucketed list of campaign ids — so
//! [`crate::queries::get_campaigns_by_tag`] can page through the campaigns
//! for a tag without scanning every campaign.
//!
//! Tags are append-only, mirroring the category index: there is no
//! `remove_campaign_tag`, so an index bucket never develops holes and the
//! shared bucket-pagination helper stays correct.

use soroban_sdk::{Env, String};

use crate::errors::Error;
use crate::lifecycle::{get_creator_campaign, require_active_campaign, require_not_paused};
use crate::storage::{
    append_campaign_to_tag, bump_instance_ttl, get_campaign_tags, hash_text, set_campaign_tags,
};

/// Creator adds `tag` to `campaign_id` and indexes the campaign under it (#798).
///
/// # Errors
/// * `CampaignNotFound` — no campaign with the given id.
/// * `NotAuthorized` — caller is not the campaign creator.
/// * `ContractPaused` — the contract is paused.
/// * `CampaignNotActive` — the campaign is cancelled or no longer active.
/// * `ValidationFailed` — the tag is empty, longer than
///   [`crate::CAMPAIGN_TAG_MAX_LEN`], already present on the campaign, or the
///   campaign already carries [`crate::MAX_TAGS_PER_CAMPAIGN`] tags.
pub(crate) fn add_campaign_tag(env: &Env, campaign_id: u32, tag: String) -> Result<(), Error> {
    let campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;
    require_active_campaign(&campaign)?;

    if tag.len() < crate::CAMPAIGN_TAG_MIN_LEN || tag.len() > crate::CAMPAIGN_TAG_MAX_LEN {
        return Err(Error::ValidationFailed);
    }

    let mut tags = get_campaign_tags(env, campaign_id);
    if tags.len() >= crate::MAX_TAGS_PER_CAMPAIGN {
        return Err(Error::ValidationFailed);
    }
    // Reject a duplicate rather than silently no-op: a second identical tag
    // would push the campaign id into the tag's index bucket twice, so the
    // query would return the campaign twice.
    if tags.iter().any(|existing| existing == tag) {
        return Err(Error::ValidationFailed);
    }

    bump_instance_ttl(env);
    let tag_hash = hash_text(env, &tag);
    append_campaign_to_tag(env, &tag_hash, campaign_id);
    tags.push_back(tag.clone());
    set_campaign_tags(env, campaign_id, &tags);

    env.events()
        .publish(("campaign_tag_added", campaign_id), tag);
    Ok(())
}
