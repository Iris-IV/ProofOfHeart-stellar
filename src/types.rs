use soroban_sdk::{contracttype, Address, String};

/// Represents an optional pending campaign creator for ownership transfers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaybePendingCreator {
    /// No ownership transfer is in progress.
    None,
    /// An ownership transfer to this address is pending acceptance.
    Some(Address),
}

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
    /// The address of the proposed new creator (for two-step transfer).
    pub pending_creator: MaybePendingCreator,
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
    /// Maximum tokens a single contributor may contribute in total. 0 means no cap.
    pub max_contribution_per_user: i128,
    /// Per-campaign platform fee override in basis points. None = use global fee.
    pub fee_override: Option<u32>,
    /// Whether the deadline has already been extended once.
    pub deadline_extended: bool,
}

/// Aggregate platform metrics for dashboard and indexer consumers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformStats {
    /// Total campaigns ever created.
    pub total_campaigns: u32,
    /// Campaigns currently active and not cancelled.
    pub active_campaigns: u32,
    /// Campaigns that were verified (admin or voting).
    pub verified_campaigns: u32,
    /// Campaigns cancelled by their creators.
    pub cancelled_campaigns: u32,
    /// Sum of `amount_raised` across all campaigns.
    pub total_amount_raised: i128,
}

/// Parameters for `create_campaign`, grouped into a single struct to avoid
/// positional-argument mistakes when calling via CLI or SDK.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateCampaignParams {
    /// The address of the campaign creator (must sign the transaction).
    pub creator: Address,
    /// Short display name (1–100 characters).
    pub title: String,
    /// Long description of the campaign's purpose (1–1000 characters).
    pub description: String,
    /// Target token amount (must be positive).
    pub funding_goal: i128,
    /// How long the campaign runs, in days (1–365).
    pub duration_days: u64,
    /// Campaign category; only `EducationalStartup` may use revenue sharing.
    pub category: Category,
    /// Whether contributors receive a share of future revenue.
    pub has_revenue_sharing: bool,
    /// Contributor revenue share in basis points (1–5000). Ignored (stored as 0) when
    /// `has_revenue_sharing` is `false`.
    pub revenue_share_percentage: u32,
    /// Per-user contribution cap in tokens. `0` means no cap.
    pub max_contribution_per_user: i128,
}

/// Stores details about withheld funds for a campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignReserve {
    /// The amount held in reserve.
    pub amount: i128,
    /// Unix timestamp after which the reserve can be released.
    pub release_timestamp: u64,
    /// Whether the reserve has already been released.
    pub released: bool,
}
