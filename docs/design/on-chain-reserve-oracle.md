# Design: On-chain reserve oracle & mint invariant

**Status:** Design only (implementation = follow-up issue)  
**Parent:** [Issue #25](https://github.com/Pi-Defi-world/Wpi/issues/25)  
**Depends on short-term:** [Proof of Reserve process](../proof-of-reserve.md)

## Goal

Ensure that **no mint** can push `wpi-token::total_supply()` above the **attested Pi reserve** (plus an explicit safety margin), using on-chain state that anyone can audit.

## Non-goals (this design)

- Trustless verification of Pi Network balances without any oracle/attestor
- Replacing the bridge relayer
- Full multi-oracle consensus network (v1 can start with a single privileged feeder)

## Current architecture

```
Pi Network (custody)  --relayer observes-->  Stellar admin mints wPi
                                              wpi-token.total_supply()
```

Today there is **no** link between custody balance and mint eligibility.

## Target architecture

```
Pi custody balance
        |
        v
  [Off-chain attestors / feeders]  --signed update-->  reserve_oracle (Soroban)
                                                              |
                                                              | get_reserve()
                                                              v
                                                     wpi-token.mint()
                                                     checks:
                                                       total_supply + amount
                                                         <= reserve * (1 - margin)
                                                         and reserve is fresh
```

## Contracts

### 1. `reserve_oracle` (new)

Minimal interface:

```rust
// Admin / feeder management
fn initialize(admin: Address);
fn set_feeder(admin: Address, feeder: Address, allowed: bool);
fn set_max_age(admin: Address, max_age_seconds: u64);
fn set_safety_margin_bps(admin: Address, bps: u32);

// Feeder writes attested reserve (stroops, 7 decimals to match wPi)
fn update_reserve(
    feeder: Address,
    reserve_balance: i128,
    observed_at: u64,   // wall-clock or ledger time of observation
);

// Public reads
fn get_reserve() -> ReserveSnapshot;
fn is_fresh() -> bool;

struct ReserveSnapshot {
    balance: i128,
    updated_at: u64,      // ledger timestamp when written
    observed_at: u64,
    feeder: Address,
}
```

**Storage (instance):**

| Key | Value |
|-----|--------|
| Admin | Address |
| Feeder(Address) | bool |
| MaxAge | u64 |
| SafetyMarginBps | u32 |
| Snapshot | ReserveSnapshot |

**Events:**

- `reserve_updated(feeder, balance, observed_at, updated_at)`
- `feeder_changed(feeder, allowed)`
- `oracle_config_updated(param, old, new)`

### 2. Changes to `wpi-token`

Add optional oracle binding (upgrade or new initialize param):

```rust
fn set_reserve_oracle(admin: Address, oracle: Option<Address>);

fn mint(admin: Address, to: Address, amount: i128) -> Result<(), Error> {
    // existing admin auth...
    if let Some(oracle) = read_oracle(&env) {
        let snap = oracle_client.get_reserve();
        require_fresh(&env, &snap);
        let margin_bps = oracle_client.safety_margin_bps(); // or stored on token
        let cap = apply_margin(snap.balance, margin_bps);
        let new_supply = total + amount;
        if new_supply > cap {
            return Err(Error::ExceedsReserve);
        }
    }
    // existing mint accounting...
}
```

**New errors:** `ExceedsReserve`, `OracleStale`, `OracleNotSet` (policy choice: fail closed if oracle required in production).

### Freshness rule

```
fresh <=> env.ledger().timestamp() - snap.updated_at <= max_age_seconds
```

Recommended defaults:

| Network | `max_age_seconds` | Rationale |
|---------|-------------------|-----------|
| Testnet | 6 hours | Dev flexibility |
| Mainnet | 2 hours | Align with hourly off-chain cadence |

If stale: **mint reverts**; burns and transfers still work (exit liquidity).

### Safety margin

```
cap = reserve_balance * (10_000 - safety_margin_bps) / 10_000
```

Use checked i128 math. Example: `safety_margin_bps = 100` → 1% haircut for transfer lag.

## Trust model

| Actor | Trust |
|-------|--------|
| Feeder(s) | Can overstate reserve → allow excess mint. Mitigate with multi-sig feeder, multi-feeder median, or rate limits (see Issue #26). |
| Admin | Can change feeder / max age / disable oracle. Prefer multi-sig admin + timelock in production. |
| Relayer | Still required to observe Pi deposits; mint still admin-gated. |

**Honest under-collateralization detection:** Off-chain PoR + public oracle reads remain useful even after on-chain guards (detect feeder lies vs real custody).

## Upgrade / rollout plan

1. **Phase 0 (done):** Off-chain signed attestations ([proof-of-reserve.md](../proof-of-reserve.md)).
2. **Phase 1:** Deploy `reserve_oracle` on testnet; feeder cron posts same numbers as PoR JSON.
3. **Phase 2:** Deploy upgraded `wpi-token` with `set_reserve_oracle`; enable check on testnet; soak.
4. **Phase 3:** Mainnet — oracle required for mint (`OracleNotSet` fails closed); document ops runbook.
5. **Phase 4 (optional):** Multi-feeder median / stake-slash; bridge-wide daily mint caps (Issue #26).

## Failure modes

| Failure | Behavior | Ops response |
|---------|----------|--------------|
| Feeder offline | Mint freezes when snapshot ages out | Investigate; manual `update_reserve` from backup feeder |
| Feeder overstates reserve | Mint can over-issue until PoR / audit | Pause token; rotate feeder; reconcile |
| Oracle set to wrong contract | Mint uses wrong data | Admin only; use multi-sig |
| Clock skew on `observed_at` | Prefer ledger `updated_at` for freshness | Document that `observed_at` is metadata |

## Testing plan (implementation issue)

- Unit: margin math, overflow, zero supply, amount edge cases  
- Unit: stale snapshot rejects mint; fresh accepts  
- Integration: feeder update → mint under/over cap  
- Invariant property: after any successful mint, `total_supply <= cap(reserve)`  
- Negative: unauthorized feeder, unauthorized admin  

## Open questions for maintainers

1. Should production **require** an oracle (fail closed) or allow mint without one during bootstrap?
2. Single feeder vs multi-sig contract as feeder from day one?
3. Store `safety_margin_bps` on the oracle, the token, or both?
4. Align `observed_at` with Pi block/time identifiers for audit trails?

## Acceptance for this design doc

- [x] Describes oracle surface and mint invariant  
- [x] Freshness and safety margin  
- [x] Trust model and phased rollout  
- [x] Explicitly defers implementation to a follow-up issue  

**Suggested follow-up issue title:** `Implement reserve_oracle + wpi-token mint invariant (PoR medium-term)`
