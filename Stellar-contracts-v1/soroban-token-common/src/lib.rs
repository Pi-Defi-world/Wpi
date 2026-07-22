#![no_std]

use soroban_sdk::{contracterror, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub struct AllowanceData {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotAdmin = 1,
    Paused = 2,
    InsufficientBalance = 3,
    InsufficientAllowance = 4,
    InvalidExpirationLedger = 5,
}

pub fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

pub fn write_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn read_balance(env: &Env, addr: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Balance(addr.clone()))
        .unwrap_or(0)
}

pub fn write_balance(env: &Env, addr: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Balance(addr.clone()), &amount);
}

pub fn read_allowance(env: &Env, from: &Address, spender: &Address) -> i128 {
    let allowance = env
        .storage()
        .instance()
        .get::<DataKey, AllowanceData>(&DataKey::Allowance(from.clone(), spender.clone()));
    match allowance {
        Some(data) if data.expiration_ledger >= env.ledger().sequence() => data.amount,
        _ => 0,
    }
}

pub fn write_allowance(
    env: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    env.storage().instance().set(
        &DataKey::Allowance(from.clone(), spender.clone()),
        &AllowanceData {
            amount,
            expiration_ledger,
        },
    );
}

pub fn read_total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalSupply)
        .unwrap_or(0)
}

pub fn write_total_supply(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalSupply, &amount);
}

pub fn check_admin(env: &Env, admin: &Address) -> Result<(), Error> {
    let current_admin = read_admin(env);
    if *admin != current_admin {
        return Err(Error::NotAdmin);
    }
    admin.require_auth();
    Ok(())
}

pub fn transfer_internal(
    env: &Env,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InsufficientBalance);
    }
    let from_balance = read_balance(env, from);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    let to_balance = read_balance(env, to);
    write_balance(env, from, from_balance - amount);
    write_balance(env, to, to_balance + amount);
    Ok(())
}

pub fn initialize_token(env: &Env, admin: &Address) {
    if env.storage().instance().has(&DataKey::Admin) {
        panic!("already initialized");
    }
    admin.require_auth();
    write_admin(env, admin);
    set_paused(env, false);
}

pub fn mint_token(env: &Env, admin: &Address, to: &Address, amount: i128) -> Result<(), Error> {
    check_admin(env, admin)?;
    if amount <= 0 {
        return Ok(());
    }
    let to_balance = read_balance(env, to);
    let total = read_total_supply(env);
    write_balance(env, to, to_balance + amount);
    write_total_supply(env, total + amount);
    Ok(())
}

pub fn burn_token(env: &Env, admin: &Address, from: &Address, amount: i128) -> Result<(), Error> {
    check_admin(env, admin)?;
    if amount <= 0 {
        return Ok(());
    }
    let from_balance = read_balance(env, from);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    let total = read_total_supply(env);
    write_balance(env, from, from_balance - amount);
    write_total_supply(env, total - amount);
    Ok(())
}

pub fn approve_token(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
) -> Result<(), Error> {
    approve_token_with_expiration(env, owner, spender, amount, u32::MAX)
}

pub fn approve_token_with_expiration(
    env: &Env,
    owner: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) -> Result<(), Error> {
    if is_paused(env) {
        return Err(Error::Paused);
    }
    owner.require_auth();
    if amount < 0 {
        return Err(Error::InsufficientAllowance);
    }
    if amount != 0 && expiration_ledger < env.ledger().sequence() {
        return Err(Error::InvalidExpirationLedger);
    }
    write_allowance(env, owner, spender, amount, expiration_ledger);
    Ok(())
}

pub fn transfer_token(env: &Env, from: &Address, to: &Address, amount: i128) -> Result<(), Error> {
    if is_paused(env) {
        return Err(Error::Paused);
    }
    from.require_auth();
    transfer_internal(env, from, to, amount)
}

pub fn transfer_from_token(
    env: &Env,
    spender: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> Result<(), Error> {
    if is_paused(env) {
        return Err(Error::Paused);
    }
    spender.require_auth();
    let current_allowance = read_allowance(env, from, spender);
    if current_allowance < amount {
        return Err(Error::InsufficientAllowance);
    }
    let expiration_ledger = env
        .storage()
        .instance()
        .get::<DataKey, AllowanceData>(&DataKey::Allowance(from.clone(), spender.clone()))
        .map(|data| data.expiration_ledger)
        .unwrap_or(0);
    write_allowance(
        env,
        from,
        spender,
        current_allowance - amount,
        expiration_ledger,
    );
    transfer_internal(env, from, to, amount)
}

pub fn burn_from_token(
    env: &Env,
    spender: &Address,
    from: &Address,
    amount: i128,
) -> Result<(), Error> {
    if is_paused(env) {
        return Err(Error::Paused);
    }
    if amount < 0 {
        return Err(Error::InsufficientBalance);
    }
    spender.require_auth();
    let current_allowance = read_allowance(env, from, spender);
    if current_allowance < amount {
        return Err(Error::InsufficientAllowance);
    }
    let expiration_ledger = env
        .storage()
        .instance()
        .get::<DataKey, AllowanceData>(&DataKey::Allowance(from.clone(), spender.clone()))
        .map(|data| data.expiration_ledger)
        .unwrap_or(0);
    let from_balance = read_balance(env, from);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    write_allowance(
        env,
        from,
        spender,
        current_allowance - amount,
        expiration_ledger,
    );
    write_balance(env, from, from_balance - amount);
    write_total_supply(env, read_total_supply(env) - amount);
    Ok(())
}

pub fn burn_holder_token(env: &Env, from: &Address, amount: i128) -> Result<(), Error> {
    if is_paused(env) {
        return Err(Error::Paused);
    }
    if amount < 0 {
        return Err(Error::InsufficientBalance);
    }
    from.require_auth();
    let from_balance = read_balance(env, from);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    write_balance(env, from, from_balance - amount);
    write_total_supply(env, read_total_supply(env) - amount);
    Ok(())
}

pub fn set_admin_token(env: &Env, admin: &Address, new_admin: &Address) -> Result<(), Error> {
    check_admin(env, admin)?;
    write_admin(env, new_admin);
    Ok(())
}

pub fn set_paused_token(env: &Env, admin: &Address, paused: bool) -> Result<(), Error> {
    check_admin(env, admin)?;
    set_paused(env, paused);
    Ok(())
}
