# Campaign Lifecycle

## States

```
Created → Active → Verified (optional) → Withdrawn (goal met)
                                           → Refunded (goal not met / cancelled)
```

A campaign progresses through the following states:

1. **Created** — After `create_campaign()`. Starts active.
2. **Active** — Accepting contributions until the deadline.
3. **Verified** — (Optional) Admin `verify_campaign()` or community `verify_campaign_with_votes()`.
4. **Withdrawn** — Creator calls `withdraw_funds()` after `amount_raised >= funding_goal`. Campaign becomes inactive.
5. **Cancelled** — Creator calls `cancel_campaign()`. Campaign becomes inactive; contributors can `claim_refund()`.
6. **Expired** — Deadline passes without meeting the goal. Contributors can `claim_refund()`.

## Pause Mechanism

The contract has two independent pause flags:

### Manual Pause (`DataKey::Paused`)

- Set by admin via `pause()`.
- Cleared by admin via `unpause()`.
- Emits `contract_paused` / `contract_unpaused`.

### Auto-Pause (`DataKey::AutoPaused`)

Automatically set on either of two anomaly triggers during `contribute()`:

- **Huge contribution** — A single contribution exceeds 200% of the campaign's `funding_goal` (`amount * 10000 > funding_goal * 20000`). Emits `("auto_paused",)` with `("huge_contribution", amount)`.
- **Burst** — More than 10 contributions to the same campaign in a single ledger (block). Emits `("auto_paused",)` with `("burst", block_count)`.

In both cases the contribution is rejected (`ContractPaused` error) and the storage write is rolled back, so `AutoPaused` never persists in production — the flag is always cleared on the next successful call. This caveat is important for indexers.

- Blocks all state-changing operations (same as manual pause).
- Cleared by:
  - **`unpause()`** — Admin can always clear the auto-pause flag, even if the triggering campaign is no longer active.
  - **`resume_campaign(campaign_id)`** — Admin clears the flag, but only if the referenced campaign is still active (not cancelled/expired).

### Why two flags?

Using separate flags provides a clearer audit trail — indexers can distinguish between an admin-initiated pause and an automatic safety pause. The admin can always recover the contract via `unpause()`, even when `resume_campaign()` is blocked (e.g., the triggering campaign was cancelled).

### Recovery Scenarios

| Scenario | Recovery |
|----------|----------|
| Burst contribution triggers auto-pause; campaign is still active | `resume_campaign(campaign_id)` or `unpause()` |
| Burst contribution triggers auto-pause; campaign was cancelled | `unpause()` only (`resume_campaign` fails with `CampaignNotActive`) |
| Admin pauses manually | `unpause()` |
# Campaign Lifecycle (State Machine)

Campaigns are represented by a `Campaign` struct with these key state flags:

- `is_active`: whether the campaign is accepting actions as "open"
- `is_cancelled`: whether the creator has cancelled the campaign
- `funds_withdrawn`: whether the creator has withdrawn raised funds
- `is_verified`: whether the campaign has been verified (admin or community vote)

Additional derived conditions used by the contract:

- **Funded**: `amount_raised >= funding_goal`
- **Expired/Failed**: `ledger.timestamp() > deadline && amount_raised < funding_goal`

## States

### 1) Active (unverified)

- Set on `create_campaign`: `is_active = true`, `is_cancelled = false`, `funds_withdrawn = false`, `is_verified = false`.
- Contributions are blocked until verified: `contribute` returns `CampaignNotVerified` while `is_verified = false`.
- Creator can still update/cancel while active (subject to each method's rules).
- Full `update_campaign` edits are only available before verification and before any contributions.

### 2) Active + Verified

- Reached by either:
  - `verify_campaign` (admin verification), or
  - `verify_campaign_with_votes` (community verification after quorum + threshold).
- Once verified, contributions are allowed until the deadline, as long as `is_active = true` and `is_cancelled = false`.
- Re-verification errors are intentionally path-specific: `verify_campaign` returns `AdminVerificationConflict` and `verify_campaign_with_votes` returns `CommunityVerificationConflict` when the campaign is already verified. Voting on an already verified campaign still returns `CampaignAlreadyVerified`.
- After verification, `update_campaign` is blocked so the verified title/description cannot be changed without re-review.

### 3) Funded (derived)

- When `amount_raised >= funding_goal`, the campaign is considered funded.
- The contract does not set a dedicated boolean for "funded"; it is checked when withdrawing.
- The creator may call `withdraw_funds` once funded (and if not cancelled / not previously withdrawn).

### 4) Withdrawn / Closed

- Reached by `withdraw_funds`:
  - sets `funds_withdrawn = true`
  - sets `is_active = false`
- After this point:
  - `withdraw_funds` is blocked by `FundsAlreadyWithdrawn`
  - `cancel_campaign` is blocked by `CancellationNotAllowed`

### 5) Cancelled

- Reached by `cancel_campaign` (creator only):
  - sets `is_cancelled = true`
  - sets `is_active = false`
- Contributors can claim refunds via `claim_refund` after cancellation (if they contributed).
- Successful refunds remove the contributor's stored contribution record instead of leaving a zero-value entry behind.
- Refunds also reduce the campaign's live contribution denominator used for revenue sharing, ensuring remaining contributors receive the correct pro-rata share of future revenue claims.

### 6) Expired / Failed (derived)

- If the deadline passes and the campaign did not reach its goal (`Expired/Failed` derived condition), contributors can claim refunds via `claim_refund`.
- The contract does not currently toggle `is_active` automatically when a deadline passes; "expired" is computed at call time using the ledger timestamp.
