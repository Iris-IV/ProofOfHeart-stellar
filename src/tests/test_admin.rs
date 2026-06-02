use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::String;

#[test]
fn test_update_platform_fee() {
    let (env, _admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let result = client.try_update_platform_fee(&500);
    assert!(result.is_ok());

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let expected_topics = (String::from_str(&env, "fee_updated"),).into_val(&env);
    assert_eq!(last_event.1, expected_topics);

    let data_vec: soroban_sdk::Vec<u32> = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data_vec.get(0).unwrap(), 300);
    assert_eq!(data_vec.get(1).unwrap(), 500);

    // Issue #343: fees above the cap are rejected, not silently clamped.
    let result = client.try_update_platform_fee(&5000);
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidPlatformFee);
}

#[test]
fn test_pause_and_unpause() {
    let (_env, _admin, _creator, _contributor1, _, _token, _token_admin, client) = setup_env();

    assert!(!client.is_paused());

    client.pause();
    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn test_pause_blocks_state_changing_operations() {
    let (env, _admin, creator, contributor1, _contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Paused Test"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.pause();
    assert!(client.is_paused());

    let res = client.try_create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "New Campaign"),
        String::from_str(&env, "Testing pause functionality"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_contribute(&campaign_id, &contributor1, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_vote_on_campaign(&campaign_id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_verify_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let res = client.try_update_platform_fee(&400);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.title, String::from_str(&env, "Paused Test"));

    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());

    client.contribute(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);

    let _ = token;
}

// ── Issue #407: accept_token_update must not strand campaign balances ──────────

#[test]
fn test_token_swap_blocked_with_active_campaign() {
    let (env, admin, creator, _, _, _, token_admin, client) = setup_env();

    let _campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Active Campaign"),
        String::from_str(&env, "Token swap must be blocked"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));

    // Register a second token to use as the migration target.
    let new_token_address = env.register_stellar_asset_contract(admin.clone());

    client.propose_token_update(&admin, &new_token_address);

    // Advance timestamp past the 7-day delay.
    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    // Must fail: there is still an active campaign with escrowed funds.
    let res = client.try_accept_token_update(&admin);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Token must remain unchanged.
    let _ = token_admin;
}

#[test]
fn test_token_swap_succeeds_after_all_campaigns_terminal() {
    let (env, admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Terminal Campaign"),
        String::from_str(&env, "Withdraw before swap"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);

    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    // All campaigns terminal (withdrawn) → swap must succeed.
    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());

    assert_eq!(client.get_token(), new_token_address);
}

#[test]
fn test_token_swap_succeeds_after_campaign_cancelled() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancellable"),
        String::from_str(&env, "Cancel then swap"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.cancel_campaign(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);

    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    let res = client.try_accept_token_update(&admin);
    assert!(res.is_ok());
    assert_eq!(client.get_token(), new_token_address);
}

// ── Issue #407 follow-up: cancelling a campaign drops the active-campaign count
//    to zero, but contributor refunds remain escrowed in the old token until
//    claimed. The swap must stay blocked until those funds actually leave. ──────
#[test]
fn test_token_swap_blocked_with_unrefunded_cancelled_campaign() {
    let (env, admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Cancel With Funds"),
        String::from_str(&env, "Refund pending after cancel"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Cancel: ActiveCampaignCount → 0, but the 500 is still escrowed in the
    // old token pending claim_refund.
    client.cancel_campaign(&campaign_id);

    let new_token_address = env.register_stellar_asset_contract(admin.clone());
    client.propose_token_update(&admin, &new_token_address);
    env.ledger().with_mut(|l| {
        l.timestamp += 7 * 86400 + 1;
    });

    // Must still be blocked: outstanding balance remains in the old token.
    let res = client.try_accept_token_update(&admin);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Once the contributor claims their refund, no old-token escrow remains and
    // the swap can proceed.
    client.claim_refund(&campaign_id, &contributor1);
    let res2 = client.try_accept_token_update(&admin);
    assert!(res2.is_ok());
    assert_eq!(client.get_token(), new_token_address);
}
