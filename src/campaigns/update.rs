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

/// Default minimum voter quorum for community-governed campaign extensions (#1008).
pub const EXTENSION_GOVERNANCE_MIN_QUORUM: u32 = 3;

/// Default approval threshold in basis points (6000 = 60%) for campaign extensions (#1008).
pub const EXTENSION_GOVERNANCE_THRESHOLD_BPS: u32 = 6_000;

/// Verifies that a campaign deadline or goal extension proposal satisfies the
/// community governance quorum and approval threshold (#1008).
pub const fn verify_extension_governance_threshold(
    approve_votes: u32,
    reject_votes: u32,
    min_quorum: u32,
    threshold_bps: u32,
) -> Result<(), Error> {
    if min_quorum == 0
        || threshold_bps < crate::voting::MIN_APPROVAL_THRESHOLD_BPS
        || threshold_bps > crate::BPS_DENOMINATOR
    {
        return Err(Error::ValidationFailed);
    }
    let total_votes = match approve_votes.checked_add(reject_votes) {
        Some(v) => v,
        None => return Err(Error::Overflow),
    };
    if total_votes < min_quorum {
        return Err(Error::VotingQuorumNotMet);
    }
    let approve_scaled = (approve_votes as u64) * (crate::BPS_DENOMINATOR as u64);
    let required_scaled = (total_votes as u64) * (threshold_bps as u64);
    if approve_scaled < required_scaled {
        return Err(Error::VotingThresholdNotMet);
    }
    Ok(())
}

/// Validates a proposed campaign funding goal extension against platform bounds
/// and a maximum 2x step-up limit (#1008).
pub const fn validate_goal_extension_bounds(
    current_goal: i128,
    proposed_goal: i128,
    amount_raised: i128,
    max_goal: i128,
) -> Result<(), Error> {
    if current_goal <= 0 || proposed_goal <= current_goal || amount_raised < 0 {
        return Err(Error::ValidationFailed);
    }
    if proposed_goal > max_goal {
        return Err(Error::FundingGoalTooHigh);
    }
    let max_step_goal = match current_goal.checked_mul(2) {
        Some(v) => v,
        None => return Err(Error::Overflow),
    };
    if proposed_goal > max_step_goal {
        return Err(Error::ValidationFailed);
    }
    Ok(())
}

const _: () = {
    assert!(verify_extension_governance_threshold(
        3,
        1,
        EXTENSION_GOVERNANCE_MIN_QUORUM,
        EXTENSION_GOVERNANCE_THRESHOLD_BPS
    )
    .is_ok());
    assert!(validate_goal_extension_bounds(
        100_000,
        150_000,
        80_000,
        crate::CAMPAIGN_FUNDING_GOAL_MAX
    )
    .is_ok());
};

#[cfg(test)]
mod tests {
    use super::{
        validate_goal_extension_bounds, verify_extension_governance_threshold,
        EXTENSION_GOVERNANCE_MIN_QUORUM, EXTENSION_GOVERNANCE_THRESHOLD_BPS,
    };
    use crate::errors::Error;

    #[test]
    fn test_extension_governance_threshold_and_goal_bounds() {
        assert_eq!(
            verify_extension_governance_threshold(
                3,
                2,
                EXTENSION_GOVERNANCE_MIN_QUORUM,
                EXTENSION_GOVERNANCE_THRESHOLD_BPS
            ),
            Ok(())
        );
        // Quorum not met (2 < 3)
        assert_eq!(
            verify_extension_governance_threshold(
                2,
                0,
                EXTENSION_GOVERNANCE_MIN_QUORUM,
                EXTENSION_GOVERNANCE_THRESHOLD_BPS
            ),
            Err(Error::VotingQuorumNotMet)
        );
        // Threshold not met (2/4 = 50% < 60%)
        assert_eq!(
            verify_extension_governance_threshold(
                2,
                2,
                EXTENSION_GOVERNANCE_MIN_QUORUM,
                EXTENSION_GOVERNANCE_THRESHOLD_BPS
            ),
            Err(Error::VotingThresholdNotMet)
        );

        // Goal extension within 2x limit and platform max succeeds
        assert_eq!(
            validate_goal_extension_bounds(1_000, 2_000, 500, crate::CAMPAIGN_FUNDING_GOAL_MAX),
            Ok(())
        );
        // Goal extension exceeding 2x step-up limit rejected
        assert_eq!(
            validate_goal_extension_bounds(1_000, 2_001, 500, crate::CAMPAIGN_FUNDING_GOAL_MAX),
            Err(Error::ValidationFailed)
        );
        // Goal extension exceeding platform max rejected
        assert_eq!(
            validate_goal_extension_bounds(1_000, 1_500, 500, 1_200),
            Err(Error::FundingGoalTooHigh)
        );
    }
}
