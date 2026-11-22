# Contract API Reference

> **Generated** by `python3 scripts/gen-api-docs.py` from `src/lib.rs`. Do not edit by hand; CI (`--check`) fails when this file is stale or misses an entrypoint (#1465).

The contract exposes **142** public entrypoints. Errors are the `Error` codes documented in `src/errors.rs`; authorization follows [AUTHORIZATION.md](AUTHORIZATION.md).

## Index

[`init`](#init), [`create_campaign`](#create_campaign), [`create_campaign_with_token`](#create_campaign_with_token), [`get_campaign_token`](#get_campaign_token), [`set_token_allowed`](#set_token_allowed), [`is_token_allowed`](#is_token_allowed), [`contribute`](#contribute), [`claim_refund`](#claim_refund), [`batch_contribute`](#batch_contribute), [`withdraw_funds`](#withdraw_funds), [`withdraw_reserve`](#withdraw_reserve), [`set_vesting_params`](#set_vesting_params), [`emergency_withdraw`](#emergency_withdraw), [`cancel_emergency_withdrawal`](#cancel_emergency_withdrawal), [`execute_emergency_withdrawal`](#execute_emergency_withdrawal), [`get_emergency_withdrawal`](#get_emergency_withdrawal), [`rescue_stranded_tokens`](#rescue_stranded_tokens), [`set_milestones`](#set_milestones), [`verify_milestone`](#verify_milestone), [`batch_verify_milestones`](#batch_verify_milestones), [`bump_campaign_state`](#bump_campaign_state), [`claim_milestone`](#claim_milestone), [`get_milestones`](#get_milestones), [`is_milestone_claimed`](#is_milestone_claimed), [`cancel_campaign`](#cancel_campaign), [`set_cancellation_penalty_bps`](#set_cancellation_penalty_bps), [`get_cancellation_penalty_bps`](#get_cancellation_penalty_bps), [`admin_cancel_campaign`](#admin_cancel_campaign), [`update_campaign`](#update_campaign), [`update_campaign_description`](#update_campaign_description), [`add_campaign_tag`](#add_campaign_tag), [`extend_campaign_deadline`](#extend_campaign_deadline), [`record_proof_of_impact`](#record_proof_of_impact), [`get_proof_of_impact`](#get_proof_of_impact), [`initiate_campaign_transfer`](#initiate_campaign_transfer), [`accept_campaign_transfer`](#accept_campaign_transfer), [`cancel_campaign_transfer`](#cancel_campaign_transfer), [`admin_cancel_campaign_transfer`](#admin_cancel_campaign_transfer), [`deposit_revenue`](#deposit_revenue), [`claim_revenue`](#claim_revenue), [`claim_creator_revenue`](#claim_creator_revenue), [`get_creator_bucket_balance`](#get_creator_bucket_balance), [`vote_on_campaign`](#vote_on_campaign), [`get_dynamic_voting_weight`](#get_dynamic_voting_weight), [`get_donor_total_contributed`](#get_donor_total_contributed), [`delegate_votes`](#delegate_votes), [`undelegate_votes`](#undelegate_votes), [`get_delegate`](#get_delegate), [`get_voter_balance_snapshot`](#get_voter_balance_snapshot), [`get_vote_snapshot_ledger`](#get_vote_snapshot_ledger), [`cast_delegated_vote`](#cast_delegated_vote), [`verify_campaign`](#verify_campaign), [`verify_campaigns`](#verify_campaigns), [`verify_campaign_with_votes`](#verify_campaign_with_votes), [`resume_campaign`](#resume_campaign), [`purge_voting_state`](#purge_voting_state), [`pause`](#pause), [`unpause`](#unpause), [`emergency_pause`](#emergency_pause), [`set_emergency_pause_signers`](#set_emergency_pause_signers), [`get_emergency_pause_signers`](#get_emergency_pause_signers), [`is_paused`](#is_paused), [`set_creation_disabled`](#set_creation_disabled), [`is_creation_disabled`](#is_creation_disabled), [`update_platform_fee`](#update_platform_fee), [`set_max_contribution_per_tx`](#set_max_contribution_per_tx), [`set_campaign_fee_override`](#set_campaign_fee_override), [`set_category_duration_cap`](#set_category_duration_cap), [`remove_category_duration_cap`](#remove_category_duration_cap), [`set_min_campaign_funding_goal`](#set_min_campaign_funding_goal), [`set_max_campaign_funding_goal`](#set_max_campaign_funding_goal), [`set_voting_params`](#set_voting_params), [`set_min_voting_balance`](#set_min_voting_balance), [`set_category_voting_threshold`](#set_category_voting_threshold), [`remove_category_voting_threshold`](#remove_category_voting_threshold), [`get_category_voting_threshold`](#get_category_voting_threshold), [`propose_token_update`](#propose_token_update), [`accept_token_update`](#accept_token_update), [`cancel_token_update`](#cancel_token_update), [`set_token_update_delay_secs`](#set_token_update_delay_secs), [`initiate_admin_transfer`](#initiate_admin_transfer), [`accept_admin_transfer`](#accept_admin_transfer), [`cancel_admin_transfer`](#cancel_admin_transfer), [`update_admin`](#update_admin), [`migrate`](#migrate), [`set_personal_cap`](#set_personal_cap), [`remove_personal_cap`](#remove_personal_cap), [`get_campaign`](#get_campaign), [`get_campaign_optional`](#get_campaign_optional), [`get_campaign_count`](#get_campaign_count), [`check_settlement_status`](#check_settlement_status), [`get_total_raised_global`](#get_total_raised_global), [`get_total_raised_by_token`](#get_total_raised_by_token), [`get_total_contributors_count`](#get_total_contributors_count), [`get_contribution`](#get_contribution), [`get_lifetime_contribution`](#get_lifetime_contribution), [`get_revenue_pool`](#get_revenue_pool), [`get_revenue_claimed`](#get_revenue_claimed), [`get_version`](#get_version), [`contract_version`](#contract_version), [`get_admin`](#get_admin), [`get_pending_admin`](#get_pending_admin), [`get_token`](#get_token), [`get_platform_fee`](#get_platform_fee), [`get_max_contribution_per_tx`](#get_max_contribution_per_tx), [`get_bps_denominator`](#get_bps_denominator), [`get_token_update_delay_secs`](#get_token_update_delay_secs), [`get_min_campaign_funding_goal`](#get_min_campaign_funding_goal), [`get_max_campaign_funding_goal`](#get_max_campaign_funding_goal), [`get_min_voting_balance`](#get_min_voting_balance), [`get_approve_votes`](#get_approve_votes), [`get_reject_votes`](#get_reject_votes), [`has_voted`](#has_voted), [`get_min_votes_quorum`](#get_min_votes_quorum), [`get_approval_threshold_bps`](#get_approval_threshold_bps), [`get_personal_cap`](#get_personal_cap), [`get_campaign_reserve`](#get_campaign_reserve), [`get_campaign_payout_marker`](#get_campaign_payout_marker), [`has_pending_campaign_transfer`](#has_pending_campaign_transfer), [`is_campaign_creator`](#is_campaign_creator), [`list_campaigns`](#list_campaigns), [`list_active_campaigns`](#list_active_campaigns), [`get_campaigns_by_category`](#get_campaigns_by_category), [`get_campaigns_by_tag`](#get_campaigns_by_tag), [`get_campaign_tags`](#get_campaign_tags), [`get_creator_campaigns`](#get_creator_campaigns), [`get_platform_stats`](#get_platform_stats), [`get_platform_report`](#get_platform_report), [`get_creator_stats`](#get_creator_stats), [`get_campaign_stats`](#get_campaign_stats), [`get_contributor_portfolio`](#get_contributor_portfolio), [`save_campaign`](#save_campaign), [`batch_save_campaigns`](#batch_save_campaigns), [`remove_saved_campaign`](#remove_saved_campaign), [`clear_saved_campaigns`](#clear_saved_campaigns), [`get_saved_campaigns`](#get_saved_campaigns), [`get_saved_campaigns_count`](#get_saved_campaigns_count), [`censure_comment`](#censure_comment), [`uncensure_comment`](#uncensure_comment), [`is_comment_censured`](#is_comment_censured), [`get_comment_censure`](#get_comment_censure), [`get_censured_comment_count`](#get_censured_comment_count)

## Entrypoints

### init

```rust
init(admin: Address, token: Address, platform_fee: u32) -> Result<(), Error>
```

Initializes the contract with its `admin`, the platform `token` and the
`platform_fee` (basis points). Callable exactly once; requires `admin`'s
authorization.

- **Auth:** `admin` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `AlreadyInitialized` — `init` has already been called.
  - `InvalidPlatformFee` — `platform_fee` exceeds the allowed maximum.

### create_campaign

```rust
create_campaign(params: CreateCampaignParams) -> Result<u32, Error>
```

Creates a campaign denominated in the platform token and returns its new
id. The campaign creator in `params` must authorize the call.

Reverts if the contract is paused, campaign creation is disabled, or any
field in `params` fails validation (title/description length, funding-goal
bounds, duration and category caps).

- **Auth:** `creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### create_campaign_with_token

```rust
create_campaign_with_token(params: CreateCampaignParams, token: Address) -> Result<u32, Error>
```

Creates a campaign denominated in `token` rather than the platform
token (#784).

The currency is fixed for the campaign's lifetime — contributions,
refunds, withdrawals, milestone payouts and revenue all move in it —
and must be on the admin allowlist. `create_campaign` remains the
platform-token shorthand.

- **Auth:** `params.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_campaign_token

```rust
get_campaign_token(campaign_id: u32) -> Address
```

The currency a campaign accepts. Falls back to the platform token for
campaigns created before per-campaign currencies existed.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### set_token_allowed

```rust
set_token_allowed(token: Address, allowed: bool) -> Result<(), Error>
```

Admin: allow or disallow a token as a campaign currency (#784).

Disallowing affects only future campaigns; existing ones keep the
currency they were created with.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### is_token_allowed

```rust
is_token_allowed(token: Address) -> bool
```

Whether a token may be chosen as a campaign currency. The platform
token always is.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### contribute

```rust
contribute(campaign_id: u32, contributor: Address, amount: i128) -> Result<(), Error>
```

Contributes `amount` of the campaign's currency from `contributor` to
`campaign_id`. Requires `contributor`'s authorization.

Reverts if the contract is paused, the campaign is not active or past its
deadline, or `amount` breaks a global, campaign or personal cap.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### claim_refund

```rust
claim_refund(campaign_id: u32, contributor: Address) -> Result<(), Error>
```

Refunds `contributor`'s full contribution to `campaign_id` when the
campaign is cancelled or ended without meeting its goal. Requires
`contributor`'s authorization.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### batch_contribute

```rust
batch_contribute(contributor: Address, contributions: soroban_sdk::Vec<(u32, i128)>) -> Result<(), Error>
```

Contributes to multiple campaigns in a single transaction, moving the
combined token amount in one transfer instead of one per campaign
(#518). Every item is validated with the same rules as `contribute`;
if any item fails, the whole batch reverts atomically.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### withdraw_funds

```rust
withdraw_funds(campaign_id: u32) -> Result<(), Error>
```

Pays out a verified campaign that met its goal to its creator, net of
the platform fee (and of any vesting reserve). Creator only; reverts if the
contract is paused, the campaign is unverified or funds were already
withdrawn.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### withdraw_reserve

```rust
withdraw_reserve(campaign_id: u32) -> Result<(), Error>
```

Releases a campaign's held-back vesting reserve (see
`set_vesting_params`) once its vesting delay has elapsed. Reverts if the
contract is paused or the reserve was already released.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_vesting_params

```rust
set_vesting_params(admin: Address, delay_days: u64, reserve_bps: u32) -> Result<(), Error>
```

Admin: sets the vesting policy applied at withdrawal — `reserve_bps` of
the payout is held back and becomes withdrawable via `withdraw_reserve`
after `delay_days`.

- **Auth:** stored `admin` (and `admin` must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.
  - `ValidationFailed` — `reserve_bps > 10000` or `delay_days > 365`.

### emergency_withdraw

```rust
emergency_withdraw(admin: Address, campaign_id: u32, recipient: Address) -> Result<(), Error>
```

**Last resort.** Proposes recovering the escrowed funds of a campaign
that met its goal but can no longer pay out — e.g. the creator address
is a dead/locked account — sending them to `recipient`.

Moves no funds. Starts a mandatory 7-day timelock; the transfer only
happens when `execute_emergency_withdrawal` is called afterwards. Emits
`emergency_withdrawal_proposed`. Admin only.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### cancel_emergency_withdrawal

```rust
cancel_emergency_withdrawal(admin: Address, campaign_id: u32) -> Result<(), Error>
```

Cancels a pending emergency withdrawal before it is executed (#802).
Emits `emergency_withdrawal_cancelled`. Admin only.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### execute_emergency_withdrawal

```rust
execute_emergency_withdrawal(admin: Address, campaign_id: u32) -> Result<(), Error>
```

Executes a pending emergency withdrawal once its 7-day timelock has
elapsed, transferring the campaign's escrowed principal to the recipient
recorded at proposal time (#802). Emits `emergency_withdrawal_executed`.
Admin only.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_emergency_withdrawal

```rust
get_emergency_withdrawal(campaign_id: u32) -> Option<EmergencyWithdrawal>
```

The pending emergency withdrawal for a campaign (recipient and the
timestamp it becomes executable), or `None` (#802).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### rescue_stranded_tokens

```rust
rescue_stranded_tokens(admin: Address, token: Address, to: Address, amount: i128) -> Result<(), Error>
```

Admin: rescues stranded foreign tokens accidentally sent to the
contract (#1037). Only non-escrow tokens may be rescued: `token` must
differ from the platform token and must not be on the campaign-currency
allowlist. Emits `stranded_tokens_rescued`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_milestones

```rust
set_milestones(campaign_id: u32, milestones: soroban_sdk::Vec<Milestone>) -> Result<(), Error>
```

Creator: defines the milestone schedule for `campaign_id`, splitting its
payout into separately verified tranches (#783). Reverts if the contract
is paused, the campaign is inactive, or the milestones fail validation.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### verify_milestone

```rust
verify_milestone(admin: Address, campaign_id: u32, milestone_id: u32) -> Result<(), Error>
```

Admin: marks milestone `milestone_id` of `campaign_id` as verified,
making it claimable by the creator via `claim_milestone`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### batch_verify_milestones

```rust
batch_verify_milestones(admin: Address, campaign_id: u32) -> Result<u32, Error>
```

Admin: verifies all milestones for `campaign_id` in one call (#1043).

Intended for verified non-profit creator accounts where per-milestone
review is not required. Already-verified milestones are skipped.
Returns the count of milestones newly verified.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized`     — Caller is not the stored admin.
  - `ContractPaused`    — The contract is paused.
  - `CampaignNotFound`  — No campaign with that id.
  - `CampaignNotActive` — The campaign is cancelled.
  - `MilestoneNotFound` — No milestone plan has been set.

### bump_campaign_state

```rust
bump_campaign_state(campaign_id: u32) -> ()
```

Extends the TTL of all core persistent storage keys for a campaign in
one call (#1062). Useful as a keep-alive for long-running active
campaigns whose entries might otherwise expire between user activity.
Instance-storage entries are not touched here; call the standard
`bump_instance_ttl` for those.

- **Auth:** no explicit auth (anyone); only extends storage TTLs (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### claim_milestone

```rust
claim_milestone(campaign_id: u32, milestone_id: u32) -> Result<(), Error>
```

Creator: withdraws the tranche for a verified milestone. Reverts if the
contract is paused, the campaign is inactive or unverified, or the
milestone was already claimed.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_milestones

```rust
get_milestones(campaign_id: u32) -> soroban_sdk::Vec<Milestone>
```

Returns the milestone schedule configured for `campaign_id` (empty if
none).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### is_milestone_claimed

```rust
is_milestone_claimed(campaign_id: u32, milestone_id: u32) -> bool
```

Whether milestone `milestone_id` of `campaign_id` has already been
paid out.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### cancel_campaign

```rust
cancel_campaign(campaign_id: u32) -> Result<(), Error>
```

Creator: cancels an active campaign so contributors can claim refunds.
Reverts once the campaign has met its goal (anti-rug-pull guard) — use
`admin_cancel_campaign` for fraud response instead.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_cancellation_penalty_bps

```rust
set_cancellation_penalty_bps(admin: Address, penalty_bps: u32) -> Result<(), Error>
```

Admin: sets the voluntary-cancellation penalty in basis points
(max 500 = 5%, 0 disables, #1052). Charged creator -> admin on
`cancel_campaign`; contributor refunds are unaffected.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_cancellation_penalty_bps

```rust
get_cancellation_penalty_bps() -> u32
```

Returns the voluntary-cancellation penalty in basis points (#1052).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### admin_cancel_campaign

```rust
admin_cancel_campaign(admin: Address, campaign_id: u32, reason: String) -> Result<(), Error>
```

Admin-only targeted fraud response: cancels a single campaign without
pausing the entire platform (#508). Unlike `cancel_campaign`, this
bypasses the goal-met anti-rug-pull guard so admins can stop verified
fraudulent campaigns even after they've hit their funding goal.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### update_campaign

```rust
update_campaign(campaign_id: u32, title: String, description: String) -> Result<(), Error>
```

Creator: replaces the title and description of a campaign that has not
been verified yet. Reverts if paused, already verified, or the new text
fails length validation.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### update_campaign_description

```rust
update_campaign_description(campaign_id: u32, description: String) -> Result<(), Error>
```

Creator: replaces only the description of a campaign. Reverts if paused
or the description fails length validation.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### add_campaign_tag

```rust
add_campaign_tag(campaign_id: u32, tag: String) -> Result<(), Error>
```

Creator: adds a discovery tag to an active campaign and indexes the
campaign under it (#798). Tags are append-only. Reverts if paused, the
campaign is not active, or the tag is empty, longer than
`CAMPAIGN_TAG_MAX_LEN`, a duplicate, or would exceed
`MAX_TAGS_PER_CAMPAIGN`. Emits `campaign_tag_added`.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### extend_campaign_deadline

```rust
extend_campaign_deadline(campaign_id: u32, additional_days: u64) -> Result<(), Error>
```

Creator: pushes an active campaign's deadline out by `additional_days`.
A campaign may be extended at most once and only before its deadline
passes.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `DeadlineAlreadyExtended` — The deadline was already extended.
  - `DeadlinePassed` — The current deadline is already in the past.

### record_proof_of_impact

```rust
record_proof_of_impact(campaign_id: u32, verifier: Address, proof_hash: BytesN<32>) -> Result<(), Error>
```

Records a cryptographic proof-of-impact hash submitted by an authorized verifier.

- **Auth:** `verifier` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_proof_of_impact

```rust
get_proof_of_impact(campaign_id: u32) -> Option<ProofOfImpact>
```

Returns the recorded proof of impact for a campaign, if one exists.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### initiate_campaign_transfer

```rust
initiate_campaign_transfer(campaign_id: u32, new_creator: Address) -> Result<(), Error>
```

Starts a two-step ownership transfer of `campaign_id` to `new_creator`.
Both the current creator and `new_creator` must authorize. The nomination
expires after `TRANSFER_EXPIRY_SECS`.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `InvalidNewOwner` — `new_creator` is already the creator.
  - `TransferAlreadyPending` — An unexpired nomination already exists.

### accept_campaign_transfer

```rust
accept_campaign_transfer(campaign_id: u32) -> Result<(), Error>
```

Completes a pending campaign transfer. The nominated creator must
authorize; ownership and the creator index move to them.

- **Auth:** pending creator (from `campaign.pending_creator`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NoTransferPending` — Nothing is pending, or the nomination expired.

### cancel_campaign_transfer

```rust
cancel_campaign_transfer(campaign_id: u32) -> Result<(), Error>
```

Creator: withdraws a pending ownership nomination.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NoTransferPending` — No transfer is pending.

### admin_cancel_campaign_transfer

```rust
admin_cancel_campaign_transfer(admin: Address, campaign_id: u32) -> Result<(), Error>
```

Admin recovery path for a pending transfer whose nominee is
unreachable (#869): lets the admin clear `pending_creator` without
waiting for `TRANSFER_EXPIRY_SECS` to elapse.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### deposit_revenue

```rust
deposit_revenue(campaign_id: u32, amount: i128) -> Result<(), Error>
```

Creator: deposits `amount` of post-campaign revenue into the campaign's
revenue pool for pro-rata distribution to contributors. Requires revenue
sharing to be enabled on the campaign.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### claim_revenue

```rust
claim_revenue(campaign_id: u32, contributor: Address) -> Result<(), Error>
```

Claims `contributor`'s pro-rata share of the campaign's revenue pool.
Requires `contributor`'s authorization.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### claim_creator_revenue

```rust
claim_creator_revenue(campaign_id: u32) -> Result<(), Error>
```

Creator: claims the creator's retained share of the campaign's revenue
pool. Requires revenue sharing to be enabled.

- **Auth:** `campaign.creator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_creator_bucket_balance

```rust
get_creator_bucket_balance(campaign_id: u32, creator: Address) -> i128
```

Returns the isolated creator revenue bucket balance for a campaign.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### vote_on_campaign

```rust
vote_on_campaign(campaign_id: u32, voter: Address, approve: bool) -> Result<(), Error>
```

Casts `voter`'s approve/reject vote on an unverified campaign. Requires
`voter`'s authorization and the minimum voting balance; each address may
vote once.

- **Auth:** `voter` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `ContractPaused` — The contract is paused.
  - `CampaignNotFound` — No campaign with that id.
  - `CampaignNotActive` — The campaign is not in an active state.
  - `DeadlinePassed` — The campaign deadline has passed.
  - `CampaignAlreadyVerified` — The campaign is already verified.
  - `NotTokenHolder` — Voter holds no tokens or is below the minimum balance.
  - `AlreadyVoted` — Voter has already voted on this campaign.

### get_dynamic_voting_weight

```rust
get_dynamic_voting_weight(campaign_id: u32, voter: Address) -> i128
```

Returns a voter's dynamic voting weight for a campaign: platform-token
balance plus cumulative donor history (#1054). Informational ranking
signal; consensus stays 1-address-1-vote (#469).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_donor_total_contributed

```rust
get_donor_total_contributed(donor: Address) -> i128
```

Returns a donor's cumulative contributions across all campaigns (#1054).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### delegate_votes

```rust
delegate_votes(delegator: Address, delegate: Address) -> Result<(), Error>
```

Delegates the caller's voting rights to a designated community delegate.

- **Auth:** `delegator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### undelegate_votes

```rust
undelegate_votes(delegator: Address) -> Result<(), Error>
```

Revokes an existing vote delegation.

- **Auth:** `delegator` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_delegate

```rust
get_delegate(delegator: Address) -> Option<Address>
```

Returns the active delegate for a given delegator, if one has been assigned.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_voter_balance_snapshot

```rust
get_voter_balance_snapshot(campaign_id: u32, voter: Address) -> Option<(i128, u32)>
```

Returns the `(balance, ledger_sequence)` snapshotted when `voter` cast
their vote on `campaign_id` (#967). `None` if the voter has not voted.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_vote_snapshot_ledger

```rust
get_vote_snapshot_ledger(campaign_id: u32) -> Option<u32>
```

Returns the ledger sequence at which voting on `campaign_id` was
initiated, or `None` if no vote has been cast (#967).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### cast_delegated_vote

```rust
cast_delegated_vote(campaign_id: u32, delegate: Address, delegator: Address, approve: bool) -> Result<(), Error>
```

Casts a vote on behalf of a delegator who delegated their voting rights to the caller.

- **Auth:** `delegate` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### verify_campaign

```rust
verify_campaign(campaign_id: u32) -> Result<(), Error>
```

Admin: verifies `campaign_id` directly, bypassing community voting.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.
  - `ContractPaused` — The contract is paused.
  - `CampaignNotFound` — No campaign with that id.
  - `CampaignNotActive` — The campaign is not in an active state.
  - `CampaignAlreadyVerified` — The campaign is already verified.

### verify_campaigns

```rust
verify_campaigns(campaign_ids: soroban_sdk::Vec<u32>) -> Result<(soroban_sdk::Vec<u32>, soroban_sdk::Vec<u32>), Error>
```

Batch-verifies up to 50 campaigns in one admin call (#442).

Returns `Ok((verified_ids, failed_ids))` covering every id it
processed. Successful verifications are committed even when other ids
fail, so callers can distinguish partial success from total failure and
retry only the failed ids — the previous behaviour collapsed the whole
batch to `Err(first_error)`. Per-campaign failures are collected in
`failed_ids` and never abort the batch; only hard errors (not admin,
paused) return `Err`. The voting-state TTL is extended for every
processed id, success or failure.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.
  - `ContractPaused` — The contract is paused.

### verify_campaign_with_votes

```rust
verify_campaign_with_votes(campaign_id: u32) -> Result<(), Error>
```

Verifies `campaign_id` once its community vote meets quorum and the
applicable approval threshold. Callable by anyone.

- **Auth:** no explicit auth (anyone) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `ContractPaused` — The contract is paused.
  - `CampaignNotFound` — No campaign with that id.
  - `CampaignNotActive` — The campaign is not in an active state.
  - `DeadlinePassed` — The campaign deadline has passed.
  - `CampaignAlreadyVerified` — The campaign is already verified.
  - `VotingQuorumNotMet` — Fewer total votes than the required quorum.
  - `VotingThresholdNotMet` — Approval percentage below the required threshold.

### resume_campaign

```rust
resume_campaign(campaign_id: u32, caller: Address) -> Result<(), Error>
```

Clears an automatic (anomaly-triggered) pause for `campaign_id`. `caller`
must authorize and be either the campaign creator or the admin.

- **Auth:** `caller` (must be `campaign.creator` or stored `admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `ValidationFailed` — The contract is not auto-paused.
  - `NotAuthorized` — `caller` is neither the creator nor the admin.

### purge_voting_state

```rust
purge_voting_state(campaign_id: u32, voters: soroban_sdk::Vec<Address>, finalize_aggregate: bool) -> Result<(), Error>
```

Admin: deletes per-voter records for `voters` on `campaign_id` to reclaim
storage; `finalize_aggregate` also clears the campaign's vote tallies.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.

### pause

```rust
pause() -> Result<(), Error>
```

Admin: pauses all state-changing user operations.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.

### unpause

```rust
unpause() -> Result<(), Error>
```

Admin: lifts a manual or automatic pause.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — Caller is not the stored admin.

### emergency_pause

```rust
emergency_pause(caller: Address) -> Result<(), Error>
```

Pauses the contract from a registered emergency signer, so an incident
can be contained without the admin key. `caller` must authorize.

- **Auth:** `caller` (must be in the emergency pause signer set) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NotAuthorized` — `caller` is not a registered emergency signer.

### set_emergency_pause_signers

```rust
set_emergency_pause_signers(admin: Address, signers: soroban_sdk::Vec<Address>) -> Result<(), Error>
```

Admin: replaces the set of addresses allowed to call `emergency_pause`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_emergency_pause_signers

```rust
get_emergency_pause_signers() -> soroban_sdk::Vec<Address>
```

Returns the addresses allowed to call `emergency_pause`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### is_paused

```rust
is_paused() -> bool
```

Whether the contract is paused, either manually (`pause` /
`emergency_pause`) or automatically by anomaly detection.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### set_creation_disabled

```rust
set_creation_disabled(disabled: bool) -> Result<(), Error>
```

Admin: turns new campaign creation off (`true`) or back on. Works while
paused.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### is_creation_disabled

```rust
is_creation_disabled() -> bool
```

Whether new campaign creation is currently disabled.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### update_platform_fee

```rust
update_platform_fee(new_fee: u32) -> Result<(), Error>
```

Admin: sets the platform fee in basis points. Works while paused.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `InvalidPlatformFee` — `new_fee` exceeds `PLATFORM_FEE_MAX_BPS`.

### set_max_contribution_per_tx

```rust
set_max_contribution_per_tx(admin: Address, amount: i128) -> Result<(), Error>
```

Sets the global per-transaction contribution cap. `0` disables the cap.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_campaign_fee_override

```rust
set_campaign_fee_override(campaign_id: u32, admin: Address, fee_bps: u32) -> Result<(), Error>
```

Admin: sets a campaign-specific platform fee (`fee_bps`) that replaces
the global fee for `campaign_id`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_category_duration_cap

```rust
set_category_duration_cap(admin: Address, category: Category, max_days: u64) -> Result<(), Error>
```

Admin: caps the duration (in days) of new campaigns in `category`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### remove_category_duration_cap

```rust
remove_category_duration_cap(admin: Address, category: Category) -> Result<(), Error>
```

Admin: removes the duration cap for `category`, restoring the global
maximum.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_min_campaign_funding_goal

```rust
set_min_campaign_funding_goal(admin: Address, min_goal: i128) -> Result<(), Error>
```

Admin: sets the minimum funding goal a new campaign may request.

- **Auth:** stored `admin` (and `admin` must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_max_campaign_funding_goal

```rust
set_max_campaign_funding_goal(admin: Address, max_goal: i128) -> Result<(), Error>
```

Admin: sets the maximum funding goal a new campaign may request.

- **Auth:** stored `admin` (and `admin` must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_voting_params

```rust
set_voting_params(admin: Address, min_votes_quorum: u32, approval_threshold_bps: u32) -> Result<(), Error>
```

Admin: sets the global voting quorum (`min_votes_quorum`) and approval
threshold (`approval_threshold_bps`) used by `verify_campaign_with_votes`.

- **Auth:** `admin` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_min_voting_balance

```rust
set_min_voting_balance(admin: Address, min_balance: i128) -> Result<(), Error>
```

Admin: sets the minimum token balance an address needs to vote.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_category_voting_threshold

```rust
set_category_voting_threshold(admin: Address, category: Category, threshold_bps: u32) -> Result<(), Error>
```

Admin: overrides the approval threshold (basis points) for `category`
(#536).

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### remove_category_voting_threshold

```rust
remove_category_voting_threshold(admin: Address, category: Category) -> Result<(), Error>
```

Admin: removes `category`'s approval-threshold override, falling back to
the global default.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_category_voting_threshold

```rust
get_category_voting_threshold(category: Category) -> u32
```

Returns the approval threshold (in basis points) that actually applies
to `category`: its per-category override if one is set, otherwise the
global default (#536).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### propose_token_update

```rust
propose_token_update(admin: Address, new_token: Address) -> Result<(), Error>
```

Admin: proposes replacing the platform token with `new_token`. Takes
effect only via `accept_token_update` after the timelock (see
`get_token_update_delay_secs`).

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### accept_token_update

```rust
accept_token_update(admin: Address) -> Result<(), Error>
```

Admin: applies a proposed token update once its timelock has elapsed.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `ValidationFailed` — Nothing is pending or the timelock has not elapsed.

### cancel_token_update

```rust
cancel_token_update(admin: Address) -> Result<(), Error>
```

Admin: discards a pending token update proposal.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_token_update_delay_secs

```rust
set_token_update_delay_secs(admin: Address, delay_secs: u64) -> Result<(), Error>
```

Overrides the timelock delay `propose_token_update` enforces before a
pending token update can be accepted (default: 7 days), so platforms
that want a longer or shorter timelock don't need a code change and
redeploy (#650). Must be in `(0, 365 days]`.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### initiate_admin_transfer

```rust
initiate_admin_transfer(admin: Address, new_admin: Address) -> Result<(), Error>
```

Admin: nominates `new_admin` as the next admin (two-step transfer).
Works while paused.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `InvalidNewOwner` — `new_admin` is already the admin.

### accept_admin_transfer

```rust
accept_admin_transfer() -> Result<(), Error>
```

Completes a pending admin transfer. The nominated admin must authorize.
Works while paused.

- **Auth:** pending admin (from `get_pending_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `NoTransferPending` — No admin transfer is pending.

### cancel_admin_transfer

```rust
cancel_admin_transfer(admin: Address) -> Result<(), Error>
```

Admin: withdraws a pending admin nomination.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### update_admin

```rust
update_admin(new_admin: Address) -> Result<(), Error>
```

Legacy alias for `initiate_admin_transfer` using the stored admin. Starts
a two-step transfer; `new_admin` must still call `accept_admin_transfer`.

- **Auth:** stored `admin` — legacy shim, initiates a 2-step transfer via `initiate_admin_transfer` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### migrate

```rust
migrate(admin: Address, expected_old_version: u32) -> Result<(), Error>
```

Admin: runs storage migrations after an upgrade. `expected_old_version`
must match the stored version, guarding against running a migration twice
or on the wrong build.

- **Auth:** `admin` (must match stored admin) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### set_personal_cap

```rust
set_personal_cap(campaign_id: u32, contributor: Address, amount: i128) -> Result<(), Error>
```

Sets a self-imposed cap on how much `contributor` may contribute to
`campaign_id` in total (#503). Requires `contributor`'s authorization.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### remove_personal_cap

```rust
remove_personal_cap(campaign_id: u32, contributor: Address) -> Result<(), Error>
```

Removes the contributor's personal contribution cap for a campaign,
restoring the campaign-wide `max_contribution_per_user` as the only
bound on their contributions (#503). Requires `contributor`'s auth.

- **Auth:** `contributor` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_campaign

```rust
get_campaign(campaign_id: u32) -> Result<Campaign, Error>
```

Returns campaign `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `CampaignNotFound` — No campaign with that id.

### get_campaign_optional

```rust
get_campaign_optional(campaign_id: u32) -> Option<Campaign>
```

Returns campaign `campaign_id`, or `None` if it does not exist.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaign_count

```rust
get_campaign_count() -> u32
```

Returns the total number of campaigns ever created.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### check_settlement_status

```rust
check_settlement_status(campaign_id: u32) -> Result<(bool, bool, bool), Error>
```

Checks settlement status for a campaign (#1027).

Returns `(deadline_passed, goal_met, ready_for_withdrawal)` to help
external systems determine if a campaign needs settlement action:
- `deadline_passed`: true if current timestamp > campaign.deadline
- `goal_met`: true if amount_raised >= funding_goal
- `ready_for_withdrawal`: true if verified, goal met, deadline passed,
  and not yet withdrawn

Background keepers can call this for active campaigns to find ones
ready for auto-settlement. Campaigns with `deadline_passed && goal_met`
have succeeded and can be withdrawn; those with `deadline_passed &&
!goal_met` have expired and contributors can claim refunds.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:**
  - `CampaignNotFound` — No campaign with that id.

### get_total_raised_global

```rust
get_total_raised_global() -> i128
```

Returns the total amount raised across all campaigns denominated in the
platform token. For multi-token platforms use `get_total_raised_by_token`
to query per-currency totals (#1438).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_total_raised_by_token

```rust
get_total_raised_by_token(token: Address) -> i128
```

Returns the total amount raised for a specific token across all
campaigns (#1438). Use this instead of `get_total_raised_global` on
multi-token platforms to avoid summing incompatible currencies.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_total_contributors_count

```rust
get_total_contributors_count(campaign_id: u32) -> u32
```

Returns the number of distinct contributors to `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_contribution

```rust
get_contribution(campaign_id: u32, contributor: Address) -> i128
```

Returns `contributor`'s current (unrefunded) contribution to
`campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_lifetime_contribution

```rust
get_lifetime_contribution(campaign_id: u32, contributor: Address) -> i128
```

Returns everything `contributor` has ever contributed to `campaign_id`,
including amounts later refunded.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_revenue_pool

```rust
get_revenue_pool(campaign_id: u32) -> i128
```

Returns the revenue deposited into `campaign_id`'s pool.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_revenue_claimed

```rust
get_revenue_claimed(campaign_id: u32, contributor: Address) -> i128
```

Returns how much revenue `contributor` has already claimed from
`campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_version

```rust
get_version() -> u32
```

Returns the contract version recorded in storage (updated by `migrate`).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### contract_version

```rust
contract_version() -> u32
```

Returns the compiled-in contract version. Unlike `get_version`, this
reads a constant baked into the WASM at build time rather than
instance storage, so it can be called on a freshly deployed contract
before `init` has ever been invoked (#523).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_admin

```rust
get_admin() -> Address
```

Returns the current admin address.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_pending_admin

```rust
get_pending_admin() -> Option<Address>
```

Returns the nominated admin of a pending transfer, if any.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_token

```rust
get_token() -> Address
```

Returns the platform token address.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_platform_fee

```rust
get_platform_fee() -> u32
```

Returns the global platform fee in basis points.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_max_contribution_per_tx

```rust
get_max_contribution_per_tx() -> i128
```

Returns the global per-transaction contribution cap; `0` means unlimited.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_bps_denominator

```rust
get_bps_denominator() -> u32
```

Returns the basis-point denominator (10_000 == 100%) that fee and
threshold values are expressed against, so off-chain code can read it
from the deployed contract instead of hardcoding it (#652).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_token_update_delay_secs

```rust
get_token_update_delay_secs() -> u64
```

Returns the timelock delay (seconds) currently enforced by
`propose_token_update`: the admin override if one has been set via
`set_token_update_delay_secs`, otherwise the compiled-in
`TOKEN_UPDATE_DELAY_SECS` default (#650, #652).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_min_campaign_funding_goal

```rust
get_min_campaign_funding_goal() -> i128
```

Returns the minimum funding goal for new campaigns (admin override or
`CAMPAIGN_FUNDING_GOAL_MIN`).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_max_campaign_funding_goal

```rust
get_max_campaign_funding_goal() -> i128
```

Returns the maximum funding goal for new campaigns (admin override or
`CAMPAIGN_FUNDING_GOAL_MAX`).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_min_voting_balance

```rust
get_min_voting_balance() -> i128
```

Returns the minimum token balance required to vote.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_approve_votes

```rust
get_approve_votes(campaign_id: u32) -> u32
```

Returns the number of approve votes on `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_reject_votes

```rust
get_reject_votes(campaign_id: u32) -> u32
```

Returns the number of reject votes on `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### has_voted

```rust
has_voted(campaign_id: u32, voter: Address) -> bool
```

Whether `voter` has already voted on `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_min_votes_quorum

```rust
get_min_votes_quorum() -> u32
```

Returns the global voting quorum (admin override or the default).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_approval_threshold_bps

```rust
get_approval_threshold_bps() -> u32
```

Returns the global approval threshold in basis points (admin override
or the default).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_personal_cap

```rust
get_personal_cap(campaign_id: u32, contributor: Address) -> i128
```

Returns `contributor`'s personal cap for `campaign_id`; `0` means no
personal cap is set.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaign_reserve

```rust
get_campaign_reserve(campaign_id: u32) -> Option<CampaignReserve>
```

Returns the vesting reserve held back for `campaign_id`, if any.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaign_payout_marker

```rust
get_campaign_payout_marker(campaign_id: u32) -> Option<u32>
```

Returns the payout marker stored for `campaign_id`, or `None` if none
has been recorded.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### has_pending_campaign_transfer

```rust
has_pending_campaign_transfer(campaign_id: u32) -> bool
```

Whether `campaign_id` has a pending ownership nomination. Returns
`false` for unknown campaigns.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### is_campaign_creator

```rust
is_campaign_creator(campaign_id: u32, creator: Address) -> bool
```

Checks whether `creator` owns `campaign_id` in O(1) via the creator reverse
index, without scanning the creator's campaign bucket (#478).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### list_campaigns

```rust
list_campaigns(start: u32, limit: u32) -> (soroban_sdk::Vec<Campaign>, u32)
```

Returns up to `limit` campaigns (capped at `LIST_MAX_LIMIT`) with ids
after the exclusive cursor `start`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### list_active_campaigns

```rust
list_active_campaigns(start: u32, limit: u32) -> (soroban_sdk::Vec<Campaign>, u32)
```

Returns up to `limit` active campaigns with ids after `start`, plus the
next cursor to pass as `start`; `0` means there are no more pages.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaigns_by_category

```rust
get_campaigns_by_category(category: Category, offset: u32, limit: u32) -> (soroban_sdk::Vec<Campaign>, u32)
```

`offset` is a **zero-based positional index** into this category's
campaign list, not a campaign ID — unlike `list_campaigns`'s `start`,
which is an exclusive campaign-ID cursor. Page by advancing
`offset += page.len()`; the two cursor styles are not interchangeable
(#845).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaigns_by_tag

```rust
get_campaigns_by_tag(tag: String, offset: u32, limit: u32) -> (soroban_sdk::Vec<Campaign>, u32)
```

Campaigns carrying `tag`, in the order they were tagged (#798).
`offset` is a zero-based positional index into the tag's campaign
list, with the same cursor semantics as `get_campaigns_by_category`.
Tags are matched exactly (case-sensitive); an unknown, empty or
over-long tag returns an empty page.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaign_tags

```rust
get_campaign_tags(campaign_id: u32) -> soroban_sdk::Vec<String>
```

Tags applied to a campaign, in the order they were added (#798).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_creator_campaigns

```rust
get_creator_campaigns(creator: Address, start: u32, limit: u32) -> (soroban_sdk::Vec<Campaign>, u32)
```

`start` is a **zero-based positional index** into this creator's
campaign list, not a campaign ID — unlike `list_campaigns`'s `start`,
which is an exclusive campaign-ID cursor. Page by advancing
`start += page.len()`; the two cursor styles are not interchangeable
(#845).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_platform_stats

```rust
get_platform_stats() -> PlatformStats
```

Returns platform-wide aggregate statistics.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_platform_report

```rust
get_platform_report() -> PlatformReport
```

Returns the platform report (aggregate figures for dashboards).

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_creator_stats

```rust
get_creator_stats(creator: Address) -> CreatorStats
```

Returns aggregate statistics for campaigns owned by `creator`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_campaign_stats

```rust
get_campaign_stats(campaign_id: u32) -> CampaignStats
```

Returns aggregate statistics for `campaign_id`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_contributor_portfolio

```rust
get_contributor_portfolio(contributor: Address, start: u32, limit: u32) -> (soroban_sdk::Vec<(u32, i128, String, bool)>, u32)
```

Returns a page of `contributor`'s positions as
`(campaign_id, amount, status, refundable)` tuples, where `status` is one
of `active`, `verified`, `inactive`, `withdrawn` or `cancelled`. The
returned `u32` is the next cursor to pass as `start`; `0` means done.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### save_campaign

```rust
save_campaign(user: Address, campaign_id: u32) -> Result<(), Error>
```

Saves `campaign_id` to `user`'s on-chain bookmark list. Requires
`user`'s authorization.

- **Auth:** `user` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### batch_save_campaigns

```rust
batch_save_campaigns(user: Address, campaign_ids: soroban_sdk::Vec<u32>) -> Result<(), Error>
```

Saves multiple `campaign_ids` to `user`'s on-chain bookmark list in a
single transaction. Requires `user`'s authorization. The batch is
atomic — any invalid id reverts the whole call.

- **Auth:** `user` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### remove_saved_campaign

```rust
remove_saved_campaign(user: Address, campaign_id: u32) -> Result<(), Error>
```

Removes `campaign_id` from `user`'s on-chain bookmark list. Requires
`user`'s authorization.

- **Auth:** `user` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### clear_saved_campaigns

```rust
clear_saved_campaigns(user: Address) -> Result<(), Error>
```

Removes every bookmark from `user`'s on-chain bookmark list in a single
transaction. Requires `user`'s authorization.

- **Auth:** `user` (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### get_saved_campaigns

```rust
get_saved_campaigns(user: Address) -> soroban_sdk::Vec<u32>
```

Returns the list of campaign ids `user` has bookmarked, in the order
they were saved, excluding those bookmarked to campaigns that have
since been cancelled.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_saved_campaigns_count

```rust
get_saved_campaigns_count(user: Address) -> u32
```

Returns the number of `user`'s live (non-cancelled) bookmarks.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### censure_comment

```rust
censure_comment(campaign_id: u32, comment_hash: BytesN<32>, reason: String) -> Result<(), Error>
```

Record that an off-chain comment was removed. Admin only.

Comments themselves stay off-chain; what lands here is the immutable
evidence that one was suppressed, so moderation cannot be silent.
Idempotent — re-censuring the same hash is a no-op and emits no second
event.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### uncensure_comment

```rust
uncensure_comment(campaign_id: u32, comment_hash: BytesN<32>) -> Result<(), Error>
```

Lift a censure, restoring a comment to displayable. Admin only.

Emits its own event so a censure-then-revert cannot be used to hide
that the suppression ever happened.

- **Auth:** stored `admin` (from `get_admin`) (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.

### is_comment_censured

```rust
is_comment_censured(campaign_id: u32, comment_hash: BytesN<32>) -> bool
```

Whether a comment is currently censured — the check a frontend makes
before rendering. Total: an unknown hash reads as not censured.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_comment_censure

```rust
get_comment_censure(campaign_id: u32, comment_hash: BytesN<32>) -> Option<CommentCensure>
```

The censure record (reason, timestamp, acting admin), or `None`.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).

### get_censured_comment_count

```rust
get_censured_comment_count(campaign_id: u32) -> u32
```

How many comments have been censured on a campaign.

- **Auth:** no auth (per [AUTHORIZATION.md](AUTHORIZATION.md))
- **Errors:** none (infallible).
