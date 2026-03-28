use soroban_sdk::{contracttype, Address, String};

/// Represents a category for a campaign, determining its type and eligibility for revenue sharing.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Category {
    Learner = 0,
    EducationalStartup = 1,
    Educator = 2,
    Publisher = 3,
}

/// Stores all details related to a funding campaign.
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

/// Keys representing the unique storage state for the contract.
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
    Version,
    ApproveVotes(u32),
    RejectVotes(u32),
    HasVoted(u32, Address),
    MinVotesQuorum,
    ApprovalThresholdBps,
}