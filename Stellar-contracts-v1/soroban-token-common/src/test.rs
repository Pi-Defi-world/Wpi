extern crate std;

use super::{
    burn_token, initialize_token, mint_token, read_balance, read_total_supply, set_paused_token,
    Error,
};
use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env};

#[contract]
struct TestToken;

#[contractimpl]
impl TestToken {
    pub fn initialize(env: Env, admin: Address) {
        initialize_token(&env, &admin);
    }

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        mint_token(&env, &admin, &to, amount)
    }

    pub fn burn(env: Env, admin: Address, from: Address, amount: i128) -> Result<(), Error> {
        burn_token(&env, &admin, &from, amount)
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        set_paused_token(&env, &admin, paused)
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        read_balance(&env, &account)
    }

    pub fn total_supply(env: Env) -> i128 {
        read_total_supply(&env)
    }
}

#[test]
fn pause_blocks_mint_and_burn_and_unpause_restores_both() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let holder = Address::generate(&env);
    let contract_id = env.register(TestToken, ());
    let client = TestTokenClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.mint(&admin, &holder, &100);
    client.set_paused(&admin, &true);

    assert_eq!(
        client.try_mint(&admin, &holder, &25),
        Err(Ok(Error::Paused))
    );
    assert_eq!(
        client.try_burn(&admin, &holder, &25),
        Err(Ok(Error::Paused))
    );
    assert_eq!(client.balance(&holder), 100);
    assert_eq!(client.total_supply(), 100);

    client.set_paused(&admin, &false);
    client.mint(&admin, &holder, &10);
    assert_eq!(client.balance(&holder), 110);
    assert_eq!(client.total_supply(), 110);

    client.burn(&admin, &holder, &25);
    assert_eq!(client.balance(&holder), 85);
    assert_eq!(client.total_supply(), 85);
}
