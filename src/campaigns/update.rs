use soroban_sdk::{Address, BytesN, Env, String};

use crate::errors::Error;
use crate::lifecycle::{
    campaign_start_time_or_error, get_campaign_or_error, get_creator_campaign,
    require_active_campaign, require_not_paused, require_unverified_campaign,
};
use crate::storage::{
    bump_instance_ttl, creator_has_title, get_campaign_fee_recipient, get_category_duration_cap,
    get_max_campaign_funding_goal, get_min_campaign_funding_goal, hash_text,
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

/// Changes a campaign's funding goal before it has received its first
/// contribution (#1056).
///
/// A creator who mis-sized their target can fix it while nobody has funded the
/// campaign yet. Once the first contribution lands the goal is frozen:
/// contributors chose to fund against that number, and moving it afterwards
/// would change whether the campaign succeeds or is refunded under them.
///
/// The first contribution is detected two ways: `amount_raised` (never
/// decremented by refunds, so it stays non-zero once anyone has contributed)
/// and the fee-recipient snapshot written on that first contribution (#800).
///
/// The new goal goes through the same bounds as `create_campaign`. Like the
/// other metadata edits, the goal is frozen once the campaign is verified,
/// and it cannot change after the deadline or on an inactive campaign.
/// Setting the goal to its current value is a no-op.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `ContractPaused` - The contract is paused.
/// * `CampaignAlreadyVerified` - The campaign has been verified.
/// * `CampaignNotActive` - The campaign is cancelled, withdrawn or inactive.
/// * `DeadlinePassed` - The campaign's deadline has passed.
/// * `ValidationFailed` - The campaign has already received a contribution.
/// * `FundingGoalMustBePositive` / `FundingGoalTooLow` / `FundingGoalTooHigh` -
///   The new goal is outside the platform's bounds.
pub(crate) fn update_campaign_goal(
    env: &Env,
    campaign_id: u32,
    new_goal: i128,
) -> Result<(), Error> {
    let mut campaign = get_creator_campaign(env, campaign_id)?;
    require_not_paused(env)?;
    require_unverified_campaign(&campaign)?;
    require_active_campaign(&campaign)?;

    if env.ledger().timestamp() >= campaign.deadline {
        return Err(Error::DeadlinePassed);
    }
    // `ValidationFailed`, matching `update_campaign`'s "already funded" check:
    // `Error` has no free slot for a dedicated code.
    if campaign.amount_raised > 0 || get_campaign_fee_recipient(env, campaign_id).is_some() {
        return Err(Error::ValidationFailed);
    }

    if new_goal <= 0 {
        return Err(Error::FundingGoalMustBePositive);
    }
    if new_goal < get_min_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MIN) {
        return Err(Error::FundingGoalTooLow);
    }
    if new_goal > get_max_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MAX) {
        return Err(Error::FundingGoalTooHigh);
    }

    if new_goal == campaign.funding_goal {
        return Ok(());
    }

    bump_instance_ttl(env);
    let old_goal = core::mem::replace(&mut campaign.funding_goal, new_goal);
    set_campaign(env, campaign_id, &campaign);

    env.events()
        .publish(("campaign_goal_updated", campaign_id), (old_goal, new_goal));

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

/// Records a cryptographic proof-of-impact hash submitted by an authorized verifier (e.g. admin).
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignNotActive` - Campaign is cancelled or inactive.
/// * `NotAuthorized` - Caller is not the admin or authorized verifier.
/// * `ContractPaused` - The contract is paused.
pub(crate) fn record_proof_of_impact(
    env: &Env,
    campaign_id: u32,
    verifier: Address,
    proof_hash: BytesN<32>,
) -> Result<(), Error> {
    verifier.require_auth();
    require_not_paused(env)?;

    let campaign = get_campaign_or_error(env, campaign_id)?;
    require_active_campaign(&campaign)?;

    let admin = crate::storage::get_admin(env);
    if verifier != admin {
        return Err(Error::NotAuthorized);
    }

    let timestamp = env.ledger().timestamp();
    let proof = crate::types::ProofOfImpact {
        verifier: verifier.clone(),
        proof_hash: proof_hash.clone(),
        timestamp,
    };

    bump_instance_ttl(env);
    crate::storage::set_proof_of_impact(env, campaign_id, &proof);

    env.events().publish(
        ("proof_of_impact_recorded", campaign_id, verifier),
        (proof_hash, timestamp),
    );

    Ok(())
}

/// Returns the recorded proof of impact for a campaign, if one exists.
pub(crate) fn get_proof_of_impact(
    env: &Env,
    campaign_id: u32,
) -> Option<crate::types::ProofOfImpact> {
    crate::storage::get_proof_of_impact(env, campaign_id)
}
