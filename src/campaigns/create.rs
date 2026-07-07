use soroban_sdk::Env;

use crate::errors::Error;
use crate::lifecycle::{calculate_deadline, require_not_paused};
use crate::storage::{
    bump_instance_ttl, get_campaign_count, get_category_campaign_bucket,
    get_category_campaign_count, get_category_duration_cap, get_creation_disabled,
    get_creator_campaign_bucket, get_creator_campaign_count, get_max_campaign_funding_goal,
    get_min_campaign_funding_goal, set_campaign, set_campaign_count, set_campaign_start_time,
    set_category_campaign_bucket, set_category_campaign_count, set_creator_campaign_bucket,
    set_creator_campaign_count, set_revenue_pool, CATEGORY_CAMPAIGNS_BUCKET_SIZE,
    CREATOR_CAMPAIGNS_BUCKET_SIZE,
};
use crate::types::{Campaign, Category, CreateCampaignParams, MaybePendingCreator};

pub(crate) fn create_campaign(env: &Env, params: CreateCampaignParams) -> Result<u32, Error> {
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
    if max_contribution_per_user < 0 {
        return Err(Error::ValidationFailed);
    }

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
    set_creator_campaign_count(env, &creator, creator_count + 1);

    env.events().publish(
        ("campaign_created", count, creator),
        (title, category as u32),
    );

    Ok(count)
}
