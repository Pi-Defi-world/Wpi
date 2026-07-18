# Wpi — Wrapped Pi on Stellar

Soroban contracts and operational docs for the **wrapped Pi (wPi)** bridge token on Stellar.

## Repository layout

| Path | Description |
|------|-------------|
| [`Stellar-contracts-v1/`](./Stellar-contracts-v1/) | `wpi-token`, `mock-usdc`, `mock-amm` contracts |
| [`docs/proof-of-reserve.md`](./docs/proof-of-reserve.md) | Off-chain signed reserve attestation process |
| [`docs/design/on-chain-reserve-oracle.md`](./docs/design/on-chain-reserve-oracle.md) | Medium-term on-chain oracle + mint invariant design |
| [`scripts/por/`](./scripts/por/) | Attest / verify CLI (Node.js, no dependencies) |
| [`attestations/`](./attestations/) | PoR feed (`latest.json` is **demo** until production cadence; schema + attestor pubkey) |

## Quick start (contracts)

```bash
cd Stellar-contracts-v1
cargo build --target wasm32-unknown-unknown --release
```

See [`Stellar-contracts-v1/README.md`](./Stellar-contracts-v1/README.md).

## Quick start (proof of reserve)

```bash
# Verify the published attestation
node scripts/por/verify.mjs attestations/latest.json

# Produce a new attestation (requires env — see docs)
node scripts/por/attest.mjs keygen   # once; keep secret offline
node scripts/por/attest.mjs attest
```

Full ops guide: [`docs/proof-of-reserve.md`](./docs/proof-of-reserve.md).
