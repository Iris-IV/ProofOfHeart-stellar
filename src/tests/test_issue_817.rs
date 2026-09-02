//! Tests for issue #817:
//! set_personal_cap must reject cap amounts below the contributor's current lifetime_contribution.

use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{Address, String};

fn make_campaign(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    goal: i128,
    days: u64,
    category: Category,
    max_per_user: i128,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "Issue 817 Test Campaign"),
        description: String::from_str(env, "Testing personal cap lifetime contribution validation"),
        funding_goal: goal,
        duration_days: days,
        category,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: max_per_user,
    })
}

#[test]
fn test_set_personal_cap_below_lifetime_contribution_rejected() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    let campaign_id = make_campaign(
        &env,
        &client,
        &creator,
        10_000,
        30,
        Category::Learner,
        0,
    );

    token_admin.mint(&contributor, &10_000);
    client.verify_campaign(&campaign_id);

    // Contributor donates 1,000 tokens
    client.contribute(&campaign_id, &contributor, &1_000);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        1_000
    );

    // Setting personal cap below lifetime contribution (e.g. 0, 1, 500, 999) must fail
    let res_zero = client.try_set_personal_cap(&campaign_id, &contributor, &0);
    assert_eq!(res_zero.unwrap_err().unwrap(), Error::ValidationFailed);

    let res_one = client.try_set_personal_cap(&campaign_id, &contributor, &1);
    assert_eq!(res_one.unwrap_err().unwrap(), Error::ValidationFailed);

    let res_500 = client.try_set_personal_cap(&campaign_id, &contributor, &500);
    assert_eq!(res_500.unwrap_err().unwrap(), Error::ValidationFailed);

    let res_999 = client.try_set_personal_cap(&campaign_id, &contributor, &999);
    assert_eq!(res_999.unwrap_err().unwrap(), Error::ValidationFailed);

    // Setting personal cap equal to lifetime contribution is allowed
    let res_exact = client.try_set_personal_cap(&campaign_id, &contributor, &1_000);
    assert!(res_exact.is_ok());

    // Further contribution is blocked since personal cap (1000) == lifetime (1000)
    let res_contrib = client.try_contribute(&campaign_id, &contributor, &1);
    assert_eq!(res_contrib.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    // Increasing personal cap above lifetime contribution is allowed
    let res_increase = client.try_set_personal_cap(&campaign_id, &contributor, &2_500);
    assert!(res_increase.is_ok());

    // Contributor can now contribute further up to 2_500 total
    client.contribute(&campaign_id, &contributor, &500);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        1_500
    );

    // Now setting personal cap below new lifetime contribution (1500) fails
    let res_below_new_lifetime = client.try_set_personal_cap(&campaign_id, &contributor, &1_499);
    assert_eq!(res_below_new_lifetime.unwrap_err().unwrap(), Error::ValidationFailed);

    // Setting personal cap to exactly new lifetime contribution (1500) succeeds
    let res_exact_new = client.try_set_personal_cap(&campaign_id, &contributor, &1_500);
    assert!(res_exact_new.is_ok());
}

#[test]
fn test_set_personal_cap_before_any_contributions() {
    let (env, _admin, creator, contributor, _, _token, _token_admin, client) = setup_env();

    let campaign_id = make_campaign(
        &env,
        &client,
        &creator,
        10_000,
        30,
        Category::Learner,
        5_000,
    );

    // Before any contributions, lifetime contribution is 0
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor),
        0
    );

    // Setting negative cap fails
    let res_neg = client.try_set_personal_cap(&campaign_id, &contributor, &-1);
    assert_eq!(res_neg.unwrap_err().unwrap(), Error::ValidationFailed);

    // Setting cap above campaign max_contribution_per_user fails
    let res_above_campaign_max = client.try_set_personal_cap(&campaign_id, &contributor, &5_001);
    assert_eq!(res_above_campaign_max.unwrap_err().unwrap(), Error::ValidationFailed);

    // Setting valid cap succeeds
    let res_valid = client.try_set_personal_cap(&campaign_id, &contributor, &2_000);
    assert!(res_valid.is_ok());
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor), 2_000);
}
