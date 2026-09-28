# Fuzz Testing Strategy and Honggfuzz Setup

## Overview

ProofOfHeart uses libfuzzer-based property testing to validate contract behavior under adversarial input. Fuzz targets exercise critical contract functions with arbitrary, mutated input to catch edge cases that hand-written tests miss.

## Fuzz Targets

The contract exposes four primary fuzz targets:

### 1. `fuzz_contribute` - Contribution Path Testing

**Function:** Validates the `contribute()` entry point under arbitrary contributions.

**Property:** `contribute` must never panic or trap the host, regardless of amount or campaign state. Every invocation must resolve to either `Ok(())` or a typed `Err(Error)`.

**Coverage:**
- Extreme `i128` amounts (positive, negative, overflow edges)
- Campaigns at various lifecycle stages (created, verified, cancelled, withdrawn)
- Contributions exceeding funding goals, caps, or available balance
- Rapid successive contributions from the same account
- Multi-campaign batch contributions

**Input Structure:**
```rust
struct FuzzInput {
    campaign_id: u64,
    amount: i128,
    campaign_params: CreateCampaignParams,
    contributor: Address,
    action: ContributeAction,
}

enum ContributeAction {
    Create,
    Verify,
    Contribute,
    Refund,
}
```

**Running:**
```bash
# Build the fuzz target
cd fuzz
cargo fuzz fuzz_contribute

# Limit fuzzing time (default is infinite)
cargo fuzz fuzz_contribute -- -max_total_time=60

# Limit input length
cargo fuzz fuzz_contribute -- -max_len=512

# Generate minimized crash input
cargo fuzz fuzz_contribute -- crash-input.bin
```

### 2. `fuzz_voting` - Voting Path Testing

**Function:** Validates the `vote_on_campaign()` entry point under arbitrary voting patterns.

**Property:** Voting operations must maintain ballot integrity and not corrupt campaign state, even under malicious voting sequences.

**Coverage:**
- Double-voting from the same account
- Voting on non-existent campaigns
- Voting before campaign verification
- Voting after campaign cancellation
- Voting with zero platform token balance
- Rapid successive votes in sequence

**Input Structure:**
```rust
struct FuzzInput {
    campaign_id: u64,
    voter: Address,
    is_approval: bool,
    voting_sequence: Vec<VotingAction>,
}

enum VotingAction {
    Create,
    Verify,
    Vote,
    Check,
}
```

**Running:**
```bash
# Build and run with default settings
cargo fuzz fuzz_voting

# Use a custom corpus directory
cargo fuzz fuzz_voting -- -artifact_prefix=/tmp/voting_crashes/
```

### 3. `fuzz_lifecycle` - Campaign Lifecycle Testing

**Function:** Validates full campaign lifecycle (create → verify → contribute → vote → withdraw/refund/cancel).

**Property:** Campaign state transitions must be atomic and consistent. Campaigns must never enter invalid states, regardless of invocation order or timing.

**Coverage:**
- Out-of-order operations (e.g., withdraw before contributions)
- Verify-after-cancel operations
- Multiple verify attempts
- Contributions exceeding deadline
- Refunds before cancellation
- Revenue share calculations at various scales

**Input Structure:**
```rust
struct FuzzInput {
    campaign_params: CreateCampaignParams,
    operations: Vec<CampaignOperation>,
}

enum CampaignOperation {
    Create,
    Verify,
    Contribute(i128),
    Vote(bool),
    Withdraw,
    Cancel,
    ClaimRefund,
}
```

**Running:**
```bash
# Enable verbose logging
RUST_LOG=debug cargo fuzz fuzz_lifecycle

# Save crash reproductions
cargo fuzz fuzz_lifecycle -- -artifact_prefix=./crashes/lifecycle/
```

### 4. `fuzz_revenue` - Revenue Sharing Testing

**Function:** Validates revenue sharing calculations and distribution logic.

**Property:** Revenue shares must be calculated correctly and distributed atomically without loss or duplication.

**Coverage:**
- Various revenue share percentages (0%, 25%, 50%, 75%, 100%)
- Contributions at different scales
- Revenue shares across multiple contributors
- Creator withdrawals with active revenue share
- Refunds with pending revenue shares

**Input Structure:**
```rust
struct FuzzInput {
    revenue_share_percentage: u32,
    contributions: Vec<(i128, Address)>,
    withdrawal_timing: WithdrawalTiming,
}

enum WithdrawalTiming {
    Immediate,
    AfterContributions,
    AfterDeadline,
}
```

**Running:**
```bash
# Build the revenue fuzzer
cargo fuzz fuzz_revenue

# Run with corpus directory
cargo fuzz fuzz_revenue -- -artifact_prefix=./corpus/revenue/
```

## Project Structure

```
fuzz/
├── Cargo.toml              # Cargo configuration for libfuzzer
├── fuzz_targets/
│   ├── fuzz_contribute.rs  # Contribution path testing
│   ├── fuzz_voting.rs      # Voting path testing
│   ├── fuzz_lifecycle.rs   # Lifecycle testing
│   └── fuzz_revenue.rs     # Revenue sharing testing
└── corpus/                 # Seed inputs (optional)
    ├── contribute/
    ├── voting/
    ├── lifecycle/
    └── revenue/
```

## Cargo Configuration

The fuzz harness is configured in `fuzz/Cargo.toml`:

```toml
[package]
name = "proof-of-heart-fuzz"
version = "0.0.0"

[dependencies]
libfuzzer-sys = "0.4"
arbitrary = { version = "1", features = ["derive"] }

[dependencies.proof-of-heart]
path = ".."
features = ["testutils"]

[[bin]]
name = "fuzz_contribute"
path = "fuzz_targets/fuzz_contribute.rs"
test = false
doc = false
bench = false
```

### Key Settings

- **libfuzzer-sys 0.4:** Rust wrapper for libfuzzer, providing the `fuzz_target!` macro
- **arbitrary 1.x:** Derives arbitrary input generation for custom types
- **features = ["testutils"]:** Enables `soroban_sdk::testutils` for mock auth and contract registration

## Running the Fuzzer

### Quick Start

```bash
# Install libfuzzer (if not already installed)
cargo install cargo-fuzz

# Enter the fuzz directory
cd fuzz

# Run a single target for 30 seconds
cargo fuzz run fuzz_contribute -- -max_total_time=30

# Run all targets
cargo fuzz run fuzz_*
```

### Advanced Options

#### Time and Input Limits

```bash
# Run for 60 seconds
cargo fuzz run fuzz_contribute -- -max_total_time=60

# Limit maximum input size to 1KB
cargo fuzz run fuzz_contribute -- -max_len=1024

# Combine limits
cargo fuzz run fuzz_contribute -- -max_total_time=60 -max_len=512
```

#### Corpus Management

```bash
# Use a custom corpus directory
cargo fuzz run fuzz_contribute -- -artifact_prefix=/tmp/corpus/

# Minimize corpus (reduce test cases to smallest reproductions)
cargo fuzz cmin fuzz_contribute -- /tmp/corpus/

# Merge corpus inputs
cargo fuzz merge fuzz_contribute corpus/contrib_old corpus/contrib_new
```

#### Crash Investigation

```bash
# Reproduce a specific crash
cargo fuzz run fuzz_contribute -- crash-input.bin

# Debug a crash with logging
RUST_LOG=debug cargo fuzz run fuzz_contribute -- crash-input.bin

# Generate minimized repro
cargo fuzz tmin fuzz_contribute crash-input.bin
```

#### Performance Tuning

```bash
# Increase workers (parallel fuzzing)
cargo fuzz run fuzz_contribute -- -workers=4

# Limit memory use
cargo fuzz run fuzz_contribute -- -rss_limit_mb=4096

# Focus on interesting test cases
cargo fuzz run fuzz_contribute -- -use_value_profile=1
```

## Corpus Generation

### Seeding with Hand-Written Tests

Create a corpus directory with seed inputs derived from successful test cases:

```bash
mkdir -p fuzz/corpus/contribute
mkdir -p fuzz/corpus/voting
mkdir -p fuzz/corpus/lifecycle
mkdir -p fuzz/corpus/revenue
```

Seed inputs should be binary files containing valid campaign parameters and operation sequences. The fuzzer will mutate these seeds to explore the input space.

### Expanding the Corpus

As the fuzzer discovers new interesting inputs, they are automatically saved to the corpus directory. Review and commit successful corpus entries to enable faster fuzzing in CI/CD:

```bash
# After a successful fuzzing run
git add fuzz/corpus/
git commit -m "fuzz: expand corpus with discovered edge cases"
```

## Integration with CI/CD

### GitHub Actions Setup

Add fuzzing to your CI workflow:

```yaml
name: Fuzz Testing

on:
  push:
    branches: [main, dev]
  pull_request:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-fuzz
      - name: Run fuzz tests
        run: |
          cd fuzz
          cargo fuzz run fuzz_contribute -- -max_total_time=30
          cargo fuzz run fuzz_voting -- -max_total_time=30
          cargo fuzz run fuzz_lifecycle -- -max_total_time=30
          cargo fuzz run fuzz_revenue -- -max_total_time=30
```

### Continuous Fuzzing

For long-running fuzzing campaigns, consider using Google's OSS-Fuzz:

1. Create a `projects/proof-of-heart` directory in the OSS-Fuzz repository
2. Configure Dockerfile and build scripts
3. OSS-Fuzz runs fuzzing continuously and reports crashes to the maintainers

See: https://google.github.io/oss-fuzz/

## Property-Based Testing with Proptest

In addition to fuzz testing, the contract uses property-based testing with `proptest` for hand-written property tests:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_contribution_never_overflows(amount in 0i128..i128::MAX) {
            let env = create_test_env();
            let client = setup_contract(&env);
            
            // This property should always hold
            let result = client.try_contribute(&campaign_id, &contributor, &amount);
            assert!(result.is_ok() || result.is_err());
            // Never panics
        }
    }
}
```

## Debugging Crashes

### Stack Trace Analysis

When the fuzzer finds a crash, it saves the input to `crash-input.bin`:

```bash
# Reproduce with full backtrace
RUST_BACKTRACE=1 cargo fuzz run fuzz_contribute crash-input.bin

# Run under a debugger
rust-gdb --args ./target/x86_64-unknown-linux-gnu/release/fuzz_contribute crash-input.bin
```

### Input Minimization

The `cargo fuzz tmin` command creates a minimal input that reproduces the crash:

```bash
# Minimize crash input
cargo fuzz tmin fuzz_contribute crash-input.bin

# Output: `minimized-from-crash.bin` with smallest reproducer
```

### Logging and Tracing

Enable debug logging in fuzz targets:

```rust
#[fuzz_target]
pub fn go(data: &[u8]) {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let input = Arbitrary::arbitrary(&mut Unstructured::new(data)).unwrap_or_default();
    run_fuzz_logic(input);
}
```

## Benchmarking Fuzz Performance

### Measuring Coverage

Use `llvm-cov` to measure code coverage during fuzzing:

```bash
# Install coverage tools
cargo install cargo-llvm-cov

# Run with coverage
cargo llvm-cov --html -- fuzz

# Open HTML report
open target/llvm-cov/html/index.html
```

### Throughput Analysis

Monitor the fuzzer's throughput (iterations per second):

```bash
# Run with stats output
cargo fuzz run fuzz_contribute -- -print_coverage=1 2>&1 | grep "cov:"
```

## Best Practices

1. **Start Simple:** Begin with basic fuzz targets covering single functions before testing complex workflows.
2. **Seed Wisely:** Use corpus seeds derived from existing unit tests to accelerate convergence.
3. **Iterate Fast:** Run fuzzing for short intervals (30-60s) during development, longer (hours) before releases.
4. **Minimize Crashes:** Always minimize crash inputs to the simplest reproduction case.
5. **Automate:** Integrate fuzzing into CI/CD to catch regressions early.
6. **Archive Corpus:** Commit corpus to version control to maintain reproducibility.
7. **Review Crashes:** Analyze each crash to understand root causes and add unit test coverage.

## Common Issues

### "Timeout in fuzzer"

**Cause:** Infinite loop or stack overflow in contract code

**Fix:** Add timeout limits and check for unbounded loops:
```bash
cargo fuzz run fuzz_contribute -- -timeout=5  # 5-second timeout per run
```

### "Out of memory"

**Cause:** Unbounded memory allocation in fuzzer or contract

**Fix:** Reduce RSS limit and input length:
```bash
cargo fuzz run fuzz_contribute -- -rss_limit_mb=2048 -max_len=256
```

### "Flaky crashes"

**Cause:** Non-deterministic contract behavior (randomness, timestamps)

**Fix:** Ensure fuzz inputs are fully determined and all randomness is seeded:
```rust
env.ledger().with_mut(|li| {
    li.timestamp = input.timestamp;  // Seed from input
    li.sequence_number = input.sequence;
});
```

## References

- [libfuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [cargo-fuzz Guide](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [arbitrary Crate](https://docs.rs/arbitrary/)
- [Soroban Testing Guide](https://developers.stellar.org/docs/build/guides/testing/)
- [OSS-Fuzz](https://google.github.io/oss-fuzz/)
- [Property-Based Testing with Proptest](https://docs.rs/proptest/)

## Running Fuzzing Locally

### Complete Workflow

```bash
# Clone and setup
git clone git@github.com:samolusey/ProofOfHeart-stellar.git
cd ProofOfHeart-stellar
cargo install cargo-fuzz

# Run all fuzz targets for 2 minutes each
cd fuzz
for target in contribute voting lifecycle revenue; do
    echo "Fuzzing $target..."
    cargo fuzz run fuzz_$target -- -max_total_time=120
done

# Review crashes
ls -la artifacts/*/crash-*

# Minimize and investigate
cargo fuzz tmin fuzz_contribute artifacts/fuzz_contribute/crash-*

# Commit improvements
cd ..
git add fuzz/corpus/
git commit -m "fuzz: expand corpus after successful fuzzing run"
```

### Long-Running Campaign

For comprehensive coverage before a release:

```bash
# Run for 8 hours (28,800 seconds) with verbose output
cd fuzz
cargo fuzz run fuzz_contribute -- -max_total_time=28800 -print_stats=1 | tee fuzz_contribute.log
cargo fuzz run fuzz_voting -- -max_total_time=28800 -print_stats=1 | tee fuzz_voting.log
cargo fuzz run fuzz_lifecycle -- -max_total_time=28800 -print_stats=1 | tee fuzz_lifecycle.log
cargo fuzz run fuzz_revenue -- -max_total_time=28800 -print_stats=1 | tee fuzz_revenue.log

# Archive results
mkdir -p ../fuzz_results/$(date +%Y%m%d)
cp *.log ../fuzz_results/$(date +%Y%m%d)/
cp -r artifacts/* ../fuzz_results/$(date +%Y%m%d)/
```
