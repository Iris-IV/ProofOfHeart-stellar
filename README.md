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
- **Create Campaign** — Launch a new fundraising campaign via `CreateCampaignParams` (title, description, funding goal, duration in days, category, revenue-sharing settings, and per-user contribution cap).
- **Update Campaign** — Edit title and/or description before any contributions are received.
- **Extend Deadline** — Extend a campaign's deadline once (within the 365-day maximum).
- **Cancel Campaign** — Campaign creators can cancel an active campaign, enabling contributor refunds.
- **Ownership Transfer** — Two-step creator transfer: `initiate_campaign_transfer` → `accept_campaign_transfer` (or `cancel_campaign_transfer`).

### Campaign Verification
- **Admin Verification** — Platform admin can mark a single campaign as verified via `verify_campaign`, or batch-verify up to 50 at once with `verify_campaigns`.
- **Community Voting Verification** — Token holders vote via `vote_on_campaign`; `verify_campaign_with_votes` finalises verification once quorum and approval threshold are met.
- **Configurable Voting Params** — Admin can set `min_votes_quorum`, `approval_threshold_bps`, and `min_voting_balance` via dedicated admin functions.
- **Voting State Cleanup** — `purge_voting_state` lets admin reclaim storage after voting concludes.

### Contributions & Withdrawals
- **Contribute** — Anyone can contribute tokens to an active, non-paused campaign before the deadline; a per-user cap can be set at the campaign or personal level.
- **Withdraw Funds** — Once the funding goal is met, the campaign creator withdraws raised funds minus a configurable platform fee (max 10%). A vesting reserve can be withheld and released after a configurable delay via `withdraw_reserve`.
- **Claim Refund** — Contributors reclaim tokens if a campaign is cancelled or fails to reach its goal by the deadline. State is updated before the token transfer (checks-effects-interactions).

### Revenue Sharing
- **Deposit Revenue** — `EducationalStartup` campaigns that opted in receive revenue deposits from the creator.
- **Claim Revenue** — Contributors claim their pro-rata share of deposited revenue based on their effective contribution.
- **Claim Creator Revenue** — Creators claim their portion of deposited revenue (the share not distributed to contributors).

### Platform Administration
- **Pause / Unpause** — Admin can halt all state-changing operations; the contract also auto-pauses on anomalous contribution activity.
- **Creation Gate** — Admin can disable new campaign creation independently of the global pause.
- **Fee Management** — Update the global platform fee or set a per-campaign fee override.
- **Category Duration Caps** — Admin can set or remove per-category maximum campaign durations.
- **Funding Goal Bounds** — Admin can override the global minimum and maximum funding goal.
- **Admin Transfer** — Two-step admin handover: `initiate_admin_transfer` → `accept_admin_transfer`.
- **Token Migration** — Two-step platform token change: `propose_token_update` → `accept_token_update`.
- **Contract Migration** — `migrate` advances the stored contract version with version-guard safety.

### View Functions

**Campaign queries**
- `get_campaign` / `get_campaign_optional` — Retrieve campaign details by ID.
- `get_campaign_count` — Total campaigns ever created.
- `get_campaigns_by_category` — Paginated list filtered by category.
- `get_creator_campaigns` — Paginated list of campaigns by a specific creator.
- `list_campaigns` — Raw paginated list by insertion order.
- `list_active_campaigns` — Paginated list of currently active campaigns with a cursor for continued scanning.
- `get_platform_stats` — Aggregate platform metrics (totals, active, verified, cancelled).

**Contribution & revenue queries**
- `get_contribution` — A contributor's current balance for a campaign.
- `get_lifetime_contribution` — Cumulative contributed amount (used for cap enforcement).
- `get_total_contributors_count` — Number of contributors to a campaign.
- `get_total_raised_global` — Sum of all tokens raised across all campaigns.
- `get_revenue_pool` — Total revenue deposited for a campaign.
- `get_revenue_claimed` — How much revenue a contributor has already claimed.
- `get_personal_cap` — A contributor's personal contribution cap for a campaign.
- `get_campaign_reserve` — Vesting reserve details for a campaign.

**Voting queries**
- `get_approve_votes` / `get_reject_votes` — Vote tallies for a campaign.
- `has_voted` — Whether an address has cast a vote.
- `get_min_votes_quorum` / `get_approval_threshold_bps` / `get_min_voting_balance` — Current voting parameters.

**Admin / config queries**
- `get_admin` / `get_pending_admin` — Current and pending admin addresses.
- `get_token` — Platform token address.
- `get_platform_fee` — Current platform fee in basis points.
- `get_min_campaign_funding_goal` / `get_max_campaign_funding_goal` — Effective funding goal bounds.
- `is_paused` / `is_creation_disabled` — Contract operation flags.
- `has_pending_campaign_transfer` — Whether a campaign has an in-flight ownership transfer.
- `get_version` — Stored contract version number.

## Documentation

- Authorization requirements for every public method: `docs/AUTHORIZATION.md`
- Campaign lifecycle state machine: `docs/CAMPAIGN_LIFECYCLE.md`
- Contribution cap semantics: `docs/CONTRIBUTION_CAP_POLICY.md`
- Storage TTL behavior: `docs/STORAGE_TTL_POLICY.md`
- Threat model and security considerations: `docs/THREAT_MODEL.md`

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Stellar CLI](https://developers.stellar.org/docs/tools/stellar-cli) — install with `cargo install --locked stellar-cli --features opt` (previously named `soroban-cli`)
- `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

### Build

```bash
# Clone the repository
git clone https://github.com/Iris-IV/ProofOfHeart-stellar.git
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
├── Cargo.toml                        # Project manifest & dependencies
└── src/
    ├── lib.rs                        # Contract entry-points and top-level dispatch
    ├── admin.rs                      # Admin functions: pause, fees, voting params, migration
    ├── contributions.rs              # Contribute, claim_refund, personal cap
    ├── errors.rs                     # Contract error enum (Error)
    ├── lifecycle.rs                  # Shared guards: require_not_paused, assert_admin, etc.
    ├── queries.rs                    # Paginated listing and platform stats helpers
    ├── revenue.rs                    # Revenue deposit and claim logic
    ├── storage.rs                    # Storage helpers and DataKey definitions
    ├── types.rs                      # Shared types: Campaign, Category, CreateCampaignParams, etc.
    ├── voting.rs                     # Community voting logic and helpers
    ├── campaigns/
    │   ├── cancel.rs                 # cancel_campaign
    │   ├── create.rs                 # create_campaign
    │   ├── transfer.rs               # Campaign ownership transfer (two-step)
    │   ├── update.rs                 # update_campaign, update_campaign_description, extend_deadline
    │   └── withdraw.rs               # withdraw_funds, withdraw_reserve, set_vesting_params
    └── tests/                        # Integration, unit, and property-based tests
        ├── helpers.rs
        ├── test_admin.rs
        ├── test_benchmark.rs
        ├── test_campaign_create.rs
        ├── test_campaign_update.rs
        ├── test_cancel_revenue_orphan.rs
        ├── test_contribute.rs
        ├── test_contribute_caps.rs
        ├── test_create_campaign_proptest.rs
        ├── test_creator_buckets.rs
        ├── test_deadline_ext.rs
        ├── test_duration_cap.rs
        ├── test_fee_override.rs
        ├── test_init.rs
        ├── test_issues.rs
        ├── test_lifecycle_events.rs
        ├── test_listing.rs
        ├── test_purge_voting.rs
        ├── test_refund.rs
        ├── test_refund_edge.rs
        ├── test_revenue.rs
        ├── test_revenue_deposit.rs
        ├── test_revenue_share_proptest.rs
        ├── test_storage_ttl.rs
        ├── test_vesting.rs
        ├── test_voting.rs
        ├── test_voting_proptest.rs
        ├── test_voting_verify.rs
        └── test_withdraw.rs
```

## Related Repositories

| Repository | Description |
| --- | --- |
| [ProofOfHeart-frontend](https://github.com/Iris-IV/ProofOfHeart-frontend) | Next.js frontend application |

## Contributing

We welcome contributors of all experience levels! For detailed setup instructions, coding standards, and PR guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
