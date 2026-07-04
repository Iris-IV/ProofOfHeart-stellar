use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::String;

/// Issue #164: creator cannot cancel after the funding goal has been reached
/// and funds have not yet been withdrawn (rug-pull prevention).
#[test]
fn test_cancel_campaign_blocked_after_goal_met() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let goal = 1000i128;
    token_admin.mint(&contributor1, &goal);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Goal Met Campaign"),
        description: String::from_str(&env, "Goal is met; cancel must be rejected"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    // Contribute exactly the funding goal
    client.contribute(&campaign_id, &contributor1, &goal);

    assert_eq!(client.get_campaign(&campaign_id).amount_raised, goal);

    // Creator tries to cancel — must be rejected
    let result = client.try_cancel_campaign(&campaign_id);
    assert_eq!(result, Err(Ok(Error::GoalMetCancellationNotAllowed)));
}

/// Creator can still cancel when contributions are below the funding goal.
#[test]
fn test_cancel_campaign_allowed_when_goal_not_met() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    let goal = 2000i128;
    token_admin.mint(&contributor1, &500);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Partial Campaign"),
        description: String::from_str(&env, "Goal not met; cancel is allowed"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &500);

    // Goal not reached — cancellation must succeed
    client.cancel_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_cancelled);
}

/// If amount_raised exceeds the goal the block still applies.
#[test]
fn test_cancel_campaign_blocked_when_amount_exceeds_goal() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    let goal = 500i128;
    token_admin.mint(&contributor1, &600);
    token_admin.mint(&contributor2, &200);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Over-funded Campaign"),
        description: String::from_str(&env, "Raised more than goal"),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &600);

    let result = client.try_cancel_campaign(&campaign_id);
    assert_eq!(result, Err(Ok(Error::GoalMetCancellationNotAllowed)));
}
