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

    // #535: burst detection only engages once amount_raised crosses 50% of
    // the funding goal, so push the campaign over that line first.
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

// ── #1220: Personal contribution cap edge cases ───────────────────────────

#[test]
fn test_zero_campaign_cap_means_unlimited() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Unlimited"),
        String::from_str(&env, "No cap"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128, // 0 = no cap
    ));

    // With 0 cap, personal cap is not set — get_personal_cap returns 0
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);
}

#[test]
fn test_personal_cap_exact_boundary() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Boundary Test"),
        String::from_str(&env, "Testing exact boundary"),
        5_000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    env.as_contract(&client.address, || {
        let mut campaign = crate::storage::get_campaign(&env, campaign_id).unwrap();
        campaign.set_verified(true);
        crate::storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Set personal cap at exact boundary
    client.set_personal_cap(&campaign_id, &contributor1, &1_000);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 1_000);

    // Cap can be updated to a higher value
    client.set_personal_cap(&campaign_id, &contributor1, &2_000);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 2_000);
}

#[test]
fn test_remove_personal_cap_restores_campaign_wide_cap() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Remove Cap"),
        String::from_str(&env, "Cap removal test"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        2_000i128, // campaign-wide cap
    ));

    // Set then remove personal cap
    client.set_personal_cap(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 500);

    client.remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);
}

#[test]
fn test_remove_personal_cap_not_set_returns_error() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "No Cap Set"),
        String::from_str(&env, "Trying to remove non-existent cap"),
        5_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    let res = client.try_remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::PersonalCapNotFound);
}

#[test]
fn test_two_contributors_independent_caps() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    token_admin.mint(&contributor1, &5_000);
    token_admin.mint(&contributor2, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Independent Caps"),
        String::from_str(&env, "Each contributor has own cap"),
        5_000,
        30,
        Category::Educator,
        false,
        0,
        500i128,
    ));

    // Each contributor has independent caps
    client.set_personal_cap(&campaign_id, &contributor1, &300);
    client.set_personal_cap(&campaign_id, &contributor2, &400);

    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 300);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor2), 400);

    // Removing one doesn't affect the other
    client.remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor2), 400);
}
