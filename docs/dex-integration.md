# DEX integration — wPi/USDC on Stellar

**Last verified:** 2026-08-26
**Related:** [Issue #10](https://github.com/Pi-Defi-world/Wpi/issues/10)
**Scope:** Stellar **testnet** only. Mainnet pool creation is explicitly out of scope
here and is gated off in tooling (see [Safety model](#safety-model)).

This document replaces the previous "DEX pool creation is not included; seed
liquidity off-chain" note in
[`Stellar-contracts-v1/README.md`](../Stellar-contracts-v1/README.md). It records:

1. which AMM the bridge integrates against and **why**,
2. a reproducible, scripted **pool-creation** flow,
3. a reproducible, scripted **initial-liquidity-seeding** flow,
4. a liquidity-seeding **runbook** for the treasury/relayer operator,
5. **price-impact and slippage** guidance for that operator,
6. the alternatives that were evaluated and rejected.

Everything below is driven by [`scripts/seed_testnet_liquidity.sh`](../scripts/seed_testnet_liquidity.sh)
and configured through [`scripts/dex.testnet.env.example`](../scripts/dex.testnet.env.example).

---

## 1. AMM choice: Soroswap

**The bridge integrates `wPi/USDC` liquidity against [Soroswap](https://soroswap.finance)
on Stellar testnet.**

Soroswap is a Uniswap-v2-style constant-product AMM implemented as Soroban
contracts (`SoroswapFactory`, `SoroswapRouter`, `SoroswapPair`). The integration
touches only two of its entrypoints:

| Call | Contract | Purpose in this repo |
|------|----------|----------------------|
| `add_liquidity(token_a, token_b, amount_a_desired, amount_b_desired, amount_a_min, amount_b_min, to, deadline)` | Router | Create the `wPi/USDC` pair (auto-created on first call) **and** seed it |
| `router_pair_for(token_a, token_b)` / factory `get_pair` | Router / Factory | Resolve the deployed pair address to read reserves back |

`swap_exact_tokens_for_tokens(...)` on the Router is the path a downstream
integrator (or the treasury, when rebalancing) uses; the bridge itself never
swaps.

### Why Soroswap

- **It is a Soroban-native AMM, not a Stellar-classic (SDEX) path.** wPi is a
  Soroban token contract (`wpi-token`), not a classic Stellar asset with a
  trustline. Classic-DEX order books and path payments require an issued asset;
  Soroswap operates directly on Soroban token-interface contracts, so wPi works
  with **no SAC wrapping and no asset issuance**.
- **Authorization model matches `wpi-token`.** The Soroswap Router moves funds
  with `token.transfer(from = user, to = pair, amount)` guarded by
  `from.require_auth()`, satisfied transitively through Soroban's authorization
  tree from the single transaction signature. It does **not** rely on
  ERC-20-style `approve` + `transfer_from`. This matters because `wpi-token`'s
  `approve` is intentionally minimal (`approve(owner, spender, amount)` — no
  `expiration_ledger`) and is **not** SEP-41-complete. An allowance-based Router
  (see [Alternatives](#7-alternatives-evaluated)) would require changing the
  token; Soroswap does not. See [Token compatibility](#token-compatibility).
- **7-decimal alignment.** `wpi-token` and `mock-usdc` both use 7 decimals
  (`DECIMALS = 7`), matching Stellar's stroop convention and the real USDC SAC.
  Constant-product math and the price ratio below are computed in whole stroops
  with no decimal rescaling.
- **Public testnet deployment with published contract IDs.** Soroswap publishes
  Router/Factory IDs per network, so the flow is reproducible from a config file
  rather than requiring us to also deploy and maintain an AMM.
- **Continuity with existing repo intent.** `Stellar-contracts-v1/README.md`
  already named "Soroswap or another Stellar AMM" as the seeding target, and
  `mock-amm` exists specifically to simulate "wPi swaps against the real USDC
  Stellar Asset Contract (SAC)". Soroswap is the production-shaped version of
  that simulation.

### Token compatibility

`add_liquidity` and `swap_*` work against `wpi-token` **today** because Soroswap
uses `require_auth`-based transfers. The following `wpi-token` gaps are **not**
blockers for this flow but are recorded so a future maintainer does not trip on
them:

- `approve` has no `expiration_ledger` parameter, so `wpi-token` is not a drop-in
  for any Soroban protocol that calls `transfer_from` after `approve`.
- `wpi-token` has no `mint`-to-arbitrary beyond the admin; seeding liquidity
  therefore requires the **admin/treasury** account to hold the wPi it deposits
  (in production that wPi is backed 1:1 by bridged Pi — see
  [`docs/proof-of-reserve.md`](./proof-of-reserve.md)).

### USDC on testnet

There is no canonical circle-issued USDC on Stellar **testnet** as a Soroban
token contract. The seeding flow uses **`mock-usdc`** (`Stellar-contracts-v1/mock-usdc`,
7 decimals, admin-mint) as the `USDC` leg on testnet. On mainnet the `USDC` leg
would be the real USDC SAC contract ID — but mainnet pool creation is out of
scope for this document and blocked in tooling.

---

## 2. Scripted pool-creation flow

The pair is created implicitly by the **first** `add_liquidity` call: if
`factory.get_pair(wPi, USDC)` does not exist, the Router calls
`factory.create_pair` before depositing. There is no separate "create pool"
transaction to run and no separate step to get wrong.

[`scripts/seed_testnet_liquidity.sh`](../scripts/seed_testnet_liquidity.sh) does,
in order:

1. Resolve the Stellar CLI identity and network (testnet by default).
2. Verify the configured `wPi` and `USDC` contract IDs resolve on-network.
3. `--amm soroswap` (default): call `SoroswapRouter.add_liquidity(...)` with the
   treasury as `to`. Pair auto-creates on first run.
   `--amm mock`: deploy/reuse `mock-amm`, `initialize` it for the `wPi→USDC`
   direction, and call `deposit_liquidity` (USDC-only, 1:1 sim). This path needs
   no external AMM and is what CI-style local demos use.
4. Resolve the pair address (`router_pair_for` for Soroswap) and print the
   on-chain reserves and implied price.

Run it with `--dry-run` first to print every `stellar contract invoke` it would
submit, with no signing.

---

## 3. Scripted initial-liquidity-seeding flow

Initial seeding **is** the first `add_liquidity` call — for a constant-product
pool the first liquidity provider sets the price by choosing the ratio of the two
deposits:

```
price(USDC per wPi) = amount_usdc_desired / amount_wpi_desired
```

Both amounts are expressed in **whole stroops** (1 unit = 1e7). Configure them in
the env file:

| Var | Meaning | Example (stroops) | Human |
|-----|---------|-------------------|-------|
| `SEED_WPI_AMOUNT` | wPi deposited | `1000000000000` | 100,000 wPi |
| `SEED_USDC_AMOUNT` | USDC deposited | `314000000000` | 31,400 USDC |
| `SEED_SLIPPAGE_BPS` | tolerance on both `*_min` | `50` | 0.50% |

(Plain digits only — the script evaluates these with shell arithmetic.)

The script derives:

```
amount_wpi_min  = SEED_WPI_AMOUNT  * (10000 - SEED_SLIPPAGE_BPS) / 10000
amount_usdc_min = SEED_USDC_AMOUNT * (10000 - SEED_SLIPPAGE_BPS) / 10000
deadline        = now + SEED_DEADLINE_SECS   (default 300)
```

For the **very first** deposit into a fresh pair, `amount_*_min` are effectively
ignored by the Router (no reserves exist to skew against), but they are still
sent so re-runs (top-ups) are protected.

### Prerequisites the script checks / needs

- Stellar CLI (`stellar`, aka soroban-cli) on `PATH`, aligned with
  `soroban-sdk 23.x` — same requirement as the deploy scripts.
- A funded testnet identity (`STELLAR_ACCOUNT`); the script funds a fresh one via
  Friendbot exactly like `scripts/deploy_testnet.sh`.
- The treasury identity must already hold:
  - `SEED_WPI_AMOUNT` wPi — minted by the bridge admin via `wpi-token.mint`
    (the script will do this if `SEED_MINT_WPI=true` **and** the identity is the
    wPi admin), and
  - `SEED_USDC_AMOUNT` mock USDC — the script mints this via `mock-usdc.mint`
    when `SEED_MINT_USDC=true` (default on testnet).
- `SOROSWAP_ROUTER_ID` (and `SOROSWAP_FACTORY_ID` for reserve read-back) set to
  the current testnet deployment. **These are not committed** — they change when
  Soroswap redeploys. Fetch the current values from Soroswap's published
  testnet address list and put them in your local `scripts/dex.testnet.env`.

---

## 4. Liquidity-seeding runbook (treasury/relayer operator)

> Do this on **testnet**. Every command below refuses to run against mainnet
> unless `I_UNDERSTAND_MAINNET_DEX=yes` is explicitly set, and even then the
> script only prints a plan — it never submits a mainnet pool transaction.

### Step 0 — deploy the tokens

```bash
make deploy-testnet
# exports WPI_CONTRACT_ID (wpi-token) and MOCK_AMM_CONTRACT_ID
```

`mock-usdc` is not deployed by `deploy-testnet`. The seeding script deploys and
initializes it on first run (or reuses `MOCK_USDC_CONTRACT_ID` if you set it).

### Step 1 — configure

```bash
cd scripts
cp dex.testnet.env.example dex.testnet.env
# edit dex.testnet.env:
#   STELLAR_ACCOUNT        = your funded testnet identity (also the token admin)
#   WPI_CONTRACT_ID        = from step 0
#   MOCK_USDC_CONTRACT_ID  = leave blank to auto-deploy
#   SOROSWAP_ROUTER_ID     = current Soroswap testnet Router id
#   SOROSWAP_FACTORY_ID    = current Soroswap testnet Factory id
#   SEED_WPI_AMOUNT / SEED_USDC_AMOUNT = your target opening price ratio
#   SEED_SLIPPAGE_BPS     = 50
```

### Step 2 — dry run

```bash
set -a; source dex.testnet.env; set +a
../scripts/seed_testnet_liquidity.sh --dry-run
```

Read every printed invocation. Confirm the token IDs, the `to` address (your
treasury), the amounts, and the derived `*_min` / `deadline`.

### Step 3 — seed

```bash
../scripts/seed_testnet_liquidity.sh
```

Expected tail of output:

```
== Resolve pair + reserves ==
pair:            C...                 (SoroswapPair for wPi/USDC)
reserves:        [1000000000000, 314000000000]   (order = pair token0/token1)
verify:          stellar contract invoke --id C<factory> ... -- get_pair ...

== Pool seeded ==
```

`reserves` are raw stroops in the pair's own token ordering; divide by 1e7 for
human units (here 100,000 wPi / 31,400 USDC → 0.314 USDC per wPi).

### Step 4 — verify independently

```bash
stellar contract invoke --id "$SOROSWAP_FACTORY_ID" --network testnet \
  --source-account "$STELLAR_ACCOUNT" -- get_pair \
  --token_a "$WPI_CONTRACT_ID" --token_b "$MOCK_USDC_CONTRACT_ID"

stellar contract invoke --id <pair-id> --network testnet \
  --source-account "$STELLAR_ACCOUNT" -- get_reserves
```

Reserves must equal what you deposited (first LP). Record the pair id in your
deployment notes / `CHANGELOG.md` if this is a shared testnet environment.

### Step 5 — top up later (optional)

Re-run `seed_testnet_liquidity.sh` with new `SEED_*_AMOUNT` values **in the
current pool ratio**. If the ratio differs from the live reserves, the Router
consumes only the matching portion and refunds the rest; `SEED_SLIPPAGE_BPS`
bounds how far off it may be before the call reverts.

### Rollback

Withdraw via the Router:

```bash
stellar contract invoke --id "$SOROSWAP_ROUTER_ID" --network testnet \
  --source-account "$STELLAR_ACCOUNT" -- remove_liquidity \
  --token_a "$WPI_CONTRACT_ID" --token_b "$MOCK_USDC_CONTRACT_ID" \
  --liquidity <lp-amount> --amount_a_min 0 --amount_b_min 0 \
  --to "$(stellar keys address "$STELLAR_ACCOUNT")" --deadline <ts>
```

On testnet with the mock USDC this has no real value at risk; it exists so the
runbook is complete.

---

## 5. Price-impact and slippage guidance

The pool is constant-product: `x * y = k`, 0.30% swap fee (Soroswap default).

### Opening price

The first deposit sets `k` and the price. Pick `SEED_WPI_AMOUNT` /
`SEED_USDC_AMOUNT` so their ratio is the price you want to open at. There is no
oracle — a wrong ratio is an immediate arbitrage giveaway, so on a **shared**
testnet keep the opening deposit small enough that a mispricing is cheap to
correct, or coordinate the ratio with whoever consumes the pool.

### Price impact of a swap

For a swap of `dx` into a reserve of `x` (ignoring the fee for the estimate):

```
price impact ≈ dx / (x + dx)
```

Practical reference for a pool seeded with **100,000 wPi**:

| Swap size (wPi) | ≈ price impact | Notes |
|-----------------|----------------|-------|
| 100 | 0.10% | negligible |
| 1,000 | 0.99% | fine for routine treasury moves |
| 5,000 | 4.76% | split into smaller clips |
| 10,000 | 9.09% | do not do in one tx |

Impact scales with **swap size relative to the reserve**, so the mitigation is
either a bigger pool or smaller clips — deeper liquidity is the only structural
fix.

### Slippage tolerance for the operator

- **Seeding / top-ups:** `SEED_SLIPPAGE_BPS = 50` (0.5%) is the default and is
  generous for a low-traffic testnet pool. Tighten to `10`–`25` once the pool
  has steady reserves.
- **Swaps (treasury rebalancing):** set `amount_out_min` from a fresh
  `router.get_amount_out` (or `get_amounts_out`) simulation, then subtract your
  tolerance:
  `amount_out_min = expected_out * (10000 - slippage_bps) / 10000`.
  Use `25`–`50` bps on a quiet pool; raise it only if calls start reverting
  because someone else is trading the same block.
- **Deadline:** keep `SEED_DEADLINE_SECS` short (300s default). A long deadline
  is a free option for a miner/arbitrageur if the tx sits in the mempool.
- **Never pass `amount_*_min = 0` for a real value transfer.** The script only
  does that for the mock-AMM 1:1 sim and for the documented `remove_liquidity`
  rollback on testnet.

### What the bridge relayer must **not** do

The relayer mints and burns wPi against observed Pi deposits/redemptions. It has
no reason to touch the AMM and must not hold an LP position or route swaps —
liquidity operations are a **treasury** function performed with the runbook
above, out-of-band from the mint/burn pipeline.

---

## 6. Safety model

- **Testnet is the default and only supported target.** `STELLAR_NETWORK`
  defaults to `testnet`; `RPC_URL` defaults to
  `https://soroban-testnet.stellar.org`.
- **Mainnet is refused.** If `STELLAR_NETWORK=mainnet` (or the passphrase is the
  public one) the script exits non-zero unless `I_UNDERSTAND_MAINNET_DEX=yes` is
  set, and even then it runs in forced `--dry-run` and prints a plan only — it
  will not submit a mainnet transaction. Real mainnet pool creation is a
  deliberate, reviewed, multi-sig treasury action and is not automated in this
  repo.
- **`mock-amm` cannot be initialized on mainnet** — the contract itself rejects
  the mainnet network id (`Error::MainnetNotSupported`).
- **No secrets committed.** `scripts/dex.testnet.env` is git-ignored; only
  `dex.testnet.env.example` is tracked. Soroswap contract IDs live in your local
  env file, not in the repo, because they change on redeploy.
- **Unit tests need no network.** Nothing in CI calls this script or a live
  network. It is an operator tool, run by hand against testnet.

---

## 7. Alternatives evaluated

| AMM | Why not (for this integration) |
|-----|-------------------------------|
| **Stellar classic DEX / path payments** | Requires wPi to be an issued classic asset with trustlines. wPi is a Soroban token contract; issuing a parallel classic asset doubles the surface and the reserve-accounting story. Rejected. |
| **Aquarius (AMM + rewards)** | Soroban AMM focused on SDEX-market-maker incentives and gauge voting. Heavier surface than the bridge needs; its value is liquidity mining, which is irrelevant for a testnet `wPi/USDC` pair. Kept as a future option if wPi ever needs incentivized mainnet liquidity. |
| **Phoenix** | Soroban DEX with a stableswap-style curve and its own UI/SDK. Viable technically, but its interface and testnet addresses move faster, and the constant-product model is the right fit for a volatile `wPi/USDC` pair, not a stable-pair curve. |
| **Comet (Balancer-style weighted pools)** | Weighted/multi-asset pools are more than a single 50/50 `wPi/USDC` pair requires. Extra config (weights, swap-fee governance) with no benefit here. |
| **Soroswap** | **Chosen.** Constant-product 50/50, `require_auth`-based transfers (works with `wpi-token` as-is), Soroban-native, published per-network Router/Factory IDs, and already the AMM this repo's docs and `mock-amm` were written against. |

If the chosen AMM's testnet deployment or interface has changed since the
"last verified" date above, re-check its current published address list and
entrypoint signatures before running the flow, and update this file plus
`scripts/dex.testnet.env.example` in the same PR.
