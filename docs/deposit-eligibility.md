# Deposit-Source Eligibility Policy for the wPi Bridge

**Status:** Enforced by the relayer from this PR onward (v1). Policy text is the
authoritative statement; the relayer's `EligibilityPolicy` implements it.
**Related:** [Issue #28](https://github.com/Pi-Defi-world/Wpi/issues/28), [Issue #7](https://github.com/Pi-Defi-world/Wpi/issues/7)
**Implementation:** `relayer/src/pi/eligibility.ts`, `relayer/src/pi/depositWatcher.ts`

## Purpose

Pi Network's mainnet rollout is graduated: not every Pi wallet is migrated
(on-chain), and not every Pioneer has completed KYC. Before this policy, the
relayer had **no defined rule** for which Pi source accounts may originate a
bridge deposit — it would mint wPi against *any* observed payment to the
deposit address. This document fixes that by stating an explicit, fail-closed
eligibility policy and making the relayer enforce it before it ever calls
`mint_from_deposit`.

## Policy statement

> **Only migrated, KYC-verified Pi mainnet accounts may originate a bridge
> deposit.** The relayer must reject (and must never mint against) any deposit
> whose source account fails the checks below.

"Eligible to deposit" means **all** of:

1. **Migrated (on-chain).** The source account exists on the Pi mainnet
   ledger — it is a real ledger account that sent the payment — not a
   pre-migration app balance, a testnet/pilot address, or a placeholder.
2. **Real on-chain payment.** The payment is a native-Pi `payment` operation
   to the bridge deposit address with `transaction_successful = true` and a
   valid destination Stellar address in its memo (the memo rule already
   enforced by `DepositWatcher`).
3. **KYC-verified (operator-attested).** The account has passed the operator's
   KYC/AML review. KYC is **off-chain**; there is no on-chain signal, so
   verification is expressed as an **allowlist** of addresses the operator has
   approved. When an allowlist is configured, **only** allowlisted accounts are
   eligible.
4. **Not blocklisted.** The account is not on the operator's blocklist
   (frozen accounts, sanctioned/flagged addresses, previously-abusive
   depositors).

Anything else is **ineligible** and must be recorded with a machine-readable
reason and never minted.

## Source-account states and their eligibility

| Pi account state | On-chain? | Eligible? | Reason |
|------------------|-----------|-----------|--------|
| Migrated mainnet account, KYC passed, not blocked | Yes | ✅ Yes | Standard eligible depositor |
| Migrated mainnet account, KYC passed, but on blocklist | Yes | ❌ No | `blocklisted` |
| Migrated mainnet account, not yet KYC-approved | Yes | ❌ No | `not_kyc_verified` (fails allowlist) |
| Non-migrated wallet (mining balance not on-chain) | No | ❌ No | `account_not_found` (no ledger account) |
| Testnet / pilot / placeholder address | No* | ❌ No | `account_not_found` or `not_allowlisted` |
| Address that does not resolve on Pi mainnet Horizon | No | ❌ No | `account_not_found` |

\* Testnet addresses may exist on Pi testnet's ledger; the policy only ever
talks to the configured Pi Horizon base URL, which defaults to **mainnet** in
production. A deposit whose source only exists on testnet therefore fails at
the chain lookup.

## How the relayer enforces it

1. `HorizonPiClient.getAccountEligibility(accountId)` resolves the account
   against the configured Pi Horizon `/accounts/{id}` endpoint. A `404` (or any
   non-account response) ⇒ `eligible: false, reason: 'account_not_found'`.
2. `DepositEligibilityPolicy.check({ from })` (`relayer/src/pi/eligibility.ts`)
   combines that chain lookup with the operator's allowlist/blocklist:
   - Policy disabled ⇒ every account passes (explicit operator opt-out).
   - Allowlist configured and account not present ⇒ `not_kyc_verified` /
     `not_allowlisted`.
   - Account on the blocklist ⇒ `blocklisted`.
   - Otherwise ⇒ the Horizon chain-lookup result.
3. `DepositWatcher` runs the policy at **ingest time** (before confirmation
   depth is tracked). An ineligible source is recorded with status
   `ineligible` and a reason; it is **never** promoted to `confirmed`, so the
   `MintSubmitter` never sees it and `mint_from_deposit` is never called for
   it.
4. The check is **fail-closed by default**: `PI_ELIGIBILITY_ENABLED` defaults
   to `true`. If the policy cannot reach the Pi Horizon endpoint at ingest
   time, the deposit is recorded as `ineligible` (`eligibility_check_failed`)
   rather than silently allowed — an operator must intervene.

## Configuration

All variables live under the `pi` section of `RelayerConfig` (see
`relayer/src/config.ts` and `relayer/.env.example`):

| Env var | Default | Meaning |
|---------|---------|---------|
| `PI_ELIGIBILITY_ENABLED` | `true` | Set `false` to disable the check entirely (not recommended; dry-run testnet demos only) |
| `PI_ELIGIBILITY_ALLOWLIST` | *(empty)* | Comma-separated `G...` addresses approved after KYC. When set, only these may deposit. |
| `PI_ELIGIBILITY_BLOCKLIST` | *(empty)* | Comma-separated `G...` addresses that are never eligible. |

`PI_HORIZON_URL` selects which chain the account lookup hits. **Production must
point at Pi mainnet** (`https://api.mainnet.minepi.com`).

## Operational notes

- **Adding an address:** KYC-approve the Pioneer's Pi address, then add it to
  `PI_ELIGIBILITY_ALLOWLIST` and redeploy/restart the relayer. Prefer env-based
  config over editing the JSON state store.
- **Removing an address:** immediately add it to `PI_ELIGIBILITY_BLOCKLIST`;
  the blocklist takes precedence over the allowlist.
- **Audit trail:** every deposit (eligible or not) is written to the
  `IdempotencyStore` with its status; ineligible deposits carry their reason in
  `ineligibleReason`. Operators can query the store for
  `status = 'ineligible'` to review rejections.
- **Failure behavior:** the relayer logs a warning per rejected deposit and
  continues. It does not stall the polling loop, and it never retries an
  `ineligible` deposit into a confirmed state.
- **Redemptions are out of scope for this policy.** Eligibility governs
  deposits (Pi → wPi). The redemption path (wPi → Pi) is handled by the burn
  watcher and payout client.

## Explicitly out of scope (v1)

- Automatic on-chain KYC or migration-status attestation: neither exists on Pi
  Network today, so KYC is operator-attested via the allowlist.
- Per-account deposit limits / velocity checks: tracked separately
  ([#26](https://github.com/Pi-Defi-world/Wpi/issues/26)).
- Jurisdictional blocking beyond the blocklist.