# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Contracts (`wpi-token`)**: Configurable per-transaction mint ceiling
  (`max_mint_per_tx`), enforced on `mint` and `mint_from_deposit` and owned by
  the volume-limit admin. An over-cap mint is rejected without state changes
  and publishes `MintTxCapExceeded`. Mints fail closed with
  `MintTxCapNotConfigured` until the ceiling is configured ([#19](https://github.com/Pi-Defi-world/Wpi/issues/19)).

### Docs
- **Compatibility**: Added `COMPATIBILITY.md` documenting the pinned Pi Core /
  Stellar protocol versions the bridge is built and tested against (Pi mainnet
  Protocol 26, Stellar mainnet Protocol 27 "Zipper" at time of writing), how
  drift is detected, and the update policy for SDK/protocol bumps. Added a
  scheduled CI drift check (`protocol-version-check.yml`) comparing the live
  networks against `protocol-versions.json` ([#27](https://github.com/Pi-Defi-world/Wpi/issues/27)).
- **Eligibility policy**: Added `docs/deposit-eligibility.md` stating that only
  migrated, KYC-verified Pi mainnet accounts may originate a deposit, and
  implemented fail-closed enforcement in the relayer (`DepositEligibilityPolicy`
  + `DepositWatcher`): ineligible sources are recorded with status `ineligible`
  and are never minted ([#28](https://github.com/Pi-Defi-world/Wpi/issues/28)).
- **Contributing**: Added `CONTRIBUTING.md`, issue/PR templates, and
  `CODEOWNERS` for the contract crates ([#30](https://github.com/Pi-Defi-world/Wpi/issues/30)).

## [0.1.0] - 2026-07-28

### Added
- **Contracts (`wpi-token`)**: Initial implementation of the Wrapped Pi (`wPi`) token contract on Stellar with admin-gated mint/burn and rolling volume-limit circuit breaker.
- **Contracts (`mock-amm`)**: Test AMM contract simulating wPi swaps against the real USDC Stellar Asset Contract (SAC).
- **Contracts (`soroban-token-common`)**: Shared balance, allowance, admin, and pause scaffolding.
- **Proof of Reserve (PoR)**: Tooling (`scripts/por/`) and schema (`attestations/schema.json`) for off-chain reserve attestation and signature verification.
- **Operations & Build**: Checked-in shell scripts for testnet and mainnet deployments (`deploy_testnet.sh`, `deploy_mainnet.sh`).
- **CI/CD**: Fully integrated workflows for cargo clippy, testing, dependency provenance verification, and WASM size regression tracking.
