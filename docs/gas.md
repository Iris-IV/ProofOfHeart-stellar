# Gas Optimization Guidelines & Resource Budget Best Practices

This document provides guidance on minimizing CPU instructions and memory footprint in the Proof of Heart Soroban smart contract, following Stellar blockchain best practices.

## Table of Contents

1. [Overview](#overview)
2. [Soroban Resource Model](#soroban-resource-model)
3. [Key Optimization Strategies](#key-optimization-strategies)
4. [Storage Efficiency](#storage-efficiency)
5. [Computational Optimization](#computational-optimization)
6. [Testing & Benchmarking](#testing--benchmarking)
7. [Common Pitfalls](#common-pitfalls)

## Overview

Soroban smart contracts run on the Stellar network and consume network resources measured in CPU instructions and memory. Optimizing contract code reduces transaction fees and improves throughput.

The Proof of Heart contract prioritizes:
- **Minimal CPU usage** per contribution and withdrawal
- **Efficient storage operations** with proper TTL management
- **Predictable resource consumption** across campaign lifecycles

## Soroban Resource Model

### CPU Instructions

Every Soroban operation consumes CPU instructions. The budget includes:
- **Signature verification**: ~5M instructions
- **Token transfers**: ~100K-500K instructions per operation
- **Storage reads/writes**: ~1K-10K instructions per operation

Current Proof of Heart target budgets:
- `contribute()`: < 5M CPU instructions
- `withdraw_funds()`: < 5M CPU instructions
- `claim_revenue()`: < 5M CPU instructions
- `claim_refund()`: < 5M CPU instructions

### Memory Footprint

Memory includes ledger entries for:
- Contract instance data
- Campaign state
- Contribution records
- Revenue pools
- Persistent storage

## Key Optimization Strategies

### 1. Minimize Storage Access

**Problem**: Each storage read/write consumes CPU and costs network fees.

**Solutions**:

```rust
// BAD: Multiple reads of the same value
let campaign = client.get_campaign(&id);
if campaign.is_active { /* ... */ }
let goal = client.get_campaign(&id).funding_goal; // Second read!

// GOOD: Cache in local variable
let campaign = client.get_campaign(&id);
if campaign.is_active {
    let goal = campaign.funding_goal; // Reuse cached value
}
```

**Best Practice**: Fetch once, use multiple times within a function scope.

### 2. Use Efficient Storage Keys

Storage keys determine how data is organized on-chain. Smaller, composite keys reduce memory overhead.

**Example from Proof of Heart**:

```rust
// Composite key: more efficient than string concatenation
let key = (
    Symbol::new(&env, "contribution"),
    campaign_id,      // u32
    contributor.clone() // Address
);
```

**Avoid**: String concatenation for keys (not supported in Soroban SDK).

### 3. Batch Operations When Possible

**Example**: `batch_contribute()` reduces overhead per contribution:

```rust
// Single token transfer vs. multiple transfers
let total = contributions.iter().sum();
token.transfer(&contributor, &contract, &total);
// Then update campaign state once
```

This saves:
- N-1 token transfer operations
- Reduced storage update frequency

### 4. Defer Heavy Computations

Move expensive calculations outside the critical path when safe.

**Example**: Revenue calculations can be deferred:

```rust
// Avoid computing revenue share on every claim
// Store pre-computed share or calculate during claim_revenue
// with explicit budget assumptions
```

### 5. Leverage Instance Storage for Constants

Instance storage is cheaper than persistent storage for contract configuration.

```rust
// GOOD: Store in instance (cheaper, contract-wide)
env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);

// LESS EFFICIENT: Persistent storage per campaign
env.storage().persistent()
    .set(&(Symbol::new(&env, "campaign_admin"), campaign_id), &admin);
```

## Storage Efficiency

### TTL (Time To Live) Management

Ledger entries require explicit TTL extension. Failing to extend TTL causes archival.

**Proof of Heart TTL Strategy**:

1. **Persistent Storage** (campaigns, contributions):
   - Extended when campaign is active
   - Minimal threshold: 16 ledgers (~80 seconds)
   - Extension target: 200,000 ledgers (~23 days)

   ```rust
   env.storage().persistent().extend_ttl(
       &key,
       100_000,  // threshold before auto-extension
       200_000   // extension target
   );
   ```

2. **Instance Storage** (contract state):
   - Extended with contract WASM
   - Lives as long as the contract code

### Storage Cleanup

Removing unused keys frees ledger space and reduces on-chain bloat.

**Best Practice**: Clear refunded contributions immediately:

```rust
// After refund, remove the contribution entry
env.storage().persistent().remove(&contribution_key);
```

## Computational Optimization

### 1. Batch Processing

**Before**: Process items one-by-one, paying gas per operation.

```rust
// Inefficient: N separate storage updates
for contribution in contributions {
    update_contribution(&env, campaign_id, &user, contribution);
}
```

**After**: Aggregate before storing:

```rust
// Efficient: Single aggregated update
let total: i128 = contributions.iter().sum();
update_contribution(&env, campaign_id, &user, total);
```

### 2. Early Exit Checks

Validate conditions early to skip expensive operations.

```rust
// GOOD: Fail fast on auth check
user.require_auth();

// Then continue with expensive operations
let campaign = env.storage().persistent().get(&key)?;
```

### 3. Checked Arithmetic

Use `checked_*` operations to prevent panics (which waste gas).

```rust
// Catches overflow before transfer
let new_total = current.checked_add(contribution)
    .ok_or(Error::Overflow)?;
```

### 4. Avoid Unnecessary Clones

Cloning complex types (Vec, Map) is expensive.

```rust
// GOOD: Pass by reference when possible
fn validate_campaign(campaign: &Campaign) { /* ... */ }

// Avoid: Cloning for no reason
fn process_campaign(campaign: Campaign) { /* ... */ }
```

## Testing & Benchmarking

### CPU Budget Tests

Proof of Heart includes budget regression tests in `src/tests/test_benchmark.rs`:

```rust
#[test]
fn test_contribute_instruction_budget() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);

    env.budget().reset_default();
    client.contribute(&id, &contributor1, &500);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < CONTRIBUTE_CPU_LIMIT,
        "contribute() used {} CPU instructions, limit is {}",
        cpu,
        CONTRIBUTE_CPU_LIMIT
    );
}
```

**Run budget tests**:

```bash
cargo test test_contribute_instruction_budget --lib
```

### Local Performance Testing

Use the Soroban SDK's budget APIs to inspect resource consumption:

```rust
env.budget().reset_default();
// Execute operation
let cpu = env.budget().cpu_instruction_cost();
let mem = env.budget().memory_byte_cost();
eprintln!("CPU: {}, Memory: {}", cpu, mem);
```

## Common Pitfalls

### 1. Forgetting to Extend TTL

**Problem**: Storage entries archived when TTL expires.

```rust
// WRONG: No TTL extension
env.storage().persistent().set(&key, &value);

// CORRECT: Always extend TTL for long-lived data
env.storage().persistent().set(&key, &value);
env.storage().persistent().extend_ttl(&key, 100_000, 200_000);
```

### 2. Reading Same Data Multiple Times

**Problem**: Redundant storage access costs gas.

```rust
// INEFFICIENT
let campaign = get_campaign(&env, id)?;
// ... later in function
let campaign = get_campaign(&env, id)?; // Redundant!

// EFFICIENT
let campaign = get_campaign(&env, id)?;
// ... reuse campaign variable
```

### 3. Not Batching Token Operations

**Problem**: Multiple transfers = multiple gas costs.

```rust
// INEFFICIENT: 3 transfers
token.transfer(&user1, &contract, &100);
token.transfer(&user2, &contract, &200);
token.transfer(&user3, &contract, &300);

// EFFICIENT: 1 transfer (if contract logic allows)
token.transfer(&aggregated_source, &contract, &600);
```

### 4. Unbounded Loops

**Problem**: Cannot predict gas cost; may exceed limits.

```rust
// RISKY: Unbounded loop
while let Some(item) = get_next_item() {
    process(item); // May exhaust budget
}

// SAFER: Bounded loop with explicit limit
for item in items.iter().take(MAX_ITEMS) {
    process(item);
}
```

### 5. Large Data Structures

**Problem**: Cloning or serializing large Vecs/Maps is expensive.

```rust
// INEFFICIENT: Collect all then process
let all_campaigns: Vec<Campaign> = get_all_campaigns(&env);
process_campaigns(&all_campaigns);

// EFFICIENT: Process in batches or iteratively
process_campaigns_in_pages(&env, page_size);
```

## Monitoring & Future Improvements

### Current Metrics (Protocol 23)

Target CPU budgets per operation:
- `contribute()`: 5M instructions
- `withdraw_funds()`: 5M instructions
- `claim_revenue()`: 5M instructions
- `get_campaigns_by_category()`: 10M instructions

### Future Optimization Opportunities

1. **Implement pagination** for bulk queries to reduce per-call overhead
2. **Cache frequently-accessed campaign metadata** in contract instance storage
3. **Optimize storage key encoding** for shorter serialization
4. **Profile anomaly detection** to reduce per-contribution overhead

## References

- [Soroban SDK Storage Guides](https://developers.stellar.org/docs/build/guides/storage)
- [Stellar Protocol 23 Documentation](https://developers.stellar.org/docs/learn/smart-contracts)
- [Soroban Testing & Benchmarking](https://developers.stellar.org/docs/build/guides/testing)
