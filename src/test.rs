use super::*;
use soroban_sdk::{Env, String, Vec};

#[test]
fn test_list_active_campaigns_with_tag_filter() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ProofOfHeartContract);
    let client = ProofOfHeartContractClient::new(&env, &contract_id);

    // Mock setup: create campaigns with tags
    let tag_africa = String::from_str(&env, "africa");
    let tag_stem = String::from_str(&env, "stem");

    // ... populate test campaigns in storage ...

    // Test filtering by 'africa' tag
    let africa_campaigns = client.list_active_campaigns(&Some(tag_africa));
    assert_eq!(africa_campaigns.len(), 1);

    // Test unfiltered retrieval returns all active campaigns
    let all_campaigns = client.list_active_campaigns(&None);
    assert!(all_campaigns.len() >= 2);
}