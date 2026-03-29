use soroban_sdk::{Address, Env};

use crate::types::{Campaign, DataKey};

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;

// ── Campaign ──────────────────────────────────────────────────────────────────

pub fn get_campaign(env: &Env, campaign_id: u32) -> Option<Campaign> {
    let key = DataKey::Campaign(campaign_id);
    let val = env.storage().persistent().get(&key);
    if val.is_some() {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_campaign(env: &Env, campaign_id: u32, campaign: &Campaign) {
    let key = DataKey::Campaign(campaign_id);
    env.storage().persistent().set(&key, campaign);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Campaign count ────────────────────────────────────────────────────────────

pub fn get_campaign_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::CampaignCount)
        .unwrap_or(0)
}

pub fn set_campaign_count(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&DataKey::CampaignCount, &count);
}

// ── Admin / token / fee ───────────────────────────────────────────────────────

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn get_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Token).unwrap()
}

pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::Token, token);
}

pub fn get_platform_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::PlatformFee)
        .unwrap_or(300)
}

pub fn set_platform_fee(env: &Env, fee: u32) {
    env.storage().instance().set(&DataKey::PlatformFee, &fee);
}

// ── Contributions ─────────────────────────────────────────────────────────────

pub fn get_contribution(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = DataKey::Contribution(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_contribution(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    let key = DataKey::Contribution(campaign_id, contributor.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Revenue ───────────────────────────────────────────────────────────────────

pub fn get_revenue_pool(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::RevenuePool(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_revenue_pool(env: &Env, campaign_id: u32, amount: i128) {
    let key = DataKey::RevenuePool(campaign_id);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address) -> i128 {
    let key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_revenue_claimed(env: &Env, campaign_id: u32, contributor: &Address, amount: i128) {
    let key = DataKey::RevenueClaimed(campaign_id, contributor.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_creator_revenue_claimed(env: &Env, campaign_id: u32) -> i128 {
    let key = DataKey::CreatorRevenueClaimed(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_creator_revenue_claimed(env: &Env, campaign_id: u32, amount: i128) {
    let key = DataKey::CreatorRevenueClaimed(campaign_id);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

// ── Voting ────────────────────────────────────────────────────────────────────

pub fn get_approve_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = DataKey::ApproveVotes(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_approve_votes(env: &Env, campaign_id: u32, count: u32) {
    let key = DataKey::ApproveVotes(campaign_id);
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_reject_votes(env: &Env, campaign_id: u32) -> u32 {
    let key = DataKey::RejectVotes(campaign_id);
    let val = env.storage().persistent().get(&key).unwrap_or(0);
    if val > 0 {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_reject_votes(env: &Env, campaign_id: u32, count: u32) {
    let key = DataKey::RejectVotes(campaign_id);
    env.storage().persistent().set(&key, &count);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_has_voted(env: &Env, campaign_id: u32, voter: &Address) -> bool {
    let key = DataKey::HasVoted(campaign_id, voter.clone());
    let val = env.storage().persistent().get(&key).unwrap_or(false);
    if val {
        env.storage().persistent().extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }
    val
}

pub fn set_has_voted(env: &Env, campaign_id: u32, voter: &Address) {
    let key = DataKey::HasVoted(campaign_id, voter.clone());
    env.storage().persistent().set(&key, &true);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn get_min_votes_quorum(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MinVotesQuorum)
        .unwrap_or(default)
}

pub fn set_min_votes_quorum(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&DataKey::MinVotesQuorum, &value);
}

pub fn get_approval_threshold_bps(env: &Env, default: u32) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ApprovalThresholdBps)
        .unwrap_or(default)
}

pub fn set_approval_threshold_bps(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ApprovalThresholdBps, &value);
}

// ── Version ───────────────────────────────────────────────────────────────────

pub fn set_version(env: &Env, version: u32) {
    env.storage().instance().set(&DataKey::Version, &version);
}

pub fn get_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::Version)
        .unwrap_or(0)
}