use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, String, TryFromVal, Val, Vec};

use crate::types::{Campaign, CampaignReserve, Category, EmergencyWithdrawal};

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const BUMP_AMOUNT: u32 = 400 * DAY_IN_LEDGERS;
pub const CATEGORY_CAMPAIGNS_BUCKET_SIZE: u32 = 500;

/// Sets a persistent storage entry and extends its TTL in a single step,
/// making it impossible to forget the TTL bump.
macro_rules! persistent_set {
    ($env:expr, $key:expr, $value:expr) => {{
        let key = $key;
        let storage = $env.storage().persistent();
        if storage.has(&key) {
            storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        }
        storage.set(&key, $value);
        storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }};
}

pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Extends TTL for every contributor-specific persistent key tied to a campaign.
///
/// Long-lived campaigns keep contributor metadata such as per-campaign totals,
/// lifetime totals, and personal caps in persistent storage. These entries need
/// the same TTL refresh as the campaign itself so they do not vanish while the
/// campaign remains active.
pub fn extend_contributor_ttl(env: &Env, campaign_id: u32, contributor: &Address) {
    let storage = env.storage().persistent();
    let keys = [
        ContributionKey::Contribution(campaign_id, contributor.clone()),
        ContributionKey::LifetimeContribution(campaign_id, contributor.clone()),
        ContributionKey::PersonalCap(campaign_id, contributor.clone()),
    ];
    for key in keys {
        if storage.has(&key) {
            storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        }
    }
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
    /// Admin-configured delay (seconds) before a proposed token update can be
    /// accepted, overriding the compiled-in `TOKEN_UPDATE_DELAY_SECS` default (#650).
    TokenUpdateDelaySecs,
    /// Emergency pause signers that may call `emergency_pause` (#785).
    EmergencyPauseSigners,
    /// Whether a token address may be chosen as a campaign's currency (#784).
    ///
    /// Keyed by the token address so the allowlist grows without rewriting a
    /// single vector entry, and so a lookup is O(1) at contribution time.
    AllowedToken(Address),
    /// Maximum amount accepted by a single contribution transaction. `0` disables the cap.
    MaxContributionPerTransaction,
}

/// Keys for campaign records, indexes, and aggregate campaign counters.
#[contracttype]
pub enum CampaignKey {
    /// Total number of campaigns ever created.
    CampaignCount,
    /// Campaign data, keyed by campaign ID.
    Campaign(u32),
    /// Per-campaign vesting parameters snapshotted at creation time (#466).
    CampaignVesting(u32),
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
    /// Milestones for a campaign, keyed by campaign ID (#783).
    CampaignMilestones(u32),
    /// Whether a milestone has been claimed, keyed by (campaign_id, milestone_id).
    MilestoneClaimed(u32, u32),
    /// Censure record for an off-chain comment, keyed by (campaign_id, comment_hash) (#797).
    CommentCensured(u32, BytesN<32>),
    /// Number of censured comments on a campaign, keyed by campaign ID (#797).
    CampaignCensuredCount(u32),
    /// The token a campaign accepts, keyed by campaign ID (#784).
    ///
    /// Absent for campaigns created before per-campaign currencies existed;
    /// those fall back to the platform token. See `get_campaign_token`.
    CampaignToken(u32),
    /// Position of a campaign in its creator's bucket, used for O(1) removal on transfer.
    CreatorCampaignPosition(Address, u32),
    /// Marks that a creator already owns a campaign with a given title,
    /// keyed by `(creator, sha256(title))`; the value is that campaign's id.
    /// Enforces title uniqueness per creator so donors cannot confuse two
    /// identically named campaigns from the same creator (#801).
    CreatorCampaignTitleIndex(Address, BytesN<32>),
    /// The address that receives the platform fee for a campaign, captured on
    /// the campaign's first contribution, keyed by campaign id (#800).
    ///
    /// Absent until the first contribution lands (and for campaigns created
    /// before this key existed); `withdraw_funds` falls back to the current
    /// admin when it is missing.
    CampaignFeeRecipient(u32),
    /// Campaign ids carrying a given tag, grouped into fixed-size buckets.
    /// Keyed by `(sha256(tag), bucket_idx)` (#798).
    TagCampaignsBucket(BytesN<32>, u32),
    /// Number of campaigns carrying a given tag, keyed by `sha256(tag)` (#798).
    TagCampaignCount(BytesN<32>),
    /// The tags applied to a campaign, keyed by campaign id (#798). Used to
    /// reject duplicate tags and to expose a campaign's tag list.
    CampaignTags(u32),
    /// A pending admin emergency withdrawal for a campaign, keyed by campaign
    /// id (#802). Absent unless `emergency_withdraw` has been called and not
    /// yet executed or cancelled.
    ///
    /// Kept last so existing on-chain enum discriminants remain unchanged.
    EmergencyWithdrawal(u32),
}

/// An admin's record of removing an off-chain comment (#797).
///
/// Stored rather than only emitted so the current state is queryable: an event
/// stream tells you a censure happened, this tells you whether it still
/// stands.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentCensure {
    /// Why the comment was removed. Non-empty by construction.
    pub reason: String,
    /// Ledger timestamp of the censure.
    pub censured_at: u64,
    /// The admin who censured it, so the act is attributable.
    pub admin: Address,
}

/// Keys for contributor balances, caps, and contribution tracking.
#[contracttype]
pub enum ContributionKey {
    Contribution(u32, Address),
    LifetimeContribution(u32, Address),
    PersonalCap(u32, Address),
    ContributorCount(u32),
    TotalRaised,
    BlockCampaignContributionCount(u32),
    /// The address of the largest contributor to a campaign, keyed by campaign ID.
    TopContributor(u32),
    /// Unix timestamp of the most recent contribution to a campaign, keyed by campaign ID.
    LastContributionTime(u32),
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
    let storage = env.storage().persistent();
    let raw: Val = storage.get(&key)?;
    storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
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

pub fn get_campaign_payout_marker(env: &Env, campaign_id: u32) -> Option<u32> {
    let key = CampaignKey::CampaignPayoutMarker(campaign_id);
    env.storage().persistent().get(&key)
}

pub fn set_campaign_payout_marker(env: &Env, campaign_id: u32, marker: u32) {
    persistent_set!(
        env,
        CampaignKey::CampaignPayoutMarker(campaign_id),
        &marker
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

/// Returns the global single-transaction contribution cap. `0` means unlimited.
pub fn get_max_contribution_per_transaction(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&AdminKey::MaxContributionPerTransaction)
        .unwrap_or(0)
}

/// Stores the global single-transaction contribution cap.
pub fn set_max_contribution_per_transaction(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&AdminKey::MaxContributionPerTransaction, &amount);
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
    let value = env.storage().persistent().get(&key);
    if value.is_some() {
        extend_contributor_ttl(env, campaign_id, contributor);
    }
    value.unwrap_or(0)
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
    let value = env.storage().persistent().get(&key);
    if value.is_some() {
        extend_contributor_ttl(env, campaign_id, contributor);
    }
    value.unwrap_or(0)
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

pub fn decrement_contributor_count(
    env: &Env,
    campaign_id: u32,
) -> Result<(), crate::errors::Error> {
    let count = get_contributor_count(env, campaign_id);
    if count == 0 {
        return Err(crate::errors::Error::InvariantBroken);
    }
    set_contributor_count(env, campaign_id, count - 1);
    Ok(())
}

pub fn get_top_contributor(env: &Env, campaign_id: u32) -> Option<Address> {
    let key = ContributionKey::TopContributor(campaign_id);
    let val: Option<Address> = env.storage().persistent().get(&key);
    if val.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_top_contributor(env: &Env, campaign_id: u32, contributor: &Address) {
    let key = ContributionKey::TopContributor(campaign_id);
    env.storage().persistent().set(&key, contributor);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_last_contribution_time(env: &Env, campaign_id: u32) -> u64 {
    let key = ContributionKey::LastContributionTime(campaign_id);
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn set_last_contribution_time(env: &Env, campaign_id: u32, time: u64) {
    let key = ContributionKey::LastContributionTime(campaign_id);
    env.storage().persistent().set(&key, &time);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
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
    let value: Option<u32> = env.storage().persistent().get(&key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    value.unwrap_or(0)
}

/// Stores the approval vote count for a campaign and extends its TTL.
pub fn set_approve_votes(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, VotingKey::ApproveVotes(campaign_id), &count);
}

/// Returns the number of rejection votes for a campaign.
pub fn get_reject_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = VotingKey::RejectVotes(campaign_id);
    let value: Option<u32> = env.storage().persistent().get(&key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    value.unwrap_or(0)
}

/// Stores the rejection vote count for a campaign and extends its TTL.
pub fn set_reject_votes(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, VotingKey::RejectVotes(campaign_id), &count);
}

// ── Vote weights (token-weighted) ─────────────────────────────────────────────

/// Returns the total approval token-weight for a campaign.
pub fn get_approve_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = VotingKey::ApproveWeight(campaign_id);
    let value: Option<i128> = env.storage().persistent().get(&key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    value.unwrap_or(0)
}

/// Stores the total approval token-weight for a campaign and extends its TTL.
pub fn set_approve_weight(env: &Env, campaign_id: u32, weight: i128) {
    persistent_set!(env, VotingKey::ApproveWeight(campaign_id), &weight);
}

/// Returns the total rejection token-weight for a campaign.
pub fn get_reject_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = VotingKey::RejectWeight(campaign_id);
    let value: Option<i128> = env.storage().persistent().get(&key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    value.unwrap_or(0)
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
///
/// Returns `0` both when the creator is unknown and when the creator is known
/// but has no current campaigns. Use [`creator_exists`] to distinguish these
/// cases.
pub fn get_creator_campaign_count(env: &Env, creator: &Address) -> u32 {
    get_creator_campaign_count_opt(env, creator).unwrap_or(0)
}

/// Returns the total number of campaigns owned by a creator as an `Option`.
///
/// `None` means the address has never been recorded as a creator; `Some(0)`
/// means the creator is known but currently has no campaigns.
pub fn get_creator_campaign_count_opt(env: &Env, creator: &Address) -> Option<u32> {
    let key = CampaignKey::CreatorCampaignCount(creator.clone());
    let val: Option<u32> = env.storage().persistent().get(&key);
    if let Some(count) = val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        Some(count)
    } else {
        None
    }
}

/// Returns `true` if `creator` has ever been recorded as a campaign creator.
///
/// This is the existence indicator for the creator index: it checks for the
/// `CreatorCampaignCount` key rather than defaulting a missing key to zero, so
/// consumers can tell an unknown address from a known creator with no active
/// campaigns.
pub fn creator_exists(env: &Env, creator: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&CampaignKey::CreatorCampaignCount(creator.clone()))
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

/// Returns a campaign's `(bucket_index, slot_index)` in the creator index.
pub fn get_creator_campaign_position(
    env: &Env,
    creator: &Address,
    campaign_id: u32,
) -> Option<(u32, u32)> {
    let key = CampaignKey::CreatorCampaignPosition(creator.clone(), campaign_id);
    let val = env.storage().persistent().get(&key);
    if val.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores a campaign's position in the creator bucket index.
pub fn set_creator_campaign_position(
    env: &Env,
    creator: &Address,
    campaign_id: u32,
    bucket_index: u32,
    slot_index: u32,
) {
    persistent_set!(
        env,
        CampaignKey::CreatorCampaignPosition(creator.clone(), campaign_id),
        &(bucket_index, slot_index)
    );
}

/// Removes a campaign's position record after it leaves a creator index.
pub fn remove_creator_campaign_position(env: &Env, creator: &Address, campaign_id: u32) {
    env.storage()
        .persistent()
        .remove(&CampaignKey::CreatorCampaignPosition(
            creator.clone(),
            campaign_id,
        ));
}

// ── Personal cap ─────────────────────────────────────────────────────────────

/// Returns a contributor's personal cap for a campaign, extending TTL if set.
pub fn get_personal_cap(env: &Env, campaign_id: u32, contributor: &Address) -> Option<i128> {
    let key = ContributionKey::PersonalCap(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key);
    if val.is_some() {
        extend_contributor_ttl(env, campaign_id, contributor);
    }
    val
}

/// Returns whether a contributor has a personal cap set for a campaign,
/// without bumping the entry's TTL. Callers that are about to remove the
/// cap (rather than read its value) should use this instead of
/// `get_personal_cap`, so a removal doesn't first extend the TTL of the
/// entry it's deleting.
pub fn has_personal_cap(env: &Env, campaign_id: u32, contributor: &Address) -> bool {
    let key = ContributionKey::PersonalCap(campaign_id, contributor.clone());
    env.storage().persistent().has(&key)
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

// ── Emergency withdrawal (#802) ──────────────────────────────────────────────

/// Returns the pending emergency withdrawal for a campaign, or `None`.
pub fn get_emergency_withdrawal(env: &Env, campaign_id: u32) -> Option<EmergencyWithdrawal> {
    env.storage()
        .persistent()
        .get(&CampaignKey::EmergencyWithdrawal(campaign_id))
}

/// Records a pending emergency withdrawal for a campaign.
pub fn set_emergency_withdrawal(env: &Env, campaign_id: u32, pending: &EmergencyWithdrawal) {
    persistent_set!(env, CampaignKey::EmergencyWithdrawal(campaign_id), pending);
}

/// Clears a pending emergency withdrawal (after execution or cancellation).
pub fn remove_emergency_withdrawal(env: &Env, campaign_id: u32) {
    env.storage()
        .persistent()
        .remove(&CampaignKey::EmergencyWithdrawal(campaign_id));
}

// ── Per-campaign vesting snapshot (#466) ─────────────────────────────────────

pub fn get_campaign_vesting(env: &Env, campaign_id: u32) -> Option<(u64, u32)> {
    let key = CampaignKey::CampaignVesting(campaign_id);
    env.storage().persistent().get(&key)
}

pub fn set_campaign_vesting(env: &Env, campaign_id: u32, delay_days: u64, reserve_bps: u32) {
    persistent_set!(
        env,
        CampaignKey::CampaignVesting(campaign_id),
        &(delay_days, reserve_bps)
    );
}

#[expect(dead_code)]
pub fn remove_campaign_vesting(env: &Env, campaign_id: u32) {
    env.storage()
        .persistent()
        .remove(&CampaignKey::CampaignVesting(campaign_id));
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

/// Returns the configured token-update timelock delay in seconds, falling
/// back to `default` (the compiled-in `TOKEN_UPDATE_DELAY_SECS`) if the admin
/// has never overridden it (#650).
pub fn get_token_update_delay_secs(env: &Env, default: u64) -> u64 {
    env.storage()
        .instance()
        .get(&AdminKey::TokenUpdateDelaySecs)
        .unwrap_or(default)
}

/// Stores the admin-configured token-update timelock delay in seconds.
pub fn set_token_update_delay_secs(env: &Env, delay_secs: u64) {
    env.storage()
        .instance()
        .set(&AdminKey::TokenUpdateDelaySecs, &delay_secs);
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

/// Decrement the verified-campaign counter, saturating at zero (#796).
///
/// Saturating rather than wrapping: an underflow here would report billions of
/// verified campaigns, and the counter is a statistic — it must never be the
/// thing that bricks a description edit.
pub fn decrement_verified_campaign_count(env: &Env) {
    let count = get_verified_campaign_count(env);
    set_verified_campaign_count(env, count.saturating_sub(1));
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
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Stores a wallet's list of bookmarked campaign ids and extends its TTL.
pub fn set_saved_campaigns(env: &Env, user: &Address, ids: &Vec<u32>) {
    persistent_set!(env, BookmarkKey::SavedCampaigns(user.clone()), ids);
}

// ── Milestones (#783) ───────────────────────────────────────────────────────

pub fn get_campaign_milestones(
    env: &Env,
    campaign_id: u32,
) -> soroban_sdk::Vec<crate::types::Milestone> {
    let key = CampaignKey::CampaignMilestones(campaign_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(soroban_sdk::Vec::new(env))
}

pub fn set_campaign_milestones(
    env: &Env,
    campaign_id: u32,
    milestones: &soroban_sdk::Vec<crate::types::Milestone>,
) {
    persistent_set!(
        env,
        CampaignKey::CampaignMilestones(campaign_id),
        milestones
    );
}

pub fn is_milestone_claimed(env: &Env, campaign_id: u32, milestone_id: u32) -> bool {
    let key = CampaignKey::MilestoneClaimed(campaign_id, milestone_id);
    env.storage().persistent().get(&key).unwrap_or(false)
}

pub fn set_milestone_claimed(env: &Env, campaign_id: u32, milestone_id: u32) {
    persistent_set!(
        env,
        CampaignKey::MilestoneClaimed(campaign_id, milestone_id),
        &true
    );
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

// ── Emergency pause signers (#785) ──────────────────────────────────────────

/// Returns the set of addresses authorized to call `emergency_pause`.
pub fn get_emergency_pause_signers(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&AdminKey::EmergencyPauseSigners)
        .unwrap_or(Vec::new(env))
}

/// Stores the set of emergency pause signers.
pub fn set_emergency_pause_signers(env: &Env, signers: &Vec<Address>) {
    env.storage()
        .instance()
        .set(&AdminKey::EmergencyPauseSigners, signers);
}

// ── Comment censure (#797) ───────────────────────────────────────────────────

pub fn set_comment_censure(
    env: &Env,
    campaign_id: u32,
    comment_hash: &BytesN<32>,
    record: &CommentCensure,
) {
    persistent_set!(
        env,
        CampaignKey::CommentCensured(campaign_id, comment_hash.clone()),
        record
    );
}

pub fn get_comment_censure(
    env: &Env,
    campaign_id: u32,
    comment_hash: &BytesN<32>,
) -> Option<CommentCensure> {
    env.storage()
        .persistent()
        .get(&CampaignKey::CommentCensured(
            campaign_id,
            comment_hash.clone(),
        ))
}

pub fn is_comment_censured(env: &Env, campaign_id: u32, comment_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&CampaignKey::CommentCensured(
            campaign_id,
            comment_hash.clone(),
        ))
}

pub fn remove_comment_censure(env: &Env, campaign_id: u32, comment_hash: &BytesN<32>) {
    env.storage()
        .persistent()
        .remove(&CampaignKey::CommentCensured(
            campaign_id,
            comment_hash.clone(),
        ));
}

pub fn get_campaign_censured_count(env: &Env, campaign_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&CampaignKey::CampaignCensuredCount(campaign_id))
        .unwrap_or(0)
}

pub fn set_campaign_censured_count(env: &Env, campaign_id: u32, count: u32) {
    persistent_set!(env, CampaignKey::CampaignCensuredCount(campaign_id), &count);
}

// ── Per-campaign token (#784) ────────────────────────────────────────────────

/// Pin the currency a campaign accepts.
///
/// Written once at creation and never updated: a campaign that changed
/// currency mid-flight would hold contributions in one asset and owe refunds
/// in another.
pub fn set_campaign_token(env: &Env, campaign_id: u32, token: &Address) {
    persistent_set!(env, CampaignKey::CampaignToken(campaign_id), token);
}

/// The currency a campaign accepts.
///
/// Falls back to the platform token for campaigns created before this key
/// existed. Those campaigns were denominated in whatever `AdminKey::Token`
/// held, and that is still the only correct answer for them — including after
/// an `accept_token_update`, which is the behaviour they were created under.
pub fn get_campaign_token(env: &Env, campaign_id: u32) -> Address {
    env.storage()
        .persistent()
        .get(&CampaignKey::CampaignToken(campaign_id))
        .unwrap_or_else(|| get_token(env))
}

/// Whether a campaign has its own pinned currency, as opposed to inheriting
/// the platform token.
///
/// Only the tests read this: the contract itself never needs to distinguish
/// "pinned to the platform token" from "inheriting it", since
/// `get_campaign_token` answers both the same way. The tests do, because the
/// absence of the key is what keeps the default path free of storage rent.
#[cfg_attr(not(test), expect(dead_code))]
pub fn has_campaign_token(env: &Env, campaign_id: u32) -> bool {
    env.storage()
        .persistent()
        .has(&CampaignKey::CampaignToken(campaign_id))
}

/// Add or remove a token from the set creators may denominate campaigns in.
pub fn set_token_allowed(env: &Env, token: &Address, allowed: bool) {
    if allowed {
        env.storage()
            .instance()
            .set(&AdminKey::AllowedToken(token.clone()), &true);
    } else {
        env.storage()
            .instance()
            .remove(&AdminKey::AllowedToken(token.clone()));
    }
}

/// Whether a token may be chosen as a campaign's currency.
///
/// The platform token is always allowed without an explicit entry: it is the
/// currency every campaign used before this feature, and requiring the admin
/// to allowlist it would make an upgrade silently break `create_campaign`.
pub fn is_token_allowed(env: &Env, token: &Address) -> bool {
    *token == get_token(env) || is_token_explicitly_allowed(env, token)
}

/// Whether a token has an explicit allowlist entry.
///
/// Split out from `is_token_allowed` so a caller that already knows the
/// platform token does not pay to read it again. Campaign creation is on this
/// path for every campaign ever made, and the redundant instance read was
/// measurable — enough to exhaust the host budget in tests that create a
/// hundred campaigns in one invocation.
pub fn is_token_explicitly_allowed(env: &Env, token: &Address) -> bool {
    env.storage()
        .instance()
        .has(&AdminKey::AllowedToken(token.clone()))
}

// ── Text hashing (#801, #798) ────────────────────────────────────────────────

/// Longest text this module will hash: a campaign title's maximum length.
/// Tags are shorter, so the same buffer covers both.
const MAX_HASHED_TEXT_LEN: usize = crate::CAMPAIGN_TITLE_MAX_LEN as usize;

/// SHA-256 of a short string's UTF-8 bytes, used as a compact fixed-size key
/// for the per-creator title index (#801) and the tag index (#798).
///
/// Callers must validate `text.len() <= CAMPAIGN_TITLE_MAX_LEN` first — this
/// copies into a fixed stack buffer and a longer string would trap.
pub fn hash_text(env: &Env, text: &String) -> BytesN<32> {
    let len = (text.len() as usize).min(MAX_HASHED_TEXT_LEN);
    let mut buf = [0u8; MAX_HASHED_TEXT_LEN];
    text.copy_into_slice(&mut buf[..len]);
    env.crypto().sha256(&Bytes::from_slice(env, &buf[..len]))
}

// ── Per-creator title index (#801) ──────────────────────────────────────────

/// Whether `creator` already owns a campaign whose title hashes to `title_hash`.
pub fn creator_has_title(env: &Env, creator: &Address, title_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&CampaignKey::CreatorCampaignTitleIndex(
            creator.clone(),
            title_hash.clone(),
        ))
}

/// Records that `creator`'s campaign `campaign_id` occupies `title_hash`.
pub fn set_creator_title_index(
    env: &Env,
    creator: &Address,
    title_hash: &BytesN<32>,
    campaign_id: u32,
) {
    persistent_set!(
        env,
        CampaignKey::CreatorCampaignTitleIndex(creator.clone(), title_hash.clone()),
        &campaign_id
    );
}

/// Frees `title_hash` for `creator` (used when a campaign's title changes).
pub fn remove_creator_title_index(env: &Env, creator: &Address, title_hash: &BytesN<32>) {
    env.storage()
        .persistent()
        .remove(&CampaignKey::CreatorCampaignTitleIndex(
            creator.clone(),
            title_hash.clone(),
        ));
}

// ── Per-campaign fee recipient snapshot (#800) ──────────────────────────────

/// The fee recipient captured on this campaign's first contribution, if any.
pub fn get_campaign_fee_recipient(env: &Env, campaign_id: u32) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&CampaignKey::CampaignFeeRecipient(campaign_id))
}

/// Pins the fee recipient for a campaign. Written once, on the first
/// contribution; never overwritten.
pub fn set_campaign_fee_recipient(env: &Env, campaign_id: u32, recipient: &Address) {
    persistent_set!(
        env,
        CampaignKey::CampaignFeeRecipient(campaign_id),
        recipient
    );
}

// ── Tag index (#798) ────────────────────────────────────────────────────────

/// Campaign ids per tag bucket, mirroring the category-index layout.
pub const TAG_CAMPAIGNS_BUCKET_SIZE: u32 = 500;

/// The tags currently applied to a campaign.
pub fn get_campaign_tags(env: &Env, campaign_id: u32) -> Vec<String> {
    env.storage()
        .persistent()
        .get(&CampaignKey::CampaignTags(campaign_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// Persists a campaign's tag list.
pub fn set_campaign_tags(env: &Env, campaign_id: u32, tags: &Vec<String>) {
    persistent_set!(env, CampaignKey::CampaignTags(campaign_id), tags);
}

/// Number of campaigns indexed under `tag_hash`.
pub fn get_tag_campaign_count(env: &Env, tag_hash: &BytesN<32>) -> u32 {
    env.storage()
        .persistent()
        .get(&CampaignKey::TagCampaignCount(tag_hash.clone()))
        .unwrap_or(0)
}

/// Sets the number of campaigns indexed under `tag_hash`.
pub fn set_tag_campaign_count(env: &Env, tag_hash: &BytesN<32>, count: u32) {
    persistent_set!(env, CampaignKey::TagCampaignCount(tag_hash.clone()), &count);
}

/// Reads a single tag-index bucket.
pub fn get_tag_campaigns_bucket(env: &Env, tag_hash: &BytesN<32>, bucket_idx: u32) -> Vec<u32> {
    env.storage()
        .persistent()
        .get(&CampaignKey::TagCampaignsBucket(
            tag_hash.clone(),
            bucket_idx,
        ))
        .unwrap_or_else(|| Vec::new(env))
}

/// Writes a single tag-index bucket.
pub fn set_tag_campaigns_bucket(
    env: &Env,
    tag_hash: &BytesN<32>,
    bucket_idx: u32,
    bucket: &Vec<u32>,
) {
    persistent_set!(
        env,
        CampaignKey::TagCampaignsBucket(tag_hash.clone(), bucket_idx),
        bucket
    );
}

/// Appends `campaign_id` to the tag index for `tag_hash`.
pub fn append_campaign_to_tag(env: &Env, tag_hash: &BytesN<32>, campaign_id: u32) {
    let count = get_tag_campaign_count(env, tag_hash);
    let bucket_idx = count / TAG_CAMPAIGNS_BUCKET_SIZE;
    let mut bucket = get_tag_campaigns_bucket(env, tag_hash, bucket_idx);
    bucket.push_back(campaign_id);
    set_tag_campaigns_bucket(env, tag_hash, bucket_idx, &bucket);
    set_tag_campaign_count(env, tag_hash, count + 1);
}
