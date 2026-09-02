use soroban_sdk::{contracttype, Address, String};

/// Represents an optional pending campaign creator for ownership transfers.
/// Mirrors `Option<Address>` — used instead of the standard `Option` because
/// Soroban 20.1.0's `#[contracttype]` derive doesn't support `Option<Address>`
/// as a struct field: the generated `TryFrom<&Campaign> for ScVal` impl
/// requires `ScVal: From<Address>`, which the pinned `=20.1.0` SDK does not
/// provide (confirmed by re-testing `pending_creator: Option<Address>` against
/// this checkout — it fails to compile with that exact missing-impl error).
/// Revisit once the `soroban-sdk = "=20.1.0"` pin in Cargo.toml is lifted.
/// Same binary layout (`None == 0`, `Some(addr) == 1(addr)`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaybePendingCreator {
    /// No ownership transfer is in progress.
    None,
    /// An ownership transfer to this address is pending acceptance.
    Some(Address),
}

impl MaybePendingCreator {
    pub fn is_some(&self) -> bool {
        matches!(self, MaybePendingCreator::Some(_))
    }
    pub fn is_none(&self) -> bool {
        matches!(self, MaybePendingCreator::None)
    }
}

impl From<Address> for MaybePendingCreator {
    fn from(addr: Address) -> Self {
        MaybePendingCreator::Some(addr)
    }
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
    /// Immutable-by-design (always the first creator).
    pub first_creator: Address,
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
    /// Maximum tokens a single contributor may contribute in total, in
    /// lifetime-contribution terms. `0` is an explicit, intentional sentinel
    /// meaning "no per-user cap" (unlimited) — it is not treated as "0 tokens
    /// allowed" (#530). Negative values are rejected at creation
    /// (`create_campaign`); frontends should send `0` (not omit the field)
    /// when the creator wants no limit, and must not send `0` to mean "block
    /// all contributions" — there is no such state for this field.
    pub max_contribution_per_user: i128,
    /// Per-campaign platform fee override in basis points. None = use global fee.
    pub fee_override: Option<u32>,
    /// Whether the deadline has already been extended once.
    pub deadline_extended: bool,
    /// Total live contributions remaining after refunds, used for revenue-sharing pro-rata.
    pub effective_amount_raised: i128,
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
    /// Whether the counts are partial due to scan limit. When true,
    /// active/verified/cancelled counts may not reflect all campaigns.
    pub stats_are_partial: bool,
    /// The ID up to which the scan was performed.
    pub scanned_up_to: u32,
}

/// Comprehensive platform report for admin dashboards, returning all key
/// metrics in a single call (#541).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformReport {
    /// Total campaigns ever created.
    pub total_campaigns: u32,
    /// Campaigns currently active and not cancelled.
    pub active_campaigns: u32,
    /// Sum of `amount_raised` across all campaigns.
    pub total_raised: i128,
    /// Total number of distinct contributors across all campaigns.
    pub total_contributors: u32,
    /// Platform fee in basis points.
    pub platform_fee_bps: u32,
    /// Whether the contract is currently paused.
    pub is_paused: bool,
    /// The accepted token contract address.
    pub token: Address,
}

/// Aggregate metrics for a single creator across all of their campaigns,
/// used for creator-profile dashboards and indexer consumers (#519).
///
/// `is_known_creator` is the existence indicator: `false` means the address is
/// not a known creator, so the zero aggregate fields below mean "unknown
/// creator", not "known creator with no activity".
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorStats {
    /// Whether this address is known to have a campaign index entry.
    /// `false` means the address is not a known creator, so the zero aggregate
    /// values below should be read as "unknown creator", not "known creator
    /// with no activity". `true` means the address has at least one campaign.
    pub is_known_creator: bool,
    /// Total campaigns ever created by this creator.
    pub total_campaigns: u32,
    /// Campaigns currently active and not cancelled.
    pub active_campaigns: u32,
    /// Sum of `amount_raised` across all of the creator's campaigns.
    pub total_raised: i128,
    /// Sum of per-campaign contributor counts across all of the creator's
    /// campaigns. Note: a contributor who backs multiple campaigns by the
    /// same creator is counted once per campaign, not once overall.
    pub total_contributors: u32,
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
    /// Per-user contribution cap in tokens. `0` explicitly means "no cap"
    /// (unlimited), matching `Campaign::max_contribution_per_user` (#530).
    /// This is intentional, not a placeholder for "disabled" — negative
    /// values are the only rejected input.
    pub max_contribution_per_user: i128,
}

/// Represents a milestone for milestone-based withdrawals (#783).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    /// Unique identifier within the campaign (e.g. 0,1,2 ...).
    pub id: u32,
    /// Short description of the milestone.
    pub description: String,
    /// Payout share in basis points (out of 10_000). Must sum to 10_000 across all milestones.
    pub payout_bps: u32,
    /// Whether this milestone has been verified by admin/community.
    pub verified: bool,
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

/// A pending admin emergency withdrawal for a campaign whose funds are
/// otherwise unrecoverable (#802).
///
/// Written by `emergency_withdraw` and consumed by
/// `execute_emergency_withdrawal` once `execute_after` has passed. Stored
/// (not just emitted) so the pending proposal — and the exact timestamp it
/// becomes executable — is queryable on-chain via `get_emergency_withdrawal`
/// for the entire timelock window.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyWithdrawal {
    /// Address that will receive the recovered funds when the proposal is
    /// executed. Chosen by the admin at proposal time and fixed thereafter —
    /// changing it requires cancelling and re-proposing, which restarts the
    /// timelock.
    pub recipient: Address,
    /// Ledger timestamp at which the proposal was made.
    pub proposed_at: u64,
    /// Ledger timestamp on or after which `execute_emergency_withdrawal` may
    /// run. Always `proposed_at + EMERGENCY_WITHDRAWAL_TIMELOCK_SECS`.
    pub execute_after: u64,
}

/// Stats for a specific campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignStats {
    pub contributor_count: u32,
    pub top_contributor: MaybePendingCreator,
    pub avg_contribution: i128,
    pub last_contribution_time: u64,
}

/// Event payload emitted by `extend_campaign_deadline`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignDeadlineExtended {
    /// The campaign deadline before the extension.
    pub old_deadline: u64,
    /// The campaign deadline after the extension.
    pub new_deadline: u64,
    /// The extension duration requested by the creator, in days.
    pub additional_days: u64,
    /// The resulting total duration after the extension, in seconds.
    pub total_duration: u64,
}
