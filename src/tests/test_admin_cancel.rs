extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams, Error};
use soroban_sdk::{Address, String};

// ── admin_cancel_campaign (#508, #858) ──────────────────────────────────────

fn make_campaign(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    creator: &Address,
    goal: i128,
    index: u32,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, &format!("Fraud Suspect {index}")),
        description: String::from_str(env, &format!("Reported for fraud {index}")),
        funding_goal: goal,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    })
}

#[test]
fn test_admin_cancel_campaign_rejects_non_admin() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &client, &creator, 1000, 0);
    client.verify_campaign(&campaign_id);

    let impostor = Address::generate(&env);
    let res =
        client.try_admin_cancel_campaign(&impostor, &campaign_id, &String::from_str(&env, "fraud"));
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_admin_cancel_campaign_succeeds_after_goal_met() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let goal = 1000i128;
    token_admin.mint(&contributor1, &goal);

    let campaign_id = make_campaign(&env, &client, &creator, goal, 0);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &goal);

    // Creator self-cancel would be rejected here (goal met); admin can still act.
    let self_cancel = client.try_cancel_campaign(&campaign_id);
    assert_eq!(self_cancel, Err(Ok(Error::GoalMetCancellationNotAllowed)));

    client.admin_cancel_campaign(
        &admin,
        &campaign_id,
        &String::from_str(&env, "fraud reported"),
    );
    assert!(client.get_campaign(&campaign_id).is_cancelled);
    assert!(!client.get_campaign(&campaign_id).is_active);
}

#[test]
fn test_admin_cancel_campaign_succeeds_on_unverified_campaign() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &client, &creator, 1000, 0);

    client.admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_verified);
}

#[test]
fn test_admin_cancel_campaign_rejected_after_withdrawal() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let goal = 500i128;
    token_admin.mint(&contributor1, &goal);

    let campaign_id = make_campaign(&env, &client, &creator, goal, 0);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &goal);
    client.withdraw_funds(&campaign_id);

    // `require_active_campaign` fires first — `withdraw_funds` already
    // clears `is_active`, matching `cancel_campaign`'s own guard ordering.
    let res =
        client.try_admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "too late"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_admin_cancel_campaign_rejects_empty_and_oversized_reason() {
    extern crate std;
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &client, &creator, 1000, 0);
    client.verify_campaign(&campaign_id);

    let empty = client.try_admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, ""));
    assert_eq!(empty.unwrap_err().unwrap(), Error::ValidationFailed);

    let too_long_reason = "a".repeat(1001);
    let too_long = client.try_admin_cancel_campaign(
        &admin,
        &campaign_id,
        &String::from_str(&env, &too_long_reason),
    );
    assert_eq!(too_long.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_admin_cancel_campaign_emits_revenue_pool_in_event() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let goal = 1000i128;
    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &5000);

    let campaign_id = make_campaign(&env, &client, &creator, goal, 0);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &800);

    // Simulate revenue deposit directly via storage: deposit_revenue requires
    // has_revenue_sharing=true and funds_withdrawn=true, which would cause a
    // non-unwinding panic (SIGABRT) if we called the contract method.
    env.as_contract(&client.address, || {
        crate::storage::set_revenue_pool(&env, campaign_id, 3000);
    });

    assert_eq!(client.get_revenue_pool(&campaign_id), 3000);

    let reason = String::from_str(&env, "fraud with revenue");
    client.admin_cancel_campaign(&admin, &campaign_id, &reason);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let payload: (Address, String, i128, i128) =
        soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(payload.0, creator);
    assert_eq!(payload.1, reason);
    assert_eq!(payload.2, 800); // effective_amount_raised
    assert_eq!(payload.3, 3000); // orphaned revenue pool
}

#[test]
fn test_admin_cancel_campaign_rejected_while_paused() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &client, &creator, 1000, 0);
    client.verify_campaign(&campaign_id);
    client.pause();

    let res =
        client.try_admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}

#[test]
fn test_admin_cancel_campaign_allows_contributor_refund() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();
    let goal = 1000i128;
    token_admin.mint(&contributor1, &2000);

    let campaign_id = make_campaign(&env, &client, &creator, goal, 0);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &600);

    client.admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));

    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2000);
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
}

#[test]
fn test_admin_cancel_campaign_refunds_revenue_pool_and_zeroes_pool() {
    let (env, admin, creator, _, _, token, token_admin, client) = setup_env();
    let campaign_id = make_campaign(&env, &client, &creator, 1000);
    client.verify_campaign(&campaign_id);

    // Simulate revenue deposited in revenue pool and contract balance
    let rev_amount = 500i128;
    token_admin.mint(&env.current_contract_address(), &rev_amount);
    storage::set_revenue_pool(&env, campaign_id, rev_amount);

    let creator_balance_before = token.balance(&creator);
    assert_eq!(storage::get_revenue_pool(&env, campaign_id), rev_amount);

    client.admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));

    // Revenue pool is refunded to creator and zeroed in storage (#858)
    assert_eq!(storage::get_revenue_pool(&env, campaign_id), 0);
    assert_eq!(token.balance(&creator), creator_balance_before + rev_amount);
}

#[test]
fn test_admin_cancel_campaign_emits_event_with_reason() {
    let (env, admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let goal = 1000i128;
    token_admin.mint(&contributor1, &goal);

    let campaign_id = make_campaign(&env, &client, &creator, goal, 0);
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &600);

    let reason = String::from_str(&env, "confirmed fraudulent activity");
    client.admin_cancel_campaign(&admin, &campaign_id, &reason);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    let payload: (Address, String, i128, i128) =
        soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(payload.0, creator);
    assert_eq!(payload.1, reason);
    // effective_amount_raised equals amount_raised (600) since no refunds
    // have been claimed yet.
    assert_eq!(payload.2, 600);
    // No revenue was deposited, so pool should be zero.
    assert_eq!(payload.3, 0);
}
