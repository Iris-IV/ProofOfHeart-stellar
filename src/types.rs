use soroban_sdk::{contracttype, Address, String};

/// Represents the lifecycle state of a campaign as a single strongly-typed
/// enum, replacing the previous ad-hoc combination of boolean flags.
///
/// The discriminant ordering mirrors the typical lifecycle progression but
/// does **not** enforce transitions — the contract modules handle that.
/// `#[contracttype]` is required for Soroban storage serialization.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CampaignStatus {
    /// Campaign is live and accepting contributions.
    Active,
    /// Campaign was cancelled by its creator or admin.
    Cancelled,
    /// Campaign met its goal and funds were withdrawn by the creator.
    Withdrawn,
}

/// Represents an optional pending campaign creator for ownership transfers.
/// Using `Option<Address>` directly since soroban-sdk 22.0.11 supports it.
/// Same binary layout as the previous `MaybePendingCreator` enum (`None == 0`, `Some(addr) == 1(addr)`).
pub type MaybePendingCreator = Option<Address>;

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
    /// Unix timestamp after which a pending `pending_creator` nomination can
    /// no longer be accepted (#869). Meaningless while `pending_creator` is
    /// `None`; always set together with it.
    pub pending_creator_expiry: u64,
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
    pub status: u32,
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

impl Campaign {
    /// Derives the current lifecycle status from the stored boolean flags.
    pub fn status(&self) -> CampaignStatus {
        if self.is_cancelled() {
            CampaignStatus::Cancelled
        } else if self.funds_withdrawn() {
            CampaignStatus::Withdrawn
        } else {
            CampaignStatus::Active
        }
    }
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
    /// Whether the reported counts should not be trusted as an accurate or
    /// complete picture. There is no scan limit anymore (#411), so in a
    /// healthy contract this is always `false`. It is set to `true` only when
    /// `get_platform_stats` detects that the stored counters violate the
    /// consistency invariants checked by
    /// `queries::counters_are_consistent` (e.g. `active_campaigns >
    /// total_campaigns` after a partial migration or a failed legacy write).
    /// Consumers should treat `true` as a signal that the aggregate counts
    /// are corrupted and must not be displayed or relied upon until the
    /// counters are reconciled.
    pub stats_are_partial: bool,
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
    pub top_contributor: Option<Address>,
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

pub const CAMPAIGN_ACTIVE: u32 = 1 << 0;
pub const CAMPAIGN_PAUSED: u32 = 1 << 1;
pub const CAMPAIGN_VALIDATED: u32 = 1 << 2;
pub const CAMPAIGN_SUCCEEDED: u32 = 1 << 3;

impl Campaign {
    pub fn is_active(&self) -> bool {
        (self.status & CAMPAIGN_ACTIVE) != 0
    }
    pub fn set_active(&mut self, val: bool) {
        if val {
            self.status |= CAMPAIGN_ACTIVE;
        } else {
            self.status &= !CAMPAIGN_ACTIVE;
        }
    }

    pub fn is_cancelled(&self) -> bool {
        (self.status & CAMPAIGN_PAUSED) != 0
    }
    pub fn set_cancelled(&mut self, val: bool) {
        if val {
            self.status |= CAMPAIGN_PAUSED;
        } else {
            self.status &= !CAMPAIGN_PAUSED;
        }
    }

    pub fn is_verified(&self) -> bool {
        (self.status & CAMPAIGN_VALIDATED) != 0
    }
    pub fn set_verified(&mut self, val: bool) {
        if val {
            self.status |= CAMPAIGN_VALIDATED;
        } else {
            self.status &= !CAMPAIGN_VALIDATED;
        }
    }

    pub fn funds_withdrawn(&self) -> bool {
        (self.status & CAMPAIGN_SUCCEEDED) != 0
    }
    pub fn set_funds_withdrawn(&mut self, val: bool) {
        if val {
            self.status |= CAMPAIGN_SUCCEEDED;
        } else {
            self.status &= !CAMPAIGN_SUCCEEDED;
        }
    }

    /// Packs the campaign's boolean lifecycle and configuration flags into a
    /// single 32-bit bitmask for compact XDR state serialization (#1014).
    pub fn packed_state_flags(&self) -> u32 {
        let mut flags = self.status
            & (CAMPAIGN_ACTIVE | CAMPAIGN_PAUSED | CAMPAIGN_VALIDATED | CAMPAIGN_SUCCEEDED);
        if self.has_revenue_sharing {
            flags |= CAMPAIGN_REVENUE_SHARING_FLAG;
        }
        if self.deadline_extended {
            flags |= CAMPAIGN_DEADLINE_EXTENDED_FLAG;
        }
        flags
    }

    /// Projects the full `Campaign` record into a lightweight, fixed-width
    /// `CompactCampaignSummary` that omits variable-length `title` and
    /// `description` strings to minimize ledger XDR read/serialization cost (#1014).
    pub fn to_compact_summary(&self) -> CompactCampaignSummary {
        CompactCampaignSummary {
            id: self.id,
            creator: self.creator.clone(),
            funding_goal: self.funding_goal,
            amount_raised: self.amount_raised,
            effective_amount_raised: self.effective_amount_raised,
            deadline: self.deadline,
            category: self.category,
            packed_flags: self.packed_state_flags(),
            revenue_share_percentage: self.revenue_share_percentage,
        }
    }
}

/// Bitmask flag indicating `has_revenue_sharing` in `CompactCampaignSummary::packed_flags` (#1014).
pub const CAMPAIGN_REVENUE_SHARING_FLAG: u32 = 1 << 4;
/// Bitmask flag indicating `deadline_extended` in `CompactCampaignSummary::packed_flags` (#1014).
pub const CAMPAIGN_DEADLINE_EXTENDED_FLAG: u32 = 1 << 5;

/// Bitmask flags for `CompactUserCampaignState::packed_flags` (#1014).
pub const USER_STATE_BOOKMARKED: u32 = 1 << 0;
pub const USER_STATE_VOTED: u32 = 1 << 1;
pub const USER_STATE_REFUNDED: u32 = 1 << 2;

/// Compact, fixed-size Soroban state summary for a campaign (#1014).
///
/// Packs lifecycle and configuration booleans into `packed_flags` and excludes
/// variable-length string metadata (`title`, `description`) to minimize XDR
/// serialization overhead during batch queries and storage reads.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactCampaignSummary {
    pub id: u32,
    pub creator: Address,
    pub funding_goal: i128,
    pub amount_raised: i128,
    pub effective_amount_raised: i128,
    pub deadline: u64,
    pub category: Category,
    pub packed_flags: u32,
    pub revenue_share_percentage: u32,
}

impl CompactCampaignSummary {
    pub const fn is_active(&self) -> bool {
        (self.packed_flags & CAMPAIGN_ACTIVE) != 0
    }

    pub const fn is_cancelled(&self) -> bool {
        (self.packed_flags & CAMPAIGN_PAUSED) != 0
    }

    pub const fn is_verified(&self) -> bool {
        (self.packed_flags & CAMPAIGN_VALIDATED) != 0
    }

    pub const fn funds_withdrawn(&self) -> bool {
        (self.packed_flags & CAMPAIGN_SUCCEEDED) != 0
    }

    pub const fn has_revenue_sharing(&self) -> bool {
        (self.packed_flags & CAMPAIGN_REVENUE_SHARING_FLAG) != 0
    }

    pub const fn deadline_extended(&self) -> bool {
        (self.packed_flags & CAMPAIGN_DEADLINE_EXTENDED_FLAG) != 0
    }
}

/// Compact per-user state for a campaign combining contribution balance,
/// claimed revenue, and bit-packed interaction flags (#1014).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactUserCampaignState {
    pub contribution: i128,
    pub claimed_revenue: i128,
    pub packed_flags: u32,
}

impl CompactUserCampaignState {
    pub const fn new(
        contribution: i128,
        claimed_revenue: i128,
        bookmarked: bool,
        voted: bool,
        refunded: bool,
    ) -> Self {
        let mut flags = 0u32;
        if bookmarked {
            flags |= USER_STATE_BOOKMARKED;
        }
        if voted {
            flags |= USER_STATE_VOTED;
        }
        if refunded {
            flags |= USER_STATE_REFUNDED;
        }
        Self {
            contribution,
            claimed_revenue,
            packed_flags: flags,
        }
    }

    pub const fn is_bookmarked(&self) -> bool {
        (self.packed_flags & USER_STATE_BOOKMARKED) != 0
    }

    pub const fn has_voted(&self) -> bool {
        (self.packed_flags & USER_STATE_VOTED) != 0
    }

    pub const fn is_refunded(&self) -> bool {
        (self.packed_flags & USER_STATE_REFUNDED) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn compact_campaign_summary_packs_all_flags_losslessly() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let mut campaign = Campaign {
            id: 7,
            creator: creator.clone(),
            first_creator: creator,
            pending_creator: None,
            pending_creator_expiry: 0,
            title: String::from_str(&env, "Compact Title"),
            description: String::from_str(&env, "Long description omitted in compact summary"),
            funding_goal: 50_000,
            deadline: 1_800_000_000,
            amount_raised: 60_000,
            status: CAMPAIGN_ACTIVE | CAMPAIGN_VALIDATED,
            category: Category::EducationalStartup,
            has_revenue_sharing: true,
            revenue_share_percentage: 1_500,
            max_contribution_per_user: 0,
            fee_override: None,
            deadline_extended: true,
            effective_amount_raised: 55_000,
        };

        let summary = campaign.to_compact_summary();
        assert_eq!(summary.id, 7);
        assert_eq!(summary.funding_goal, 50_000);
        assert_eq!(summary.amount_raised, 60_000);
        assert_eq!(summary.effective_amount_raised, 55_000);
        assert_eq!(summary.category, Category::EducationalStartup);
        assert_eq!(summary.revenue_share_percentage, 1_500);
        assert!(summary.is_active());
        assert!(!summary.is_cancelled());
        assert!(summary.is_verified());
        assert!(!summary.funds_withdrawn());
        assert!(summary.has_revenue_sharing());
        assert!(summary.deadline_extended());

        campaign.set_cancelled(true);
        campaign.set_funds_withdrawn(true);
        campaign.has_revenue_sharing = false;
        campaign.deadline_extended = false;

        let updated = campaign.to_compact_summary();
        assert!(updated.is_cancelled());
        assert!(updated.funds_withdrawn());
        assert!(!updated.has_revenue_sharing());
        assert!(!updated.deadline_extended());
    }

    #[test]
    fn compact_user_campaign_state_roundtrips_interaction_flags() {
        let state = CompactUserCampaignState::new(10_000, 2_500, true, false, true);
        assert_eq!(state.contribution, 10_000);
        assert_eq!(state.claimed_revenue, 2_500);
        assert!(state.is_bookmarked());
        assert!(!state.has_voted());
        assert!(state.is_refunded());
    }
}
