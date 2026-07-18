#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer (see `relayer/`) mints wPi after
//! Pi deposits are observed on Pi Network, and watches burns to release Pi on redemption.
//! Same interface shape as `pusd-token` for SDK compatibility.

use soroban_sdk::{
    contract, contractimpl, token::TokenInterface, Address, Env, MuxedAddress, String,
};
use soroban_token_common::{
    approve_token_with_expiration, burn_from_token, burn_holder_token, initialize_token,
    mint_token, read_admin, read_balance, set_admin_token, set_paused_token, transfer_from_token,
    transfer_token, Error,
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

    pub fn total_supply(env: Env) -> i128 {
        soroban_token_common::read_total_supply(&env)
    }

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        mint_token(&env, &admin, &to, amount)
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

#[contractimpl]
impl TokenInterface for WpiToken {
    fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        soroban_token_common::read_allowance(&env, &from, &spender)
    }

    fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        approve_token_with_expiration(&env, &from, &spender, amount, expiration_ledger).unwrap();
    }

    fn balance(env: Env, id: Address) -> i128 {
        read_balance(&env, &id)
    }

    fn transfer(env: Env, from: Address, to: MuxedAddress, amount: i128) {
        transfer_token(&env, &from, &to.address(), amount).unwrap();
    }

    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        transfer_from_token(&env, &spender, &from, &to, amount).unwrap();
    }

    fn burn(env: Env, from: Address, amount: i128) {
        burn_holder_token(&env, &from, amount).unwrap();
    }

    fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        burn_from_token(&env, &spender, &from, amount).unwrap();
    }

    fn decimals(_env: Env) -> u32 {
        DECIMALS
    }

    fn name(env: Env) -> String {
        String::from_str(&env, NAME)
    }

    fn symbol(env: Env) -> String {
        String::from_str(&env, SYMBOL)
    }
}

#[cfg(test)]
mod test {
    use super::{WpiToken, WpiTokenClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_sequence_number(100);

        let contract_id = env.register(WpiToken, ());
        let client = WpiTokenClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let spender = Address::generate(&env);
        client.initialize(&admin);
        client.mint(&admin, &owner, &1_000);
        (env, contract_id, admin, owner, spender)
    }

    #[test]
    fn allowance_expires_after_expiration_ledger() {
        let (env, contract_id, _admin, owner, spender) = setup();
        let client = WpiTokenClient::new(&env, &contract_id);
        client.approve(&owner, &spender, &400, &105);
        assert_eq!(client.allowance(&owner, &spender), 400);

        env.ledger().set_sequence_number(105);
        assert_eq!(client.allowance(&owner, &spender), 400);

        env.ledger().set_sequence_number(106);
        assert_eq!(client.allowance(&owner, &spender), 0);
    }

    #[test]
    fn transfer_from_consumes_unexpired_allowance() {
        let (env, contract_id, _admin, owner, spender) = setup();
        let client = WpiTokenClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        client.approve(&owner, &spender, &400, &105);

        client.transfer_from(&spender, &owner, &recipient, &150);

        assert_eq!(client.allowance(&owner, &spender), 250);
        assert_eq!(client.balance(&owner), 850);
        assert_eq!(client.balance(&recipient), 150);
    }

    #[test]
    fn burn_from_consumes_allowance_and_supply() {
        let (env, contract_id, _admin, owner, spender) = setup();
        let client = WpiTokenClient::new(&env, &contract_id);
        client.approve(&owner, &spender, &400, &105);

        client.burn_from(&spender, &owner, &150);

        assert_eq!(client.allowance(&owner, &spender), 250);
        assert_eq!(client.balance(&owner), 850);
        assert_eq!(client.total_supply(), 850);
    }
}
