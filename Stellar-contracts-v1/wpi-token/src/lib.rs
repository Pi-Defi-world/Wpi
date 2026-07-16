#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer mints wPi after Pi deposits
//! are observed on Pi Network. Same interface shape as `pusd-token` for SDK compatibility.

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};
use soroban_token_common::{
    approve_token, burn_token, initialize_token, mint_token, read_admin, read_balance,
    read_total_supply, set_admin_token, set_paused_token, transfer_from_token, transfer_token,
    Error,
};

const NAME: &str = "Wrapped Pi";
const SYMBOL: &str = "wPI";
/// 7 decimals to match native Pi stroops convention (1e7).
pub const DECIMALS: u32 = 7;

#[contract]
pub struct WpiToken;

#[contractimpl]
impl WpiToken {
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
