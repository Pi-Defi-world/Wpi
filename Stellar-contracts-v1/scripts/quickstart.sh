#!/usr/bin/env bash
set -euo pipefail

# End-to-end Stellar testnet walkthrough for wPI, mock USDC, and mock AMM.
# Flow: deploy -> initialize -> mint -> approve -> transfer -> swap.
#
# Prerequisites:
#   - stellar-cli (or soroban-cli aliased as `stellar`) installed
#   - rust wasm32-unknown-unknown target installed
#
# Optional environment overrides:
#   ADMIN_IDENTITY   Stellar CLI identity for the deployer/admin (default: wpi-admin)
#   RECIPIENT_IDENTITY Stellar CLI identity for transfer recipient (default: wpi-recipient)
#   NETWORK          Stellar CLI network name (default: testnet)
#   RPC_URL          Testnet RPC URL (default: https://soroban-testnet.stellar.org)
#   NETWORK_PASSPHRASE (default: Test SDF Network ; September 2015)

ADMIN_IDENTITY="${ADMIN_IDENTITY:-wpi-admin}"
RECIPIENT_IDENTITY="${RECIPIENT_IDENTITY:-wpi-recipient}"
NETWORK="${NETWORK:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
WPI_WASM="${WPI_WASM:-target/wasm32-unknown-unknown/release/wpi_token.wasm}"
USDC_WASM="${USDC_WASM:-target/wasm32-unknown-unknown/release/mock_usdc.wasm}"
AMM_WASM="${AMM_WASM:-target/wasm32-unknown-unknown/release/mock_amm.wasm}"
MINT_AMOUNT="${MINT_AMOUNT:-1000000000}"       # 100.0000000 tokens
TRANSFER_AMOUNT="${TRANSFER_AMOUNT:-10000000}" # 1.0000000 token
SWAP_AMOUNT="${SWAP_AMOUNT:-50000000}"         # 5.0000000 wPI
MIN_AMOUNT_OUT="${MIN_AMOUNT_OUT:-50000000}"   # 5.0000000 mUSDC
LIQUIDITY_AMOUNT="${LIQUIDITY_AMOUNT:-500000000}" # 50.0000000 mUSDC
RATE_BPS="${RATE_BPS:-1000000}" # 1:1 because mock-amm computes out = in * rate / 1_000_000

if command -v stellar >/dev/null 2>&1; then
  CLI=(stellar)
elif command -v soroban >/dev/null 2>&1; then
  CLI=(soroban)
else
  echo "ERROR: install stellar-cli or soroban-cli before running this quickstart." >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run() {
  echo
  echo "+ $*"
  "$@"
}

invoke() {
  local contract_id="$1"
  shift
  run "${CLI[@]}" contract invoke \
    --id "$contract_id" \
    --source-account "$ADMIN_IDENTITY" \
    --network "$NETWORK" \
    -- "$@"
}

account_address() {
  "${CLI[@]}" keys address "$1"
}

ensure_network() {
  if "${CLI[@]}" network ls 2>/dev/null | awk '{print $1}' | grep -qx "$NETWORK"; then
    return
  fi

  run "${CLI[@]}" network add \
    --global "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE"
}

ensure_identity() {
  local identity="$1"
  if "${CLI[@]}" keys address "$identity" >/dev/null 2>&1; then
    echo "Using existing identity '$identity': $(account_address "$identity")"
    return
  fi

  run "${CLI[@]}" keys generate "$identity" --network "$NETWORK" --fund
  echo "Expected output: a new funded testnet key named '$identity'."
}

echo "== wPI Stellar testnet quickstart =="
ensure_network
ensure_identity "$ADMIN_IDENTITY"
ensure_identity "$RECIPIENT_IDENTITY"

ADMIN_ADDRESS="$(account_address "$ADMIN_IDENTITY")"
RECIPIENT_ADDRESS="$(account_address "$RECIPIENT_IDENTITY")"

echo
echo "Admin address:     $ADMIN_ADDRESS"
echo "Recipient address: $RECIPIENT_ADDRESS"

echo
echo "== Build WASM artifacts =="
run rustup target add wasm32-unknown-unknown
run cargo build --target wasm32-unknown-unknown --release
echo "Expected output: Finished release build and three WASM files under target/wasm32-unknown-unknown/release/."

echo
echo "== Deploy contracts =="
WPI_CONTRACT_ID="$("${CLI[@]}" contract deploy --wasm "$WPI_WASM" --source-account "$ADMIN_IDENTITY" --network "$NETWORK")"
echo "+ ${CLI[*]} contract deploy --wasm $WPI_WASM --source-account $ADMIN_IDENTITY --network $NETWORK"
echo "Expected output: wPI contract ID"
echo "WPI_CONTRACT_ID=$WPI_CONTRACT_ID"

USDC_CONTRACT_ID="$("${CLI[@]}" contract deploy --wasm "$USDC_WASM" --source-account "$ADMIN_IDENTITY" --network "$NETWORK")"
echo "+ ${CLI[*]} contract deploy --wasm $USDC_WASM --source-account $ADMIN_IDENTITY --network $NETWORK"
echo "Expected output: mock USDC contract ID"
echo "MOCK_USDC_CONTRACT_ID=$USDC_CONTRACT_ID"

AMM_CONTRACT_ID="$("${CLI[@]}" contract deploy --wasm "$AMM_WASM" --source-account "$ADMIN_IDENTITY" --network "$NETWORK")"
echo "+ ${CLI[*]} contract deploy --wasm $AMM_WASM --source-account $ADMIN_IDENTITY --network $NETWORK"
echo "Expected output: mock AMM contract ID"
echo "MOCK_AMM_CONTRACT_ID=$AMM_CONTRACT_ID"

echo
echo "== Initialize contracts =="
invoke "$WPI_CONTRACT_ID" initialize --admin "$ADMIN_ADDRESS"
echo "Expected output: null/success"
invoke "$USDC_CONTRACT_ID" initialize --admin "$ADMIN_ADDRESS"
echo "Expected output: null/success"
invoke "$AMM_CONTRACT_ID" initialize --admin "$ADMIN_ADDRESS" --token_in "$WPI_CONTRACT_ID" --token_out "$USDC_CONTRACT_ID" --rate_bps "$RATE_BPS"
echo "Expected output: null/success"

echo
echo "== Mint balances =="
invoke "$WPI_CONTRACT_ID" mint --admin "$ADMIN_ADDRESS" --to "$ADMIN_ADDRESS" --amount "$MINT_AMOUNT"
echo "Expected output: Ok/null; admin now has 100.0000000 wPI by default."
invoke "$USDC_CONTRACT_ID" mint --admin "$ADMIN_ADDRESS" --to "$ADMIN_ADDRESS" --amount "$MINT_AMOUNT"
echo "Expected output: Ok/null; admin now has 100.0000000 mUSDC by default."

echo
echo "== Approve the mock AMM to spend wPI =="
invoke "$WPI_CONTRACT_ID" approve --owner "$ADMIN_ADDRESS" --spender "$AMM_CONTRACT_ID" --amount "$SWAP_AMOUNT"
echo "Expected output: Ok/null; AMM allowance is $SWAP_AMOUNT stroops of wPI."

echo
echo "== Transfer wPI to a second testnet account =="
invoke "$WPI_CONTRACT_ID" transfer --from "$ADMIN_ADDRESS" --to "$RECIPIENT_ADDRESS" --amount "$TRANSFER_AMOUNT"
echo "Expected output: Ok/null; recipient receives 1.0000000 wPI by default."

echo
echo "== Seed AMM liquidity with mock USDC =="
invoke "$USDC_CONTRACT_ID" approve --owner "$ADMIN_ADDRESS" --spender "$AMM_CONTRACT_ID" --amount "$LIQUIDITY_AMOUNT"
echo "Expected output: Ok/null; AMM allowance is $LIQUIDITY_AMOUNT stroops of mUSDC."
invoke "$AMM_CONTRACT_ID" deposit_liquidity --from "$ADMIN_ADDRESS" --amount_out "$LIQUIDITY_AMOUNT"
echo "Expected output: null/success; AMM holds 50.0000000 mUSDC by default."

echo
echo "== Swap wPI for mock USDC =="
invoke "$AMM_CONTRACT_ID" swap --to "$ADMIN_ADDRESS" --amount_in "$SWAP_AMOUNT" --min_amount_out "$MIN_AMOUNT_OUT"
echo "Expected output: $MIN_AMOUNT_OUT (5.0000000 mUSDC by default)."

echo
echo "== Verify balances =="
invoke "$WPI_CONTRACT_ID" balance --owner "$ADMIN_ADDRESS"
invoke "$USDC_CONTRACT_ID" balance --owner "$ADMIN_ADDRESS"
invoke "$WPI_CONTRACT_ID" balance --owner "$RECIPIENT_ADDRESS"

echo
echo "Quickstart complete. Export these for backend/test clients:"
echo "export WPI_CONTRACT_ID=$WPI_CONTRACT_ID"
echo "export MOCK_USDC_CONTRACT_ID=$USDC_CONTRACT_ID"
echo "export MOCK_AMM_CONTRACT_ID=$AMM_CONTRACT_ID"
