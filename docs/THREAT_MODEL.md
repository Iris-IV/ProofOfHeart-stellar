# Threat Model: ProofOfHeart

This document outlines known security considerations and limitations of the ProofOfHeart smart contract.

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
