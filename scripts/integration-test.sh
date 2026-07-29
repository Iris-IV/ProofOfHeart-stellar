#!/usr/bin/env bash
#
# Integration test against a real Stellar network (local quickstart container).
#
# Starts a local Stellar network, deploys the ProofOfHeart contract WASM,
# exercises the full lifecycle (init → create_campaign → verify → contribute
# → withdraw → revenue deposit → revenue claim), and asserts state at each
# step. Tears down the container on exit.
#
# Usage:
#   scripts/integration-test.sh
#
# Prerequisites:
#   - Docker (for stellar/quickstart)
#   - stellar CLI (stellar-cli)
#   - cargo (with wasm32-unknown-unknown target)
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WASM_PATH="${REPO_ROOT}/target/wasm32-unknown-unknown/release/proof_of_heart.wasm"

# ── colours ────────────────────────────────────────────────────────────────────
PASS="$(printf '\033[32m✔\033[0m')"
FAIL="$(printf '\033[31m✘\033[0m')"
INFO="$(printf '\033[34m→\033[0m')"

pass() { printf '  %s %s\n' "$PASS" "$*"; }
fail() { printf '  %s %s\n' "$FAIL" "$*"; }
info() { printf '%s %s\n' "$INFO" "$*"; }
die()  { fail "$*"; exit 1; }

# ── helpers ────────────────────────────────────────────────────────────────────
CONTAINER_NAME="proof-of-heart-integration"
NETWORK_NAME="local-integration"
RPC_URL="http://localhost:8000/soroban/rpc"

cleanup() {
  info "Cleaning up…"
  docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
  stellar network rm "$NETWORK_NAME" 2>/dev/null || true
  stellar keys rm "$NETWORK_NAME" 2>/dev/null || true
}
trap cleanup EXIT

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is not installed or not on PATH."
}

# ── 0. Prerequisites ──────────────────────────────────────────────────────────
info "Checking prerequisites…"
require_cmd docker
require_cmd stellar
require_cmd cargo

# ── 1. Start local Stellar network ────────────────────────────────────────────
info "Starting local Stellar network (stellar/quickstart:testing)…"
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
docker run -d --rm \
  --name "$CONTAINER_NAME" \
  -p 8000:8000 \
  stellar/quickstart:testing \
  --testnet \
  --enable-soroban

info "Waiting for the RPC endpoint to become available…"
for i in $(seq 1 60); do
  if curl -s -o /dev/null -w '%{http_code}' "$RPC_URL" 2>/dev/null | grep -q 200; then
    pass "RPC endpoint ready"
    break
  fi
  if [ "$i" -eq 60 ]; then
    die "RPC endpoint did not become ready within 60 seconds"
  fi
  sleep 2
done

# Give the network a moment to stabilise
sleep 5

# ── 2. Configure Stellar CLI for local network ────────────────────────────────
info "Configuring Stellar CLI for local network…"
stellar network add \
  --rpc-url "$RPC_URL" \
  --network-passphrase "Test SDF Network ; September 2015" \
  "$NETWORK_NAME"

# Generate a deployer keypair scoped to this network
stellar keys generate --network "$NETWORK_NAME" "$NETWORK_NAME" 2>/dev/null || true

# Fund the deployer via friendbot (available on quickstart)
info "Funding deployer account…"
stellar keys fund "$NETWORK_NAME" --network "$NETWORK_NAME" 2>/dev/null || true

# ── 3. Build WASM ─────────────────────────────────────────────────────────────
info "Building contract WASM…"
cargo build --target wasm32-unknown-unknown --release
[ -f "$WASM_PATH" ] || die "WASM not found at ${WASM_PATH}"
pass "WASM built: $(wc -c < "$WASM_PATH") bytes"

# ── 4. Deploy contract ────────────────────────────────────────────────────────
info "Deploying contract…"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME")"
pass "Contract deployed: ${CONTRACT_ID}"

# Verify deployment info
stellar contract info --id "$CONTRACT_ID" --network "$NETWORK_NAME" > /dev/null
pass "Contract info accessible"

# ── 5. Create a test token ────────────────────────────────────────────────────
info "Creating test token (Stellar Asset Contract)…"
TOKEN_ID="$(stellar contract asset deploy \
  --asset TEST \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME")"
pass "Token deployed: ${TOKEN_ID}"

# ── 6. Initialize the contract ────────────────────────────────────────────────
info "Initializing contract…"
DEPLOYER_ADDR="$(stellar keys address "$NETWORK_NAME")"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  init \
  --admin "$DEPLOYER_ADDR" \
  --token "$TOKEN_ID" \
  --platform_fee 300
pass "Contract initialized"

# Verify initialization
VERSION="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_version)"
[ "$VERSION" = "1" ] || die "Expected version 1, got ${VERSION}"
pass "get_version = 1"

# ── 7. Create a campaign ──────────────────────────────────────────────────────
info "Creating campaign…"
CAMPAIGN_ID="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  create_campaign \
  --creator "$DEPLOYER_ADDR" \
  --title "Integration Test Campaign" \
  --description "Created by integration-test.sh" \
  --funding_goal 10000000000 \
  --duration_days 30 \
  --category Learner \
  --has_revenue_sharing true \
  --revenue_share_percentage 1000 \
  --max_contribution_per_user 0)"
pass "Campaign created: ${CAMPAIGN_ID}"

# Verify campaign exists and is active
CAMPAIGN_JSON="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_campaign \
  --campaign_id "$CAMPAIGN_ID")"
echo "$CAMPAIGN_JSON" | grep -q '"is_active": true' || die "Campaign should be active"
pass "Campaign is active"

# ── 8. Vote on campaign & verify ──────────────────────────────────────────────
info "Voting on campaign…"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  vote_on_campaign \
  --campaign_id "$CAMPAIGN_ID" \
  --voter "$DEPLOYER_ADDR" \
  --approve true
pass "Vote cast"

info "Admin-verifying campaign…"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  verify_campaign \
  --campaign_id "$CAMPAIGN_ID"
pass "Campaign verified"

CAMPAIGN_JSON="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_campaign \
  --campaign_id "$CAMPAIGN_ID")"
echo "$CAMPAIGN_JSON" | grep -q '"is_verified": true' || die "Campaign should be verified"
pass "Campaign is_verified = true"

# ── 9. Contribute ─────────────────────────────────────────────────────────────
info "Contributing to campaign…"
# Mint tokens to deployer so they can contribute
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  mint \
  --to "$DEPLOYER_ADDR" \
  --amount 5000000000

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  contribute \
  --campaign_id "$CAMPAIGN_ID" \
  --contributor "$DEPLOYER_ADDR" \
  --amount 5000000000
pass "Contribution of 5000000000 made"

# Verify contribution
CONTRIBUTION="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_contribution \
  --campaign_id "$CAMPAIGN_ID" \
  --contributor "$DEPLOYER_ADDR")"
[ "$CONTRIBUTION" = "5000000000" ] || die "Expected contribution 5000000000, got ${CONTRIBUTION}"
pass "Contribution recorded correctly"

# Verify campaign shows amount raised
CAMPAIGN_JSON="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_campaign \
  --campaign_id "$CAMPAIGN_ID")"
echo "$CAMPAIGN_JSON" | grep -q '"amount_raised"' || die "Campaign should show amount_raised"
pass "Campaign amount_raised present"

# ── 10. Withdraw funds ────────────────────────────────────────────────────────
info "Withdrawing funds…"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  withdraw_funds \
  --campaign_id "$CAMPAIGN_ID"
pass "Funds withdrawn"

CAMPAIGN_JSON="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_campaign \
  --campaign_id "$CAMPAIGN_ID")"
echo "$CAMPAIGN_JSON" | grep -q '"funds_withdrawn": true' || die "Campaign should show funds_withdrawn"
echo "$CAMPAIGN_JSON" | grep -q '"is_active": false' || die "Campaign should be inactive"
pass "Campaign state correct after withdrawal"

# ── 11. Deposit revenue ───────────────────────────────────────────────────────
info "Depositing revenue…"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  deposit_revenue \
  --campaign_id "$CAMPAIGN_ID" \
  --amount 2000000000
pass "Revenue deposited"

REVENUE_POOL="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_revenue_pool \
  --campaign_id "$CAMPAIGN_ID")"
[ "$REVENUE_POOL" = "2000000000" ] || die "Expected revenue pool 2000000000, got ${REVENUE_POOL}"
pass "Revenue pool = 2000000000"

# ── 12. Claim revenue ─────────────────────────────────────────────────────────
info "Claiming revenue…"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  -- \
  claim_revenue \
  --campaign_id "$CAMPAIGN_ID" \
  --contributor "$DEPLOYER_ADDR"
pass "Revenue claimed"

REVENUE_CLAIMED="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_revenue_claimed \
  --campaign_id "$CAMPAIGN_ID" \
  --contributor "$DEPLOYER_ADDR")"
[ "$REVENUE_CLAIMED" != "0" ] || die "Revenue should have been claimed"
pass "Revenue claimed recorded"

# ── 13. Query contract version (compile-time built-in) ────────────────────────
info "Verifying compile-time contract_version…"
CONTRACT_VER="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  contract_version)"
[ "$CONTRACT_VER" = "1" ] || die "Expected contract_version 1, got ${CONTRACT_VER}"
pass "contract_version = 1"

# ── 14. Verify platform stats ─────────────────────────────────────────────────
info "Verifying platform stats…"
STATS="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$NETWORK_NAME" \
  --network "$NETWORK_NAME" \
  --is-read-only \
  -- \
  get_platform_stats)"
echo "$STATS" | grep -q '"total_campaigns"' || die "Platform stats missing total_campaigns"
pass "Platform stats accessible"

# ── All done ───────────────────────────────────────────────────────────────────
echo ""
printf '\033[32m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m\n'
printf '\033[32m  All integration tests passed!\033[0m\n'
printf '\033[32m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m\n'
