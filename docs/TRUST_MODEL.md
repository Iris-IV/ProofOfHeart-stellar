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

## Emergency Withdrawal Process

### What It Is

When a campaign reaches its funding goal and the creator calls `withdraw_funds`, the contract:

1. Calculates the platform fee (`amount_raised × platform_fee_bps / 10 000`).
2. Transfers the fee directly to the **current admin address**.
3. Transfers the remaining creator amount to `campaign.creator`.

The admin address at withdrawal time is therefore a privileged fee recipient. Because admin transfer has a timelock, a malicious actor who obtains the admin key cannot instantly redirect fee flows on campaigns in progress — but they can queue the redirection and execute it after the delay.

### The 7-Day Timelock Window

`propose_token_update` and `initiate_admin_transfer` / `accept_admin_transfer` enforce a delay before they take effect. This 7-day window is the **only on-chain guarantee** contributors and creators have to detect and react to a hostile admin action before funds leave the contract under different control:

| Action | Timelock | What can go wrong |
|--------|----------|-------------------|
| `propose_token_update` | 7 days (`TOKEN_UPDATE_DELAY_SECS`) | Admin proposes a malicious ERC-20 clone; contributors must act before `accept_token_update` |
| `initiate_admin_transfer` | 7 days | Admin queues a transfer to an attacker address; monitor `admin_transfer_initiated` event |
| `update_platform_fee` | None (current) | Fee can be raised to `PLATFORM_FEE_MAX_BPS` (10%) immediately on the next withdrawal |

### Events to Watch

Subscribe to these contract events to detect a hostile emergency withdrawal attempt:

| Event topic | Meaning | Recommended action |
|-------------|---------|-------------------|
| `token_update_proposed` | New token migration queued | Inspect new token contract; alert if unknown |
| `admin_transfer_initiated` | Admin key rotation queued | Verify new admin address; pause contributions if unrecognised |
| `fee_updated` | Platform fee changed | Alert if fee raised above baseline; track in campaign UI |
| `withdrawal` | Creator or admin withdrawal | Cross-check fee recipient address in event payload |
| `contract_paused` | Contract frozen | Investigate immediately; contributors cannot refund while paused |

To subscribe on-chain, use the Stellar Horizon event-streaming API or the `soroban events` CLI:

```bash
# Stream events for the deployed contract
stellar events --id <CONTRACT_ID> --start-ledger <DEPLOY_LEDGER> \
  --type contract --output json
```

Filter for the `admin_transfer_initiated` and `token_update_proposed` topics to catch the start of any timelock window.

### Recommended Multisig Setup

The single-key admin model is a known risk. Until a Soroban-native multisig contract is deployed (see **Long-Term Strategy** below), operators should:

1. **Use a hardware wallet** (Ledger, Trezor) as the admin signer. Physical confirmation prevents remote key extraction.
2. **Rotate regularly** — use `initiate_admin_transfer` / `accept_admin_transfer` on a fixed schedule (e.g., annually) to prove key control and practice the rotation procedure.
3. **Configure a Stellar multisig account** as the admin address: set the account's `med_threshold` to 2, add co-signers (e.g., a second hardware wallet and a cold-storage key), and require 2-of-3 signatures for the admin operations above. The Stellar base-layer multisig is enforced at the transaction level before the contract even sees the call.
4. **Monitor the timelock window**: set an automated alert for `admin_transfer_initiated` and `token_update_proposed` events. The 7-day window is the last line of defence — act within it.

### How Contributors Can Protect Themselves

- **Check the admin address** before contributing: `stellar contract invoke -- get_admin`. Verify it is the multisig or hardware-wallet address published by the team.
- **Monitor the `token_update_proposed` event**: if a new token contract appears, review its source code. A malicious token can allow the admin to mint unbacked supply.
- **React within the timelock**: if you see `admin_transfer_initiated` to an unknown address, claim a refund (if the campaign is still cancellable) or raise an alarm to other contributors before the 7-day window closes.
- **Prefer campaigns that have passed community voting** over admin-force-verified ones: community verification distributes trust across voters.

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
- `token_update_proposed`: Any token migration (check new token contract; see **Emergency Withdrawal Process**)
- `admin_transfer_initiated`: Admin key rotation queued (verify new address; 7-day window to react)
- `admin_updated`: Admin key rotation completed (verify legitimacy)
- `withdrawal`: Campaign fund withdrawal (cross-check fee recipient address)

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
