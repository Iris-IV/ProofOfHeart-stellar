# Threat Model

This document describes the security assumptions, trust boundaries, and known risks for the ProofOfHeart smart contract.

## Table of Contents

1. [System Overview](#system-overview)
2. [Trust Boundaries](#trust-boundaries)
3. [Threat Actors](#threat-actors)
4. [Attack Surfaces](#attack-surfaces)
5. [Known Risks & Mitigations](#known-risks--mitigations)
6. [Assumptions](#assumptions)
7. [Out of Scope](#out-of-scope)

---

## System Overview

ProofOfHeart is a decentralized crowdfunding platform built on Stellar's Soroban smart contract platform. The contract manages:

- Campaign creation and lifecycle management
- Token-based contributions and withdrawals
- Community-driven verification via token-weighted voting
- Revenue sharing for educational startup campaigns
- Platform fee collection

**Key Components:**

- Smart contract (on-chain logic)
- Token contract (for contributions, typically wrapped assets)
- Admin account (privileged governance role)
- Campaign creators (fundraisers)
- Contributors (funders)
- Voters (token holders participating in verification)

---

## Trust Boundaries

### 1. Contract Code Boundary

**Trusted:**

- The deployed smart contract code (audited and immutable once deployed)
- Soroban SDK and runtime environment
- Stellar consensus mechanism

**Untrusted:**

- All external callers (users, bots, malicious actors)
- Token contract behavior (assumed to follow standard interface but not guaranteed)
- Off-chain frontend applications

### 2. Admin Boundary

**Trusted:**

- Admin account holder (platform operator)
- Admin can:
  - Verify campaigns directly
  - Update platform fee (max 10%)
  - Pause/unpause contract
  - Update voting parameters
  - Transfer admin role

**Risk:** Admin compromise = full platform control. See [Admin Key Compromise](#1-admin-key-compromise).

### 3. Token Contract Boundary

**Assumed Trusted:**

- Token contract follows Soroban token standard
- Token balances are accurate and cannot be manipulated
- Token transfers execute atomically

**Risk:** Malicious or buggy token contract can break contribution/withdrawal logic. See [Token Contract Vulnerabilities](#4-token-contract-vulnerabilities).

---

## Threat Actors

### 1. Malicious Campaign Creator

**Capabilities:**

- Create campaigns with misleading information
- Cancel campaigns after receiving contributions (to grief contributors)
- Manipulate revenue sharing deposits

**Motivations:**

- Steal funds via fake campaigns
- Grief contributors
- Exploit revenue sharing mechanism

### 2. Malicious Contributor

**Capabilities:**

- Contribute to campaigns
- Attempt double-refund attacks
- Spam contributions to inflate metrics

**Motivations:**

- Exploit refund logic for profit
- Manipulate campaign statistics
- Grief campaign creators

### 3. Malicious Voter

**Capabilities:**

- Cast votes on campaign verification
- Sybil attack via multiple low-balance accounts
- Collude with other voters

**Motivations:**

- Verify fraudulent campaigns
- Block legitimate campaigns from verification
- Manipulate governance for profit

### 4. External Attacker (No Account)

**Capabilities:**

- Observe on-chain data
- Analyze contract logic
- Attempt reentrancy or logic exploits

**Motivations:**

- Drain contract funds
- Disrupt platform operations
- Exploit smart contract vulnerabilities

### 5. Compromised Admin

**Capabilities:**

- All admin privileges (see [Admin Boundary](#2-admin-boundary))

**Motivations:**

- Steal platform fees
- Verify fraudulent campaigns
- Disrupt platform via pause/unpause

---

## Attack Surfaces

### 1. Campaign Creation & Management

**Entry Points:**

- `create_campaign()`
- `update_campaign()`
- `update_campaign_description()`
- `cancel_campaign()`
- `initiate_campaign_transfer()` / `accept_campaign_transfer()`

**Risks:**

- Fake campaigns with misleading information
- Campaign spam (no rate limiting)
- Creator griefing via cancellation after contributions
- Campaign ownership transfer exploits

### 2. Contribution & Withdrawal

**Entry Points:**

- `contribute()`
- `withdraw_funds()`
- `claim_refund()`

**Risks:**

- Double-refund attacks
- Reentrancy during token transfers
- Arithmetic overflow/underflow in contribution accounting
- Withdrawal before deadline (if logic flawed)

### 3. Verification System

**Entry Points:**

- `verify_campaign()` (admin)
- `verify_campaign_with_votes()` (community)
- `vote_on_campaign()`

**Risks:**

- Sybil attacks on voting (mitigated by minimum balance threshold)
- Premature verification before community consensus
- Admin abuse of verification power
- Vote spam / griefing

### 4. Revenue Sharing

**Entry Points:**

- `deposit_revenue()`
- `claim_revenue()`
- `claim_creator_revenue()`

**Risks:**

- Revenue accounting errors
- Double-claim attacks
- Creator withholding revenue deposits
- Rounding errors in proportional distribution

### 5. Admin Functions

**Entry Points:**

- `update_platform_fee()`
- `update_admin()`
- `pause()` / `unpause()`
- `set_voting_params()`
- `set_min_voting_balance()`

**Risks:**

- Admin key compromise
- Excessive platform fee (mitigated by 10% cap)
- Malicious pause to block withdrawals
- Voting parameter manipulation

---

## Known Risks & Mitigations

### 1. Admin Key Compromise

**Risk:** If the admin private key is compromised, attacker gains full control over:

- Campaign verification
- Platform fee updates (up to 10%)
- Contract pause/unpause
- Voting parameter changes

**Severity:** CRITICAL

**Mitigations:**

- Admin key must be stored securely (hardware wallet recommended)
- Consider multi-sig admin in future upgrade
- Monitor admin actions via on-chain events
- Platform fee capped at 10% (1000 basis points)

**Residual Risk:** HIGH - Single point of failure

### 2. Sybil Attacks on Voting

**Risk:** Attacker creates many low-balance accounts to:

- Inflate vote count to reach quorum
- Manipulate approval/rejection ratios

**Severity:** MODERATE

**Mitigations:**

- Minimum voting balance threshold (`min_voting_balance`)
- Token-weighted voting (not just vote count)
- Quorum requirement (minimum 3 votes by default)
- Approval threshold (60% token-weighted by default)

**Residual Risk:** LOW-MODERATE - Requires significant token holdings to manipulate

### 3. Token Contract Vulnerabilities

**Risk:** Malicious or buggy token contract could:

- Fail to transfer tokens during contributions
- Allow double-spending
- Freeze user balances

**Severity:** HIGH

**Mitigations:**

- Use well-audited token contracts (e.g., Stellar wrapped assets)
- Token contract address set during initialization (immutable)
- Atomic token transfers via Soroban SDK

**Residual Risk:** MODERATE - Depends on token contract security

### 4. Reentrancy Attacks

**Risk:** Malicious token contract could reenter ProofOfHeart during token transfers to:

- Claim multiple refunds
- Withdraw funds multiple times

**Severity:** HIGH

**Mitigations:**

- Checks-Effects-Interactions pattern enforced
- State updates before external calls
- Idempotency checks (e.g., `funds_withdrawn` flag)
- Soroban's execution model limits reentrancy vectors

**Residual Risk:** LOW - Soroban architecture provides strong reentrancy protection

### 5. Arithmetic Overflow/Underflow

**Risk:** Integer overflow in contribution accounting could:

- Allow contributors to claim more than deposited
- Corrupt campaign funding totals

**Severity:** HIGH

**Mitigations:**

- Rust's checked arithmetic (panics on overflow)
- Soroban SDK uses `i128` for token amounts (large range)
- Explicit overflow checks in revenue sharing calculations

**Residual Risk:** LOW - Rust's type system prevents most overflow issues

### 6. Griefing via Campaign Cancellation

**Risk:** Creator cancels campaign after receiving contributions to grief contributors (who must then claim refunds).

**Severity:** LOW

**Mitigations:**

- Cancellation is a legitimate feature (allows creators to exit)
- Contributors can claim full refunds after cancellation
- Reputation system (off-chain) can track creator behavior

**Residual Risk:** LOW - Economic incentive against griefing (creator loses platform credibility)

### 7. Timestamp Manipulation

**Risk:** Validators manipulate `ledger.timestamp()` to:

- Extend campaign deadlines
- Bypass time-based checks

**Severity:** LOW

**Mitigations:**

- Stellar consensus ensures timestamp accuracy (within ~5 seconds)
- Timestamp manipulation requires validator collusion (impractical)
- Deadlines use block timestamps, not user-provided values

**Residual Risk:** VERY LOW - Stellar's consensus model prevents timestamp attacks

### 8. Denial of Service (DoS)

**Risk:** Attacker spams contract with:

- Campaign creation
- Contribution transactions
- Vote casting

**Severity:** MODERATE

**Mitigations:**

- Stellar transaction fees rate-limit spam
- No unbounded loops in contract code
- Storage TTL prevents indefinite data accumulation

**Residual Risk:** LOW - Economic cost of spam attacks is prohibitive

### 9. Front-Running

**Risk:** Attacker observes pending transactions and:

- Contributes before others to claim early contributor benefits
- Votes before others to influence verification outcome

**Severity:** LOW

**Mitigations:**

- No early contributor benefits in current design
- Voting is permissionless and order-independent
- Stellar's fast finality (~5 seconds) limits front-running window

**Residual Risk:** LOW - Limited economic incentive for front-running

### 10. Campaign Verification Bypass

**Risk:** Attacker verifies fraudulent campaign via:

- Admin verification (requires admin compromise)
- Community voting (requires token holdings + quorum)

**Severity:** MODERATE

**Mitigations:**

- Admin verification requires admin authorization
- Community voting requires quorum + approval threshold + minimum balance
- Verification is idempotent (cannot re-verify)

**Residual Risk:** MODERATE - Depends on admin security and token distribution

---

## Assumptions

### Security Assumptions

1. **Stellar Consensus is Secure** — Validators do not collude to manipulate timestamps or censor transactions.
2. **Soroban Runtime is Correct** — No bugs in Soroban SDK or execution environment.
3. **Token Contract is Standard-Compliant** — Token contract follows Soroban token interface and does not have malicious logic.
4. **Admin Key is Secure** — Admin private key is stored securely and not compromised.
5. **Rust Compiler is Correct** — No bugs in Rust compiler that could introduce vulnerabilities.

### Economic Assumptions

1. **Token Has Value** — The contribution token has economic value, making spam attacks costly.
2. **Platform Fee is Reasonable** — 10% maximum fee cap prevents excessive rent extraction.
3. **Contributors Act Rationally** — Contributors will claim refunds if campaigns fail or are cancelled.
4. **Creators Act Honestly** — Most creators are legitimate and will not grief contributors.

### Governance Assumptions

1. **Token Distribution is Decentralized** — No single entity controls majority of voting tokens.
2. **Voters Participate Honestly** — Voters verify campaigns based on merit, not collusion.
3. **Admin Acts in Good Faith** — Admin uses privileges responsibly and does not abuse power.

---

## Out of Scope

The following are explicitly **out of scope** for this threat model:

### 1. Frontend Security

- XSS, CSRF, or other web vulnerabilities in frontend applications
- Wallet integration security (e.g., Freighter, Albedo)
- User phishing or social engineering attacks

**Mitigation:** Frontend developers must follow web security best practices.

### 2. Off-Chain Infrastructure

- API server vulnerabilities
- Database security
- DNS hijacking or domain takeover

**Mitigation:** Off-chain infrastructure must be secured independently.

### 3. User Operational Security

- Private key theft or loss
- Malware on user devices
- Phishing attacks targeting users

**Mitigation:** Users must follow wallet security best practices.

### 4. Stellar Network Attacks

- 51% attacks on Stellar consensus
- Validator collusion
- Network-level DoS attacks

**Mitigation:** Stellar Foundation is responsible for network security.

### 5. Regulatory & Legal Risks

- Securities law compliance
- Tax implications
- Jurisdictional restrictions

**Mitigation:** Legal counsel must assess regulatory compliance.

### 6. Economic Attacks on Token

- Token price manipulation
- Liquidity attacks
- Market manipulation

**Mitigation:** Token economics are outside contract scope.

---

## Reporting Security Issues

If you discover a security vulnerability, please **do not** open a public GitHub issue.

**Report to:** security@proofofheart.io

See [SECURITY.md](../SECURITY.md) for full reporting guidelines.

---

**Last Updated:** April 25, 2026
