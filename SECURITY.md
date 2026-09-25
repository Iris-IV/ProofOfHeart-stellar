# Security Policy

## Supported Versions

The following versions of ProofOfHeart-stellar are currently receiving security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue in ProofOfHeart-stellar, please **do not** open a public GitHub issue.

### How to Report

Send a detailed report to:

**Email:** security@proofofheart.io (domain confirmed: proofofheart.io)

Please include the following in your report:

- A clear description of the vulnerability
- Steps to reproduce the issue
- The potential impact (e.g. fund loss, unauthorised access, overflow)
- Any proof-of-concept code or transaction examples if applicable

### What to Expect

- **Acknowledgement:** We will acknowledge receipt of your report within **48 hours**.
- **Status updates:** We will keep you informed of our progress and expected timeline.
- **Resolution:** We aim to resolve critical vulnerabilities within **7 days** and non-critical issues within **30 days**.
- **Credit:** With your permission, we will credit you in the release notes once the vulnerability is fixed.

### Severity Definitions

We triage reports using the following severity levels:

| Severity | Definition | Example |
|----------|------------|---------|
| **P0 — Critical** | Direct loss or theft of funds, permanent freezing of funds, or a way to bypass admin/auth controls. Can be exploited without special privileges. | Draining escrowed contributions, minting unauthorised withdrawals, admin impersonation. |
| **P1 — High** | Significant contract malfunction that does not directly move funds but breaks a core invariant (e.g. accounting, voting, campaign lifecycle) or can be chained into a P0. | Incorrect fee/revenue-share accounting, vote-count manipulation, denial of service on a core entrypoint. |
| **P2 — Medium/Low** | Limited-impact bugs, edge cases, or hardening opportunities that do not put funds or contract integrity at immediate risk. | Missing input validation with no exploitable consequence, gas/resource inefficiencies, informational issues. |

### Responsible Disclosure Timeline

We ask reporters to follow **coordinated disclosure**:

1. Report the issue privately via the email above — do not open a public issue, PR, or forum post describing the vulnerability.
2. We will work with you to validate, reproduce, and fix the issue.
3. We request **90 days** from the initial report before any public disclosure, to give us time to ship and deploy a fix. This window can be shortened by mutual agreement (e.g. once a fix is live and users have had time to upgrade) or extended if the fix requires unusual coordination (e.g. a contract migration).
4. If 90 days pass without a resolution or agreed extension, the reporter may disclose publicly, but we ask that you still coordinate the exact timing and content with us where possible.

### Bug Bounty & Rewards Scope

Security reports targeting core Soroban contract logic in `src/` are evaluated for rewards and public accreditation:

- **In-Scope Contracts & Entrypoints** (public methods of `ProofOfHeart` in `src/lib.rs`):
  - `contribute()`, `batch_contribute()`, `claim_refund()`, `withdraw_funds()`, `withdraw_reserve()`, `claim_milestone()` — Campaign escrow & asset accounting
  - `deposit_revenue()`, `claim_revenue()`, `claim_creator_revenue()` — Revenue-share accounting
  - `vote_on_campaign()`, `verify_campaign()`, `verify_campaign_with_votes()` — Governance & voting invariants
  - Admin entrypoints (`pause`, `update_platform_fee`, `propose_token_update` / `accept_token_update`, `initiate_admin_transfer`, `emergency_withdraw`, …) and the storage layer in `src/storage.rs`
- **Reward Tiers**:
  - **P0 Critical** (Direct fund drain / auth bypass): Eligible for up to $2,500 USDC bounty + release notes credit.
  - **P1 High** (State corruption / fee breakdown): Eligible for up to $1,000 USDC bounty + release notes credit.
  - **P2 Medium/Low**: Eligible for public credit in release notes.

### Audits

No formal third-party security audit has been conducted on this contract to date. Given the contract handles escrowed funds, we recommend treating it as **unaudited** and exercising appropriate caution until an audit is completed.

### Scope

This policy covers the on-chain Soroban smart contract (`src/`) and official tooling in this repository. Frontend integrations or third-party services built on top of the contract are out of scope unless the vulnerability originates from contract logic itself.

### Out of Scope

- Vulnerabilities in dependencies outside our control (e.g. `soroban-sdk`)
- Issues already publicly disclosed
- Theoretical attacks without a realistic exploit path

## Voting Sybil-Resistance Assumptions

Community verification uses a token-gated, **one-address-one-vote** model (`src/voting.rs`, since #469):

- **Eligibility:** an address must hold at least the configured minimum balance of the platform token (`get_min_voting_balance`) at the time of voting, and may vote once per campaign (`AlreadyVoted`).
- **Quorum:** counts _addresses_ that voted (approve + reject), compared against `get_min_votes_quorum`.
- **Threshold:** approval is computed from **vote counts**, not token balances, against `get_approval_threshold_bps` or the per-category override (`get_category_voting_threshold`). Balances are only an eligibility gate, so a flash-loaned balance cannot inflate voting weight.

Security assumptions and limitations:

- This mechanism is **not sybil-resistant**: anyone holding enough tokens can split them across many addresses that each meet the minimum balance, and every address gets one vote. That lets them inflate both the vote count and the approval ratio. The minimum voting balance is the main economic cost of doing this.
- The model assumes the token's distribution and issuance are outside the contract's control; if token minting is centralized or cheaply obtainable, governance can be captured.
- Admin verification (`verify_campaign`, `verify_campaigns`) is a privileged path; users should treat the stored admin as a trust assumption for campaign verification.

## Disclosure Policy

We follow a **coordinated disclosure** process. Please allow us reasonable time to investigate and patch the vulnerability before making any public disclosure.

Thank you for helping keep ProofOfHeart-stellar and its users safe.
