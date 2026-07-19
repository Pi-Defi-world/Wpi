#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer (see `relayer/`) mints wPi after
//! Pi deposits are observed on Pi Network, and watches burns to release Pi on redemption.
//! Same interface shape as `pusd-token` for SDK compatibility.

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};
use soroban_token_common::{
    approve_token, burn_token, initialize_token, mint_token, read_admin, read_balance,
    read_total_supply, set_admin_token, set_paused_token, transfer_from_token, transfer_token,
    Error,#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer mints wPi after Pi deposits
//! are observed on Pi Network. Same interface shape as `pusd-token` for SDK compatibility.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env,
};

const NAME: &str = "Wrapped Pi";
const SYMBOL: &str = "wPI";
/// 7 decimals to match native Pi stroops convention (1e7).
pub const DECIMALS: u32 = 7;

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
    Overflow = 5,
}

#[contract]
pub struct WpiToken;

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
        .set(
            &DataKey::Allowance(from.clone(), spender.clone()),
            &amount,
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

#[contractimpl]
impl WpiToken {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        write_admin(&env, &admin);
        set_paused(&env, false);
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
        read_allowance(&env, &owner, &spender)
    }

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) -> Result<(), Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        owner.require_auth();
        write_allowance(&env, &owner, &spender, amount);
        Ok(())
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        from.require_auth();
        Self::transfer_internal(&env, &from, &to, amount)
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
        spender.require_auth();
        let current_allowance = read_allowance(&env, &from, &spender);
        if current_allowance < amount {
            return Err(Error::InsufficientAllowance);
        }
        write_allowance(&env, &from, &spender, current_allowance - amount);
        Self::transfer_internal(&env, &from, &to, amount)
    }

    fn transfer_internal(env: &Env, from: &Address, to: &Address, amount: i128) -> Result<(), Error> {
        if amount < 0 {
            return Err(Error::InsufficientBalance);
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

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Ok(());
        }
        let to_balance = read_balance(&env, &to);
        let total = read_total_supply(&env);
        let new_to_balance = to_balance.checked_add(amount).ok_or(Error::Overflow)?;
        let new_total = total.checked_add(amount).ok_or(Error::Overflow)?;
        write_balance(&env, &to, new_to_balance);
        write_total_supply(&env, new_total);
        Ok(())
    }

    pub fn burn(env: Env, admin: Address, from: Address, amount: i128) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Ok(());
        }
        let from_balance = read_balance(&env, &from);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let total = read_total_supply(&env);
        let new_total = total.checked_sub(amount).ok_or(Error::Overflow)?;
        write_balance(&env, &from, from_balance - amount);
        write_total_supply(&env, new_total);
        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        write_admin(&env, &new_admin);
        Ok(())
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        set_paused(&env, paused);
        Ok(())
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}
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
        // Self-transfer is a strict no-op: skip the read/modify/write of `to`'s
        // balance entirely so it can never double-count or spuriously overflow.
        if from == to {
            return Ok(());
        }
        let to_balance = read_balance(env, to);
        let new_to_balance = match to_balance.checked_add(amount) {
            Some(v) => v,
            None => return Err(Error::Overflow),
        };
        // from_balance >= amount was already checked above, so this cannot underflow.
        write_balance(env, from, from_balance - amount);
        write_balance(env, to, new_to_balance);
        Ok(())
    }

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Ok(());
        }
        let to_balance = read_balance(&env, &to);
        let total = read_total_supply(&env);
        let new_to_balance = match to_balance.checked_add(amount) {
            Some(v) => v,
            None => return Err(Error::Overflow),
        };
        let new_total = match total.checked_add(amount) {
            Some(v) => v,
            None => return Err(Error::Overflow),
        };
        write_balance(&env, &to, new_to_balance);
        write_total_supply(&env, new_total);
        Ok(())
    }

    /// Whether `pi_deposit_id` has already been minted. Lets the relayer
    /// reconcile after a submission whose outcome was ambiguous (e.g. the
    /// network dropped the response) without guessing from error text —
    /// it just re-checks this before deciding whether to retry.
    pub fn is_deposit_processed(env: Env, pi_deposit_id: BytesN<32>) -> bool {
        is_deposit_processed(&env, &pi_deposit_id)
    }

    /// Mints wPi against an observed Pi Network deposit. Called by the relayer
    /// (see `relayer/`) once the deposit has cleared the required confirmation
    /// depth on Pi Network.
    ///
    /// Idempotent by `pi_deposit_id`: a second call with the same id is a
    /// cheap no-op (`Err(DepositAlreadyProcessed)`) rather than a double mint,
    /// so the relayer can safely retry after a crash or a dropped response
    /// without tracking submission state itself.
    pub fn mint_from_deposit(
        env: Env,
        admin: Address,
        to: Address,
        amount: i128,
        pi_deposit_id: BytesN<32>,
    ) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if is_paused(&env) {
            return Err(Error::Paused);
        }
        if is_deposit_processed(&env, &pi_deposit_id) {
            return Err(Error::DepositAlreadyProcessed);
        }
        mark_deposit_processed(&env, &pi_deposit_id);
        if amount > 0 {
            let to_balance = read_balance(&env, &to);
            let total = read_total_supply(&env);
            write_balance(&env, &to, to_balance + amount);
            write_total_supply(&env, total + amount);
        }
        DepositMinted {
            pi_deposit_id,
            to,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Burns wPi to request a Pi Network redemption, releasing native Pi to
    /// `pi_destination` (the raw 32-byte Pi Network account id — Pi uses the
    /// same StrKey/ed25519 address format as Stellar, being an SCP fork).
    /// Emits a `redeem` event tagged with a monotonic nonce so the relayer's
    /// redemption watcher (see `relayer/`) can enumerate burns in order and
    /// dedupe Pi-side releases.
    pub fn burn(
        env: Env,
        admin: Address,
        from: Address,
        amount: i128,
        pi_destination: BytesN<32>,
    ) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Ok(());
        }
        let from_balance = read_balance(&env, &from);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let total = read_total_supply(&env);
        let new_total = match total.checked_sub(amount) {
            Some(v) => v,
            None => return Err(Error::Overflow),
        };
        // from_balance >= amount was already checked above, so this cannot underflow.
        write_balance(&env, &from, from_balance - amount);
        write_total_supply(&env, total - amount);
        let nonce = next_redemption_nonce(&env);
        RedemptionBurned {
            nonce,
            from,
            amount,
            pi_destination,
        }
        .publish(&env);
        Ok(())
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
