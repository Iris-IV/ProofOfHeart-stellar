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

/// Validates that a candidate tag length and the campaign's current tag count
/// satisfy platform bounds (#1028).
pub(crate) const fn validate_campaign_tag_bounds(
    tag_len: u32,
    current_tag_count: u32,
) -> Result<(), Error> {
    if tag_len < crate::CAMPAIGN_TAG_MIN_LEN || tag_len > crate::CAMPAIGN_TAG_MAX_LEN {
        return Err(Error::ValidationFailed);
    }
    if current_tag_count >= crate::MAX_TAGS_PER_CAMPAIGN {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

const _: () = {
    assert!(validate_campaign_tag_bounds(crate::CAMPAIGN_TAG_MIN_LEN, 0).is_ok());
    assert!(validate_campaign_tag_bounds(0, 0).is_err());
    assert!(validate_campaign_tag_bounds(3, crate::MAX_TAGS_PER_CAMPAIGN).is_err());
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

    let mut tags = get_campaign_tags(env, campaign_id);
    validate_campaign_tag_bounds(tag.len(), tags.len())?;

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

/// Returns `true` if `campaign_tags` contains every tag in `required_tags`
/// (conjunctive / AND filter for fine-grained tag queries) (#1028).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn campaign_matches_all_tags(
    campaign_tags: &soroban_sdk::Vec<String>,
    required_tags: &soroban_sdk::Vec<String>,
) -> bool {
    if required_tags.is_empty() {
        return false;
    }
    for req in required_tags.iter() {
        if !campaign_tags.contains(&req) {
            return false;
        }
    }
    true
}

/// Returns `true` if `campaign_tags` contains at least one tag in
/// `candidate_tags` (disjunctive / OR filter for tag discovery) (#1028).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn campaign_matches_any_tag(
    campaign_tags: &soroban_sdk::Vec<String>,
    candidate_tags: &soroban_sdk::Vec<String>,
) -> bool {
    for candidate in candidate_tags.iter() {
        if campaign_tags.contains(&candidate) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Vec;

    #[test]
    fn fine_grained_tag_filtering_evaluates_conjunctive_and_disjunctive_matches() {
        let env = Env::default();
        let stem = String::from_str(&env, "stem");
        let math = String::from_str(&env, "math");
        let arts = String::from_str(&env, "arts");

        let campaign_tags = Vec::from_array(&env, [stem.clone(), math.clone()]);
        let both_required = Vec::from_array(&env, [stem.clone(), math]);
        let missing_required = Vec::from_array(&env, [stem, arts.clone()]);
        let only_arts = Vec::from_array(&env, [arts]);

        assert!(campaign_matches_all_tags(&campaign_tags, &both_required));
        assert!(!campaign_matches_all_tags(
            &campaign_tags,
            &missing_required
        ));
        assert!(campaign_matches_any_tag(&campaign_tags, &missing_required));
        assert!(!campaign_matches_any_tag(&campaign_tags, &only_arts));
        assert_eq!(
            validate_campaign_tag_bounds(crate::CAMPAIGN_TAG_MAX_LEN + 1, 0),
            Err(Error::ValidationFailed)
        );
    }
}
