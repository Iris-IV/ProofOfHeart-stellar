# Fix: reserve payouts are auditable from events alone

## Summary

Issue #852: `src/campaigns/withdraw.rs` moves a creator's vested reserve out of
escrow without an event that lets an indexer attribute the payout — campaign,
creator and amount — from the event stream.

The reserve lifecycle in this contract spans two calls:

* `withdraw_funds` splits the escrow after fees into an immediate creator payout
  and a withheld **reserve**, persisting `CampaignReserve` for later release.
* `withdraw_reserve` transfers that reserve to the creator once the vesting
  delay has elapsed.

`withdraw_funds` already emitted `reserve_withheld`, but that event named only
the campaign id, so the withheld reserve was not attributable to a creator from
the event alone. Off-chain accounting was left to diff token balances to
reconstruct who received a reserve payout, which is unreliable across refunds,
fee overrides and multiple tokens.

## Root cause

The two reserve events were asymmetric:

| Event | Topics | Data |
|-------|--------|------|
| `reserve_withheld` | `("reserve_withheld", campaign_id)` | `reserve_amount` |
| `reserve_released` | `("reserve_released", campaign_id, creator)` | `amount` |

A reserve payout (`reserve_released`) could only be reconciled against a
withholding (`reserve_withheld`) by joining through campaign state, and the
withholding side of the pair did not identify the recipient at all.

## Fix

Make both reserve-lifecycle events self-describing and symmetric. Each now
publishes:

* topics: `(event name, campaign_id: u32, creator: Address)`
* data: the exact reserve amount (`i128`) transferred or withheld

`reserve_released` keeps its existing shape (it already carried all three
fields). `reserve_withheld` gains the creator in its topics, matching its
counterpart. The amount in each payload is the exact value moved by the
preceding token transfer in the same call, so an indexer can post the payout
without a separate balance read.

## Verification

`src/tests/test_issue_852.rs` covers the full reserve lifecycle:

* `withdraw_funds` emits `reserve_withheld` with the campaign id and creator as
  topics and the withheld amount as data.
* `withdraw_reserve` emits `reserve_released` with the same three fields, and
  the event amount equals the creator's actual token balance delta.

`EVENT_PAYLOADS.md` and `CHANGELOG.md` are updated to match, and the stale
`EVENT_PAYLOADS.md` source pointer (`lib.rs:581 — release_reserve()`, a function
that no longer exists) is corrected to the current `withdraw_reserve`
implementation.
