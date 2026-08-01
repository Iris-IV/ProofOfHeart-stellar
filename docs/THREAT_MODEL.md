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
