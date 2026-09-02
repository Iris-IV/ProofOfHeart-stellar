# Storage TTL Policy & Rent Management

## Overview

Soroban persistent storage entries require periodic Time-To-Live (TTL) extension to prevent state eviction. To avoid rent inflation and unneeded ledger write transactions, ProofOfHeart-stellar follows an explicit TTL management policy.

---

## Core Principles

1. **Write-Path Extensions Only**:
   - Persistent storage TTL is extended during **state-mutating operations** (e.g. `create_campaign`, `contribute`, `verify_campaign`).
   - Read-only functions (e.g. `get_campaign`, `get_contribution`) return stored values without invoking `extend_ttl`, keeping read queries cost-neutral.

2. **Threshold & Extension Values (`src/storage.rs`)**:
   - `INSTANCE_TTL_THRESHOLD`: `535_680` ledgers (~31 days)
   - `INSTANCE_TTL_EXTEND`: `1_071_360` ledgers (~62 days)
   - `PERSISTENT_TTL_THRESHOLD`: `172_800` ledgers (~10 days)
   - `PERSISTENT_TTL_EXTEND`: `518_400` ledgers (~30 days)

---

## TTL Extension Rules by Data Key Category

| Storage Category | DataKey | Threshold (Ledgers) | Extension Target (Ledgers) | Trigger Path |
|---|---|---|---|---|
| **Instance Data** | `DataKey::Admin`, `DataKey::Paused` | 535,680 | 1,071,360 | Contract Admin Init / Config Update |
| **Campaign Entry** | `DataKey::Campaign(id)` | 172,800 | 518,400 | `create_campaign()`, `finalize_campaign()` |
| **User Contribution** | `DataKey::Contribution(user, campaign)` | 172,800 | 518,400 | `contribute()`, `claim_refund()` |
| **Governance Vote** | `DataKey::Vote(voter, campaign)` | 172,800 | 518,400 | `cast_vote()` |

---

## Extension Guidelines for Contributors

When introducing new storage entries:
1. Wrap storage access using helper functions in `src/storage.rs`.
2. Always pair mutating operations with `env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND)`.
3. Never invoke `extend_ttl` inside read-only getter functions.
