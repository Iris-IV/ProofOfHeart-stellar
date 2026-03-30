# Contributing to ProofOfHeart

This guide gets you from zero to contributing code.

## Prerequisites

Install these before cloning:

| Tool | Install |
|------|---------|
| Rust | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Soroban CLI | `cargo install soroban-cli` |
| wasm32 target | `rustup target add wasm32-unknown-unknown` |

Verify:

```bash
rustc --version
cargo --version
soroban --version
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

```bash
# Build WASM
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test --features testutils
```

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

- [Soroban Docs](https://soroban.stellar.org/docs)
- [Stellar Developers](https://developers.stellar.org/)
- [Issues](../../issues) — search before opening new ones

By contributing, your work falls under the MIT License.
