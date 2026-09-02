# Contribution Cap Policy

`max_contribution_per_user` is enforced as a **lifetime cap per campaign** on the ProofOfHeart Stellar smart contract.

## Core Rules

1. **Lifetime Campaign Limit**:
   - A contributor's total accumulated contributions within a specific campaign can never exceed `max_contribution_per_user`.
   - The cap applies across all contribution transactions executed by a given address.

2. **Refund & Re-contribution Protections**:
   - Calling `claim_refund` resets the active withdrawable balance for a user.
   - **Crucial Security Invariant**: Refunds **do not** reset the accumulated lifetime contribution counter used for cap enforcement.
   - This explicitly prevents malicious refund/re-contribute loops designed to bypass creator-configured per-user caps or manipulate batch campaign limits.

3. **Unlimited Contributions (`0` Value)**:
   - Setting `max_contribution_per_user` to `0` configures the campaign for unlimited per-user contribution capacity (subject only to global campaign funding targets).

## Technical Implementation Details

- **Storage Key**: `UserContribution(Address, CampaignId)` tracks both `lifetime_total` and `current_balance`.
- **Validation**: On every `contribute` call, the contract verifies `lifetime_total + new_amount <= max_contribution_per_user` whenever `max_contribution_per_user > 0`.
- **Admin Configuration**: Campaign creators define `max_contribution_per_user` upon campaign initialization; parameters are immutable post-launch.
