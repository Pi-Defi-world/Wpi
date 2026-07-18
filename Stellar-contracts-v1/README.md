# Stellar-contracts-v1

Soroban contracts deployed on **Stellar** (testnet/mainnet) for the PUSD decentralized reserve bridge.

The relayer that mints wPi after Pi deposits are observed on Pi Network, and
that releases Pi on wPi redemption, lives in [`../relayer`](../relayer/README.md).

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

Run the scripted walkthrough to build and exercise the complete testnet path. It uses real Stellar/Soroban CLI commands against testnet, creates/funds fresh identities when needed, and prints the expected successful output after each step:

```bash
cd Stellar-contracts-v1
./scripts/quickstart.sh
```

The script deploys `wpi-token`, `mock-usdc`, and `mock-amm`, then runs initialize → mint → approve → transfer → liquidity deposit → swap. Override identities, amounts, or network settings with environment variables such as `ADMIN_IDENTITY`, `RECIPIENT_IDENTITY`, `RPC_URL`, `MINT_AMOUNT`, and `SWAP_AMOUNT`.

These same values, plus the Pi Network side, configure the relayer — see
[`../relayer/.env.example`](../relayer/.env.example).


## WASM Size Tracking

CI reports the compiled WASM size for each contract on every PR and compares it against the committed baseline in [`wasm-size-baseline.json`](./wasm-size-baseline.json).

| Contract | Baseline file key |
|---|---|
| `wpi-token` | `wpi_token` |
| `mock-usdc` | `mock_usdc` |
| `mock-amm` | `mock_amm` |

A contract that grows by more than **5 %** relative to its baseline will fail the `Check WASM size regressions` step. The full size table and diff are posted to the **Job Summary** tab in GitHub Actions.

### Setting / updating the baseline

After an intentional size change (new feature, dependency bump, etc.), refresh the baseline:

```bash
# 1. Build release WASMs
cd Stellar-contracts-v1
cargo build --target wasm32-unknown-unknown --release

# 2. Regenerate the baseline file (from the repo root)
cd ..
bash scripts/update_wasm_baseline.sh

# 3. Commit
git add Stellar-contracts-v1/wasm-size-baseline.json
git commit -m "chore: update WASM size baseline"
```

> **First-time setup**: The baseline ships with zeros so the first CI run reports sizes without failing. Copy the byte values from the Job Summary into `wasm-size-baseline.json` (or run `update_wasm_baseline.sh` locally) and commit them before relying on regression detection.

## DEX / AMM

Pool creation against Soroswap or another Stellar AMM is **not** included here; seed liquidity off-chain after deploying both tokens.

## Proof of reserve

wPi minting is admin/relayer-gated. Short-term **proof of reserve** is an off-chain signed attestation process (not an on-chain mint guard yet):

| Resource | Location |
|----------|----------|
| Process & ops | [`docs/proof-of-reserve.md`](../docs/proof-of-reserve.md) |
| On-chain oracle design | [`docs/design/on-chain-reserve-oracle.md`](../docs/design/on-chain-reserve-oracle.md) |
| Attestor CLI | `scripts/por/attest.mjs`, `scripts/por/verify.mjs` |
| Public feed | [`attestations/latest.json`](../attestations/latest.json) (demo until production cadence) |

```bash
# From repo root
node scripts/por/verify.mjs attestations/latest.json
```
