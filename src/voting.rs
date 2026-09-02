use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{transition, CampaignState};
use crate::storage::{
    extend_ttl,
    get_approval_threshold_bps, get_approve_votes, get_approve_weight,
    get_category_voting_threshold_bps, get_has_voted, get_min_votes_quorum, get_min_voting_balance,
    get_reject_votes, get_reject_weight, increment_verified_campaign_count,
    set_approval_threshold_bps, set_approve_votes, set_approve_weight, set_campaign, set_has_voted,
    set_min_votes_quorum, set_reject_votes, set_reject_weight,
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

/// Resolves the approval threshold that actually applies to `category`:
/// the per-category override if the admin has set one (#536), otherwise the
/// global configured default.
pub(crate) fn effective_approval_threshold_bps(env: &Env, category: crate::types::Category) -> u32 {
    get_category_voting_threshold_bps(env, category)
        .unwrap_or_else(|| get_approval_threshold_bps(env, DEFAULT_APPROVAL_THRESHOLD_BPS))
}

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
        || !(MIN_APPROVAL_THRESHOLD_BPS..=crate::BPS_DENOMINATOR).contains(&approval_threshold_bps)
    {
        return Err(Error::ValidationFailed);
    }
    set_min_votes_quorum(env, min_votes_quorum);
    set_approval_threshold_bps(env, approval_threshold_bps);
    Ok(())
}

/// Records a vote (approve or reject) from a token-holding voter.
///
/// Voting uses a 1-address-1-vote model (#469): every eligible token holder
/// gets exactly one vote, regardless of their token balance. This prevents
/// flash-loan attacks where an attacker borrows a large balance, votes with
/// inflated weight, and returns the tokens before verification.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignAlreadyVerified` - The campaign is already verified.
/// * `CampaignNotActive` - The campaign is cancelled or inactive.
/// * `DeadlinePassed` - The voting period has closed (deadline exceeded).
/// * `NotTokenHolder` - The voter holds no tokens or is below the minimum.
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

    // Deliberately the platform token, not the campaign's own currency (#784).
    // Voting weight is a platform-wide stake: measuring it in whatever asset a
    // campaign happens to be denominated in would let a creator pick an
    // obscure token and hand voting rights to whoever holds it.
    let balance = crate::lifecycle::token_client(env).balance(&voter);
    if balance <= 0 {
        return Err(Error::NotTokenHolder);
    }

    let min_voting_balance = get_min_voting_balance(env);
    if balance < min_voting_balance {
        return Err(Error::NotTokenHolder);
    }

    if get_has_voted(env, campaign_id, &voter) {
        extend_ttl(env, campaign_id, &voter);
        return Err(Error::AlreadyVoted);
    }

    // 1-address-1-vote: each voter contributes exactly 1 to the weight sum
    // regardless of token balance (#469).
    //
    // ApproveWeight/RejectWeight are deliberately kept as a mirror of the vote
    // counts (unit weight per vote) so the legacy storage layout and the
    // get_approve_weight/get_reject_weight queries stay consistent for
    // existing deployments and indexers. verify_with_votes only consults the
    // counts, so the mirror has no security impact.
    if approve {
        let new_count = get_approve_votes(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_approve_votes(env, campaign_id, new_count);
        let new_weight = get_approve_weight(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_approve_weight(env, campaign_id, new_weight);
    } else {
        let new_count = get_reject_votes(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_reject_votes(env, campaign_id, new_count);
        let new_weight = get_reject_weight(env, campaign_id)
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        set_reject_weight(env, campaign_id, new_weight);
    }

    set_has_voted(env, campaign_id, &voter);
    extend_ttl(env, campaign_id, &voter);

    env.events().publish(
        ("campaign_vote_cast", campaign_id, voter),
        // Data shape documented in EVENT_PAYLOADS.md as
        // (approve: bool, balance: i128, weight: i128). Balance is kept for
        // indexers as informational signal; weight is always the unit 1.
        (approve, balance, 1i128),
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
    transition(CampaignState::of(&campaign), CampaignState::Verified)?;

    campaign.is_verified = true;
    // set_campaign persists the verified campaign and refreshes its persistent
    // TTL through the shared persistent_set helper.
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
/// * `DeadlinePassed` - The voting period has closed (deadline exceeded).
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
    if env.ledger().timestamp() > campaign.deadline {
        return Err(Error::DeadlinePassed);
    }

    let approve_votes = get_approve_votes(env, campaign_id);
    let reject_votes = get_reject_votes(env, campaign_id);
    let total_votes = approve_votes
        .checked_add(reject_votes)
        .ok_or(Error::Overflow)?;

    let min_quorum = get_min_votes_quorum(env, DEFAULT_MIN_VOTES_QUORUM);
    if total_votes < min_quorum {
        return Err(Error::VotingQuorumNotMet);
    }

    // 1-address-1-vote (#469): threshold is computed from vote counts, not
    // token balances, so flash-loaned tokens cannot inflate the approval
    // percentage. The unwrap_or(0) below guards the division even if
    // total_votes were 0; with a non-zero quorum (the default, and the only
    // value set_params allows) the quorum check above already guarantees
    // total_votes > 0.
    let threshold = effective_approval_threshold_bps(env, campaign.category);
    let approval_bps = ((approve_votes as u64)
        .checked_mul(crate::BPS_DENOMINATOR as u64)
        .and_then(|n| n.checked_div(total_votes as u64))
        .unwrap_or(0)) as u32;
    if approval_bps < threshold {
        return Err(Error::VotingThresholdNotMet);
    }

    transition(CampaignState::of(&campaign), CampaignState::Verified)?;

    campaign.is_verified = true;
    set_campaign(env, campaign_id, &campaign);
    increment_verified_campaign_count(env);
    env.events()
        .publish(("campaign_verified", campaign_id), approve_votes);

    Ok(())
}
