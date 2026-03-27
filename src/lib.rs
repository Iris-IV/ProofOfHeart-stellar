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
}

#[contract]
pub struct ProofOfHeart;

#[contractimpl]
impl ProofOfHeart {
    pub fn init(env: Env, admin: Address, token: Address, platform_fee: u32) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);

        let valid_fee = if platform_fee > 1000 {
            1000
        } else {
            platform_fee
        }; // Max 10% limit
        env.storage()
            .instance()
            .set(&DataKey::PlatformFee, &valid_fee);
        env.storage().instance().set(&DataKey::CampaignCount, &0u32);
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

        if funding_goal <= 0 {
            return Err(Error::FundingGoalMustBePositive);
        }
        if duration_days < 1 || duration_days > 365 {
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

        let mut count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CampaignCount)
            .unwrap_or(0);
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

        env.storage()
            .instance()
            .set(&DataKey::Campaign(count), &campaign);
        env.storage()
            .instance()
            .set(&DataKey::CampaignCount, &count);
        env.storage()
            .instance()
            .set(&DataKey::RevenuePool(count), &0i128);

        env.events()
            .publish(("campaign_created", count, creator), title);

        Ok(count)
    }

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

        let mut campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        if !campaign.is_active || campaign.is_cancelled {
            return Err(Error::CampaignNotActive);
        }
        if env.ledger().timestamp() > campaign.deadline {
            return Err(Error::DeadlinePassed);
        }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&contributor, &env.current_contract_address(), &amount);

        campaign.amount_raised += amount;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let current_contribution: i128 =
            env.storage().instance().get(&contribution_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&contribution_key, &(current_contribution + amount));

        env.events()
            .publish(("contribution_made", campaign_id, contributor), amount);

        Ok(())
    }

    pub fn withdraw_funds(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

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

        let platform_fee: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFee)
            .unwrap_or(300);
        let fee_amount = (campaign.amount_raised * (platform_fee as i128)) / 10000;
        let creator_amount = campaign.amount_raised - fee_amount;

        campaign.funds_withdrawn = true;
        campaign.is_active = false;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let admin_addr: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
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

    pub fn cancel_campaign(env: Env, campaign_id: u32) -> Result<(), Error> {
        let mut campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if campaign.funds_withdrawn {
            return Err(Error::ValidationFailed);
        }

        campaign.is_cancelled = true;
        campaign.is_active = false;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        env.events()
            .publish(("campaign_cancelled", campaign_id), ());

        Ok(())
    }

    pub fn claim_refund(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        let campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        let failed_due_to_goal = env.ledger().timestamp() > campaign.deadline
            && campaign.amount_raised < campaign.funding_goal;

        if !(campaign.is_cancelled || failed_due_to_goal) {
            return Err(Error::ValidationFailed);
        }

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let amount: i128 = env.storage().instance().get(&contribution_key).unwrap_or(0);
        if amount == 0 {
            return Err(Error::NoFundsToWithdraw);
        }

        env.storage().instance().set(&contribution_key, &0i128);

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &amount);

        env.events()
            .publish(("refund_claimed", campaign_id, contributor), amount);

        Ok(())
    }

    pub fn deposit_revenue(env: Env, campaign_id: u32, amount: i128) -> Result<(), Error> {
        let campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;
        campaign.creator.require_auth();

        if amount <= 0 {
            return Err(Error::ValidationFailed);
        }
        if !campaign.has_revenue_sharing {
            return Err(Error::ValidationFailed);
        }

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);

        client.transfer(&campaign.creator, &env.current_contract_address(), &amount);

        let pool_key = DataKey::RevenuePool(campaign_id);
        let current_pool: i128 = env.storage().instance().get(&pool_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&pool_key, &(current_pool + amount));

        env.events()
            .publish(("revenue_deposited", campaign_id), amount);

        Ok(())
    }

    pub fn claim_revenue(env: Env, campaign_id: u32, contributor: Address) -> Result<(), Error> {
        let campaign: Campaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;
        if !campaign.has_revenue_sharing {
            return Err(Error::ValidationFailed);
        }

        let contribution_key = DataKey::Contribution(campaign_id, contributor.clone());
        let contribution: i128 = env.storage().instance().get(&contribution_key).unwrap_or(0);

        if contribution == 0 {
            return Err(Error::ValidationFailed);
        }

        let pool_key = DataKey::RevenuePool(campaign_id);
        let total_pool: i128 = env.storage().instance().get(&pool_key).unwrap_or(0);

        let total_due_to_contributor = (contribution * total_pool) / campaign.amount_raised;

        let claimed_key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
        let already_claimed: i128 = env.storage().instance().get(&claimed_key).unwrap_or(0);

        let claimable = total_due_to_contributor - already_claimed;

        if claimable <= 0 {
            return Err(Error::NoFundsToWithdraw);
        }

        env.storage()
            .instance()
            .set(&claimed_key, &(already_claimed + claimable));

        let token_addr: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &contributor, &claimable);

        env.events()
            .publish(("revenue_claimed", campaign_id, contributor), claimable);

        Ok(())
    }

    pub fn get_campaign(env: Env, campaign_id: u32) -> Campaign {
        env.storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .unwrap()
    }

    pub fn get_contribution(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Contribution(campaign_id, contributor))
            .unwrap_or(0)
    }

    pub fn get_revenue_pool(env: Env, campaign_id: u32) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::RevenuePool(campaign_id))
            .unwrap_or(0)
    }

    pub fn get_revenue_claimed(env: Env, campaign_id: u32, contributor: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::RevenueClaimed(campaign_id, contributor))
            .unwrap_or(0)
    }

    pub fn get_campaign_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CampaignCount)
            .unwrap_or(0)
    }
}

mod test;
