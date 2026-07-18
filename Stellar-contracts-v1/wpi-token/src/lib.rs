#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer (see `relayer/`) mints wPi after
//! Pi deposits are observed on Pi Network, and watches burns to release Pi on redemption.
//! Same interface shape as `pusd-token` for SDK compatibility.

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

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup(env: &Env) -> (Address, WpiTokenClient) {
        let admin = Address::generate(env);
        let contract_id = env.register(WpiToken, ());
        let client = WpiTokenClient::new(env, &contract_id);
        client.initialize(&admin);
        (admin, client)
    }

    fn deposit_id(env: &Env, tag: u8) -> BytesN<32> {
        BytesN::from_array(env, &[tag; 32])
    }

    #[test]
    fn mint_from_deposit_credits_balance_and_supply() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);
        let dep = deposit_id(&env, 1);

        client.mint_from_deposit(&admin, &user, &10_000_000, &dep);

        assert_eq!(client.balance(&user), 10_000_000);
        assert_eq!(client.total_supply(), 10_000_000);
    }

    #[test]
    fn is_deposit_processed_reflects_mint_state() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);
        let dep = deposit_id(&env, 1);

        assert!(!client.is_deposit_processed(&dep));
        client.mint_from_deposit(&admin, &user, &10_000_000, &dep);
        assert!(client.is_deposit_processed(&dep));
        assert!(!client.is_deposit_processed(&deposit_id(&env, 2)));
    }

    #[test]
    fn mint_from_deposit_is_idempotent_on_retry() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);
        let dep = deposit_id(&env, 1);

        client.mint_from_deposit(&admin, &user, &5_000_000, &dep);
        // Relayer retries the same deposit id (e.g. after a crash before it
        // recorded submission) — must not double-mint.
        let retry = client.try_mint_from_deposit(&admin, &user, &5_000_000, &dep);

        assert_eq!(retry, Err(Ok(Error::DepositAlreadyProcessed)));
        assert_eq!(client.balance(&user), 5_000_000);
        assert_eq!(client.total_supply(), 5_000_000);
    }

    #[test]
    fn mint_from_deposit_distinct_ids_both_mint() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);

        client.mint_from_deposit(&admin, &user, &100, &deposit_id(&env, 1));
        client.mint_from_deposit(&admin, &user, &100, &deposit_id(&env, 2));

        assert_eq!(client.balance(&user), 200);
        assert_eq!(client.total_supply(), 200);
    }

    #[test]
    fn mint_from_deposit_rejects_non_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let (_admin, client) = setup(&env);
        let not_admin = Address::generate(&env);
        let user = Address::generate(&env);

        let result = client.try_mint_from_deposit(&not_admin, &user, &100, &deposit_id(&env, 1));

        assert_eq!(result, Err(Ok(Error::NotAdmin)));
        assert_eq!(client.balance(&user), 0);
    }

    #[test]
    fn mint_from_deposit_blocked_while_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);
        client.set_paused(&admin, &true);

        let result = client.try_mint_from_deposit(&admin, &user, &100, &deposit_id(&env, 1));

        assert_eq!(result, Err(Ok(Error::Paused)));
    }

    #[test]
    fn burn_supports_repeated_redemptions() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let user = Address::generate(&env);
        client.mint_from_deposit(&admin, &user, &300, &deposit_id(&env, 1));
        let pi_dest = BytesN::from_array(&env, &[9u8; 32]);

        client.burn(&admin, &user, &100, &pi_dest);
        client.burn(&admin, &user, &100, &pi_dest);

        assert_eq!(client.balance(&user), 100);
        assert_eq!(client.total_supply(), 100);
    }
}
