use soroban_sdk::{token, Address, Env};

use crate::errors::Error;
use crate::storage::{
    get_approval_threshold_bps, get_approve_votes, get_approve_weight, get_has_voted,
    get_min_votes_quorum, get_min_voting_balance, get_reject_votes, get_reject_weight, get_token,
    increment_verified_campaign_count, set_approval_threshold_bps, set_approve_votes,
    set_approve_weight, set_campaign, set_has_voted, set_min_votes_quorum, set_reject_votes,
    set_reject_weight,
};
use crate::{get_campaign_or_error, require_active_campaign, require_unverified_campaign};

/// Default minimum number of votes required to reach quorum.
pub const DEFAULT_MIN_VOTES_QUORUM: u32 = 3;

/// Maximum allowed minimum votes quorum to prevent governance lock.
pub const MAX_VOTES_QUORUM: u32 = 1000;

/// Default approval threshold in basis points (60%).
pub const DEFAULT_APPROVAL_THRESHOLD_BPS: u32 = 6000;

/// Minimum allowed approval threshold in basis points (10%).
/// Prevents governance misconfiguration where near-zero threshold bypasses community review.
pub const MIN_APPROVAL_THRESHOLD_BPS: u32 = 1000;

/// Updates the community voting parameters.
///
/// # Errors
/// * `NotAuthorized` - Caller is not the stored admin.
/// * `ValidationFailed` - Quorum or threshold values are out of range.
pub fn set_params(
    env: &Env,
    min_votes_quorum: u32,
    approval_threshold_bps: u32,
) -> Result<(), Error> {
    if min_votes_quorum == 0
        || min_votes_quorum > MAX_VOTES_QUORUM
        || !(MIN_APPROVAL_THRESHOLD_BPS..=10000).contains(&approval_threshold_bps)
    {
        return Err(Error::ValidationFailed);
    }
    set_min_votes_quorum(env, min_votes_quorum);
    set_approval_threshold_bps(env, approval_threshold_bps);
    Ok(())
}

/// Records a vote (approve or reject) from a token-holding voter.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignAlreadyVerified` - The campaign is already verified.
/// * `CampaignNotActive` - The campaign is cancelled or inactive.
/// * `DeadlinePassed` - The voting period has closed (deadline exceeded).
/// * `NotTokenHolder` - The voter holds no tokens.
/// * `AlreadyVoted` - The voter has already cast a vote on this campaign.
pub fn cast_vote(env: &Env, campaign_id: u32, voter: Address, approve: bool) -> Result<(), Error> {
    voter.require_auth();

    let campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.funds_withdrawn {
        return Err(Error::CampaignNotActive);
    }
    require_active_campaign(&campaign)?;
    if env.ledger().timestamp() > campaign.deadline {
        return Err(Error::DeadlinePassed);
    }
    require_unverified_campaign(&campaign)?;

    let balance = token::Client::new(env, &get_token(env)).balance(&voter);
    if balance <= 0 {
        return Err(Error::NotTokenHolder);
    }

    let min_voting_balance = get_min_voting_balance(env);
    if balance < min_voting_balance {
        return Err(Error::NotTokenHolder);
    }

    if get_has_voted(env, campaign_id, &voter) {
        return Err(Error::AlreadyVoted);
    }

    if approve {
        let new_count = get_approve_votes(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_approve_votes(env, campaign_id, new_count);
        let new_weight = get_approve_weight(env, campaign_id)
            .checked_add(balance)
            .ok_or(Error::Overflow)?;
        set_approve_weight(env, campaign_id, new_weight);
    } else {
        let new_count = get_reject_votes(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_reject_votes(env, campaign_id, new_count);
        let new_weight = get_reject_weight(env, campaign_id)
            .checked_add(balance)
            .ok_or(Error::Overflow)?;
        set_reject_weight(env, campaign_id, new_weight);
    }

    set_has_voted(env, campaign_id, &voter);

    let vote_weight = balance;
    env.events().publish(
        ("campaign_vote_cast", campaign_id, voter),
        (approve, balance, vote_weight),
    );

    Ok(())
}

/// Directly verifies a campaign. May only be called by the admin.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignNotActive` - The campaign is cancelled or inactive.
/// * `AdminVerificationConflict` - The campaign is already verified.
pub fn admin_verify(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    if campaign.is_verified {
        return Err(Error::AdminVerificationConflict);
    }
    require_active_campaign(&campaign)?;

    campaign.is_verified = true;
    set_campaign(env, campaign_id, &campaign);
    increment_verified_campaign_count(env);
    env.events().publish(("campaign_verified", campaign_id), ());

    Ok(())
}

/// Checks vote counts against quorum and threshold, then marks the campaign verified if passed.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignNotActive` - The campaign is cancelled or inactive.
/// * `CommunityVerificationConflict` - The campaign is already verified.
/// * `VotingQuorumNotMet` - Fewer votes than the required quorum.
/// * `VotingThresholdNotMet` - Approval percentage is below the required threshold.
pub fn verify_with_votes(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    if campaign.is_verified {
        return Err(Error::CommunityVerificationConflict);
    }
    require_active_campaign(&campaign)?;

    let approve_votes = get_approve_votes(env, campaign_id);
    let reject_votes = get_reject_votes(env, campaign_id);
    let total_votes = approve_votes
        .checked_add(reject_votes)
        .ok_or(Error::Overflow)?;

    let min_quorum = get_min_votes_quorum(env, DEFAULT_MIN_VOTES_QUORUM);
    if total_votes < min_quorum {
        return Err(Error::VotingQuorumNotMet);
    }

    // Use token-weighted sums for the approval-threshold check.
    let approve_weight = get_approve_weight(env, campaign_id);
    let reject_weight = get_reject_weight(env, campaign_id);
    let total_weight = approve_weight
        .checked_add(reject_weight)
        .ok_or(Error::Overflow)?;

    let threshold = get_approval_threshold_bps(env, DEFAULT_APPROVAL_THRESHOLD_BPS);
    let approval_bps = if total_weight > 0 {
        // Use checked arithmetic to avoid silent overflow/truncation when
        // approve_weight is a large i128 (e.g. whale holders on 18-decimal tokens).
        approve_weight
            .checked_mul(10000)
            .and_then(|n| n.checked_div(total_weight))
            .unwrap_or(0) as u32
    } else {
        0
    };
    if approval_bps < threshold {
        return Err(Error::VotingThresholdNotMet);
    }

    campaign.is_verified = true;
    set_campaign(env, campaign_id, &campaign);
    increment_verified_campaign_count(env);
    env.events()
        .publish(("campaign_verified", campaign_id), approve_votes);

    Ok(())
}
