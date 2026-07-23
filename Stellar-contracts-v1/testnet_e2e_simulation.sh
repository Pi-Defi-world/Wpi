#!/bin/bash
set -e

echo "=========================================================="
echo " Wrapped Pi (wPi) - Decentralized Bridge E2E Simulation"
echo "=========================================================="
echo ""
echo "This script simulates the full end-to-end lifecycle on the Stellar Testnet:"
echo " 1. Setup   : Generate identities (relayer, user), fund via Friendbot"
echo " 2. Deploy  : Compile & deploy wPi and mock-AMM"
echo " 3. Seed    : Relayer deposits faucet-funded real USDC into the AMM"
echo " 4. Deposit : Relayer observes Pi deposit and mints wPi to User"
echo " 5. Swap    : User swaps wPi for real USDC via AMM"
echo " 6. Burn    : User burns remaining wPi to withdraw back to native Pi"
echo ""

# Configuration
NETWORK="testnet"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
RPC_URL="https://soroban-testnet.stellar.org"

# Check dependencies
if ! command -v soroban &> /dev/null; then
    echo "❌ Error: 'soroban' CLI is not installed."
    exit 1
fi

echo "[1/6] Building contracts..."
cargo build --target wasm32-unknown-unknown --release

echo "[1/6] Generating identities..."
# Relayer Identity
soroban keys generate relayer --network $NETWORK 2>/dev/null || echo "Relayer identity already exists"
RELAYER_PUB=$(soroban keys address relayer)
echo "Relayer Address: $RELAYER_PUB"

# User Identity (Alice)
soroban keys generate alice --network $NETWORK 2>/dev/null || echo "Alice identity already exists"
ALICE_PUB=$(soroban keys address alice)
echo "Alice Address: $ALICE_PUB"

echo "Funding identities via Friendbot (this may take a moment)..."
curl -s "https://friendbot.stellar.org/?addr=$RELAYER_PUB" > /dev/null
curl -s "https://friendbot.stellar.org/?addr=$ALICE_PUB" > /dev/null

echo "[2/6] Deploying Contracts to Testnet..."

WPI_WASM="target/wasm32-unknown-unknown/release/wpi_token.wasm"
USDC_ID="CAQCMV4JFG4EZXQEAV7TUV2E52DMSO2LQKBOSA7UM3B4NIP4DQJ3JHQJ"
AMM_WASM="target/wasm32-unknown-unknown/release/mock_amm.wasm"

WPI_ID=$(soroban contract deploy --wasm $WPI_WASM --source relayer --network $NETWORK)
echo "wPi Deployed: $WPI_ID"

AMM_ID=$(soroban contract deploy --wasm $AMM_WASM --source relayer --network $NETWORK)
echo "MockAMM Deployed: $AMM_ID"

echo "[2/6] Initializing Contracts..."
# Initialize wPi
soroban contract invoke --id $WPI_ID --source relayer --network $NETWORK -- \
  initialize --admin $RELAYER_PUB

# Initialize MockAMM (Rate: 1 wPi = 1 USDC -> rate_bps = 1_000_000)
soroban contract invoke --id $AMM_ID --source relayer --network $NETWORK -- \
  initialize --admin $RELAYER_PUB --token_in $WPI_ID --rate_bps 1000000

echo "[3/6] Seeding Liquidity..."
# The relayer must already hold this amount of Circle testnet USDC.
LIQUIDITY_AMOUNT=100000000000 # 10,000 USDC with 7 decimals
# Fund it from Circle's faucet before running the simulation.
soroban contract invoke --id $AMM_ID --source relayer --network $NETWORK -- \
  deposit_liquidity --from $RELAYER_PUB --amount_out $LIQUIDITY_AMOUNT

echo "[4/6] Bridge Deposit (Minting wPi to Alice)..."
# Relayer mints 1,000 wPi to Alice
WPI_MINT_AMOUNT=10000000000 # 1,000 wPi with 7 decimals
soroban contract invoke --id $WPI_ID --source relayer --network $NETWORK -- \
  mint --to $ALICE_PUB --amount $WPI_MINT_AMOUNT

echo "[5/6] AMM Swap (wPi -> real USDC)..."
# Alice swaps 500 wPi for USDC
SWAP_AMOUNT=5000000000 # 500 wPi
# First, Alice approves AMM to spend her wPi
soroban contract invoke --id $WPI_ID --source alice --network $NETWORK -- \
  approve --owner $ALICE_PUB --spender $AMM_ID --amount $SWAP_AMOUNT

# Then, Alice swaps
soroban contract invoke --id $AMM_ID --source alice --network $NETWORK -- \
  swap --to $ALICE_PUB --amount_in $SWAP_AMOUNT --min_amount_out $SWAP_AMOUNT

echo "[6/6] Bridge Withdrawal (Alice burns remaining wPi)..."
# Alice approves relayer to burn? Or relayer burns?
# In wpi-token, `burn` authenticates the stored admin and takes a `from` address.
# So the user must transfer/approve or the admin just burns it.
# Usually, to burn, the admin must be the one calling `burn` acting on behalf of the user's bridge request.
# Let's say Alice requests withdrawal, Relayer sees it and burns her remaining 500 wPi.
soroban contract invoke --id $WPI_ID --source relayer --network $NETWORK -- \
  burn --from $ALICE_PUB --amount $SWAP_AMOUNT

echo ""
echo "=========================================================="
echo " Verification"
echo "=========================================================="

ALICE_WPI=$(soroban contract invoke --id $WPI_ID --source alice --network $NETWORK -- balance --owner $ALICE_PUB)
ALICE_USDC=$(soroban contract invoke --id $USDC_ID --source alice --network $NETWORK -- balance --owner $ALICE_PUB)

echo "Alice wPi Balance: $ALICE_WPI (Expected: 0)"
echo "Alice USDC Balance: $ALICE_USDC (Expected: 5000000000)"

if [ "$ALICE_WPI" == '"0"' ] && [ "$ALICE_USDC" == '"5000000000"' ]; then
    echo "✅ SUCCESS: End-to-end integration simulation passed!"
    exit 0
else
    echo "❌ FAILED: Unexpected balances."
    exit 1
fi
