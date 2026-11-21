# Multi-Token Integration Technical Blueprint

## Overview

ProofOfHeart supports campaigns denominated in multiple SEP-41 compliant tokens. Each campaign fixes its token at creation time, ensuring all value movements (contributions, refunds, withdrawals, revenue sharing) use the same currency. The platform token (XLM) remains the default and the only currency allowed for the `create_campaign` entry point.

## Architecture

### Token Allowlist

The admin controls which tokens can be used for campaigns via the `set_token_allowed` function. The platform token is implicitly allowed and requires no explicit entry.

```rust
// Allowlist a token
client.set_token_allowed(&token_address, &true);

// Remove from allowlist
client.set_token_allowed(&token_address, &false);

// Check if token is allowed
let is_allowed = client.is_token_allowed(&token_address);
```

**Key Invariants:**
- Only the admin can modify the allowlist
- The platform token is always allowed
- Removing a token from the allowlist does not affect existing campaigns
- An unallowlisted token cannot be used to create a new campaign

### Campaign Token Binding

When a campaign is created with a specific token, that currency is fixed for the campaign's lifetime. The contract queries the campaign's token before any value transfer.

```rust
// Create campaign with default platform token
let id = client.create_campaign(&params);

// Create campaign with a custom token
let id = client.create_campaign_with_token(&params, &usdc_address);

// Query a campaign's token
let token = client.get_campaign_token(&campaign_id);
```

**Storage Optimization:**
- Default campaigns (using platform token) store no currency key, saving persistent storage rent
- Custom-token campaigns store only the token address, not the entire contract reference
- Unknown campaign IDs default to the platform token to prevent query failures

## Value Transfer Patterns

### Contributions

Contributions are pulled in the campaign's designated token. The platform token remains untouched regardless of the campaign's currency.

```rust
// Contribution amount is denominated in the campaign's token
client.contribute(&campaign_id, &contributor, &amount);
```

**Token Transfer:**
```
contributor -> contract (campaign's token)
```

**Storage Changes:**
- `contribution` key incremented in the campaign's currency bucket
- `campaign.amount_raised` incremented
- No impact on platform token balance

### Batch Contributions

Batch operations group contributions by currency, executing a single transfer per token type. This minimizes host budget consumption when multiple campaigns share a currency.

```rust
let batch = soroban_sdk::Vec::from_array(
    &env,
    [
        (native_campaign_id, 1000i128),    // pulled in platform token
        (usdc_campaign_id, 2000i128),      // pulled in USDC
        (euroc_campaign_id, 500i128),      // pulled in EUROC
    ],
);
client.batch_contribute(&contributor, &batch);
```

**Execution:**
1. Group contributions by campaign token
2. Execute one `transfer` call per token type
3. Update campaign and contributor state atomically

### Refunds

Refunds are paid back in the exact currency the contribution arrived in. The contract reads the campaign's token before initiating any transfer.

```rust
client.claim_refund(&campaign_id, &contributor);
```

**Token Transfer:**
```
contract -> contributor (campaign's token)
```

**Cancellation Prerequisite:**
- Campaign must be cancelled via `cancel_campaign(&campaign_id)`
- Refund calculations exclude fees paid from platform token

### Withdrawals

Creator withdrawals are denominated in the campaign's currency. If revenue sharing is enabled, the creator claims their share in the campaign's token.

```rust
client.withdraw_funds(&campaign_id);
```

**Token Transfer:**
```
contract -> creator (campaign's token)
```

**Multi-Currency Scenarios:**
- Revenue shares are calculated and paid in the campaign's token
- Milestone payouts (if implemented) are denominated in the campaign's token
- Platform fees are paid separately from the campaign's currency

## Soroban SDK Token Operations

### Token Client Interface

The contract uses the Soroban SDK's `token::Client` for all token interactions:

```rust
use soroban_sdk::token;

let token_client = token::Client::new(&env, &token_address);

// Query operations
let balance = token_client.balance(&address);
let allowance = token_client.allowance(&from, &spender);

// Mutation operations
token_client.transfer(&from, &to, &amount);
token_client.approve(&from, &spender, &amount, &expiration_ledger);
token_client.transfer_from(&spender, &from, &to, &amount);
```

### Admin Operations

For token minting and burning in tests, use the `StellarAssetClient`:

```rust
use soroban_sdk::token::StellarAssetClient;

let token_admin = StellarAssetClient::new(&env, &token_address);
token_admin.mint(&to, &amount);
token_admin.burn(&from, &amount);
token_admin.clawback(&from, &amount);
```

## Storage Layout

### Campaign Token Storage

```rust
// Only stored if campaign uses a custom token (not platform default)
// Key: ("campaign_token", campaign_id)
// Value: Address of the token contract
```

### Token Allowlist Storage

```rust
// Key: ("token_allowed", token_address)
// Value: bool (true if allowlisted)
// Implicitly true for platform token
```

### Contribution Accounting

```rust
// Key: ("contribution", campaign_id, contributor_address)
// Value: i128 (amount in campaign's token)
```

## Testing Token Interactions

### Unit Test Setup

```rust
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

fn setup_token(env: &Env) -> (Address, TokenClient, StellarAssetClient) {
    let token_admin = Address::generate(env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());
    
    (
        token_id.clone(),
        TokenClient::new(env, &token_id),
        StellarAssetClient::new(env, &token_id),
    )
}

#[test]
fn test_multi_token_contribution() {
    let env = Env::default();
    env.set_default_info();
    env.ledger().with_mut(|li| li.protocol_version = 23);
    env.mock_all_auths();

    let (usdc, usdc_client, usdc_admin) = setup_token(&env);
    let contributor = Address::generate(&env);
    
    // Mint tokens
    usdc_admin.mint(&contributor, &10_000);
    
    // Create campaign and contribute
    let campaign_id = /* create_campaign_with_token */ 1;
    usdc_client.transfer(&contributor, &contract, &5_000);
    
    assert_eq!(usdc_client.balance(&contract), 5_000);
}
```

## Error Handling

### Token Validation Errors

```rust
#[contracterror]
pub enum Error {
    ValidationFailed = 1,           // Token not in allowlist
    InsufficientBalance = 2,        // Contributor lacks funds
    InsufficientAllowance = 3,      // Allowance too low for transfer
    TransferFailed = 4,             // Token contract revert
}
```

### Common Failure Scenarios

1. **Unallowlisted Token:** Attempting to create a campaign with a non-allowlisted token returns `ValidationFailed`
2. **Insufficient Balance:** If a contributor lacks the requested amount in the campaign's token, `transfer` fails with `InsufficientBalance`
3. **Insufficient Allowance:** If the contract lacks an approved allowance for the token, `transfer_from` fails with `InsufficientAllowance`
4. **Token Contract Failure:** If the token contract reverts, the transfer propagates as `TransferFailed`

## Voting and Platform Token

**Critical Invariant:** Voting weight is denominated in the platform token, regardless of the campaign's currency.

This prevents a creator from denominating a campaign in an obscure token and handing voting rights to whoever holds it.

```rust
// Voting requires platform token balance
pub fn vote_on_campaign(
    env: Env,
    campaign_id: u64,
    voter: Address,
    is_approval: bool,
) -> Result<(), Error> {
    voter.require_auth();
    
    // Check platform token balance (not campaign token)
    let platform_token = env.storage().instance().get::<_, Address>(&platform_key())?;
    let platform_client = token::Client::new(&env, &platform_token);
    
    if platform_client.balance(&voter) == 0 {
        return Err(Error::NotTokenHolder);
    }
    
    // Proceed with vote
    Ok(())
}
```

## Deployment Considerations

### Protocol Version

Multi-token support requires Soroban SDK 23.4.1 and protocol version 23. Verify the contract is deployed with compatible versions:

```bash
stellar contract build

stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/contract.wasm \
  --source ACCOUNT \
  --network testnet
```

### Token Address Format

Soroban tokens are accessed via their contract address, which can be obtained from:

1. **Stellar Asset Contract (SAC):** Deploy an asset and wrap it as a Soroban contract
2. **Custom Token:** Deploy a custom SEP-41 token contract
3. **Bridge Tokens:** Wrapped tokens from other chains via bridge contracts

### Testing Multi-Token Contracts on Testnet

```bash
# Register a test token (Stellar Asset Contract)
stellar contract asset deploy \
  --asset "TEST:GBUQWP3BOUZX34LOCALTOKEN2K6QQM4KUEL33B4PG3WMNNRNX4F5LHCA5"
  --source ACCOUNT \
  --network testnet

# Get the token contract ID
stellar contract info \
  --id TEST_CONTRACT_ID \
  --network testnet

# Mint tokens for testing
stellar contract invoke \
  --id TEST_CONTRACT_ID \
  --source ACCOUNT \
  --network testnet \
  -- \
  mint \
  --to RECIPIENT_ADDRESS \
  --amount 10000000000
```

## Example: Multi-Currency Campaign Workflow

```rust
#[test]
fn test_multi_currency_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);

    let (usdc_id, usdc_client, usdc_admin) = setup_token(&env);
    let (euroc_id, euroc_client, euroc_admin) = setup_token(&env);

    // Setup: Mint tokens
    usdc_admin.mint(&contributor, &10_000);
    euroc_admin.mint(&contributor, &5_000);

    let client = ProofOfHeartClient::new(&env, &contract_id);

    // Allowlist tokens
    client.set_token_allowed(&usdc_id, &true);
    client.set_token_allowed(&euroc_id, &true);

    // Create campaigns in different currencies
    let usdc_campaign = client.create_campaign_with_token(
        &make_params(creator.clone(), "USDC Campaign"),
        &usdc_id,
    );
    let euroc_campaign = client.create_campaign_with_token(
        &make_params(creator.clone(), "EUROC Campaign"),
        &euroc_id,
    );

    // Verify campaigns
    client.verify_campaign(&usdc_campaign);
    client.verify_campaign(&euroc_campaign);

    // Batch contribute across currencies
    let batch = soroban_sdk::Vec::from_array(
        &env,
        [(usdc_campaign, 3_000i128), (euroc_campaign, 2_000i128)],
    );
    client.batch_contribute(&contributor, &batch);

    // Verify balances
    assert_eq!(usdc_client.balance(&contract_id), 3_000);
    assert_eq!(euroc_client.balance(&contract_id), 2_000);

    // Creator withdrawal
    client.withdraw_funds(&usdc_campaign);
    assert!(usdc_client.balance(&creator) > 0);
}
```

## Performance and Gas Optimization

### Batch Operations

Batch contributions minimize host budget consumption by grouping transfers by currency:

```
Single Currency:
  - 1 transfer call
  - O(1) host budget cost

Multiple Currencies (N distinct):
  - N transfer calls
  - O(N) host budget cost
  - Still more efficient than N separate requests
```

### Storage Rent Optimization

Default-token campaigns avoid persistent storage rent by not writing a currency key:

```
Platform Token Campaign: 1 campaign key
Custom Token Campaign:   2 keys (campaign + token)
```

### TTL Extension

Token balances and allowances are stored in temporary storage with automatic cleanup. Persistent campaign state requires manual TTL extension:

```rust
env.storage().persistent().extend_ttl(&campaign_key, 100_000, 200_000);
```

## Security Considerations

### Token Validation

All token contracts are validated against the allowlist before use. This prevents:
- Arbitrary contract calls disguised as token transfers
- Flash loan attacks via malicious token contracts
- Unreviewed code execution during transfers

### Authorization

All token-modifying operations require proper authorization:
- Contributions require contributor signature (`transfer_from`)
- Refunds/withdrawals execute as contract calls (implicit allowance)
- Admin operations (allowlist changes) require admin signature

### Reentrancy Protection

Soroban's linear type system prevents reentrancy attacks. Token callbacks cannot recursively invoke contract functions during execution.

## Troubleshooting

### "ValidationFailed" on Create Campaign

**Cause:** Token not in allowlist or admin has not called `set_token_allowed`

**Fix:** Verify the token is allowlisted:
```rust
assert!(client.is_token_allowed(&token_address));
```

### "InsufficientBalance" on Contribution

**Cause:** Contributor lacks funds in the campaign's token

**Fix:** Mint or transfer tokens:
```rust
token_admin.mint(&contributor, &amount);
```

### "TransferFailed" During Withdrawal

**Cause:** Contract lacks allowance or token contract reverted

**Fix:** Verify allowance and contract state:
```rust
let allowance = token_client.allowance(&contract, &creator);
```

## References

- [Soroban SDK API Reference](https://docs.rs/soroban-sdk/latest/soroban_sdk/)
- [Stellar Asset Contract Guide](https://developers.stellar.org/docs/build/guides/tokens/stellar-asset-contract)
- [Token Interface (SEP-41)](https://developers.stellar.org/docs/build/guides/tokens/stellar-asset-contract)
- [Soroban Storage Guide](https://developers.stellar.org/docs/build/guides/storage/choosing-the-right-storage)
