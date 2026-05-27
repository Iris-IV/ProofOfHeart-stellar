use super::helpers::*;
use crate::Category;
use soroban_sdk::String;

#[test]
fn test_storage_ttl_persistence_365_days() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) = setup_env();

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
