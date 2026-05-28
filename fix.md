[BUG] Revenue share base does not adjust after refunds, leaving non-refunded contributors short

Summary
claim_revenue computes a contributor's pro-rata share as
(contribution * contributor_pool) / campaign.amount_raised.

amount_raised is set during contribute and is never decreased on claim_refund. If a campaign reaches an edge state where some contributors refund (creator cancels post-funding-met but pre-withdrawal, or contract is later paused/unpaused with deposits/refunds interleaved), the denominator stays inflated. Remaining contributors will receive less revenue than their fair share, and the leftover dust stays orphaned in the pool.

Where
src/lib.rs claim_revenue (line ~480)
src/lib.rs claim_refund (sets contribution to 0 but does not touch campaign.amount_raised)

Reproduction
Create a campaign with has_revenue_sharing=true.
Two contributors each contribute 1000.
Goal met (≥2000), but creator calls cancel_campaign before withdraw.
Contributor A claims a refund (now contribution=0, amount_raised still 2000).
Creator deposits revenue (1000) into the pool.
Contributor B calls claim_revenue and receives 1000 * pool / 2000 = half — but they should receive the full pool, since they are the only remaining contributor.

Fix options
Block this state: forbid cancel_campaign once amount_raised >= funding_goal (only the funded path can proceed).
OR maintain a separate effective_raised that tracks live (non-refunded) contributions and use it as the denominator in claim_revenue and claim_creator_revenue.

Acceptance criteria
 A test reproducing the scenario shows correct payout (no orphaned tokens, no shortchange).
 Decision documented in docs/.