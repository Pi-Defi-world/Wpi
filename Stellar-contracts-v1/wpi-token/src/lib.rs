#![no_std]

//! Wrapped Pi (wPi) — Soroban token on **Stellar** testnet/mainnet.
//! Mint/burn is admin-only; the cross-chain relayer (see `relayer/`) mints wPi after
//! Pi deposits are observed on Pi Network, and watches burns to release Pi on redemption.
//! Same interface shape as `pusd-token` for SDK compatibility.

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
    /// Marks a Pi Network deposit id as already minted, so the relayer can
    /// safely retry `mint_from_deposit` without risking a double mint.
    ProcessedDeposit(BytesN<32>),
    /// Monotonic counter tagged onto every burn so the relayer's redemption
    /// watcher has a stable, ordered id to dedupe against.
    RedemptionNonce,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotAdmin = 1,
    Paused = 2,
    InsufficientBalance = 3,
    InsufficientAllowance = 4,
    /// `mint_from_deposit` was called again with a `pi_deposit_id` that was
    /// already processed. The relayer should treat this as success (no-op).
    DepositAlreadyProcessed = 5,
}

#[contract]
pub struct WpiToken;

/// Emitted once per successful `mint_from_deposit` call. Topic is
/// `("deposit_minted", pi_deposit_id)` — the relayer's mint submitter
/// doesn't need to observe this (it already knows the outcome of its own
/// submission), but it gives external indexers/auditors a verifiable link
/// from a Pi deposit id to the wPi it produced.
#[contractevent]
pub struct DepositMinted {
    #[topic]
    pub pi_deposit_id: BytesN<32>,
    pub to: Address,
    pub amount: i128,
}

/// Emitted once per `burn` call. Topic is `("redemption_burned", nonce)`;
/// the relayer's redemption watcher (see `relayer/`) polls for these via
/// `getEvents` and dedupes Pi-side releases by `(tx_hash, nonce)`.
#[contractevent]
pub struct RedemptionBurned {
    #[topic]
    pub nonce: u64,
    pub from: Address,
    pub amount: i128,
    pub pi_destination: BytesN<32>,
}

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

fn is_deposit_processed(env: &Env, deposit_id: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::ProcessedDeposit(deposit_id.clone()))
        .unwrap_or(false)
}

fn mark_deposit_processed(env: &Env, deposit_id: &BytesN<32>) {
    env.storage()
        .persistent()
        .set(&DataKey::ProcessedDeposit(deposit_id.clone()), &true);
}

fn next_redemption_nonce(env: &Env) -> u64 {
    let nonce = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::RedemptionNonce)
        .unwrap_or(0)
        + 1;
    env.storage()
        .instance()
        .set(&DataKey::RedemptionNonce, &nonce);
    nonce
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
        write_balance(&env, &to, to_balance + amount);
        write_total_supply(&env, total + amount);
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
