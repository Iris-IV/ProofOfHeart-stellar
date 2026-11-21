# Community Governance & Voting

This document explains how campaign verification voting actually works in
ProofOfHeart: the weight formula, quorum/threshold math, and how the admin
tunes them. It goes deeper than [`CAMPAIGN_LIFECYCLE.md`](CAMPAIGN_LIFECYCLE.md),
which only covers voting as one step in the campaign state machine.

> **Terminology note:** the originating issue for this doc asked about
> "proposal thresholds" and "proposal execution." There are no proposals in
> this contract — no on-chain mechanism submits or executes arbitrary
> governance actions. What exists is **campaign verification voting**: token
> holders vote to approve or reject verifying a *specific campaign*, which is
> what unlocks its funds for withdrawal. This doc describes that mechanism as
> it's actually implemented, in `src/voting.rs`.

## Table of Contents

1. [Two ways a campaign gets verified](#two-ways-a-campaign-gets-verified)
2. [Who can vote, and how much their vote counts](#who-can-vote-and-how-much-their-vote-counts)
3. [Casting a vote](#casting-a-vote)
4. [The approval formula](#the-approval-formula)
5. [Admin-tunable parameters](#admin-tunable-parameters)
6. [Cleaning up after a campaign closes](#cleaning-up-after-a-campaign-closes)

## Two ways a campaign gets verified

A campaign's `is_verified` flag can be set by exactly one of two paths, never
both (`src/voting.rs`):

- **Admin verification** — `verify_campaign` (or the batch form,
  `verify_campaigns`, up to 50 ids per call) — the admin directly marks a
  campaign verified, bypassing voting entirely.
- **Community verification** — `verify_campaign_with_votes` — anyone can call
  this once quorum and the approval threshold are met; it tallies the votes
  already cast and verifies the campaign if they pass.

Calling either path on an already-verified campaign fails, but with different
errors: `verify_campaign` returns `AdminVerificationConflict`,
`verify_campaign_with_votes` returns `CommunityVerificationConflict`. Casting
a vote on an already-verified campaign returns a third error,
`CampaignAlreadyVerified` — see `CAMPAIGN_LIFECYCLE.md` for why these are kept
distinct.

## Who can vote, and how much their vote counts

Voting is **1-address-1-vote**, not balance-weighted (issue #469). Every
eligible voter contributes exactly **1** to either the approve or reject
tally, regardless of how many platform tokens they hold beyond the minimum.

This is a deliberate anti-flash-loan design: if voting weight scaled with
token balance, an attacker could borrow a large balance for one transaction,
vote with inflated weight, and return the tokens before the campaign's
deadline — buying outsized influence with capital they never actually held.
Fixed unit weight per address removes the incentive.

**Eligibility**, checked in `cast_vote`:

- The voter must hold a positive balance of the **platform token** — the
  same token the contract was `init`-ialized with, not whatever currency the
  campaign itself happens to be denominated in (issue #784). This is
  intentional: if voting weight were measured in a campaign's own currency, a
  creator could pick an obscure token and effectively hand voting rights to
  whoever holds it.
- That balance must be at least `min_voting_balance` (admin-configurable, see
  [Admin-tunable parameters](#admin-tunable-parameters); `0` by default,
  meaning any positive balance qualifies).
- The voter must not have already voted on this campaign
  (`AlreadyVoted`) — re-voting is rejected, not overwritten.

`ApproveWeight`/`RejectWeight` storage entries mirror the approve/reject vote
*counts* — they exist for backwards compatibility with the original storage
layout and any indexer reading them, but `verify_with_votes` only ever
consults the counts, never the weight fields, so they carry no security
implication of their own.

## Casting a vote

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source voter \
  --network testnet \
  -- \
  vote_on_campaign \
  --campaign_id 42 \
  --voter "$VOTER_ADDRESS" \
  --approve true
```

Requires the voter's signature (`require_auth`). Fails with:

| Error | When |
|---|---|
| `CampaignNotFound` | No campaign with that id. |
| `CampaignNotActive` | Campaign is cancelled, or its funds were already withdrawn. |
| `CampaignAlreadyVerified` | Campaign is already verified (either path). |
| `DeadlinePassed` | Past `campaign.deadline` — voting is closed. |
| `NotTokenHolder` | Balance is zero, or below `min_voting_balance`. |
| `AlreadyVoted` | This address already voted on this campaign. |

A successful vote emits `campaign_vote_cast` with `(approve: bool, weight:
1i128)` — the weight is always `1`, per the unit-weight model above.

## The approval formula

Anyone can call `verify_campaign_with_votes(campaign_id)` once voting closes
in the campaign's favor. It re-derives the result from the stored counts
rather than trusting a cached outcome:

```text
total_votes  = approve_votes + reject_votes
approval_bps = (approve_votes * 10_000) / total_votes      // BPS_DENOMINATOR = 10_000
```

Two independent conditions must both hold, checked in this order:

1. **Quorum**: `total_votes >= min_votes_quorum`
   (default **3**, admin-configurable up to `MAX_VOTES_QUORUM = 1000`).
   Below quorum → `VotingQuorumNotMet`, regardless of how lopsided the votes
   cast so far are.
2. **Threshold**: `approval_bps >= effective_approval_threshold_bps(category)`
   (default **6000** = 60%, admin floor of `MIN_APPROVAL_THRESHOLD_BPS = 1000`
   = 10% — the floor exists specifically to prevent an admin from
   misconfiguring threshold to near-zero and bypassing real community
   review). Below threshold → `VotingThresholdNotMet`.

`effective_approval_threshold_bps` resolves to a **per-category override** if
the admin has set one for that campaign's `Category` (`Learner`,
`EducationalStartup`, `Educator`, `Publisher`), falling back to the global
`approval_threshold_bps` otherwise (issue #536) — see
[Admin-tunable parameters](#admin-tunable-parameters).

Passing both checks flips `is_verified = true`, transitions the campaign's
lifecycle state, and emits `campaign_verified` with the winning `approve_votes`
count. This call must also happen before `campaign.deadline` passes
(`DeadlinePassed` otherwise) — quorum and threshold being met doesn't mean the
verification call itself can be late.

## Admin-tunable parameters

All of these require the stored contract admin's signature.

**Global quorum and threshold:**

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network testnet \
  -- \
  set_voting_params \
  --admin "$ADMIN_ADDRESS" \
  --min_votes_quorum 5 \
  --approval_threshold_bps 6500
```

Rejects (`ValidationFailed`) if `min_votes_quorum` is `0` or exceeds `1000`,
or if `approval_threshold_bps` is outside `[1000, 10000]`.

**Minimum voting balance** (raises the bar for `NotTokenHolder` above "any
positive balance"):

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network testnet \
  -- \
  set_min_voting_balance \
  --admin "$ADMIN_ADDRESS" \
  --min_balance 1000000000
```

**Per-category threshold override**, and clearing it back to the global
default:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network testnet \
  -- \
  set_category_voting_threshold \
  --admin "$ADMIN_ADDRESS" \
  --category Educator \
  --threshold_bps 7500

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network testnet \
  -- \
  remove_category_voting_threshold \
  --admin "$ADMIN_ADDRESS" \
  --category Educator
```

## Vote Delegation

Token holders may delegate their voting power to another address without transferring tokens, enabling participation by inactive holders and reducing sybil risk.

### Core Mechanics

- **Delegation Model:** A delegator may delegate their voting power to a single delegate; a delegate may receive delegations from multiple delegators.
- **Chaining:** Delegation is **not** chainable. If Alice delegates to Bob and Bob delegates to Charlie, only Bob's delegation is active — Charlie does not inherit Alice's power. Delegators must delegate directly to their chosen representative.
- **Balance Requirement:** The delegator's platform-token **balance** determines voting weight, not the delegate's. If a delegator's balance drops below `min_voting_balance` (or to zero), their vote becomes ineligible, and any existing vote cast on their behalf is still counted but new votes cannot be cast (the delegator falls below the minimum). Conversely, if a delegator receives an incoming token transfer, their balance increases and future delegated votes become eligible again.
- **Direct Voting:** A delegator can vote directly and delegate to a different address in the same contract state — both are independent. The last vote counts:
  - If the delegator votes directly, their `has_voted` flag is set and further attempts to cast a delegated vote return `AlreadyVoted`.
  - If a delegated vote is cast on the delegator's behalf (by the delegate), their `has_voted` flag is set and the delegator cannot vote directly.

### Entrypoints

#### Delegate Votes

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source delegator \
  --network testnet \
  -- \
  delegate_votes \
  --delegator "$DELEGATOR_ADDRESS" \
  --delegate "$DELEGATE_ADDRESS"
```

**Errors:**
- `NotTokenHolder` — Delegator's balance is below `min_voting_balance`.
- `ValidationFailed` — Attempting to delegate to oneself.

**Behavior:**
- Sets `delegator`'s delegate to `delegate` (overwrites any prior delegation).
- Emits `voting_delegated` with `(delegator, delegate)`.

#### Undelegate Votes

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source delegator \
  --network testnet \
  -- \
  undelegate_votes \
  --delegator "$DELEGATOR_ADDRESS"
```

**Behavior:**
- Clears the delegator's delegate entry (if one exists).
- A delegator with no delegate cannot have votes cast on their behalf.
- Emits `voting_undelegated` with `(delegator,)`.

#### Get Delegate

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source delegator \
  --network testnet \
  --is-read-only \
  -- \
  get_delegate \
  --delegator "$DELEGATOR_ADDRESS"
```

**Returns:** The address of the delegator's current delegate, or `None` if no delegation is set.

#### Cast Delegated Vote

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source delegate \
  --network testnet \
  -- \
  cast_delegated_vote \
  --delegator "$DELEGATOR_ADDRESS" \
  --campaign_id 42 \
  --approve true
```

**Errors:**
- `CampaignNotFound`, `CampaignNotActive`, `CampaignAlreadyVerified`, `DeadlinePassed` — Same as direct voting.
- `NotTokenHolder` — Delegator's balance is below `min_voting_balance` (even though the delegate initiated the call).
- `AlreadyVoted` — Delegator already voted directly or has another delegated vote cast.

**Behavior:**
- Requires the **delegator's** signature (`require_auth(delegator)`), not the delegate's — the delegate is merely a convenient caller.
- Validates that the delegator has delegated to the caller.
- Validates that the delegator's balance meets the minimum voting requirement.
- Casts a vote on behalf of the delegator; the vote weight is always `1` (unit weight, per issue #469).
- Sets the delegator's `has_voted` flag for the campaign.
- Emits `campaign_vote_cast` with `(approve: bool, weight: 1i128)`.

### Delegation & Direct Voting Interaction

| Scenario | Outcome |
|----------|---------|
| Delegator votes directly on campaign C | `has_voted[C]` is set; further delegated votes on C return `AlreadyVoted`. |
| Delegated vote cast for delegator on C | `has_voted[C]` is set; delegator cannot vote directly after this. |
| Delegator undelegates after delegation is active | No automatic vote revocation; existing votes remain. Future delegated votes require a re-delegation. |
| Delegator's balance drops below minimum after voting | The vote already cast remains valid; future votes (direct or delegated) are rejected with `NotTokenHolder`. |

### Balance Snapshots

Voting eligibility is recorded in a snapshot at the time each vote is cast (issue #967). The delegator's balance at the moment of the delegated vote determines eligibility; if the balance is later reduced, the vote counts but no new votes can be cast by that delegator on that campaign.

## Cleaning up after a campaign closes

Per-voter `has_voted` entries and the aggregate vote-count storage for a
campaign persist after it's resolved, and can be pruned by the admin once the
campaign has reached a terminal state (`funds_withdrawn` or `is_cancelled`):

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source admin \
  --network testnet \
  -- \
  purge_voting_state \
  --campaign_id 42 \
  --voters '["GVOTERADDR1...","GVOTERADDR2..."]' \
  --finalize_aggregate true
```

`voters` is capped at 50 addresses per call (page through larger voter sets
across multiple calls). `finalize_aggregate: true` additionally removes the
campaign's aggregate vote-count state and emits `voting_state_purged`; pass
`false` on intermediate pages and `true` only on the last one for a campaign
with more than 50 voters.
