# Proof of Heart — Event Payloads

Every `publish(...)` call in the contract, with its topics, data shape, and the code that emits it.

---

### `initialized`

| Field   | Value                                                        |
|---------|--------------------------------------------------------------|
| Topics  | `("initialized", admin: Address)`                            |
| Data    | `(token: Address, fee_bps: u32, min_quorum: u32, threshold_bps: u32, version: u32)` |
| Source  | `lib.rs:181` — `init()`                                      |

---

### `campaign_created`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_created", id: u32, creator: Address)`          |
| Data    | `(title: String, category: u32)`                           |
| Source  | `lib.rs:327` — `create_campaign()`                         |

---

### `auto_paused` (huge contribution)

| Field   | Value                                                  |
|---------|--------------------------------------------------------|
| Topics  | `("auto_paused",)`                                     |
| Data    | `("huge_contribution", amount: i128)`                  |
| Source  | `lib.rs:396` — `contribute()` when contribution > 200% of goal |

---

### `auto_paused` (burst)

| Field   | Value                                                  |
|---------|--------------------------------------------------------|
| Topics  | `("auto_paused",)`                                     |
| Data    | `("burst", count: u32)`                                |
| Source  | `lib.rs:414` — `contribute()` when >10 tx/block for a campaign |

---

### `contribution_made`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("contribution_made", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                             |
| Source  | `lib.rs:438` — `contribute()`                              |

---

### `withdrawal`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("withdrawal", campaign_id: u32, creator: Address)`       |
| Data    | `(fee_amount: i128, creator_amount: i128, reserve_amount: i128)` |
| Source  | `lib.rs:533` — `withdraw_funds()`                          |

---

### `reserve_withheld`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("reserve_withheld", campaign_id: u32)`                   |
| Data    | `reserve_amount: i128`                                     |
| Source  | `lib.rs:540` — `withdraw_funds()` when `reserve_amount > 0` |

---

### `reserve_released`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("reserve_released", campaign_id: u32, creator: Address)` |
| Data    | `amount: i128`                                             |
| Source  | `lib.rs:581` — `release_reserve()`                         |

---

### `vesting_params_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("vesting_params_updated", admin: Address)`               |
| Data    | `(delay_days: u64, reserve_bps: u32)`                      |
| Source  | `lib.rs:606` — `set_vesting_params()`                      |

---

### `vesting_disabled`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("vesting_disabled", admin: Address)`                     |
| Data    | `()`                                                       |
| Source  | `lib.rs:606` — `set_vesting_params()`                      |

---

### `revenue_pool_refunded`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("revenue_pool_refunded", campaign_id: u32)`              |
| Data    | `pool_amount: i128`                                        |
| Source  | `lib.rs:649` — `cancel_campaign()` when `revenue_pool > 0` |

---

### `campaign_cancelled`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_cancelled", campaign_id: u32, creator: Address)` |
| Data    | `amount_raised: i128`                                      |
| Source  | `lib.rs:659` — `cancel_campaign()`                         |

---

### `campaign_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_updated", campaign_id: u32)`                   |
| Data    | `(title: String, description: String)`                     |
| Source  | `lib.rs:706` — `update_campaign()`                         |

---

### `campaign_description_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_description_updated", campaign_id: u32)`       |
| Data    | `new_desc: String`                                         |
| Source  | `lib.rs:752` — `update_campaign_description()`             |

---

### `refund_claimed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("refund_claimed", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                             |
| Source  | `lib.rs:805` — `claim_refund()`                            |

---

### `revenue_deposited`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("revenue_deposited", campaign_id: u32)`                  |
| Data    | `amount: i128`                                             |
| Source  | `lib.rs:842` — `deposit_revenue()`                         |

---

### `revenue_claimed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("revenue_claimed", campaign_id: u32, contributor: Address)` |
| Data    | `claimable: i128`                                          |
| Source  | `lib.rs:907` — `claim_revenue()`                           |

---

### `creator_revenue_claimed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("creator_revenue_claimed", campaign_id: u32, creator: Address)` |
| Data    | `claimable: i128`                                          |
| Source  | `lib.rs:954` — `claim_creator_revenue()`                   |

---

### `voting_params_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `(Symbol("voting_params_updated"),)`                       |
| Data    | `(old_quorum: u32, new_quorum: u32, old_threshold: u32, new_threshold: u32)` |
| Source  | `lib.rs:985` — `set_voting_params()`                       |

---

### `warning_high_voting_balance`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("warning_high_voting_balance",)`                         |
| Data    | `min_balance: i128`                                        |
| Source  | `lib.rs:1020` — `set_min_voting_balance()` when `min_balance > 10^15` |

---

### `min_voting_balance_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `(Symbol("min_voting_balance_updated"),)`                  |
| Data    | `(old_balance: i128, new_balance: i128)`                   |
| Source  | `lib.rs:1026` — `set_min_voting_balance()`                 |

---

### `contract_paused`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("contract_paused", admin: Address)`                      |
| Data    | `()`                                                       |
| Source  | `lib.rs:1045` — `pause()`                                  |

---

### `contract_unpaused`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("contract_unpaused", admin: Address)`                    |
| Data    | `()`                                                       |
| Source  | `lib.rs:1060` — `unpause()`                                |

---

### `creation_disabled_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("creation_disabled_updated", admin: Address)`            |
| Data    | `disabled: bool`                                           |
| Source  | `lib.rs:1091` — `set_creation_disabled()`                  |

---

### `campaigns_bulk_verified`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaigns_bulk_verified",)`                             |
| Data    | `(verified_count: u32, total: u32)`                        |
| Source  | `lib.rs:1175` — `verify_campaigns()`                       |

---

### `migrated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("migrated",)`                                            |
| Data    | `(expected_old_version: u32, new_version: u32)`            |
| Source  | `lib.rs:1309` — `migrate()`                                |

---

### `token_update_proposed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("token_update_proposed",)`                               |
| Data    | `(new_token: Address, release_after: u64)`                 |
| Source  | `lib.rs:1339` — `propose_token_update()`                   |

---

### `token_update_accepted`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("token_update_accepted",)`                               |
| Data    | `(old_token: Address, new_token: Address)`                 |
| Source  | `lib.rs:1359` — `accept_token_update()`                    |

---

### `token_update_cancelled`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("token_update_cancelled",)`                              |
| Data    | `()`                                                       |
| Source  | `lib.rs:1374` — `cancel_token_update()`                    |

---

### `fee_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `(Symbol("fee_updated"),)`                                 |
| Data    | `(old_fee: u32, new_fee: u32)`                             |
| Source  | `lib.rs:1392` — `update_platform_fee()`                    |

---

### `campaign_fee_override_set`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_fee_override_set", campaign_id: u32)`          |
| Data    | `fee_bps: u32`                                             |
| Source  | `lib.rs:1420` — `set_campaign_fee_override()`              |

---

### `category_duration_cap_set`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("category_duration_cap_set", category: u32)`             |
| Data    | `max_days: u64`                                            |
| Source  | `lib.rs:1444` — `set_category_duration_cap()`              |

---

### `category_duration_cap_removed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("category_duration_cap_removed", category: u32)`         |
| Data    | `()`                                                       |
| Source  | `lib.rs:1461` — `remove_category_duration_cap()`           |

---

### `campaign_deadline_extended`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_deadline_extended", campaign_id: u32)`         |
| Data    | `(old_deadline: u64, new_deadline: u64)`                   |
| Source  | `src/campaigns/update.rs:82` — `extend_campaign_deadline()` |

---

### `personal_cap_set`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("personal_cap_set", campaign_id: u32, contributor: Address)` |
| Data    | `amount: i128`                                             |
| Source  | `lib.rs:1556` — `set_personal_cap()`                       |

---

### `admin_transfer_initiated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("admin_transfer_initiated",)`                            |
| Data    | `(current_admin: Address, new_admin: Address)`             |
| Source  | `lib.rs:1598` — `initiate_admin_transfer()`                |

---

### `admin_transfer_cancelled`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("admin_transfer_cancelled",)`                            |
| Data    | `admin: Address`                                           |
| Source  | `lib.rs:1592` (re-initiate overwrites old pending) / `lib.rs:1631` (cancel) |

---

### `admin_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("admin_updated", old_admin: Address)`                    |
| Data    | `new_admin: Address`                                       |
| Source  | `lib.rs:1615` — `update_admin()`                           |

---

### `min_campaign_funding_goal_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("min_campaign_funding_goal_updated",)`                   |
| Data    | `(old_min: i128, new_min: i128)`                           |
| Source  | `lib.rs:1710` — `set_min_campaign_funding_goal()`          |

---

### `max_campaign_funding_goal_updated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("max_campaign_funding_goal_updated",)`                   |
| Data    | `(old_max: i128, new_max: i128)`                           |
| Source  | `lib.rs:1743` — `set_max_campaign_funding_goal()`          |

---

### `campaign_transfer_initiated`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `(Symbol("campaign_transfer_initiated"), campaign_id: u32, current_creator: Address)` |
| Data    | `new_creator: Address`                                     |
| Source  | `lib.rs:1990` — `initiate_campaign_transfer()`             |

---

### `campaign_transfer_completed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_transfer_completed", campaign_id: u32)`        |
| Data    | `(old_creator: Address, new_creator: Address)`             |
| Source  | `lib.rs:2047` — `accept_campaign_transfer()`               |

---

### `voting_state_purged`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("voting_state_purged", campaign_id: u32)`                |
| Data    | `()`                                                       |
| Source  | `lib.rs:2113` — `purge_voting_state()`                     |

---

### `campaign_transfer_cancelled`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_transfer_cancelled", campaign_id: u32)`        |
| Data    | `pending_creator: Address`                                 |
| Source  | `lib.rs:2136` — `cancel_campaign_transfer()`               |

---

### `campaign_resumed`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_resumed", campaign_id: u32, caller: Address)`  |
| Data    | `()`                                                       |
| Source  | `lib.rs:2187` — `resume_campaign()`                        |

---

### `campaign_vote_cast`

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_vote_cast", campaign_id: u32, voter: Address)` |
| Data    | `(approve: bool, balance: i128, weight: i128)`             |
| Source  | `voting.rs:100` — `cast_vote()`                            |

---

### `campaign_verified` (admin)

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_verified", campaign_id: u32)`                  |
| Data    | `()`                                                       |
| Source  | `voting.rs:127` — admin `verify_campaign()`                |

---

### `campaign_verified` (community)

| Field   | Value                                                      |
|---------|------------------------------------------------------------|
| Topics  | `("campaign_verified", campaign_id: u32)`                  |
| Data    | `approve_votes: u32`                                       |
| Source  | `voting.rs:183` — `verify_campaign_with_votes()`           |

---

> **Total: 48 documented `publish()` call sites**
