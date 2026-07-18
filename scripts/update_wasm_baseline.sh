#!/usr/bin/env bash
# update_wasm_baseline.sh
#
# Reads compiled WASM files from a release build and writes fresh byte-count
# values into Stellar-contracts-v1/wasm-size-baseline.json.
#
# Usage (from the repo root):
#   bash scripts/update_wasm_baseline.sh
#
# Prerequisites:
#   - A completed release WASM build:
#       cd Stellar-contracts-v1
#       cargo build --target wasm32-unknown-unknown --release
#   - jq must be installed (brew install jq  /  apt install jq)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM_DIR="${REPO_ROOT}/Stellar-contracts-v1/target/wasm32-unknown-unknown/release"
BASELINE="${REPO_ROOT}/Stellar-contracts-v1/wasm-size-baseline.json"

CONTRACTS=("wpi_token" "mock_usdc" "mock_amm")

echo "=== Updating WASM size baseline ==="
echo ""

# Verify jq is available
if ! command -v jq &>/dev/null; then
  echo "ERROR: jq is required but not found. Install it and re-run."
  exit 1
fi

# Build new baseline JSON
JSON="{"
JSON+=$'\n'
JSON+="  \"_comment\": \"WASM size baseline in bytes. CI fails if any contract grows by more than WASM_SIZE_THRESHOLD_PCT (default 5%).\","
JSON+=$'\n'
JSON+="  \"_update_instructions\": \"Run \`bash scripts/update_wasm_baseline.sh\` from the repo root after a clean release build, then commit the result.\","
JSON+=$'\n'

for name in "${CONTRACTS[@]}"; do
  WASM_FILE="${WASM_DIR}/${name}.wasm"
  if [[ -f "$WASM_FILE" ]]; then
    if [[ "$(uname)" == "Darwin" ]]; then
      BYTES=$(stat -f%z "$WASM_FILE")
    else
      BYTES=$(stat -c%s "$WASM_FILE")
    fi
    KB=$(awk "BEGIN { printf \"%.2f\", ${BYTES}/1024 }")
    echo "  ${name}.wasm  →  ${BYTES} bytes  (${KB} KB)"
    JSON+="  \"${name}\": ${BYTES},"
    JSON+=$'\n'
  else
    echo "  WARNING: ${WASM_FILE} not found — skipping (build the contracts first)."
  fi
done

# Trim trailing comma from last data line
JSON="${JSON%,$'\n'}"$'\n'
JSON+="}"

echo "$JSON" > "$BASELINE"

echo ""
echo "Written: ${BASELINE}"
echo ""
echo "Next steps:"
echo "  1. Review the updated values above."
echo "  2. git add Stellar-contracts-v1/wasm-size-baseline.json"
echo "  3. git commit -m 'chore: update WASM size baseline'"
