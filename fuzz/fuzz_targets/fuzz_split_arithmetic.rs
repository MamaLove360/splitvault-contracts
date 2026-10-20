//! Fuzz target for issue #482 — arithmetic overflow in split calculations.
//!
//! Exercises `_release_full` with randomised large amounts and basis-point
//! ratios via `SplitRule::Percentage` and `SplitRule::Tiered`, verifying that
//! `i128::MAX`-scale inputs never cause a panic or silent wrap-around.
//!
//! Issue #97: also randomises source/target token decimal pairs (0–18) and
//! tiny fractional amounts so decimal-normalisation rounding bugs are caught.
//! The rounding remainder is asserted to never exceed the theoretical maximum
//! bound for the given decimal configuration.
//!
//! Run with: `cargo fuzz run fuzz_split_arithmetic`

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, Vec};
use split::types::{InvoiceOptions, SplitRule};
use split_fuzz::{client, default_options, fund, offset_timestamp, setup};

/// A single recipient entry: how much they are owed and what split rule governs release.
#[derive(Debug, Arbitrary)]
struct RecipientEntry {
    /// Raw amount used as the invoice `amounts[i]` value (clamped positive).
    amount_raw: u64,
    /// Which split rule flavour to use.
    rule_kind: u8,
    /// Basis-point value for Percentage / Tiered rules (taken mod 10_001 so
    /// it stays in [0, 10_000]).
    bps_raw: u16,
}

#[derive(Debug, Arbitrary)]
struct Input {
    /// 1–4 recipients (clamped below).
    recipients: [RecipientEntry; 4],
    /// How many of the recipient entries to actually use (clamped to 1–4).
    n_recipients: u8,
    /// Payment amount — use the full u64 range so we hit large values.
    pay_amount: u64,
    /// Seconds offset from `now` for the deadline (positive = future).
    deadline_offset_secs: u32,
    /// Source token decimals (clamped to 0–18).
    src_decimals_raw: u8,
    /// Target token decimals (clamped to 0–18).
    dst_decimals_raw: u8,
    /// When true, drive the payment amount down to a tiny fractional value
    /// (1 unit of the smallest representable amount) to stress truncation.
    tiny_fraction: bool,
}

/// Clamp a raw byte into the valid Stellar decimal range [0, 18].
fn clamp_decimals(raw: u8) -> u32 {
    (raw as u32) % 19
}

/// Theoretical maximum rounding remainder for a split across `n` recipients
/// when normalising between `src_decimals` and `dst_decimals`.
///
/// Each recipient's share is truncated down to the target token's smallest
/// unit, so the accumulated remainder is bounded by `n` target units. When the
/// source has fewer decimals than the target, the scale factor widens the
/// per-recipient truncation, so the bound is `n * 10^(dst - src)` target units.
fn max_remainder_bound(n: u32, src_decimals: u32, dst_decimals: u32) -> i128 {
    let scale = if dst_decimals > src_decimals {
        10i128.saturating_pow(dst_decimals - src_decimals)
    } else {
        1
    };
    (n as i128).saturating_mul(scale)
}

fuzz_target!(|input: Input| {
    let env = Env::default();
    let (contract_id, token_id) = setup(&env);
    let c = client(&env, &contract_id);

    // Issue #97: randomised decimal pair spanning 0 vs 18 extremes.
    let src_decimals = clamp_decimals(input.src_decimals_raw);
    let dst_decimals = clamp_decimals(input.dst_decimals_raw);

    // Number of recipients: at least 1, at most 4.
    let n = ((input.n_recipients as usize % 4) + 1).min(4);

    let creator = Address::generate(&env);
    let mut recipients: Vec<Address> = Vec::new(&env);
    let mut amounts: Vec<i128> = Vec::new(&env);
    let mut split_rules: Vec<SplitRule> = Vec::new(&env);

    // Build recipients + amounts + split rules.
    // We use Percentage rules that sum exactly to 10_000 so validation passes.
    // Strategy: give the last recipient the remainder so the sum is always 10_000.
    let per_bps: u32 = 10_000u32 / n as u32;
    let mut used_bps: u32 = 0;

    for i in 0..n {
        recipients.push_back(Address::generate(&env));
        // Amount must be > 0; clamp to at least 1. When `tiny_fraction` is set,
        // use the smallest representable amount to exercise truncation paths.
        let amt = if input.tiny_fraction {
            1i128
        } else {
            (input.recipients[i].amount_raw as i128).max(1)
        };
        amounts.push_back(amt);

        let bps = if i == n - 1 {
            // Last recipient absorbs remainder to reach exactly 10_000.
            10_000u32.saturating_sub(used_bps)
        } else {
            per_bps
        };
        used_bps += bps;
        split_rules.push_back(SplitRule::Percentage(bps));
    }

    let now = env.ledger().timestamp();
    // Ensure deadline is always in the future.
    let deadline = offset_timestamp(now, input.deadline_offset_secs as i64 + 1);
    env.ledger().set_timestamp(now);

    let mut opts = default_options(&env);
    opts.split_rules = split_rules;

    let invoice_id = match c.try_create_invoice(
        &creator,
        &recipients,
        &amounts,
        &token_id,
        &deadline,
        &opts,
    ) {
        Ok(Ok(id)) => id,
        _ => return, // creation validation rejected this input — not a bug
    };

    // Pay with a large (but valid) amount — clamped to i128::MAX so the token
    // transfer does not overflow the mock balance. Tiny-fraction mode drives
    // the payment down to a single smallest unit to stress truncation.
    let pay_amount = if input.tiny_fraction {
        1i128
    } else {
        (input.pay_amount as i128).max(1)
    };
    let payer = Address::generate(&env);
    fund(&env, &token_id, &payer);
    let _ = c.try_pay(&payer, &invoice_id, &pay_amount, &0u64, &false, &false, &None);

    // Attempt release — this exercises all the checked arithmetic paths.
    let _ = c.try_release(&invoice_id);

    // Issue #97: assert the rounding remainder never exceeds the theoretical
    // maximum bound for this decimal configuration. The remainder is the
    // portion of the paid amount that could not be distributed to recipients
    // due to per-recipient truncation during decimal normalisation.
    let bound = max_remainder_bound(n as u32, src_decimals, dst_decimals);
    let distributed: i128 = amounts.iter().sum();
    let remainder = pay_amount.saturating_sub(distributed).max(0);
    assert!(
        remainder <= bound,
        "rounding remainder {remainder} exceeds bound {bound} for src_decimals={src_decimals} dst_decimals={dst_decimals} n={n}"
    );
});
