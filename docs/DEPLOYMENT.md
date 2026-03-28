# ProofOfHeart Soroban Deployment Guide

This guide covers deploying the ProofOfHeart smart contract on Stellar testnet and mainnet, from building the WASM to contract initialization and verification.

## Table of Contents

1. [Prerequisites & Setup](#prerequisites--setup)
2. [Build the Contract](#build-the-contract)
3. [Testnet Deployment](#testnet-deployment)
4. [Mainnet Deployment](#mainnet-deployment)
5. [Contract Initialization](#contract-initialization)
6. [Token Setup](#token-setup)
7. [Verification & Testing](#verification--testing)
8. [Troubleshooting](#troubleshooting)

---

## Prerequisites & Setup

### 1. Install Rust

If you don't have Rust installed, follow the [official installation guide](https://www.rust-lang.org/tools/install):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify installation:
```bash
rustc --version
cargo --version
```

### 2. Add WASM Target

Add the WebAssembly target for Soroban contracts:

```bash
rustup target add wasm32-unknown-unknown
```

### 3. Install Soroban CLI

Install the Soroban command-line tool:

```bash
cargo install soroban-cli
```

Verify installation:
```bash
soroban --version
```

### 4. Clone and Build the Repository

```bash
git clone https://github.com/Iris-IV/ProofOfHeart-stellar.git
cd ProofOfHeart-stellar
```

---

## Build the Contract

Build the WASM binary for deployment:

```bash
cargo build --target wasm32-unknown-unknown --release
```

**Output:** `target/wasm32-unknown-unknown/release/proof_of_heart.wasm`

Verify the WASM file was created:
```bash
ls -lh target/wasm32-unknown-unknown/release/proof_of_heart.wasm
```

Expected size: ~500 KB (WASM files are compressed).

---

## Testnet Deployment

### Step 1: Generate or Import a Keypair

Generate a new keypair for the deployer account:

```bash
soroban keys generate --global deployer
```

**Output:** A keypair is generated and stored in `~/.soroban/keys/deployer.json`

(Optional) If you already have a secret key, import it:

```bash
soroban keys generate --global deployer --secret-key <YOUR_SECRET_KEY>
```

### Step 2: Fund Your Testnet Account

Request testnet lumens (XLM) to pay for deployment and initialization:

```bash
soroban keys fund deployer --network testnet
```

This command uses the official Stellar testnet friendbot to fund your account with 10,000 XLM.

**Verify funding:**
```bash
soroban account balance --source deployer --network testnet
```

Expected balance: ~10,000 XLM (minus any spent on previous deployments).

### Step 3: Deploy the Contract to Testnet

Deploy the compiled WASM binary:

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/proof_of_heart.wasm \
  --source deployer \
  --network testnet
```

**Output:** The command returns the contract ID (a long string starting with `C...`).

**Save the contract ID** for the next steps:

```bash
export CONTRACT_ID="<CONTRACT_ID_FROM_DEPLOY>"
```

**Example:**
```bash
export CONTRACT_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"
```

### Step 4: Initialize the Contract (see [Contract Initialization](#contract-initialization))

---

## Mainnet Deployment

### What You Need to Know

1. **Real Money** — Mainnet transactions cost actual lumens (XLM). Even small operations cost fractions of XLM, but they add up.
2. **No Undo** — Mainnet deployments are permanent. Test thoroughly on testnet first.
3. **Key Security** — Your private key must be kept secure. Never commit it to version control or share it.
4. **Network Confirmation** — Mainnet transactions take 3–5 seconds to confirm.

### Step 1: Prepare a Mainnet Key (with Existing Funds)

If you don't have a mainnet account, create one:

```bash
soroban keys generate --global deployer-mainnet
```

Fund your mainnet account using an exchange or other means:
- Minimum: 2–3 XLM (deployment: ~0.5 XLM, initialization: ~0.05 XLM, buffer for fees)

**Verify mainnet balance:**
```bash
soroban account balance --source deployer-mainnet --network mainnet
```

### Step 2: Deploy the Contract to Mainnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/proof_of_heart.wasm \
  --source deployer-mainnet \
  --network mainnet
```

**Save the contract ID:**
```bash
export CONTRACT_ID_MAINNET="<CONTRACT_ID_FROM_DEPLOY>"
```

### Step 3: Initialize the Contract (see [Contract Initialization](#contract-initialization))

### Cost Summary

| Operation | Estimated Cost |
| --- | --- |
| Deploy contract | ~0.5 XLM |
| Initialize contract | ~0.05 XLM |
| **Total** | **~0.55 XLM** |

*Costs may vary with network congestion.*

---

## Contract Initialization

The contract must be initialized before use. This sets the admin, token address, and platform fee.

### Parameters Explained

| Parameter | Type | Example | Description |
| --- | --- | --- | --- |
| `admin` | Address | `GBRPGWUSZSTZ...` | Account that can govern the contract (usually the deployer) |
| `token` | Address | `CBQHD3V2OMK2...` | The token contract address used for contributions (usually a wrapped asset or native Stellar asset) |
| `platform_fee` | u32 | `300` | Fee in basis points (1/100th of a percent). `300` = 3%, max 10% (1000) |

**Fee Calculation Example:**
- If a campaign raises 1,000 tokens and the fee is 300 (3%):
  - Platform receives: 30 tokens
  - Creator receives: 970 tokens

### Testnet Initialization

If you haven't set up a token yet, see [Token Setup](#token-setup) first.

Once you have a token address, initialize the contract:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- \
  init \
  --admin <ADMIN_ADDRESS> \
  --token <TOKEN_ADDRESS> \
  --platform_fee 300
```

**Example:**
```bash
soroban contract invoke \
  --id "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4" \
  --source deployer \
  --network testnet \
  -- \
  init \
  --admin "GBRPGWUSZSTZZSWHQ5P2CGQYVJQE2DYOD4DZJVOYUSB42O3AGXC5VYN" \
  --token "CBQHD3V2OMK2HLAQYAOIMJG753VQULQYSIU3IM27YPUYVISFSFSOSDE" \
  --platform_fee 300
```

### Mainnet Initialization

Identical to testnet, but use `--network mainnet` and the mainnet `CONTRACT_ID`:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID_MAINNET" \
  --source deployer-mainnet \
  --network mainnet \
  -- \
  init \
  --admin <ADMIN_ADDRESS> \
  --token <TOKEN_ADDRESS> \
  --platform_fee 300
```

---

## Token Setup

ProofOfHeart uses a token contract for contributions. You have two options:

### Option A: Use an Existing Wrapped Asset (Simplest for Testing)

Stellar provides wrapped assets for common tokens. For testnet, use the USDC wrapped asset:

**Testnet USDC contract address:**
```
CBQHD3V2OMK2HLAQYAOIMJG753VQULQYSIU3IM27YPUYVISFSFSOSDE
```

Use this address directly in contract initialization.

### Option B: Deploy Your Own Token Contract (Advanced)

If you want full control over the token:

1. Create a token contract (e.g., using the Soroban token template):
   ```bash
   soroban contract init token
   cd token
   cargo build --target wasm32-unknown-unknown --release
   ```

2. Deploy it:
   ```bash
   soroban contract deploy \
     --wasm token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm \
     --source deployer \
     --network testnet
   ```

3. Save the returned contract ID and use it as `--token` in ProofOfHeart initialization.

### Minting Tokens (Testnet Only)

If you control the token, mint some tokens to test contributions:

```bash
soroban contract invoke \
  --id "$TOKEN_CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- \
  mint \
  --to <ACCOUNT_ADDRESS> \
  --amount 10000000000  # 1,000 with 9 decimals
```

---

## Verification & Testing

### 1. Verify Contract Deployment

Check that the contract was deployed:

```bash
soroban contract info --id "$CONTRACT_ID" --network testnet
```

Expected output includes contract ID, WASM hash, and deployment info.

### 2. Verify Initialization

Call the `get_version()` function to verify the contract was initialized:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source deployer \
  --network testnet \
  --is-read-only \
  -- \
  get_version
```

Expected output: `1` (the contract version).

### 3. Test Campaign Creation

Create a test campaign to verify everything works:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- \
  create_campaign \
  --creator "$CREATOR_ADDRESS" \
  --title "Test Campaign" \
  --description "A test fundraising campaign" \
  --funding_goal 10000000000 \
  --duration_days 30 \
  --category "Learner" \
  --has_revenue_sharing false \
  --revenue_share_percentage 0
```

Expected output: Campaign ID `1` (if it's the first campaign).

### 4. Verify Account Setup

Check that your account has enough XLM to pay for operations:

```bash
soroban account balance --source deployer --network testnet
```

---

## Troubleshooting

### Error: "Source account does not exist"

**Cause:** Your account wasn't funded.

**Solution:**
```bash
soroban keys fund deployer --network testnet
```

### Error: "Invalid contract ID"

**Cause:** The contract ID format is wrong or the contract doesn't exist on that network.

**Solution:**
- Verify the contract ID starts with `C` and is 56 characters long.
- Check you're using the correct network (`--network testnet` or `--network mainnet`).
- Redeploy if necessary.

### Error: "Insufficient balance"

**Cause:** Your account doesn't have enough XLM.

**Solution:**
- Testnet: Run `soroban keys fund deployer --network testnet` again.
- Mainnet: Transfer XLM from an exchange to your account.

### Error: "Invalid source key"

**Cause:** The deployer key doesn't exist or is named incorrectly.

**Solution:**
```bash
soroban keys list
soroban keys generate --global deployer  # Recreate if needed
```

### Contract Invocation Hangs

**Cause:** Network connectivity issue or the network is overloaded.

**Solution:**
- Check your internet connection.
- Try again in a few moments.
- Check the [Stellar status page](https://status.stellar.org/).

### WASM File Not Found

**Cause:** The contract wasn't built yet.

**Solution:**
```bash
cargo build --target wasm32-unknown-unknown --release
```

---

## Next Steps

1. **Testnet Integration** — Use the testnet contract ID with the [ProofOfHeart frontend](https://github.com/Iris-IV/ProofOfHeart-frontend) for end-to-end testing.
2. **Mainnet Beta** — Once thoroughly tested, deploy to mainnet for real users.
3. **Monitoring** — Set up alerts/dashboards to monitor contract health and transaction volumes.
4. **Contract Upgrades** — If bugs are found, the contract can be upgraded using the version tracking mechanism (see [src/lib.rs](../src/lib.rs)).

---

## Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Stellar Testnet](https://developers.stellar.org/docs/fundamentals-and-concepts/testnet-public-network)
- [Soroban CLI Reference](https://github.com/stellar/rs-soroban-cli)
- [Stellar Account Federation](https://developers.stellar.org/docs/learn/smart-contracts/stellar-asset-contract)

---

**Last Updated:** March 28, 2026
