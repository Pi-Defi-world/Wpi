use super::{MockUsdcToken, MockUsdcTokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env};
use soroban_token_common::Error;

#[test]
fn pause_blocks_mint_and_burn_without_changing_supply() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let holder = Address::generate(&env);
    let contract_id = env.register(MockUsdcToken, ());
    let client = MockUsdcTokenClient::new(&env, &contract_id);

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
    client.burn(&admin, &holder, &25);
    assert_eq!(client.balance(&holder), 75);
    assert_eq!(client.total_supply(), 75);
}
