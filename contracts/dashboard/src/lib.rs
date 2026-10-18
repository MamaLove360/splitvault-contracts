//! Dashboard contract: aggregates per-creator invoice metrics.
//!
//! The split contract calls into this contract to record invoice lifecycle
//! events. Metrics are stored per creator so the frontend can render a
//! dashboard without scanning the full invoice history.

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CreatorDashboard {
    pub invoice_count: u64,
    pub total_volume: i128,
    pub released_count: u64,
    pub refunded_count: u64,
    pub released_volume: i128,
}

#[contract]
pub struct DashboardContract;

#[contractimpl]
impl DashboardContract {
    /// One-time initialization. Stores the authorized split contract address.
    pub fn initialize(env: Env, split_contract: Address) {
        if env.storage().instance().has(&symbol_short!("split")) {
            panic!("already initialized");
        }
        env.storage()
            .instance()
            .set(&symbol_short!("split"), &split_contract);
    }

    fn require_split(env: &Env) -> Address {
        let split: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("split"))
            .expect("not initialized");
        split.require_auth();
        split
    }

    fn load(env: &Env, creator: &Address) -> CreatorDashboard {
        env.storage()
            .persistent()
            .get(&(symbol_short!("dash"), creator.clone()))
            .unwrap_or_default()
    }

    fn save(env: &Env, creator: &Address, dash: &CreatorDashboard) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("dash"), creator.clone()), dash);
    }

    /// Record a newly created invoice for `creator`.
    pub fn record_created(env: Env, creator: Address, amount: i128) {
        Self::require_split(&env);

        let mut dash = Self::load(&env, &creator);
        dash.invoice_count += 1;
        dash.total_volume += amount;
        Self::save(&env, &creator, &dash);

        env.events()
            .publish((symbol_short!("dash"), symbol_short!("created")), (creator, amount));
    }

    /// Record a single released invoice for `creator`.
    pub fn record_released(env: Env, creator: Address, amount: i128) {
        Self::require_split(&env);

        let mut dash = Self::load(&env, &creator);
        dash.released_count += 1;
        dash.released_volume += amount;
        Self::save(&env, &creator, &dash);

        env.events()
            .publish((symbol_short!("dash"), symbol_short!("released")), (creator, amount));
    }

    /// Record a batch of released invoices for `creator` in a single call.
    ///
    /// Increments `released_count` by `count` and `released_volume` by
    /// `total_volume`, avoiding one cross-contract invocation per invoice.
    pub fn record_batch_released(env: Env, creator: Address, count: u64, total_volume: i128) {
        Self::require_split(&env);

        let mut dash = Self::load(&env, &creator);
        dash.released_count += count;
        dash.released_volume += total_volume;
        Self::save(&env, &creator, &dash);

        env.events().publish(
            (symbol_short!("dash"), symbol_short!("batch_rel")),
            (creator, count, total_volume),
        );
    }

    /// Record a refunded invoice for `creator`.
    pub fn record_refunded(env: Env, creator: Address) {
        Self::require_split(&env);

        let mut dash = Self::load(&env, &creator);
        dash.refunded_count += 1;
        Self::save(&env, &creator, &dash);

        env.events()
            .publish((symbol_short!("dash"), symbol_short!("refunded")), creator);
    }

    /// Read the aggregated dashboard for `creator`.
    pub fn get_creator_dashboard(env: Env, creator: Address) -> CreatorDashboard {
        Self::load(&env, &creator)
    }
}
