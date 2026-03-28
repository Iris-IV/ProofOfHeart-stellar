#![no_std]
#![allow(unexpected_cfgs)]

/// Current contract version. Increment this on each breaking upgrade.
/// To upgrade a deployed Soroban contract, call `env.deployer().update_current_contract_wasm(new_wasm_hash)`
/// from an admin-guarded function after deploying the new WASM to the network. The storage layout
/// (DataKey variants, struct fields) must remain backwards-compatible unless a migration function
/// is included in the upgrade transaction.
const CONTRACT_VERSION: u32 = 1;

mod errors;
mod storage;
mod types;

pub use errors::Error;
pub use types::*;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String};
use storage::*;

/// The main contract struct for the Proof of Heart Stellar implementation.
#[contract]
pub struct ProofOfHeart;

const DEFAULT_MIN_VOTES_QUORUM: u32 = 3;
const DEFAULT_APPROVAL_THRESHOLD_BPS: u32 = 6000;

#[allow(clippy::too_many_arguments)]
#[contractimpl]
impl ProofOfHeart {
    /// Initializes the Proof of Heart contract.
    ///
    /// # Arguments
    /// * `admin` - The global admin address.
    /// * `token` - The required token for contributions and revenue.
    /// * `platform_fee` - The fee percentage taken from funds.
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn init(env: Env, admin: Address, token: Address, platform_fee: u32) -> Result<(), Error> {
        if has_admin(&env) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        set_admin(&env, &admin);
        set_token(&env, &token);

        let valid_fee = if platform_fee > 1000 { 1000 } else { platform_fee }; // Max 10% limit
        set_platform_fee(&env, valid_fee);
        set_campaign_count(&env, 0);
        set_version(&env, CONTRACT_VERSION);
        set_min_votes_quorum(&env, DEFAULT_MIN_VOTES_QUORUM);
        set_approval_threshold_bps(&env, DEFAULT_APPROVAL_THRESHOLD_BPS);

        env.events()
            .publish(("initialized", admin.clone()), (token.clone(), valid_fee));
        Ok(())
    }

    /// Creates a new campaign to raise funds for learning/educational uses.
    ///
    /// # Arguments
    /// * `creator` - The address of the individual/startup starting the campaign.
    /// * `title` - Short name of the campaign.
    /// * `description` - Long description of the campaign.
    /// * `funding_goal` - Target token amount.
    /// * `duration_days` - Number of days until deadline.
    /// * `category` - The specific categorical nature.
    /// * `has_revenue_sharing` - Should it enforce revenue deposits.
    /// * `revenue_share_percentage` - The percentage of share.
    ///
    /// # Returns
    /// The unique 32-bit `id` of the created campaign.
    ///
    /// # Authorization
    /// Requires `creator.require_auth()`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_campaign(
        env: Env,
        creator: Address,
        title: String,
        description: String,
        funding_goal: i128,
        duration_days: u64,
        category: Category,
        has_revenue_sharing: bool,
        revenue_share_percentage: u32,
    ) -> Result<u32, Error> {
        creator.require_auth();

        if funding_goal <= 0 {
            return Err(Error::FundingGoalMustBePositive);
        }
        if duration_days < 1 || duration_days > 365 {
        if !(1..=365).contains(&duration_days) {
            return Err(Error::InvalidDuration);
        }
        if title.len() == 0 || title.len() > 100 {
            return Err(Error::ValidationFailed);
        }
        if description.len() == 0 || description.len() > 1000 {
            return Err(Error::ValidationFailed);
        }
        if category != Category::EducationalStartup && has_revenue_sharing {
            return Err(Error::RevenueShareOnlyForStartup);
        }
        if has_revenue_sharing && (revenue_share_percentage == 0 || revenue_share_percentage > 5000)
        {
            return Err(Error::InvalidRevenueShare);
        }

        let mut count = get_campaign_count(&env);
        count += 1;

        let current_time = env.ledger().timestamp();
        let deadline = current_time + (duration_days * 86400);

        let campaign = Campaign {
            id: count,
            creator: creator.clone(),
            title: title.clone(),
            description,
            funding_goal,
            deadline,
            amount_raised: 0,
            is_active: true,
            funds_withdrawn: false,
            is_cancelled: false,
            is_verified: false,
            category,
            has_revenue_sharing,
            revenue_share_percentage,
        };

        set_campaign(&env, count, &campaign);
        set_campaign_count(&env, count);
        set_revenue_pool(&env, count, 0);

        env.events()
            .publish(("campaign_created", count, creator), title);

        Ok(count)
    }

    /// Contributes tokens to an active campaign.
    ///
    /// # Arguments
    /// * `campaign_id` - The ID of the campaign to contribute to.
    /// * `contributor` - The address performing the contribution.
    /// * `amount` - The non-zero amount to contribute.
    ///
    /// # Errors
    /// * `CampaignNotFound` - Campaign ID doesn't exist.
    /// * `CampaignNotActive` - Campaign is inactive or cancelled.
    /// * `DeadlinePassed` - Contribution after deadline.
    ///
    /// # Authorization
    /// Requires `contributor.require_auth()`.
    pub fn contribute(
        env: Env,
        campaign_id: u32,
        contributor: Address,
        amount: i128,
    ) -> Result<(), Error> {
        contributor.require_auth();

        if amount <= 0 {
            return Err(Error::ContributionMustBePositive);
        }

        let mut campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        if !campaign.is_active || campaign.is_cancelled {
            return Err(Error::CampaignNotActive);
        }
        if contributor == campaign.creator {
            return Err(Error::NotAuthorized);
        }
        if env.ledger().timestamp() > campaign.deadline {
            return Err(Error::DeadlinePassed);
        }

        let token_addr = get_token(&env);
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&contributor, &env.current_contract_address(), &amount);

        campaign.amount_raised += amount;
        set_campaign(&env, campaign_id, &campaign);

        let current = get_contribution(&env, campaign_id, &contributor);
        set_contribution(&env, campaign_id, &contributor, current + amount);

        env.events()
            .publish(("contribution_made", campaign_id, contributor), amount);

        Ok(())
    }

    /// Withdraws campaign funds if the funding goal was reached by the creator.
    ///
    /// # Arguments
    /// * `campaign_id` - ID of the target campaign.
    ///
    /// # Errors
    /// * `FundingGoalNotReached` - Target goal has not met.
    /// * `NoFundsToWithdraw` - Zero balance or already withdrawn.
    ///
    /// # Authorization
    /// Requires `campaign.creator.require_auth()`.
    pub fn withdraw_funds(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        campaign.creator.require_auth();

        if campaign.is_cancelled {
            return Err(Error::CampaignNotActive);
        }
        if campaign.funds_withdrawn {
            return Err(Error::FundsAlreadyWithdrawn);
        }
        if campaign.amount_raised == 0 {
            return Err(Error::NoFundsToWithdraw);
        }

        let time_remaining = env.ledger().timestamp() <= campaign.deadline;
        if campaign.amount_raised < campaign.funding_goal {
            if time_remaining {
                return Err(Error::FundingGoalNotReached);
            } else {
                return Err(Error::CampaignNotActive);
            }
        }

        let platform_fee = get_platform_fee(&env);
        let fee_amount = (campaign.amount_raised * (platform_fee as i128)) / 10000;
        let creator_amount = campaign.amount_raised - fee_amount;

        campaign.funds_withdrawn = true;
        campaign.is_active = false;
        set_campaign(&env, campaign_id, &campaign);

        let token_addr = get_token(&env);
        let admin_addr = get_admin(&env);
        let client = token::Client::new(&env, &token_addr);

        client.transfer(&env.current_contract_address(), &admin_addr, &fee_amount);
        client.transfer(
            &env.current_contract_address(),
            &campaign.creator,
            &creator_amount,
        );

        env.events().publish(
            ("withdrawal", campaign_id, campaign.creator.clone()),
            creator_amount,
        );

        Ok(())
    }

    /// Cancels a campaign. Can only be performed by the creator before funds are withdrawn.
    ///
    /// # Authorization
    /// Requires `campaign.creator.require_auth()`.
    pub fn cancel_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if campaign.funds_withdrawn {
            return Err(Error::ValidationFailed);
        }

        campaign.is_cancelled = true;
        campaign.is_active = false;
        set_campaign(&env, campaign_id, &campaign);

        env.events()
            .publish(("campaign_cancelled", campaign_id), ());

        Ok(())
    }

    /// Claim refunds for contributors if the campaign is cancelled or failed to reach the goal.
    ///
    /// # Authorization
    /// Requires `contributor.require_auth()`.
    pub fn claim_refund(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        contributor.require_auth();

        let campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        let failed_due_to_goal = env.ledger().timestamp() > campaign.deadline
            && campaign.amount_raised < campaign.funding_goal;

        if !(campaign.is_cancelled || failed_due_to_goal) {
            return Err(Error::ValidationFailed);
        }

        let amount = get_contribution(&env, campaign_id, &contributor);
        if amount == 0 {
            return Err(Error::NoFundsToWithdraw);
        }

        set_contribution(&env, campaign_id, &contributor, 0);

        let token_addr = get_token(&env);
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &amount);

        env.events()
            .publish(("refund_claimed", campaign_id, contributor), amount);

        Ok(())
    }

    /// Deposits revenue back into a profit-sharing campaign pool (For start-ups).
    ///
    /// # Authorization
    /// Requires `campaign.creator.require_auth()`.
    pub fn deposit_revenue(env: Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
        let campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if amount <= 0 {
            return Err(Error::ValidationFailed);
        }
        if !campaign.has_revenue_sharing {
            return Err(Error::ValidationFailed);
        }

        let token_addr = get_token(&env);
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&campaign.creator, &env.current_contract_address(), &amount);

        let current_pool = get_revenue_pool(&env, campaign_id);
        set_revenue_pool(&env, campaign_id, current_pool + amount);

        env.events()
            .publish(("revenue_deposited", campaign_id), amount);

        Ok(())
    }

    /// Claims a share of the revenue pool proportional to the individual contributor's contribution.
    pub fn claim_revenue(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        let campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;
        if !campaign.has_revenue_sharing {
            return Err(Error::ValidationFailed);
        }

        let contribution = get_contribution(&env, campaign_id, &contributor);
        if contribution == 0 {
            return Err(Error::ValidationFailed);
        }

        let total_pool = get_revenue_pool(&env, campaign_id);
        let total_due = (contribution * total_pool) / campaign.amount_raised;
        let already_claimed = get_revenue_claimed(&env, campaign_id, &contributor);
        let claimable = total_due - already_claimed;

        if claimable <= 0 {
            return Err(Error::NoFundsToWithdraw);
        }

        set_revenue_claimed(&env, campaign_id, &contributor, already_claimed + claimable);

        let token_addr = get_token(&env);
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &claimable);

        env.events()
            .publish(("revenue_claimed", campaign_id, contributor), claimable);

        Ok(())
    }

    /// Sets the community voting parameters for verifying a campaign.
    ///
    /// # Arguments
    /// * `admin` - The admin address.
    /// * `min_votes_quorum` - The minimum votes needed to reach quorum.
    /// * `approval_threshold_bps` - The approval threshold in basis points (100 = 1%).
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn set_voting_params(
        env: Env,
        admin: Address,
        min_votes_quorum: u32,
        approval_threshold_bps: u32,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            return Err(Error::NotAuthorized);
        }

        if min_votes_quorum == 0 || approval_threshold_bps == 0 || approval_threshold_bps > 10000 {
            return Err(Error::ValidationFailed);
        }

        set_min_votes_quorum(&env, min_votes_quorum);
        set_approval_threshold_bps(&env, approval_threshold_bps);

        Ok(())
    }

    /// Cast a vote on a campaign (approve or reject) to move it towards community verification.
    ///
    /// # Authorization
    /// Requires `voter.require_auth()`.
    pub fn vote_on_campaign(
        env: Env,
        campaign_id: u32,
        voter: Address,
        approve: bool,
    ) -> Result<(), Error> {
        voter.require_auth();

        let campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        if campaign.is_verified {
            return Err(Error::CampaignAlreadyVerified);
        }
        if campaign.is_cancelled || !campaign.is_active {
            return Err(Error::CampaignNotActive);
        }

        let token_addr = get_token(&env);
        let token_client = token::Client::new(&env, &token_addr);
        let voter_balance = token_client.balance(&voter);
        if voter_balance <= 0 {
            return Err(Error::NotTokenHolder);
        }

        if get_has_voted(&env, campaign_id, &voter) {
            return Err(Error::AlreadyVoted);
        }

        if approve {
            let current = get_approve_votes(&env, campaign_id);
            set_approve_votes(&env, campaign_id, current + 1);
        } else {
            let current = get_reject_votes(&env, campaign_id);
            set_reject_votes(&env, campaign_id, current + 1);
        }

        set_has_voted(&env, campaign_id, &voter);
        env.events()
            .publish(("campaign_vote_cast", campaign_id, voter), approve);

        Ok(())
    }

    /// Directly verify a campaign. Can only be performed by the admin.
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn verify_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let admin = get_admin(&env);
        admin.require_auth();

        let mut campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        if campaign.is_verified {
            return Err(Error::CampaignAlreadyVerified);
        }

        campaign.is_verified = true;
        set_campaign(&env, campaign_id, &campaign);
        env.events().publish(("campaign_verified", campaign_id), ());

        Ok(())
    }

    /// Checks if a campaign meets community verification thresholds and marks it verified.
    pub fn verify_campaign_with_votes(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign =
            get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)?;

        if campaign.is_verified {
            return Err(Error::CampaignAlreadyVerified);
        }

        let approve_votes = get_approve_votes(&env, campaign_id);
        let reject_votes = get_reject_votes(&env, campaign_id);
        let total_votes = approve_votes + reject_votes;

        let min_votes_quorum = get_min_votes_quorum(&env, DEFAULT_MIN_VOTES_QUORUM);
        if total_votes < min_votes_quorum {
            return Err(Error::VotingQuorumNotMet);
        }

        let approval_threshold_bps = get_approval_threshold_bps(&env, DEFAULT_APPROVAL_THRESHOLD_BPS);
        let approval_bps = (approve_votes * 10000) / total_votes;
        if approval_bps < approval_threshold_bps {
            return Err(Error::VotingThresholdNotMet);
        }

        campaign.is_verified = true;
        set_campaign(&env, campaign_id, &campaign);
        env.events()
            .publish(("campaign_verified", campaign_id), approve_votes);

        Ok(())
    }

    pub fn get_campaign(env: Env, campaign_id: u32) -> Campaign {
        env.storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap()
    /// Gets a campaign's current state.
    ///
    /// # Returns
    /// `Result<Campaign, Error>` where the Error is `CampaignNotFound` if the ID is invalid.
    pub fn get_campaign(env: Env, campaign_id: u32) -> Result<Campaign, Error> {
        get_campaign(&env, campaign_id).ok_or(Error::CampaignNotFound)
    }

    /// Gets the contributor's contribution amount for a specific campaign.
    pub fn get_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_contribution(&env, campaign_id, &contributor)
    }

    /// Gets the total revenue pool for a given campaign.
    pub fn get_revenue_pool(env: Env, campaign_id: u32) -> i128 {
        get_revenue_pool(&env, campaign_id)
    }

    /// Gets the total revenue claimed by a specific contributor.
    pub fn get_revenue_claimed(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        get_revenue_claimed(&env, campaign_id, &contributor)
    }

    pub fn get_campaign_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CampaignCount)
            .unwrap_or(0)
    /// Returns the current contract version stored in instance storage.
    /// A return value of 0 indicates the contract was initialized before version tracking was added.
    pub fn get_version(env: Env) -> u32 {
        get_version(&env)
    }

    /// Updates the global platform fee.
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn update_platform_fee(env: Env, new_fee: u32) -> Result<(), Error> {
        let admin = get_admin(&env);
        admin.require_auth();
        let valid_fee = if new_fee > 1000 { 1000 } else { new_fee };
        let old_fee = get_platform_fee(&env);
        set_platform_fee(&env, valid_fee);
        env.events().publish(("fee_updated",), (old_fee, valid_fee));
        Ok(())
    }

    /// Gets the number of recorded approval votes for a campaign.
    pub fn get_approve_votes(env: Env, campaign_id: u32) -> u32 {
        get_approve_votes(&env, campaign_id)
    }

    /// Gets the number of recorded rejection votes for a campaign.
    pub fn get_reject_votes(env: Env, campaign_id: u32) -> u32 {
        get_reject_votes(&env, campaign_id)
    }

    /// Checks if a voter has already voted on a specific campaign.
    pub fn has_voted(env: Env, campaign_id: u32, voter: Address) -> bool {
        get_has_voted(&env, campaign_id, &voter)
    }

    /// Gets the minimum votes needed to reach quorum.
    pub fn get_min_votes_quorum(env: Env) -> u32 {
        get_min_votes_quorum(&env, DEFAULT_MIN_VOTES_QUORUM)
    }

    /// Gets the required approval threshold in basis points.
    pub fn get_approval_threshold_bps(env: Env) -> u32 {
        get_approval_threshold_bps(&env, DEFAULT_APPROVAL_THRESHOLD_BPS)
    }

    pub fn get_admin(env: Env) -> Address {
        get_admin(&env)
    }

    pub fn get_token(env: Env) -> Address {
        get_token(&env)
    }

    pub fn get_platform_fee(env: Env) -> u32 {
        get_platform_fee(&env)
    }
}

mod test;