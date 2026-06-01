## Contribution Cap Policy

`max_contribution_per_user` is enforced as a **lifetime cap per campaign**.

- A contributor can never exceed this cap across all successful contribution attempts in the same campaign.
- Refunds (`claim_refund`) reset the current withdrawable contribution balance, but **do not** reset lifetime contribution used for cap enforcement.
- A cap value of `0` means unlimited contributions.

This policy prevents refund/re-contribute loops from bypassing creator-configured per-user limits.
