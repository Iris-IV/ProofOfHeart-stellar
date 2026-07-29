use soroban_sdk::{Address, Env};

use crate::storage::{
    get_active_campaign_count, get_campaign, get_campaign_count, get_cancelled_campaign_count,
    get_category_campaign_bucket, get_category_campaign_count, get_contributor_count,
    get_creator_campaign_bucket, get_creator_campaign_count, get_total_raised_global,
    get_verified_campaign_count, CATEGORY_CAMPAIGNS_BUCKET_SIZE, CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
use crate::types::{Campaign, Category, CreatorStats, PlatformStats};

pub(crate) fn list_campaigns(env: &Env, start: u32, limit: u32) -> soroban_sdk::Vec<Campaign> {
    let total_count = get_campaign_count(env);
    let mut campaigns = soroban_sdk::Vec::new(env);

    if start >= total_count || limit == 0 {
        return campaigns;
    }

    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let end = start.saturating_add(capped_limit).min(total_count);

    for id in (start + 1)..=end {
        if let Some(campaign) = get_campaign(env, id) {
            campaigns.push_back(campaign);
        }
    }

    campaigns
}

/// Maximum number of campaign IDs scanned per `list_active_campaigns` call (#475).
/// Widened from the original 200 so pagination can reach active campaigns that
/// sit behind a long run of inactive ones; a maintained active-only index was
/// considered (see issue #475) but rejected because it adds a per-`create_campaign`
/// write whose cost compounds with the existing category/creator buckets and
/// exceeds the per-invocation CPU budget once a creator has created several dozen
/// campaigns (see `test_creator_buckets_100_campaigns`).
const MAX_SCAN_WINDOW: u32 = 1000;

/// Lists active campaigns by scanning campaign IDs starting after `start`, up to
/// `MAX_SCAN_WINDOW` ids per call. If the scan window is exhausted before
/// `limit` active campaigns are collected, a `scan_window_exhausted` event is
/// published so callers/indexers know to re-query with the returned cursor
/// rather than assuming pagination is complete.
pub(crate) fn list_active_campaigns(
    env: &Env,
    start: u32,
    limit: u32,
) -> (soroban_sdk::Vec<Campaign>, u32) {
    let total_count = get_campaign_count(env);
    let mut campaigns = soroban_sdk::Vec::new(env);

    if start >= total_count || limit == 0 {
        return (campaigns, 0);
    }

    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let mut collected = 0u32;
    let mut current_id = start + 1;
    let mut next_cursor = 0u32;

    while current_id <= total_count {
        if current_id > start + MAX_SCAN_WINDOW {
            env.events()
                .publish(("scan_window_exhausted",), (start, current_id, collected));
            next_cursor = current_id;
            break;
        }

        if let Some(campaign) = get_campaign(env, current_id) {
            if campaign.is_active && !campaign.is_cancelled {
                campaigns.push_back(campaign);
                collected += 1;
                if collected >= capped_limit {
                    next_cursor = current_id + 1;
                    break;
                }
            }
        }
        current_id += 1;
    }

    (campaigns, next_cursor)
}

pub(crate) fn get_campaigns_by_category(
    env: &Env,
    category: Category,
    offset: u32,
    limit: u32,
) -> soroban_sdk::Vec<Campaign> {
    let mut campaigns = soroban_sdk::Vec::new(env);
    if limit == 0 {
        return campaigns;
    }

    let total = get_category_campaign_count(env, category);
    if offset >= total {
        return campaigns;
    }

    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let end = offset.saturating_add(capped_limit).min(total);

    let mut position = offset;
    while position < end {
        let bucket_idx = position / CATEGORY_CAMPAIGNS_BUCKET_SIZE;
        let bucket = get_category_campaign_bucket(env, category, bucket_idx);
        let bucket_start = bucket_idx * CATEGORY_CAMPAIGNS_BUCKET_SIZE;
        let mut idx_in_bucket = position - bucket_start;

        let bucket_len = bucket.len();
        while idx_in_bucket < bucket_len && position < end {
            let campaign_id = bucket.get(idx_in_bucket).unwrap();
            if let Some(campaign) = get_campaign(env, campaign_id) {
                campaigns.push_back(campaign);
            }
            idx_in_bucket += 1;
            position += 1;
        }

        if idx_in_bucket >= bucket_len {
            position = if bucket_len == 0 {
                bucket_start + CATEGORY_CAMPAIGNS_BUCKET_SIZE
            } else {
                bucket_start + bucket_len
            };
        }
    }

    campaigns
}

/// #534: jumps straight to the bucket containing `start` instead of reading
/// every preceding bucket just to advance a counter, so paginating deep into
/// a creator with many campaigns no longer costs one ledger read per skipped
/// bucket (mirrors `get_campaigns_by_category`'s direct-jump approach).
pub(crate) fn get_creator_campaigns(
    env: &Env,
    creator: Address,
    start: u32,
    limit: u32,
) -> soroban_sdk::Vec<Campaign> {
    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let total = get_creator_campaign_count(env, &creator);
    let mut campaigns = soroban_sdk::Vec::new(env);

    if start >= total || capped_limit == 0 {
        return campaigns;
    }

    let end = (start + capped_limit).min(total);
    let mut position = start;

    while position < end {
        let bucket_idx = position / CREATOR_CAMPAIGNS_BUCKET_SIZE;
        let bucket = get_creator_campaign_bucket(env, &creator, bucket_idx);
        let bucket_start = bucket_idx * CREATOR_CAMPAIGNS_BUCKET_SIZE;
        let mut idx_in_bucket = position - bucket_start;

        let bucket_len = bucket.len();
        while idx_in_bucket < bucket_len && position < end {
            if let Some(campaign_id) = bucket.get(idx_in_bucket) {
                if let Some(campaign) = get_campaign(env, campaign_id) {
                    campaigns.push_back(campaign);
                }
            }
            idx_in_bucket += 1;
            position += 1;
        }

        if idx_in_bucket >= bucket_len {
            position = if bucket_len == 0 {
                bucket_start + CREATOR_CAMPAIGNS_BUCKET_SIZE
            } else {
                bucket_start + bucket_len
            };
        }
    }

    campaigns
}

/// Aggregates total raised, active campaign count, and total contributors
/// across every campaign owned by `creator` (#519). Walks the creator's
/// campaign buckets directly (same storage layout `get_creator_campaigns`
/// paginates over) rather than the paginated query, since a creator's own
/// campaign count is bounded by normal usage and the caller wants a
/// complete aggregate, not a page.
pub(crate) fn get_creator_stats(env: &Env, creator: Address) -> CreatorStats {
    let total = get_creator_campaign_count(env, &creator);

    let mut active_campaigns = 0u32;
    let mut total_raised: i128 = 0;
    let mut total_contributors: u32 = 0;

    let num_buckets = total.div_ceil(CREATOR_CAMPAIGNS_BUCKET_SIZE);
    for bucket_idx in 0..num_buckets {
        let bucket = get_creator_campaign_bucket(env, &creator, bucket_idx);
        for i in 0..bucket.len() {
            if let Some(campaign_id) = bucket.get(i) {
                if let Some(campaign) = get_campaign(env, campaign_id) {
                    if campaign.is_active && !campaign.is_cancelled {
                        active_campaigns += 1;
                    }
                    total_raised += campaign.amount_raised;
                    total_contributors += get_contributor_count(env, campaign_id);
                }
            }
        }
    }

    CreatorStats {
        total_campaigns: total,
        active_campaigns,
        total_raised,
        total_contributors,
    }
}

pub(crate) fn get_platform_stats(env: &Env) -> PlatformStats {
    // O(1) reads from maintained instance-storage counters (#411).
    // Counters are kept in sync by: create_campaign (+active), cancel_campaign (-active,
    // +cancelled), withdraw_funds (-active), and admin_verify / verify_with_votes
    // (+verified). No scan needed; stats_are_partial is always false.
    let total_campaigns = get_campaign_count(env);
    PlatformStats {
        total_campaigns,
        active_campaigns: get_active_campaign_count(env),
        verified_campaigns: get_verified_campaign_count(env),
        cancelled_campaigns: get_cancelled_campaign_count(env),
        total_amount_raised: get_total_raised_global(env),
        stats_are_partial: false,
        scanned_up_to: total_campaigns,
    }
}
