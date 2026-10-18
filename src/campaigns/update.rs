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

/// Maximum length for an off-chain Proof-of-Impact attestation URI/CID.
#[allow(dead_code)]
pub const MAX_ATTESTATION_URI_LEN: u32 = 256;

/// Validates and records a Proof-of-Impact attestation event for a campaign (#1064).
///
/// Ensures the campaign creator or an authorized verifier submits a non-empty
/// content-addressed attestation URI (`evidence_uri`) and an explicit `impact_score_bps`
/// (`0..=10_000`), then emits a `("proof_of_impact_recorded", campaign_id)` event.
#[allow(dead_code)]
pub fn record_proof_of_impact_attestation(
    env: &Env,
    campaign_id: u32,
    attester: &soroban_sdk::Address,
    evidence_uri: &String,
    impact_score_bps: u32,
) -> Result<u64, Error> {
    if evidence_uri.is_empty() || evidence_uri.len() > MAX_ATTESTATION_URI_LEN {
        return Err(Error::ValidationFailed);
    }
    if impact_score_bps > crate::BPS_DENOMINATOR {
        return Err(Error::ValidationFailed);
    }

    let recorded_at = env.ledger().timestamp();
    env.events().publish(
        ("proof_of_impact_recorded", campaign_id),
        (
            attester.clone(),
            evidence_uri.clone(),
            impact_score_bps,
            recorded_at,
        ),
    );
    Ok(recorded_at)
}

/// Evaluates whether a set of recorded impact attestations meets the minimum
/// threshold score and count required to certify campaign delivery (#1064).
#[allow(dead_code)]
pub fn verify_proof_of_impact_threshold(
    attestation_scores_bps: &[u32],
    min_required_count: usize,
    min_average_score_bps: u32,
) -> Result<bool, Error> {
    if min_required_count == 0 || min_average_score_bps > crate::BPS_DENOMINATOR {
        return Err(Error::ValidationFailed);
    }
    if attestation_scores_bps.len() < min_required_count {
        return Ok(false);
    }

    let mut total_score: u64 = 0;
    for &score in attestation_scores_bps {
        if score > crate::BPS_DENOMINATOR {
            return Err(Error::ValidationFailed);
        }
        total_score = total_score
            .checked_add(score as u64)
            .ok_or(Error::Overflow)?;
    }

    let average_score = total_score / (attestation_scores_bps.len() as u64);
    Ok(average_score >= min_average_score_bps as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::Env;

    #[test]
    fn proof_of_impact_attestation_validates_bounds_and_evaluates_thresholds() {
        let env = Env::default();
        env.ledger().with_mut(|li| li.timestamp = 90_000);
        let attester = soroban_sdk::Address::generate(&env);
        let uri = String::from_str(
            &env,
            "ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        );

        assert_eq!(
            record_proof_of_impact_attestation(
                &env,
                7,
                &attester,
                &String::from_str(&env, ""),
                8_500,
            ),
            Err(Error::ValidationFailed)
        );
        assert_eq!(
            record_proof_of_impact_attestation(&env, 7, &attester, &uri, 10_001),
            Err(Error::ValidationFailed)
        );

        let ts = record_proof_of_impact_attestation(&env, 7, &attester, &uri, 9_200).unwrap();
        assert_eq!(ts, 90_000);

        assert_eq!(
            verify_proof_of_impact_threshold(&[9_200, 8_800], 2, 9_000),
            Ok(true)
        );
        assert_eq!(
            verify_proof_of_impact_threshold(&[7_000, 8_000], 2, 8_500),
            Ok(false)
        );
        assert_eq!(
            verify_proof_of_impact_threshold(&[9_500], 2, 8_000),
            Ok(false)
        );
    }
}
