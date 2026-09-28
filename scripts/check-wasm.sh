#!/usr/bin/env bash
# Validates the release contract WASM produced by
# `cargo build --target wasm32-unknown-unknown --release` (issue #1258):
#   1. the file exists and has the WASM magic header,
#   2. it passes `wasm-tools validate` when wasm-tools is installed,
#   3. it carries the Soroban contract spec / env-meta custom sections,
#   4. its size stays within 5% of .github/wasm-size-baseline.txt.
set -euo pipefail

WASM_PATH="${WASM_PATH:-target/wasm32-unknown-unknown/release/proof_of_heart.wasm}"
BASELINE_FILE="${BASELINE_FILE:-.github/wasm-size-baseline.txt}"

if [ ! -f "$WASM_PATH" ]; then
  echo "::error::WASM binary not found at $WASM_PATH. Run the release wasm build first."
  exit 1
fi

MAGIC=$(head -c 4 "$WASM_PATH" | od -An -tx1 | tr -d ' \n')
if [ "$MAGIC" != "0061736d" ]; then
  echo "::error::$WASM_PATH is not a WASM module (magic: $MAGIC)."
  exit 1
fi

if command -v wasm-tools >/dev/null 2>&1; then
  wasm-tools validate "$WASM_PATH"
  echo "wasm-tools validate: OK"
else
  echo "wasm-tools not installed; skipping structural validation."
fi

for section in contractspecv0 contractenvmetav0; do
  if ! grep -q "$section" "$WASM_PATH"; then
    echo "::error::$WASM_PATH is missing the Soroban '$section' custom section."
    exit 1
  fi
done
echo "Soroban custom sections: OK"

ACTUAL=$(wc -c < "$WASM_PATH" | tr -d ' ')
BASELINE=$(tr -d '[:space:]' < "$BASELINE_FILE")
# Allow up to 5% growth over the recorded baseline before failing.
LIMIT=$((BASELINE * 105 / 100))
echo "Baseline: ${BASELINE} bytes"
echo "Actual:   ${ACTUAL} bytes"
if [ "$ACTUAL" -gt "$LIMIT" ]; then
  DIFF=$((ACTUAL - BASELINE))
  PCT=$((DIFF * 100 / BASELINE))
  echo "::error::WASM binary grew by ${DIFF} bytes (${PCT}%), exceeding the 5% regression budget over the ${BASELINE}-byte baseline in ${BASELINE_FILE}. If this growth is expected, update the baseline in the same PR."
  exit 1
fi
echo "WASM size within budget."
