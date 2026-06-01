# Threat Model: ProofOfHeart

This document outlines known security considerations and limitations of the ProofOfHeart smart contract.

## Verification System

### Verification Mechanisms

The contract supports two verification paths:

#### 1. Admin Verification (`admin_verify`)

- **Access Control**: Requires admin authorization via `admin.require_auth()`
- **Idempotency**: Rejects if campaign already verified
- **Griefing Risk**: **MINIMAL** - Only admin can call; no third-party access

#### 2. Community Voting Verification (`verify_with_votes`)

- **Access Control**: Permissionless call (anyone can invoke)
- **Requirements**:
  - Minimum quorum of votes (default: 3)
  - Approval threshold (default: 60% token-weighted)
  - Campaign must not already be verified
- **Griefing Risk**: **MODERATE** - See griefing vectors below

### Griefing Attack Vectors

#### Vector 1: Timing Attacks via Premature Verification

**Scenario**: Attacker calls `verify_with_votes()` before legitimate votes are cast, potentially verifying a campaign with insufficient community consensus.

**Mitigation**:
- Quorum requirement (minimum 3 votes) prevents single-voter verification
- Token-weighted voting ensures large holders have proportional influence
- Approval threshold (60%) requires supermajority consensus
- Campaign remains unverified if thresholds not met

**Risk Level**: **LOW** - Quorum and threshold requirements prevent premature verification

#### Vector 2: Governance Griefing via Vote Spam

**Scenario**: Attacker creates many low-balance accounts to spam votes and manipulate voting outcomes.

**Mitigation**:
- Minimum voting balance threshold (`min_voting_balance`) now enforced
- Configurable by admin to prevent low-balance spam
- Default: 0 (no restriction) for backwards compatibility
- Can be adjusted via `set_min_voting_balance()` admin function

**Risk Level**: **REDUCED** - Minimum balance threshold prevents low-balance spam attacks

#### Vector 3: Denial of Service via Repeated Verification Calls

**Scenario**: Attacker repeatedly calls `verify_with_votes()` on the same campaign to consume gas/resources.

**Mitigation**:
- Idempotency check: `if campaign.is_verified { return Err(...) }`
- Once verified, subsequent calls immediately fail
- No state changes occur on repeated calls
- Minimal gas consumption after first verification

**Risk Level**: **LOW** - Idempotency prevents repeated state changes

#### Vector 4: Campaign State Manipulation

**Scenario**: Attacker attempts to verify cancelled or inactive campaigns.

**Mitigation**:
- Verification only checks vote counts, not campaign state
- However, cancelled campaigns cannot receive votes (checked in `cast_vote`)
- Inactive campaigns cannot receive votes
- Verification of cancelled campaigns is technically possible but harmless (campaign already inactive)

**Risk Level**: **LOW** - Cancelled/inactive campaigns cannot accumulate votes

### Timing Considerations

#### Ledger Timestamp Dependency

- Verification uses `env.ledger().timestamp()` indirectly (via vote timestamps)
- No direct timestamp manipulation in verification logic
- Voting is not time-gated; votes can be cast anytime before verification

**Risk Level**: **LOW** - No timestamp-based griefing vectors

#### TTL (Time-To-Live) Expiration

- Vote data has 30-day TTL with automatic bumping
- If votes expire before verification, verification would fail
- Attacker could theoretically delay verification to cause TTL expiration

**Risk Level**: **VERY LOW** - Requires 30+ day delay; impractical attack

## Voting System

### Token-weighted Sybil Attack (#177)

**Description**: The current voting system uses a token-weighted model where a voter's influence is determined by their token balance at the time of voting. While the contract prevents an address from voting multiple times (`HasVoted` check), it does not prevent a user from transferring tokens between multiple addresses to vote repeatedly with the same capital.

**Attack Vector**:
1. An attacker holds 1,000,000 tokens in Address A.
2. Attacker votes with Address A (weight: 1,000,000).
3. Attacker transfers 1,000,000 tokens from Address A to Address B.
4. Attacker votes with Address B (weight: 1,000,000).
5. The process can be repeated across any number of addresses.

**Mitigation Status**:
This is a **known limitation** of the current implementation. A robust fix would require a consistent "ledger snapshot" of token balances at a specific point in time (e.g., campaign creation), which is not natively supported by standard SEP-41 token contracts without a specialized history oracle or custom token logic.

**Risk Management**:
- **Monitoring**: Integration layers (frontends/indexers) should monitor for large token transfers between addresses that subsequently vote on the same campaign.
- **Minimum Balance**: The `MinVotingBalance` setting helps increase the cost of creating multiple voting accounts but does not prevent the attack by a sufficiently capitalized entity.
- **Future Improvements**: Future versions may explore integration with specialized governance tokens that support historical snapshots.
