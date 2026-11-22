# Storage TTL Policy & Rent Management

## Overview

Soroban persistent storage entries require periodic Time-To-Live (TTL) extension to prevent state eviction. To avoid rent inflation and unneeded ledger write transactions, ProofOfHeart-stellar follows an explicit TTL management policy.

---

## Core Principles

1. **Write-Path Extensions Only**:
   - Persistent storage TTL is extended during **state-mutating operations** (e.g. `create_campaign`, `contribute`, `verify_campaign`).
   - Read-only functions (e.g. `get_campaign`, `get_contribution`) return stored values without invoking `extend_ttl`, keeping read queries cost-neutral.

2. **Threshold & Extension Values (`src/storage.rs`)**:
   - Persistent keys are refreshed when their TTL is below `120_960` ledgers (~7 days), extending them to `6_912_000` ledgers (~400 days).

---

## TTL Extension Rules by Data Key Category

| Storage Category | DataKey | Threshold (Ledgers) | Extension Target (Ledgers) | Trigger Path |
|---|---|---|---|---|
| **Instance Data** | `AdminKey::Admin`, `AdminKey::Paused` | 120,960 | 6,912,000 | Contract Admin Init / Config Update |
| **Campaign Entry** | `CampaignKey::Campaign(id)` | 120,960 | 6,912,000 | `create_campaign()`, `finalize_campaign()`, `bump_campaign_state()` |
| **User Contribution** | `ContributionKey::Contribution(user, campaign)` | 120,960 | 6,912,000 | `contribute()`, `claim_refund()` |
| **Governance Vote** | `VotingKey::HasVoted(voter, campaign)` | 120,960 | 6,912,000 | `cast_vote()` |

---

## Permissionless Campaign Keep-Alive

`bump_campaign_state(env, campaign_id)` is permissionless and can be called by
off-chain keepers or the frontend when a long-running campaign needs its
persistent entries kept alive. It refreshes the campaign record and related
campaign-scoped indexes, milestones, reserve, token, contributor-count, and
aggregate voting entries that exist; missing entries are skipped, so an
unknown campaign ID is a no-op. It does not extend instance-storage TTL.

The call checks up to 12 persistent keys. Each existing key incurs a presence
check and a TTL extension, so resource use and rent scale with the number of
stored entries refreshed. Callers should use it as needed for long-running
campaigns rather than polling continuously.

## Extension Guidelines for Contributors

When introducing new storage entries:
1. Wrap storage access using helper functions in `src/storage.rs`.
2. Always pair mutating operations with `env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND)`.
3. Never invoke `extend_ttl` inside read-only getter functions.
