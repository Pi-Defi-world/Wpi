use super::*;
use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::IntoVal;

fn deposit_id(env: &Env, tag: u8) -> BytesN<32> {
    BytesN::from_array(env, &[tag; 32])
}

fn setup(
    env: &Env,
    mint_limit: i128,
    burn_limit: i128,
    window_seconds: u64,
) -> (Address, WpiTokenClient<'_>, Address) {
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);
    let admin = Address::generate(env);
    let user = Address::generate(env);
    let contract_id = env.register(WpiToken, ());
    let client = WpiTokenClient::new(env, &contract_id);
    client.initialize(&admin);
    client.configure_volume_limits(&mint_limit, &burn_limit, &window_seconds);
    (admin, client, user)
}

#[test]
fn bridge_operations_fail_closed_until_limits_are_configured() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register(WpiToken, ());
    let client = WpiTokenClient::new(&env, &contract_id);
    client.initialize(&admin);

    let result = client.try_mint_from_deposit(&user, &1, &deposit_id(&env, 1));

    assert_eq!(result, Err(Ok(Error::VolumeLimitsNotConfigured)));
    assert_eq!(client.balance(&user), 0);
}

#[test]
fn mint_limit_trips_breaker_emits_event_and_halts_activity() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 100, 1_000, 86_400);

    client.mint_from_deposit(&user, &60, &deposit_id(&env, 1));
    client.mint_from_deposit(&user, &40, &deposit_id(&env, 2));
    // The triggering invocation emits both VolumeLimitTriggered and DepositMinted.
    assert_eq!(env.events().all().len(), 2);

    assert_eq!(client.balance(&user), 100);
    assert!(client.paused());
    assert!(client.circuit_breaker_active());

    let blocked = client.try_mint_from_deposit(&user, &1, &deposit_id(&env, 3));
    assert_eq!(blocked, Err(Ok(Error::Paused)));
    assert!(!client.is_deposit_processed(&deposit_id(&env, 3)));
    assert_eq!(client.balance(&user), 100);
}

#[test]
fn mint_that_would_exceed_limit_is_rejected_but_alert_is_committed() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 100, 1_000, 86_400);
    client.mint_from_deposit(&user, &60, &deposit_id(&env, 1));

    let accepted = client.mint_from_deposit(&user, &41, &deposit_id(&env, 2));

    assert!(!accepted);
    assert_eq!(env.events().all().len(), 1);
    assert_eq!(client.balance(&user), 60);
    assert_eq!(client.total_supply(), 60);
    assert_eq!(client.current_volume_window().minted, 60);
    assert!(!client.is_deposit_processed(&deposit_id(&env, 2)));
    assert!(client.circuit_breaker_active());
    assert!(client.paused());
}

#[test]
fn burn_limit_is_tracked_independently_and_halts_activity() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 1_000, 100, 86_400);
    let destination = BytesN::from_array(&env, &[9; 32]);
    client.mint_from_deposit(&user, &200, &deposit_id(&env, 1));

    client.burn(&user, &60, &destination);
    client.burn(&user, &40, &destination);

    assert_eq!(client.balance(&user), 100);
    assert!(client.paused());
    assert_eq!(client.current_volume_window().burned, 100);
    assert_eq!(client.current_volume_window().minted, 200);

    let blocked = client.try_burn(&user, &1, &destination);
    assert_eq!(blocked, Err(Ok(Error::Paused)));
    assert_eq!(client.balance(&user), 100);
}

#[test]
fn expired_window_resets_volume_before_next_operation() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 100, 100, 10);
    client.mint_from_deposit(&user, &60, &deposit_id(&env, 1));

    env.ledger().set_timestamp(1_011);
    client.mint_from_deposit(&user, &60, &deposit_id(&env, 2));

    let window = client.current_volume_window();
    assert_eq!(window.started_at, 1_001);
    assert_eq!(window.minted, 60);
    assert!(!client.paused());
    assert_eq!(client.balance(&user), 120);
}

#[test]
fn rolling_window_counts_volume_across_time_buckets() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 100, 100, 10);
    client.mint_from_deposit(&user, &60, &deposit_id(&env, 1));

    env.ledger().set_timestamp(1_009);
    client.mint_from_deposit(&user, &40, &deposit_id(&env, 2));

    assert_eq!(client.current_volume_window().minted, 100);
    assert!(client.circuit_breaker_active());
    assert!(client.paused());
}

#[test]
fn rolling_window_does_not_expire_volume_early_at_bucket_boundary() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 100, 100, 86_400);
    env.ledger().set_timestamp(3_599);
    client.mint_from_deposit(&user, &60, &deposit_id(&env, 1));

    // This is only 82,801 seconds later, even though it is 24 bucket indexes
    // ahead. The safety bucket must keep the first mint in the rolling total.
    env.ledger().set_timestamp(86_400);
    client.mint_from_deposit(&user, &40, &deposit_id(&env, 2));

    assert_eq!(client.current_volume_window().minted, 100);
    assert!(client.circuit_breaker_active());
}

#[test]
fn only_override_can_lift_a_tripped_circuit_breaker() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 50, 100, 86_400);
    client.mint_from_deposit(&user, &50, &deposit_id(&env, 1));

    let ordinary_unpause = client.try_set_paused(&false);
    assert_eq!(ordinary_unpause, Err(Ok(Error::CircuitBreakerActive)));

    client.override_volume_limit();
    assert!(!client.paused());
    assert!(!client.circuit_breaker_active());
    assert_eq!(client.current_volume_window().minted, 0);
    assert_eq!(client.current_volume_window().burned, 0);

    client.mint_from_deposit(&user, &10, &deposit_id(&env, 2));
    assert_eq!(client.balance(&user), 60);
}

/// Regardless of which address signs the transaction, only the address read
/// from storage (`read_admin`/`read_volume_limit_admin`) can ever satisfy
/// `require_auth`. Since these functions no longer accept an admin argument,
/// there is nothing left for a caller to "pass" that could stand in for the
/// real admin -- the only way to reach the privileged branch is to be the
/// stored admin.
#[test]
#[should_panic]
fn non_admin_signer_cannot_authenticate_mint() {
    let env = Env::default();
    let (_admin, client, user) = setup(&env, 10, 10, 10);
    let attacker = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "mint",
                args: (&user, &1i128).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .mint(&user, &1);
}

#[test]
#[should_panic]
fn non_admin_signer_cannot_authenticate_configure_volume_limits() {
    let env = Env::default();
    let (_admin, client, _user) = setup(&env, 10, 10, 10);
    let attacker = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "configure_volume_limits",
                args: (&20i128, &20i128, &20u64).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .configure_volume_limits(&20, &20, &20);
}

#[test]
#[should_panic]
fn non_admin_signer_cannot_authenticate_override_volume_limit() {
    let env = Env::default();
    let (_admin, client, _user) = setup(&env, 10, 10, 10);
    let attacker = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "override_volume_limit",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .override_volume_limit();
}

#[test]
fn volume_limit_admin_is_independent_from_bridge_admin() {
    let env = Env::default();
    let (bridge_admin, client, user) = setup(&env, 50, 100, 86_400);
    let guardian = Address::generate(&env);
    client.set_volume_limit_admin(&guardian);

    assert_eq!(client.volume_limit_admin(), guardian);
    assert_eq!(client.admin(), bridge_admin);

    client.mint_from_deposit(&user, &50, &deposit_id(&env, 1));
    assert!(client.circuit_breaker_active());

    // Only the volume-limit admin (guardian), not the bridge admin, can lift
    // the circuit breaker.
    client.override_volume_limit();
    assert!(!client.circuit_breaker_active());
    assert!(!client.paused());
}

/// Demonstrates that the bridge admin role and the volume-limit admin role
/// are enforced independently from stored state: after the volume-limit role
/// is rotated to `guardian`, the (still valid, still-a-real-admin)
/// `bridge_admin` address can no longer authenticate volume-limit-gated
/// calls, even though it could before the rotation.
#[test]
#[should_panic]
fn bridge_admin_cannot_authenticate_as_volume_limit_admin_after_rotation() {
    let env = Env::default();
    let (bridge_admin, client, _user) = setup(&env, 50, 100, 86_400);
    let guardian = Address::generate(&env);
    client.set_volume_limit_admin(&guardian);

    client
        .mock_auths(&[MockAuth {
            address: &bridge_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "override_volume_limit",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .override_volume_limit();
}

#[test]
fn invalid_limit_configuration_is_rejected() {
    let env = Env::default();
    let (admin, client, _user) = setup(&env, 10, 10, 10);

    assert_eq!(
        client.try_configure_volume_limits(&0, &10, &10),
        Err(Ok(Error::InvalidVolumeLimit))
    );
    assert_eq!(
        client.try_configure_volume_limits(&10, &10, &0),
        Err(Ok(Error::InvalidVolumeLimit))
    );
}

#[test]
fn deposit_idempotency_is_preserved() {
    let env = Env::default();
    let (admin, client, user) = setup(&env, 1_000, 1_000, 86_400);
    let deposit = deposit_id(&env, 1);
    client.mint_from_deposit(&user, &100, &deposit);

    let retry = client.try_mint_from_deposit(&user, &100, &deposit);

    assert_eq!(retry, Err(Ok(Error::DepositAlreadyProcessed)));
    assert_eq!(client.balance(&user), 100);
    assert_eq!(client.current_volume_window().minted, 100);
}

const NUM_USERS: u8 = 4;

#[derive(Clone, Debug)]
enum Op {
    Mint(u8, i128),
    Burn(u8, i128),
    Transfer(u8, u8, i128),
}

fn user_index() -> impl Strategy<Value = u8> {
    0..NUM_USERS
}

fn amount_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        3 => 0i128..=1_000_000i128,
        2 => (i128::MAX - 1_000_000)..=i128::MAX,
        1 => i128::MIN..=-1i128,
    ]
}

fn operation_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (user_index(), amount_strategy()).prop_map(|(user, amount)| Op::Mint(user, amount)),
        (user_index(), amount_strategy()).prop_map(|(user, amount)| Op::Burn(user, amount)),
        (user_index(), user_index(), amount_strategy())
            .prop_map(|(from, to, amount)| Op::Transfer(from, to, amount)),
    ]
}

fn property_setup(
    env: &Env,
) -> (
    WpiTokenClient<'_>,
    Address,
    [Address; NUM_USERS as usize],
    BytesN<32>,
) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let users = core::array::from_fn(|_| Address::generate(env));
    let destination = BytesN::from_array(env, &[9; 32]);
    let contract_id = env.register(WpiToken, ());
    let client = WpiTokenClient::new(env, &contract_id);
    client.initialize(&admin);
    client.configure_volume_limits(&i128::MAX, &i128::MAX, &86_400);
    (client, admin, users, destination)
}

fn assert_supply_invariant(client: &WpiTokenClient<'_>, users: &[Address; NUM_USERS as usize]) {
    let sum = users
        .iter()
        .fold(0i128, |total, user| total + client.balance(user));
    assert_eq!(sum, client.total_supply());
}

proptest! {
    #[test]
    fn arbitrary_operations_preserve_total_supply(
        operations in prop::collection::vec(operation_strategy(), 0..30)
    ) {
        let env = Env::default();
        let (client, admin, users, destination) = property_setup(&env);

        for operation in operations {
            match operation {
                Op::Mint(user, amount) => {
                    let _ = client.try_mint(&users[user as usize], &amount);
                }
                Op::Burn(user, amount) => {
                    let _ = client.try_burn(&users[user as usize], &amount, &destination);
                }
                Op::Transfer(from, to, amount) => {
                    let owner = users[from as usize].clone();
                    let _ = client.try_transfer(&owner, &users[to as usize], &amount);
                }
            }
            assert_supply_invariant(&client, &users);
        }
    }

    #[test]
    fn self_transfer_never_changes_balance(
        user_index in user_index(),
        mint_amount in 1i128..=1_000_000_000i128,
        transfer_amount in amount_strategy(),
    ) {
        let env = Env::default();
        let (client, admin, users, _destination) = property_setup(&env);
        let user = users[user_index as usize].clone();
        client.mint(&user, &mint_amount);
        let before = client.balance(&user);

        let _ = client.try_transfer(&user, &user, &transfer_amount);

        prop_assert_eq!(client.balance(&user), before);
        assert_supply_invariant(&client, &users);
    }
}
