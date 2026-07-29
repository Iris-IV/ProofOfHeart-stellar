use soroban_sdk::{contracttype, Address, Env, TryFromVal, Val, Vec};

use crate::types::{Campaign, CampaignReserve, Category};

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const BUMP_AMOUNT: u32 = 400 * DAY_IN_LEDGERS;
pub const CATEGORY_CAMPAIGNS_BUCKET_SIZE: u32 = 500;

/// Sets a persistent storage entry and extends its TTL in a single step,
/// making it impossible to forget the TTL bump.
macro_rules! persistent_set {
    ($env:expr, $key:expr, $value:expr) => {{
        let key = $key;
        $env.storage().persistent().set(&key, $value);
        $env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }};
}

pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Marker trait implemented by every domain storage-key enum.
///
/// Ties the sub-enums together as the contract's storage-key surface and
/// guarantees each of them converts into a host `Val` usable as a storage key.
pub trait StorageKey: soroban_sdk::IntoVal<Env, soroban_sdk::Val> {}

impl StorageKey for AdminKey {}
impl StorageKey for CampaignKey {}
impl StorageKey for ContributionKey {}
impl StorageKey for VotingKey {}
impl StorageKey for RevenueKey {}
impl StorageKey for BookmarkKey {}

/// Keys for platform administration and global configuration state.
///
/// Variant names are shared with the pre-split `DataKey` enum, so the XDR
/// encoding of every key (and therefore all existing ledger entries) is
/// unchanged: `#[contracttype]` encodes only the variant name, not the enum
/// name.
#[contracttype]
pub enum AdminKey {
    /// The global admin address.
    Admin,
    /// Pending admin during two-step admin transfer.
    PendingAdmin,
    /// The contract's accepted token address.
    Token,
    /// Pending token address during two-step token update.
    PendingToken,
    /// Ledger timestamp after which the pending token update can be accepted.
    PendingTokenRelease,
    /// Platform fee in basis points (e.g. 300 = 3%).
    PlatformFee,
    /// The stored contract version number.
    Version,
    /// Whether the contract has been initialized.
    Initialized,
    /// Whether the contract is paused by admin.
    Paused,
    /// Whether the contract is auto-paused by anomaly detection.
    AutoPaused,
    /// Whether campaign creation is disabled.
    CreationDisabled,
    /// Minimum funding goal required for new campaigns.
    MinCampaignFundingGoal,
    /// Maximum funding goal allowed for new campaigns (anti-spam cap).
    MaxCampaignFundingGoal,
    /// Delay in days before the reserve can be released.
    WithdrawReleaseDelayDays,
    /// Percentage of funds held in reserve (basis points).
    WithdrawReservePercentage,
}

/// Keys for campaign records, indexes, and aggregate campaign counters.
#[contracttype]
pub enum CampaignKey {
    /// Total number of campaigns ever created.
    CampaignCount,
    /// Campaign data, keyed by campaign ID.
    Campaign(u32),
    /// Unix timestamp when the campaign was created, keyed by campaign ID.
    CampaignStartTime(u32),
    /// Held reserve for a campaign, keyed by campaign ID.
    CampaignReserve(u32),
    /// Campaign ids grouped by category as append-only creation index.
    CategoryCampaigns(u32),
    /// Campaign ids grouped by category into fixed-size buckets.
    CategoryCampaignsBucket(u32, u32),
    /// Total number of campaigns in a category.
    CategoryCampaignCount(u32),
    /// Per-category maximum duration cap in days, keyed by category discriminant.
    CategoryDurationCap(u32),
    /// Number of campaigns owned by a creator.
    CreatorCampaignCount(Address),
    /// Bucket of campaign IDs owned by a creator (≤ CREATOR_CAMPAIGNS_BUCKET_SIZE per bucket).
    CreatorCampaignsBucket(Address, u32),
    /// Number of currently active (non-cancelled, non-withdrawn) campaigns.
    ActiveCampaignCount,
    /// Number of campaigns that have been verified.
    VerifiedCampaignCount,
    /// Number of campaigns that have been cancelled.
    CancelledCampaignCount,
    /// Reverse mapping from campaign ID to its current creator, keyed by campaign ID.
    /// Enables O(1) ownership verification without scanning a creator's campaign bucket.
    CampaignCreatorIndex(u32),
}

/// Keys for contributor balances, caps, and contribution tracking.
#[contracttype]
pub enum ContributionKey {
    /// A contributor's total contribution to a campaign, keyed by `(campaign_id, contributor)`.
    Contribution(u32, Address),
    /// A contributor's lifetime contribution to a campaign, keyed by `(campaign_id, contributor)`.
    LifetimeContribution(u32, Address),
    /// A contributor's personal contribution cap for a campaign, keyed by `(campaign_id, contributor)`.
    PersonalCap(u32, Address),
    /// Contributor count for a campaign.
    ContributorCount(u32),
    /// Total amount raised across all campaigns.
    TotalRaised,
    /// Tracking contributions per block for anomaly detection (global, legacy).
    BlockContributionCount,
    /// Per-campaign contributions per block for anomaly detection, keyed by campaign ID.
    BlockCampaignContributionCount(u32),
}

/// Keys for campaign voting state and voting configuration.
#[contracttype]
pub enum VotingKey {
    /// Number of approval votes cast for a campaign, keyed by campaign ID.
    ApproveVotes(u32),
    /// Number of rejection votes cast for a campaign, keyed by campaign ID.
    RejectVotes(u32),
    /// Total token-weight of approval votes for a campaign, keyed by campaign ID.
    ApproveWeight(u32),
    /// Total token-weight of rejection votes for a campaign, keyed by campaign ID.
    RejectWeight(u32),
    /// Whether a specific voter has already voted on a campaign, keyed by `(campaign_id, voter)`.
    HasVoted(u32, Address),
    /// Minimum number of votes required to reach quorum.
    MinVotesQuorum,
    /// Required approval percentage in basis points (e.g. 6000 = 60%).
    ApprovalThresholdBps,
    /// Minimum token balance required to vote on campaigns.
    MinVotingBalance,
    /// Per-category approval threshold override in basis points, keyed by
    /// category discriminant. Falls back to `ApprovalThresholdBps` when unset.
    CategoryThresholdBps(u32),
}

/// Keys for revenue-sharing pools and claim tracking.
#[contracttype]
pub enum RevenueKey {
    /// Total revenue deposited into a campaign's pool, keyed by campaign ID.
    RevenuePool(u32),
    /// Revenue already claimed by a contributor, keyed by `(campaign_id, contributor)`.
    RevenueClaimed(u32, Address),
    /// Revenue already claimed by the campaign creator, keyed by campaign ID.
    CreatorRevenueClaimed(u32),
    /// Cumulative revenue already paid out to contributors from a campaign's
    /// pool (running sum across every `claim_revenue` call), keyed by
    /// campaign ID. Used to let the last unclaimed contributor absorb any
    /// dust left over from per-contributor integer-division truncation
    /// (#526).
    ContributorRevenueDistributed(u32),
    /// Number of distinct contributors who have claimed revenue at least
    /// once for a campaign, keyed by campaign ID (#526).
    ContributorRevenueClaimants(u32),
}

/// Keys for a wallet's saved/bookmarked campaigns (#507).
#[contracttype]
pub enum BookmarkKey {
    /// A wallet's saved campaign ids, keyed by the wallet address.
    SavedCampaigns(Address),
}

// ── Campaign ──────────────────────────────────────────────────────────────────

/// Returns the campaign for the given ID.
///
/// #528: reads the raw stored `Val` and converts it via `TryFromVal` instead
/// of the SDK's `get::<K, Campaign>()`, which panics (traps the host) if the
/// stored bytes fail to deserialize into `Campaign`. If the persistent key
/// exists but its value is corrupted, this returns `None` instead of
/// panicking, so callers can gracefully surface `Error::CampaignNotFound`
/// rather than aborting the entire transaction.
pub fn get_campaign(env: &Env, campaign_id: u32) -> Option<Campaign> {
    let key = CampaignKey::Campaign(campaign_id);
    let raw: Val = env.storage().persistent().get(&key)?;
    Campaign::try_from_val(env, &raw).ok()
}

/// Persists a campaign and extends its TTL.
pub fn set_campaign(env: &Env, campaign_id: u32, campaign: &Campaign) {
    persistent_set!(env, CampaignKey::Campaign(campaign_id), campaign);
}

pub fn get_campaign_start_time(env: &Env, campaign_id: u32) -> Option<u64> {
    let key = CampaignKey::CampaignStartTime(campaign_id);
    env.storage().persistent().get(&key)
}

pub fn set_campaign_start_time(env: &Env, campaign_id: u32, start_time: u64) {
    persistent_set!(
        env,
        CampaignKey::CampaignStartTime(campaign_id),
        &start_time
    );
}

// ── Campaign count ────────────────────────────────────────────────────────────

/// Returns the total number of campaigns created, defaulting to 0.
pub fn get_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&CampaignKey::CampaignCount)
        .unwrap_or(0)
}

/// Stores the total campaign count.
pub fn set_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&CampaignKey::CampaignCount, &count);
}

// ── Admin / token / fee ───────────────────────────────────────────────────────

/// Returns `true` if the contract is initialized.
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&AdminKey::Initialized)
}

/// Marks the contract as initialized.
pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&AdminKey::Initialized, &true);
}

/// Returns the admin address. Panics if not yet initialized.
pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&AdminKey::Admin).unwrap()
}

/// Stores the admin address.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&AdminKey::Admin, admin);
}

/// Returns the pending admin address if an admin transfer is in progress.
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&AdminKey::PendingAdmin)
}

/// Stores the pending admin address for two-step admin transfer.
pub fn set_pending_admin(env: &Env, pending_admin: &Address) {
    env.storage()
        .instance()
        .set(&AdminKey::PendingAdmin, pending_admin);
}

/// Clears any pending admin transfer.
pub fn remove_pending_admin(env: &Env) {
    env.storage().instance().remove(&AdminKey::PendingAdmin);
}

/// Returns the accepted token address. Panics if not yet initialized.
pub fn get_token(env: &Env) -> Address {
    env.storage().instance().get(&AdminKey::Token).unwrap()
}

/// Stores the accepted token address.
pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&AdminKey::Token, token);
}

/// Returns the platform fee in basis points, defaulting to 300 (3%).
pub fn get_platform_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&AdminKey::PlatformFee)
        .unwrap_or(300)
}

/// Stores the platform fee in basis points.
pub fn set_platform_fee(env: &Env, fee: u32) {
    env.storage().instance().set(&AdminKey::PlatformFee, &fee);
}

/// Returns the minimum funding goal, falling back to `default` if unset.
pub fn get_min_campaign_funding_goal(env: &Env, default: i128) -> i128 {
    env.storage()
        .instance()
        .get(&AdminKey::MinCampaignFundingGoal)
        .unwrap_or(default)
}

/// Stores the minimum funding goal.
pub fn set_min_campaign_funding_goal(env: &Env, min_goal: i128) {
    env.storage()
        .instance()
        .set(&AdminKey::MinCampaignFundingGoal, &min_goal);
}

/// Returns the maximum funding goal, falling back to `default` if not set.
pub fn get_max_campaign_funding_goal(env: &Env, default: i128) -> i128 {
    env.storage()
        .instance()
        .get(&AdminKey::MaxCampaignFundingGoal)
        .unwrap_or(default)
}

/// Stores the maximum funding goal.
pub fn set_max_campaign_funding_goal(env: &Env, max_goal: i128) {
    env.storage()
        .instance()
        .set(&AdminKey::MaxCampaignFundingGoal, &max_goal);
}

// ── Contributions ─────────────────────────────────────────────────────────────

/// Returns a contributor's total contribution to a campaign.
pub fn get_contribution(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = ContributionKey::Contribution(campaign_id, contributor.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores a contributor's contribution amount and extends its TTL.
pub fn set_contribution(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    persistent_set!(
        env,
        ContributionKey::Contribution(campaign_id, contributor.clone()),
        &amount
    );
}

/// Returns a contributor's lifetime (non-decreasing) contribution to a campaign.
pub fn get_lifetime_contribution(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = ContributionKey::LifetimeContribution(campaign_id, contributor.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores a contributor's lifetime contribution amount and extends its TTL.
pub fn set_lifetime_contribution(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    persistent_set!(
        env,
        ContributionKey::LifetimeContribution(campaign_id, contributor.clone()),
        &amount
    );
}

/// Removes a contributor's contribution record entirely.
pub fn remove_contribution(env: &Env, campaign_id: u32, contributor: &Address) {
    let key = ContributionKey::Contribution(campaign_id, contributor.clone());
    env.storage().persistent().remove(&key);
}

/// Removes a contributor's lifetime contribution record.
#[expect(dead_code)]
pub fn remove_lifetime_contribution(env: &Env, campaign_id: u32, contributor: &Address) {
    let key = ContributionKey::LifetimeContribution(campaign_id, contributor.clone());
    env.storage().persistent().remove(&key);
}

// ── Contributor count ───────────────────────────────────────────────────────────

pub fn get_contributor_count(env: &Env, campaign_id: u32) -> u32 {
    let key = ContributionKey::ContributorCount(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn set_contributor_count(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, ContributionKey::ContributorCount(campaign_id), &count);
}

pub fn increment_contributor_count(env: &Env, campaign_id: u32) {
    let count = get_contributor_count(env, campaign_id);
    set_contributor_count(env, campaign_id, count + 1);
}

pub fn decrement_contributor_count(env: &Env, campaign_id: u32) {
    let count = get_contributor_count(env, campaign_id);
    if count > 0 {
        set_contributor_count(env, campaign_id, count - 1);
    }
}

// ── Revenue ───────────────────────────────────────────────────────────────────

/// Returns the revenue pool balance for a campaign.
pub fn get_revenue_pool(env: &Env, campaign_id: u32) -> i128 {
    let key = RevenueKey::RevenuePool(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the revenue pool balance for a campaign and extends its TTL.
pub fn set_revenue_pool(env: &Env, campaign_id: u32, amount: i128) {
    persistent_set!(env, RevenueKey::RevenuePool(campaign_id), &amount);
}

/// Returns the revenue already claimed by a contributor.
pub fn get_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = RevenueKey::RevenueClaimed(campaign_id, contributor.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the revenue claimed amount for a contributor and extends its TTL.
pub fn set_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    persistent_set!(
        env,
        RevenueKey::RevenueClaimed(campaign_id, contributor.clone()),
        &amount
    );
}

/// Removes the revenue claimed record for a contributor in a campaign.
pub fn remove_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address) {
    let key = RevenueKey::RevenueClaimed(campaign_id, contributor.clone());
    env.storage().persistent().remove(&key);
}

/// Returns the creator's total claimed revenue for a campaign.
pub fn get_creator_revenue_claimed(env: &Env, campaign_id: u32) -> i128 {
    let key = RevenueKey::CreatorRevenueClaimed(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the creator's claimed revenue amount for a campaign and extends its TTL.
pub fn set_creator_revenue_claimed(env: &Env, campaign_id: u32, amount: i128) {
    persistent_set!(env, RevenueKey::CreatorRevenueClaimed(campaign_id), &amount);
}

/// Returns the cumulative amount already paid out to contributors from a
/// campaign's revenue pool (#526).
pub fn get_contributor_revenue_distributed(env: &Env, campaign_id: u32) -> i128 {
    let key = RevenueKey::ContributorRevenueDistributed(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the cumulative amount paid out to contributors from a campaign's
/// revenue pool and extends its TTL.
pub fn set_contributor_revenue_distributed(env: &Env, campaign_id: u32, amount: i128) {
    persistent_set!(
        env,
        RevenueKey::ContributorRevenueDistributed(campaign_id),
        &amount
    );
}

/// Returns the number of distinct contributors who have claimed revenue at
/// least once for a campaign (#526).
pub fn get_contributor_revenue_claimants(env: &Env, campaign_id: u32) -> u32 {
    let key = RevenueKey::ContributorRevenueClaimants(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the number of distinct contributors who have claimed revenue at
/// least once for a campaign and extends its TTL.
pub fn set_contributor_revenue_claimants(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(
        env,
        RevenueKey::ContributorRevenueClaimants(campaign_id),
        &count
    );
}

// ── Voting ────────────────────────────────────────────────────────────────────

/// Returns the number of approval votes for a campaign.
pub fn get_approve_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = VotingKey::ApproveVotes(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the approval vote count for a campaign and extends its TTL.
pub fn set_approve_votes(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, VotingKey::ApproveVotes(campaign_id), &count);
}

/// Returns the number of rejection votes for a campaign.
pub fn get_reject_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = VotingKey::RejectVotes(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the rejection vote count for a campaign and extends its TTL.
pub fn set_reject_votes(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, VotingKey::RejectVotes(campaign_id), &count);
}

// ── Vote weights (token-weighted) ─────────────────────────────────────────────

/// Returns the total approval token-weight for a campaign.
pub fn get_approve_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = VotingKey::ApproveWeight(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the total approval token-weight for a campaign and extends its TTL.
pub fn set_approve_weight(env: &Env, campaign_id: u32, weight: i128) {
    persistent_set!(env, VotingKey::ApproveWeight(campaign_id), &weight);
}

/// Returns the total rejection token-weight for a campaign.
pub fn get_reject_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = VotingKey::RejectWeight(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

/// Stores the total rejection token-weight for a campaign and extends its TTL.
pub fn set_reject_weight(env: &Env, campaign_id: u32, weight: i128) {
    persistent_set!(env, VotingKey::RejectWeight(campaign_id), &weight);
}

/// Returns whether a voter has already voted on a campaign.
pub fn get_has_voted(env: &Env, campaign_id: u32, voter: &Address) -> bool {
    let key = VotingKey::HasVoted(campaign_id, voter.clone());
    env.storage().persistent().get(&key).unwrap_or(false)
}

/// Records that a voter has voted on a campaign and extends the entry's TTL.
pub fn set_has_voted(env: &Env, campaign_id: u32, voter: &Address) {
    persistent_set!(env, VotingKey::HasVoted(campaign_id, voter.clone()), &true);
}

/// Removes the HasVoted record for a voter on a campaign.
pub fn remove_has_voted(env: &Env, campaign_id: u32, voter: &Address) {
    env.storage()
        .persistent()
        .remove(&VotingKey::HasVoted(campaign_id, voter.clone()));
}

/// Removes all aggregate voting keys for a campaign (vote counts and weights).
pub fn remove_voting_state(env: &Env, campaign_id: u32) {
    let storage = env.storage().persistent();
    storage.remove(&VotingKey::ApproveVotes(campaign_id));
    storage.remove(&VotingKey::RejectVotes(campaign_id));
    storage.remove(&VotingKey::ApproveWeight(campaign_id));
    storage.remove(&VotingKey::RejectWeight(campaign_id));
}

/// Extends TTL on all voting state keys for a campaign.
pub fn extend_voting_state_ttl(env: &Env, campaign_id: u32) {
    let storage = env.storage().persistent();
    let keys = [
        VotingKey::ApproveVotes(campaign_id),
        VotingKey::RejectVotes(campaign_id),
        VotingKey::ApproveWeight(campaign_id),
        VotingKey::RejectWeight(campaign_id),
    ];
    for key in keys {
        if storage.has(&key) {
            storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        }
    }
}

/// Returns the minimum vote quorum setting, falling back to `default` if unset.
pub fn get_min_votes_quorum(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&VotingKey::MinVotesQuorum)
        .unwrap_or(default)
}

/// Stores the minimum vote quorum.
pub fn set_min_votes_quorum(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&VotingKey::MinVotesQuorum, &value);
}

/// Returns the approval threshold in basis points, falling back to `default` if unset.
pub fn get_approval_threshold_bps(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&VotingKey::ApprovalThresholdBps)
        .unwrap_or(default)
}

/// Stores the approval threshold in basis points.
pub fn set_approval_threshold_bps(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&VotingKey::ApprovalThresholdBps, &value);
}

/// Returns the minimum voting balance in stroops, defaulting to 0 if unset.
pub fn get_min_voting_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&VotingKey::MinVotingBalance)
        .unwrap_or(0)
}

/// Stores the minimum voting balance in stroops.
pub fn set_min_voting_balance(env: &Env, balance: i128) {
    env.storage()
        .instance()
        .set(&VotingKey::MinVotingBalance, &balance);
}

/// Returns the per-category approval-threshold override in basis points, if
/// the admin has set one for this category (#536).
pub fn get_category_voting_threshold_bps(env: &Env, category: Category) -> Option<u32> {
    let key = VotingKey::CategoryThresholdBps(category as u32);
    env.storage().instance().get(&key)
}

/// Stores a per-category approval-threshold override in basis points.
pub fn set_category_voting_threshold_bps(env: &Env, category: Category, bps: u32) {
    let key = VotingKey::CategoryThresholdBps(category as u32);
    env.storage().instance().set(&key, &bps);
}

/// Removes a per-category approval-threshold override, reverting to the
/// global `ApprovalThresholdBps` default for that category.
pub fn remove_category_voting_threshold_bps(env: &Env, category: Category) {
    let key = VotingKey::CategoryThresholdBps(category as u32);
    env.storage().instance().remove(&key);
}

/// Returns all campaign ids for a category in creation order.
#[allow(dead_code)]
pub fn get_category_campaigns(env: &Env, category: Category) -> Vec<u32> {
    let key = CampaignKey::CategoryCampaigns(category as u32);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Stores all campaign ids for a category and extends entry TTL.
#[allow(dead_code)]
pub fn set_category_campaigns(env: &Env, category: Category, ids: &Vec<u32>) {
    persistent_set!(env, CampaignKey::CategoryCampaigns(category as u32), ids);
}

/// Returns the total number of campaigns in a category.
pub fn get_category_campaign_count(env: &Env, category: Category) -> u32 {
    let key = CampaignKey::CategoryCampaignCount(category as u32);
    env.storage().instance().get(&key).unwrap_or(0)
}

/// Sets the total number of campaigns in a category.
pub fn set_category_campaign_count(env: &Env, category: Category, count: u32) {
    let key = CampaignKey::CategoryCampaignCount(category as u32);
    env.storage().instance().set(&key, &count);
}

/// Returns the campaign bucket for the specified category and bucket index.
pub fn get_category_campaign_bucket(env: &Env, category: Category, bucket_idx: u32) -> Vec<u32> {
    let key = CampaignKey::CategoryCampaignsBucket(category as u32, bucket_idx);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Stores a campaign bucket and extends entry TTL.
pub fn set_category_campaign_bucket(
    env: &Env,
    category: Category,
    bucket_idx: u32,
    ids: &Vec<u32>,
) {
    persistent_set!(
        env,
        CampaignKey::CategoryCampaignsBucket(category as u32, bucket_idx),
        ids
    );
}

// ── Version ───────────────────────────────────────────────────────────────────

/// Stores the contract version number.
pub fn set_version(env: &Env, version: u32) {
    env.storage().instance().set(&AdminKey::Version, &version);
}

/// Returns the stored contract version, defaulting to 0 if unset.
pub fn get_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&AdminKey::Version)
        .unwrap_or(0)
}

// ── Total raised global ───────────────────────────────────────────────────────

/// Returns the total amount raised across all campaigns.
pub fn get_total_raised_global(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ContributionKey::TotalRaised)
        .unwrap_or(0)
}

/// Stores the total amount raised across all campaigns.
pub fn set_total_raised_global(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&ContributionKey::TotalRaised, &amount);
}

// ── Creator campaigns (bucketed) ──────────────────────────────────────────────

/// Maximum number of campaign IDs stored in a single bucket for a creator.
pub const CREATOR_CAMPAIGNS_BUCKET_SIZE: u32 = 500;

/// Returns the total number of campaigns owned by a creator.
pub fn get_creator_campaign_count(env: &Env, creator: &Address) -> u32 {
    let key = CampaignKey::CreatorCampaignCount(creator.clone());
    let val: Option<u32> = env.storage().persistent().get(&key);
    if let Some(count) = val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        count
    } else {
        0
    }
}

/// Stores the total number of campaigns owned by a creator.
pub fn set_creator_campaign_count(env: &Env, creator: &Address, count: u32) {
    persistent_set!(
        env,
        CampaignKey::CreatorCampaignCount(creator.clone()),
        &count
    );
}

/// Returns the campaign IDs in a specific bucket for a creator.
pub fn get_creator_campaign_bucket(
    env: &Env,
    creator: &Address,
    bucket_index: u32,
) -> soroban_sdk::Vec<u32> {
    let key = CampaignKey::CreatorCampaignsBucket(creator.clone(), bucket_index);
    let val: Option<soroban_sdk::Vec<u32>> = env.storage().persistent().get(&key);
    if let Some(ids) = val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        ids
    } else {
        soroban_sdk::Vec::new(env)
    }
}

/// Stores a bucket of campaign IDs for a creator.
pub fn set_creator_campaign_bucket(
    env: &Env,
    creator: &Address,
    bucket_index: u32,
    ids: &soroban_sdk::Vec<u32>,
) {
    persistent_set!(
        env,
        CampaignKey::CreatorCampaignsBucket(creator.clone(), bucket_index),
        ids
    );
}

// ── Personal cap ─────────────────────────────────────────────────────────────

/// Returns a contributor's personal cap for a campaign, extending TTL if set.
pub fn get_personal_cap(env: &Env, campaign_id: u32, contributor: &Address) -> Option<i128> {
    let key = ContributionKey::PersonalCap(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key);
    if val.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores a contributor's personal cap for a campaign and extends its TTL.
pub fn set_personal_cap(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    persistent_set!(
        env,
        ContributionKey::PersonalCap(campaign_id, contributor.clone()),
        &amount
    );
}

/// Removes a contributor's personal cap for a campaign.
pub fn remove_personal_cap(env: &Env, campaign_id: u32, contributor: &Address) {
    let key = ContributionKey::PersonalCap(campaign_id, contributor.clone());
    env.storage().persistent().remove(&key);
}

// ── Anomaly detection ─────────────────────────────────────────────────────────

/// Returns (ledger_sequence, contribution_count) for the block tracking.
#[allow(dead_code)]
pub fn get_block_contribution_count(env: &Env) -> (u32, u32) {
    env.storage()
        .instance()
        .get(&ContributionKey::BlockContributionCount)
        .unwrap_or((0, 0))
}

/// Stores (ledger_sequence, contribution_count) for the block tracking.
#[allow(dead_code)]
pub fn set_block_contribution_count(env: &Env, sequence: u32, count: u32) {
    env.storage()
        .instance()
        .set(&ContributionKey::BlockContributionCount, &(sequence, count));
}

/// Returns (ledger_sequence, contribution_count) for a specific campaign.
pub fn get_campaign_block_contribution_count(env: &Env, campaign_id: u32) -> (u32, u32) {
    env.storage()
        .instance()
        .get(&ContributionKey::BlockCampaignContributionCount(
            campaign_id,
        ))
        .unwrap_or((0, 0))
}

/// Stores (ledger_sequence, contribution_count) for a specific campaign.
pub fn set_campaign_block_contribution_count(
    env: &Env,
    campaign_id: u32,
    sequence: u32,
    count: u32,
) {
    env.storage().instance().set(
        &ContributionKey::BlockCampaignContributionCount(campaign_id),
        &(sequence, count),
    );
}

// ── Withdrawal Vesting ───────────────────────────────────────────────────────

pub fn get_withdraw_release_delay_days(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&AdminKey::WithdrawReleaseDelayDays)
        .unwrap_or(0)
}

pub fn set_withdraw_release_delay_days(env: &Env, days: u64) {
    env.storage()
        .instance()
        .set(&AdminKey::WithdrawReleaseDelayDays, &days);
}

pub fn get_withdraw_reserve_percentage(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&AdminKey::WithdrawReservePercentage)
        .unwrap_or(0)
}

pub fn set_withdraw_reserve_percentage(env: &Env, bps: u32) {
    env.storage()
        .instance()
        .set(&AdminKey::WithdrawReservePercentage, &bps);
}

pub fn get_campaign_reserve(env: &Env, campaign_id: u32) -> Option<CampaignReserve> {
    let key = CampaignKey::CampaignReserve(campaign_id);
    env.storage().persistent().get(&key)
}

pub fn set_campaign_reserve(env: &Env, campaign_id: u32, reserve: &CampaignReserve) {
    persistent_set!(env, CampaignKey::CampaignReserve(campaign_id), reserve);
}

// ── Creation disabled flag ───────────────────────────────────────────────────

pub fn get_creation_disabled(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&AdminKey::CreationDisabled)
        .unwrap_or(false)
}

pub fn set_creation_disabled(env: &Env, disabled: bool) {
    env.storage()
        .instance()
        .set(&AdminKey::CreationDisabled, &disabled);
}

// ── Per-category duration cap ─────────────────────────────────────────────────

pub fn get_category_duration_cap(env: &Env, category: Category) -> Option<u64> {
    let key = CampaignKey::CategoryDurationCap(category as u32);
    env.storage().instance().get(&key)
}

pub fn set_category_duration_cap(env: &Env, category: Category, max_days: u64) {
    let key = CampaignKey::CategoryDurationCap(category as u32);
    env.storage().instance().set(&key, &max_days);
}

/// Removes a per-category duration cap, reverting to the code default.
pub fn remove_category_duration_cap(env: &Env, category: Category) {
    let key = CampaignKey::CategoryDurationCap(category as u32);
    env.storage().instance().remove(&key);
}

// ── Pending token update ──────────────────────────────────────────────────────

/// Stores the pending token address for a two-step token update.
pub fn set_pending_token(env: &Env, token: &Address) {
    env.storage().instance().set(&AdminKey::PendingToken, token);
}

/// Returns the pending token address if a token update is in progress.
pub fn get_pending_token(env: &Env) -> Option<Address> {
    env.storage().instance().get(&AdminKey::PendingToken)
}

/// Removes the pending token state.
pub fn remove_pending_token(env: &Env) {
    env.storage().instance().remove(&AdminKey::PendingToken);
    env.storage()
        .instance()
        .remove(&AdminKey::PendingTokenRelease);
}

/// Stores the release timestamp for the pending token update.
pub fn set_pending_token_release(env: &Env, timestamp: u64) {
    env.storage()
        .instance()
        .set(&AdminKey::PendingTokenRelease, &timestamp);
}

/// Returns the release timestamp for the pending token update.
pub fn get_pending_token_release(env: &Env) -> Option<u64> {
    env.storage().instance().get(&AdminKey::PendingTokenRelease)
}

// ── O(1) platform stat counters ───────────────────────────────────────────────

pub fn get_active_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&CampaignKey::ActiveCampaignCount)
        .unwrap_or(0)
}

pub fn set_active_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&CampaignKey::ActiveCampaignCount, &count);
}

#[allow(dead_code)]
pub fn increment_active_campaign_count(env: &Env) {
    set_active_campaign_count(env, get_active_campaign_count(env) + 1);
}

pub fn decrement_active_campaign_count(env: &Env) {
    let c = get_active_campaign_count(env);
    if c > 0 {
        set_active_campaign_count(env, c - 1);
    }
}

pub fn get_verified_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&CampaignKey::VerifiedCampaignCount)
        .unwrap_or(0)
}

pub fn set_verified_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&CampaignKey::VerifiedCampaignCount, &count);
}

pub fn increment_verified_campaign_count(env: &Env) {
    set_verified_campaign_count(env, get_verified_campaign_count(env) + 1);
}

pub fn get_cancelled_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&CampaignKey::CancelledCampaignCount)
        .unwrap_or(0)
}

pub fn set_cancelled_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&CampaignKey::CancelledCampaignCount, &count);
}

pub fn increment_cancelled_campaign_count(env: &Env) {
    set_cancelled_campaign_count(env, get_cancelled_campaign_count(env) + 1);
}

// ── Saved / bookmarked campaigns ──────────────────────────────────────────────

/// Returns the list of campaign ids a wallet has bookmarked, in the order
/// they were saved. Defaults to an empty list.
pub fn get_saved_campaigns(env: &Env, user: &Address) -> Vec<u32> {
    let key = BookmarkKey::SavedCampaigns(user.clone());
    let val: Option<Vec<u32>> = env.storage().persistent().get(&key);
    if let Some(ids) = val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        ids
    } else {
        Vec::new(env)
    }
}

/// Stores a wallet's list of bookmarked campaign ids and extends its TTL.
pub fn set_saved_campaigns(env: &Env, user: &Address, ids: &Vec<u32>) {
    persistent_set!(env, BookmarkKey::SavedCampaigns(user.clone()), ids);
}

// ── Campaign creator reverse index (#478) ─────────────────────────────────────

/// Returns the creator recorded for a campaign via the O(1) reverse index, if any.
pub fn get_campaign_creator_index(env: &Env, campaign_id: u32) -> Option<Address> {
    let key = CampaignKey::CampaignCreatorIndex(campaign_id);
    env.storage().persistent().get(&key)
}

/// Stores the creator recorded for a campaign in the O(1) reverse index and extends its TTL.
pub fn set_campaign_creator_index(env: &Env, campaign_id: u32, creator: &Address) {
    let key = CampaignKey::CampaignCreatorIndex(campaign_id);
    env.storage().persistent().set(&key, creator);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns `true` if `creator` owns `campaign_id`, checked in O(1) via the reverse index
/// instead of scanning the creator's campaign bucket.
pub fn is_campaign_creator(env: &Env, campaign_id: u32, creator: &Address) -> bool {
    get_campaign_creator_index(env, campaign_id).is_some_and(|c| &c == creator)
}
