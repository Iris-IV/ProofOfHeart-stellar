use soroban_sdk::{Address, Env};

use crate::storage::{
    get_campaign, get_campaign_count, get_category_campaign_bucket, get_category_campaign_count,
    get_creator_campaign_bucket, get_creator_campaign_count, get_total_raised_global,
    CATEGORY_CAMPAIGNS_BUCKET_SIZE, CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
use crate::types::{Campaign, Category, PlatformStats};

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

    const MAX_SCAN_WINDOW: u32 = 200;
    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let mut collected = 0u32;
    let mut current_id = start + 1;
    let mut next_cursor = 0u32;

    while current_id <= total_count {
        if current_id > start + MAX_SCAN_WINDOW {
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
    let num_buckets = total.div_ceil(CREATOR_CAMPAIGNS_BUCKET_SIZE);
    let mut global_idx = 0u32;

    'outer: for bucket_idx in 0..num_buckets {
        let bucket = get_creator_campaign_bucket(env, &creator, bucket_idx);
        for i in 0..bucket.len() {
            if global_idx >= end {
                break 'outer;
            }
            if global_idx >= start {
                if let Some(campaign_id) = bucket.get(i) {
                    if let Some(campaign) = get_campaign(env, campaign_id) {
                        campaigns.push_back(campaign);
                    }
                }
            }
            global_idx += 1;
        }
    }

    campaigns
}

pub(crate) fn get_platform_stats(env: &Env) -> PlatformStats {
    let total_campaigns = get_campaign_count(env);
    let mut active_campaigns = 0u32;
    let mut verified_campaigns = 0u32;
    let mut cancelled_campaigns = 0u32;

    const MAX_SCAN_LIMIT: u32 = 1000;
    let scan_end = total_campaigns.min(MAX_SCAN_LIMIT);

    let mut id = 1u32;
    while id <= scan_end {
        if let Some(campaign) = get_campaign(env, id) {
            if campaign.is_active && !campaign.is_cancelled {
                active_campaigns += 1;
            }
            if campaign.is_verified {
                verified_campaigns += 1;
            }
            if campaign.is_cancelled {
                cancelled_campaigns += 1;
            }
        }
        id += 1;
    }

    PlatformStats {
        total_campaigns,
        active_campaigns,
        verified_campaigns,
        cancelled_campaigns,
        total_amount_raised: get_total_raised_global(env),
        stats_are_partial: total_campaigns > MAX_SCAN_LIMIT,
        scanned_up_to: scan_end,
    }
}
