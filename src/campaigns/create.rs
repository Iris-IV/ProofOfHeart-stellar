use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{calculate_deadline, require_not_paused};
use crate::storage::{
    bump_instance_ttl, get_campaign_count, get_category_campaign_bucket,
    get_category_campaign_count, get_category_duration_cap, get_creation_disabled,
    get_creator_campaign_bucket, get_creator_campaign_count, get_max_campaign_funding_goal,
    get_min_campaign_funding_goal, get_withdraw_release_delay_days,
    get_withdraw_reserve_percentage, set_campaign, set_campaign_count, set_campaign_creator_index,
    set_campaign_start_time, set_campaign_token, set_campaign_vesting,
    set_category_campaign_bucket, set_category_campaign_count, set_creator_campaign_bucket,
    set_creator_campaign_count, set_creator_campaign_position, set_revenue_pool,
    CATEGORY_CAMPAIGNS_BUCKET_SIZE, CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
use crate::storage::{get_token, is_token_explicitly_allowed};
use crate::types::{Campaign, Category, CreateCampaignParams, MaybePendingCreator};

/// Creates a campaign denominated in the platform token.
///
/// Kept as the default entry point so existing clients are unaffected by
/// per-campaign currencies (#784); it delegates to
/// `create_campaign_with_token` with the platform token, which is always
/// allowed.
pub(crate) fn create_campaign(env: &Env, params: CreateCampaignParams) -> Result<u32, Error> {
    create_campaign_inner(env, params, None)
}

/// Creates a campaign denominated in `token` (#784).
///
/// The token is pinned at creation and never changes: contributions, refunds,
/// withdrawals, milestone payouts and revenue for this campaign all move in
/// this currency. A campaign that could switch currency mid-flight would hold
/// contributions in one asset and owe refunds in another.
///
/// The token must be on the admin allowlist. Letting a creator name an
/// arbitrary contract as their campaign's currency would hand every
/// contributor a `transfer` call into code the platform never reviewed — the
/// asset a contributor is asked to part with has to be one the platform
/// stands behind, not one the fundraiser chose.
pub(crate) fn create_campaign_with_token(
    env: &Env,
    params: CreateCampaignParams,
    token: Address,
) -> Result<u32, Error> {
    create_campaign_inner(env, params, Some(token))
}

/// Shared body. `token` is `None` for the platform-token default.
///
/// The distinction is not cosmetic: the default path must not read the
/// platform token, allowlist it, or write a currency key, so that a campaign
/// created the ordinary way costs exactly what it cost before per-campaign
/// currencies existed. Campaign creation runs for every campaign ever made,
/// and the host budget in tests that create a hundred of them in a single
/// invocation is tight enough to notice the difference.
fn create_campaign_inner(
    env: &Env,
    params: CreateCampaignParams,
    token: Option<Address>,
) -> Result<u32, Error> {
    params.creator.require_auth();
    require_not_paused(env)?;
    if get_creation_disabled(env) {
        return Err(Error::CreationDisabled);
    }

    let CreateCampaignParams {
        creator,
        title,
        description,
        funding_goal,
        duration_days,
        category,
        has_revenue_sharing,
        revenue_share_percentage,
        max_contribution_per_user,
    } = params;

    if funding_goal <= 0 {
        return Err(Error::FundingGoalMustBePositive);
    }
    if funding_goal < get_min_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MIN) {
        return Err(Error::FundingGoalTooLow);
    }
    if funding_goal > get_max_campaign_funding_goal(env, crate::CAMPAIGN_FUNDING_GOAL_MAX) {
        return Err(Error::FundingGoalTooHigh);
    }
    let duration_max =
        get_category_duration_cap(env, category).unwrap_or(crate::CAMPAIGN_DURATION_MAX_DAYS);
    if !(crate::CAMPAIGN_DURATION_MIN_DAYS..=duration_max).contains(&duration_days) {
        return Err(Error::InvalidDuration);
    }
    if title.len() < crate::CAMPAIGN_TITLE_MIN_LEN || title.len() > crate::CAMPAIGN_TITLE_MAX_LEN {
        return Err(Error::ValidationFailed);
    }
    if description.len() < crate::CAMPAIGN_DESCRIPTION_MIN_LEN
        || description.len() > crate::CAMPAIGN_DESCRIPTION_MAX_LEN
    {
        return Err(Error::ValidationFailed);
    }
    if category != Category::EducationalStartup && has_revenue_sharing {
        return Err(Error::RevenueShareOnlyForStartup);
    }

    // Normalise: force percentage to 0 when revenue sharing is disabled so
    // the stored (has_revenue_sharing, percentage) pair is always coherent.
    let revenue_share_percentage = if !has_revenue_sharing {
        0u32
    } else {
        revenue_share_percentage
    };

    if revenue_share_percentage > crate::REVENUE_SHARE_MAX_BPS {
        return Err(Error::InvalidRevenueShare);
    }
    if has_revenue_sharing && revenue_share_percentage == 0 {
        return Err(Error::InvalidRevenueShare);
    }
    // `0` is accepted and means "no per-user cap" (unlimited) — an explicit,
    // documented sentinel, not "0 tokens allowed". Only negative values are
    // rejected here (#530).
    if max_contribution_per_user < 0 {
        return Err(Error::ValidationFailed);
    }
    // `ValidationFailed` rather than a dedicated code: `Error` is already at
    // Soroban's fifty-case ceiling for `#[contracterror]` unions, so there is
    // no free slot. The event and the `is_token_allowed` getter give callers a
    // way to tell this apart from the other validation failures above.
    // `ValidationFailed` rather than a dedicated code: `Error` is already at
    // Soroban's fifty-case ceiling for `#[contracterror]` unions, so there is
    // no free slot. `is_token_allowed` lets a caller tell this apart from the
    // other validation failures above.
    let pinned_token = match &token {
        Some(t) if *t != get_token(env) => {
            if !is_token_explicitly_allowed(env, t) {
                return Err(Error::ValidationFailed);
            }
            Some(t.clone())
        }
        // Explicitly naming the platform token is accepted and behaves
        // identically to `create_campaign`.
        _ => None,
    };

    bump_instance_ttl(env);
    let mut count = get_campaign_count(env);
    count += 1;

    let deadline = calculate_deadline(env.ledger().timestamp(), duration_days)?;

    let campaign = Campaign {
        id: count,
        creator: creator.clone(),
        first_creator: creator.clone(),
        pending_creator: MaybePendingCreator::None,
        title: title.clone(),
        description,
        funding_goal,
        deadline,
        amount_raised: 0,
        is_active: true,
        funds_withdrawn: false,
        is_cancelled: false,
        is_verified: false,
        category,
        has_revenue_sharing,
        revenue_share_percentage,
        max_contribution_per_user,
        fee_override: None,
        deadline_extended: false,
        effective_amount_raised: 0,
    };

    // Snapshot the current global vesting parameters per-campaign so that
    // future changes to `set_vesting_params` do not retroactively affect
    // campaigns already created (#466).
    let vesting_delay = get_withdraw_release_delay_days(env);
    let vesting_bps = get_withdraw_reserve_percentage(env);
    set_campaign_vesting(env, count, vesting_delay, vesting_bps);

    // Record the campaign's currency only when it differs from the platform
    // token (#784).
    //
    // Writing it unconditionally would add a persistent entry — and its rent —
    // to every campaign ever created, to store a value that is already the
    // default. That cost is not hypothetical: it is enough to exhaust the host
    // budget in `bookmarks::tests::test_bookmark_limit_reached`, which creates
    // fifty-one campaigns in one invocation.
    //
    // Campaigns that omit the key resolve through `get_campaign_token`'s
    // fallback to the platform token, which is exactly the behaviour they had
    // before this feature existed. The trade-off is that such a campaign
    // follows a later `accept_token_update` rather than staying pinned — again
    // the pre-existing behaviour, and one `accept_token_update` already
    // narrows by refusing to swap while any campaign holds escrowed funds
    // (#407). A campaign created with an explicit non-default currency is
    // pinned and never follows a platform-level change.
    if let Some(t) = pinned_token {
        set_campaign_token(env, count, &t);
        env.events().publish(("campaign_token_set", count), t);
    }

    set_campaign(env, count, &campaign);
    set_campaign_start_time(env, count, env.ledger().timestamp());
    set_campaign_count(env, count);
    set_revenue_pool(env, count, 0);
    let category_count = get_category_campaign_count(env, category);
    let bucket_idx = category_count / CATEGORY_CAMPAIGNS_BUCKET_SIZE;
    let mut bucket = get_category_campaign_bucket(env, category, bucket_idx);
    bucket.push_back(count);
    set_category_campaign_bucket(env, category, bucket_idx, &bucket);
    set_category_campaign_count(env, category, category_count + 1);

    let creator_count = get_creator_campaign_count(env, &creator);
    let bucket_idx = creator_count / CREATOR_CAMPAIGNS_BUCKET_SIZE;
    let mut bucket = get_creator_campaign_bucket(env, &creator, bucket_idx);
    bucket.push_back(count);
    set_creator_campaign_bucket(env, &creator, bucket_idx, &bucket);
    set_creator_campaign_position(env, &creator, count, bucket_idx, bucket.len() - 1);
    set_creator_campaign_count(env, &creator, creator_count + 1);
    set_campaign_creator_index(env, count, &creator);

    env.events().publish(
        ("campaign_created", count, creator),
        (title, category as u32),
    );

    Ok(count)
}
