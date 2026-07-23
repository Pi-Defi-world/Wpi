#![no_std]

//! Mock AMM pool for testing wPi -> USDC swaps.
//! Hardcodes a 1:1 swap rate (or configurable) for testnet simulation without complex math.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env,
};

mod usdc;
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TokenIn,  // wPi
    TokenOut, // Network USDC SAC
    Rate,     // Rate: out_amount = in_amount * Rate / 1_000_000
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotAdmin = 1,
    InsufficientLiquidity = 2,
    SlippageExceeded = 3,
}

#[contract]
pub struct MockAmm;

#[contractimpl]
impl MockAmm {
    pub fn initialize(env: Env, admin: Address, token_in: Address, rate_bps: u32) {
        let token_out = usdc::address(&env);
        admin.require_auth();
        Self::set_config(env, admin, token_in, token_out, rate_bps);
    }

    fn set_config(env: Env, admin: Address, token_in: Address, token_out: Address, rate_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenIn, &token_in);
        env.storage().instance().set(&DataKey::TokenOut, &token_out);
        env.storage().instance().set(&DataKey::Rate, &rate_bps);
    }

    /// Swap token_in (wPi) for the network's USDC SAC.
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
        token_out.transfer(&from, env.current_contract_address(), &amount_out);
    }

    pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != current_admin {
            return Err(Error::NotAdmin);
        }
        admin.require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{MockAmm, MockAmmClient};
    use soroban_sdk::{testutils::Address as _, token, Address, Env};

    #[test]
    fn swaps_against_registered_stellar_asset_contract() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let trader = Address::generate(&env);
        let wpi_admin = Address::generate(&env);
        let usdc_admin = Address::generate(&env);
        let wpi = env.register_stellar_asset_contract_v2(wpi_admin.clone());
        let usdc = env.register_stellar_asset_contract_v2(usdc_admin.clone());
        let amm_id = env.register(MockAmm, ());
        let amm = MockAmmClient::new(&env, &amm_id);

        env.as_contract(&amm_id, || {
            MockAmm::set_config(
                env.clone(),
                admin.clone(),
                wpi.address(),
                usdc.address(),
                1_000_000,
            );
        });

        env.mock_all_auths();
        let wpi_admin_client = token::StellarAssetClient::new(&env, &wpi.address());
        let usdc_admin_client = token::StellarAssetClient::new(&env, &usdc.address());
        wpi_admin_client.mint(&trader, &100);
        usdc_admin_client.mint(&admin, &100);

        amm.deposit_liquidity(&admin, &100);
        assert_eq!(amm.swap(&trader, &40, &40), 40);

        let wpi_client = token::Client::new(&env, &wpi.address());
        let usdc_client = token::Client::new(&env, &usdc.address());
        assert_eq!(wpi_client.balance(&trader), 60);
        assert_eq!(usdc_client.balance(&trader), 40);
        assert_eq!(usdc_client.balance(&amm_id), 60);
    }
}
