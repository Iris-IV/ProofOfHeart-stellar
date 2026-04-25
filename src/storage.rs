use soroban_sdk::{contracttype, Address, Env};

use crate::types::Campaign;

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;

pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Keys representing the unique storage state for the contract.
#[contracttype]
pub enum DataKey {
    /// The global admin address.
    Admin,
    /// The contract's accepted token address.
    Token,
    /// Platform fee in basis points (e.g. 300 = 3%).
    PlatformFee,
    /// Minimum funding goal required for new campaigns.
    MinCampaignFundingGoal,
    /// Total number of campaigns ever created.
    CampaignCount,
    /// Campaign data, keyed by campaign ID.
    Campaign(u32),
    /// A contributor's total contribution to a campaign, keyed by `(campaign_id, contributor)`.
    Contribution(u32, Address),
    /// Total revenue deposited into a campaign's pool, keyed by campaign ID.
    RevenuePool(u32),
    /// Revenue already claimed by a contributor, keyed by `(campaign_id, contributor)`.
    RevenueClaimed(u32, Address),
    /// Revenue already claimed by the campaign creator, keyed by campaign ID.
    CreatorRevenueClaimed(u32),
    /// The stored contract version number.
    Version,
    /// Whether the contract is paused.
    Paused,
    /// Number of approval votes cast for a campaign, keyed by campaign ID.
    ApproveVotes(u32),
    /// Number of rejection votes cast for a campaign, keyed by campaign ID.
    RejectVotes(u32),
    /// Whether a specific voter has already voted on a campaign, keyed by `(campaign_id, voter)`.
    HasVoted(u32, Address),
    /// Minimum number of votes required to reach quorum.
    MinVotesQuorum,
    /// Required approval percentage in basis points (e.g. 6000 = 60%).
    ApprovalThresholdBps,
    /// Total token-weight of approval votes for a campaign, keyed by campaign ID.
    ApproveWeight(u32),
    /// Total token-weight of rejection votes for a campaign, keyed by campaign ID.
    RejectWeight(u32),
    /// Whether the contract has been initialized.
    Initialized,
    /// Minimum token balance required to vote on campaigns.
    MinVotingBalance,
    /// Total amount raised across all campaigns.
    TotalRaised,
    /// List of campaign IDs owned by a creator.
    CreatorCampaigns(Address),
    /// A contributor's personal contribution cap for a campaign, keyed by `(campaign_id, contributor)`.
    PersonalCap(u32, Address),
    /// Tracking contributions per block for anomaly detection.
    BlockContributionCount,
}

// ── Campaign ──────────────────────────────────────────────────────────────────

/// Returns the campaign for the given ID, extending its TTL if found.
pub fn get_campaign(env: &Env, campaign_id: u32) -> Option<Campaign> {
    let key = DataKey::Campaign(campaign_id);
    let val = env.storage().persistent().get(&key);
    if val.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Persists a campaign and extends its TTL.
pub fn set_campaign(env: &Env, campaign_id: u32, campaign: &Campaign) {
    let key = DataKey::Campaign(campaign_id);
    env.storage().persistent().set(&key, campaign);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Campaign count ────────────────────────────────────────────────────────────

/// Returns the total number of campaigns created, defaulting to 0.
pub fn get_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::CampaignCount)
        .unwrap_or(0)
}

/// Stores the total campaign count.
pub fn set_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&DataKey::CampaignCount, &count);
}

// ── Admin / token / fee ───────────────────────────────────────────────────────

/// Returns `true` if the contract is initialized.
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Initialized)
}

/// Marks the contract as initialized.
pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&DataKey::Initialized, &true);
}

/// Returns the admin address. Panics if not yet initialized.
pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

/// Stores the admin address.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

/// Returns the accepted token address. Panics if not yet initialized.
pub fn get_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Token).unwrap()
}

/// Stores the accepted token address.
pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::Token, token);
}

/// Returns the platform fee in basis points, defaulting to 300 (3%).
pub fn get_platform_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::PlatformFee)
        .unwrap_or(300)
}

/// Stores the platform fee in basis points.
pub fn set_platform_fee(env: &Env, fee: u32) {
    env.storage().instance().set(&DataKey::PlatformFee, &fee);
}

/// Returns the minimum funding goal, falling back to `default` if unset.
pub fn get_min_campaign_funding_goal(env: &Env, default: i128) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinCampaignFundingGoal)
        .unwrap_or(default)
}

/// Stores the minimum funding goal.
pub fn set_min_campaign_funding_goal(env: &Env, min_goal: i128) {
    env.storage()
        .instance()
        .set(&DataKey::MinCampaignFundingGoal, &min_goal);
}

// ── Contributions ─────────────────────────────────────────────────────────────

/// Returns a contributor's total contribution to a campaign, extending TTL if non-zero.
pub fn get_contribution(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = DataKey::Contribution(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores a contributor's contribution amount and extends its TTL.
pub fn set_contribution(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    let key = DataKey::Contribution(campaign_id, contributor.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Revenue ───────────────────────────────────────────────────────────────────

/// Returns the revenue pool balance for a campaign, extending TTL if non-zero.
pub fn get_revenue_pool(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::RevenuePool(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the revenue pool balance for a campaign and extends its TTL.
pub fn set_revenue_pool(env: &Env, campaign_id: u32, amount: i128) {
    let key = DataKey::RevenuePool(campaign_id);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns the revenue already claimed by a contributor, extending TTL if non-zero.
pub fn get_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the revenue claimed amount for a contributor and extends its TTL.
pub fn set_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    let key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns the creator's total claimed revenue for a campaign, extending TTL if non-zero.
pub fn get_creator_revenue_claimed(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::CreatorRevenueClaimed(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the creator's claimed revenue amount for a campaign and extends its TTL.
pub fn set_creator_revenue_claimed(env: &Env, campaign_id: u32, amount: i128) {
    let key = DataKey::CreatorRevenueClaimed(campaign_id);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Voting ────────────────────────────────────────────────────────────────────

/// Returns the number of approval votes for a campaign, extending TTL if non-zero.
pub fn get_approve_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = DataKey::ApproveVotes(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the approval vote count for a campaign and extends its TTL.
pub fn set_approve_votes(env: &Env, campaign_id: u32, count: u32) {
    let key = DataKey::ApproveVotes(campaign_id);
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns the number of rejection votes for a campaign, extending TTL if non-zero.
pub fn get_reject_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = DataKey::RejectVotes(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the rejection vote count for a campaign and extends its TTL.
pub fn set_reject_votes(env: &Env, campaign_id: u32, count: u32) {
    let key = DataKey::RejectVotes(campaign_id);
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Vote weights (token-weighted) ─────────────────────────────────────────────

/// Returns the total approval token-weight for a campaign, extending TTL if non-zero.
pub fn get_approve_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::ApproveWeight(campaign_id);
    let val: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the total approval token-weight for a campaign and extends its TTL.
pub fn set_approve_weight(env: &Env, campaign_id: u32, weight: i128) {
    let key = DataKey::ApproveWeight(campaign_id);
    env.storage().persistent().set(&key, &weight);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns the total rejection token-weight for a campaign, extending TTL if non-zero.
pub fn get_reject_weight(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::RejectWeight(campaign_id);
    let val: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Stores the total rejection token-weight for a campaign and extends its TTL.
pub fn set_reject_weight(env: &Env, campaign_id: u32, weight: i128) {
    let key = DataKey::RejectWeight(campaign_id);
    env.storage().persistent().set(&key, &weight);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns whether a voter has already voted on a campaign, extending TTL if true.
pub fn get_has_voted(env: &Env, campaign_id: u32, voter: &Address) -> bool {
    let key = DataKey::HasVoted(campaign_id, voter.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(false);
    if val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

/// Records that a voter has voted on a campaign and extends the entry's TTL.
pub fn set_has_voted(env: &Env, campaign_id: u32, voter: &Address) {
    let key = DataKey::HasVoted(campaign_id, voter.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

/// Returns the minimum vote quorum setting, falling back to `default` if unset.
pub fn get_min_votes_quorum(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MinVotesQuorum)
        .unwrap_or(default)
}

/// Stores the minimum vote quorum.
pub fn set_min_votes_quorum(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&DataKey::MinVotesQuorum, &value);
}

/// Returns the approval threshold in basis points, falling back to `default` if unset.
pub fn get_approval_threshold_bps(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ApprovalThresholdBps)
        .unwrap_or(default)
}

/// Stores the approval threshold in basis points.
pub fn set_approval_threshold_bps(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ApprovalThresholdBps, &value);
}

/// Returns the minimum voting balance in stroops, defaulting to 0 if unset.
pub fn get_min_voting_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinVotingBalance)
        .unwrap_or(0)
}

/// Stores the minimum voting balance in stroops.
pub fn set_min_voting_balance(env: &Env, balance: i128) {
    env.storage()
        .instance()
        .set(&DataKey::MinVotingBalance, &balance);
}

// ── Version ───────────────────────────────────────────────────────────────────

/// Stores the contract version number.
pub fn set_version(env: &Env, version: u32) {
    env.storage().instance().set(&DataKey::Version, &version);
}

/// Returns the stored contract version, defaulting to 0 if unset.
pub fn get_version(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::Version).unwrap_or(0)
}

// ── Total raised global ───────────────────────────────────────────────────────

/// Returns the total amount raised across all campaigns.
pub fn get_total_raised_global(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalRaised)
        .unwrap_or(0)
}

/// Stores the total amount raised across all campaigns.
pub fn set_total_raised_global(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalRaised, &amount);
}

// ── Creator campaigns ─────────────────────────────────────────────────────────

/// Returns the list of campaign IDs owned by a creator.
pub fn get_creator_campaign_ids(env: &Env, creator: &Address) -> soroban_sdk::Vec<u32> {
    let key = DataKey::CreatorCampaigns(creator.clone());
    let val = env
        .storage()
        .persistent()
        .get::<DataKey, soroban_sdk::Vec<u32>>(&key);

    if let Some(ids) = val {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        ids
    } else {
        soroban_sdk::Vec::new(env)
    }
}

/// Stores the list of campaign IDs owned by a creator.
pub fn set_creator_campaign_ids(env: &Env, creator: &Address, ids: &soroban_sdk::Vec<u32>) {
    let key = DataKey::CreatorCampaigns(creator.clone());
    env.storage().persistent().set(&key, ids);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Personal cap ─────────────────────────────────────────────────────────────

/// Returns a contributor's personal cap for a campaign, extending TTL if set.
pub fn get_personal_cap(env: &Env, campaign_id: u32, contributor: &Address) -> Option<i128> {
    let key = DataKey::PersonalCap(campaign_id, contributor.clone());
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
    let key = DataKey::PersonalCap(campaign_id, contributor.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Anomaly detection ─────────────────────────────────────────────────────────

/// Returns (ledger_sequence, contribution_count) for the block tracking.
pub fn get_block_contribution_count(env: &Env) -> (u32, u32) {
    env.storage()
        .temporary()
        .get(&DataKey::BlockContributionCount)
        .unwrap_or((0, 0))
}

/// Stores (ledger_sequence, contribution_count) for the block tracking.
pub fn set_block_contribution_count(env: &Env, sequence: u32, count: u32) {
    env.storage()
        .temporary()
        .set(&DataKey::BlockContributionCount, &(sequence, count));
}
