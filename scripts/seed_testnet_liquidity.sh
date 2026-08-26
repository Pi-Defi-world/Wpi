#!/usr/bin/env bash
# seed_testnet_liquidity.sh
#
# Reproducible wPi/USDC pool creation + initial liquidity seeding on Stellar
# TESTNET. See docs/dex-integration.md for the AMM choice, rationale, runbook,
# and price-impact / slippage guidance.
#
# Usage:
#   set -a; source scripts/dex.testnet.env; set +a
#   scripts/seed_testnet_liquidity.sh [--dry-run] [--amm soroswap|mock]
#
# Defaults: --amm soroswap, live submit on testnet.
# Mainnet is refused unless I_UNDERSTAND_MAINNET_DEX=yes, and even then the
# script forces --dry-run and only prints a plan.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT_DIR="${ROOT_DIR}/Stellar-contracts-v1"
CLI="${STELLAR_CLI:-stellar}"

# --- args ---------------------------------------------------------------------
DRY_RUN=false
AMM="soroswap"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=true; shift ;;
    --amm) AMM="${2:-}"; shift 2 ;;
    --amm=*) AMM="${1#*=}"; shift ;;
    -h|--help)
      sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ "$AMM" != "soroswap" && "$AMM" != "mock" ]]; then
  echo "ERROR: --amm must be 'soroswap' or 'mock' (got '${AMM}')" >&2
  exit 2
fi

# --- config -----------------------------------------------------------------
NETWORK="${STELLAR_NETWORK:-testnet}"
RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
SOURCE_ACCOUNT="${STELLAR_ACCOUNT:-${ADMIN_IDENTITY:-wpi-testnet-admin}}"

MAINNET_PASSPHRASE="Public Global Stellar Network ; September 2015"

WPI_CONTRACT_ID="${WPI_CONTRACT_ID:-}"
MOCK_USDC_CONTRACT_ID="${MOCK_USDC_CONTRACT_ID:-}"
MOCK_AMM_CONTRACT_ID="${MOCK_AMM_CONTRACT_ID:-}"
SOROSWAP_ROUTER_ID="${SOROSWAP_ROUTER_ID:-}"
SOROSWAP_FACTORY_ID="${SOROSWAP_FACTORY_ID:-}"

SEED_WPI_AMOUNT="${SEED_WPI_AMOUNT:-1000000000000}"
SEED_USDC_AMOUNT="${SEED_USDC_AMOUNT:-314000000000}"
SEED_SLIPPAGE_BPS="${SEED_SLIPPAGE_BPS:-50}"
SEED_DEADLINE_SECS="${SEED_DEADLINE_SECS:-300}"
SEED_MINT_WPI="${SEED_MINT_WPI:-false}"
SEED_MINT_USDC="${SEED_MINT_USDC:-true}"

MOCK_USDC_WASM="${MOCK_USDC_WASM:-${CONTRACT_DIR}/target/wasm32-unknown-unknown/release/mock_usdc.wasm}"
MOCK_AMM_WASM="${MOCK_AMM_WASM:-${CONTRACT_DIR}/target/wasm32-unknown-unknown/release/mock_amm.wasm}"

NETWORK_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")

# --- helpers ---------------------------------------------------------------
run() {
  echo "+ $*"
  if [[ "$DRY_RUN" == "true" ]]; then
    return 0
  fi
  "$@"
}

# run + capture stdout (last line). Honors --dry-run by echoing a placeholder.
run_capture() {
  echo "+ $*" >&2
  if [[ "$DRY_RUN" == "true" ]]; then
    printf '<dry-run>'
    return 0
  fi
  "$@" | tail -n 1
}

ensure_cli() {
  if ! command -v "$CLI" >/dev/null 2>&1; then
    echo "ERROR: Stellar CLI not found. Set STELLAR_CLI or install stellar-cli." >&2
    exit 1
  fi
}

guard_network() {
  if [[ "$NETWORK" == "mainnet" || "$NETWORK_PASSPHRASE" == "$MAINNET_PASSPHRASE" ]]; then
    if [[ "${I_UNDERSTAND_MAINNET_DEX:-}" != "yes" ]]; then
      echo "ERROR: this script is testnet-only. Refusing to target mainnet." >&2
      echo "       Mainnet pool creation is a reviewed multi-sig treasury action;" >&2
      echo "       it is intentionally not automated here (see docs/dex-integration.md)." >&2
      exit 1
    fi
    echo "WARNING: mainnet target acknowledged — forcing --dry-run, plan only." >&2
    DRY_RUN=true
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

ensure_identity() {
  if "$CLI" keys address "$SOURCE_ACCOUNT" >/dev/null 2>&1; then
    ADMIN_ADDRESS="$("$CLI" keys address "$SOURCE_ACCOUNT")"
    return
  fi
  run "$CLI" keys generate "$SOURCE_ACCOUNT" "${NETWORK_ARGS[@]}" --fund
  if [[ "$DRY_RUN" == "true" ]]; then
    ADMIN_ADDRESS="<treasury-address>"
  else
    ADMIN_ADDRESS="$("$CLI" keys address "$SOURCE_ACCOUNT")"
  fi
}

require() {
  local name="$1" val="$2"
  if [[ -z "$val" ]]; then
    echo "ERROR: ${name} is required (set it in scripts/dex.testnet.env)." >&2
    exit 1
  fi
}

invoke() {
  local contract_id="$1"; shift
  run "$CLI" contract invoke \
    --id "$contract_id" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}" \
    -- "$@"
}

invoke_capture() {
  local contract_id="$1"; shift
  run_capture "$CLI" contract invoke \
    --id "$contract_id" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}" \
    -- "$@"
}

build_if_missing() {
  local wasm="$1"
  if [[ -f "$wasm" ]]; then
    return
  fi
  echo "== Building contracts (missing $(basename "$wasm")) ==" >&2
  ( cd "$CONTRACT_DIR"
    run rustup target add wasm32-unknown-unknown
    run cargo build --locked --target wasm32-unknown-unknown --release )
}

deploy_contract() {
  local label="$1" wasm="$2"
  build_if_missing "$wasm"
  echo "== Deploy ${label} ==" >&2
  run_capture "$CLI" contract deploy \
    --wasm "$wasm" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}"
}

stroops_to_human() {
  awk "BEGIN { printf \"%.7f\", ${1:-0} / 10000000 }"
}

# --- preflight -----------------------------------------------------------
ensure_cli
guard_network
ensure_network
ensure_identity

echo "AMM:             ${AMM}"
echo "Network:         ${NETWORK} (${RPC_URL})"
echo "Treasury:        ${SOURCE_ACCOUNT}  ${ADMIN_ADDRESS}"
echo "Dry run:         ${DRY_RUN}"

require "WPI_CONTRACT_ID" "$WPI_CONTRACT_ID"

# mock-usdc: reuse or deploy+initialize.
if [[ -z "$MOCK_USDC_CONTRACT_ID" ]]; then
  MOCK_USDC_CONTRACT_ID="$(deploy_contract MOCK_USDC "$MOCK_USDC_WASM")"
  echo "MOCK_USDC_CONTRACT_ID=${MOCK_USDC_CONTRACT_ID}"
  invoke "$MOCK_USDC_CONTRACT_ID" initialize --admin "$ADMIN_ADDRESS"
fi
echo "USDC leg:        ${MOCK_USDC_CONTRACT_ID} (mock-usdc)"

# --- derived amounts ---------------------------------------------------------
KEEP_BPS=$(( 10000 - SEED_SLIPPAGE_BPS ))
WPI_MIN=$(( SEED_WPI_AMOUNT * KEEP_BPS / 10000 ))
USDC_MIN=$(( SEED_USDC_AMOUNT * KEEP_BPS / 10000 ))
if [[ "$DRY_RUN" == "true" ]]; then
  DEADLINE=$(( SEED_DEADLINE_SECS ))   # placeholder; real run uses wall clock
  echo "Deadline:        now + ${SEED_DEADLINE_SECS}s (computed at submit time)"
else
  DEADLINE=$(( $(date +%s) + SEED_DEADLINE_SECS ))
fi

echo
echo "== Seed plan =="
echo "wPi desired:     $(stroops_to_human "$SEED_WPI_AMOUNT")  (min $(stroops_to_human "$WPI_MIN"))"
echo "USDC desired:    $(stroops_to_human "$SEED_USDC_AMOUNT")  (min $(stroops_to_human "$USDC_MIN"))"
echo "Opening price:   $(awk "BEGIN { printf \"%.7f\", ${SEED_USDC_AMOUNT}/${SEED_WPI_AMOUNT} }") USDC per wPi"
echo "Slippage:        ${SEED_SLIPPAGE_BPS} bps"
echo

# --- optional minting --------------------------------------------------------
if [[ "$SEED_MINT_WPI" == "true" ]]; then
  echo "== Mint wPi to treasury =="
  invoke "$WPI_CONTRACT_ID" mint \
    --admin "$ADMIN_ADDRESS" --to "$ADMIN_ADDRESS" --amount "$SEED_WPI_AMOUNT"
fi
if [[ "$SEED_MINT_USDC" == "true" ]]; then
  echo "== Mint mock USDC to treasury =="
  invoke "$MOCK_USDC_CONTRACT_ID" mint \
    --admin "$ADMIN_ADDRESS" --to "$ADMIN_ADDRESS" --amount "$SEED_USDC_AMOUNT"
fi

# --- seed via Soroswap -----------------------------------------------------
seed_soroswap() {
  require "SOROSWAP_ROUTER_ID" "$SOROSWAP_ROUTER_ID"

  echo "== Soroswap add_liquidity (pair auto-creates on first call) =="
  invoke "$SOROSWAP_ROUTER_ID" add_liquidity \
    --token_a "$WPI_CONTRACT_ID" \
    --token_b "$MOCK_USDC_CONTRACT_ID" \
    --amount_a_desired "$SEED_WPI_AMOUNT" \
    --amount_b_desired "$SEED_USDC_AMOUNT" \
    --amount_a_min "$WPI_MIN" \
    --amount_b_min "$USDC_MIN" \
    --to "$ADMIN_ADDRESS" \
    --deadline "$DEADLINE"

  echo "== Resolve pair + reserves =="
  local pair reserves
  pair="$(invoke_capture "$SOROSWAP_ROUTER_ID" router_pair_for \
    --token_a "$WPI_CONTRACT_ID" --token_b "$MOCK_USDC_CONTRACT_ID")"
  echo "pair:            ${pair}"

  if [[ "$DRY_RUN" != "true" ]]; then
    reserves="$(invoke_capture "$pair" get_reserves)"
    echo "reserves:        ${reserves}   (order follows the pair's token0/token1)"
    echo "verify:          stellar contract invoke --id ${SOROSWAP_FACTORY_ID:-<factory>} ${NETWORK_ARGS[*]} \\"
    echo "                   --source-account ${SOURCE_ACCOUNT} -- get_pair \\"
    echo "                   --token_a ${WPI_CONTRACT_ID} --token_b ${MOCK_USDC_CONTRACT_ID}"
  fi
}

# --- seed via local mock-amm ----------------------------------------------
seed_mock() {
  if [[ -z "$MOCK_AMM_CONTRACT_ID" ]]; then
    MOCK_AMM_CONTRACT_ID="$(deploy_contract MOCK_AMM "$MOCK_AMM_WASM")"
    echo "MOCK_AMM_CONTRACT_ID=${MOCK_AMM_CONTRACT_ID}"
    echo "== Initialize mock-amm for wPi -> USDC (1:1 sim) =="
    invoke "$MOCK_AMM_CONTRACT_ID" initialize \
      --admin "$ADMIN_ADDRESS" \
      --token_in "$WPI_CONTRACT_ID" \
      --token_out "$MOCK_USDC_CONTRACT_ID" \
      --rate_bps 1000000
  fi

  echo "== mock-amm deposit_liquidity (USDC side only; 1:1 swap sim) =="
  invoke "$MOCK_AMM_CONTRACT_ID" deposit_liquidity \
    --from "$ADMIN_ADDRESS" \
    --amount_out "$SEED_USDC_AMOUNT"

  if [[ "$DRY_RUN" != "true" ]]; then
    local bal
    bal="$(invoke_capture "$MOCK_USDC_CONTRACT_ID" balance --owner "$MOCK_AMM_CONTRACT_ID")"
    echo "pool USDC balance: $(stroops_to_human "$bal")"
  fi
}

case "$AMM" in
  soroswap) seed_soroswap ;;
  mock)     seed_mock ;;
esac

echo
if [[ "$DRY_RUN" == "true" ]]; then
  echo "== Dry run complete — nothing submitted =="
else
  echo "== Pool seeded =="
fi
