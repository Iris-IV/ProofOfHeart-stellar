#!/usr/bin/env bash
#
# Automates the manual deployment steps documented in docs/DEPLOYMENT.md
# (#495): build, deploy, init, and verify the ProofOfHeart Soroban contract
# on testnet or mainnet.
#
# Usage:
#   scripts/deploy.sh <command> [options]
#
# Commands:
#   deploy-testnet    Build (if needed) and deploy the contract to testnet.
#   deploy-mainnet    Build (if needed) and deploy the contract to mainnet.
#   init              Initialize a deployed contract (admin/token/fee).
#   verify            Verify a deployed contract is live and initialized.
#
# Environment variables:
#   SOURCE_ACCOUNT   Stellar CLI identity to sign with (default: deployer).
#   ADMIN_ADDRESS    Admin address to pass to `init`.
#   TOKEN_ADDRESS    Token contract address to pass to `init`.
#   PLATFORM_FEE     Platform fee in basis points for `init` (default: 300).
#   CONTRACT_ID      Contract ID to target for `init`/`verify`.
#
# Examples:
#   scripts/deploy.sh deploy-testnet
#   SOURCE_ACCOUNT=deployer-mainnet scripts/deploy.sh deploy-mainnet
#   CONTRACT_ID=C... ADMIN_ADDRESS=G... TOKEN_ADDRESS=C... scripts/deploy.sh init --network testnet
#   CONTRACT_ID=C... scripts/deploy.sh verify --network testnet

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WASM_PATH="${REPO_ROOT}/target/wasm32-unknown-unknown/release/proof_of_heart.wasm"

DEFAULT_PLATFORM_FEE=300

log() {
  printf '==> %s\n' "$1"
}

fail() {
  printf 'Error: %s\n' "$1" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "'$1' is not installed or not on PATH."
}

build_contract() {
  require_cmd cargo
  log "Building contract WASM (cargo build --target wasm32-unknown-unknown --release)"
  (cd "${REPO_ROOT}" && cargo build --target wasm32-unknown-unknown --release)

  [ -f "${WASM_PATH}" ] || fail "Build finished but WASM not found at ${WASM_PATH}"
  log "WASM built: ${WASM_PATH}"
}

deploy_to_network() {
  local network="$1"
  local source_account="${SOURCE_ACCOUNT:-deployer}"

  require_cmd stellar
  build_contract

  log "Deploying to ${network} using source account '${source_account}'"
  local contract_id
  contract_id="$(stellar contract deploy \
    --wasm "${WASM_PATH}" \
    --source "${source_account}" \
    --network "${network}")"

  log "Deployed. Contract ID: ${contract_id}"
  printf 'export CONTRACT_ID="%s"\n' "${contract_id}"
}

cmd_deploy_testnet() {
  deploy_to_network "testnet"
}

cmd_deploy_mainnet() {
  deploy_to_network "mainnet"
}

cmd_init() {
  local network="testnet"
  while [ $# -gt 0 ]; do
    case "$1" in
      --network)
        network="$2"
        shift 2
        ;;
      *)
        fail "Unknown option '$1' for 'init'"
        ;;
    esac
  done

  require_cmd stellar
  [ -n "${CONTRACT_ID:-}" ] || fail "CONTRACT_ID is required (export CONTRACT_ID=\"C...\")"
  [ -n "${ADMIN_ADDRESS:-}" ] || fail "ADMIN_ADDRESS is required"
  [ -n "${TOKEN_ADDRESS:-}" ] || fail "TOKEN_ADDRESS is required"
  local source_account="${SOURCE_ACCOUNT:-deployer}"
  local platform_fee="${PLATFORM_FEE:-${DEFAULT_PLATFORM_FEE}}"

  log "Initializing contract ${CONTRACT_ID} on ${network} (fee=${platform_fee}bps)"
  stellar contract invoke \
    --id "${CONTRACT_ID}" \
    --source "${source_account}" \
    --network "${network}" \
    -- \
    init \
    --admin "${ADMIN_ADDRESS}" \
    --token "${TOKEN_ADDRESS}" \
    --platform_fee "${platform_fee}"

  log "Initialization complete."
}

cmd_verify() {
  local network="testnet"
  while [ $# -gt 0 ]; do
    case "$1" in
      --network)
        network="$2"
        shift 2
        ;;
      *)
        fail "Unknown option '$1' for 'verify'"
        ;;
    esac
  done

  require_cmd stellar
  [ -n "${CONTRACT_ID:-}" ] || fail "CONTRACT_ID is required (export CONTRACT_ID=\"C...\")"
  local source_account="${SOURCE_ACCOUNT:-deployer}"

  log "Checking contract info for ${CONTRACT_ID} on ${network}"
  stellar contract info --id "${CONTRACT_ID}" --network "${network}"

  log "Reading contract_version() for ${CONTRACT_ID} on ${network}"
  stellar contract invoke \
    --id "${CONTRACT_ID}" \
    --source "${source_account}" \
    --network "${network}" \
    --is-read-only \
    -- \
    contract_version

  log "Verification complete."
}

usage() {
  sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

main() {
  local command="${1:-}"
  [ -n "${command}" ] || { usage; exit 1; }
  shift || true

  case "${command}" in
    deploy-testnet) cmd_deploy_testnet "$@" ;;
    deploy-mainnet) cmd_deploy_mainnet "$@" ;;
    init) cmd_init "$@" ;;
    verify) cmd_verify "$@" ;;
    -h|--help|help) usage ;;
    *) fail "Unknown command '${command}'. Run '$0 --help' for usage." ;;
  esac
}

main "$@"
