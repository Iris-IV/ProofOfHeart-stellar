# Proof of Heart Event Payloads

This document describes the events emitted by the Proof of Heart contract and their payloads for client-side indexing and integration.

## Event Format

All events follow the Soroban event format with `topics` and a `data` field. Topics are indexed and searchable; the data field contains additional details.

## Contract Events

### Initialization Events

#### `initialized`
Emitted when the contract is initialized.

- **Topics**: `["initialized", admin_address]`
- **Data**: `[token_address, platform_fee_bps, min_quorum, approval_threshold_bps, version]`
- **Emitted By**: `init()`
- **Example Usage**: Track contract setup and initial admin/token configuration.

### Campaign Events

#### `campaign_created`
Emitted when a new campaign is created.

- **Topics**: `["campaign_created", campaign_id, creator_address]`
- **Data**: `campaign_title`
- **Emitted By**: `create_campaign()`
- **Indexing Tip**: Track all campaigns by creator or scan campaign IDs chronologically.

#### `campaign_updated`
Emitted when campaign title and description are updated (before any contributions).

- **Topics**: `["campaign_updated", campaign_id]`
- **Data**: `new_title`
- **Emitted By**: `update_campaign()`

#### `campaign_description_updated`
Emitted when only the campaign description is updated (after contributions allowed).

- **Topics**: `["campaign_description_updated", campaign_id]`
- **Data**: `()`
- **Emitted By**: `update_campaign_description()`

#### `campaign_cancelled`
Emitted when a campaign is cancelled by its creator.

- **Topics**: `["campaign_cancelled", campaign_id]`
- **Data**: `()`
- **Emitted By**: `cancel_campaign()`

#### `campaign_transfer_initiated`
Emitted when ownership transfer begins.

- **Topics**: `["campaign_transfer_initiated", campaign_id, current_creator_address]`
- **Data**: `new_creator_address`
- **Emitted By**: `initiate_campaign_transfer()`

#### `campaign_transfer_completed`
Emitted when ownership transfer is finalized.

- **Topics**: `["campaign_transfer_completed", campaign_id]`
- **Data**: `(old_creator_address, new_creator_address)`
- **Emitted By**: `accept_campaign_transfer()`

#### `campaign_transfer_cancelled`
Emitted when a pending ownership transfer is cancelled.

- **Topics**: `["campaign_transfer_cancelled", campaign_id]`
- **Data**: `()`
- **Emitted By**: `cancel_campaign_transfer()`

### Contribution Events

#### `contribution_made`
Emitted when a contributor funds a campaign.

- **Topics**: `["contribution_made", campaign_id, contributor_address]`
- **Data**: `amount_tokens`
- **Emitted By**: `contribute()`
- **Indexing Tip**: Track all contributions per campaign or per contributor for dashboards.

#### `refund_claimed`
Emitted when a contributor claims a refund (campaign cancelled or funding goal not reached).

- **Topics**: `["refund_claimed", campaign_id, contributor_address]`
- **Data**: `refund_amount_tokens`
- **Emitted By**: `claim_refund()`

#### `withdrawal`
Emitted when a campaign creator withdraws funds after reaching the funding goal.

- **Topics**: `["withdrawal", campaign_id, creator_address]`
- **Data**: `creator_amount_after_fee`
- **Emitted By**: `withdraw_funds()`
- **Note**: Platform fee is deducted before creator receives this amount.

### Revenue Sharing Events

#### `revenue_deposited`
Emitted when a creator deposits revenue into a campaign's revenue pool.

- **Topics**: `["revenue_deposited", campaign_id]`
- **Data**: `deposited_amount_tokens`
- **Emitted By**: `deposit_revenue()`
- **Precondition**: Campaign must have `has_revenue_sharing == true`.

#### `revenue_claimed`
Emitted when a contributor claims their share of the revenue pool.

- **Topics**: `["revenue_claimed", campaign_id, contributor_address]`
- **Data**: `claimable_amount_tokens`
- **Emitted By**: `claim_revenue()`
- **Calculation**: Share is proportional to the contributor's initial contribution relative to total raised.

#### `creator_revenue_claimed`
Emitted when a creator claims their retained share of the revenue pool.

- **Topics**: `["creator_revenue_claimed", campaign_id, creator_address]`
- **Data**: `claimable_amount_tokens`
- **Emitted By**: `claim_creator_revenue()`
- **Calculation**: Creator share = total pool - (pool * revenue_share_percentage / 10000).

### Voting & Verification Events

#### `campaign_vote_cast`
Emitted when a token-holding voter casts an approve or reject vote on a campaign.

- **Topics**: `["campaign_vote_cast", campaign_id, voter_address]`
- **Data**: `approve` (bool — `true` = approve, `false` = reject)
- **Emitted By**: `vote_on_campaign()`
- **Indexing Tip**: Filter by `campaign_id` to tally live vote counts, or by `voter_address` to audit a voter's history.

**Sample payload**:
```json
{
  "type": "contract",
  "topics": ["campaign_vote_cast", 42, "GVOTER...ADDRESS"],
  "data": true
}
```

#### `voting_params_updated`
Emitted when voting parameters are updated by the admin.

- **Topics**: `["voting_params_updated"]`
- **Data**: `(old_min_votes_quorum, new_min_votes_quorum, old_approval_threshold_bps, new_approval_threshold_bps)`
- **Emitted By**: `set_voting_params()`

### Platform Management Events

#### `contract_paused`
Emitted when the contract is paused by the admin.

- **Topics**: `["contract_paused", admin_address]`
- **Data**: `()`
- **Emitted By**: `pause()`

#### `contract_unpaused`
Emitted when the contract is unpaused by the admin.

- **Topics**: `["contract_unpaused", admin_address]`
- **Data**: `()`
- **Emitted By**: `unpause()`

#### `fee_updated`
Emitted when the platform fee is updated.

- **Topics**: `["fee_updated"]`
- **Data**: `(old_fee_bps, new_fee_bps)`
- **Emitted By**: `update_platform_fee()`

#### `admin_updated`
Emitted when admin privileges are transferred.

- **Topics**: `["admin_updated", current_admin_address]`
- **Data**: `new_admin_address`
- **Emitted By**: `update_admin()`

## Client-Side Indexing Patterns

### Pattern 1: Track All Campaigns
```
Listen for: "campaign_created" events
Index: Map campaign_id -> (creator, title, timestamp)
```

### Pattern 2: Track Campaign Contributions
```
Listen for: "contribution_made" events
Index: Map (campaign_id, contributor) -> [amounts]
```

### Pattern 3: Track Revenue Pool Activity
```
Listen for: "revenue_deposited", "revenue_claimed", "creator_revenue_claimed" events
Index: Map campaign_id -> [deposits, claims]
```

### Pattern 4: Monitor Campaign State Transitions
```
Listen for: "campaign_updated", "campaign_cancelled", "campaign_transfer_*" events
Use to trigger cache invalidation and UI refreshes
```

### Pattern 5: Platform Metrics
```
Listen for: "fee_updated", "contract_paused", "admin_updated" events
Use to track platform governance and operational changes
```

## Soroban Event Query Tips

When querying events from a Soroban node:
- Events are indexed by `(contract_id, topic)`
- Filter by the first topic to isolate event types
- Use contributor or creator addresses in topics for user-specific filtering
- Timestamp and ledger info are provided by the node's event API

## Type Information

- `Address`: Soroban contract address (33 bytes)
- `u32`: Unsigned 32-bit integer (campaign IDs, votes, fees in basis points)
- `i128`: Signed 128-bit integer (token amounts)
- `String`: UTF-8 string (titles, descriptions)
- `()`: Empty tuple (no additional data)
