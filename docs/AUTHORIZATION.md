# Authorization Matrix

This contract uses Soroban `Address::require_auth()` checks to ensure only the correct party can call state-changing methods.

> Note: Some functions take an `Address` argument (e.g. `contributor`, `voter`, `admin`) and require that address to authorize the call. Others derive the authorized address from contract storage (e.g. `get_admin`) or from campaign state (e.g. `campaign.creator`).

| Public method | Who must authorize |
| --- | --- |
| `init(admin, ...)` | `admin` |
| `create_campaign(creator, ...)` | `creator` |
| `contribute(..., contributor, ...)` | `contributor` |
| `withdraw_funds(campaign_id)` | `campaign.creator` |
| `cancel_campaign(campaign_id)` | `campaign.creator` |
| `update_campaign(campaign_id, ...)` | `campaign.creator` |
| `update_campaign_description(campaign_id, ...)` | `campaign.creator` |
| `add_campaign_tag(campaign_id, tag)` | `campaign.creator` |
| `claim_refund(..., contributor)` | `contributor` |
| `deposit_revenue(campaign_id, ...)` | `campaign.creator` |
| `claim_revenue(..., contributor)` | `contributor` |
| `claim_creator_revenue(campaign_id)` | `campaign.creator` |
| `set_voting_params(admin, ...)` | `admin` |
| `pause(admin)` | `admin` (must match stored admin) |
| `unpause(admin)` | `admin` (must match stored admin) |
| `vote_on_campaign(..., voter, ...)` | `voter` |
| `verify_campaign(campaign_id)` | stored `admin` (from `get_admin`) |
| `verify_campaign_with_votes(campaign_id)` | no explicit auth (anyone) |
| `get_campaign(...)` | no auth |
| `get_campaign_optional(...)` | no auth |
| `get_campaign_count()` | no auth |
| `get_contribution(..., contributor)` | no auth |
| `get_revenue_pool(...)` | no auth |
| `get_revenue_claimed(..., contributor)` | no auth |
| `get_version()` | no auth |
| `update_platform_fee(new_fee)` | stored `admin` (from `get_admin`) |
| `set_min_campaign_funding_goal(admin, min_goal)` | stored `admin` (and `admin` must match stored admin) |
| `set_max_campaign_funding_goal(admin, max_goal)` | stored `admin` (and `admin` must match stored admin) |
| `update_admin(new_admin)` | stored `admin` — legacy shim, initiates a 2-step transfer via `initiate_admin_transfer` |
| `initiate_admin_transfer(admin, new_admin)` | `admin` (must match stored admin) |
| `accept_admin_transfer()` | pending admin (from `get_pending_admin`) |
| `cancel_admin_transfer(admin)` | `admin` (must match stored admin) |
| `propose_token_update(admin, new_token)` | `admin` (must match stored admin) |
| `accept_token_update(admin)` | `admin` (must match stored admin) |
| `cancel_token_update(admin)` | `admin` (must match stored admin) |
| `migrate(admin, expected_old_version)` | `admin` (must match stored admin) |
| `set_campaign_fee_override(campaign_id, admin, fee_bps)` | `admin` (must match stored admin) |
| `set_category_duration_cap(admin, category, max_days)` | `admin` (must match stored admin) |
| `remove_category_duration_cap(admin, category)` | `admin` (must match stored admin) |
| `set_min_voting_balance(admin, min_balance)` | `admin` (must match stored admin) |
| `set_creation_disabled(disabled)` | stored `admin` (from `get_admin`) |
| `resume_campaign(campaign_id, caller)` | `caller` (must be `campaign.creator` or stored `admin`) |
| `purge_voting_state(campaign_id, voters, finalize_aggregate)` | stored `admin` (from `get_admin`) |
| `set_personal_cap(campaign_id, contributor, amount)` | `contributor` |
| `remove_personal_cap(campaign_id, contributor)` | `contributor` |
| `extend_campaign_deadline(campaign_id, additional_days)` | `campaign.creator` |
| `withdraw_reserve(campaign_id)` | `campaign.creator` |
| `set_vesting_params(admin, delay_days, reserve_bps)` | stored `admin` (and `admin` must match stored admin) |
| `get_approve_votes(...)` | no auth |
| `get_reject_votes(...)` | no auth |
| `has_voted(..., voter)` | no auth |
| `get_min_votes_quorum()` | no auth |
| `get_approval_threshold_bps()` | no auth |
| `get_admin()` | no auth |
| `get_token()` | no auth |
| `get_platform_fee()` | no auth |
| `get_min_campaign_funding_goal()` | no auth |
| `list_campaigns(...)` | no auth |
| `list_active_campaigns(...)` | no auth |
| `get_campaigns_by_tag(tag, offset, limit)` | no auth |
| `get_campaign_tags(campaign_id)` | no auth |
| `initiate_campaign_transfer(campaign_id, new_creator)` | `campaign.creator` |
| `accept_campaign_transfer(campaign_id)` | pending creator (from `campaign.pending_creator`) |
| `cancel_campaign_transfer(campaign_id)` | `campaign.creator` |
| `save_campaign(user, campaign_id)` | `user` |
| `remove_saved_campaign(user, campaign_id)` | `user` |
| `batch_save_campaigns(user, campaign_ids)` | `user` |
| `clear_saved_campaigns(user)` | `user` |
| `get_saved(user)` | no auth |
| `get_saved_count(user)` | no auth |
| `get_saved_campaigns(user)` | no auth |
| `create_campaign_with_token(params, token)` | `params.creator` |
| `set_token_allowed(token, allowed)` | stored `admin` (from `get_admin`) |
| `batch_contribute(contributor, contributions)` | `contributor` |
| `emergency_withdraw(admin, campaign_id, recipient)` | `admin` (must match stored admin) |
| `cancel_emergency_withdrawal(admin, campaign_id)` | `admin` (must match stored admin) |
| `execute_emergency_withdrawal(admin, campaign_id)` | `admin` (must match stored admin) |
| `rescue_stranded_tokens(admin, token, to, amount)` | `admin` (must match stored admin) |
| `set_milestones(campaign_id, milestones)` | `campaign.creator` |
| `verify_milestone(admin, campaign_id, milestone_id)` | `admin` (must match stored admin) |
| `batch_verify_milestones(admin, campaign_id)` | `admin` (must match stored admin) |
| `claim_milestone(campaign_id, milestone_id)` | `campaign.creator` |
| `bump_campaign_state(campaign_id)` | no explicit auth (anyone); only extends storage TTLs |
| `set_cancellation_penalty_bps(admin, penalty_bps)` | `admin` (must match stored admin) |
| `admin_cancel_campaign(admin, campaign_id, reason)` | `admin` (must match stored admin) |
| `admin_cancel_campaign_transfer(admin, campaign_id)` | `admin` (must match stored admin) |
| `record_proof_of_impact(campaign_id, verifier, proof_hash)` | `verifier` |
| `delegate_votes(delegator, delegate)` | `delegator` |
| `undelegate_votes(delegator)` | `delegator` |
| `cast_delegated_vote(campaign_id, delegate, delegator, approve)` | `delegate` |
| `verify_campaigns(campaign_ids)` | stored `admin` (from `get_admin`) |
| `emergency_pause(caller)` | `caller` (must be in the emergency pause signer set) |
| `set_emergency_pause_signers(admin, signers)` | `admin` (must match stored admin) |
| `set_max_contribution_per_tx(admin, amount)` | `admin` (must match stored admin) |
| `set_category_voting_threshold(admin, category, threshold_bps)` | `admin` (must match stored admin) |
| `remove_category_voting_threshold(admin, category)` | `admin` (must match stored admin) |
| `set_token_update_delay_secs(admin, delay_secs)` | `admin` (must match stored admin) |
| `censure_comment(campaign_id, comment_hash, reason)` | stored `admin` (from `get_admin`) |
| `uncensure_comment(campaign_id, comment_hash)` | stored `admin` (from `get_admin`) |
| `get_campaign_token(...)` | no auth |
| `is_token_allowed(...)` | no auth |
| `get_emergency_withdrawal(...)` | no auth |
| `get_milestones(...)` | no auth |
| `is_milestone_claimed(...)` | no auth |
| `get_cancellation_penalty_bps(...)` | no auth |
| `get_proof_of_impact(...)` | no auth |
| `get_creator_bucket_balance(...)` | no auth |
| `get_dynamic_voting_weight(...)` | no auth |
| `get_donor_total_contributed(...)` | no auth |
| `get_delegate(...)` | no auth |
| `get_voter_balance_snapshot(...)` | no auth |
| `get_vote_snapshot_ledger(...)` | no auth |
| `get_emergency_pause_signers(...)` | no auth |
| `is_paused(...)` | no auth |
| `is_creation_disabled(...)` | no auth |
| `get_category_voting_threshold(...)` | no auth |
| `check_settlement_status(...)` | no auth |
| `get_total_raised_global(...)` | no auth |
| `get_total_raised_by_token(...)` | no auth |
| `get_total_contributors_count(...)` | no auth |
| `get_lifetime_contribution(...)` | no auth |
| `contract_version(...)` | no auth |
| `get_pending_admin(...)` | no auth |
| `get_max_contribution_per_tx(...)` | no auth |
| `get_bps_denominator(...)` | no auth |
| `get_token_update_delay_secs(...)` | no auth |
| `get_max_campaign_funding_goal(...)` | no auth |
| `get_min_voting_balance(...)` | no auth |
| `get_personal_cap(...)` | no auth |
| `get_campaign_reserve(...)` | no auth |
| `get_campaign_payout_marker(...)` | no auth |
| `has_pending_campaign_transfer(...)` | no auth |
| `is_campaign_creator(...)` | no auth |
| `get_campaigns_by_category(...)` | no auth |
| `get_creator_campaigns(...)` | no auth |
| `get_platform_stats(...)` | no auth |
| `get_platform_report(...)` | no auth |
| `get_creator_stats(...)` | no auth |
| `get_campaign_stats(...)` | no auth |
| `get_contributor_portfolio(...)` | no auth |
| `get_saved_campaigns_count(...)` | no auth |
| `is_comment_censured(...)` | no auth |
| `get_comment_censure(...)` | no auth |
| `get_censured_comment_count(...)` | no auth |

## Pause behaviour and failure modes (#1466)

- **Paused contract.** While the contract is paused, `contribute`, `batch_contribute`, `claim_milestone`,
  `delegate_votes`, `undelegate_votes`, `cast_delegated_vote` and `verify_campaigns` revert with `ContractPaused`.
  Admin configuration setters, `emergency_pause` and the views keep working.
- **Wrong signer.** A call without the required authorization fails the `require_auth()` check (host error). Admin
  entrypoints that take an `admin` argument additionally return `NotAuthorized` when it is not the stored admin.
- **Emergency pause.** `emergency_pause` returns `NotAuthorized` when the signer set is empty or `caller` is not in it.
- **Milestones.** `claim_milestone` reverts unless the milestone was verified by `verify_milestone` /
  `batch_verify_milestones`, the campaign is active and verified, and the milestone was not already claimed.
- **Views.** Every `get_*` / `is_*` / `has_*` / `check_*` entrypoint is read-only and needs no authorization.
