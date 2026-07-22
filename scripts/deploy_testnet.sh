#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT_DIR="${ROOT_DIR}/Stellar-contracts-v1"
CLI="${STELLAR_CLI:-stellar}"

NETWORK="${STELLAR_NETWORK:-testnet}"
RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org/}"
NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
SOURCE_ACCOUNT="${STELLAR_ACCOUNT:-${ADMIN_IDENTITY:-wpi-testnet-admin}}"
RATE_BPS="${RATE_BPS:-1000000}"

WPI_WASM="${WPI_WASM:-${CONTRACT_DIR}/target/wasm32-unknown-unknown/release/wpi_token.wasm}"
AMM_WASM="${AMM_WASM:-${CONTRACT_DIR}/target/wasm32-unknown-unknown/release/mock_amm.wasm}"

NETWORK_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")

run() {
  echo "+ $*"
  "$@"
}

ensure_cli() {
  if ! command -v "$CLI" >/dev/null 2>&1; then
    echo "ERROR: Stellar CLI not found. Set STELLAR_CLI or install stellar-cli." >&2
    exit 1
  fi
}

ensure_network() {
  if "$CLI" network ls 2>/dev/null | awk '{print $1}' | grep -qx "$NETWORK"; then
    return
  fi

  run "$CLI" network add "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
}

ensure_testnet_identity() {
  if "$CLI" keys address "$SOURCE_ACCOUNT" >/dev/null 2>&1; then
    ADMIN_ADDRESS="$("$CLI" keys address "$SOURCE_ACCOUNT")"
    return
  fi

  run "$CLI" keys generate "$SOURCE_ACCOUNT" "${NETWORK_ARGS[@]}" --fund
  ADMIN_ADDRESS="$("$CLI" keys address "$SOURCE_ACCOUNT")"
}

build_contracts() {
  (
    cd "$CONTRACT_DIR"
    run rustup target add wasm32-unknown-unknown
    run cargo build --target wasm32-unknown-unknown --release
  )
}

require_artifact() {
  local artifact="$1"
  if [[ ! -f "$artifact" ]]; then
    echo "ERROR: missing WASM artifact: ${artifact}" >&2
    exit 1
  fi
}

upload_wasm() {
  local label="$1"
  local wasm="$2"
  local hash

  echo "== Upload ${label} WASM ==" >&2
  require_artifact "$wasm"
  hash="$("$CLI" contract upload \
    --wasm "$wasm" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}" | tail -n 1)"
  echo "${label}_WASM_HASH=${hash}" >&2
  printf '%s' "$hash"
}

deploy_uploaded_wasm() {
  local label="$1"
  local wasm_hash="$2"
  local contract_id

  echo "== Deploy ${label} contract ==" >&2
  contract_id="$("$CLI" contract deploy \
    --wasm-hash "$wasm_hash" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}" | tail -n 1)"
  echo "${label}_CONTRACT_ID=${contract_id}" >&2
  printf '%s' "$contract_id"
}

invoke_contract() {
  local contract_id="$1"
  shift
  run "$CLI" contract invoke \
    --id "$contract_id" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}" \
    -- "$@"
}

ensure_cli
ensure_network
ensure_testnet_identity
build_contracts

echo "Admin identity: ${SOURCE_ACCOUNT}"
echo "Admin address:  ${ADMIN_ADDRESS}"

WPI_HASH="$(upload_wasm WPI "$WPI_WASM")"
WPI_CONTRACT_ID="$(deploy_uploaded_wasm WPI "$WPI_HASH")"

AMM_HASH="$(upload_wasm MOCK_AMM "$AMM_WASM")"
MOCK_AMM_CONTRACT_ID="$(deploy_uploaded_wasm MOCK_AMM "$AMM_HASH")"

echo "== Initialize contracts =="
invoke_contract "$WPI_CONTRACT_ID" initialize --admin "$ADMIN_ADDRESS"
invoke_contract "$MOCK_AMM_CONTRACT_ID" initialize \
  --admin "$ADMIN_ADDRESS" \
  --token_in "$WPI_CONTRACT_ID" \
  --rate_bps "$RATE_BPS"

cat <<EOF

Testnet deployment complete.
export WPI_CONTRACT_ID=${WPI_CONTRACT_ID}
export MOCK_AMM_CONTRACT_ID=${MOCK_AMM_CONTRACT_ID}
EOF
