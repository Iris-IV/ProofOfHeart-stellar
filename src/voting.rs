use soroban_sdk::{token, Address, Env};

use crate::errors::Error;
use crate::storage::{
    get_admin, get_approval_threshold_bps, get_approve_votes, get_campaign, get_has_voted,
    get_min_votes_quorum, get_reject_votes, get_token, set_approval_threshold_bps,
    set_approve_votes, set_campaign, set_has_voted, set_min_votes_quorum, set_reject_votes,
};

/// Default minimum number of votes required to reach quorum.
pub const DEFAULT_MIN_VOTES_QUORUM: u32 = 3;

/// Default approval threshold in basis points (60%).
pub const DEFAULT_APPROVAL_THRESHOLD_BPS: u32 = 6000;

/// Updates the community voting parameters.
///
/// # Errors
/// * `NotAuthorized` - Caller is not the stored admin.
/// * `ValidationFailed` - Quorum or threshold values are out of range.
pub fn set_params(
    env: &Env,
    admin: Address,
    min_votes_quorum: u32,
    approval_threshold_bps: u32,
) -> Result<(), Error> {
    admin.require_auth();
    let stored_admin = get_admin(env);
    if admin != stored_admin {
        return Err(Error::NotAuthorized);
    }
    if min_votes_quorum == 0 || approval_threshold_bps == 0 || approval_threshold_bps > 10000 {
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
/// * `NotTokenHolder` - The voter holds no tokens.
/// * `AlreadyVoted` - The voter has already cast a vote on this campaign.
pub fn cast_vote(env: &Env, campaign_id: u32, voter: Address, approve: bool) -> Result<(), Error> {
    voter.require_auth();

    let campaign = get_campaign(env, campaign_id).ok_or(Error::CampaignNotFound)?;

    if campaign.is_verified {
        return Err(Error::CampaignAlreadyVerified);
    }
    if campaign.is_cancelled || !campaign.is_active {
        return Err(Error::CampaignNotActive);
    }

    let token_addr = get_token(env);
    let token_client = token::Client::new(env, &token_addr);
    if token_client.balance(&voter) <= 0 {
        return Err(Error::NotTokenHolder);
    }

    if get_has_voted(env, campaign_id, &voter) {
        return Err(Error::AlreadyVoted);
    }

    if approve {
        set_approve_votes(env, campaign_id, get_approve_votes(env, campaign_id) + 1);
    } else {
        set_reject_votes(env, campaign_id, get_reject_votes(env, campaign_id) + 1);
    }

    set_has_voted(env, campaign_id, &voter);
    env.events()
        .publish(("campaign_vote_cast", campaign_id, voter), approve);

    Ok(())
}

/// Directly verifies a campaign. May only be called by the admin.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignAlreadyVerified` - The campaign is already verified.
pub fn admin_verify(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let admin = get_admin(env);
    admin.require_auth();

    let mut campaign = get_campaign(env, campaign_id).ok_or(Error::CampaignNotFound)?;

    if campaign.is_verified {
        return Err(Error::CampaignAlreadyVerified);
    }

    campaign.is_verified = true;
    set_campaign(env, campaign_id, &campaign);
    env.events().publish(("campaign_verified", campaign_id), ());

    Ok(())
}

/// Checks vote counts against quorum and threshold, then marks the campaign verified if passed.
///
/// # Errors
/// * `CampaignNotFound` - No campaign with the given ID.
/// * `CampaignAlreadyVerified` - The campaign is already verified.
/// * `VotingQuorumNotMet` - Fewer votes than the required quorum.
/// * `VotingThresholdNotMet` - Approval percentage is below the required threshold.
pub fn verify_with_votes(env: &Env, campaign_id: u32) -> Result<(), Error> {
    let mut campaign = get_campaign(env, campaign_id).ok_or(Error::CampaignNotFound)?;

    if campaign.is_verified {
        return Err(Error::CampaignAlreadyVerified);
    }

    let approve_votes = get_approve_votes(env, campaign_id);
    let reject_votes = get_reject_votes(env, campaign_id);
    let total_votes = approve_votes + reject_votes;

    let min_quorum = get_min_votes_quorum(env, DEFAULT_MIN_VOTES_QUORUM);
    if total_votes < min_quorum {
        return Err(Error::VotingQuorumNotMet);
    }

    let threshold = get_approval_threshold_bps(env, DEFAULT_APPROVAL_THRESHOLD_BPS);
    let approval_bps = (approve_votes * 10000) / total_votes;
    if approval_bps < threshold {
        return Err(Error::VotingThresholdNotMet);
    }

    campaign.is_verified = true;
    set_campaign(env, campaign_id, &campaign);
    env.events()
        .publish(("campaign_verified", campaign_id), approve_votes);

    Ok(())
}
