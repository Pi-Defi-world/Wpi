# End-to-end demo: real Pi testnet deposit → wPi mint

This documents how to run the full bridge flow — a real Pi Network testnet
deposit observed, confirmed, and minted as wPi on Stellar testnet — with the
production relayer. It also covers the redemption leg (burn → Pi release).

For a demo that runs immediately, without testnet accounts or waiting on
real confirmations, run `npm run demo:e2e` instead (see
[README.md](../README.md#demo-scripted-e2e-no-testnet-required)). That
script drives the same `DepositWatcher` / `MintSubmitter` /
`RedemptionWatcher` classes used here, against in-process fakes for the two
network edges. This document is the live-network counterpart.

## Prerequisites

1. A Pi Network testnet account funded with test-Pi, and its Horizon-compatible
   RPC URL and network passphrase (obtained via Pi Network's developer program —
   Pi's testnet is invite-gated, so this step happens outside this repo).
2. A Stellar testnet account funded via
   [Friendbot](https://developers.stellar.org/docs/tutorials/create-account),
   used as the wPi contract admin.
3. `wpi-token` deployed to Stellar testnet and initialized with that admin
   (see [Stellar-contracts-v1/README.md](../../Stellar-contracts-v1/README.md#deploy-stellar-testnet)).
4. A second Stellar testnet account to receive the minted wPi (the
   "destination" address below).

## 1. Configure the relayer

```bash
cd relayer
cp .env.example .env
```

Fill in `.env`:

| Var | Value |
|---|---|
| `PI_HORIZON_URL` | Pi Network testnet Horizon URL |
| `PI_NETWORK_PASSPHRASE` | Pi Network testnet passphrase |
| `PI_BRIDGE_DEPOSIT_ADDRESS` | The Pi account the bridge watches for deposits |
| `PI_BRIDGE_CUSTODIAN_SECRET_KEY` | Secret key for that same account (signs redemption releases) |
| `STELLAR_SOROBAN_RPC_URL` | `https://soroban-testnet.stellar.org` |
| `STELLAR_NETWORK_PASSPHRASE` | `Test SDF Network ; September 2015` |
| `WPI_CONTRACT_ID` | The deployed `wpi-token` contract id |
| `BRIDGE_STELLAR_ADMIN_SECRET_KEY` | The contract admin's secret key |
| `PI_CONFIRMATION_DEPTH` | Leave at the default (30) or lower for a faster demo |

## 2. Start the relayer

```bash
npm install
npm run build
npm start
```

Watch the logs for `relayer starting` followed by periodic poll cycles.

## 3. Send the test deposit

From the Pi testnet wallet, send a native-Pi payment to
`PI_BRIDGE_DEPOSIT_ADDRESS`, with the **destination Stellar address** (the
wPi recipient) as the transaction's **text memo**. This memo convention is
how the relayer maps a Pi deposit to a Stellar recipient — see
[README.md](../README.md#deposit-routing-the-memo-convention).

## 4. Observe the pipeline

- Within one `PI_POLL_INTERVAL_MS` cycle, the log shows `observed new Pi
  deposit, awaiting confirmations` with the derived `depositId`.
- Once `PI_CONFIRMATION_DEPTH` Pi ledgers have closed on top of the deposit,
  the log shows `deposit reached confirmation depth`.
- The relayer then submits `mint_from_deposit`; look for `minted wPi for Pi
  deposit` with a Stellar transaction hash.
- Confirm on-chain: query the contract's `balance` for the destination
  address (e.g. via `soroban contract invoke ... -- balance --owner
  <destination>`) and check it increased by the deposited amount.

## 5. Redemption leg (optional)

From the destination Stellar account, call the contract's `burn` with the
amount to redeem and a `pi_destination` (the raw 32-byte Pi account id to
receive the payout — see the contract's `burn` doc comment for the
StrKey/raw-bytes relationship). Within one `STELLAR_POLL_INTERVAL_MS` cycle
the relayer logs `observed wPi redemption burn`, then `released Pi for wPi
redemption` with the Pi payout transaction id.

## Notes on repeatability

Re-running step 3 with the same Pi transaction produces the same
`depositId` (`sha256` of the Pi tx hash) — the contract's
`mint_from_deposit` rejects a repeat with `DepositAlreadyProcessed`, and the
relayer's mint submitter treats that as success rather than retrying
forever. This is the same idempotency path exercised in
`Stellar-contracts-v1/wpi-token`'s `mint_from_deposit_is_idempotent_on_retry`
test and the scripted demo's step 4.
