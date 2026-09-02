//! Tests for the admin last-resort emergency withdrawal (#802).

use super::helpers::*;
use crate::{Category, EmergencyWithdrawal, Error, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS};
use soroban_sdk::testutils::{Events, Ledger};
use soroban_sdk::{Address, String, TryFromVal};

/// Creates a verified campaign that has met its funding goal and returns its id
/// plus the amount escrowed.
fn goal_met_campaign(
    env: &Env,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
    contributor: &Address,
    token_admin: &TokenAdminClient<'_>,
) -> (u32, i128) {
    let goal: i128 = 1_000;
    token_admin.mint(contributor, &goal);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Misconfigured Campaign"),
        String::from_str(env, "Creator key is dead; funds are locked"),
        goal,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, contributor, &goal);
    (campaign_id, goal)
}

fn advance(env: &Env, secs: u64) {
    let now = env.ledger().timestamp();
    env.ledger().with_mut(|li| li.timestamp = now + secs);
}

#[test]
fn test_emergency_withdraw_full_happy_path() {
    let (env, admin, creator, contributor, _c2, token, token_admin, client) = setup_env();
    let (campaign_id, goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    let global_before = client.get_total_raised_global();

    client.emergency_withdraw(&admin, &campaign_id, &recipient);

    // Timelock still active: execution is refused.
    let res = client.try_execute_emergency_withdrawal(&admin, &campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS);
    client.execute_emergency_withdrawal(&admin, &campaign_id);

    assert_eq!(token.balance(&recipient), goal);

    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.funds_withdrawn);
    assert!(!campaign.is_active);
    assert_eq!(campaign.effective_amount_raised, 0);
    // Audit total is preserved.
    assert_eq!(campaign.amount_raised, goal);

    assert_eq!(client.get_total_raised_global(), global_before - goal);
    assert_eq!(client.get_emergency_withdrawal(&campaign_id), None);
}

#[test]
fn test_emergency_withdraw_timelock_boundary() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    let pending = client.get_emergency_withdrawal(&campaign_id).unwrap();

    // One second before `execute_after` — still locked.
    env.ledger()
        .with_mut(|li| li.timestamp = pending.execute_after - 1);
    assert_eq!(
        client
            .try_execute_emergency_withdrawal(&admin, &campaign_id)
            .unwrap_err()
            .unwrap(),
        Error::ValidationFailed
    );

    // Exactly at `execute_after` — allowed.
    env.ledger()
        .with_mut(|li| li.timestamp = pending.execute_after);
    client.execute_emergency_withdrawal(&admin, &campaign_id);
}

#[test]
fn test_emergency_withdraw_records_pending_proposal() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    let proposed_at = env.ledger().timestamp();
    client.emergency_withdraw(&admin, &campaign_id, &recipient);

    let expected = EmergencyWithdrawal {
        recipient: recipient.clone(),
        proposed_at,
        execute_after: proposed_at + EMERGENCY_WITHDRAWAL_TIMELOCK_SECS,
    };
    assert_eq!(
        client.get_emergency_withdrawal(&campaign_id),
        Some(expected)
    );
    let _ = goal;
}

#[test]
fn test_emergency_withdraw_requires_admin() {
    let (env, _admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let impostor = Address::generate(&env);
    let recipient = Address::generate(&env);

    assert_eq!(
        client
            .try_emergency_withdraw(&impostor, &campaign_id, &recipient)
            .unwrap_err()
            .unwrap(),
        Error::NotAuthorized
    );
}

#[test]
fn test_execute_requires_admin() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);
    let impostor = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS);

    assert_eq!(
        client
            .try_execute_emergency_withdrawal(&impostor, &campaign_id)
            .unwrap_err()
            .unwrap(),
        Error::NotAuthorized
    );
}

#[test]
fn test_emergency_withdraw_double_propose_rejected() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    assert_eq!(
        client
            .try_emergency_withdraw(&admin, &campaign_id, &recipient)
            .unwrap_err()
            .unwrap(),
        Error::ValidationFailed
    );
}

#[test]
fn test_cancel_emergency_withdrawal_then_repropose_restarts_timelock() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS / 2);
    client.cancel_emergency_withdrawal(&admin, &campaign_id);
    assert_eq!(client.get_emergency_withdrawal(&campaign_id), None);

    // Re-propose: the timelock is measured from the new proposal.
    let reproposed_at = env.ledger().timestamp();
    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    let pending = client.get_emergency_withdrawal(&campaign_id).unwrap();
    assert_eq!(
        pending.execute_after,
        reproposed_at + EMERGENCY_WITHDRAWAL_TIMELOCK_SECS
    );

    // The half-window that elapsed under the first proposal does not count.
    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS / 2);
    assert_eq!(
        client
            .try_execute_emergency_withdrawal(&admin, &campaign_id)
            .unwrap_err()
            .unwrap(),
        Error::ValidationFailed
    );
}

#[test]
fn test_cancel_without_proposal_errors() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);

    assert_eq!(
        client
            .try_cancel_emergency_withdrawal(&admin, &campaign_id)
            .unwrap_err()
            .unwrap(),
        Error::ValidationFailed
    );
}

#[test]
fn test_execute_without_proposal_errors() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);

    assert_eq!(
        client
            .try_execute_emergency_withdrawal(&admin, &campaign_id)
            .unwrap_err()
            .unwrap(),
        Error::ValidationFailed
    );
}

#[test]
fn test_emergency_withdraw_rejects_campaign_below_goal() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Underfunded"),
        String::from_str(&env, "Never met its goal — refundable, not locked"),
        10_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor, &500);
    let recipient = Address::generate(&env);

    assert_eq!(
        client
            .try_emergency_withdraw(&admin, &campaign_id, &recipient)
            .unwrap_err()
            .unwrap(),
        Error::FundingGoalNotReached
    );
}

#[test]
fn test_emergency_withdraw_rejects_cancelled_campaign() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);

    client.admin_cancel_campaign(&admin, &campaign_id, &String::from_str(&env, "fraud"));
    let recipient = Address::generate(&env);

    assert_eq!(
        client
            .try_emergency_withdraw(&admin, &campaign_id, &recipient)
            .unwrap_err()
            .unwrap(),
        Error::CampaignNotActive
    );
}

#[test]
fn test_emergency_withdraw_rejects_already_withdrawn_campaign() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);

    client.withdraw_funds(&campaign_id);
    let recipient = Address::generate(&env);

    assert_eq!(
        client
            .try_emergency_withdraw(&admin, &campaign_id, &recipient)
            .unwrap_err()
            .unwrap(),
        Error::FundsAlreadyWithdrawn
    );
}

#[test]
fn test_execute_clears_stale_proposal_when_creator_withdrew_meanwhile() {
    let (env, admin, creator, contributor, _c2, token, token_admin, client) = setup_env();
    let (campaign_id, goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);

    // The creator key turns out to be fine after all and withdraws normally.
    client.withdraw_funds(&campaign_id);

    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS);
    let res = client.try_execute_emergency_withdrawal(&admin, &campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundsAlreadyWithdrawn);

    // Stale proposal is cleared, recipient got nothing.
    assert_eq!(client.get_emergency_withdrawal(&campaign_id), None);
    assert_eq!(token.balance(&recipient), 0);
    let _ = goal;
}

#[test]
fn test_emergency_withdrawal_executed_event_payload() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS);
    client.execute_emergency_withdrawal(&admin, &campaign_id);

    let found = env.events().all().iter().any(|event| {
        let topics = &event.1;
        if topics.len() < 3 {
            return false;
        }
        let topic0 = soroban_sdk::String::try_from_val(&env, &topics.get(0).unwrap()).ok();
        if topic0
            != Some(soroban_sdk::String::from_str(
                &env,
                "emergency_withdrawal_executed",
            ))
        {
            return false;
        }
        let data: (Address, i128) = soroban_sdk::FromVal::from_val(&env, &event.2);
        data == (recipient.clone(), goal)
    });
    assert!(found, "expected emergency_withdrawal_executed event");
}

#[test]
fn test_emergency_withdrawn_campaign_not_refundable() {
    let (env, admin, creator, contributor, _c2, _token, token_admin, client) = setup_env();
    let (campaign_id, _goal) =
        goal_met_campaign(&env, &client, &creator, &contributor, &token_admin);
    let recipient = Address::generate(&env);

    client.emergency_withdraw(&admin, &campaign_id, &recipient);
    advance(&env, EMERGENCY_WITHDRAWAL_TIMELOCK_SECS);
    client.execute_emergency_withdrawal(&admin, &campaign_id);

    // Goal was met and the campaign is not cancelled, so contributors were
    // never on the refund path — nothing changes here, but assert it explicitly.
    let res = client.try_claim_refund(&campaign_id, &contributor);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // And the normal creator withdrawal is now closed too.
    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::FundsAlreadyWithdrawn);
}
