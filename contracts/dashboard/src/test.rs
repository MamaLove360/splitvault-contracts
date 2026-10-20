#![cfg(test)]
#![allow(clippy::all)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let split_id = Address::generate(&env);
    let admin = Address::generate(&env);
    let dashboard_id = env.register(DashboardContract, ());

    let client = DashboardContractClient::new(&env, &dashboard_id);
    client.initialize(&split_id, &admin);

    (env, split_id, dashboard_id)
}

#[test]
fn test_initial_dashboard_empty() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);
    let dash = client.get_creator_dashboard(&creator);

    assert_eq!(dash.invoice_count, 0);
    assert_eq!(dash.total_volume, 0);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 0);
}

#[test]
fn test_record_created_updates_stats() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&creator, &1000_i128);
    client.record_created(&creator, &2000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 2);
    assert_eq!(dash.total_volume, 3000);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 0);
}

#[test]
fn test_record_released_updates_stats() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&creator, &5000_i128);
    client.record_released(&creator, &3000_i128);
    client.record_released(&creator, &2000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 5000);
    assert_eq!(dash.released_count, 2);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 5000);
}

#[test]
fn test_record_refunded_updates_stats() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&creator, &1000_i128);
    client.record_refunded(&creator, &1000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 1000);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 1);
    assert_eq!(dash.released_volume, 0);
    assert_eq!(dash.refunded_volume, 1000);
}

#[test]
fn test_stats_are_per_creator() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    client.record_created(&creator_a, &100_i128);
    client.record_created(&creator_a, &200_i128);
    client.record_created(&creator_b, &500_i128);
    client.record_released(&creator_a, &300_i128);
    client.record_refunded(&creator_b, &500_i128);

    let dash_a = client.get_creator_dashboard(&creator_a);
    assert_eq!(dash_a.invoice_count, 2);
    assert_eq!(dash_a.total_volume, 300);
    assert_eq!(dash_a.released_count, 1);
    assert_eq!(dash_a.refunded_count, 0);
    assert_eq!(dash_a.released_volume, 300);

    let dash_b = client.get_creator_dashboard(&creator_b);
    assert_eq!(dash_b.invoice_count, 1);
    assert_eq!(dash_b.total_volume, 500);
    assert_eq!(dash_b.released_count, 0);
    assert_eq!(dash_b.refunded_count, 1);
    assert_eq!(dash_b.released_volume, 0);
}

#[test]
#[should_panic(expected = "HostError")]
fn test_unauthorized_caller_rejected() {
    let env = Env::default();

    let split_id = Address::generate(&env);
    let admin = Address::generate(&env);
    let dashboard_id = env.register(DashboardContract, ());

    let client = DashboardContractClient::new(&env, &dashboard_id);
    client.initialize(&split_id, &admin);

    let creator = Address::generate(&env);

    // Call without mock_all_auths — require_auth() on the authorized address will fail
    // because the test caller is not the authorized split contract.
    DashboardContractClient::new(&env, &dashboard_id).record_created(&creator, &100_i128);
}

// ---------------------------------------------------------------------------
// #83 — Multi-instance split contract registry tests
// ---------------------------------------------------------------------------

/// A second split contract registered by the admin can report metrics (#83).
#[test]
fn test_multiple_split_contracts_can_report() {
    let (env, split_a, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let split_b = Address::generate(&env);
    client.register_split_contract(&split_b);

    assert!(client.is_authorized_split_contract(&split_a));
    assert!(client.is_authorized_split_contract(&split_b));

    let creator = Address::generate(&env);
    client.record_created(&creator, &1000_i128);
    client.record_created(&creator, &2000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 2);
    assert_eq!(dash.total_volume, 3000);
}

/// Registering the same split contract twice is idempotent (#83).
#[test]
fn test_register_split_contract_idempotent() {
    let (env, split_a, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    client.register_split_contract(&split_a);
    assert!(client.is_authorized_split_contract(&split_a));
}

/// An unregistered split contract is not authorized (#83).
#[test]
fn test_unregistered_split_contract_not_authorized() {
    let (env, _split_a, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let stranger = Address::generate(&env);
    assert!(!client.is_authorized_split_contract(&stranger));
}

// ---------------------------------------------------------------------------
// #84 — Admin rotation and authorized contract management tests
// ---------------------------------------------------------------------------

/// The admin can update the authorized contract, and the new address can report (#84).
#[test]
fn test_set_authorized_contract_as_admin() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let new_split = Address::generate(&env);
    client.set_authorized_contract(&new_split);

    assert!(client.is_authorized_split_contract(&new_split));

    let creator = Address::generate(&env);
    client.record_created(&creator, &1000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 1000);
}

/// The admin can rotate the admin address (#84).
#[test]
fn test_set_admin_rotates_admin() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);

    // New admin can still perform administrative actions.
    let new_split = Address::generate(&env);
    client.set_authorized_contract(&new_split);
    assert!(client.is_authorized_split_contract(&new_split));
}

// ---------------------------------------------------------------------------
// #50 — Dashboard event emission tests
// ---------------------------------------------------------------------------

use soroban_sdk::testutils::Events as _;
use soroban_sdk::{symbol_short, TryFromVal, Val};

fn find_dash_event(env: &Env, action: soroban_sdk::Symbol) -> bool {
    let events = env.events().all();
    for event in events.events().iter() {
        let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
        let mut topics = soroban_sdk::Vec::new(env);
        let topics_ref: &[soroban_sdk::xdr::ScVal] = body.topics.as_ref();
        for v in topics_ref.iter() {
            if let Ok(val) = <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(env, v) {
                topics.push_back(val);
            }
        }
        if topics.len() >= 2 {
            let t0 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(0));
            let t1 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(1));
            if let (Ok(topic0), Ok(topic1)) = (t0, t1) {
                if topic0 == symbol_short!("dash") && topic1 == action {
                    return true;
                }
            }
        }
    }
    false
}

/// `record_created` must emit `(dash, created, creator)` (#50).
#[test]
fn test_record_created_emits_event() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&creator, &500_i128);

    assert!(
        find_dash_event(&env, symbol_short!("created")),
        "record_created did not emit (dash, created) event",
    );
}

/// `record_released` must emit `(dash, released, creator)` (#50).
#[test]
fn test_record_released_emits_event() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&creator, &500_i128);
    client.record_released(&creator, &500_i128);

    assert!(
        find_dash_event(&env, symbol_short!("released")),
        "record_released did not emit (dash, released) event",
    );
}

/// `record_refunded` must emit `(dash, refunded, creator)` (#50).
#[test]
fn test_record_refunded_emits_event() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_refunded(&creator, &100_i128);

    assert!(
        find_dash_event(&env, symbol_short!("refunded")),
        "record_refunded did not emit (dash, refunded) event",
    );
}

// ---------------------------------------------------------------------------
// #107 — Dashboard event payload tests
// ---------------------------------------------------------------------------

/// Collect the data payload of the first `(dash, action)` event as a tuple of
/// `(creator, count, volume)` where applicable.
fn find_dash_event_data(
    env: &Env,
    action: soroban_sdk::Symbol,
) -> Option<(Address, i128, i128)> {
    let events = env.events().all();
    for event in events.events().iter() {
        let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
        let mut topics = soroban_sdk::Vec::new(env);
        let topics_ref: &[soroban_sdk::xdr::ScVal] = body.topics.as_ref();
        for v in topics_ref.iter() {
            if let Ok(val) = <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(env, v) {
                topics.push_back(val);
            }
        }
        if topics.len() >= 2 {
            let t0 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(0));
            let t1 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(1));
            if let (Ok(topic0), Ok(topic1)) = (t0, t1) {
                if topic0 == symbol_short!("dash") && topic1 == action {
                    let data_val = <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(
                        env,
                        &body.data,
                    )
                    .ok()?;
                    let tuple = <(Address, i128, i128) as TryFromVal<Env, Val>>::try_from_val(
                        env, &data_val,
                    )
                    .ok()?;
                    return Some(tuple);
                }
            }
        }
    }
    None
}

/// `record_created` event data must carry creator, invoice_count, total_volume (#107).
#[test]
fn test_record_created_event_payload() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&creator, &1000_i128);
    client.record_created(&creator, &2000_i128);

    let (ev_creator, count, volume) =
        find_dash_event_data(&env, symbol_short!("created")).expect("missing created event");
    assert_eq!(ev_creator, creator);
    assert_eq!(count, 2);
    assert_eq!(volume, 3000);
}

/// `record_released` event data must carry creator, released_count, released_volume (#107).
#[test]
fn test_record_released_event_payload() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_released(&creator, &3000_i128);
    client.record_released(&creator, &2000_i128);

    let (ev_creator, count, volume) =
        find_dash_event_data(&env, symbol_short!("released")).expect("missing released event");
    assert_eq!(ev_creator, creator);
    assert_eq!(count, 2);
    assert_eq!(volume, 5000);
}

/// `record_refunded` event data must carry creator and refunded_count (#107).
#[test]
fn test_record_refunded_event_payload() {
    let (env, _split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_refunded(&creator);
    client.record_refunded(&creator);

    let (ev_creator, count, _volume) =
        find_dash_event_data(&env, symbol_short!("refunded")).expect("missing refunded event");
    assert_eq!(ev_creator, creator);
    assert_eq!(count, 2);
}
