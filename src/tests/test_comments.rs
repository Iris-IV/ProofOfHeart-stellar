#![cfg(test)]
extern crate std;

use super::helpers::{setup_env, setup_env_with_active_campaign};
use crate::ProofOfHeartContractClient;
use soroban_sdk::{testutils::Events, String};

#[test]
fn test_add_and_remove_comment() {
    let (env, contract_id, _, creator, _) = setup_env_with_active_campaign(100);
    let client = ProofOfHeartContractClient::new(&env, &contract_id);

    let commenter = soroban_sdk::Address::generate(&env);
    let comment_hash = String::from_str(&env, "QmHash123");

    // Add comment
    let comment_id = client.add_campaign_comment(&1, &commenter, &comment_hash);
    assert_eq!(comment_id, 1);

    // Verify events
    let events = env.events().all();
    let mut found_added = false;
    for (contract, topic, data) in events.iter() {
        if contract == contract_id {
            if let Ok(t) = topic.clone().try_into_val(&env) {
                let t: soroban_sdk::Vec<soroban_sdk::Val> = t;
                if t.len() == 3 {
                    let event_name: String = t.get(0).unwrap().try_into_val(&env).unwrap();
                    if event_name == String::from_str(&env, "campaign_comment_added") {
                        found_added = true;
                        let cid: u32 = t.get(2).unwrap().try_into_val(&env).unwrap();
                        assert_eq!(cid, 1);
                    }
                }
            }
        }
    }
    assert!(found_added);

    // Remove comment by creator
    client.remove_campaign_comment(&1, &comment_id, &creator);

    // Verify remove event
    let events = env.events().all();
    let mut found_removed = false;
    for (contract, topic, data) in events.iter() {
        if contract == contract_id {
            if let Ok(t) = topic.clone().try_into_val(&env) {
                let t: soroban_sdk::Vec<soroban_sdk::Val> = t;
                if t.len() == 3 {
                    let event_name: String = t.get(0).unwrap().try_into_val(&env).unwrap();
                    if event_name == String::from_str(&env, "campaign_comment_removed") {
                        found_removed = true;
                    }
                }
            }
        }
    }
    assert!(found_removed);

    // Ensure double remove fails
    let res = client.try_remove_campaign_comment(&1, &comment_id, &creator);
    assert!(res.is_err()); // CommentAlreadyRemoved
}

#[test]
fn test_remove_comment_unauthorized() {
    let (env, contract_id, _, creator, _) = setup_env_with_active_campaign(100);
    let client = ProofOfHeartContractClient::new(&env, &contract_id);

    let commenter = soroban_sdk::Address::generate(&env);
    let comment_hash = String::from_str(&env, "QmHash123");

    let comment_id = client.add_campaign_comment(&1, &commenter, &comment_hash);

    let stranger = soroban_sdk::Address::generate(&env);
    let res = client.try_remove_campaign_comment(&1, &comment_id, &stranger);
    assert!(res.is_err()); // NotAuthorized
}
