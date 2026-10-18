//! # Emergency withdrawal — admin last-resort fund recovery (#802)
//!
//! **This is a last-resort function.** It exists only to recover contributions
//! that are otherwise permanently locked because a campaign was misconfigured
//! — the canonical case being a creator address that is a dead or locked
//! account, so `withdraw_funds` (which pays the creator) can never succeed and
//! the escrowed tokens have no exit path.
//!
//! It is deliberately slow and loud:
//!
//! * **Two steps.** `emergency_withdraw` only *proposes*; funds move only when
//!   a separate `execute_emergency_withdrawal` call is made afterwards.
//! * **Mandatory 7-day timelock.** `execute_emergency_withdrawal` reverts with
//!   `ValidationFailed` until `EMERGENCY_WITHDRAWAL_TIMELOCK_SECS` has
//!   elapsed since the proposal. The
//!   delay is not admin-configurable.
//! * **Prominent events.** `emergency_withdrawal_proposed`,
//!   `emergency_withdrawal_cancelled` and `emergency_withdrawal_executed` are
//!   emitted with the campaign id, the acting admin, the recipient and the
//!   amount so indexers and watchers can alarm on them.
//!
//! Scope: this recovers the escrowed contribution principal
//! (`effective_amount_raised`) of a campaign that **met its funding goal** but
//! can no longer pay it out. That is the only state in which contributions are
//! genuinely locked — a campaign that never reached its goal lets every
//! contributor self-serve a refund via `claim_refund` after the deadline, and
//! a cancelled campaign does the same immediately, so those are refused here
//! (`FundingGoalNotReached` / `CampaignNotActive`) and left to the existing
//! refund path. It also does **not** touch a campaign's revenue-sharing pool —
//! contributors keep claiming revenue via `claim_revenue` as normal.

use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::lifecycle::{assert_admin, campaign_token_client, get_campaign_or_error};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_campaign_payout_marker,
    get_emergency_withdrawal, get_total_raised_global, remove_emergency_withdrawal, set_campaign,
    set_campaign_payout_marker, set_emergency_withdrawal, set_total_raised_global,
};
use crate::types::EmergencyWithdrawal;

/// Proposes an emergency withdrawal of a campaign's escrowed funds to
/// `recipient`, starting the mandatory timelock (#802).
///
/// Moves no funds. Emits `emergency_withdrawal_proposed`. The proposal must
/// then sit for `EMERGENCY_WITHDRAWAL_TIMELOCK_SECS` before
/// `execute_emergency_withdrawal` will run.
///
/// # Errors
/// * `NotAuthorized` — caller is not the stored admin.
/// * `CampaignNotFound` — no campaign with that id.
/// * `CampaignNotActive` — the campaign is already cancelled.
/// * `FundsAlreadyWithdrawn` — funds have already left the escrow.
/// * `FundingGoalNotReached` — the goal was never met, so contributors can
///   refund themselves; emergency recovery does not apply.
/// * `NoFundsToWithdraw` — the campaign holds no escrowed principal.
/// * `ValidationFailed` — a proposal is already pending (cancel it first),
///   or another precondition was violated.
pub(crate) fn emergency_withdraw(
    env: &Env,
    admin: Address,
    campaign_id: u32,
    recipient: Address,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if recipient == env.current_contract_address() {
        return Err(Error::ValidationFailed);
    }

    let campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.is_cancelled() {
        return Err(Error::CampaignNotActive);
    }
    if campaign.funds_withdrawn() || get_campaign_payout_marker(env, campaign_id) == Some(1) {
        return Err(Error::FundsAlreadyWithdrawn);
    }
    // Only funds that reached the goal are truly locked: a failed campaign is
    // refundable via `claim_refund`. Using `amount_raised` (the monotonic
    // audit total, never decremented by refunds) matches the goal-met check in
    // `cancel_campaign`.
    if campaign.amount_raised < campaign.funding_goal {
        return Err(Error::FundingGoalNotReached);
    }
    if campaign.effective_amount_raised <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    if get_emergency_withdrawal(env, campaign_id).is_some() {
        return Err(Error::ValidationFailed);
    }

    let proposed_at = env.ledger().timestamp();
    let execute_after = proposed_at
        .checked_add(crate::EMERGENCY_WITHDRAWAL_TIMELOCK_SECS)
        .ok_or(Error::Overflow)?;

    bump_instance_ttl(env);
    set_emergency_withdrawal(
        env,
        campaign_id,
        &EmergencyWithdrawal {
            recipient: recipient.clone(),
            proposed_at,
            execute_after,
        },
    );

    env.events().publish(
        ("emergency_withdrawal_proposed", campaign_id, admin),
        (recipient, execute_after, campaign.effective_amount_raised),
    );

    Ok(())
}

/// Cancels a pending emergency withdrawal before it is executed (#802).
///
/// Emits `emergency_withdrawal_cancelled`. Cancelling and re-proposing
/// restarts the timelock from zero.
///
/// # Errors
/// * `NotAuthorized` — caller is not the stored admin.
/// * `ValidationFailed` — nothing is pending for this campaign.
pub(crate) fn cancel_emergency_withdrawal(
    env: &Env,
    admin: Address,
    campaign_id: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;

    let pending = get_emergency_withdrawal(env, campaign_id).ok_or(Error::ValidationFailed)?;

    bump_instance_ttl(env);
    remove_emergency_withdrawal(env, campaign_id);

    env.events().publish(
        ("emergency_withdrawal_cancelled", campaign_id, admin),
        pending.recipient,
    );

    Ok(())
}

/// Executes a pending emergency withdrawal once its timelock has elapsed,
/// transferring the campaign's escrowed principal to the recipient recorded
/// at proposal time (#802).
///
/// Marks the campaign withdrawn and inactive, so no other payout path can
/// double-spend the recovered funds. Emits `emergency_withdrawal_executed`.
///
/// # Errors
/// * `NotAuthorized` — caller is not the stored admin.
/// * `ValidationFailed` — nothing is pending for this campaign.
/// * `ValidationFailed` — nothing is pending, or the 7-day timelock has not
///   elapsed.
/// * `CampaignNotFound` — the campaign disappeared (should not happen).
/// * `CampaignNotActive` / `FundsAlreadyWithdrawn` — the campaign reached a
///   terminal state after the proposal; the stale proposal is cleared.
/// * `NoFundsToWithdraw` — nothing left to recover; the stale proposal is
///   cleared.
pub(crate) fn execute_emergency_withdrawal(
    env: &Env,
    admin: Address,
    campaign_id: u32,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;

    let pending = get_emergency_withdrawal(env, campaign_id).ok_or(Error::ValidationFailed)?;

    if env.ledger().timestamp() < pending.execute_after {
        return Err(Error::ValidationFailed);
    }
    if pending.recipient == env.current_contract_address() {
        remove_emergency_withdrawal(env, campaign_id);
        return Err(Error::ValidationFailed);
    }

    let mut campaign = get_campaign_or_error(env, campaign_id)?;

    // The campaign could have been cancelled or withdrawn during the timelock
    // window (e.g. a rescued creator key withdrew normally). Clear the now
    // meaningless proposal rather than leaving it dangling.
    if campaign.is_cancelled() {
        remove_emergency_withdrawal(env, campaign_id);
        return Err(Error::CampaignNotActive);
    }
    if campaign.funds_withdrawn() || get_campaign_payout_marker(env, campaign_id) == Some(1) {
        remove_emergency_withdrawal(env, campaign_id);
        return Err(Error::FundsAlreadyWithdrawn);
    }

    let amount = campaign.effective_amount_raised;
    if amount <= 0 {
        remove_emergency_withdrawal(env, campaign_id);
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // State first (CEI): a malicious token contract must not be able to
    // re-enter and drain the escrow twice. After this, the campaign is
    // withdrawn+inactive and holds no escrowed principal.
    campaign.set_funds_withdrawn(true);
    campaign.set_active(false);
    campaign.effective_amount_raised = 0;
    set_campaign(env, campaign_id, &campaign);
    set_campaign_payout_marker(env, campaign_id, 1);
    decrement_active_campaign_count(env);
    remove_emergency_withdrawal(env, campaign_id);

    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised.checked_sub(amount).ok_or(Error::Overflow)?,
    );

    // Interaction last.
    let client = campaign_token_client(env, campaign_id);
    client.transfer(&env.current_contract_address(), &pending.recipient, &amount);

    env.events().publish(
        ("emergency_withdrawal_executed", campaign_id, admin),
        (pending.recipient, amount),
    );

    Ok(())
}

/// Rescues stranded foreign tokens accidentally sent to the contract (#1037).
///
/// Only non-escrow tokens may be rescued: `token` must differ from the
/// platform token and must not be on the campaign-currency allowlist
/// (allowlisted tokens may be live campaign escrow). Contributor funds are
/// never reachable via this path. Reuses `ValidationFailed` (error enum is
/// at the 50-case XDR cap, so no new variant is introduced).
pub(crate) fn rescue_stranded_tokens(
    env: &Env,
    admin: Address,
    token: Address,
    to: Address,
    amount: i128,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    if amount <= 0 || to == env.current_contract_address() {
        return Err(Error::ValidationFailed);
    }
    if token == crate::storage::get_token(env) {
        return Err(Error::ValidationFailed);
    }
    if crate::storage::is_token_allowed(env, &token) {
        return Err(Error::ValidationFailed);
    }
    bump_instance_ttl(env);
    soroban_sdk::token::Client::new(env, &token).transfer(
        &env.current_contract_address(),
        &to,
        &amount,
    );
    env.events()
        .publish(("stranded_tokens_rescued", token, to), amount);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::errors::Error;
    use crate::tests::helpers::*;
    use crate::Category;
    use soroban_sdk::{testutils::Address as _, testutils::Ledger, Vec};

    #[test]
    fn test_emergency_pause_signer_cannot_bypass_admin_on_emergency_or_unpause_flows() {
        let (env, admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
        let pause_signer = Address::generate(&env);
        let recipient = Address::generate(&env);

        let id = client.create_campaign(&make_params(
            creator,
            String::from_str(&env, "Emergency Auth Audit"),
            String::from_str(&env, "Description"),
            1_000,
            10,
            Category::Learner,
            false,
            0,
            0,
        ));
        client.verify_campaign(&id);
        token_admin.mint(&c1, &1_000);
        client.contribute(&id, &c1, &1_000);

        let mut signers = Vec::new(&env);
        signers.push_back(pause_signer.clone());
        client.set_emergency_pause_signers(&admin, &signers);

        // Emergency pause signer is authorized to pause the contract.
        client.emergency_pause(&pause_signer);
        assert!(client.is_paused());

        // Emergency pause signer must NOT be able to propose, cancel, or execute emergency withdrawals,
        // nor rescue stranded tokens.
        assert_eq!(
            client.try_emergency_withdraw(&pause_signer, &id, &recipient),
            Err(Ok(Error::NotAuthorized))
        );
        client.emergency_withdraw(&admin, &id, &recipient);
        assert_eq!(
            client.try_cancel_emergency_withdrawal(&pause_signer, &id),
            Err(Ok(Error::NotAuthorized))
        );

        let now = env.ledger().timestamp();
        env.ledger()
            .set_timestamp(now + crate::EMERGENCY_WITHDRAWAL_TIMELOCK_SECS + 1);
        assert_eq!(
            client.try_execute_emergency_withdrawal(&pause_signer, &id),
            Err(Ok(Error::NotAuthorized))
        );

        let foreign_token = Address::generate(&env);
        assert_eq!(
            client.try_rescue_stranded_tokens(&pause_signer, &foreign_token, &recipient, &100),
            Err(Ok(Error::NotAuthorized))
        );
    }

    #[test]
    fn test_emergency_withdraw_and_rescue_reject_contract_self_recipient() {
        let (env, admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
        let id = client.create_campaign(&make_params(
            creator,
            String::from_str(&env, "Self Recipient Guard"),
            String::from_str(&env, "Description"),
            1_000,
            10,
            Category::Learner,
            false,
            0,
            0,
        ));
        client.verify_campaign(&id);
        token_admin.mint(&c1, &1_000);
        client.contribute(&id, &c1, &1_000);

        assert_eq!(
            client.try_emergency_withdraw(&admin, &id, &client.address),
            Err(Ok(Error::ValidationFailed))
        );

        let foreign_token = Address::generate(&env);
        assert_eq!(
            client.try_rescue_stranded_tokens(&admin, &foreign_token, &client.address, &100),
            Err(Ok(Error::ValidationFailed))
        );
    }
}
