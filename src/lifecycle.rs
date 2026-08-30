use soroban_sdk::{token, Address, Env};

use crate::errors::Error;
use crate::storage::{get_admin, get_campaign, get_campaign_start_time, get_token, AdminKey};
use crate::types::Campaign;

/// A campaign's position in its lifecycle, derived from its stored flags.
///
/// This is the single place that documents which lifecycle transitions are
/// valid (#537), complementing — not replacing — the `require_*` guards
/// below. The guards check *preconditions* for an action (is the campaign
/// paused, has the deadline passed, is the caller authorized); `transition`
/// checks that the state change an action is about to make is one the
/// lifecycle actually allows. `is_active` and `is_verified` are independent
/// flags on `Campaign` (a campaign can be active and verified at once), so
/// `of` derives one dominant state by priority: a cancelled or withdrawn
/// campaign is always terminal regardless of its other flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignState {
    /// Accepting contributions; not yet verified.
    Active,
    /// Verified (by admin or community vote); still open for withdrawal.
    Verified,
    /// Creator has withdrawn funds. Terminal.
    Withdrawn,
    /// Cancelled by the creator. Terminal.
    Cancelled,
}

impl CampaignState {
    /// Derives the dominant lifecycle state from a campaign's flags, in
    /// priority order: `Cancelled` > `Withdrawn` > `Verified` > `Active`.
    pub fn of(campaign: &Campaign) -> Self {
        if campaign.is_cancelled {
            CampaignState::Cancelled
        } else if campaign.funds_withdrawn {
            CampaignState::Withdrawn
        } else if campaign.is_verified {
            CampaignState::Verified
        } else {
            CampaignState::Active
        }
    }
}

/// Validates a campaign lifecycle transition, returning
/// `Error::InvalidStateTransition` if `to` is not reachable from `from`.
///
/// Valid transitions: `Active -> Verified` (admin or community verification),
/// `Active -> Cancelled` and `Verified -> Cancelled` (creator cancellation),
/// `Verified -> Withdrawn` (funds withdrawal). `Withdrawn` and `Cancelled`
/// are terminal — nothing transitions out of them.
pub(crate) fn transition(from: CampaignState, to: CampaignState) -> Result<(), Error> {
    use CampaignState::*;
    let allowed = matches!(
        (from, to),
        (Active, Verified) | (Active, Cancelled) | (Verified, Cancelled) | (Verified, Withdrawn)
    );
    if allowed {
        Ok(())
    } else {
        Err(Error::InvalidStateTransition)
    }
}

pub(crate) fn get_campaign_or_error(env: &Env, campaign_id: u32) -> Result<Campaign, Error> {
    get_campaign(env, campaign_id).ok_or(Error::CampaignNotFound)
}

pub(crate) fn get_creator_campaign(env: &Env, campaign_id: u32) -> Result<Campaign, Error> {
    let campaign = get_campaign_or_error(env, campaign_id)?;
    assert_creator(&campaign)?;
    Ok(campaign)
}

/// #787: `Campaign` is a heavy struct (contains Strings, Addresses, 20+ fields)
/// so validation helpers take `&Campaign` to avoid unnecessary WASM copying.
pub(crate) fn assert_creator(campaign: &Campaign) -> Result<(), Error> {
    campaign.creator.require_auth();
    Ok(())
}

pub(crate) fn assert_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin = get_admin(env);
    if *caller != admin {
        return Err(Error::NotAuthorized);
    }
    caller.require_auth();
    Ok(())
}

pub(crate) fn require_active_campaign(campaign: &Campaign) -> Result<(), Error> {
    if campaign.is_cancelled || !campaign.is_active {
        return Err(Error::CampaignNotActive);
    }
    Ok(())
}

pub(crate) fn require_unverified_campaign(campaign: &Campaign) -> Result<(), Error> {
    if campaign.is_verified {
        return Err(Error::CampaignAlreadyVerified);
    }
    Ok(())
}

pub(crate) fn require_revenue_sharing(campaign: &Campaign, error: Error) -> Result<(), Error> {
    if !campaign.has_revenue_sharing {
        return Err(error);
    }
    Ok(())
}

pub(crate) fn calculate_deadline(current_time: u64, duration_days: u64) -> Result<u64, Error> {
    let seconds_in_duration = duration_days
        .checked_mul(crate::SECONDS_PER_DAY)
        .ok_or(Error::ValidationFailed)?;
    current_time
        .checked_add(seconds_in_duration)
        .ok_or(Error::ValidationFailed)
}

pub(crate) fn require_not_paused(env: &Env) -> Result<(), Error> {
    if env
        .storage()
        .instance()
        .get(&AdminKey::Paused)
        .unwrap_or(false)
        || env
            .storage()
            .instance()
            .get(&AdminKey::AutoPaused)
            .unwrap_or(false)
    {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

pub(crate) fn token_client(env: &Env) -> token::Client<'_> {
    token::Client::new(env, &get_token(env))
}

pub(crate) fn campaign_start_time_or_error(env: &Env, campaign_id: u32) -> Result<u64, Error> {
    get_campaign_start_time(env, campaign_id).ok_or(Error::InvalidDuration)
}

#[cfg(test)]
mod tests {
    use super::{transition, CampaignState};
    use crate::types::{Campaign, Category, MaybePendingCreator};
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn make_campaign(
        env: &Env,
        is_active: bool,
        funds_withdrawn: bool,
        is_cancelled: bool,
        is_verified: bool,
    ) -> Campaign {
        let creator = Address::generate(env);
        Campaign {
            id: 1,
            creator: creator.clone(),
            first_creator: creator,
            pending_creator: MaybePendingCreator::None,
            title: String::from_str(env, "t"),
            description: String::from_str(env, "d"),
            funding_goal: 1_000,
            deadline: 0,
            amount_raised: 0,
            is_active,
            funds_withdrawn,
            is_cancelled,
            is_verified,
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
    fn state_of_active_unverified() {
        let env = Env::default();
        let campaign = make_campaign(&env, true, false, false, false);
        assert_eq!(CampaignState::of(&campaign), CampaignState::Active);
    }

    #[test]
    fn state_of_verified_still_open() {
        let env = Env::default();
        let campaign = make_campaign(&env, true, false, false, true);
        assert_eq!(CampaignState::of(&campaign), CampaignState::Verified);
    }

    #[test]
    fn state_of_withdrawn_takes_priority_over_verified() {
        let env = Env::default();
        let campaign = make_campaign(&env, false, true, false, true);
        assert_eq!(CampaignState::of(&campaign), CampaignState::Withdrawn);
    }

    #[test]
    fn state_of_cancelled_takes_priority_over_everything() {
        let env = Env::default();
        // Cancelled and (implausibly) also verified/withdrawn — Cancelled wins.
        let campaign = make_campaign(&env, false, true, true, true);
        assert_eq!(CampaignState::of(&campaign), CampaignState::Cancelled);
    }

    #[test]
    fn active_to_verified_is_allowed() {
        assert_eq!(
            transition(CampaignState::Active, CampaignState::Verified),
            Ok(())
        );
    }

    #[test]
    fn active_to_cancelled_is_allowed() {
        assert_eq!(
            transition(CampaignState::Active, CampaignState::Cancelled),
            Ok(())
        );
    }

    #[test]
    fn verified_to_cancelled_is_allowed() {
        assert_eq!(
            transition(CampaignState::Verified, CampaignState::Cancelled),
            Ok(())
        );
    }

    #[test]
    fn verified_to_withdrawn_is_allowed() {
        assert_eq!(
            transition(CampaignState::Verified, CampaignState::Withdrawn),
            Ok(())
        );
    }

    #[test]
    fn withdrawn_is_terminal() {
        assert!(transition(CampaignState::Withdrawn, CampaignState::Cancelled).is_err());
        assert!(transition(CampaignState::Withdrawn, CampaignState::Verified).is_err());
        assert!(transition(CampaignState::Withdrawn, CampaignState::Active).is_err());
    }

    #[test]
    fn cancelled_is_terminal() {
        assert!(transition(CampaignState::Cancelled, CampaignState::Active).is_err());
        assert!(transition(CampaignState::Cancelled, CampaignState::Verified).is_err());
        assert!(transition(CampaignState::Cancelled, CampaignState::Withdrawn).is_err());
    }

    #[test]
    fn active_to_withdrawn_is_rejected_without_verification() {
        assert!(transition(CampaignState::Active, CampaignState::Withdrawn).is_err());
    }

    #[test]
    fn no_transition_to_active() {
        assert!(transition(CampaignState::Verified, CampaignState::Active).is_err());
        assert!(transition(CampaignState::Cancelled, CampaignState::Active).is_err());
        assert!(transition(CampaignState::Withdrawn, CampaignState::Active).is_err());
    }
}
