# Wpi — Wrapped Pi on Stellar

Soroban contracts and operational docs for the **wrapped Pi (wPi)** bridge token on Stellar.

## Repository layout

| Path | Description |
|------|-------------|
| [`Stellar-contracts-v1/`](./Stellar-contracts-v1/) | `wpi-token` and test AMM contracts integrated with the real USDC SAC |
| [`relayer/`](./relayer/) | TypeScript bridge service: Pi deposit watcher, mint submitter, redemption watcher |
| [`COMPATIBILITY.md`](./COMPATIBILITY.md) | Pi ↔ Stellar protocol-version skew, drift detection, and update policy (Issue #27) |
| [`docs/dex-integration.md`](./docs/dex-integration.md) | Soroswap `wPi/USDC` testnet pool creation + liquidity-seeding runbook and slippage guidance (Issue #10) |
| [`docs/deposit-eligibility.md`](./docs/deposit-eligibility.md) | Which Pi accounts may originate a bridge deposit, and how the relayer enforces it (Issue #28) |
| [`docs/proof-of-reserve.md`](./docs/proof-of-reserve.md) | Off-chain signed reserve attestation process |
| [`docs/release-management.md`](./docs/release-management.md) | Release management, semantic versioning policy, and deployment artifact tracking |
| [`docs/design/on-chain-reserve-oracle.md`](./docs/design/on-chain-reserve-oracle.md) | Medium-term on-chain oracle + mint invariant design |
| [`scripts/por/`](./scripts/por/) | Attest / verify CLI (Node.js, no dependencies) |
| [`attestations/`](./attestations/) | PoR feed (`latest.json` is **demo** until production cadence; schema + attestor pubkey) |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | How to build, test, and submit changes (Issue #30) |
| [`CHANGELOG.md`](./CHANGELOG.md) | Centralized project changelog and release history |


## Quick start (contracts)

```bash
make build
make test
```

Deploy to Stellar testnet with:

```bash
make deploy-testnet
```

This runs [`scripts/deploy_testnet.sh`](./scripts/deploy_testnet.sh).

Mainnet deploys require an explicit signer and RPC endpoint:

```bash
STELLAR_ACCOUNT=<identity> STELLAR_RPC_URL=<mainnet-rpc-url> make deploy-mainnet
```

This runs [`scripts/deploy_mainnet.sh`](./scripts/deploy_mainnet.sh).

See [`Stellar-contracts-v1/README.md`](./Stellar-contracts-v1/README.md).

## Quick start (DEX liquidity — testnet)

Create and seed the Soroswap `wPi/USDC` pool on Stellar testnet:

```bash
cp scripts/dex.testnet.env.example scripts/dex.testnet.env   # fill in contract ids + amounts
set -a; source scripts/dex.testnet.env; set +a
scripts/seed_testnet_liquidity.sh --dry-run                  # print every invocation, sign nothing
scripts/seed_testnet_liquidity.sh                            # create pair (auto) + add_liquidity
```

Testnet only; the script refuses mainnet. AMM choice, rationale, the full
seeding runbook, and price-impact / slippage guidance:
[`docs/dex-integration.md`](./docs/dex-integration.md).

## Quick start (proof of reserve)

```bash
# Verify the published attestation
node scripts/por/verify.mjs attestations/latest.json

# Produce a new attestation (requires env — see docs)
node scripts/por/attest.mjs keygen   # once; keep secret offline
node scripts/por/attest.mjs attest
```

Full ops guide: [`docs/proof-of-reserve.md`](./docs/proof-of-reserve.md).
