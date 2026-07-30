# Trust Model & Administrative Security Assumptions

This document describes the administrative trust model, privileged contract operations, security assumptions, and planned future security enhancements for the **ProofOfHeart** Soroban smart contract.

---

## 1. Current Trust Model

The ProofOfHeart protocol currently operates under a **single administrative authority** model. During contract initialization (`init`), a single Stellar/Soroban `Address` is designated as the protocol administrator (`admin`).

### Core Characteristics

- **Monolithic Privilege**: The administrative key holds full authority over protocol configuration, operational control, campaign verification, parameter tuning, token migration, and storage maintenance.
- **Single Point of Authority**: State-changing administrative functions verify the caller against the stored `admin` address using `assert_admin(&env, &admin)` or `require_auth()`.
- **Two-Step Admin Handover**: Changing the administrator address requires a two-step transfer workflow (`initiate_admin_transfer` -> `accept_admin_transfer`), preventing accidental loss of access to an invalid or mistyped address.

---

## 2. Privileged Operations

Administrative capabilities are divided into several key operational categories. Below is the comprehensive list of privileged methods protected by administrative authorization.

### Operational & Emergency Controls

| Operation | Description | Impact |
| --- | --- | --- |
| `pause` | Halts all non-administrative state-changing functions across the contract. | Freezes contributions, withdrawals, refunds, and voting during security incidents. |
| `unpause` | Restores normal protocol functionality and clears auto-pause status. | Re-enables public interactions. |
| `set_creation_disabled` | Toggles whether new campaigns can be created via `create_campaign`. | Gates new campaign intake independently of the global contract pause status. |

### Campaign Verification & Governance

| Operation | Description | Impact |
| --- | --- | --- |
| `verify_campaign` | Directly marks a single campaign as verified. | Bypasses community voting to verify a campaign immediately. |
| `verify_campaigns` | Batch-verifies up to 50 campaigns in a single invocation. | Bulk verification for operational efficiency. |
| `purge_voting_state` | Deletes stored vote records and aggregate voting state for completed or cancelled campaigns. | Reclaims contract instance/temporary storage rentals after voting finishes. |

### Fee & Financial Parameters

| Operation | Description | Impact |
| --- | --- | --- |
| `update_platform_fee` | Updates the global platform fee rate in basis points (capped at `1000` bps / 10%). | Alters the platform fee deducted from creator withdrawals for future withdrawals. |
| `set_campaign_fee_override` | Sets a per-campaign fee rate (in bps, max 10%) that overrides the global platform fee. | Customizes fee terms for specific campaigns. |
| `set_vesting_params` | Configures global withdrawal release delay (up to 365 days) and reserve percentage (up to 100%). | Alters fund vesting terms applied to subsequent campaign withdrawals. |

### Protocol Configuration & Bounds

| Operation | Description | Impact |
| --- | --- | --- |
| `set_voting_params` | Adjusts global minimum vote quorum and approval threshold (basis points). | Modifies community voting verification rules. |
| `set_min_voting_balance` | Sets the minimum token balance required for an address to cast a vote. | Modifies voting eligibility criteria. |
| `set_min_campaign_funding_goal` | Sets the global minimum funding goal allowed for new campaigns. | Constrains valid funding goal range for creation. |
| `set_max_campaign_funding_goal` | Sets the global maximum funding goal allowed for new campaigns. | Constrains valid funding goal range for creation. |
| `set_category_duration_cap` | Sets a maximum campaign duration for a specific campaign category. | Limits maximum runtime for new campaigns in that category. |
| `remove_category_duration_cap` | Removes the custom duration cap for a category, reverting to global limits. | Removes category-specific duration restrictions. |
| `set_category_voting_threshold` | Sets a custom approval threshold (bps) for a specific campaign category. | Overrides global voting threshold for that category. |
| `remove_category_voting_threshold` | Removes the custom voting threshold for a category. | Reverts category voting to global threshold. |

### System Lifecycle & Maintenance

| Operation | Description | Impact |
| --- | --- | --- |
| `propose_token_update` | Proposes changing the protocol payment token and starts a time delay window. | Initiates two-step token migration. |
| `accept_token_update` | Finalizes payment token update after the delay, requiring zero active campaigns and zero escrowed funds. | Changes the contract's accepted payment token address. |
| `cancel_token_update` | Cancels a pending proposed token update. | Aborts payment token migration. |
| `migrate` | Updates the stored contract version marker following code upgrades. | Synchronizes contract storage with newly deployed code versions. |
| `initiate_admin_transfer` | Nominates a new address to assume the admin role. | Begins two-step administrative handover. |
| `accept_admin_transfer` | Called by pending admin to finalize transfer of admin rights. | Completes handover of administrative authority. |
| `cancel_admin_transfer` | Cancels an in-flight administrative transfer. | Revokes pending administrative nomination. |

---

## 3. Security Assumptions & Operational Risks

### Key Security Assumptions

1. **Trusted Administrator**: The current protocol implementation assumes that the account holding the `admin` key is trusted, secure, and acts in the best interest of protocol users.
2. **Key Security**: Operational security of the admin key (e.g. key storage, signing environment) is assumed to prevent unauthorized access or disclosure.

### Risks Associated with Admin Authority

Users and contributors should be aware of the operational risks inherent to the single-admin model:

- **Admin Key Compromise**: If the single administrator key is compromised by a malicious actor, the attacker could pause protocol operations, modify global fee structures, set punitive vesting parameters, or force-verify unvalidated campaigns.
- **Key Loss / Inaccessibility**: If the administrator key is lost without initiating a transfer to a backup address, administrative functions (such as contract unpausing, fee adjustments, or migration) will become permanently inaccessible.
- **Centralized Parameter Discretion**: Because global parameter changes (such as fee updates or vesting policies) take effect immediately upon administrative invocation, existing participants rely on administrative discretion when parameter changes occur.

> **User Advisory**: Participants should understand these single-administrator trust assumptions and associated operational risks before interacting with the protocol or depositing funds into campaigns.

---

## 4. Future Improvements

To progressively decentralize governance and minimize administrative trust assumptions, future protocol upgrades may introduce the following architectural enhancements:

1. **Soroban Multisig Governance**: Replacing the single administrative `Address` with a multi-signature account contract (requiring $M$-of-$N$ consensus among distinct keyholders) for all administrative actions.
2. **Timelock Mechanisms**: Introducing on-chain timelock delays for sensitive parameter modifications (such as fee updates, vesting changes, or token migration), allowing users time to inspect proposed changes and exit if desired.
3. **Hardware-Backed Operational Security**: Requiring administrative interactions to originate from Hardware Security Modules (HSMs) or multi-party computation (MPC) key management solutions.
4. **Role Separation (Least Privilege Architecture)**: Decomposing the monolithic `admin` role into granular, purpose-built permissions (e.g., separating an emergency `Pauser` role, a `CampaignVerifier` role, and a `ParameterGovernor` role) to restrict blast radius in case of partial key compromise.

---

*Note: The improvements described above are planned directions for future protocol iterations and are not implemented in the current contract release.*
