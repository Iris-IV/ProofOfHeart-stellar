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

- Automatically set when a single contribution exceeds the campaign's `funding_goal` (burst contribution).
- Emits `auto_paused` event.
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
