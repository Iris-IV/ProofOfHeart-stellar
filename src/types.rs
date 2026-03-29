use soroban_sdk::{contracttype, Address, String};

/// Represents a category for a campaign, determining its type and eligibility for revenue sharing.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Category {
    /// A learner seeking funding for education.
    Learner = 0,
    /// An educational startup eligible for revenue sharing.
    EducationalStartup = 1,
    /// An educator creating learning content.
    Educator = 2,
    /// A publisher creating educational materials.
    Publisher = 3,
}

/// Stores all details related to a funding campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    /// Unique numeric identifier assigned at creation.
    pub id: u32,
    /// The address of the campaign creator.
    pub creator: Address,
    /// Short display name of the campaign.
    pub title: String,
    /// Long description of the campaign's purpose.
    pub description: String,
    /// Target token amount required to consider the campaign successful.
    pub funding_goal: i128,
    /// Unix timestamp after which contributions are no longer accepted.
    pub deadline: u64,
    /// Total tokens raised so far.
    pub amount_raised: i128,
    /// Whether the campaign is currently accepting contributions.
    pub is_active: bool,
    /// Whether the creator has already withdrawn funds.
    pub funds_withdrawn: bool,
    /// Whether the campaign has been cancelled by the creator.
    pub is_cancelled: bool,
    /// Whether the campaign has been verified (by admin or community vote).
    pub is_verified: bool,
    /// The category of the campaign.
    pub category: Category,
    /// Whether contributors are entitled to a share of future revenue.
    pub has_revenue_sharing: bool,
    /// Percentage of deposited revenue distributed to contributors, in basis points.
    pub revenue_share_percentage: u32,
}
