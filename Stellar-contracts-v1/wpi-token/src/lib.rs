#![no_std]

//! Wrapped Pi (wPi) Soroban token.
//!
//! Bridge mint and burn operations are protected by independently configurable
//! volume limits. Reaching either limit pauses the whole contract and emits a
//! `VolumeLimitTriggered` event. The independent volume-limit admin must call
//! `override_volume_limit` to reset the window and lift that circuit breaker.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address,
    BytesN, Env, Symbol,
};

const NAME: &str = "Wrapped Pi";
const SYMBOL: &str = "wPI";
pub const DECIMALS: u32 = 7;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    VolumeLimitAdmin,
    Paused,
    CircuitBreaker,
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
    ProcessedDeposit(BytesN<32>),
    RedemptionNonce,
    VolumeLimitConfig,
    VolumeGeneration,
    VolumeBucket(u32),
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceData {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeLimitConfig {
    pub mint_limit: i128,
    pub burn_limit: i128,
    pub window_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeWindow {
    pub started_at: u64,
    pub minted: i128,
    pub burned: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct VolumeBucket {
    generation: u32,
    index: u64,
    minted: i128,
    burned: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAdmin = 1,
    Paused = 2,
    InsufficientBalance = 3,
    InsufficientAllowance = 4,
    Overflow = 5,
    DepositAlreadyProcessed = 6,
    VolumeLimitsNotConfigured = 7,
    InvalidVolumeLimit = 8,
    CircuitBreakerActive = 9,
    InvalidAmount = 10,
    InvalidExpirationLedger = 11,
}

#[contractevent]
pub struct DepositMinted {
    #[topic]
    pub pi_deposit_id: BytesN<32>,
    pub to: Address,
    pub amount: i128,
}

#[contractevent]
pub struct RedemptionBurned {
    #[topic]
    pub nonce: u64,
    pub from: Address,
    pub amount: i128,
    pub pi_destination: BytesN<32>,
}

/// Alert emitted when a successful bridge operation reaches its configured
/// rolling-window threshold and activates the circuit breaker.
#[contractevent]
pub struct VolumeLimitTriggered {
    #[topic]
    pub operation: Symbol,
    pub attempted_volume: i128,
    pub limit: i128,
    pub accepted: bool,
    pub window_started_at: u64,
    pub window_seconds: u64,
}

#[contractevent]
pub struct VolumeLimitsConfigured {
    pub mint_limit: i128,
    pub burn_limit: i128,
    pub window_seconds: u64,
}

#[contractevent]
pub struct VolumeLimitOverride {
    pub admin: Address,
    pub reset_at: u64,
}

#[contract]
pub struct WpiToken;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

/// Authenticates the current on-chain admin from stored state and returns
/// it. Callers must never accept an admin address as an argument and
/// authenticate that instead — doing so couples correctness to argument
/// order and lets a caller-supplied address silently stand in for the real
/// admin.
fn require_admin(env: &Env) -> Address {
    let admin = read_admin(env);
    admin.require_auth();
    admin
}

fn read_volume_limit_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::VolumeLimitAdmin)
        // Backwards-compatible default for an upgraded deployment whose
        // pre-Issue-26 state does not contain this key yet.
        .unwrap_or_else(|| read_admin(env))
}

/// Authenticates the current on-chain volume-limit admin from stored state
/// and returns it. See [`require_admin`] for why this must not take the
/// admin as an argument.
fn require_volume_limit_admin(env: &Env) -> Address {
    let admin = read_volume_limit_admin(env);
    admin.require_auth();
    admin
}

fn write_volume_limit_admin(env: &Env, admin: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::VolumeLimitAdmin, admin);
}

fn write_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::Paused)
        .unwrap_or(false)
}

fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

fn is_circuit_breaker_active(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::CircuitBreaker)
        .unwrap_or(false)
}

fn set_circuit_breaker(env: &Env, active: bool) {
    env.storage()
        .instance()
        .set(&DataKey::CircuitBreaker, &active);
}

fn read_balance(env: &Env, address: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Balance(address.clone()))
        .unwrap_or(0)
}

fn write_balance(env: &Env, address: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Balance(address.clone()), &amount);
}

fn read_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let allowance = env
        .storage()
        .instance()
        .get::<DataKey, AllowanceData>(&DataKey::Allowance(owner.clone(), spender.clone()));
    match allowance {
        Some(data) if data.expiration_ledger >= env.ledger().sequence() => data.amount,
        _ => 0,
    }
}

fn write_allowance(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    env.storage().instance().set(
        &DataKey::Allowance(owner.clone(), spender.clone()),
        &AllowanceData {
            amount,
            expiration_ledger,
        },
    );
}

fn read_total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalSupply)
        .unwrap_or(0)
}

fn write_total_supply(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalSupply, &amount);
}

fn is_deposit_processed(env: &Env, deposit_id: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::ProcessedDeposit(deposit_id.clone()))
        .unwrap_or(false)
}

fn mark_deposit_processed(env: &Env, deposit_id: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::ProcessedDeposit(deposit_id.clone()), &true);
}

fn next_redemption_nonce(env: &Env) -> Result<u64, Error> {
    let current = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::RedemptionNonce)
        .unwrap_or(0);
    let next = current.checked_add(1).ok_or(Error::Overflow)?;
    env.storage()
        .instance()
        .set(&DataKey::RedemptionNonce, &next);
    Ok(next)
}

fn read_volume_config(env: &Env) -> Result<VolumeLimitConfig, Error> {
    env.storage()
        .instance()
        .get::<DataKey, VolumeLimitConfig>(&DataKey::VolumeLimitConfig)
        .ok_or(Error::VolumeLimitsNotConfigured)
}

fn read_volume_generation(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::VolumeGeneration)
        .unwrap_or(0)
}

fn advance_volume_generation(env: &Env) -> Result<u32, Error> {
    let next = read_volume_generation(env)
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    env.storage()
        .instance()
        .set(&DataKey::VolumeGeneration, &next);
    Ok(next)
}

fn bucket_geometry(config: &VolumeLimitConfig) -> (u64, u32) {
    const MAX_BUCKETS: u64 = 24;
    let bucket_seconds = config.window_seconds.div_ceil(MAX_BUCKETS).max(1);
    let bucket_count = config.window_seconds.div_ceil(bucket_seconds) as u32;
    (bucket_seconds, bucket_count)
}

fn read_volume_bucket(env: &Env, generation: u32, index: u64) -> VolumeBucket {
    // Twenty-four subdivisions plus one safety bucket prevent operations near
    // a bucket boundary from expiring before the configured rolling window.
    let slot = (index % 25) as u32;
    let stored = env
        .storage()
        .instance()
        .get::<DataKey, VolumeBucket>(&DataKey::VolumeBucket(slot));
    match stored {
        Some(bucket) if bucket.generation == generation && bucket.index == index => bucket,
        _ => VolumeBucket {
            generation,
            index,
            minted: 0,
            burned: 0,
        },
    }
}

fn write_volume_bucket(env: &Env, bucket: &VolumeBucket) {
    let slot = (bucket.index % 25) as u32;
    env.storage()
        .instance()
        .set(&DataKey::VolumeBucket(slot), bucket);
}

fn read_volume_window(env: &Env, config: &VolumeLimitConfig) -> Result<VolumeWindow, Error> {
    let now = env.ledger().timestamp();
    let generation = read_volume_generation(env);
    let (bucket_seconds, bucket_count) = bucket_geometry(config);
    let current_index = now / bucket_seconds;
    let mut minted = 0i128;
    let mut burned = 0i128;
    let mut offset = 0u32;
    // Include one older boundary bucket. This is deliberately conservative:
    // it may retain volume for at most one bucket longer, but never releases
    // capacity before the full configured window has elapsed.
    while offset <= bucket_count {
        if u64::from(offset) > current_index {
            break;
        }
        let bucket = read_volume_bucket(env, generation, current_index - u64::from(offset));
        minted = minted.checked_add(bucket.minted).ok_or(Error::Overflow)?;
        burned = burned.checked_add(bucket.burned).ok_or(Error::Overflow)?;
        offset += 1;
    }

    Ok(VolumeWindow {
        started_at: now.saturating_sub(config.window_seconds),
        minted,
        burned,
    })
}

fn record_bridge_volume(env: &Env, operation: Symbol, amount: i128) -> Result<bool, Error> {
    let config = read_volume_config(env)?;
    let window = read_volume_window(env, &config)?;
    let generation = read_volume_generation(env);
    let (bucket_seconds, _) = bucket_geometry(&config);
    let current_index = env.ledger().timestamp() / bucket_seconds;
    let mut bucket = read_volume_bucket(env, generation, current_index);
    let (new_volume, limit) = if operation == symbol_short!("mint") {
        let volume = window.minted.checked_add(amount).ok_or(Error::Overflow)?;
        bucket.minted = bucket.minted.checked_add(amount).ok_or(Error::Overflow)?;
        (volume, config.mint_limit)
    } else {
        let volume = window.burned.checked_add(amount).ok_or(Error::Overflow)?;
        bucket.burned = bucket.burned.checked_add(amount).ok_or(Error::Overflow)?;
        (volume, config.burn_limit)
    };

    let accepted = new_volume <= limit;
    if accepted {
        write_volume_bucket(env, &bucket);
    }
    if new_volume >= limit {
        set_circuit_breaker(env, true);
        set_paused(env, true);
        VolumeLimitTriggered {
            operation,
            attempted_volume: new_volume,
            limit,
            accepted,
            window_started_at: window.started_at,
            window_seconds: config.window_seconds,
        }
        .publish(env);
    }
    Ok(accepted)
}

fn transfer_internal(env: &Env, from: &Address, to: &Address, amount: i128) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InvalidAmount);
    }
    let from_balance = read_balance(env, from);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    if from == to {
        return Ok(());
    }
    let to_balance = read_balance(env, to);
    let new_to_balance = to_balance.checked_add(amount).ok_or(Error::Overflow)?;
    write_balance(env, from, from_balance - amount);
    write_balance(env, to, new_to_balance);
    Ok(())
}

#[contractimpl]
impl WpiToken {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        write_admin(&env, &admin);
        // The deployer must rotate this role to governance/multisig before
        // enabling bridge traffic if the bridge admin is a relayer key.
        write_volume_limit_admin(&env, &admin);
        set_paused(&env, false);
        set_circuit_breaker(&env, false);
    }

    pub fn name(env: Env) -> BytesN<32> {
        let mut out = [0u8; 32];
        let bytes = NAME.as_bytes();
        let length = if bytes.len() > 32 { 32 } else { bytes.len() };
        out[..length].copy_from_slice(&bytes[..length]);
        BytesN::from_array(&env, &out)
    }

    pub fn symbol(env: Env) -> BytesN<32> {
        let mut out = [0u8; 32];
        let bytes = SYMBOL.as_bytes();
        let length = if bytes.len() > 32 { 32 } else { bytes.len() };
        out[..length].copy_from_slice(&bytes[..length]);
        BytesN::from_array(&env, &out)
    }

    pub fn decimals(_env: Env) -> u32 {
        DECIMALS
    }

    pub fn total_supply(env: Env) -> i128 {
        read_total_supply(&env)
    }

    pub fn balance(env: Env, owner: Address) -> i128 {
        read_balance(&env, &owner)
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        read_allowance(&env, &owner, &spender)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn volume_limit_admin(env: Env) -> Address {
        read_volume_limit_admin(&env)
    }

    pub fn paused(env: Env) -> bool {
        is_paused(&env)
    }

    pub fn circuit_breaker_active(env: Env) -> bool {
        is_circuit_breaker_active(&env)
    }

    pub fn volume_limit_config(env: Env) -> Result<VolumeLimitConfig, Error> {
        read_volume_config(&env)
    }

    pub fn current_volume_window(env: Env) -> Result<VolumeWindow, Error> {
        let config = read_volume_config(&env)?;
        read_volume_window(&env, &config)
    }

    pub fn configure_volume_limits(
        env: Env,
        mint_limit: i128,
        burn_limit: i128,
        window_seconds: u64,
    ) -> Result<(), Error> {
        require_volume_limit_admin(&env);
        if mint_limit <= 0 || burn_limit <= 0 || window_seconds == 0 {
            return Err(Error::InvalidVolumeLimit);
        }
        let config = VolumeLimitConfig {
            mint_limit,
            burn_limit,
            window_seconds,
        };
        advance_volume_generation(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::VolumeLimitConfig, &config);
        VolumeLimitsConfigured {
            mint_limit,
            burn_limit,
            window_seconds,
        }
        .publish(&env);
        Ok(())
    }

    /// Admin-only override for a tripped bridge-volume circuit breaker.
    /// Resets both rolling counters and starts a fresh window before unpausing.
    pub fn override_volume_limit(env: Env) -> Result<(), Error> {
        let admin = require_volume_limit_admin(&env);
        read_volume_config(&env)?;
        let now = env.ledger().timestamp();
        advance_volume_generation(&env)?;
        set_circuit_breaker(&env, false);
        set_paused(&env, false);
        VolumeLimitOverride {
            admin,
            reset_at: now,
        }
        .publish(&env);
        Ok(())
    }

    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) -> Result<(), Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }
        if amount != 0 && expiration_ledger < env.ledger().sequence() {
            return Err(Error::InvalidExpirationLedger);
        }
        owner.require_auth();
        write_allowance(&env, &owner, &spender, amount, expiration_ledger);
        Ok(())
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        from.require_auth();
        transfer_internal(&env, &from, &to, amount)
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            return Err(Error::InsufficientAllowance);
        }
        transfer_internal(&env, &from, &to, amount)?;
        let expiration_ledger = env
            .storage()
            .instance()
            .get::<DataKey, AllowanceData>(&DataKey::Allowance(from.clone(), spender.clone()))
            .map(|data| data.expiration_ledger)
            .unwrap_or(0);
        write_allowance(&env, &from, &spender, allowance - amount, expiration_ledger);
        Ok(())
    }

    pub fn burn_from(
        env: Env,
        spender: Address,
        from: Address,
        amount: i128,
    ) -> Result<bool, Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            return Err(Error::InsufficientAllowance);
        }
        let balance = read_balance(&env, &from);
        if balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let supply = read_total_supply(&env);
        let new_supply = supply.checked_sub(amount).ok_or(Error::Overflow)?;
        if !record_bridge_volume(&env, symbol_short!("burn"), amount)? {
            return Ok(false);
        }
        let expiration_ledger = env
            .storage()
            .instance()
            .get::<DataKey, AllowanceData>(&DataKey::Allowance(from.clone(), spender.clone()))
            .map(|data| data.expiration_ledger)
            .unwrap_or(0);
        write_allowance(&env, &from, &spender, allowance - amount, expiration_ledger);
        write_balance(&env, &from, balance - amount);
        write_total_supply(&env, new_supply);
        Ok(true)
    }

    /// Administrative mint. It uses the same bridge-wide mint counter as
    /// `mint_from_deposit`, so no privileged mint path bypasses the cap.
    pub fn mint(env: Env, to: Address, amount: i128) -> Result<bool, Error> {
        require_admin(&env);
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let balance = read_balance(&env, &to);
        let supply = read_total_supply(&env);
        let new_balance = balance.checked_add(amount).ok_or(Error::Overflow)?;
        let new_supply = supply.checked_add(amount).ok_or(Error::Overflow)?;
        if !record_bridge_volume(&env, symbol_short!("mint"), amount)? {
            return Ok(false);
        }
        write_balance(&env, &to, new_balance);
        write_total_supply(&env, new_supply);
        Ok(true)
    }

    pub fn is_deposit_processed(env: Env, pi_deposit_id: BytesN<32>) -> bool {
        is_deposit_processed(&env, &pi_deposit_id)
    }

    pub fn mint_from_deposit(
        env: Env,
        to: Address,
        amount: i128,
        pi_deposit_id: BytesN<32>,
    ) -> Result<bool, Error> {
        require_admin(&env);
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if is_deposit_processed(&env, &pi_deposit_id) {
            return Err(Error::DepositAlreadyProcessed);
        }
        let balance = read_balance(&env, &to);
        let supply = read_total_supply(&env);
        let new_balance = balance.checked_add(amount).ok_or(Error::Overflow)?;
        let new_supply = supply.checked_add(amount).ok_or(Error::Overflow)?;

        if !record_bridge_volume(&env, symbol_short!("mint"), amount)? {
            return Ok(false);
        }
        write_balance(&env, &to, new_balance);
        write_total_supply(&env, new_supply);
        mark_deposit_processed(&env, &pi_deposit_id);
        DepositMinted {
            pi_deposit_id,
            to,
            amount,
        }
        .publish(&env);
        Ok(true)
    }

    pub fn burn(
        env: Env,
        from: Address,
        amount: i128,
        pi_destination: BytesN<32>,
    ) -> Result<bool, Error> {
        require_admin(&env);
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let balance = read_balance(&env, &from);
        if balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let supply = read_total_supply(&env);
        let new_supply = supply.checked_sub(amount).ok_or(Error::Overflow)?;
        if !record_bridge_volume(&env, symbol_short!("burn"), amount)? {
            return Ok(false);
        }
        let nonce = next_redemption_nonce(&env)?;
        write_balance(&env, &from, balance - amount);
        write_total_supply(&env, new_supply);
        RedemptionBurned {
            nonce,
            from,
            amount,
            pi_destination,
        }
        .publish(&env);
        Ok(true)
    }

    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        require_admin(&env);
        write_admin(&env, &new_admin);
        Ok(())
    }

    /// Rotates the independent circuit-breaker authority. Only the current
    /// volume-limit admin can transfer this role; the mint/burn admin cannot
    /// reclaim it after it has been handed to a multisig.
    pub fn set_volume_limit_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        require_volume_limit_admin(&env);
        write_volume_limit_admin(&env, &new_admin);
        Ok(())
    }

    pub fn set_paused(env: Env, paused: bool) -> Result<(), Error> {
        require_admin(&env);
        if !paused && is_circuit_breaker_active(&env) {
            return Err(Error::CircuitBreakerActive);
        }
        set_paused(&env, paused);
        Ok(())
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        require_admin(&env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }
}

#[cfg(test)]
mod test;
