#![no_std]

//! Mock USDC for Stellar testnet — same admin-mint token interface as wPi / PUSD.
//! Use only for DEX / reserve simulations; production reserves use real USDC.

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};
use soroban_token_common::{
    approve_token, burn_token, initialize_token, mint_token, read_admin, read_balance,
    read_total_supply, set_admin_token, set_paused_token, transfer_from_token, transfer_token,
    Error,
};

const NAME: &str = "Mock USDC";
const SYMBOL: &str = "mUSDC";
/// 7 decimals (Stellar-style); align pools with wPi.
pub const DECIMALS: u32 = 7;

#[contract]
pub struct MockUsdcToken;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
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

fn read_balance(env: &Env, addr: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Balance(addr.clone()))
        .unwrap_or(0)
}

fn write_balance(env: &Env, addr: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Balance(addr.clone()), &amount);
}

fn read_allowance(env: &Env, from: &Address, spender: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Allowance(from.clone(), spender.clone()))
        .unwrap_or(0)
}

fn write_allowance(env: &Env, from: &Address, spender: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Allowance(from.clone(), spender.clone()), &amount);
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

#[contractimpl]
impl MockUsdcToken {
    pub fn initialize(env: Env, admin: Address) {
        initialize_token(&env, &admin);
    }

    pub fn name(_env: Env) -> BytesN<32> {
        let mut out = [0u8; 32];
        let b = NAME.as_bytes();
        let n = if b.len() > 32 { 32 } else { b.len() };
        out[..n].copy_from_slice(&b[..n]);
        BytesN::from_array(&_env, &out)
    }

    pub fn symbol(_env: Env) -> BytesN<32> {
        let mut out = [0u8; 32];
        let b = SYMBOL.as_bytes();
        let n = if b.len() > 32 { 32 } else { b.len() };
        out[..n].copy_from_slice(&b[..n]);
        BytesN::from_array(&_env, &out)
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
        soroban_token_common::read_allowance(&env, &owner, &spender)
    }

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) -> Result<(), Error> {
        approve_token(&env, &owner, &spender, amount)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        transfer_token(&env, &from, &to, amount)
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        transfer_from_token(&env, &spender, &from, &to, amount)
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        spender.require_auth();
        let current_allowance = read_allowance(&env, &from, &spender);
        if current_allowance < amount {
            return Err(Error::InsufficientAllowance);
        }
        write_allowance(&env, &from, &spender, current_allowance - amount);
        Self::transfer_internal(&env, &from, &to, amount)
    }

    fn transfer_internal(
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

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        mint_token(&env, &admin, &to, amount)
    }

    pub fn burn(env: Env, admin: Address, from: Address, amount: i128) -> Result<(), Error> {
        burn_token(&env, &admin, &from, amount)
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        set_admin_token(&env, &admin, &new_admin)
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        set_paused_token(&env, &admin, paused)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}
