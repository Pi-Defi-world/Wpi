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

## DEX / AMM

The `wPi/USDC` pair integrates against **Soroswap** (Uniswap-v2-style Soroban
AMM) on Stellar testnet. Pool creation and initial liquidity seeding are scripted
and reproducible:

```bash
cp scripts/dex.testnet.env.example scripts/dex.testnet.env   # fill in ids + amounts
set -a; source scripts/dex.testnet.env; set +a
scripts/seed_testnet_liquidity.sh --dry-run                  # review the plan
scripts/seed_testnet_liquidity.sh                            # create + seed the pool
```

The pair is created implicitly by the first `add_liquidity` call. A local
`mock-amm` fallback (`--amm mock`) needs no external AMM. Testnet only — the
script refuses mainnet. AMM choice + rationale, the seeding runbook, and
price-impact / slippage guidance are in
[`../docs/dex-integration.md`](../docs/dex-integration.md).
