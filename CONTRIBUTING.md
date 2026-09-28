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

This repository includes property-based fuzz testing using `cargo-fuzz` targeting contract contribution entrypoints (`fuzz/fuzz_targets/fuzz_contribute.rs`).

### Running Fuzz Targets Locally

1. Install `cargo-fuzz` and use nightly Rust:
   ```bash
   cargo install cargo-fuzz
   rustup toolchain install nightly
   ```

2. Execute fuzz target:
   ```bash
   cargo +nightly fuzz run fuzz_contribute
   ```

3. Analyzing crashes or corpus output:
   - Discovered artifacts and crash inputs are saved under `fuzz/artifacts/fuzz_contribute/`.
   - Re-test specific crash input:
     ```bash
     cargo +nightly fuzz run fuzz_contribute fuzz/artifacts/fuzz_contribute/crash-<hash>
     ```

## Code Style

CI runs these checks on every PR. Run locally before pushing:

```bash
cargo fmt --check
cargo clippy --all-targets --features testutils -- -D warnings
cargo test --features testutils
stellar contract build
cargo audit --ignore RUSTSEC-2026-0009 --ignore RUSTSEC-2025-0001 --ignore RUSTSEC-2025-0056 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097
```

All five must pass. `cargo audit` (install with `cargo install cargo-audit`) scans the dependency tree for known security advisories and fails if any apply to a locked version. The `--ignore` flags are required and mirror the CI gate (see `.github/workflows/ci.yml`); the remaining advisories are documented there:

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

## Architecture & Key Modules

### Core Contract Modules

| Module | Purpose |
|--------|---------|
| `lib.rs` | Contract entry points and public interface |
| `types.rs` | Core data structures (Campaign, Contribution, etc.) |
| `lifecycle.rs` | Campaign state transitions and validation |
| `storage.rs` | Persistent storage operations with TTL management |
| `contributions.rs` | Contribution tracking and goal-checking logic |
| `revenue.rs` | Revenue sharing calculations and distribution |
| `voting.rs` | Verification voting mechanism |
| `admin.rs` | Admin-only operations (pause, cancel, etc.) |
| `errors.rs` | Custom error types |

### Critical Design Patterns

#### Check-Effect-Interact (CEI)
State updates precede external calls to prevent re-entrancy:

```rust
// ✓ Correct: update state first
set_revenue_claimed(env, campaign_id, &contributor, new_amount);
client.transfer(&env.current_contract_address(), &contributor, &claimable);

// ✗ Wrong: transfer first opens re-entrancy window
client.transfer(&env.current_contract_address(), &contributor, &claimable);
set_revenue_claimed(env, campaign_id, &contributor, new_amount);
```

#### Boundary Condition Testing
All deadline and numeric operations must test exact boundaries:

```rust
#[test]
fn test_contribution_at_exact_deadline_is_accepted() {
    // Test timestamp == deadline (inclusive)
    env.ledger().with_mut(|l| l.timestamp = deadline);
    client.contribute(&id, &contributor, &amount); // Must succeed

    // Test timestamp > deadline (exclusive)
    env.ledger().with_mut(|l| l.timestamp = deadline + 1);
    let res = client.try_contribute(&id, &contributor, &amount); // Must fail
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}
```

#### Division Precision
Multiplication before division avoids premature truncation:

```rust
// ✓ Correct: multiply before divide
let result = contribution
    .checked_mul(total_pool)
    .and_then(|n| n.checked_mul(percentage as i128))
    .and_then(|n| n.checked_div(denominator as i128))?;

// ✗ Wrong: intermediate truncation loses precision
let result = (contribution / denominator) * total_pool * percentage;
```

### Storage Strategy

ProofOfHeart uses Soroban's three storage types strategically:

- **Instance Storage**: Contract configuration, admin address, pause state (long TTL, extends with contract)
- **Persistent Storage**: Campaign data, contributions, revenue pools (manual TTL extension required)
- **Temporary Storage**: Session data, temporary caches (auto-cleanup, no TTL management needed)

All persistent entries are subject to archival if TTL expires. The contract includes `bump_instance_ttl()` calls before writes to extend entry lifetimes.

### Revenue Sharing Model

Revenue is split using basis points (BPS, where 10,000 = 100%):

```text
Contributor Share (%) = revenue_share_percentage / 100
Creator Share (%) = (10_000 - revenue_share_percentage) / 100

Per-contributor allocation = (their_contribution / total_raised) * pool * contributor_bps / 10_000
Creator allocation = total_pool * creator_bps / 10_000
```

Key invariant: Contributors cannot claim until `funds_withdrawn = true` to prevent race conditions with the growing `amount_raised` denominator.

## Testing Best Practices

### Test Structure

```rust
#[test]
fn test_descriptive_scenario_name() {
    let (env, admin, creator, contributor1, contributor2, token, token_admin, client) = setup_env();
    
    // Setup: prepare state
    token_admin.mint(&contributor1, &1000);
    let campaign_id = client.create_campaign(&params);
    
    // Act: perform the operation
    client.contribute(&campaign_id, &contributor1, &500);
    
    // Assert: verify outcomes and side effects
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 500);
    
    // Verify events
    let events = env.events().all();
    assert!(events.len() > 0);
}
```

### Coverage Requirements

Every function must cover:
1. **Happy path**: Normal operation with valid inputs
2. **Error cases**: Each error variant the function returns
3. **Boundary conditions**: Numeric limits, exact thresholds (e.g., `timestamp == deadline`)
4. **State transitions**: Verify state changes are correct and complete

### Testing Ledger Time

Use ledger timestamp control to test time-based logic:

```rust
let campaign = client.get_campaign(&id);
let deadline = campaign.deadline;

// Test at deadline (inclusive)
env.ledger().with_mut(|l| l.timestamp = deadline);
assert!(operation_succeeds);

// Test past deadline (exclusive)
env.ledger().with_mut(|l| l.timestamp = deadline + 1);
assert!(operation_fails);
```

### Mocking and Assertions

- Use `env.mock_all_auths()` to skip authentication checks in tests
- Use `env.events().all()` to capture and assert event emissions
- Use `try_*` methods to capture error results for assertion

## Pull Request Checklist

Before opening a PR, verify:

- [ ] All tests pass locally: `cargo test --features testutils`
- [ ] Code is formatted: `cargo fmt`
- [ ] Clippy passes: `cargo clippy --all-targets --features testutils -- -D warnings`
- [ ] Contract builds: `stellar contract build`
- [ ] Security audit passes: `cargo audit --ignore RUSTSEC-2026-0009 --ignore RUSTSEC-2025-0001 --ignore RUSTSEC-2025-0056 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0097`
- [ ] New tests cover edge cases and error paths
- [ ] Documentation and comments explain non-obvious logic
- [ ] `EVENT_PAYLOADS.md` is updated if events were added/modified/removed
- [ ] `CHANGELOG.md` has an entry in `[Unreleased]` if behavior changed
- [ ] No issue has multiple PRs — one issue per PR

## Debugging Failed Tests

If a test fails:

1. Run with output: `cargo test test_name --test test_file -- --nocapture`
2. Check ledger state: Print campaign/storage values before assertions
3. Verify time assumptions: Use `env.ledger()` to inspect timestamps
4. Inspect events: Print `env.events().all()` to see what was emitted
5. Check error context: Use `try_*` methods to see exact error codes

Example debug session:

```bash
# Run one test with output
cargo test test_contribution_at_exact_deadline_is_accepted -- --nocapture

# Show what's happening
cargo test test_admin_cancel_campaign_succeeds_after_goal_met -- --nocapture 2>&1 | head -50
```

## Getting Help

- [Stellar CLI Docs](https://developers.stellar.org/docs/tools/stellar-cli)
- [Soroban Docs](https://soroban.stellar.org/docs)
- [Stellar Developers](https://developers.stellar.org/)
- [Issues](../../issues) — search before opening new ones
- [Soroban Examples](https://github.com/stellar/soroban-examples) — reference implementations

By contributing, your work falls under the MIT License.
