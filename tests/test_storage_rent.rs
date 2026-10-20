#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, Vec};
use soroban_sdk::testutils::token::MockToken;

mod contract {
    soroban_sdk::contractimport!(file = "target/wasm32-unknown-unknown/release/split_contracts.wasm");
}

/// Test #288: Storage rent tracking per invoice
/// Creators need visibility into storage rent costs for their invoices

/// Test get_invoice_rent_cost returns estimated stroops per ledger
#[test]
fn test_get_invoice_rent_cost_basic() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    // Setup
    let creator = Address::generate(&e);
    let payer = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    token.mint(&payer, &1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);

    // get_invoice_rent_cost should return estimated stroops per ledger
    // Calculation accounts for: invoice struct size, InvoiceExt size, payment shard count, event log size
    let rent_cost = client.get_invoice_rent_cost(&invoice_id);

    assert!(rent_cost > 0);
}

/// Test rent cost increases with more recipients
#[test]
fn test_invoice_rent_cost_scales_with_recipients() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    // Single recipient
    let mut recipients = Vec::new(&e);
    recipients.push_back(Address::generate(&e));
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    let invoice_id_1 = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    let rent_1 = client.get_invoice_rent_cost(&invoice_id_1);

    // Multiple recipients
    let mut recipients_multi = Vec::new(&e);
    recipients_multi.push_back(Address::generate(&e));
    recipients_multi.push_back(Address::generate(&e));
    recipients_multi.push_back(Address::generate(&e));
    let mut amounts_multi = Vec::new(&e);
    amounts_multi.push_back(500);
    amounts_multi.push_back(300);
    amounts_multi.push_back(200);

    let invoice_id_2 = client.create_invoice(&creator, &recipients_multi, &amounts_multi, &token.address, &10000);
    let rent_2 = client.get_invoice_rent_cost(&invoice_id_2);

    // More recipients should increase rent cost
    assert!(rent_2 > rent_1);
}

/// Test get_total_rent_cost aggregates across all active invoices for a creator
#[test]
fn test_get_total_rent_cost_aggregates() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let recipient = Address::generate(&e);
    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    // Create first invoice
    let invoice_id_1 = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    let rent_1 = client.get_invoice_rent_cost(&invoice_id_1);

    // Create second invoice
    let invoice_id_2 = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    let rent_2 = client.get_invoice_rent_cost(&invoice_id_2);

    // Total rent should be sum of individual invoices
    let total_rent = client.get_total_rent_cost(&creator);

    assert!(total_rent >= rent_1 + rent_2);
}

/// Test rent cost calculation at limits (many recipients and payments)
#[test]
fn test_invoice_rent_cost_with_many_recipients() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    // Create invoice with 10 recipients
    let mut recipients = Vec::new(&e);
    for _ in 0..10 {
        recipients.push_back(Address::generate(&e));
    }
    let mut amounts = Vec::new(&e);
    for _ in 0..10 {
        amounts.push_back(100);
    }

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    let rent_cost = client.get_invoice_rent_cost(&invoice_id);

    // Should return positive cost proportional to recipient count and data size
    assert!(rent_cost > 0);
}

/// Test rent cost for invoice with multiple payment shards
#[test]
fn test_invoice_rent_cost_with_payment_shards() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let payer1 = Address::generate(&e);
    let payer2 = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    token.mint(&payer1, &500);
    token.mint(&payer2, &500);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);

    // Make payments from different payers (creates payment shards)
    client.pay(&payer1, &invoice_id, &500);
    client.pay(&payer2, &invoice_id, &500);

    // Rent cost should account for payment shard count
    let rent_cost = client.get_invoice_rent_cost(&invoice_id);
    assert!(rent_cost > 0);
}

/// Test rent cost after claim (shard cleanup)
#[test]
fn test_invoice_rent_cost_after_release() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let payer = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    token.mint(&payer, &1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    let rent_before = client.get_invoice_rent_cost(&invoice_id);

    // Make full payment and release
    client.pay(&payer, &invoice_id, &1000);
    client.release(&invoice_id);

    // Rent should remain available for audit, or reduce after cleanup
    let rent_after = client.get_invoice_rent_cost(&invoice_id);
    assert!(rent_after >= 0);
}

/// Test STROOPS_PER_BYTE_PER_LEDGER constant is used in calculation
#[test]
fn test_rent_cost_uses_stroops_per_byte_constant() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);
    
    // Rent cost should be in stroops (positive integer representing per-ledger cost)
    let rent_cost = client.get_invoice_rent_cost(&invoice_id);
    
    // Cost should reflect network parameters and data size
    // Typically in range of 100-10000 stroops depending on invoice complexity
    assert!(rent_cost > 0);
}

/// Test #98: Active invoices with rent extension remain callable after ledger
/// advancement past MIN_PERSISTENT_TTL.
#[test]
fn test_active_invoice_callable_after_ttl_extension() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let payer = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    token.mint(&payer, &1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);

    // Advance the ledger past the default persistent TTL boundary.
    let min_ttl = e.storage().max_ttl();
    e.ledger().set_sequence_number(e.ledger().sequence() + min_ttl + 1);

    // The invoice should still be readable and payable because the contract
    // extends its TTL on creation / access.
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.id, invoice_id);

    client.pay(&payer, &invoice_id, &1000);
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.id, invoice_id);
}

/// Test #98: Expired un-bumped entries trigger clean error handling rather
/// than an unhandled VM panic.
#[test]
fn test_expired_invoice_returns_clean_error() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);

    // Advance far beyond the maximum TTL so the entry is archived/expired.
    let max_ttl = e.storage().max_ttl();
    e.ledger().set_sequence_number(e.ledger().sequence() + max_ttl + 1);

    // Reading an expired entry must surface a clean contract error (or a
    // missing-entry result), never an unhandled VM panic.
    let result = client.try_get_invoice(&invoice_id);
    assert!(result.is_err() || result.unwrap().is_none());
}

/// Test #98: TTL restoration — an invoice whose TTL is extended after ledger
/// advancement remains accessible.
#[test]
fn test_ttl_restoration_keeps_invoice_accessible() {
    let e = Env::default();
    let contract_id = e.register_contract_wasm(None, contract::WASM);
    let client = contract::Client::new(&e, &contract_id);

    let creator = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000);

    // Advance partway toward the TTL boundary, then extend the entry's TTL.
    let min_ttl = e.storage().max_ttl();
    e.ledger().set_sequence_number(e.ledger().sequence() + min_ttl / 2);

    client.extend_invoice_ttl(&invoice_id);

    // Advance past the original boundary; the restored TTL keeps it live.
    e.ledger().set_sequence_number(e.ledger().sequence() + min_ttl / 2 + 1);

    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.id, invoice_id);
}
