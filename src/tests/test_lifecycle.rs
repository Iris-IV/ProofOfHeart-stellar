use super::helpers::*;
use crate::{lifecycle::calculate_deadline, Category, Error, VotingKey, SECONDS_PER_DAY};
use soroban_sdk::{FromVal, String, TryFromVal};

// ── lifecycle events ────────────────────────────────────────────────────────────

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

// ── storage TTL policy ──────────────────────────────────────────────────────────

#[test]
fn test_storage_ttl_persistence_365_days() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    // 1. Create a campaign with 365 days duration
    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Long Campaign"),
        String::from_str(&env, "Testing TTL"),
        1000,
        365,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    // 2. Verify it's created and contributing works
    token_admin.mint(&contributor1, &1000);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);

    // 3. Fast-forward ledger sequence by 365 days
    // 17280 ledgers per day * 365 days = 6,307,200 ledgers
    let days_365_ledgers = 17280 * 365;
    let current_ledger = env.ledger().sequence();

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (365 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: current_ledger + days_365_ledgers,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    // 4. Verify campaign and contribution still exist
    let campaign = client.get_campaign(&id);
    assert_eq!(campaign.id, id);
    assert_eq!(campaign.amount_raised, 500);

    let contribution = client.get_contribution(&id, &contributor1);
    assert_eq!(contribution, 500);
}

fn has_persistent_key(env: &Env, client: &ProofOfHeartClient<'_>, key: VotingKey) -> bool {
    env.as_contract(&client.address, || env.storage().persistent().has(&key))
}

#[test]
fn test_admin_verification_refreshes_campaign_ttl() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = client.create_campaign(&make_params(
        creator,
        String::from_str(&env, "Admin Verification TTL"),
        String::from_str(&env, "Verification refreshes campaign storage"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    // Campaign storage is initially extended to roughly 400 days. Move close
    // to that expiry, then verify so admin_verify must refresh the campaign key.
    let current_ledger = env.ledger().sequence();
    let age_ledgers = 390 * 17280;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (390 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: current_ledger + age_ledgers,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10000000,
    });

    client.verify_campaign(&campaign_id);

    // Without the TTL refresh, the campaign would expire after the next 10
    // days; the refreshed entry should survive well beyond that point.
    let current_ledger = env.ledger().sequence();
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (20 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: current_ledger + (20 * 17280),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10000000,
    });

    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_verified);
}

#[test]
fn test_storage_state_after_withdraw_funds() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Withdraw State"),
        String::from_str(&env, "Test state after withdraw"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    client.withdraw_funds(&id);

    let campaign = client.get_campaign(&id);
    assert!(campaign.funds_withdrawn, "funds_withdrawn must be true");
    assert!(
        !campaign.is_active,
        "campaign must be inactive after withdraw"
    );
}

#[test]
fn test_voting_keys_absent_after_cancel() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Voting Keys Cancel"),
        String::from_str(&env, "Test voting key cleanup"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.cancel_campaign(&id);

    assert!(
        !has_persistent_key(&env, &client, VotingKey::ApproveVotes(id)),
        "ApproveVotes must not exist"
    );
    assert!(
        !has_persistent_key(&env, &client, VotingKey::RejectVotes(id)),
        "RejectVotes must not exist"
    );
}

#[test]
fn test_voting_keys_purged_after_cancel_with_prior_votes() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    let voter = Address::generate(&env);
    token_admin.mint(&voter, &500);

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Voting Keys Purge"),
        String::from_str(&env, "Test voting key purge"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.vote_on_campaign(&id, &voter, &true);

    assert!(
        has_persistent_key(&env, &client, VotingKey::ApproveVotes(id)),
        "ApproveVotes must exist before cancel"
    );

    client.cancel_campaign(&id);

    assert!(
        !has_persistent_key(&env, &client, VotingKey::ApproveVotes(id)),
        "ApproveVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, VotingKey::RejectVotes(id)),
        "RejectVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, VotingKey::ApproveWeight(id)),
        "ApproveWeight must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, VotingKey::RejectWeight(id)),
        "RejectWeight must be purged after cancel"
    );
}

#[test]
fn test_contributor_cap_ttl_is_extended_while_campaign_stays_active() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap TTL Campaign"),
        String::from_str(&env, "Cap TTL Test"),
        1_000,
        365,
        Category::EducationalStartup,
        false,
        0,
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.set_personal_cap(&campaign_id, &contributor, &500);
    client.contribute(&campaign_id, &contributor, &100);

    let current_ledger = env.ledger().sequence();
    let advance_ledgers = 300 * 17280;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (300 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: current_ledger + advance_ledgers,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100000,
        min_persistent_entry_ttl: 100000,
        max_entry_ttl: 1000000,
    });

    client.contribute(&campaign_id, &contributor, &50);

    let later_ledger = env.ledger().sequence();
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (200 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: later_ledger + (200 * 17280),
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100000,
        min_persistent_entry_ttl: 100000,
        max_entry_ttl: 1000000,
    });

    assert_eq!(client.get_personal_cap(&campaign_id, &contributor), 500);
    assert_eq!(client.get_lifetime_contribution(&campaign_id, &contributor), 150);
}

#[test]
fn test_verify_campaigns_extends_ttl_on_failure() {
    let (env, _admin, creator, contributor, _, _token, token_admin, client) = setup_env();

    // 1. Create a campaign
    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Failure TTL Campaign"),
        String::from_str(&env, "Failure TTL Test"),
        1000,
        30,
        Category::EducationalStartup,
        false,
        0,
        0i128,
    ));

    // 2. Cast a vote so voting state is initialized
    token_admin.mint(&contributor, &1000);
    client.vote_on_campaign(&campaign_id, &contributor, &true);

    // Assert voting state exists initially
    assert_eq!(client.get_approve_votes(&campaign_id), 1);

    // 3. Verify the campaign successfully first.
    let ids = soroban_sdk::Vec::from_array(&env, [campaign_id]);
    let (first_verified, first_failed) = client.verify_campaigns(&ids);
    assert_eq!(first_verified, ids);
    assert!(first_failed.is_empty());

    // Now try to verify the campaign again. Since it's already verified,
    // admin_verify fails — but #442 means the failure is reported in
    // failed_ids instead of collapsing the whole batch to Err(first_error).
    let (second_verified, second_failed) = client.verify_campaigns(&ids);
    assert!(second_verified.is_empty());
    assert_eq!(second_failed, ids);

    // 4. Despite the failure, the voting state TTL should have been extended.
    let current_ledger = env.ledger().sequence();
    let advance_ledgers = 20 * 17280; // 20 days
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (20 * SECONDS_PER_DAY),
        protocol_version: 22,
        sequence_number: current_ledger + advance_ledgers,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100000,
        min_persistent_entry_ttl: 100000,
        max_entry_ttl: 1000000,
    });

    // The voting state should still exist!
    assert_eq!(client.get_approve_votes(&campaign_id), 1);
}

// ── multi-step transaction sequence ─────────────────────────────────────────────

#[test]
fn test_multi_step_sequence() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = setup_token(&env, &admin);
    let client = setup_contract(&env, &admin, &token.address);

    let creator = Address::generate(&env);
    let params = crate::types::CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Test"),
        description: String::from_str(&env, "Desc"),
        funding_goal: 100_000,
        duration_days: 10,
        category: crate::types::Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };

    let id = client.create_campaign(&params);
    client.cancel_campaign(&id);
}

// ── calculate_deadline ───────────────────────────────────────────────────────────

#[test]
fn test_calculate_deadline_happy_path() {
    let current_time = 1_000_000;
    let duration_days = 30;
    let expected = current_time + duration_days * SECONDS_PER_DAY;
    assert_eq!(
        calculate_deadline(current_time, duration_days).unwrap(),
        expected
    );
}

#[test]
fn test_calculate_deadline_zero_days() {
    let current_time = 1_000_000;
    assert_eq!(calculate_deadline(current_time, 0).unwrap(), current_time);
}

#[test]
fn test_calculate_deadline_overflow_rejected() {
    let huge_days = u64::MAX / SECONDS_PER_DAY + 1;
    assert_eq!(
        calculate_deadline(0, huge_days),
        Err(Error::ValidationFailed)
    );
}
