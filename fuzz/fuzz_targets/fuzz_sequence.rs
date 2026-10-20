#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env, Vec};
use split_fuzz::{client, default_options, fund, offset_timestamp, setup};

const MAX_OPS: usize = 24;

#[derive(Debug, Arbitrary)]
enum Op {
    Create {
        amount: i64,
        deadline_offset_secs: i32,
    },
    Pay {
        invoice_slot: u8,
        amount: i64,
        use_fresh_payer: bool,
    },
    Release {
        invoice_slot: u8,
    },
    Refund {
        invoice_slot: u8,
        advance_secs: u16,
    },
}

#[derive(Debug, Arbitrary)]
struct Input {
    ops: std::vec::Vec<Op>,
}

fn resolve_id(created: &[u64], slot: u8) -> u64 {
    if !created.is_empty() && slot % 2 == 0 {
        created[(slot as usize / 2) % created.len()]
    } else {
        slot as u64
    }
}

/// Assert core protocol invariants after each transaction step.
///
/// 1. The contract's token balance must cover all tracked active liabilities
///    (sum over invoices of `funded - released`).
/// 2. The sum of recipient shares must equal the total amount released.
fn assert_invariants(
    env: &Env,
    token_id: &Address,
    contract_id: &Address,
    funded: &std::vec::Vec<i128>,
    released: &std::vec::Vec<i128>,
) {
    let token_client = token::Client::new(env, token_id);
    let contract_balance = token_client.balance(contract_id);

    let mut active_liabilities: i128 = 0;
    let mut total_released: i128 = 0;
    for (f, r) in funded.iter().zip(released.iter()) {
        assert!(*r <= *f, "released exceeds funded for an invoice");
        active_liabilities = active_liabilities
            .checked_add(f - r)
            .expect("active liabilities overflow");
        total_released = total_released
            .checked_add(*r)
            .expect("total released overflow");
    }

    // Invariant 1: contract balance covers tracked active liabilities.
    assert!(
        contract_balance >= active_liabilities,
        "contract balance {} does not cover active liabilities {}",
        contract_balance,
        active_liabilities
    );

    // Invariant 2: sum of recipient shares equals total amount released.
    // Each invoice tracks a single recipient share equal to its released amount.
    let mut recipient_shares: i128 = 0;
    for r in released.iter() {
        recipient_shares = recipient_shares
            .checked_add(*r)
            .expect("recipient shares overflow");
    }
    assert!(
        recipient_shares == total_released,
        "recipient shares {} do not equal total released {}",
        recipient_shares,
        total_released
    );
}

fuzz_target!(|input: Input| {
    let env = Env::default();
    let (contract_id, token_id) = setup(&env);
    let c = client(&env, &contract_id);
    let options = default_options(&env);

    let mut created: std::vec::Vec<u64> = std::vec::Vec::new();
    let mut payers: std::vec::Vec<Address> = std::vec::Vec::new();
    // Per-invoice tracked funded and released amounts, indexed parallel to `created`.
    let mut funded: std::vec::Vec<i128> = std::vec::Vec::new();
    let mut released: std::vec::Vec<i128> = std::vec::Vec::new();

    for op in input.ops.into_iter().take(MAX_OPS) {
        match op {
            Op::Create { amount, deadline_offset_secs } => {
                let creator = Address::generate(&env);
                let recipient = Address::generate(&env);
                let mut recipients = Vec::new(&env);
                recipients.push_back(recipient);
                let mut amounts = Vec::new(&env);
                amounts.push_back(amount as i128);
                let now = env.ledger().timestamp();
                let deadline = offset_timestamp(now, deadline_offset_secs as i64);

                if let Ok(Ok(id)) = c.try_create_invoice(
                    &creator, &recipients, &amounts, &token_id, &deadline, &options,
                ) {
                    created.push(id);
                    funded.push(0);
                    released.push(0);
                }
            }
            Op::Pay { invoice_slot, amount, use_fresh_payer } => {
                let invoice_id = resolve_id(&created, invoice_slot);
                let payer = if !use_fresh_payer && !payers.is_empty() {
                    payers[invoice_slot as usize % payers.len()].clone()
                } else {
                    let fresh = Address::generate(&env);
                    fund(&env, &token_id, &fresh);
                    payers.push(fresh.clone());
                    fresh
                };
                let pay_amount = amount as i128;
                if let Ok(Ok(_)) = c.try_pay(&payer, &invoice_id, &pay_amount, &0u64, &false, &false, &None) {
                    if let Some(idx) = created.iter().position(|id| *id == invoice_id) {
                        funded[idx] = funded[idx].saturating_add(pay_amount);
                    }
                }
            }
            Op::Release { invoice_slot } => {
                let invoice_id = resolve_id(&created, invoice_slot);
                if let Ok(Ok(_)) = c.try_release(&invoice_id) {
                    if let Some(idx) = created.iter().position(|id| *id == invoice_id) {
                        // On release the invoice's funded amount is paid out to recipients.
                        released[idx] = funded[idx];
                    }
                }
            }
            Op::Refund { invoice_slot, advance_secs } => {
                let invoice_id = resolve_id(&created, invoice_slot);
                if advance_secs > 0 {
                    let now = env.ledger().timestamp();
                    env.ledger().set_timestamp(now.saturating_add(advance_secs as u64));
                }
                if let Ok(Ok(_)) = c.try_refund(&invoice_id) {
                    if let Some(idx) = created.iter().position(|id| *id == invoice_id) {
                        // On refund the funded amount is returned to payers, clearing liability.
                        funded[idx] = 0;
                        released[idx] = 0;
                    }
                }
            }
        }

        assert_invariants(&env, &token_id, &contract_id, &funded, &released);
    }
});
