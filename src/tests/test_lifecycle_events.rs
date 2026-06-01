use super::helpers::*;
use soroban_sdk::{FromVal, String, TryFromVal};

fn has_event(env: &soroban_sdk::Env, topic: &str) -> bool {
    let expected = String::from_str(env, topic);
    env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|v| String::try_from_val(env, &v).ok())
            .map(|s| s == expected)
            .unwrap_or(false)
    })
}

#[test]
fn test_full_lifecycle_event_sequence() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &5_000);

    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Lifecycle Campaign"),
        description: String::from_str(&env, "Full lifecycle test"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 1000,
        max_contribution_per_user: 0,
    });

    assert!(
        has_event(&env, "campaign_created"),
        "campaign_created event must be emitted"
    );

    client.verify_campaign(&id);
    assert!(
        has_event(&env, "campaign_verified"),
        "campaign_verified event must be emitted"
    );

    client.contribute(&id, &contributor1, &1_000);
    assert!(
        has_event(&env, "contribution_made"),
        "contribution_made event must be emitted"
    );

    client.withdraw_funds(&id);
    assert!(
        has_event(&env, "withdrawal"),
        "withdrawal event must be emitted"
    );

    client.deposit_revenue(&id, &2_000);
    assert!(
        has_event(&env, "revenue_deposited"),
        "revenue_deposited event must be emitted"
    );

    client.claim_revenue(&id, &contributor1);
    assert!(
        has_event(&env, "revenue_claimed"),
        "revenue_claimed event must be emitted"
    );

    client.claim_creator_revenue(&id);
    assert!(
        has_event(&env, "creator_revenue_claimed"),
        "creator_revenue_claimed event must be emitted"
    );

    let total = env.events().all().len();
    assert!(
        total >= 8,
        "full lifecycle must emit at least 8 events, got {}",
        total
    );
}

#[test]
fn test_cancel_lifecycle_event_sequence() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5_000);

    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cancelled Campaign"),
        description: String::from_str(&env, "Will be cancelled"),
        funding_goal: 10_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);
    client.cancel_campaign(&id);

    assert!(
        has_event(&env, "campaign_cancelled"),
        "campaign_cancelled event must be emitted"
    );

    client.claim_refund(&id, &contributor1);
    assert!(
        has_event(&env, "refund_claimed"),
        "refund_claimed event must be emitted"
    );
}

#[test]
fn test_campaign_cancelled_event_includes_creator_and_amount() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5_000);

    let id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cancelled Campaign Event Payload"),
        description: String::from_str(&env, "Verify cancel event schema"),
        funding_goal: 10_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);
    client.cancel_campaign(&id);

    let events = env.events().all();
    let last_event = events.last().unwrap();

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);
    let creator_in_topics: Address = FromVal::from_val(&env, &topics.get(2).unwrap());
    assert_eq!(creator_in_topics, creator);

    let amount_raised: i128 = FromVal::from_val(&env, &last_event.2);
    assert_eq!(amount_raised, 500);
}

#[test]
fn test_campaign_created_event_includes_category() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let expected_title = String::from_str(&env, "Created Event Category");
    let expected_category = Category::Learner;

    client.create_campaign(&CreateCampaignParams {
        creator,
        title: expected_title.clone(),
        description: String::from_str(&env, "Schema coverage"),
        funding_goal: 1_000,
        duration_days: 30,
        category: expected_category,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    let events = env.events().all();
    let created_event = events
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|topic| topic == String::from_str(&env, "campaign_created"))
                .unwrap_or(false)
        })
        .expect("campaign_created event must exist");

    let (title, category_discriminant): (String, u32) = FromVal::from_val(&env, &created_event.2);
    assert_eq!(title, expected_title);
    assert_eq!(category_discriminant, expected_category as u32);
}
