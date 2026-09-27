# State expiration & storage rent — maintenance guide

Soroban charges _rent_ to keep ledger entries alive. Every entry has a
**TTL** (time to live, in ledgers). When it reaches zero the entry is
**archived** and the contract can no longer read or write it until someone
restores it. This guide explains how ProofOfHeart keeps its state alive, what
happens when it is not, and what operators do about it.

The values below are read from `src/storage.rs`. That file is the source of
truth; `docs/STORAGE_TTL_POLICY.md` is an older summary whose numbers no longer
match the code.

## 1. Constants

| Name             | Value                      | Meaning                                   |
| ---------------- | -------------------------- | ----------------------------------------- |
| `DAY_IN_LEDGERS` | `17_280`                   | One day at ~5 s per ledger                |
| `BUMP_THRESHOLD` | `7 * 17_280 = 120_960`     | ~7 days: re-extend when TTL falls to this |
| `BUMP_AMOUNT`    | `400 * 17_280 = 6_912_000` | ~400 days: TTL to extend to               |

`extend_ttl(key, threshold, extend_to)` is a no-op while the entry still has
more than `threshold` ledgers left. Otherwise it extends the entry to
`extend_to` ledgers from now. Calling it on every write is therefore cheap:
rent is only paid when a real extension happens.

The network caps how far an entry can be extended. In the Soroban host this
project vendors, extending a **persistent** entry past the network maximum
clamps the TTL to that maximum (a _temporary_ entry would error instead). So
`BUMP_AMOUNT` is an upper bound, not a promise of 400 days — check the
network's `maxEntryTTL` setting.

## 2. What gets extended, and when

All contract state lives in **persistent** and **instance** storage. The
contract uses no temporary storage, so nothing is deleted outright when it
expires; it is archived and can be restored.

**Writes extend TTL automatically.** Persistent entries are written through the
`persistent_set!` macro, which extends the entry both before and after the
write, so a setter cannot forget the bump:

```rust
macro_rules! persistent_set {
    ($env:expr, $key:expr, $value:expr) => {{
        let key = $key;
        let storage = $env.storage().persistent();
        if storage.has(&key) {
            storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        }
        storage.set(&key, $value);
        storage.extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
    }};
}
```

Instance storage (admin, paused flag, counters and other platform config) is
extended by `bump_instance_ttl`, which state-changing entrypoints call:

```rust
pub fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);
}
```

**A few paths also refresh TTL without changing the value:**

- `extend_contributor_ttl` — a contributor's contribution total, lifetime total
  and personal cap (called from contribution and refund paths, and by
  `get_personal_cap` when a cap exists).
- `extend_voting_state_ttl` / `extend_ttl` (voting) — the approval/rejection
  counters, weights and a voter's `HasVoted` record.
- `bump_campaign` — a campaign record.

`has_personal_cap` deliberately does _not_ extend TTL: it is used right before a
cap is removed, and extending an entry you are about to delete wastes rent.

**Plain reads do not extend TTL.** A campaign that nobody writes to (or that
none of the paths above touches) is not refreshed just because people view it.

## 3. What expiry means for users

An entry whose TTL reaches zero is archived, and any invocation whose footprint
includes it **fails** until the entry is restored. In practice:

- An idle campaign can become unusable — contributions, withdrawals and
  refunds against it fail — even though the funds are still held by the contract.
- Instance storage expiring blocks _every_ entrypoint that reads platform
  config, so it matters more than any single campaign.
- Nothing is lost. Archived entries keep their data and come back on restore.

## 4. Operator runbook

Check the exact flags with `stellar contract extend --help` and
`stellar contract restore --help`; they vary slightly by CLI version. Set
`--source-account` and `--network` for your environment.

**Keep the contract instance alive** (do this on a schedule well inside
`BUMP_AMOUNT`, for example monthly):

```bash
stellar contract extend \
  --id "$CONTRACT_ID" \
  --ledgers-to-extend 6912000 \
  --source-account "$OPERATOR" --network "$NETWORK"
```

**Extend one persistent entry**, for example a long-running campaign that has
had no writes:

```bash
stellar contract extend \
  --id "$CONTRACT_ID" \
  --key "$STORAGE_KEY" --durability persistent \
  --ledgers-to-extend 6912000 \
  --source-account "$OPERATOR" --network "$NETWORK"
```

**Restore an archived entry** (or the instance, by omitting `--key`):

```bash
stellar contract restore \
  --id "$CONTRACT_ID" \
  --key "$STORAGE_KEY" --durability persistent \
  --source-account "$OPERATOR" --network "$NETWORK"
```

Restoring lets calls succeed again, but the restored entry may have little TTL
left, so extend it straight afterwards.

Suggested routine:

1. Extend the contract instance (and code) monthly.
2. List campaigns that are still open or hold escrow, and extend any that have
   not been written to recently. `get_campaign` reports the current state.
3. Alert on invocations that fail because an entry is archived, and restore then
   extend the affected entry.

## 5. Rules for contributors

1. Write persistent state through the setters in `src/storage.rs` (which use
   `persistent_set!`). Do not call `env.storage().persistent().set` directly.
2. Call `bump_instance_ttl` from every state-changing entrypoint.
3. When adding state that outlives a single call, decide whether an existing
   `extend_*` helper covers it; if not, add one and call it from the write paths
   that keep the entry relevant.
4. Do not extend TTL in read-only getters, except where a getter is the only
   thing keeping the entry alive (as `get_personal_cap` does).
5. Keep `BUMP_THRESHOLD` well below `BUMP_AMOUNT` so a routine write does not
   pay for an extension on every call.
