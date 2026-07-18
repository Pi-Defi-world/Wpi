# Proof of Reserve (PoR) — Wrapped Pi (wPi)

**Status:** Process and tooling are ready (v1); the published feed under `attestations/` is currently **demonstration data** until operators configure a real wPi contract ID, Pi custody balance source, and cadence job.  
**Related:** [Issue #25](https://github.com/Pi-Defi-world/Wpi/issues/25)  
**On-chain follow-up design:** [design/on-chain-reserve-oracle.md](./design/on-chain-reserve-oracle.md)

## Problem

Nothing on-chain verifies `wpi-token::total_supply()` against Pi held in the bridge custodial account on Pi Network. If the relayer misbehaves or custody is compromised, wPi can become under-collateralized with no public signal.

## Short-term solution (this document)

Publish a **signed reserve attestation** on a fixed cadence. Anyone can:

1. Fetch the latest attestation (JSON)
2. Verify the Ed25519 signature against the published attestor public key
3. Compare `pi_custody_balance` vs `wpi_total_supply` and check the collateral ratio

This is **transparency**, not full cryptographic proof of custody on Pi. Medium-term work moves the attested balance into a Soroban oracle and enforces a mint invariant on-chain (see design doc).

## Invariant (off-chain check)

```
collateral_ratio = pi_custody_balance / wpi_total_supply   (when supply > 0)

healthy  <=>  pi_custody_balance >= wpi_total_supply * (1 - safety_margin_bps / 10_000)
```

Default `safety_margin_bps = 0` (1:1). Operators may set a small positive margin for transfer lag.

## Attestation schema

See [`attestations/schema.json`](../attestations/schema.json). Core fields:

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | e.g. `"1.0"` |
| `issued_at` | string (ISO-8601 UTC) | When the attestation was produced |
| `network` | string | Stellar network (`testnet` / `public`) |
| `wpi_contract_id` | string | Deployed wPi contract ID (C…) |
| `wpi_total_supply` | string (integer stroops) | On-chain `total_supply()` |
| `pi_custody_account` | string | Pi Network custody account identifier |
| `pi_custody_balance` | string (integer stroops, 7 decimals) | Reported Pi reserve |
| `pi_balance_source` | string | How the Pi balance was obtained (`file` / `env` / `api` / `manual`) |
| `safety_margin_bps` | number | Allowed under-collateralization in bps |
| `collateral_ratio` | string | Display ratio (recomputed by verifier; not signed), or `"inf"` if supply is 0 |
| `status` | string | `healthy` \| `under_collateralized` (negative quantities are rejected) |
| `attestor_public_key` | string | Hex-encoded Ed25519 public key (32 bytes) |
| `signature` | string | Hex-encoded Ed25519 signature over the **canonical payload** |

`wpi_total_supply` and `pi_custody_balance` are **non-negative** decimal integer strings only (no leading `-`).

### Canonical payload (what is signed)

UTF-8 bytes of JSON with:

- Exactly the **immutable v1** `signed_fields` list below (order fixed; verifier rejects any other list)
- No whitespace (`JSON.stringify` compact form)
- **Excludes** `signature`, `attestor_public_key`, `collateral_ratio`, and `status` from the signed body

`signed_fields` (v1, immutable order):

```
schema_version, issued_at, network, wpi_contract_id, wpi_total_supply,
pi_custody_account, pi_custody_balance, pi_balance_source, safety_margin_bps
```

## Cadence

| Mode | Interval | Notes |
|------|----------|--------|
| **Recommended** | Hourly | Cron or GitHub Actions |
| **Minimum for public demo** | On every release / manual | Still better than no signal |

Stale attestations: consumers should treat `issued_at` older than **2× cadence** (e.g. 2 hours) as **stale** and surface a warning.

## Key management

1. Generate an attestor keypair (never commit the private key):

   ```bash
   node scripts/por/attest.mjs keygen
   ```

2. Store `POR_ATTESTOR_SECRET_KEY` (hex 64-byte seed or PKCS8 PEM from the tool) in a secret manager / CI secret.
3. Publish `POR_ATTESTOR_PUBLIC_KEY` (hex) in this repo: [`attestations/ATTESTOR_PUBLIC_KEY`](../attestations/ATTESTOR_PUBLIC_KEY).
4. **Key rotation:** commit the new public key to `ATTESTOR_PUBLIC_KEY` (current key). Keep retired public keys under e.g. `attestations/keys/retired/<YYYY-MM-DD>.hex`. Historical attestations signed by an old key verify with:

   ```bash
   node scripts/por/verify.mjs attestations/history/<file>.json \
     --pubkey attestations/keys/retired/<YYYY-MM-DD>.hex
   ```

   The default verifier uses only the **current** committed public key. There is no dual-signature envelope in v1.

## Running the attestor

### Prerequisites

- Node.js 18+
- Network access to Stellar Soroban RPC (to read `total_supply`)
- Pi custody balance input (until a public Pi balance API is wired)

### Environment

| Variable | Required | Description |
|----------|----------|-------------|
| `POR_ATTESTOR_SECRET_KEY` | yes (sign) | Hex 64-char seed or path to PEM |
| `WPI_CONTRACT_ID` | yes | Stellar contract ID |
| `STELLAR_SOROBAN_RPC_URL` | yes | e.g. `https://soroban-testnet.stellar.org` |
| `STELLAR_NETWORK` | no | `testnet` (default) or `public` |
| `PI_CUSTODY_ACCOUNT` | yes | Public identifier of custody account |
| `PI_CUSTODY_BALANCE` | * | Balance in stroops (if not using file) |
| `PI_CUSTODY_BALANCE_FILE` | * | Path to a one-line integer file |
| `PI_BALANCE_SOURCE` | no | Label stored in attestation |
| `SAFETY_MARGIN_BPS` | no | Default `0` |
| `POR_OUT` | no | Output path (default `attestations/latest.json`) |

\* One of `PI_CUSTODY_BALANCE` or `PI_CUSTODY_BALANCE_FILE` is required.

### Commands

```bash
# Generate keypair (prints hex public/secret; save secret offline)
node scripts/por/attest.mjs keygen

# Produce signed attestation (writes latest.json + history snapshot)
node scripts/por/attest.mjs attest

# Verify a file against the published public key
node scripts/por/verify.mjs attestations/latest.json

# Dry-run without signing: prints JSON to stdout only (does not write latest.json
# unless POR_OUT is set explicitly)
node scripts/por/attest.mjs attest --unsigned
```

### Example cron (hourly)

```cron
0 * * * * cd /path/to/Wpi && \
  export $(grep -v '^#' .env.por | xargs) && \
  node scripts/por/attest.mjs attest && \
  node scripts/por/verify.mjs attestations/latest.json
```

### Example GitHub Action outline

1. Secret: `POR_ATTESTOR_SECRET_KEY`, `PI_CUSTODY_BALANCE` (or fetch step)
2. Schedule: `0 * * * *`
3. Steps: checkout → `node scripts/por/attest.mjs attest` → commit `attestations/latest.json` to a `por-feed` branch **or** upload as artifact / gist

Publishing **signed, production** JSON on a stable cadence is what makes the process operational for external observers. Until then, treat repo paths as a **demo feed**.

## Dashboard (public link surface)

Until a dedicated UI ships, these paths are the public surface. **Today they serve demonstration / bootstrap data** (placeholder contract id, manual balances), not production custody numbers.

| Resource | Path / URL | Notes |
|----------|------------|--------|
| Latest attestation | [`attestations/latest.json`](../attestations/latest.json) | Demo until ops cadence is configured |
| Sample | [`attestations/sample.json`](../attestations/sample.json) | Same shape; for offline demos |
| Schema | [`attestations/schema.json`](../attestations/schema.json) | |
| Attestor pubkey | [`attestations/ATTESTOR_PUBLIC_KEY`](../attestations/ATTESTOR_PUBLIC_KEY) | Current key only |
| Process docs | this file | |
| On-chain design | [`docs/design/on-chain-reserve-oracle.md`](./design/on-chain-reserve-oracle.md) | |

A minimal static page can load `latest.json` and show ratio + signature validity; not required for v1 acceptance.

## Operator checklist

- [ ] Attestor key generated offline; pubkey committed
- [ ] Cadence job running (cron or CI)
- [ ] Pi custody balance feed automated or dual-controlled manual input
- [ ] Alerts if `status != healthy` or attestation is stale
- [ ] Runbook for under-collateralization (pause mint, investigate custody)

## Limitations (honest)

- Pi custody balance is **reported**, not proven on Stellar without an oracle bridge.
- The attestor is trusted; compromise of the signing key can produce false “healthy” reports.
- Does not stop under-collateralized mints by itself — that is the medium-term oracle + mint guard.

## Acceptance mapping (Issue #25)

| Criterion | Deliverable |
|-----------|-------------|
| Off-chain attestation process live and documented | This doc + `scripts/por/*` + `attestations/*` |
| Design doc for on-chain oracle-fed invariant | [`design/on-chain-reserve-oracle.md`](./design/on-chain-reserve-oracle.md) |
