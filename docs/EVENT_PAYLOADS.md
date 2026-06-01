# Event Payloads

This document enumerates every event emitted by the Proof of Heart contract.
Each entry lists the topics, the data shape, where in the code it is emitted, and a hint for indexers.

---

### `initialized`

| Field   | Value                                |
|---------|--------------------------------------|
| Topics  | `("initialized", admin: Address)`    |
| Data    | `(token: Address, platform_fee: u32)` |
| Emitted by | `lib.rs:76` — `init()` on first successful initialization |
| Indexing | Index `admin` to track who initialized. The `token` and `platform_fee` in data are fixed after init. |

---

### `campaign_created`

| Field   | Value                                      |
|---------|--------------------------------------------|
| Topics  | `("campaign_created", id: u32, creator: Address)` |
| Data    | `title: String`                            |
| Emitted by | `lib.rs:168` — `create_campaign()` after storing the campaign |
| Indexing | Index `creator` to list campaigns by user. |

---

### `contribution_made`

| Field   | Value                                          |
|---------|------------------------------------------------|
| Topics  | `("contribution_made", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                 |
| Emitted by | `lib.rs:230` — `contribute()` after token transfer |
| Indexing | Index `contributor` to track a user's contributions. Index `campaign_id` to watch campaign funding. |

---

### `withdrawal`

| Field   | Value                                        |
|---------|----------------------------------------------|
| Topics  | `("withdrawal", campaign_id: u32, creator: Address)` |
| Data    | `creator_amount: i128`                       |
| Emitted by | `lib.rs:299` — `withdraw_funds()` after transferring platform fee and creator share |
| Indexing | Index `campaign_id` to replay withdrawal events. |

---

### `campaign_cancelled`

| Field   | Value                                      |
|---------|--------------------------------------------|
| Topics  | `("campaign_cancelled", campaign_id: u32)` |
| Data    | `amount_raised: i128`                      |
| Emitted by | `lib.rs:325` — `cancel_campaign()` when the creator cancels |
| Indexing | Watch for this event on any campaign to trigger refund UX. |

---

### `campaign_updated`

| Field   | Value                                    |
|---------|------------------------------------------|
| Topics  | `("campaign_updated", campaign_id: u32)` |
| Data    | `title: String`                          |
| Emitted by | `lib.rs:365` — `update_campaign()` when title and description change before any contribution |
| Indexing | Index `campaign_id` for metadata refreshes. |

---

### `campaign_description_updated`

| Field   | Value                                              |
|---------|----------------------------------------------------|
| Topics  | `("campaign_description_updated", campaign_id: u32)` |
| Data    | `description: String`                              |
| Emitted by | `lib.rs:408` — `update_campaign_description()` when description is updated after contributions |
| Indexing | Only the description changes; title stays the same. |

---

### `refund_claimed`

| Field   | Value                                            |
|---------|--------------------------------------------------|
| Topics  | `("refund_claimed", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                   |
| Emitted by | `lib.rs:442` — `claim_refund()` after token transfer back to contributor |
| Indexing | Index `contributor` to track refunds per user. Index `campaign_id` to track refunded amount. |

---

### `revenue_deposited`

| Field   | Value                                        |
|---------|----------------------------------------------|
| Topics  | `("revenue_deposited", campaign_id: u32)`    |
| Data    | `amount: i128`                               |
| Emitted by | `lib.rs:471` — `deposit_revenue()` after token transfer into pool |
| Indexing | Emitted once per batch deposit by creator. |

---

### `revenue_claimed`

| Field   | Value                                              |
|---------|----------------------------------------------------|
| Topics  | `("revenue_claimed", campaign_id: u32, contributor: Address)` |
| Data    | `claimable: i128`                                  |
| Emitted by | `lib.rs:510` — `claim_revenue()` after token transfer to contributor |
| Indexing | Index `contributor` to track total revenue claimed per user. |

---

### `creator_revenue_claimed`

| Field   | Value                                                  |
|---------|--------------------------------------------------------|
| Topics  | `("creator_revenue_claimed", campaign_id: u32, creator: Address)` |
| Data    | `claimable: i128`                                      |
| Emitted by | `lib.rs:554` — `claim_creator_revenue()` after token transfer to creator |
| Indexing | Index `campaign_id` to track creator revenue per campaign. |

---

### `voting_params_updated`

| Field   | Value                                                                   |
|---------|-------------------------------------------------------------------------|
| Topics  | `(Symbol::new("voting_params_updated"),)`                               |
| Data    | `(old_quorum: u32, new_quorum: u32, old_threshold_bps: u32, new_threshold_bps: u32)` |
| Emitted by | `lib.rs:582` — `set_voting_params()` after validating and storing new values |
| Indexing | Single-topic event. Data is a 4-element Vec. Use array destructuring.  |

---

### `contract_paused`

| Field   | Value                                    |
|---------|------------------------------------------|
| Topics  | `("contract_paused", admin: Address)`    |
| Data    | `()`                                     |
| Emitted by | `lib.rs:604` — `pause()`                 |
| Indexing | Index `admin` for audit trail.           |

---

### `contract_unpaused`

| Field   | Value                                      |
|---------|--------------------------------------------|
| Topics  | `("contract_unpaused", admin: Address)`    |
| Data    | `()`                                       |
| Emitted by | `lib.rs:620` — `unpause()`                 |
| Indexing | Also clears `AutoPaused` flag (see `auto_paused` event). |

---

### `auto_paused`

| Field   | Value                                          |
|---------|------------------------------------------------|
| Topics  | `("auto_paused", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                 |
| Emitted by | `lib.rs:238` — `contribute()` when a single contribution exceeds the campaign's `funding_goal` |
| Indexing | Watch this event to detect burst contributions. All state-changing calls are blocked until admin calls `unpause()` or `resume_campaign()`. |

---

### `campaign_resumed`

| Field   | Value                                        |
|---------|----------------------------------------------|
| Topics  | `("campaign_resumed", campaign_id: u32)`     |
| Data    | `()`                                         |
| Emitted by | `lib.rs:642` — `resume_campaign()` when admin clears auto-pause for an active campaign |
| Indexing | Signals that the contract is no longer auto-paused. Requires the campaign to be active. |

---

### `fee_updated`

| Field   | Value                                                    |
|---------|----------------------------------------------------------|
| Topics  | `(Symbol::new("fee_updated"),)`                          |
| Data    | `(old_fee: u32, new_fee: u32)`                           |
| Emitted by | `lib.rs:731` — `update_platform_fee()`                   |
| Indexing | Capped at 1000 bps (10%). See data for the clamped value. |

---

### `admin_updated`

| Field   | Value                                                    |
|---------|----------------------------------------------------------|
| Topics  | `(Symbol::new("admin_updated"),)`                        |
| Data    | `(old_admin: Address, new_admin: Address)`               |
| Emitted by | `lib.rs:750` — `update_admin()`                          |
| Indexing | Single-topic event. Data is a 2-element Vec.             |

---

### `campaign_transfer_initiated`

| Field   | Value                                                         |
|---------|---------------------------------------------------------------|
| Topics  | `("campaign_transfer_initiated", campaign_id: u32, current_creator: Address)` |
| Data    | `new_creator: Address`                                        |
| Emitted by | `lib.rs:861` — `initiate_campaign_transfer()`                |
| Indexing | Listen for this to show a pending transfer UX.               |

---

### `campaign_transfer_completed`

| Field   | Value                                                    |
|---------|----------------------------------------------------------|
| Topics  | `("campaign_transfer_completed", campaign_id: u32)`      |
| Data    | `(old_creator: Address, new_creator: Address)`           |
| Emitted by | `lib.rs:892` — `accept_campaign_transfer()`             |
| Indexing | Data is a 2-element Address tuple.                       |

---

### `campaign_transfer_cancelled`

| Field   | Value                                                    |
|---------|----------------------------------------------------------|
| Topics  | `("campaign_transfer_cancelled", campaign_id: u32)`      |
| Data    | `()`                                                     |
| Emitted by | `lib.rs:916` — `cancel_campaign_transfer()`             |
| Indexing | No data payload.                                         |

---

### `campaign_vote_cast`

| Field   | Value                                            |
|---------|--------------------------------------------------|
| Topics  | `("campaign_vote_cast", campaign_id: u32, voter: Address)` |
| Data    | `approve: bool`                                  |
| Emitted by | `voting.rs:90` — `cast_vote()`                  |
| Indexing | Index `voter` to prevent double-vote tracking. `approve` is true for approve, false for reject. |

---

### `campaign_verified` (admin)

| Field   | Value                                        |
|---------|----------------------------------------------|
| Topics  | `("campaign_verified", campaign_id: u32)`    |
| Data    | `()`                                         |
| Emitted by | `voting.rs:112` — `admin_verify()`          |
| Indexing | Emitted when admin marks a campaign verified directly. |

---

### `campaign_verified` (community)

| Field   | Value                                        |
|---------|----------------------------------------------|
| Topics  | `("campaign_verified", campaign_id: u32)`    |
| Data    | `approve_votes: u32`                         |
| Emitted by | `voting.rs:158` — `verify_with_votes()`     |
| Indexing | The data is the raw number of approve votes at the time of verification. |
