# Storage TTL Policy

To protect users and contract maintainers from rent inflation and unexpected archival of persistent contract data on Stellar/Soroban, ProofOfHeart follows a deterministic TTL (Time-To-Live) management strategy.

## Key Principles

1. **Write-Path TTL Extensions**:
   - Persistent storage entries (such as `Campaign`, `UserContribution`, and `VoterRecord`) have their TTL extended **strictly during write operations**.
   - Executing write entrypoints automatically invokes `env.storage().persistent().extend_ttl(...)` with defined `threshold` and `extend_to` ledger bounds.

2. **Read-Path Economic Neutrality**:
   - Read/view functions (such as `get_campaign` or `get_contribution`) fetch stored values without extending TTL.
   - This design ensures read-heavy indexing services or dashboard polling calls do not cause rent inflation or impose unnecessary fee overheads.

3. **Storage Tiering Rules**:
   - **Instance Storage**: Stores core configuration (admin address, token address, minimum quorum thresholds) and is extended on all state-modifying admin invocations.
   - **Persistent Storage**: Stores campaign state and user balances; entries are maintained with minimum threshold `LOW_TTL_THRESHOLD` and extended up to `HIGH_TTL_BUMP` ledgers upon write operations.

4. **Archival Recovery**:
   - In the event a dormant persistent entry becomes archived, callers can restore state via Stellar RPC `restore_footprint` before executing state updates.
