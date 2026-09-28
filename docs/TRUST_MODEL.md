# Admin Trust Model

## Overview

ProofOfHeart employs a **single-key admin model** where one address controls sensitive operations that affect the platform's integrity, security, and economics. This document outlines the admin's capabilities, the risks of key compromise, and the security strategy for the current implementation and future upgrades.

## Admin-Gated Operations

The following operations require `assert_admin()` authorization:

### Financial & Fee Control
- **`update_platform_fee`**: Changes the platform's cut from successful campaign withdrawals (bounded by `PLATFORM_FEE_MAX_BPS = 1000` bps / 10% policy ceiling)
- **`set_campaign_fee_override`**: Sets a per-campaign fee override independent of the global platform fee
- **`set_vesting_params`**: Configures the withdrawal release delay and reserve percentage applied to future `withdraw_funds` calls

### Campaign Verification & Lifecycle
- **`verify_campaign` / `admin_verify`**: Force-verifies a campaign, bypassing community voting
- **`set_creation_disabled`**: Pauses campaign creation platform-wide
- **`set_category_duration_cap`**: Sets per-category maximum campaign duration (only affects new campaigns)

### Emergency & Control
- **`pause`**: Pause the entire contract (no contributions, withdrawals, or fund operations)
- **`unpause`**: Resume contract operations
- **`set_emergency_pause_signers`**: Delegate emergency pause authority to multiple signers (still requires admin to unpause)

### Token & Governance
- **`propose_token_update` / `accept_token_update`**: Migrate the platform token (gated by a timelock: `propose_token_update` seeds the pending token, and `accept_token_update` enforces a delay before accepting)
- **`migrate`**: Bump the contract version (for future upgrades)
- **`purge_voting_state`**: Remove voting records for a campaign (used for compliance or bug recovery)

### Governance Parameters
- **`set_voting_params`**: Adjust minimum votes quorum and approval threshold
- **`set_category_voting_threshold`**: Override voting threshold per category
- **`set_min_voting_balance`**: Require a minimum token balance for voting eligibility
- **`set_min_campaign_funding_goal` / `set_max_campaign_funding_goal`**: Adjust funding goal boundaries
- **`set_max_contribution_per_transaction`**: Cap the amount a single `contribute` call can deposit
- **`set_token_allowed_fn`**: Whitelist or remove tokens for per-campaign currencies

### Admin Transfer
- **`initiate_admin_transfer` / `accept_admin_transfer`**: Two-step admin key rotation (prevents accidental transfer)

## Risk Model: Compromised Admin Key

A compromised or malicious admin can:

1. **Force-verify fraudulent campaigns** via `admin_verify`, allowing unreviewed fundraisers to withdraw funds
2. **Set `fee_override` to 0** on high-value campaigns, redirecting the platform's revenue
3. **Retroactively increase vesting delays** via `set_vesting_params`, locking creator funds indefinitely
4. **Pause the contract**, freezing all operations and contributor refunds
5. **Migrate to a malicious token** contract via `propose_token_update` (bounded by `TOKEN_UPDATE_DELAY_SECS` timelock, currently 7 days)
6. **Disable campaign creation** via `set_creation_disabled`, effectively shutting down the platform
7. **Purge voting records** to hide evidence or allow re-voting on critical decisions

## Current Mitigations

### Short-Term (In Place)
- **Hardware wallet or multisig signer** for the admin key reduces the risk of casual compromise
- **Two-step admin transfer** via `initiate_admin_transfer` / `accept_admin_transfer` prevents accidental key rotation
- **Timelock on token updates**: `propose_token_update` enforces a 7-day delay before `accept_token_update` can complete, giving community time to detect malicious proposals
- **Event logging**: All admin actions emit events (e.g., `fee_updated`, `campaign_verified`, `contract_paused`) for on-chain monitoring and off-chain alerting
- **Basis-point bounds**: Critical parameters like `PLATFORM_FEE_MAX_BPS` and `CAMPAIGN_DURATION_MAX_DAYS` limit the worst-case damage (e.g., fees capped at 10%, not 100%)

### Fee Recipient Snapshot (#800)
The platform fee is sent to a fee recipient that is **snapshotted on the first contribution** to a campaign, not to the current admin at withdrawal time. This prevents an admin transfer from redirecting fees that were earned under the previous admin's stewardship.

## Long-Term Strategy: Timelock & Multisig

Future upgrades should implement:

1. **Soroban-native multisig contract**: Require multiple signers (e.g., 2-of-3 or 3-of-5) for sensitive operations, increasing the attack surface from one key to multiple independent parties
2. **Mandatory timelocks** for high-impact operations:
   - `update_platform_fee`: 48-hour delay
   - `set_campaign_fee_override`: 48-hour delay
   - `set_vesting_params`: 7-day delay
   - `pause`: No delay (emergency-only, requires multisig)
3. **Graduated admin roles**:
   - **Emergency admin** (multisig): Can pause and unpause only
   - **Governance admin** (multisig + timelock): Can set fees, vesting, voting parameters
   - **Upgrade admin** (timelock): Can migrate to new contract versions

## Incident Response

If the admin key is suspected compromised:

1. **Immediate**: Call `emergency_pause_signers` to empower trusted signers, then `emergency_pause` to freeze operations
2. **Verify**: Check on-chain events for suspicious admin actions (`verify_campaign`, `fee_override`, `set_vesting_params`)
3. **Prepare**: Initiate `initiate_admin_transfer` to a new, secure key
4. **Execute**: Accepted signer calls `accept_admin_transfer` to rotate the key
5. **Restore**: Call `unpause` once the new admin is in place

## Monitoring & Alerting

Integrators should monitor the following events and alert if suspicious:

- `fee_updated`: Any increase to platform fee (policy-level check)
- `campaign_verified`: Direct admin verifications (frequency baseline)
- `contract_paused`: Any pause event (investigate reason)
- `vesting_params_updated`: Changes to withdrawal delays/reserves (alert creators)
- `token_update_proposed`: Any token migration (check new token contract)
- `admin_updated`: Admin key rotation (verify legitimacy)

## Comparison: Other Platforms

| Platform | Admin Model | Timelock | Multisig | Notes |
|----------|-------------|----------|----------|-------|
| ProofOfHeart (current) | Single key | Token migration only (7d) | None | Event-based monitoring required |
| ProofOfHeart (planned) | Multisig + Timelock | All sensitive ops | Yes | Aligns with DeFi best practices |
| Uniswap Governance | Multisig (6-of-9) | Timelock (2d) | Yes | Established gold standard |

## Recommendations for Users & Operators

1. **Contributors**: Verify campaigns and creators independently; do not rely solely on platform verification
2. **Creators**: Monitor admin events and consider withdrawing funds promptly if suspicious activity is detected
3. **Operators**: Run alert infrastructure on admin events; maintain secure backups of the admin private key and engage a hardware wallet provider for key custody
4. **Auditors**: Review admin operations quarterly and escalate any unauthorized or anomalous calls

## References

- Admin code: `src/admin.rs`
- Threat model: `docs/THREAT_MODEL.md`
- Security audit checklist: `docs/SECURITY_CHECKLIST.md` (if available)
- Issue #825: Long-term multisig/timelock upgrade tracking
