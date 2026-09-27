# ADR-001: Instance vs. Persistent Storage Split

## Status

Accepted

## Context

The contract keeps its state in two Soroban storage planes (`src/storage.rs`):

- **Instance storage** (`env.storage().instance()`), used for global,
  singleton-style configuration: admin address, accepted token, platform fee,
  funding-goal bounds, voting thresholds, per-category overrides, aggregate
  counters (`CampaignCount`, `ActiveCampaignCount`, `VerifiedCampaignCount`,
  `CancelledCampaignCount`, `TotalRaised`), and the withdrawal-vesting config.
- **Persistent storage** (`env.storage().persistent()`), used for per-entity
  records that grow with usage: campaigns, contributions, personal caps,
  revenue pools/claims, voting records, creator/category campaign-ID
  buckets, and wallet saved campaign bookmarks (`BookmarkKey::SavedCampaigns(Address)`).

Instance storage shares one TTL for the whole contract instance and is bumped
via `bump_instance_ttl` (called once per invocation from `lib.rs`). Persistent
storage entries each carry their own TTL; every write path uses the
`persistent_set!` macro (`storage.rs:12`), which sets the value and calls
`extend_ttl` in the same step so a TTL bump can't be forgotten on new code
paths. This split, and the TTL values (`BUMP_THRESHOLD` = 7 days ledgers,
`BUMP_AMOUNT` = 400 days) were never written down, so a new contributor
adding storage has no documented rule for which plane to use or what
happens if a TTL lapses.

## Decision

1. **Instance storage** holds only state that is small, singleton per
   contract, and read on nearly every invocation (admin/config/aggregate
   counters). Its TTL is bumped unconditionally once per call via
   `bump_instance_ttl`, so these keys never expire as long as the contract
   receives traffic.
2. **Persistent storage** holds everything keyed by an entity (campaign ID,
   `(campaign_id, Address)`, category, creator, wallet address for bookmarks
   `BookmarkKey::SavedCampaigns(Address)`) because this data is
   unbounded and must be independently rent-priced and independently
   expirable. Every setter for persistent state goes through
   `persistent_set!` so TTL extension is structural, not something callers
   opt into.
3. **TTL values**: `BUMP_THRESHOLD = 7 * DAY_IN_LEDGERS` (extend once the
   remaining TTL drops under ~7 days) and `BUMP_AMOUNT = 400 * DAY_IN_LEDGERS`
   (extend out to ~400 days). This mirrors [`STORAGE_TTL_POLICY.md`](STORAGE_TTL_POLICY.md):
   TTL is extended on writes only, so read/view calls stay rent-neutral.

## Consequences

- **If a persistent entry's TTL is allowed to lapse** (no write touches it for
  >400 days and no other write triggers eviction protection), the entry is
  archived by the network and reads return `None`/default via the
  `unwrap_or(...)` fallbacks already present on every persistent getter in
  `storage.rs`. This is why every persistent getter has an explicit default
  or `Option` return instead of panicking — an expired-and-restored key must
  behave like a never-written one.
- **Instance storage has no per-key TTL**, so an inactive contract (no calls
  at all) can still have its instance entries expire. Recovering from that
  requires a `restore` operation before the next invocation; this is a
  platform-level operational concern, not something the contract code
  mitigates.
- Adding a new persistent key *must* go through `persistent_set!` (or
  otherwise call `extend_ttl` explicitly) — a plain `env.storage().persistent().set()`
  will silently create the entry with the storage minimum TTL, which is a
  correctness bug of the exact resurfacing kind this ADR is meant to prevent
  future contributors from reintroducing.
- Adding a new instance key needs no explicit TTL handling since
  `bump_instance_ttl` covers the whole instance uniformly, but it does mean
  instance storage should stay small — every key in it is paid for even if
  never read again.
