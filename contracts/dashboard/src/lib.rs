#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug)]
pub struct CreatorDashboard {
    pub invoice_count: u64,
    pub total_volume: i128,
    pub released_count: u64,
    pub refunded_count: u64,
    pub released_volume: i128,
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

fn is_authorized_split_contract(env: &Env, caller: &Address) -> bool {
    env.storage()
        .instance()
        .get(&authorized_contract_key(caller))
        .unwrap_or(false)
}

fn require_authorized_split_contract(env: &Env, caller: &Address) {
    assert!(
        is_authorized_split_contract(env, caller),
        "unauthorized split contract"
    );
    caller.require_auth();
}

fn require_admin(env: &Env) -> Address {
    let admin: Address = env
        .storage()
        .instance()
        .get(&admin_key())
        .expect("not initialized");
    admin.require_auth();
    admin
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
        // without polling contract storage (#50).
        env.events().publish(
            (symbol_short!("dash"), symbol_short!("created"), creator),
            total,
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

        // Emit event for real-time indexing of creator metrics (#50).
        env.events().publish(
            (symbol_short!("dash"), symbol_short!("released"), creator),
            total,
        );
    }

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
                });
        dashboard.refunded_count = dashboard
            .refunded_count
            .checked_add(1)
            .expect("refunded_count overflow");
        env.storage().persistent().set(&key, &dashboard);

        // Emit event for real-time indexing of creator metrics (#50).
        env.events().publish(
            (symbol_short!("dash"), symbol_short!("refunded"), creator),
            (),
        );
    }

    pub fn get_creator_dashboard(env: Env, creator: Address) -> CreatorDashboard {
        let key = creator_dashboard_key(&creator);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(CreatorDashboard {
                invoice_count: 0,
                total_volume: 0,
                released_count: 0,
                refunded_count: 0,
                released_volume: 0,
            })
    }
}

#[cfg(test)]
mod test;
