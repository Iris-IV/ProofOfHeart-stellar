use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::String;

#[test]
fn test_contribution_cap_persists_across_refund_recontribution_cycles() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Cap persistence"),
        String::from_str(&env, "lifetime cap test"), 2_000, 1,
        Category::Learner, false, 0, 1_000i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &900);
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(client.get_lifetime_contribution(&campaign_id, &contributor1), 900);
}

#[test]
fn test_personal_cap_enforcement() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Cap Test"),
        String::from_str(&env, "Testing caps"), 5000, 30,
        Category::Educator, false, 0, 1000i128,
    ));
    client.verify_campaign(&campaign_id);

    client.set_personal_cap(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 500);

    client.contribute(&campaign_id, &contributor1, &400);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    client.set_personal_cap(&campaign_id, &contributor1, &2000);
    client.contribute(&campaign_id, &contributor1, &500);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
}

#[test]
fn test_anomaly_auto_pause_huge_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Science Book"),
        String::from_str(&env, "Teaching science to kids"), 2000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);

    let res = client.try_contribute(&campaign_id, &contributor1, &4001);
    assert!(res.is_ok());
    assert!(client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &100);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 100);
}

#[test]
fn test_anomaly_auto_pause_burst() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Burst Test"),
        String::from_str(&env, "Testing burst"), 2000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);

    for _ in 0..10 {
        client.contribute(&campaign_id, &contributor1, &10);
    }
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 100);

    let res = client.try_contribute(&campaign_id, &contributor1, &10);
    assert!(res.is_ok());
    assert!(client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 100);

    client.unpause();

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: env.ledger().timestamp(),
        protocol_version: 22,
        sequence_number: env.ledger().sequence() + 1,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10,
    });

    client.contribute(&campaign_id, &contributor1, &10);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 110);
}
