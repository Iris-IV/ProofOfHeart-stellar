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

/// Validates whether a campaign's funding goal can be adjusted to `new_funding_goal`
/// prior to receiving its first contribution (#1056).
#[allow(dead_code)]
pub fn validate_pre_contribution_target_adjustment(
    campaign: &crate::types::Campaign,
    new_funding_goal: i128,
    min_goal: i128,
    max_goal: i128,
) -> Result<(), Error> {
    require_active_campaign(campaign)?;
    require_unverified_campaign(campaign)?;

    if campaign.amount_raised > 0 || campaign.effective_amount_raised > 0 {
        return Err(Error::ValidationFailed);
    }
    if new_funding_goal <= 0 {
        return Err(Error::FundingGoalMustBePositive);
    }
    if new_funding_goal < min_goal {
        return Err(Error::FundingGoalTooLow);
    }
    if new_funding_goal > max_goal {
        return Err(Error::FundingGoalTooHigh);
    }
    Ok(())
}

/// Applies a validated funding-goal adjustment to an active, unverified campaign
/// that has not yet received any contributions, returning `(old_goal, new_goal)` (#1056).
#[allow(dead_code)]
pub fn apply_flexible_target_adjustment(
    campaign: &mut crate::types::Campaign,
    new_funding_goal: i128,
    min_goal: i128,
    max_goal: i128,
) -> Result<(i128, i128), Error> {
    validate_pre_contribution_target_adjustment(campaign, new_funding_goal, min_goal, max_goal)?;
    let old_goal = campaign.funding_goal;
    campaign.funding_goal = new_funding_goal;
    Ok((old_goal, new_funding_goal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Campaign, Category, MaybePendingCreator, CAMPAIGN_ACTIVE};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, Env};

    fn sample_campaign(env: &Env) -> Campaign {
        let creator = Address::generate(env);
        Campaign {
            id: 1,
            creator: creator.clone(),
            first_creator: creator,
            pending_creator: MaybePendingCreator::None,
            pending_creator_expiry: 0,
            title: String::from_str(env, "Clean Water Well"),
            description: String::from_str(env, "Solar-powered filtration unit for primary school."),
            funding_goal: 5_000,
            deadline: 100_000,
            amount_raised: 0,
            status: CAMPAIGN_ACTIVE,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: 0,
            fee_override: None,
            deadline_extended: false,
            effective_amount_raised: 0,
        }
    }

    #[test]
    fn flexible_target_adjustment_allows_pre_contribution_updates_and_blocks_post_contribution() {
        let env = Env::default();
        let mut campaign = sample_campaign(&env);

        let (old_goal, new_goal) =
            apply_flexible_target_adjustment(&mut campaign, 8_000, 1_000, 100_000).unwrap();
        assert_eq!(old_goal, 5_000);
        assert_eq!(new_goal, 8_000);
        assert_eq!(campaign.funding_goal, 8_000);

        assert_eq!(
            apply_flexible_target_adjustment(&mut campaign, 500, 1_000, 100_000),
            Err(Error::FundingGoalTooLow)
        );
        assert_eq!(
            apply_flexible_target_adjustment(&mut campaign, 200_000, 1_000, 100_000),
            Err(Error::FundingGoalTooHigh)
        );

        campaign.amount_raised = 100;
        assert_eq!(
            apply_flexible_target_adjustment(&mut campaign, 10_000, 1_000, 100_000),
            Err(Error::ValidationFailed)
        );
    }
}
