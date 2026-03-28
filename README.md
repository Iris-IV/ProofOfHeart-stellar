# ProofOfHeart — Stellar Smart Contract

**A decentralized launchpad where the community — not a corporation — validates a cause.**

ProofOfHeart empowers everyday people to rally behind the causes they believe in. By leveraging blockchain transparency and community-driven governance, it removes gatekeepers from the fundraising process and puts trust back where it belongs: in the hands of the people.

This repository contains the **Soroban smart contract** that powers the on-chain logic for campaign management, contributions, fund withdrawal, refunds, and revenue sharing.

## Vision & Mission

**Vision** — A world where any meaningful cause can receive support without needing permission from a centralized authority.

**Mission** — To build an open, transparent launchpad that lets communities discover, validate, and fund causes through decentralized consensus — ensuring that every voice counts and every contribution is accounted for on-chain.

### Core Principles

- **Community First** — Causes are validated by the people, not by a corporate board.
- **Radical Transparency** — Every decision and transaction lives on-chain for anyone to verify.
- **Permissionless Participation** — Anyone can propose, support, or challenge a cause.
- **Trust Through Code** — Smart contracts enforce the rules, removing the need for intermediaries.

## Tech Stack

| Layer | Technology |
| --- | --- |
| Blockchain | [Stellar](https://stellar.org/) |
| Smart Contract Platform | [Soroban](https://soroban.stellar.org/) |
| Language | Rust |
| SDK | [soroban-sdk 20.1.0](https://crates.io/crates/soroban-sdk) |

## Smart Contract Features

### Campaign Management
- **Create Campaign** — Launch a new fundraising campaign with a title, description, funding goal, deadline, and category (`Learner`, `EducationalStartup`, `Educator`, `Publisher`).
- **Cancel Campaign** — Campaign creators can cancel an active campaign at any time, enabling contributor refunds.

### Campaign Verification
- **Admin Verification** — Platform admin can mark a campaign as verified via `verify_campaign`.
- **Community Voting Verification** — Token holders can vote and verify via `verify_campaign_with_votes` based on quorum and approval threshold.

### Contributions & Withdrawals
- **Contribute** — Anyone can contribute tokens to an active campaign before the deadline.
- **Withdraw Funds** — Once the funding goal is met, the campaign creator can withdraw raised funds (minus a configurable platform fee, max 10%).
- **Claim Refund** — Contributors can reclaim their tokens if a campaign is cancelled or fails to reach its goal by the deadline.

### Revenue Sharing
- **Deposit Revenue** — `EducationalStartup` campaigns can opt into revenue sharing; the creator deposits revenue back into the contract.
- **Claim Revenue** — Contributors to a revenue-sharing campaign can claim their proportional share of deposited revenue.

### View Functions
- `get_campaign` — Retrieve campaign details by ID.
- `get_contribution` — Check a contributor's amount for a given campaign.
- `get_revenue_pool` — View the total revenue pool for a campaign.
- `get_revenue_claimed` — Check how much revenue a contributor has already claimed.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)
- `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

### Build

```bash
# Clone the repository
git clone https://github.com/dmystical-coder/ProofOfHeart-stellar.git
cd ProofOfHeart-stellar

# Build the contract
cargo build --target wasm32-unknown-unknown --release
```

### Test

```bash
cargo test
```

## Deployment

For detailed instructions on deploying the contract to Stellar testnet and mainnet, see the [**Deployment Guide**](docs/DEPLOYMENT.md). It covers:

- Soroban CLI setup and configuration
- Testnet deployment with copy-pasteable examples
- Mainnet deployment and cost considerations
- Contract initialization with admin, token, and fee parameters
- Token setup for the platform
- Verification and troubleshooting

## Project Structure

```
ProofOfHeart-stellar/
├── Cargo.toml          # Project manifest & dependencies
└── src/
    ├── lib.rs          # Smart contract implementation
    └── test.rs         # Unit tests
```

## Related Repositories

| Repository | Description |
| --- | --- |
| [ProofOfHeart-frontend](https://github.com/dmystical-coder/ProofOfHeart-frontend) | Next.js frontend application |

## Contributing

We welcome contributors of all experience levels! Check the [Issues](../../issues) tab for open tasks — many are tagged as `good first issue` to help you get started.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
