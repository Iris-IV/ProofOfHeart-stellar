# Contributing to ProofOfHeart

This guide gets you from zero to contributing code.

## Prerequisites

Install these before cloning:

| Tool | Install |
|------|---------|
| Rust | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Stellar CLI | `cargo install --locked stellar-cli --features opt` |
| wasm32 target | `rustup target add wasm32-unknown-unknown` |

> **Note:** The CLI was previously named `soroban-cli` (binary: `soroban`). It has been rebranded to `stellar-cli` (binary: `stellar`). All commands in this repo use `stellar`.

Verify:

```bash
rustc --version
cargo --version
stellar --version
```

## Clone & Setup

1. Fork the repo on GitHub.
2. Clone your fork:

```bash
git clone https://github.com/<your-username>/ProofOfHeart-stellar.git
cd ProofOfHeart-stellar
```

3. (Optional) Track upstream for syncing:

```bash
git remote add upstream https://github.com/Iris-IV/ProofOfHeart-stellar.git
```

## Build & Test

> **Heads up:** The first `cargo build` downloads and compiles all Rust dependencies. This can take **10–20 minutes** and use **1–2 GB** of disk space. Subsequent builds are much faster.

```bash
# Build WASM
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test --features testutils
```

The repo includes a `rust-toolchain.toml` that pins the Rust toolchain automatically — `rustup` will download the correct version on first use.

## Code Style

CI runs these checks on every PR. Run locally before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets --features testutils -- -D warnings
cargo test --features testutils
cargo build --target wasm32-unknown-unknown --release
```

All four must pass.

## Branches

Branch off `main`. Use a type prefix:

| Prefix | Use for |
|--------|---------|
| `docs/` | Documentation |
| `feat/` | New features |
| `fix/` | Bug fixes |
| `chore/` | Tooling, deps |
| `test/` | Tests only |

Examples: `docs/add-contributing-md`, `feat/campaign-ownership-transfer`, `fix/reentrancy-guard`

Delete your branch after merge.

## Commits

Conventional Commits format:

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `test`, `chore`, `refactor`, `security`

Examples:
```
docs: add CONTRIBUTING.md
fix: reentrancy guard on withdraw_funds
feat: campaign ownership transfer
test: deadline boundary coverage
```

## Pull Requests

1. Reference the issue: `Closes #28`
2. Fill out the PR template (auto-applied from `.github/PULL_REQUEST_TEMPLATE.md`)
3. Ensure CI is green — all four checks in the Code Style section must pass
4. One issue per PR

## Changelog

Every PR that changes behaviour (bug fix, feature, refactor, security) **must** add a bullet under the `[Unreleased]` section of `CHANGELOG.md` before merging.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/):

```markdown
## [Unreleased]

### Fixed
- Short description of the fix (#issue-number).

### Added
- Short description of the new feature (#issue-number).

### Changed / Refactored
- Short description of the change (#issue-number).
```

Pure documentation or tooling PRs that do not affect contract behaviour may skip the changelog entry.

## Issue Labels

| Label | What it means |
|-------|---------------|
| `good first issue` | Beginner-friendly, good for first PR |
| `bug` | Something is broken |
| `enhancement` | New functionality request |
| `documentation` | Docs changes |
| `security` | Security vulnerability or hardening |
| `testing` | Test coverage or quality |
| `infrastructure` | CI/CD, tooling, repo setup |
| `Stellar Wave` | Part of the Stellar Wave program |

## Milestones

| Milestone | Focus |
|-----------|-------|
| MVP Hardening | Core security, bug fixes |
| Testing & QA | Test coverage |
| DevOps & Infrastructure | CI/CD, tooling |
| Documentation | Docs, guides |

View the full board at the [Issues page](../../milestones).

## Getting Help

- [Stellar CLI Docs](https://developers.stellar.org/docs/tools/stellar-cli)
- [Soroban Docs](https://soroban.stellar.org/docs)
- [Stellar Developers](https://developers.stellar.org/)
- [Issues](../../issues) — search before opening new ones

By contributing, your work falls under the MIT License.
