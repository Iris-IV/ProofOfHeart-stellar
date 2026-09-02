use soroban_sdk::{Address, Env, String};

use crate::constants::MAX_SCAN_WINDOW;
use crate::storage::{
    get_active_campaign_count, get_campaign, get_campaign_count, get_campaign_tags,
    get_cancelled_campaign_count, get_category_campaign_bucket, get_category_campaign_count,
    get_contribution, get_contributor_count, get_creator_campaign_bucket,
    get_creator_campaign_count, get_last_contribution_time, get_platform_fee,
    get_tag_campaign_count, get_tag_campaigns_bucket, get_token, get_top_contributor,
    get_total_raised_global, get_verified_campaign_count, hash_text,
    CATEGORY_CAMPAIGNS_BUCKET_SIZE, CREATOR_CAMPAIGNS_BUCKET_SIZE, TAG_CAMPAIGNS_BUCKET_SIZE,
};
use crate::types::{
    Campaign, CampaignStats, Category, CreatorStats, MaybePendingCreator, PlatformReport,
    PlatformStats,
};

// #787: read-only query helpers work with `&Campaign` references where they
// inspect campaign state, avoiding by-value copies of the heavy Campaign
// struct in the WASM VM.

/// Returns all campaigns (active, inactive, cancelled) ordered by campaign ID,
/// in ascending order.
///
/// # Pagination
///
/// The `start` parameter is an **exclusive cursor** — pass the last campaign ID
/// from the previous page to begin the next page. Begin with `start = 0`.
///
/// After each request, set `start` to the ID of the last campaign received.
/// Stop when fewer than `limit` results are returned (all results have been
/// retrieved).
///
/// ```text
/// // Example: fetch all campaigns in pages of 10
/// let mut start = 0u32;
/// let limit = 10u32;
/// loop {
///     let page = client.list_campaigns(&start, &limit);
///     if page.len() == 0 { break; }
///     // process page
///     start = page.get(page.len() - 1).unwrap().id;
///     if page.len() < limit as usize { break; }
/// }
/// ```
pub(crate) fn list_campaigns(env: &Env, start: u32, limit: u32) -> soroban_sdk::Vec<Campaign> {
    let total_count = get_campaign_count(env);
    let mut campaigns = soroban_sdk::Vec::new(env);

    if start >= total_count || limit == 0 {
        return campaigns;
    }

    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let end = start.saturating_add(capped_limit).min(total_count);

    for id in (start.saturating_add(1))..=end {
        if let Some(campaign) = get_campaign(env, id) {
            campaigns.push_back(campaign);
        }
    }

    campaigns
}

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
    let mut current_id = start.saturating_add(1);
    let mut next_cursor = 0u32;
    let scan_window_end = start.saturating_add(MAX_SCAN_WINDOW);

    while current_id <= total_count {
        if current_id > start.saturating_add(MAX_SCAN_WINDOW) {
            env.events().publish(
                ("scan_window_exhausted",),
                (start, current_id, collected, capped_limit),
            );
            next_cursor = current_id;
            break;
        }

        if let Some(campaign) = get_campaign(env, current_id) {
            if campaign.is_active && !campaign.is_cancelled {
                campaigns.push_back(campaign);
                collected += 1;
                if collected >= capped_limit {
                    next_cursor = current_id.saturating_add(1);
                    break;
                }
            }
        }
        
        if current_id == u32::MAX {
            break;
        }
        current_id += 1;
    }

    (campaigns, next_cursor)
}

/// Shared bucket-pagination helper used by `get_campaigns_by_category`,
/// `get_campaigns_by_tag`, and `get_creator_campaigns`. The query functions
/// differ only in how they derive the total count and how they load a
/// bucket — this helper captures the identical traversal algorithm so there
/// is one canonical implementation.
///
/// # Cursor contract (#845)
///
/// `start` here is a **zero-based positional offset** into the query's own
/// result ordering (the Nth campaign matching that category/tag/creator),
/// *not* a campaign ID. This is a different contract from [`list_campaigns`],
/// whose `start` is an **exclusive campaign-ID cursor**. The two are not
/// interchangeable: a cursor obtained from one of these bucket-paginated
/// queries cannot be passed as `start` to `list_campaigns` (or vice versa).
/// To page through a bucket-paginated query, set the next `start` to
/// `previous_start + previous_page.len()` and stop once a page returns fewer
/// than `limit` results.
///
/// Algorithm overview:
///   1. Jump to the bucket containing `start`.
///   2. Walk entries within that bucket starting at the requested position.
///   3. Collect up to `limit` campaigns (capped at `LIST_MAX_LIMIT`).
///   4. When the bucket is exhausted, advance `position` past the bucket
///      boundary and repeat from step 1 with the next bucket.
fn get_campaigns_from_buckets<F>(
    env: &Env,
    start: u32,
    limit: u32,
    total: u32,
    bucket_size: u32,
    get_bucket: F,
) -> (soroban_sdk::Vec<Campaign>, u32)
where
    F: Fn(&Env, u32) -> soroban_sdk::Vec<u32>,
{
    let mut campaigns = soroban_sdk::Vec::new(env);
    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);

    if start >= total || capped_limit == 0 {
        return (campaigns, start);
    }

    let end = start.saturating_add(capped_limit).min(total);
    let mut position = start;
    let mut next_cursor = start;

    while position < end {
        let bucket_idx = position / bucket_size;
        let bucket = get_bucket(env, bucket_idx);
        let bucket_start = bucket_idx * bucket_size;
        let mut idx_in_bucket = position - bucket_start;

        let bucket_len = bucket.len();
        while idx_in_bucket < bucket_len && position < end {
            // `if let Some` rather than `unwrap()` is intentional: a sparse
            // bucket entry is skipped (not a panic), mirroring the
            // creator-campaign path's behaviour.
            if let Some(campaign_id) = bucket.get(idx_in_bucket) {
                if let Some(campaign) = get_campaign(env, campaign_id) {
                    campaigns.push_back(campaign);
                    next_cursor = position + 1;
                }
            }
            idx_in_bucket += 1;
            position += 1;
        }

        if idx_in_bucket >= bucket_len {
            position = if bucket_len == 0 {
                bucket_start + bucket_size
            } else {
                bucket_start + bucket_len
            };
        } else {
            next_cursor = position;
        }
    }

    (campaigns, next_cursor)
}

/// Returns the campaigns in `category`, paginated.
///
/// `offset` is a zero-based positional index into this category's campaign
/// list — **not** a campaign ID. See the cursor contract note on
/// [`get_campaigns_from_buckets`] (#845) for how this differs from
/// `list_campaigns`'s ID-based cursor.
pub(crate) fn get_campaigns_by_category(
    env: &Env,
    category: Category,
    offset: u32,
    limit: u32,
) -> (soroban_sdk::Vec<Campaign>, u32) {
    let total = get_category_campaign_count(env, category);
    get_campaigns_from_buckets(
        env,
        offset,
        limit,
        total,
        CATEGORY_CAMPAIGNS_BUCKET_SIZE,
        |e, idx| get_category_campaign_bucket(e, category, idx),
    )
}

/// Returns the campaigns tagged with `tag`, paginated (#798).
///
/// Backed by the inverted tag index maintained by `add_campaign_tag`, so this
/// is O(page) rather than a full campaign scan. `offset` is a zero-based
/// index into the tag's campaign list (not a campaign id) — see the cursor
/// contract note on [`get_campaigns_from_buckets`] (#845). An unknown,
/// empty, or over-long tag returns an empty page.
pub(crate) fn get_campaigns_by_tag(
    env: &Env,
    tag: String,
    offset: u32,
    limit: u32,
) -> (soroban_sdk::Vec<Campaign>, u32) {
    if tag.len() < crate::CAMPAIGN_TAG_MIN_LEN || tag.len() > crate::CAMPAIGN_TAG_MAX_LEN {
        return (soroban_sdk::Vec::new(env), offset);
    }
    let tag_hash = hash_text(env, &tag);
    let total = get_tag_campaign_count(env, &tag_hash);
    get_campaigns_from_buckets(
        env,
        offset,
        limit,
        total,
        TAG_CAMPAIGNS_BUCKET_SIZE,
        |e, idx| get_tag_campaigns_bucket(e, &tag_hash, idx),
    )
}

/// Returns the tags applied to a campaign, in the order they were added (#798).
pub(crate) fn get_campaign_tag_list(env: &Env, campaign_id: u32) -> soroban_sdk::Vec<String> {
    get_campaign_tags(env, campaign_id)
}

/// #534: jumps straight to the bucket containing `start` instead of reading
/// every preceding bucket just to advance a counter, so paginating deep into
/// a creator with many campaigns no longer costs one ledger read per skipped
/// bucket (mirrors `get_campaigns_by_category`'s direct-jump approach).
///
/// `start` is a zero-based positional index into this creator's campaign
/// list — **not** a campaign ID. See the cursor contract note on
/// [`get_campaigns_from_buckets`] (#845) for how this differs from
/// `list_campaigns`'s ID-based cursor.
pub(crate) fn get_creator_campaigns(
    env: &Env,
    creator: Address,
    start: u32,
    limit: u32,
) -> (soroban_sdk::Vec<Campaign>, u32) {
    let total = get_creator_campaign_count(env, &creator);
    get_campaigns_from_buckets(
        env,
        start,
        limit,
        total,
        CREATOR_CAMPAIGNS_BUCKET_SIZE,
        |e, idx| get_creator_campaign_bucket(e, &creator, idx),
    )
}

/// Aggregates total raised, active campaign count, and total contributors
/// across every campaign owned by `creator` (#519). Walks the creator's
/// campaign buckets directly (same storage layout `get_creator_campaigns`
/// paginates over) rather than the paginated query, since a creator's own
/// campaign count is bounded by normal usage and the caller wants a
/// complete aggregate, not a page.
///
/// **Existence semantics:** `total_campaigns` is the existence indicator.
/// Consumers can distinguish an unknown creator from a known creator with no
/// activity by checking `total_campaigns > 0`. A value of `0` means `creator`
/// has no campaign index and is not a known creator. A known creator with no
/// activity still has `total_campaigns > 0`; its other aggregate fields may be
/// zero.
///
/// **Note:** `total_contributors` is a sum of the contributor counts of all
/// creator's campaigns. Because no registry of unique contributor addresses
/// is maintained per campaign/creator in storage, this value can double-count
/// contributors who support multiple campaigns by this creator. It represents
/// the total contribution events rather than the count of unique wallets.
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
                    if !campaign.is_cancelled {
                        total_raised += campaign.amount_raised;
                    }
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
    //
    // Since counters were made O(1) (issue #411), `stats_are_partial` and
    // `scanned_up_to` are hardcoded constants retained for API compatibility.
    // These fields no longer vary and can never differ from their hardcoded
    // values (false and total_campaigns, respectively).
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

/// Returns aggregate contribution stats for a single campaign: contributor
/// count, current top contributor, average contribution size, and the
/// timestamp of the most recent contribution.
pub(crate) fn get_campaign_stats(env: &Env, campaign_id: u32) -> CampaignStats {
    let contributor_count = get_contributor_count(env, campaign_id);
    let amount_raised = get_campaign(env, campaign_id)
        .map(|c| c.amount_raised)
        .unwrap_or(0);

    let avg_contribution = if contributor_count > 0 {
        amount_raised / contributor_count as i128
    } else {
        0
    };

    let top_contributor = get_top_contributor(env, campaign_id)
        .map(MaybePendingCreator::from)
        .unwrap_or(MaybePendingCreator::None);

    CampaignStats {
        contributor_count,
        top_contributor,
        avg_contribution,
        last_contribution_time: get_last_contribution_time(env, campaign_id),
    }
}

/// Returns a comprehensive platform report with all key metrics in a
/// single call (#541). Useful for admin dashboards and health checks.
pub(crate) fn get_platform_report(env: &Env) -> PlatformReport {
    let total_campaigns = get_campaign_count(env);
    let active_campaigns = get_active_campaign_count(env);
    let total_raised = get_total_raised_global(env);
    let platform_fee_bps = get_platform_fee(env);
    let is_paused = env
        .storage()
        .instance()
        .get(&crate::storage::AdminKey::Paused)
        .unwrap_or(false)
        || env
            .storage()
            .instance()
            .get(&crate::storage::AdminKey::AutoPaused)
            .unwrap_or(false);

    let mut total_contributors: u32 = 0;
    for id in 1..=total_campaigns {
        if get_campaign(env, id).is_some() {
            total_contributors += get_contributor_count(env, id);
        }
    }

    PlatformReport {
        total_campaigns,
        active_campaigns,
        total_raised,
        total_contributors,
        platform_fee_bps,
        is_paused,
        token: get_token(env),
    }
}

/// Returns the contributor's portfolio across all campaigns: for each
/// campaign the contributor has backed, returns the campaign ID, the
/// contribution amount, the campaign's current status, and whether a
/// refund is currently available (#539).
pub(crate) fn get_contributor_portfolio(
    env: &Env,
    contributor: Address,
    start: u32,
    limit: u32,
) -> soroban_sdk::Vec<(u32, i128, String, bool)> {
    let total_campaigns = get_campaign_count(env);
    let mut portfolio = soroban_sdk::Vec::new(env);

    if start >= total_campaigns || limit == 0 {
        return portfolio;
    }

    let capped_limit = limit.min(crate::LIST_MAX_LIMIT);
    let mut collected = 0;

    for id in (start + 1)..=total_campaigns {
        let amount = get_contribution(env, id, &contributor);
        if amount == 0 {
            continue;
        }

        if let Some(campaign) = get_campaign(env, id) {
            let status = if campaign.is_cancelled {
                "cancelled"
            } else if campaign.funds_withdrawn {
                "withdrawn"
            } else if !campaign.is_active {
                "inactive"
            } else if campaign.is_verified {
                "verified"
            } else {
                "active"
            };

            let refundable = campaign.is_cancelled
                || (env.ledger().timestamp() > campaign.deadline
                    && campaign.amount_raised < campaign.funding_goal);

            portfolio.push_back((id, amount, String::from_str(env, status), refundable));
            collected += 1;

            if collected >= capped_limit {
                break;
            }
        }
    }

    portfolio
}
