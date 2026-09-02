# Fix: contributor TTL metadata expiry for long-lived campaigns

## Summary

Issue #875: contributor metadata persisted with a campaign-specific key can expire while the campaign remains active.

`src/contributions.rs` calls `bump_instance_ttl(env)` during contribution flows, but the persistent contributor records for lifetime totals and per-campaign caps are stored under separate keys and were not receiving the same TTL extension. Over time, a long-lived campaign can lose the contributor cap/lifetime metadata even though the campaign is still active.

## Root cause

The contract extended TTL only on the instance ledger entry, while the contributor-specific persistent keys remained untouched unless they were read via the accessor helpers. That left these entries vulnerable to expiry:

- `ContributionKey::Contribution(campaign_id, contributor)`
- `ContributionKey::LifetimeContribution(campaign_id, contributor)`
- `ContributionKey::PersonalCap(campaign_id, contributor)`

## Fix

Add a single helper to extend TTL for every persistent contributor key touched by a contribution workflow and invoke it from the contribution paths before persisting updates.

The fix also makes the read helpers extend TTL when the contributor entry is present so that active contributor metadata remains alive for the duration of the campaign lifecycle.

## Verification

Relevant regression coverage was added for a long-running active campaign to confirm the personal cap and lifetime contribution metadata remain available after the TTL window advances.
