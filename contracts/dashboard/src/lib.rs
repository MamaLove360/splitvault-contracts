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
    pub refunded_volume: i128,
}

fn admin_key() -> Symbol {
    symbol_short!("admin")
}

fn authorized_contract_key(split_contract: &Address) -> (Symbol, Address) {
    (symbol_short!("auth_ctr"), split_contract.clone())
}

fn creator_dashboard_key(creator: &Address) -> (Symbol, Address) {
    (symbol_short!("cr_dash"), creator.clone())
}

/// Publish a structured event when a creator's created-invoice metrics update.
fn dashboard_record_created(env: &Env, creator: &Address, invoice_count: u64, total_volume: i128) {
    env.events().publish(
        (symbol_short!("dash"), symbol_short!("created"), creator.clone()),
        (invoice_count, total_volume),
    );
}

/// Publish a structured event when a creator's released-invoice metrics update.
fn dashboard_record_released(
    env: &Env,
    creator: &Address,
    released_count: u64,
    released_volume: i128,
) {
    env.events().publish(
        (symbol_short!("dash"), symbol_short!("released"), creator.clone()),
        (released_count, released_volume),
    );
}

/// Publish a structured event when a creator's refunded-invoice count updates.
fn dashboard_record_refunded(env: &Env, creator: &Address, refunded_count: u64) {
    env.events().publish(
        (symbol_short!("dash"), symbol_short!("refunded"), creator.clone()),
        refunded_count,
    );
}

#[contract]
pub struct DashboardContract;

#[contractimpl]
impl DashboardContract {
    pub fn initialize(env: Env, admin: Address, split_contract: Address) {
        assert!(
            !env.storage().instance().has(&admin_key()),
            "already initialized"
        );
        env.storage().instance().set(&admin_key(), &admin);
        env.storage()
            .instance()
            .set(&authorized_contract_key(&split_contract), &true);
    }

    /// Register an additional split contract instance (e.g. factory-deployed).
    /// Only the dashboard admin may authorize new split contracts.
    pub fn register_split_contract(env: Env, split_contract: Address) {
        require_admin(&env);

        env.storage()
            .instance()
            .set(&authorized_contract_key(&split_contract), &true);

        env.events().publish(
            (symbol_short!("dash"), symbol_short!("register")),
            split_contract,
        );
    }

    /// Update the authorized split contract address, e.g. after a protocol
    /// upgrade replaces the split contract. Requires admin authorization.
    pub fn set_authorized_contract(env: Env, split_contract: Address) {
        require_admin(&env);

        env.storage()
            .instance()
            .set(&authorized_contract_key(&split_contract), &true);

        env.events().publish(
            (symbol_short!("dash"), symbol_short!("set_auth")),
            split_contract,
        );
    }

    /// Rotate the dashboard admin. Requires the current admin's authorization.
    pub fn set_admin(env: Env, new_admin: Address) {
        require_admin(&env);

        env.storage().instance().set(&admin_key(), &new_admin);

        env.events().publish(
            (symbol_short!("dash"), symbol_short!("set_admin")),
            new_admin,
        );
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&admin_key())
            .expect("not initialized")
    }

    pub fn is_authorized_split_contract(env: Env, caller: Address) -> bool {
        is_authorized_split_contract(&env, &caller)
    }

    pub fn record_created(env: Env, creator: Address, total: i128) {
        require_authorized_split_contract(&env, &env.current_contract_address());

        let key = creator_dashboard_key(&creator);
        let mut dashboard: CreatorDashboard =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(CreatorDashboard {
                    invoice_count: 0,
                    total_volume: 0,
                    released_count: 0,
                    refunded_count: 0,
                    released_volume: 0,
                    refunded_volume: 0,
                });
        dashboard.invoice_count = dashboard
            .invoice_count
            .checked_add(1)
            .expect("invoice_count overflow");
        dashboard.total_volume = dashboard
            .total_volume
            .checked_add(total)
            .expect("total_volume overflow");
        env.storage().persistent().set(&key, &dashboard);

        // Emit event so external analytics pipelines can stream metric updates
        // without polling contract storage (#107).
        dashboard_record_created(
            &env,
            &creator,
            dashboard.invoice_count,
            dashboard.total_volume,
        );
    }

    pub fn record_released(env: Env, creator: Address, total: i128) {
        require_authorized_split_contract(&env, &env.current_contract_address());

        let key = creator_dashboard_key(&creator);
        let mut dashboard: CreatorDashboard =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(CreatorDashboard {
                    invoice_count: 0,
                    total_volume: 0,
                    released_count: 0,
                    refunded_count: 0,
                    released_volume: 0,
                    refunded_volume: 0,
                });
        dashboard.released_count = dashboard
            .released_count
            .checked_add(1)
            .expect("released_count overflow");
        dashboard.released_volume = dashboard
            .released_volume
            .checked_add(total)
            .expect("released_volume overflow");
        env.storage().persistent().set(&key, &dashboard);

        // Emit event for real-time indexing of creator metrics (#107).
        dashboard_record_released(
            &env,
            &creator,
            dashboard.released_count,
            dashboard.released_volume,
        );
    }

    /// Record a refunded invoice for `creator`.
    pub fn record_refunded(env: Env, creator: Address) {
        require_authorized_split_contract(&env, &env.current_contract_address());

        let key = creator_dashboard_key(&creator);
        let mut dashboard: CreatorDashboard =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(CreatorDashboard {
                    invoice_count: 0,
                    total_volume: 0,
                    released_count: 0,
                    refunded_count: 0,
                    released_volume: 0,
                    refunded_volume: 0,
                });
        dashboard.refunded_count = dashboard
            .refunded_count
            .checked_add(1)
            .expect("refunded_count overflow");
        dashboard.refunded_volume = dashboard
            .refunded_volume
            .checked_add(total)
            .expect("refunded_volume overflow");
        env.storage().persistent().set(&key, &dashboard);

        // Emit event for real-time indexing of creator metrics (#107).
        dashboard_record_refunded(&env, &creator, dashboard.refunded_count);
    }

    /// Read the aggregated dashboard for `creator`.
    pub fn get_creator_dashboard(env: Env, creator: Address) -> CreatorDashboard {
        Self::load(&env, &creator)
    }
}
