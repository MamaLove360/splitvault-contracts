#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, Vec};
use soroban_sdk::testutils::token::MockToken;

use split_contracts::contract::{SplitContractClient, InvoiceOptions};

#[test]
fn test_full_invoice_lifecycle() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, split_contracts::contract::SplitContract);
    let client = SplitContractClient::new(&e, &contract_id);

    // Setup
    let creator = Address::generate(&e);
    let payer = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token = MockToken::new(&e, &Address::generate(&e));

    // Create
    let mut recipients = Vec::new(&e);
    recipients.push_back(recipient.clone());
    let mut amounts = Vec::new(&e);
    amounts.push_back(1000);

    let options = InvoiceOptions::default();
    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token.address, &10000, &options);

    // Pay
    token.mint(&payer, &1000);
    client.pay(&payer, &invoice_id, &1000);

    // Release
    client.release(&invoice_id);

    // Assert
    assert_eq!(token.balance(&recipient), 1000);
}
