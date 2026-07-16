# Stellar-contracts-v1

Soroban contracts deployed on **Stellar** (testnet/mainnet) for the PUSD decentralized reserve bridge:

| Crate        | WASM artifact   | Purpose                                      |
|-------------|-----------------|----------------------------------------------|
| `wpi-token` | `wpi_token.wasm` | Wrapped Pi minted by the relayer after Pi deposits |
| `mock-usdc` | `mock_usdc.wasm` | Test-only USDC stand-in for AMM / reserve sims |

## Requirements

- Rust stable + `wasm32-unknown-unknown` target
- Soroban CLI aligned with **soroban-sdk 23.0.1** (same as `Pusd-contracts-v1`)

## Build

```bash
cd Stellar-contracts-v1
cargo build --target wasm32-unknown-unknown --release
```

Artifacts: `target/wasm32-unknown-unknown/release/*.wasm`

## Deploy (Stellar testnet)

Use Stellar CLI / Soroban with Stellar testnet RPC and passphrase `Test SDF Network ; September 2015`.  
Initialize each contract with `initialize(admin_address)` after upload.

Set backend env:

- `STELLAR_SOROBAN_RPC_URL` — e.g. `https://soroban-testnet.stellar.org`
- `STELLAR_NETWORK_PASSPHRASE` — Stellar testnet passphrase
- `WPI_CONTRACT_ID` / `MOCK_USDC_CONTRACT_ID` — deployed contract IDs
- `BRIDGE_STELLAR_ADMIN_SECRET_KEY` — admin key that mints wPi (keep offline in production)


## Quickstart: full testnet flow

Run the scripted walkthrough to build and exercise the complete testnet path:

```bash
cd Stellar-contracts-v1
./scripts/quickstart.sh
```

The script creates/funds fresh Stellar testnet identities when needed, deploys `wpi-token`, `mock-usdc`, and `mock-amm`, then runs initialize → mint → approve → transfer → liquidity deposit → swap with expected output notes after each command.

## DEX / AMM

Pool creation against Soroswap or another Stellar AMM is **not** included here; seed liquidity off-chain after deploying both tokens.
