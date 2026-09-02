use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::String;

#[test]
fn test_contribution_cap_persists_across_refund_recontribution_cycles() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap persistence"),
        String::from_str(&env, "lifetime cap test"),
        2_000,
        1,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &900);
    client.cancel_campaign(&campaign_id);
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        900
    );
}

#[test]
fn test_max_contribution_per_user_enforced_across_multiple_transactions() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Multi tx cap"),
        String::from_str(&env, "lifetime cap across txs"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        1_000i128,
    ));
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &600);
    let res = client.try_contribute(&campaign_id, &contributor1, &600);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 600);
    assert_eq!(
        client.get_lifetime_contribution(&campaign_id, &contributor1),
        600
    );
}

#[test]
fn test_personal_cap_enforcement() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cap Test"),
        String::from_str(&env, "Testing caps"),
        5000,
        30,
        Category::Educator,
        false,
        0,
        1000i128,
    ));
    client.verify_campaign(&campaign_id);

    client.set_personal_cap(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 500);

    client.contribute(&campaign_id, &contributor1, &400);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);

    let res_set = client.try_set_personal_cap(&campaign_id, &contributor1, &2000);
    assert_eq!(res_set.unwrap_err().unwrap(), Error::ValidationFailed);

    client.set_personal_cap(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor1, &500);
    let res = client.try_contribute(&campaign_id, &contributor1, &200);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContributionCapExceeded);
}

#[test]
fn test_anomaly_rejects_huge_contribution() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Science Book"),
        String::from_str(&env, "Teaching science to kids"),
        2000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    let res = client.try_contribute(&campaign_id, &contributor1, &4001);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
    // Rejected, not paused: no code path sets AutoPaused.
    assert!(!client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &100);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 100);
}

#[test]
fn test_anomaly_rejects_burst() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Burst Test"),
        String::from_str(&env, "Testing burst"),
        20, // Goal low enough that contributions exceed 50% quickly
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // #535: burst detection only engages once amount_raised crosses 50% of the
    // funding goal, so push the campaign over that line first. This single
    // contribution itself isn't burst-checked (amount_raised is still 0 going
    // into it).
    client.contribute(&campaign_id, &contributor1, &1_100);

    for _ in 0..10 {
        client.contribute(&campaign_id, &contributor1, &100);
    }
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_200);

    // The 11th contribution should push block_count to 11 > AUTO_PAUSE_BURST_THRESHOLD (10).
    let res = client.try_contribute(&campaign_id, &contributor1, &10);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
    // Rejected, not paused: no code path sets AutoPaused.
    assert!(!client.is_paused());
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_200);

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
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 1_210);
}

#[test]
fn test_huge_contribution_is_rejected() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Huge Contribution Test"),
        description: String::from_str(&env, "Testing auto-pause via huge contribution"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    // Anomaly detection fires and rejects the transaction. It does not pause
    // the contract: a rejected Soroban invocation rolls back its own writes.
    let res = client.try_contribute(&campaign_id, &contributor1, &2001i128);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}
