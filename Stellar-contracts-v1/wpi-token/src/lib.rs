#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer mints wPi after Pi deposits
//! are observed on Pi Network. Same interface shape as `pusd-token` for SDK compatibility.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
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
    InvalidAmount = 5,
}

/// Topics: `("transfer", from, to)`, data: `amount`.
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// Topics: `("mint", admin, to)`, data: `amount`.
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mint {
    #[topic]
    pub admin: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// Topics: `("burn", admin, from)`, data: `amount`.
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Burn {
    #[topic]
    pub admin: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}

/// Topics: `("approve", owner, spender)`, data: `amount`.
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approve {
    #[topic]
    pub owner: Address,
    #[topic]
    pub spender: Address,
    pub amount: i128,
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
        Approve {
            owner: owner.clone(),
            spender: spender.clone(),
            amount,
        }
        .publish(&env);
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
        Transfer {
            from: from.clone(),
            to: to.clone(),
            amount,
        }
        .publish(env);
        Ok(())
    }

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let to_balance = read_balance(&env, &to);
        let total = read_total_supply(&env);
        write_balance(&env, &to, to_balance + amount);
        write_total_supply(&env, total + amount);
        Mint {
            admin: admin.clone(),
            to: to.clone(),
            amount,
        }
        .publish(&env);
        Ok(())
    }

    pub fn burn(env: Env, admin: Address, from: Address, amount: i128) -> Result<(), Error> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let from_balance = read_balance(&env, &from);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let total = read_total_supply(&env);
        write_balance(&env, &from, from_balance - amount);
        write_total_supply(&env, total - amount);
        Burn {
            admin: admin.clone(),
            from: from.clone(),
            amount,
        }
        .publish(&env);
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

#[cfg(test)]
mod test {
    extern crate std;

    use super::{Error, WpiToken, WpiTokenClient};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, WpiTokenClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WpiToken, ());
        let client = WpiTokenClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        client.initialize(&admin);

        (env, client, admin, user)
    }

    #[test]
    fn test_mint_validations() {
        let (_env, client, admin, user) = setup();

        // 1. Zero amount mint must fail
        let result_zero = client.try_mint(&admin, &user, &0);
        assert_eq!(result_zero, Err(Ok(Error::InvalidAmount)));
        assert_eq!(client.balance(&user), 0);
        assert_eq!(client.total_supply(), 0);

        // 2. Negative amount mint must fail
        let result_neg = client.try_mint(&admin, &user, &-100);
        assert_eq!(result_neg, Err(Ok(Error::InvalidAmount)));
        assert_eq!(client.balance(&user), 0);
        assert_eq!(client.total_supply(), 0);

        // 3. Positive amount mint must succeed
        let result_pos = client.mint(&admin, &user, &100);
        assert_eq!(result_pos, ());
        assert_eq!(client.balance(&user), 100);
        assert_eq!(client.total_supply(), 100);
    }

    #[test]
    fn test_burn_validations() {
        let (_env, client, admin, user) = setup();

        // Mint some tokens first
        client.mint(&admin, &user, &100);

        // 1. Zero amount burn must fail
        let result_zero = client.try_burn(&admin, &user, &0);
        assert_eq!(result_zero, Err(Ok(Error::InvalidAmount)));
        assert_eq!(client.balance(&user), 100);
        assert_eq!(client.total_supply(), 100);

        // 2. Negative amount burn must fail
        let result_neg = client.try_burn(&admin, &user, &-50);
        assert_eq!(result_neg, Err(Ok(Error::InvalidAmount)));
        assert_eq!(client.balance(&user), 100);
        assert_eq!(client.total_supply(), 100);

        // 3. Positive amount burn must succeed
        let result_pos = client.burn(&admin, &user, &30);
        assert_eq!(result_pos, ());
        assert_eq!(client.balance(&user), 70);
        assert_eq!(client.total_supply(), 70);
    }
}
