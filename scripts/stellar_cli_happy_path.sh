#!/usr/bin/env bash
set -euo pipefail

NETWORK="${STELLAR_NETWORK:-local}"
WASM_PATH="${WASM_PATH:-target/wasm32-unknown-unknown/release/proof_of_heart.wasm}"
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$(mktemp -d)}"

stellar_args=(--network "$NETWORK")

wait_for_network() {
  local attempts=60
  for _ in $(seq 1 "$attempts"); do
    if stellar network health "${stellar_args[@]}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done

  echo "stellar local sandbox did not become ready" >&2
  return 1
}

invoke() {
  local source="$1"
  local contract_id="$2"
  local fn_name="$3"
  shift 3

  stellar contract invoke \
    "${stellar_args[@]}" \
    --source "$source" \
    --id "$contract_id" \
    -- \
    "$fn_name" \
    "$@"
}

assert_eq() {
  local actual="$1"
  local expected="$2"
  local label="$3"

  if [[ "$actual" != "$expected" ]]; then
    echo "assertion failed for ${label}: expected '${expected}', got '${actual}'" >&2
    return 1
  fi
}

extract_campaign_bool() {
  local campaign="$1"
  local field="$2"

  grep -Eo "${field}: (true|false)" <<<"$campaign" | awk '{print $2}' | head -n1
}

extract_campaign_i128() {
  local campaign="$1"
  local field="$2"

  grep -Eo "${field}: -?[0-9]+" <<<"$campaign" | awk '{print $2}' | head -n1
}

if [[ ! -f "$WASM_PATH" ]]; then
  echo "contract wasm not found at ${WASM_PATH}" >&2
  exit 1
fi

stellar container start "$NETWORK"
trap 'stellar container stop "$NETWORK" || true' EXIT
wait_for_network

stellar keys generate admin --fund "${stellar_args[@]}" >/dev/null
stellar keys generate creator --fund "${stellar_args[@]}" >/dev/null
stellar keys generate contributor --fund "${stellar_args[@]}" >/dev/null

ADMIN_ADDRESS="$(stellar keys address admin)"
CREATOR_ADDRESS="$(stellar keys address creator)"
CONTRIBUTOR_ADDRESS="$(stellar keys address contributor)"

TOKEN_ID="$(
  stellar contract asset deploy \
    "${stellar_args[@]}" \
    --source admin \
    --asset native
)"

CONTRACT_ID="$(
  stellar contract deploy \
    "${stellar_args[@]}" \
    --source admin \
    --wasm "$WASM_PATH"
)"

invoke admin "$CONTRACT_ID" init \
  --admin "$ADMIN_ADDRESS" \
  --token "$TOKEN_ID" \
  --platform_fee 300 >/dev/null

assert_eq "$(invoke admin "$CONTRACT_ID" get_version)" "1" "contract version"
assert_eq "$(invoke admin "$CONTRACT_ID" get_platform_fee)" "300" "platform fee"

PARAMS_FILE="$(mktemp)"
cat >"$PARAMS_FILE" <<JSON
{
  "creator": "$CREATOR_ADDRESS",
  "title": "CLI sandbox happy path",
  "description": "Exercises deployed WASM through stellar-cli local sandbox",
  "funding_goal": "1000",
  "duration_days": 7,
  "category": "Learner",
  "has_revenue_sharing": false,
  "revenue_share_percentage": 0,
  "max_contribution_per_user": "0"
}
JSON

CAMPAIGN_ID="$(invoke creator "$CONTRACT_ID" create_campaign --params-file-path "$PARAMS_FILE")"
assert_eq "$CAMPAIGN_ID" "1" "created campaign id"
assert_eq "$(invoke admin "$CONTRACT_ID" get_campaign_count)" "1" "campaign count"

invoke admin "$CONTRACT_ID" verify_campaign --campaign_id "$CAMPAIGN_ID" >/dev/null
assert_eq "$(invoke admin "$CONTRACT_ID" get_total_contributors_count --campaign_id "$CAMPAIGN_ID")" "0" "initial contributor count"

invoke contributor "$CONTRACT_ID" contribute \
  --campaign_id "$CAMPAIGN_ID" \
  --contributor "$CONTRIBUTOR_ADDRESS" \
  --amount 1000 >/dev/null

assert_eq "$(invoke contributor "$CONTRACT_ID" get_contribution --campaign_id "$CAMPAIGN_ID" --contributor "$CONTRIBUTOR_ADDRESS")" "1000" "contribution"
assert_eq "$(invoke admin "$CONTRACT_ID" get_total_raised_global)" "1000" "global raised after contribution"

invoke creator "$CONTRACT_ID" withdraw_funds --campaign_id "$CAMPAIGN_ID" >/dev/null

CAMPAIGN="$(invoke admin "$CONTRACT_ID" get_campaign --campaign_id "$CAMPAIGN_ID")"
assert_eq "$(extract_campaign_bool "$CAMPAIGN" "funds_withdrawn")" "true" "funds withdrawn"
assert_eq "$(extract_campaign_bool "$CAMPAIGN" "is_active")" "false" "campaign inactive"
assert_eq "$(extract_campaign_i128 "$CAMPAIGN" "amount_raised")" "1000" "amount raised"
assert_eq "$(invoke admin "$CONTRACT_ID" get_total_raised_global)" "0" "global raised after withdrawal"

echo "stellar-cli local sandbox happy path passed"
