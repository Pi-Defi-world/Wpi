# Compatibility — Pi ↔ Stellar protocol skew

**Last verified:** 2026-08-19  
**Related:** [Issue #27](https://github.com/Pi-Defi-world/Wpi/issues/27)

This document records which Pi Network and Stellar protocol versions the bridge is
built and tested against, how the two chains differ in practice, how protocol
drift is detected, and what maintainers must do when either side upgrades.
It is the human-readable counterpart of the machine-checked expectations in
[`protocol-versions.json`](./protocol-versions.json).

## Why this exists

Pi Network is an SCP (Stellar Consensus Protocol) fork. Both chains ship a
shared transaction/ledger **XDR format** and StrKey address encoding, but they
**evolve independently**:

- **Stellar** upgrades through protocol voting on the public network
  (`Protocol 20` introduced Soroban; `Protocol 27` "Zipper" activated on
  mainnet on 2026-07-10).
- **Pi Network** upgrades on its own cadence and does **not** track Stellar
  protocol versions 1:1. Pi Core tags its own binaries (e.g. `v27.1.0`) that
  are *not* Stellar's `stellar-core v27.x`, and the live protocol versions on
  Pi mainnet/testnet can lag Stellar by a full major (see below).

Neither difference is fatal — the bridge only needs classic native-Pi payments
on the Pi side and Soroban `mint_from_deposit`/`burn` calls on the Stellar
side — but each major protocol bump has historically changed fee/resource
economics and (on Stellar) the Soroban auth/environment model. That is exactly
the kind of silent breakage this document and its CI check are meant to catch.

## What this bridge is built and tested against

| Layer | Pinned dependency | Notes |
|-------|-------------------|-------|
| `wpi-token` / `mock-amm` contracts | `soroban-sdk = 23.0.1` | Rust `1.88.0`, `wasm32-unknown-unknown`. Compiled against the Soroban env interface of Stellar **protocol 23**. |
| Relayer SDK | `@stellar/stellar-sdk ^12.3.0` | Node `>= 20`. Used for Soroban RPC (`contract.Client`, `rpc.Server`) and StrKey encoding. |
| Pi read/payout client | Horizon-compatible REST | No Pi SDK dependency. Talks to Pi Horizon `/`, `/accounts/{id}`, `/accounts/{id}/payments`. |

The contracts' Soroban env baseline (protocol 23) is **older** than every live
network below. Soroban's env ABI is intentionally forward-compatible, so the
deployed WASM keeps working across protocol upgrades; upgrades can still change
resource limits, event sizes, auth requirements, and fee behavior.

## As-tested network versions (live check 2026-08-19)

| Network | Horizon root | `current_protocol_version` | core version | Horizon version |
|---------|--------------|----------------------------|--------------|-----------------|
| Stellar **mainnet** | `https://horizon.stellar.org` | **27** (Zipper, live 2026-07-10) | `stellar-core 28.0.0` | `27.0.1` |
| Stellar **testnet** | `https://horizon-testnet.stellar.org` | **27** | `stellar-core 28.0.0` | `27.0.1` |
| Pi **mainnet** | `https://api.mainnet.minepi.com` | **26** | `stellar-core 26.1.0` | `26.0.0` |
| Pi **testnet** | `https://api.testnet.minepi.com` | **26** (supports 27) | `v27.1.0` (Pi's own tag) | `pi-horizon-27.0.0` |

The canonical, machine-checked source of truth is
[`protocol-versions.json`](./protocol-versions.json); the CI job in
`.github/workflows/protocol-version-check.yml` re-queries these endpoints and
fails if the live values drift from it.

## Known differences and risk areas

### Transaction / ledger format

- Both chains serialize operations in the same XDR envelope family, and a
  Pi-native payment record read from Pi Horizon parses with the same field
  layout as a Stellar Horizon payment (`from`, `to`, `amount`, `transaction_hash`,
  `paging_token`, memo).
- Pi Horizon returns the memo under `transaction.memo` with `memo_type: 'text'`
  (`join=transactions`), identical to Stellar Horizon.
- **Risk:** an XDR bump on either side (new transaction types, changed op
  result shapes) can break the relayer's payment parsing without a Rust or SDK
  recompile. The relayer treats parse failures as hard errors and does not
  silently skip records.

### Fee and resource structure

- Stellar charges a **base fee per operation** (in stroops) plus per-resource
  fees for Soroban (CPU instructions, memory, read/write bytes, events).
  Protocol 27 raised Soroban limits: CPU 100M → **150M** instructions/tx,
  read bytes 200KB → **300KB**, write bytes 65KB → **100KB**, event size
  8KB → **16KB** per tx.
- Pi Network's classic fees are Stellar-shaped (base fee in stroops); Pi has no
  public Soroban resource fee market yet.
- **Risk:** hardcoded fee values in the relayer or deployment scripts can go
  stale. Always prefer `simulateTransaction`/Horizon `fee_stats` over constants
  (see `scripts/deploy_*.sh` and the relayer's Soroban client).

### Soroban SDK / auth model

- `soroban-sdk 23.0.1` corresponds to the **protocol 23** Soroban env. Protocol
  27 (CAP-0071) introduced authentication delegation and address-bound
  credentials (`SOROBAN_CREDENTIALS_ADDRESS_V2`); contracts compiled against
  older SDK envs still execute, but new auth flows require SDK bumps.
- `@stellar/stellar-sdk` `^12.3.0` supports current protocols; the
  `basicNodeSigner` used by `SorobanWpiContractClient` must be re-verified
  against each protocol upgrade's auth envelope changes.

### Pi versioning is not Stellar versioning

- Pi testnet currently reports `core_version: v27.1.0` while `current_protocol_version`
  is still 26. **Do not** assume a Pi `vX.Y` tag maps to Stellar protocol X.
- Pi mainnet rolled to protocol 26 behind the 2026-08-11 node-operator deadline
  and is one major behind Stellar's protocol 27. The gap can widen or shrink at
  different times on Pi mainnet vs testnet, so both are monitored.

## How drift is detected

`.github/workflows/protocol-version-check.yml` runs:

- on a **schedule** (Monday weekly),
- on **push to `main`** touching `COMPATIBILITY.md` or `protocol-versions.json`,
- on **pull requests** touching those files,
- and via **`workflow_dispatch`** (manual).

For each endpoint in `protocol-versions.json` (Stellar mainnet/testnet, Pi
mainnet/testnet) the job fetches the Horizon root and:

1. compares `current_protocol_version` against the pinned expectation — **any
   mismatch fails the job**;
2. reports `core_version` and `horizon_version` for human review in the job
   summary (these change more often and are informational, not failing).

A failed run means a live network has moved ahead of (or behind) what this
bridge is pinned to, and the [update policy](#update-policy) below kicks in.

## Update policy

1. **A protocol upgrade on either chain** does not automatically break this
   repo; **drift is flagged by CI** before it matters. When the check fails,
   someone must (a) read the chain's release notes, (b) test against the
   affected testnet, and (c) decide whether the bridge needs a change.
2. **Contract SDK bumps** (`soroban-sdk`) are made deliberately: bump the
   workspace `Cargo.toml`, re-run `cargo test` + the WASM-size CI, and re-test
   against **both** Stellar testnet and Pi testnet. Record the new SDK in
   [Stellar-contracts-v1/README.md](./Stellar-contracts-v1/README.md).
3. **Relayer SDK bumps** (`@stellar/stellar-sdk`, Node) follow the same rule:
   bump, run typecheck/lint/tests, verify against Stellar testnet.
4. **Every accepted bump updates all three artifacts in one PR:**
   - `protocol-versions.json` (new expected values / URLs),
   - `COMPATIBILITY.md` (the table, "last verified" date, and any new
     risk-area notes),
   - `CHANGELOG.md` (an `Unreleased` entry).

   PRs that change dependency versions or deployment scripts but leave
   `protocol-versions.json` stale will fail the drift check.
5. The "last verified" date at the top of this file should be refreshed on the
   same PR, using the live values shown by the CI run.

## Exceptions

An operator may temporarily accept known drift in dry-run or testnet-only
deployments, but **production minting must not bypass the policy**: if the
drift check is failing, do not run the relayer against mainnet until the
affected change is understood and this file is updated.