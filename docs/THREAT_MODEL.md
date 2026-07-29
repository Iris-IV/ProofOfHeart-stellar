# Threat Model: ProofOfHeart

This document outlines known security considerations and limitations of the ProofOfHeart smart contract.

## Voting System

### Token-weighted Sybil Attack (#177)

**Description**: The current voting system uses a token-weighted model where a voter's influence is determined by their token balance at the time of voting. While the contract prevents an address from voting multiple times (`HasVoted` check), it does not prevent a user from transferring tokens between multiple addresses to vote repeatedly with the same capital.

**Attack Vector**:
1. An attacker holds 1,000,000 tokens in Address A.
2. Attacker votes with Address A (weight: 1,000,000).
3. Attacker transfers 1,000,000 tokens from Address A to Address B.
4. Attacker votes with Address B (weight: 1,000,000).
5. The process can be repeated across any number of addresses.

**Mitigation Status**:
This is a **known limitation** of the current implementation. A robust fix would require a consistent "ledger snapshot" of token balances at a specific point in time (e.g., campaign creation), which is not natively supported by standard SEP-41 token contracts without a specialized history oracle or custom token logic.

**Risk Management**:
- **Monitoring**: Integration layers (frontends/indexers) should monitor for large token transfers between addresses that subsequently vote on the same campaign.
- **Minimum Balance**: The `MinVotingBalance` setting helps increase the cost of creating multiple voting accounts but does not prevent the attack by a sufficiently capitalized entity.
- **Future Improvements**: Future versions may explore integration with specialized governance tokens that support historical snapshots.

## Withdrawals & Funds Management

### Vesting Reserve Admin Power (#493)

**Description**: `set_vesting_params` lets the admin set a withdrawal release delay (`delay_days`, up to 365) and a reserve percentage (`reserve_bps`, up to `BPS_DENOMINATOR`, i.e. up to 100%) that apply to every subsequent `withdraw_funds` call. Because these are global settings read at withdrawal time (not snapshotted per-campaign at creation), an admin can change them at any point, including to a maximally punitive combination (e.g. 365-day delay with a 99.99% reserve), and the new values immediately apply to campaigns that were created and funded under a more favorable — or no — vesting policy.

**Attack Vector**:
1. A campaign is created and funded to goal while `delay_days == 0` and `reserve_bps == 0` (vesting disabled).
2. Before the creator calls `withdraw_funds`, the admin calls `set_vesting_params` with `delay_days = 365` and `reserve_bps` near `BPS_DENOMINATOR`.
3. The creator withdraws and receives only a sliver of `total_after_fee` immediately; the remainder is locked in `CampaignReserve` for up to a year, with no per-campaign override.
4. Even a creator who has already reached `funding_goal` has no way to "lock in" the vesting terms that were in effect when contributors backed the campaign.

**Mitigation Status**:
This is a **known limitation** of the current implementation. `set_vesting_params` is intentionally admin-gated (`assert_admin`) and rejects internally inconsistent values (`reserve_bps > BPS_DENOMINATOR`, `delay_days > 365`, or a nonzero reserve paired with a zero delay via `Error::InvalidVestingDelay`), but it does not, and currently cannot, protect a specific campaign's withdrawal terms from a later admin policy change, since vesting parameters are stored as a single global pair (`WithdrawReleaseDelayDays`, `WithdrawReservePercentage`) rather than snapshotted onto `Campaign` at creation time.

**Risk Management**:
- **Trust assumption**: The admin key is already a fully trusted role in this contract (it also controls `platform_fee`, `creation_disabled`, campaign verification, and pausing) — this is consistent with, not an escalation beyond, the existing admin trust model.
- **Monitoring**: The `vesting_params_updated` / `vesting_disabled` events give integrators an on-chain signal to alert creators and contributors whenever the policy changes, so a sudden tightening ahead of expected withdrawals can be flagged.
- **Future Improvements**: A future version could snapshot `delay_days`/`reserve_bps` onto each `Campaign` at creation (mirroring how `fee_override` already pins a per-campaign fee independent of the global `platform_fee`), so in-flight campaigns are unaffected by later global changes.

## Contribution Caps

### Personal Cap Self-DoS on Refund Path (#493)

**Description**: `set_personal_cap` lets a contributor voluntarily lower their own per-campaign contribution ceiling (bounded above by `Campaign::max_contribution_per_user` when that field is nonzero). This cap is enforced on `contribute` but is not consulted by `claim_refund`, `claim_revenue`, or any withdrawal path — those only read the contributor's stored `Contribution`/`LifetimeContribution` amounts. A contributor cannot use `set_personal_cap` to block their own refunds or revenue claims; the cap only ever restricts future contributions.

**Attack Vector (self-inflicted, not third-party)**:
1. A contributor sets an unusually low personal cap (e.g. `0` or `1`) via `set_personal_cap` after already having contributed a larger amount.
2. The contributor cannot "un-contribute" past funds — `set_personal_cap` only rejects *future* `contribute` calls above the new cap; it never rewrites `Contribution`/`LifetimeContribution`.
3. If the campaign later fails or is cancelled, `claim_refund` still reads and refunds the full stored contribution amount, unaffected by the personal cap. Likewise `claim_revenue` computes payouts from `effective_amount_raised`/contribution share, not the personal cap.

**Mitigation Status**:
**Not a vulnerability** under the current design: `set_personal_cap` only gates the `check_contribution_caps` / personal-cap comparison inside `contribute` (`current + amount > cap`). `claim_refund` and revenue-claim paths never read `PersonalCap` storage, so there is no code path by which a contributor's own cap can lock up their own refund. This entry documents the boundary explicitly so future changes to the refund/revenue paths don't accidentally start consulting the personal cap and reintroduce a self-DoS.

**Risk Management**:
- **Invariant to preserve**: Refund (`claim_refund`) and revenue claim (`claim_revenue`) logic must never gate on `PersonalCap` — only on the actual recorded `Contribution` and `RevenueClaimed` amounts.
- **Testing**: Regression tests should cover "contributor lowers their own personal cap after contributing, then successfully claims a full refund" to guard this invariant across future refactors.

## Admin Power Concentration (#468)

### Single-Key Admin Control Over Sensitive Operations

**Description**: The stored admin address (`Admin`) controls a broad set of sensitive contract operations with no timelock, multisig, or governance delay:

- **Fee override**: `update_platform_fee` and `set_campaign_fee_override` can set per-campaign or global fees anywhere between 0 and `PLATFORM_FEE_ABSOLUTE_MAX_BPS` (10 000 bps = 100%), including 0%, which would leave no platform revenue for the protocol.
- **Forced verification**: `verify_campaign` and `verify_campaigns` bypass community voting, granting immediate verification (and thus withdrawal eligibility) to any campaign the admin chooses.
- **Pause/Unpause**: `pause` and `unpause` freeze or unfreeze the entire contract, halting all contributions, withdrawals, and state-changing operations.
- **Campaign creation gate**: `set_creation_disabled` can block (or re-enable) all new campaign creation while leaving existing campaigns operational.
- **Vesting parameters**: `set_vesting_params` sets the global vesting reserve percentage and release delay.
- **Funding goal limits**: `set_min_campaign_funding_goal` and `set_max_campaign_funding_goal` cap the range of acceptable funding goals for new campaigns.
- **Voting parameters**: `set_voting_params`, `set_min_voting_balance`, `set_category_voting_threshold`, and `set_category_duration_cap` adjust the community voting and campaign-creation policies.
- **Token migration**: `propose_token_update` begins a 7-day timelocked migration to a new token address; `accept_token_update` finalises it (once all campaigns are terminal). See Token Migration below for the timelock details.
- **Admin transfer**: `initiate_admin_transfer` begins a two-step admin ownership transfer; `accept_admin_transfer` completes it.
- **Voting state purge**: `purge_voting_state` removes per-voter and aggregate vote records for a terminal campaign.

**Attack Vector**:
A compromised admin key allows an attacker to:

1. **Force-verify a fraudulent campaign**: Grant verification to a campaign the attacker controls, bypassing community review. Once verified, the attacker can withdraw escrowed contributions (minus platform fees). Combined with a zero-fee override (`set_campaign_fee_override` to 0), the attacker receives the full raised amount.
2. **Set fees to zero**: Remove all protocol revenue by calling `update_platform_fee(0)`, potentially as a distraction or to starve the platform operator.
3. **Extract tokens via migration**: Propose and accept a token migration to a contract the attacker controls, rendering all existing token balances held by the contract worthless and enabling future contributions in a malicious token.
4. **Lock the contract**: Call `pause` to freeze all state-changing operations indefinitely, preventing creators from withdrawing funds and contributors from claiming refunds.
5. **Combine with vesting**: Set a high reserve percentage and long delay to lock creator payouts from future withdrawals, then force-verify their own campaign to extract whatever immediate fraction remains.

**Mitigation Status**:
This is a **known architectural limitation** of the current single-admin model. Every operation listed above is protected by `assert_admin`/`Admin.require_auth()`, meaning a compromise of the single stored `Admin` private key is sufficient to execute any of them. The contract has no built-in multisig, role-based access control (e.g. separate "operator" and "governance" roles), or timelock on sensitive operations (with the exception of `propose_token_update` → `accept_token_update`, which enforces a 7-day delay).

**Risk Management**:
- **Key hygiene**: The admin private key SHOULD be held in a multisig wallet (e.g., a Soroban-compatible multisig contract) or a hardware security module rather than a single address controlled by one individual. The contract is designed to interoperate with a wrapper multisig; `initiate_admin_transfer` and `accept_admin_transfer` enable handover to such a setup.
- **Monitoring**: Every admin action emits an indexed event (`fee_updated`, `campaign_fee_override_set`, `campaign_created`, `contract_paused`, `token_update_proposed`, etc.) that indexers and monitoring systems can alert on. An unexpected fee change or forced verification should trigger immediate investigation.
- **Transparency commitments**: Operators SHOULD disclose the admin address(es) publicly and use predictable, on-chain governance windows (e.g., announcing planned fee changes via off-chain channels before executing them).
- **Future Improvements**: Long-term hardening paths include:
  - **Soroban multisig**: Deploying the admin key behind a threshold-signature scheme or a Soroban multisig wallet contract so that no single key compromise is sufficient for sensitive operations.
  - **Timelock**: Introducing a mandatory delay (e.g., 48–72 hours) between proposing and executing sensitive operations such as fee changes, forced verification, and token migration, giving users and monitoring systems time to react.
  - **Role separation**: Splitting admin into distinct roles (e.g., an "operator" role limited to pausing/resuming and a "governance" role for parameter changes), each with its own key.

### Token Migration Timelock (#407, #551, #562)

**Description**: `propose_token_update` stores the new token address and a `release_after` timestamp set 7 days in the future (`TOKEN_UPDATE_DELAY_SECS = 7 * 86400`). `accept_token_update` refuses to execute until `env.ledger().timestamp() >= release_after`. This gives contributors and creators a 7-day window to withdraw, claim refunds, or react before the contract's token address changes.

**Additional Safeguard**: Even after the timelock expires, `accept_token_update` also requires `get_active_campaign_count(env) == 0 && get_total_raised_global(env) == 0`, i.e. no campaign may have outstanding escrowed funds. This prevents a token swap from stranding balances in the old token.

**Cancellation**: `cancel_token_update` lets the admin abort a proposed update at any point before acceptance, in case the proposal was made in error or circumstances change.

## Campaign Creation

### Category Duration Cap Below Existing Deadlines (#493)

**Description**: `set_category_duration_cap` lets the admin set a per-category maximum `duration_days` for *future* `create_campaign` calls (checked against `crate::CAMPAIGN_DURATION_MIN_DAYS..=duration_max` in `create_campaign`). This cap only constrains new campaigns at creation time — it is never re-checked against, or applied to, campaigns that already exist. An admin lowering a category's cap below the remaining time on already-active campaigns in that category does not shorten, invalidate, or otherwise affect those campaigns' `deadline` fields.

**Attack Vector**:
1. Category `Learner` has no explicit cap, so campaigns may run up to `CAMPAIGN_DURATION_MAX_DAYS` (365 days). A campaign is created with a 300-day duration; its `deadline` is fixed at creation via `calculate_deadline`.
2. The admin later calls `set_category_duration_cap` for `Learner` with `max_days = 30`.
3. The existing 300-day campaign's stored `deadline` is untouched — `Campaign.deadline` is only read, never recomputed against the current cap, by any lifecycle path (`require_active_campaign`, `contribute`, `withdraw_funds`, etc.).
4. New campaigns in `Learner` are now limited to 30 days, while the old one continues accepting contributions on its original, longer schedule until its original deadline.

**Mitigation Status**:
This is **expected, non-breaking behavior**: applying a lowered cap retroactively would require unilaterally shortening a `deadline` that contributors relied on when deciding to fund the campaign, which would be a strictly worse outcome (a de facto un-consented early cutoff) than leaving existing campaigns on their original terms. The one-way check at creation time (`duration_max = get_category_duration_cap(...).unwrap_or(CAMPAIGN_DURATION_MAX_DAYS)`) is intentional.

**Risk Management**:
- **Documentation**: This entry exists so cap changes are understood as prospective-only; operators should not assume lowering a category cap affects campaigns already in flight.
- **Monitoring**: The `category_duration_cap_set` / `category_duration_cap_removed` events let indexers flag the change and, if desired, surface a "grandfathered under a previous, longer cap" indicator on affected campaigns in a frontend.
- **Future Improvements**: If retroactive shortening is ever desired (e.g. for abuse response), it should be a distinct, explicitly audited admin action — such as `force_shorten_deadline` — rather than an implicit side effect of `set_category_duration_cap`, to keep the two concerns (future policy vs. existing campaign state) separate.
