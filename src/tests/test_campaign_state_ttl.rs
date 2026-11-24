//! Tests for the permissionless campaign-state TTL keep-alive (#1461).

#![allow(deprecated)]

extern crate std;

use super::helpers::*;
use crate::{CampaignKey, ContributionKey, VotingKey};
use soroban_sdk::testutils::storage::Persistent;
use soroban_sdk::{IntoVal, Val};

#[test]
fn bump_campaign_state_extends_every_campaign_scoped_key() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let campaign_id = 42;
    let keys: [Val; 12] = [
        CampaignKey::Campaign(campaign_id).into_val(&env),
        CampaignKey::CampaignStartTime(campaign_id).into_val(&env),
        CampaignKey::CampaignCreatorIndex(campaign_id).into_val(&env),
        CampaignKey::CampaignMilestones(campaign_id).into_val(&env),
        CampaignKey::CampaignReserve(campaign_id).into_val(&env),
        CampaignKey::CampaignToken(campaign_id).into_val(&env),
        ContributionKey::ContributorCount(campaign_id).into_val(&env),
        VotingKey::ApproveVotes(campaign_id).into_val(&env),
        VotingKey::RejectVotes(campaign_id).into_val(&env),
        VotingKey::ApproveWeight(campaign_id).into_val(&env),
        VotingKey::RejectWeight(campaign_id).into_val(&env),
        VotingKey::VoteSnapshotLedger(campaign_id).into_val(&env),
    ];

    let mut ledger = env.ledger().get();
    ledger.max_entry_ttl = 10_000_000;
    ledger.min_persistent_entry_ttl = 10;
    env.ledger().set(ledger);

    env.as_contract(&client.address, || {
        let persistent = env.storage().persistent();
        for key in &keys {
            persistent.set(key, &1u32);
            persistent.extend_ttl(key, 0, 6_912_000);
        }
    });

    env.ledger()
        .with_mut(|ledger| ledger.sequence_number += 395 * 17_280);
    let near_expiry = env.as_contract(&client.address, || {
        let persistent = env.storage().persistent();
        keys.iter()
            .map(|key| persistent.get_ttl(key))
            .collect::<std::vec::Vec<_>>()
    });
    assert!(near_expiry.iter().all(|ttl| *ttl > 0));

    client.bump_campaign_state(&campaign_id);

    env.as_contract(&client.address, || {
        let persistent = env.storage().persistent();
        for (key, old_ttl) in keys.iter().zip(&near_expiry) {
            assert!(persistent.get_ttl(key) > *old_ttl);
        }
    });
}

#[test]
fn bump_campaign_state_for_missing_id_is_a_no_op() {
    let (env, _admin, _creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let missing_id = u32::MAX;

    client.bump_campaign_state(&missing_id);

    env.as_contract(&client.address, || {
        assert!(!env
            .storage()
            .persistent()
            .has(&CampaignKey::Campaign(missing_id)));
        assert!(!env
            .storage()
            .persistent()
            .has(&CampaignKey::CampaignStartTime(missing_id)));
        assert!(!env
            .storage()
            .persistent()
            .has(&VotingKey::ApproveVotes(missing_id)));
    });
}
