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

### Scope

This policy covers the on-chain Soroban smart contract (`src/`) and any official tooling in this repository. Frontend integrations or third-party services built on top of the contract are out of scope unless the vulnerability originates from the contract itself.

### Out of Scope

- Vulnerabilities in dependencies outside our control (e.g. `soroban-sdk`)
- Issues already publicly disclosed
- Theoretical attacks without a realistic exploit path

## Voting Sybil-Resistance Assumptions

Community verification uses a token-gated voting model:

- **Eligibility:** an address must hold a positive balance of the configured token at the time of voting.
- **Quorum:** counts _addresses_ that voted (approve + reject).
- **Threshold:** uses _token-weighted_ approval vs rejection weight (sum of voter balances at vote time).

Security assumptions and limitations:

- This mechanism is **not inherently sybil-resistant**: a single token holder can split tokens across many addresses to inflate the _vote count_ and reach quorum more easily (even though total voting weight stays similar).
- The model assumes the token's distribution and issuance are outside the contract’s control; if token minting is centralized or cheaply obtainable, governance can be captured.
- Admin verification (`verify_campaign`) is a privileged path; users should treat the stored admin as a trust assumption for campaign verification.

## Disclosure Policy

We follow a **coordinated disclosure** process. Please allow us reasonable time to investigate and patch the vulnerability before making any public disclosure.

Thank you for helping keep ProofOfHeart-stellar and its users safe.
