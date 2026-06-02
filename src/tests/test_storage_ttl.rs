use super::helpers::*;
use crate::{Category, DataKey};
use soroban_sdk::String;

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
        timestamp: env.ledger().timestamp() + (365 * 86400),
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

fn has_persistent_key(env: &Env, client: &ProofOfHeartClient<'_>, key: DataKey) -> bool {
    env.as_contract(&client.address, || env.storage().persistent().has(&key))
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
        !has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must not exist"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectVotes(id)),
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
        has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must exist before cancel"
    );

    client.cancel_campaign(&id);

    assert!(
        !has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectVotes(id)),
        "RejectVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::ApproveWeight(id)),
        "ApproveWeight must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectWeight(id)),
        "RejectWeight must be purged after cancel"
    );
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
    let first_res = client.verify_campaigns(&ids);
    assert_eq!(first_res, 1);

    // Now try to verify the campaign again.
    // Since it's already verified, it will fail verification.
    let second_res = client.try_verify_campaigns(&ids);
    assert!(second_res.is_err()); // verification failed (AdminVerificationConflict error)

    // 4. Despite the failure, the voting state TTL should have been extended.
    let current_ledger = env.ledger().sequence();
    let advance_ledgers = 20 * 17280; // 20 days
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp() + (20 * 86400),
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
