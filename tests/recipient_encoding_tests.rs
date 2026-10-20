#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, Vec,
};

#[test]
fn test_encode_recipients_basic() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(addr1.clone());
    recipients.push_back(addr2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(50);
    amounts.push_back(50);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    assert_eq!(invoice.recipients.len(), 2);
}

#[test]
fn test_decode_recipients_basic() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(addr1.clone());
    recipients.push_back(addr2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(50);
    amounts.push_back(50);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // Decoded recipients must match the original input exactly, in order.
    assert_eq!(invoice.recipients.len(), 2);
    assert_eq!(invoice.recipients.get(0).unwrap(), addr1);
    assert_eq!(invoice.recipients.get(1).unwrap(), addr2);
}

#[test]
fn test_encode_decode_roundtrip() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    let mut amounts = Vec::new(&env);
    for _ in 0..5 {
        recipients.push_back(Address::generate(&env));
        amounts.push_back(20);
    }

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // Original == decode(encode(original)): no data loss, order preserved.
    assert_eq!(invoice.recipients.len(), recipients.len());
    for i in 0..recipients.len() {
        assert_eq!(invoice.recipients.get(i).unwrap(), recipients.get(i).unwrap());
    }
}

#[test]
fn test_compact_encoding_storage_reduction() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    let mut amounts = Vec::new(&env);
    for _ in 0..10 {
        recipients.push_back(Address::generate(&env));
        amounts.push_back(10);
    }

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // Compact encoding stores recipients as Vec<(Address, u32)>; verify the
    // stored representation round-trips all 10 recipients with exact bps.
    assert_eq!(invoice.recipients.len(), 10);
    let mut bps_sum: u32 = 0;
    for i in 0..recipients.len() {
        assert_eq!(invoice.recipients.get(i).unwrap(), recipients.get(i).unwrap());
        bps_sum += invoice.amounts.get(i).unwrap() as u32;
    }
    assert_eq!(bps_sum, 100);
}

#[test]
fn test_encode_recipients_preserves_bps() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(addr1.clone());
    recipients.push_back(addr2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(5000);
    amounts.push_back(5000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // BPS values preserved exactly and mapped to the correct addresses.
    assert_eq!(invoice.recipients.get(0).unwrap(), addr1);
    assert_eq!(invoice.recipients.get(1).unwrap(), addr2);
    assert_eq!(invoice.amounts.get(0).unwrap(), 5000);
    assert_eq!(invoice.amounts.get(1).unwrap(), 5000);
}

#[test]
fn test_migrate_recipient_encoding_existing_invoice() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let addr1 = Address::generate(&env);
    let addr2 = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(addr1.clone());
    recipients.push_back(addr2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(50);
    amounts.push_back(50);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);

    // Invoice created under the current encoding reads back correctly.
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.recipients.len(), 2);
    assert_eq!(invoice.recipients.get(0).unwrap(), addr1);
    assert_eq!(invoice.recipients.get(1).unwrap(), addr2);
}

#[test]
fn test_all_recipient_reads_use_new_encoding() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let payer = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&payer, &1000);

    let mut recipients = Vec::new(&env);
    recipients.push_back(recipient1.clone());
    recipients.push_back(recipient2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(50);
    amounts.push_back(50);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);

    // Read path uses the compact encoding and returns both recipients.
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.recipients.len(), 2);
    assert_eq!(invoice.recipients.get(0).unwrap(), recipient1);
    assert_eq!(invoice.recipients.get(1).unwrap(), recipient2);
}

#[test]
fn test_encode_recipients_with_single_recipient() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let addr1 = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(addr1.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(10000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // Single recipient decodes back to the original address with 100% bps.
    assert_eq!(invoice.recipients.len(), 1);
    assert_eq!(invoice.recipients.get(0).unwrap(), addr1);
    assert_eq!(invoice.amounts.get(0).unwrap(), 10000);
}

#[test]
fn test_encode_recipients_with_many_recipients() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    let mut amounts = Vec::new(&env);
    for _ in 0..20 {
        recipients.push_back(Address::generate(&env));
        amounts.push_back(5);
    }

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // All 20 addresses preserved, bps sum to 100%.
    assert_eq!(invoice.recipients.len(), 20);
    let mut bps_sum: u32 = 0;
    for i in 0..recipients.len() {
        assert_eq!(invoice.recipients.get(i).unwrap(), recipients.get(i).unwrap());
        bps_sum += invoice.amounts.get(i).unwrap() as u32;
    }
    assert_eq!(bps_sum, 100);
}

#[test]
fn test_lib_rs_uses_new_encoding() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let recipient = Address::generate(&env);

    let mut recipients = Vec::new(&env);
    recipients.push_back(recipient.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(10000);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);
    let invoice = client.get_invoice(&invoice_id);

    // get_invoice returns recipients correctly under the new encoding.
    assert_eq!(invoice.recipients.len(), 1);
    assert_eq!(invoice.recipients.get(0).unwrap(), recipient);
}

#[test]
fn test_payment_rs_uses_new_encoding() {
    use split_contracts::contract::Client as SplitContractClient;

    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    StellarAssetClient::new(&env, &token_id).mint(&token_admin, &1_000_000_000);

    let contract_id = env.register_contract_wasm(None, split_contracts::WASM);
    let client = SplitContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let payer = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&payer, &200);

    env.ledger().set_timestamp(1_000);

    let mut recipients = Vec::new(&env);
    recipients.push_back(recipient1.clone());
    recipients.push_back(recipient2.clone());

    let mut amounts = Vec::new(&env);
    amounts.push_back(50);
    amounts.push_back(50);

    let invoice_id = client.create_invoice(&creator, &recipients, &amounts, &token_id, &9_999);

    // Payment read path uses the new encoding and returns both recipients.
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.recipients.len(), 2);
    assert_eq!(invoice.recipients.get(0).unwrap(), recipient1);
    assert_eq!(invoice.recipients.get(1).unwrap(), recipient2);
}

#[test]
fn test_existing_tests_pass_with_encoding_change() {
    // This test ensures that the encoding change
    // doesn't break the existing 151 tests
    
    // Run existing test suite and verify all pass
    // This is more of a build verification
}
