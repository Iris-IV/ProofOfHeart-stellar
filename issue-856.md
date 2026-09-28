# Issue #856: Missing old vesting values in withdraw event

## Summary

In `src/campaigns/withdraw.rs`, the logic that updates the global vesting configuration changes the delay and reserve parameters, but the emitted event does not include the previous values. As a result, governance participants and indexers cannot audit what changed between the old and new settings.

This makes it difficult to review governance decisions, reconstruct campaign state transitions, and confirm whether the update was a legitimate configuration change.

## Problem

When the vesting parameters are updated, the contract emits a change event, but the payload only reports the new values. The previous delay and reserve values are omitted.

For example, if the system changes:

- old delay = 30 days
- new delay = 60 days
- old reserve = 1000
- new reserve = 2000

The event should record both the old and new values to support auditing and indexing.

## Root cause

The event payload in the withdraw/update flow is written with only the post-update values, so the historical state is lost at the moment the event is emitted.

The governance and downstream event consumers rely on the event data to answer:

- what was the previous delay?
- what is the new delay?
- what was the previous reserve?
- what is the new reserve?

Without those fields, the audit trail is incomplete.

## Expected behavior

The emitted event should include both the old and new values for the vesting configuration, specifically:

- old delay
- new delay
- old reserve
- new reserve

This ensures that the event is self-describing and can be used to trace historical updates without recomputing state from off-chain sources.

## Acceptance criteria

- Update the withdraw/update event payload to emit the previous and new vesting delay values.
- Update the withdraw/update event payload to emit the previous and new vesting reserve values.
- Ensure the emitted data is consistent with the actual values being applied in `src/campaigns/withdraw.rs`.
- Preserve backward compatibility where appropriate, while making the audit data available to governance and indexers.

## Notes

This issue specifically affects governance review and indexer auditability for vesting parameter updates in the withdraw flow.
