# Proof of Heart — Event Payloads

Every `publish(...)` call in the contract, with its topics, data shape, and the code that emits it.

**Keep in sync:** When adding or modifying any `env.events().publish(...)` call, update this file's topics, data shape, and source location to match.

---

### `initialized`

| Field  | Value |
|--------|-------|
| Topics | `("initialized", admin: Address)` |
| Data   | `(token: Address, fee_bps: u32, min_quorum: u32, threshold_bps: u32, version: u32)` |
| Source | `src/admin.rs` — `init()` |

---

### `campaign_created`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_created", id: u32, creator: Address)` |
| Data   | `(title: String, category: u32)` |
| Source | `src/campaigns/create.rs` — `create_campaign()` |

---

### `auto_paused (huge contribution)`

| Field  | Value |
|--------|-------|
| Topics | `("auto_paused",)` |
| Data   | `("huge_contribution", amount: i128)` |
| Source | `src/contributions.rs` — `contribute()` when contribution > 200% of goal |

---

### `auto_paused (burst)`

| Field  | Value |
|--------|-------|
| Topics | `("auto_paused",)` |
| Data   | `("burst", count: u32)` |
| Source | `src/contributions.rs` — `contribute()` when >10 tx/block for a campaign |

---

### `contribution_made`

| Field  | Value |
|--------|-------|
| Topics | `("contribution_made", campaign_id: u32, contributor: Address)` |
| Data   | `amount: i128` |
| Source | `src/contributions.rs` — `contribute()` |

---

### `withdrawal`

| Field  | Value |
|--------|-------|
| Topics | `("withdrawal", campaign_id: u32, creator: Address)` |
| Data   | `(fee_bps: u32, creator_amount: i128, reserve_amount: i128)` |
| Source | `src/campaigns/withdraw.rs` — `withdraw_funds()` |

---

### `reserve_withheld`

| Field  | Value |
|--------|-------|
| Topics | `("reserve_withheld", campaign_id: u32, creator: Address)` |
| Data   | `reserve_amount: i128` |
| Source | `src/campaigns/withdraw.rs` — `withdraw_funds()` when `reserve_amount > 0` |

---

### `reserve_released`

| Field  | Value |
|--------|-------|
| Topics | `("reserve_released", campaign_id: u32, creator: Address)` |
| Data   | `amount: i128` |
| Source | `src/campaigns/withdraw.rs` — `withdraw_reserve()` |

> The reserve lifecycle is deliberately symmetric (#852): both events name the event, the campaign id and the creator in their topics and carry the exact reserve amount as data. An indexer can therefore attribute a `reserve_withheld` / `reserve_released` pair to a campaign and creator from the event stream alone, without diffing token balances to reconstruct reserve payouts.

---

### `vesting_params_updated`

| Field  | Value |
|--------|-------|
| Topics | `("vesting_params_updated", admin: Address)` |
| Data   | `(delay_days: u64, reserve_bps: u32)` |
| Source | `src/campaigns/withdraw.rs` — `set_vesting_params()` |

---

### `vesting_disabled`

| Field  | Value |
|--------|-------|
| Topics | `("vesting_disabled", admin: Address)` |
| Data   | `()` |
| Source | `src/campaigns/withdraw.rs` — `set_vesting_params()` |

---

### `revenue_pool_refunded`

| Field  | Value |
|--------|-------|
| Topics | `("revenue_pool_refunded", campaign_id: u32)` |
| Data   | `pool_amount: i128` |
| Source | `src/campaigns/cancel.rs` — `cancel_campaign()` when revenue_pool > 0 |

---

### `campaign_bookmarked`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_bookmarked", user: Address)` |
| Data   | `campaign_id: u32` |
| Source | `src/bookmarks.rs` — `save_campaign()` |

---

### `campaign_cancelled`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_cancelled", campaign_id: u32, creator: Address)` |
| Data   | `amount_raised: i128` |
| Source | `src/campaigns/cancel.rs` — `cancel_campaign()` |

---

### `campaign_updated`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_updated", campaign_id: u32)` |
| Data   | `(title: String, description: String)` |
| Source | `src/campaigns/update.rs` — `update_campaign()` |

---

### `campaign_description_updated`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_description_updated", campaign_id: u32)` |
| Data   | `new_desc: String` |
| Source | `src/campaigns/update.rs` — `update_campaign_description()` |

---

### `campaign_unbookmarked`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_unbookmarked", user: Address)` |
| Data   | `campaign_id: u32` |
| Source | `src/bookmarks.rs` — `remove_saved_campaign()` |

---

### `campaign_bookmarks_cleared`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_bookmarks_cleared", user: Address)` |
| Data   | `cleared: u32` |
| Source | `src/bookmarks.rs` — `clear_saved_campaigns()` |

---

### `refund_claimed`

| Field  | Value |
|--------|-------|
| Topics | `("refund_claimed", campaign_id: u32, contributor: Address)` |
| Data   | `amount: i128` |
| Source | `src/contributions.rs` — `claim_refund()` |

---

### `revenue_deposited`

| Field  | Value |
|--------|-------|
| Topics | `("revenue_deposited", campaign_id: u32, creator: Address)` |
| Data   | `amount: i128` |
| Source | `src/revenue.rs` — `deposit_revenue()` |

---

### `revenue_claimed`

| Field  | Value |
|--------|-------|
| Topics | `("revenue_claimed", campaign_id: u32, contributor: Address)` |
| Data   | `claimable: i128` |
| Source | `src/revenue.rs` — `claim_revenue()` |

---

### `creator_revenue_claimed`

| Field  | Value |
|--------|-------|
| Topics | `("creator_revenue_claimed", campaign_id: u32, creator: Address)` |
| Data   | `claimable: i128` |
| Source | `src/revenue.rs` — `claim_creator_revenue()` |

---

### `voting_params_updated`

| Field  | Value |
|--------|-------|
| Topics | `("voting_params_updated",)` |
| Data   | `(old_quorum: u32, new_quorum: u32, old_threshold: u32, new_threshold: u32)` |
| Source | `src/admin.rs` — `set_voting_params()` |

---

### `warning_high_voting_balance`

| Field  | Value |
|--------|-------|
| Topics | `("warning_high_voting_balance",)` |
| Data   | `min_balance: i128` |
| Source | `src/admin.rs` — `set_min_voting_balance()` when `min_balance > 10^15` |

---

### `min_voting_balance_updated`

| Field  | Value |
|--------|-------|
| Topics | `("min_voting_balance_updated",)` |
| Data   | `(old_balance: i128, new_balance: i128)` |
| Source | `src/admin.rs` — `set_min_voting_balance()` |

---

### `contract_paused`

| Field  | Value |
|--------|-------|
| Topics | `("contract_paused", admin: Address)` |
| Data   | `()` |
| Source | `src/admin.rs` — `pause()` |

---

### `contract_unpaused`

| Field  | Value |
|--------|-------|
| Topics | `("contract_unpaused", admin: Address)` |
| Data   | `()` |
| Source | `src/admin.rs` — `unpause()` |

---

### `creation_disabled_updated`

| Field  | Value |
|--------|-------|
| Topics | `("creation_disabled_updated", admin: Address)` |
| Data   | `disabled: bool` |
| Source | `src/admin.rs` — `set_creation_disabled()` |

---

### `campaigns_bulk_verified`

| Field  | Value |
|--------|-------|
| Topics | `("campaigns_bulk_verified",)` |
| Data   | `(verified_count: u32, failed_ids: Vec<u32>)` |
| Source | `src/lib.rs` — `verify_campaigns()` |

> #442: the payload used to be (verified_count, total); it now carries the per-id outcome. `failed_ids` lists every campaign id in the processed batch (up to 50) that could not be verified, so indexers can distinguish partial success from total failure without re-querying the contract.

---

### `migrated`

| Field  | Value |
|--------|-------|
| Topics | `("migrated",)` |
| Data   | `(expected_old_version: u32, new_version: u32)` |
| Source | `src/admin.rs` — `migrate()` |

---

### `token_update_proposed`

| Field  | Value |
|--------|-------|
| Topics | `("token_update_proposed",)` |
| Data   | `(new_token: Address, release_after: u64)` |
| Source | `src/admin.rs` — `propose_token_update()` |

---

### `token_update_accepted`

| Field  | Value |
|--------|-------|
| Topics | `("token_update_accepted",)` |
| Data   | `(old_token: Address, new_token: Address)` |
| Source | `src/admin.rs` — `accept_token_update()` |

---

### `token_update_cancelled`

| Field  | Value |
|--------|-------|
| Topics | `("token_update_cancelled",)` |
| Data   | `()` |
| Source | `src/admin.rs` — `cancel_token_update()` |

---

### `fee_updated`

| Field  | Value |
|--------|-------|
| Topics | `("fee_updated",)` |
| Data   | `(old_fee: u32, new_fee: u32)` |
| Source | `src/admin.rs` — `update_platform_fee()` |

---

### `campaign_fee_override_set`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_fee_override_set", campaign_id: u32)` |
| Data   | `fee_bps: u32` |
| Source | `src/admin.rs` — `set_campaign_fee_override()` |

---

### `category_duration_cap_set`

| Field  | Value |
|--------|-------|
| Topics | `("category_duration_cap_set", category: u32)` |
| Data   | `max_days: u64` |
| Source | `src/admin.rs` — `set_category_duration_cap()` |

---

### `category_duration_cap_removed`

| Field  | Value |
|--------|-------|
| Topics | `("category_duration_cap_removed", category: u32)` |
| Data   | `()` |
| Source | `src/admin.rs` — `remove_category_duration_cap()` |

---

### `campaign_deadline_extended`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_deadline_extended", campaign_id: u32)` |
| Data   | `(old_deadline: u64, new_deadline: u64)` |
| Source | `src/campaigns/update.rs` — `extend_campaign_deadline()` |

---

### `personal_cap_set`

| Field  | Value |
|--------|-------|
| Topics | `("personal_cap_set", campaign_id: u32, contributor: Address)` |
| Data   | `amount: i128` |
| Source | `src/contributions.rs` — `set_personal_cap()` |

---

### `personal_cap_removed`

| Field  | Value |
|--------|-------|
| Topics | `("personal_cap_removed", campaign_id: u32, contributor: Address)` |
| Data   | `()` |
| Source | `src/contributions.rs` — `remove_personal_cap()` |

---

### `admin_transfer_initiated`

| Field  | Value |
|--------|-------|
| Topics | `("admin_transfer_initiated",)` |
| Data   | `(current_admin: Address, new_admin: Address)` |
| Source | `src/admin.rs` — `initiate_admin_transfer()` |

---

### `admin_transfer_cancelled`

| Field  | Value |
|--------|-------|
| Topics | `("admin_transfer_cancelled",)` |
| Data   | `admin: Address` |
| Source | `src/admin.rs` — `cancel_admin_transfer()` |

---

### `admin_updated`

| Field  | Value |
|--------|-------|
| Topics | `("admin_updated", old_admin: Address)` |
| Data   | `new_admin: Address` |
| Source | `src/admin.rs` — `update_admin()` |

---

### `min_campaign_funding_goal_updated`

| Field  | Value |
|--------|-------|
| Topics | `("min_campaign_funding_goal_updated",)` |
| Data   | `(old_min: i128, new_min: i128)` |
| Source | `src/admin.rs` — `set_min_campaign_funding_goal()` |

---

### `max_campaign_funding_goal_updated`

| Field  | Value |
|--------|-------|
| Topics | `("max_campaign_funding_goal_updated",)` |
| Data   | `(old_max: i128, new_max: i128)` |
| Source | `src/admin.rs` — `set_max_campaign_funding_goal()` |

---

### `campaign_transfer_initiated`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_transfer_initiated", campaign_id: u32, current_creator: Address)` |
| Data   | `new_creator: Address` |
| Source | `src/campaigns/transfer.rs` — `initiate_campaign_transfer()` |

---

### `campaign_transfer_completed`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_transfer_completed", campaign_id: u32)` |
| Data   | `(old_creator: Address, new_creator: Address)` |
| Source | `src/campaigns/transfer.rs` — `accept_campaign_transfer()` |

---

### `voting_state_purged`

| Field  | Value |
|--------|-------|
| Topics | `("voting_state_purged", campaign_id: u32)` |
| Data   | `()` |
| Source | `src/admin.rs` — `purge_voting_state()` |

---

### `campaign_transfer_cancelled`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_transfer_cancelled", campaign_id: u32)` |
| Data   | `pending_creator: Address` |
| Source | `src/campaigns/transfer.rs` — `cancel_campaign_transfer()` |

---

### `campaign_resumed`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_resumed", campaign_id: u32, caller: Address)` |
| Data   | `()` |
| Source | `src/admin.rs` — `resume_campaign()` |

---

### `campaign_vote_cast`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_vote_cast", campaign_id: u32, voter: Address)` |
| Data   | `(approve: bool, balance: i128, weight: i128)` |
| Source | `src/voting.rs` — `cast_vote()` |

---

### `campaign_verified (admin)`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_verified", campaign_id: u32)` |
| Data   | `()` |
| Source | `src/voting.rs` — `admin_verify()` |

---

### `campaign_verified (community)`

| Field  | Value |
|--------|-------|
| Topics | `("campaign_verified", campaign_id: u32)` |
| Data   | `approve_votes: u32` |
| Source | `src/voting.rs` — `verify_with_votes()` |

---

### `platform_stats_inconsistent`

| Field  | Value |
|--------|-------|
| Topics | `("platform_stats_inconsistent",)` |
| Data   | `(total_campaigns: u32, active_campaigns: u32, verified_campaigns: u32, cancelled_campaigns: u32)` |
| Source | `src/queries.rs` — `get_platform_stats()` |

> Published only when the stored platform counters violate the consistency invariants checked by `queries::counters_are_consistent` (e.g. a partial migration or failed legacy write leaves `active_campaigns > total_campaigns`). The raw stored values are reported in the event data so the corruption is auditable; the same query returns `stats_are_partial = true`.

---

Total: 52 documented publish() call sites