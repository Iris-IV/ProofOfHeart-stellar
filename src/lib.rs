#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    CampaignNotFound = 2,
    CampaignNotActive = 3,
    FundingGoalMustBePositive = 4,
    InvalidDuration = 5,
    InvalidRevenueShare = 6,
    RevenueShareOnlyForStartup = 7,
    DeadlinePassed = 8,
    ContributionMustBePositive = 9,
    DeadlineNotPassed = 10,
    FundsAlreadyWithdrawn = 11,
    FundingGoalNotReached = 12,
    NoFundsToWithdraw = 13,
    CampaignAlreadyVerified = 14,
    ValidationFailed = 15,
    AlreadyVoted = 16,
    NotTokenHolder = 17,
    VotingQuorumNotMet = 18,
    VotingThresholdNotMet = 19,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Category {
    Learner = 0,
    EducationalStartup = 1,
    Educator = 2,
    Publisher = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    pub id: u32,
    pub creator: Address,
    pub title: String,
    pub description: String,
    pub funding_goal: i128,
    pub deadline: u64,
    pub amount_raised: i128,
    pub is_active: bool,
    pub funds_withdrawn: bool,
    pub is_cancelled: bool,
    pub is_verified: bool,
    pub category: Category,
    pub has_revenue_sharing: bool,
    pub revenue_share_percentage: u32,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Token,
    PlatformFee,
    CampaignCount,
    Campaign(u32),
    Contribution(u32, Address),
    RevenuePool(u32),
    RevenueClaimed(u32, Address),
    ApproveVotes(u32),
    RejectVotes(u32),
    HasVoted(u32, Address),
    MinVotesQuorum,
    ApprovalThresholdBps,
}

#[contract]
pub struct ProofOfHeart;

const DEFAULT_MIN_VOTES_QUORUM: u32 = 3;
const DEFAULT_APPROVAL_THRESHOLD_BPS: u32 = 6000;

#[contractimpl]
impl ProofOfHeart {
    pub fn init(env: Env, admin: Address, token: Address, platform_fee: u32) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);

        let valid_fee = if platform_fee > 1000 { 1000 } else { platform_fee }; // Max 10% limit
        env.storage().instance().set(&DataKey::PlatformFee, &valid_fee);
        env.storage().instance().set(&DataKey::CampaignCount, &0u32);
        env.storage()
            .instance()
            .set(&DataKey::MinVotesQuorum, &DEFAULT_MIN_VOTES_QUORUM);
        env.storage()
            .instance()
            .set(&DataKey::ApprovalThresholdBps, &DEFAULT_APPROVAL_THRESHOLD_BPS);
    }

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

        if funding_goal <= 0 { return Err(Error::FundingGoalMustBePositive); }
        if duration_days < 1 || duration_days > 365 { return Err(Error::InvalidDuration); }
        if title.len() == 0 || title.len() > 100 { return Err(Error::ValidationFailed); }
        if description.len() == 0 || description.len() > 1000 { return Err(Error::ValidationFailed); }
        
        if category != Category::EducationalStartup && has_revenue_sharing {
            return Err(Error::RevenueShareOnlyForStartup);
        }

        if has_revenue_sharing && (revenue_share_percentage == 0 || revenue_share_percentage > 5000) {
            return Err(Error::InvalidRevenueShare);
        }

        let mut count: u32 = env.storage().instance().get(&DataKey::CampaignCount).unwrap_or(0);
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

        env.storage().instance().set(&DataKey::Campaign(count), &campaign);
        env.storage().instance().set(&DataKey::CampaignCount, &count);
        env.storage().instance().set(&DataKey::RevenuePool(count), &0i128);

        env.events().publish(("campaign_created", count, creator), title);

        Ok(count)
    }

    pub fn contribute(env: Env, campaign_id: u32, contributor: Address, amount: i128) -> Result<(), Error> {
        contributor.require_auth();

        if amount <= 0 { return Err(Error::ContributionMustBePositive); }

        let mut campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;

        if !campaign.is_active || campaign.is_cancelled { return Err(Error::CampaignNotActive); }
        if contributor == campaign.creator { return Err(Error::NotAuthorized); }
        if env.ledger().timestamp() > campaign.deadline { return Err(Error::DeadlinePassed); }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&contributor, &env.current_contract_address(), &amount);

        campaign.amount_raised += amount;
        env.storage().instance().set(&DataKey::Campaign(campaign_id), &campaign);

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let current_contribution: i128 = env.storage().instance().get(&contribution_key).unwrap_or(0);
        env.storage().instance().set(&contribution_key, &(current_contribution + amount));

        env.events().publish(("contribution_made", campaign_id, contributor), amount);

        Ok(())
    }

    pub fn withdraw_funds(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;

        campaign.creator.require_auth();

        if campaign.is_cancelled { return Err(Error::CampaignNotActive); }
        if campaign.funds_withdrawn { return Err(Error::FundsAlreadyWithdrawn); }
        if campaign.amount_raised == 0 { return Err(Error::NoFundsToWithdraw); }
        
        let time_remaining = env.ledger().timestamp() <= campaign.deadline;
        if campaign.amount_raised < campaign.funding_goal {
             if time_remaining {
                 return Err(Error::FundingGoalNotReached);
             } else {
                 return Err(Error::CampaignNotActive); 
             }
        }

        let platform_fee: u32 = env.storage().instance().get(&DataKey::PlatformFee).unwrap_or(300);
        let fee_amount = (campaign.amount_raised * (platform_fee as i128)) / 10000;
        let creator_amount = campaign.amount_raised - fee_amount;

        campaign.funds_withdrawn = true;
        campaign.is_active = false;
        env.storage().instance().set(&DataKey::Campaign(campaign_id), &campaign);

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let admin_addr: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let client = token::Client::new(&env, &token_addr);

        client.transfer(&env.current_contract_address(), &admin_addr, &fee_amount);
        client.transfer(&env.current_contract_address(), &campaign.creator, &creator_amount);

        env.events().publish(("withdrawal", campaign_id, campaign.creator.clone()), creator_amount);

        Ok(())
    }

    pub fn cancel_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if campaign.funds_withdrawn { return Err(Error::ValidationFailed); } 

        campaign.is_cancelled = true;
        campaign.is_active = false;
        env.storage().instance().set(&DataKey::Campaign(campaign_id), &campaign);

        env.events().publish(("campaign_cancelled", campaign_id), ());

        Ok(())
    }

    pub fn claim_refund(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        contributor.require_auth();

        let campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;

        let failed_due_to_goal = env.ledger().timestamp() > campaign.deadline && campaign.amount_raised < campaign.funding_goal;
        
        if !(campaign.is_cancelled || failed_due_to_goal) {
            return Err(Error::ValidationFailed); 
        }

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let amount: i128 = env.storage().instance().get(&contribution_key).unwrap_or(0);
        if amount == 0 { return Err(Error::NoFundsToWithdraw); }

        env.storage().instance().set(&contribution_key, &0i128);

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &amount);

        env.events().publish(("refund_claimed", campaign_id, contributor), amount);

        Ok(())
    }

    pub fn deposit_revenue(env: Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
        let campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if amount <= 0 { return Err(Error::ValidationFailed); }
        if !campaign.has_revenue_sharing { return Err(Error::ValidationFailed); }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        
        client.transfer(&campaign.creator, &env.current_contract_address(), &amount);

        let pool_key = DataKey::RevenuePool(campaign_id);
        let current_pool: i128 = env.storage().instance().get(&pool_key).unwrap_or(0);
        env.storage().instance().set(&pool_key, &(current_pool + amount));

        env.events().publish(("revenue_deposited", campaign_id), amount);

        Ok(())
    }

    pub fn claim_revenue(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        let campaign: Campaign = env.storage().instance().get(&DataKey::Campaign(campaign_id)).ok_or(Error::CampaignNotFound)?;
        if !campaign.has_revenue_sharing { return Err(Error::ValidationFailed); }

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let contribution: i128 = env.storage().instance().get(&contribution_key).unwrap_or(0);
        
        if contribution == 0 { return Err(Error::ValidationFailed); }

        let pool_key = DataKey::RevenuePool(campaign_id);
        let total_pool: i128 = env.storage().instance().get(&pool_key).unwrap_or(0);

        let total_due_to_contributor = (contribution * total_pool) / campaign.amount_raised;

        let claimed_key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
        let already_claimed: i128 = env.storage().instance().get(&claimed_key).unwrap_or(0);

        let claimable = total_due_to_contributor - already_claimed;

        if claimable <= 0 { return Err(Error::NoFundsToWithdraw); }

        env.storage().instance().set(&claimed_key, &(already_claimed + claimable));

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &claimable);

        env.events().publish(("revenue_claimed", campaign_id, contributor), claimable);

        Ok(())
    }

    pub fn set_voting_params(
        env: Env,
        admin: Address,
        min_votes_quorum: u32,
        approval_threshold_bps: u32,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            return Err(Error::NotAuthorized);
        }

        if min_votes_quorum == 0 || approval_threshold_bps == 0 || approval_threshold_bps > 10000 {
            return Err(Error::ValidationFailed);
        }

        env.storage()
            .instance()
            .set(&DataKey::MinVotesQuorum, &min_votes_quorum);
        env.storage()
            .instance()
            .set(&DataKey::ApprovalThresholdBps, &approval_threshold_bps);

        Ok(())
    }

    pub fn vote_on_campaign(
        env: Env,
        campaign_id: u32,
        voter: Address,
        approve: bool,
    ) -> Result<(), Error> {
        voter.require_auth();

        let campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        if campaign.is_verified {
            return Err(Error::CampaignAlreadyVerified);
        }
        if campaign.is_cancelled || !campaign.is_active {
            return Err(Error::CampaignNotActive);
        }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        let voter_balance = token_client.balance(&voter);
        if voter_balance <= 0 {
            return Err(Error::NotTokenHolder);
        }

        let has_voted_key = DataKey::HasVoted(campaign_id, voter.clone());
        let has_voted: bool = env.storage().instance().get(&has_voted_key).unwrap_or(false);
        if has_voted {
            return Err(Error::AlreadyVoted);
        }

        if approve {
            let key = DataKey::ApproveVotes(campaign_id);
            let current: u32 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(current + 1));
        } else {
            let key = DataKey::RejectVotes(campaign_id);
            let current: u32 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(current + 1));
        }

        env.storage().instance().set(&has_voted_key, &true);
        env.events()
            .publish(("campaign_vote_cast", campaign_id, voter), approve);

        Ok(())
    }

    pub fn verify_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        if campaign.is_verified {
            return Err(Error::CampaignAlreadyVerified);
        }

        let approve_votes: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ApproveVotes(campaign_id))
            .unwrap_or(0);
        let reject_votes: u32 = env
            .storage()
            .instance()
            .get(&DataKey::RejectVotes(campaign_id))
            .unwrap_or(0);
        let total_votes = approve_votes + reject_votes;

        let min_votes_quorum: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MinVotesQuorum)
            .unwrap_or(DEFAULT_MIN_VOTES_QUORUM);
        if total_votes < min_votes_quorum {
            return Err(Error::VotingQuorumNotMet);
        }

        let approval_threshold_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ApprovalThresholdBps)
            .unwrap_or(DEFAULT_APPROVAL_THRESHOLD_BPS);
        let approval_bps = (approve_votes * 10000) / total_votes;
        if approval_bps < approval_threshold_bps {
            return Err(Error::VotingThresholdNotMet);
        }

        campaign.is_verified = true;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.events()
            .publish(("campaign_verified", campaign_id), approve_votes);

        Ok(())
    }

    pub fn get_campaign(env: Env, campaign_id: u32) -> Campaign {
        env.storage().instance().get(&DataKey::Campaign(campaign_id)).unwrap()
    }

    pub fn get_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        env.storage().instance().get(&DataKey::Contribution(campaign_id, contributor)).unwrap_or(0)
    }

    pub fn get_revenue_pool(env: Env, campaign_id: u32) -> i128 {
        env.storage().instance().get(&DataKey::RevenuePool(campaign_id)).unwrap_or(0)
    }

    pub fn get_revenue_claimed(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        env.storage().instance().get(&DataKey::RevenueClaimed(campaign_id, contributor)).unwrap_or(0)
    }

    pub fn get_approve_votes(env: Env, campaign_id: u32) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ApproveVotes(campaign_id))
            .unwrap_or(0)
    }

    pub fn get_reject_votes(env: Env, campaign_id: u32) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::RejectVotes(campaign_id))
            .unwrap_or(0)
    }

    pub fn has_voted(env: Env, campaign_id: u32, voter: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::HasVoted(campaign_id, voter))
            .unwrap_or(false)
    }

    pub fn get_min_votes_quorum(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MinVotesQuorum)
            .unwrap_or(DEFAULT_MIN_VOTES_QUORUM)
    }

    pub fn get_approval_threshold_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ApprovalThresholdBps)
            .unwrap_or(DEFAULT_APPROVAL_THRESHOLD_BPS)
    }
}

mod test;
