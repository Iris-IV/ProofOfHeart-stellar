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
# Build WASM (Local development)
stellar contract build

# Build reproducible WASM (Required for production deployment)
make build-docker

# Run tests
cargo test --features testutils
```

The repo includes a `rust-toolchain.toml` that pins the Rust toolchain automatically — `rustup` will download the correct version on first use.

## Fuzz Testing Workflow

ProofOfHeart uses property-based and fuzz testing via `cargo-fuzz` (libFuzzer) to stress-test arithmetic, state transitions, and edge cases.

### Running Fuzz Tests

1. Install `cargo-fuzz` and nightly toolchain:
   ```bash
   cargo install cargo-fuzz
   rustup toolchain install nightly
   ```

2. List available fuzz targets:
   ```bash
   cargo +nightly fuzz list
   ```

3. Run a specific fuzz target:
   ```bash
   cargo +nightly fuzz run fuzz_campaign_accounting
   ```

4. Reproduce a crash artifact:
   ```bash
   cargo +nightly fuzz run fuzz_campaign_accounting fuzz/artifacts/fuzz_campaign_accounting/crash-xyz
   ```

## Code Style

CI runs these checks on every PR. Run locally before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets --features testutils -- -D warnings
cargo test --features testutils
stellar contract build
cargo audit --ignore RUSTSEC-2024-0344 --ignore RUSTSEC-2026-0009
```

All five must pass. `cargo audit` (install with `cargo install cargo-audit`) scans the dependency tree for known security advisories and fails if any apply to a locked version. The two `--ignore` flags are required and mirror the CI gate (see `.github/workflows/ci.yml`); both advisories are documented there:

- `RUSTSEC-2024-0344` (`curve25519-dalek < 4.1.3`): `soroban-env-host 20.1.0` pins this crate to exactly `4.1.1`, which is in turn fixed by the pinned `soroban-sdk =20.1.0`. The patched version is not installable without upgrading the SDK.
- `RUSTSEC-2026-0009` (`time < 0.3.47`): `time` only enters the graph as an optional dependency of `serde_with` behind its disabled `time_0_3` feature, so the vulnerable code is never compiled.

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
3. Update `EVENT_PAYLOADS.md` if your PR adds, modifies, or removes any `env.events().publish(...)` call — keep topics, data shape, and source location in sync
4. Ensure CI is green — all four checks in the Code Style section must pass
5. One issue per PR

## Changelog

Every PR that changes behaviour (bug fix, feature, refactor, security) **must** add a bullet under the `[Unreleased]` section of `CHANGELOG.md` before merging.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), with a few project-specific headings added for categories that come up often in this repo:

```markdown
## [Unreleased]

### Added
- Short description of the new feature (#issue-number).

### Fixed
- Short description of the fix (#issue-number).

### Changed / Refactored
- Short description of the change (#issue-number).

### Removed
- Short description of what was removed and why (#issue-number).

### Security
- Short description of the hardening or vulnerability fix (#issue-number).

### Infrastructure
- Short description of the CI/CD, build, or tooling change (#issue-number).

### Documentation
- Short description of the docs change (#issue-number).
```

Conventions:

- **Where:** add your bullet to the matching category under `[Unreleased]`. Create the category heading if it doesn't exist yet for this release.
- **When:** new entries go at the top of their category's list, newest first.
- **What:** one line, written from the user/integrator's point of view (what changed and why it matters), ending with the issue number in parentheses. Match the tone of existing entries in the file.
- **Skip it for:** pure documentation or tooling PRs that do not affect contract behaviour (an entry under `### Documentation` or `### Infrastructure` is still welcome but not required).

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
