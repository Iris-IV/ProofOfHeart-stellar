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

### 6) Expired / Failed (derived)
- If the deadline passes and the campaign did not reach its goal (`Expired/Failed` derived condition), contributors can claim refunds via `claim_refund`.
- The contract does not currently toggle `is_active` automatically when a deadline passes; "expired" is computed at call time using the ledger timestamp.
