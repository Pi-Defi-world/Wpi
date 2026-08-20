#![no_std]

//! Mock AMM pool for testing wPi -> MockUSDC swaps.
//! Hardcodes a 1:1 swap rate (or configurable) for testnet simulation without complex math.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env,
};

/// Maximum supported swap rate. The rate denominator is 1,000,000,
/// so this caps the test AMM at a 1:1 exchange rate.
pub const MAX_RATE_BPS: u32 = 1_000_000;

/// SHA-256 hash of the Stellar mainnet network passphrase
/// ("Public Global Stellar Network ; September 2015"). This is the value
/// `env.ledger().network_id()` returns when a contract is running on
/// mainnet. mock-amm is test-only and must never be initialized there.
const MAINNET_NETWORK_ID: [u8; 32] = [
    0x7a, 0xc3, 0x39, 0x97, 0x54, 0x4e, 0x31, 0x75, 0xd2, 0x66, 0xbd, 0x02, 0x24, 0x39, 0xb2, 0x2c,
    0xdb, 0x16, 0x50, 0x8c, 0x01, 0x16, 0x3f, 0x26, 0xe5, 0xcb, 0x2a, 0x3e, 0x10, 0x45, 0xa9, 0x79,
];

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TokenIn,  // wPi
    TokenOut, // MockUSDC
    Rate,     // Rate: out_amount = in_amount * Rate / 1_000_000
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotAdmin = 1,
    InsufficientLiquidity = 2,
    SlippageExceeded = 3,
    MainnetNotSupported = 4,
    InvalidRate = 5,
}

#[contract]
pub struct MockAmm;

#[contractimpl]
impl MockAmm {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_in: Address,
        token_out: Address,
        rate_bps: u32,
    ) -> Result<(), Error> {
        if env.ledger().network_id() == BytesN::from_array(&env, &MAINNET_NETWORK_ID) {
            return Err(Error::MainnetNotSupported);
        }
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        if rate_bps == 0 || rate_bps > MAX_RATE_BPS {
            return Err(Error::InvalidRate);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenIn, &token_in);
        env.storage().instance().set(&DataKey::TokenOut, &token_out);
        env.storage().instance().set(&DataKey::Rate, &rate_bps);
        Ok(())
    }

    /// Swap token_in (wPi) for token_out (MockUSDC)
    pub fn swap(
        env: Env,
        to: Address,
        amount_in: i128,
        min_amount_out: i128,
    ) -> Result<i128, Error> {
        to.require_auth();

        let token_in_addr: Address = env.storage().instance().get(&DataKey::TokenIn).unwrap();
        let token_out_addr: Address = env.storage().instance().get(&DataKey::TokenOut).unwrap();
        let rate: u32 = env.storage().instance().get(&DataKey::Rate).unwrap();

        let amount_out = (amount_in * rate as i128) / 1_000_000;

        if amount_out < min_amount_out {
            return Err(Error::SlippageExceeded);
        }

        let token_in = token::Client::new(&env, &token_in_addr);
        let token_out = token::Client::new(&env, &token_out_addr);

        let contract_addr = env.current_contract_address();

        if token_out.balance(&contract_addr) < amount_out {
            return Err(Error::InsufficientLiquidity);
        }

        token_in.transfer(&to, &contract_addr, &amount_in);
        token_out.transfer(&contract_addr, &to, &amount_out);

        Ok(amount_out)
    }

    pub fn deposit_liquidity(env: Env, from: Address, amount_out: i128) {
        from.require_auth();
        let token_out_addr: Address = env.storage().instance().get(&DataKey::TokenOut).unwrap();
        let token_out = token::Client::new(&env, &token_out_addr);
        let pool_address = env.current_contract_address();
        token_out.transfer(&from, &pool_address, &amount_out);
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{Error, MockAmm, MockAmmClient, MAX_RATE_BPS};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, MockAmmClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(MockAmm, ());
        let client = MockAmmClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token_in = Address::generate(&env);
        let token_out = Address::generate(&env);

        (env, client, admin, token_in, token_out)
    }

    #[test]
    fn initialize_rejects_zero_rate() {
        let (_env, client, admin, token_in, token_out) = setup();

        let result = client.try_initialize(&admin, &token_in, &token_out, &0);

        assert_eq!(result, Err(Ok(Error::InvalidRate)));
    }

    #[test]
    fn initialize_rejects_rate_above_max() {
        let (_env, client, admin, token_in, token_out) = setup();

        let excessive_rate = MAX_RATE_BPS + 1;
        let result = client.try_initialize(&admin, &token_in, &token_out, &excessive_rate);

        assert_eq!(result, Err(Ok(Error::InvalidRate)));
    }
}
