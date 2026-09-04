//! The funding ledger's custody conjunct is a function of a CHAIN PARAMETER.
//!
//! `AuthenticatedFundingLedgerV2::validate_native_custody` asks whether the
//! account holds exactly `rent.minimum_balance(len)` plus the principal its own
//! rows still owe. The first term is read from the Rent sysvar at the moment of
//! the check; the account was funded at whatever that term was when it was
//! created, and nothing in the ledger records which. So a cluster that changes
//! its rent-exempt rate under a live Market moves the answer for an account
//! nothing has touched — and every admission path asks it with
//! `allow_lamport_surplus = false`, which is exact equality:
//! `authenticate_funding` for Core's terminal admission, verify-fund-ready, and
//! the activation-receipt arm, plus `dclutch-core-sbf`'s own
//! `resolution.rs` check, which is the deployed program's copy of the same
//! conjunct.
//!
//! These two ledgers are cohort-15's, read off devnet at finalized commitment.
//! Market 1 `9xQHh4n6cMsLTuEyvS7bQ7ho9Qoiyb5fuJLQE9bCqium` was admitted to
//! Terminal at slot 492,829,917 (2026-09-04 03:44:45 UTC). Market 3
//! `5Sa5WXPpAJ1FhebYmxRNtTBEh9XEtZwFZUcffn6Tkb2M` refused four hours later with
//! everything this conjunct reads equal to market 1's, because devnet's
//! rent-exempt rate fell from 6,333 to 5,080 lamports per byte at the
//! epoch-1141 boundary (slot 492,912,000, 07:31:40 UTC) in between. This test
//! holds both readings side by side so the parameter, and not the ledgers, is
//! what a reader is pointed at.

use dclutch_capability_contract::{
    CapabilityManifestV1, Error, FundingLedgerV2, manifest_entry_for_ledger_row_v2,
};
use solana_program::rent::Rent;

const MARKET_1_LEDGER: &[u8] = include_bytes!("fixtures/m1_ledger.bin");
const MARKET_1_MANIFEST: &[u8] = include_bytes!("fixtures/m1_manifest.bin");
const MARKET_3_LEDGER: &[u8] = include_bytes!("fixtures/m3_ledger.bin");
const MARKET_3_MANIFEST: &[u8] = include_bytes!("fixtures/m3_manifest.bin");

/// Both ledgers held this, unchanged, from their activation onward.
const OBSERVED_LEDGER_LAMPORTS: u64 = 2_482_539;

/// Devnet's rent-exempt rate while cohort-15 founded, activated and admitted
/// market 1. Every account this cohort created reads back at exactly this rate.
const CREATION_RATE_LAMPORTS_PER_BYTE: u64 = 6_333;

/// The rate the Rent sysvar and `getMinimumBalanceForRentExemption` report from
/// the epoch-1141 boundary onward.
const EPOCH_1141_RATE_LAMPORTS_PER_BYTE: u64 = 5_080;

#[allow(deprecated)]
fn rent_at(rate: u64) -> Rent {
    Rent {
        lamports_per_byte_year: rate,
        exemption_threshold: 1.0,
        burn_percent: 50,
    }
}

/// Everything the conjunct reads out of one ledger, in one value.
#[derive(Debug, Eq, PartialEq)]
struct LedgerReading {
    selected_mask: u16,
    rows: Vec<(u16, String, u64, u64, u64, u64)>,
    remaining_native_total: u64,
}

fn read(ledger_bytes: &[u8], manifest_bytes: &[u8]) -> LedgerReading {
    let ledger = FundingLedgerV2::decode(ledger_bytes).expect("ledger decodes");
    let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest decodes");
    // The identity the ledger itself binds; `authenticate` refuses any other.
    let manifest_id = ledger.manifest_content_id();
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .expect("the ledger binds its own manifest");
    let mut rows = Vec::new();
    let mut row_index = 0_u16;
    while let Ok(entry_index) = manifest_entry_for_ledger_row_v2(ledger.selected_mask(), row_index)
    {
        let slot = authenticated.slot(entry_index).expect("slot derives");
        rows.push((
            entry_index,
            format!("{:?}", slot.status()),
            slot.remaining().native_lamports_total(),
            slot.remaining().realm_collateral_total(),
            slot.released().native_lamports_total(),
            slot.released().realm_collateral_total(),
        ));
        row_index += 1;
    }
    LedgerReading {
        selected_mask: ledger.selected_mask(),
        rows,
        remaining_native_total: authenticated
            .remaining_native_lamports_total()
            .expect("remaining sums"),
    }
}

fn custody_at(ledger_bytes: &[u8], manifest_bytes: &[u8], rate: u64) -> Result<(), Error> {
    let ledger = FundingLedgerV2::decode(ledger_bytes).expect("ledger decodes");
    let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest decodes");
    ledger
        .authenticate(ledger.manifest_content_id(), manifest)
        .expect("the ledger binds its own manifest")
        .validate_native_custody(
            OBSERVED_LEDGER_LAMPORTS,
            rent_at(rate).minimum_balance(ledger_bytes.len()),
            false,
        )
}

/// The lane's first question, answered on the accounts rather than by
/// inspection: every quantity this conjunct reads is equal across the two
/// markets. Whatever separates their verdicts is not in either ledger.
#[test]
fn everything_the_custody_conjunct_reads_is_equal_across_the_two_ledgers() {
    let market_1 = read(MARKET_1_LEDGER, MARKET_1_MANIFEST);
    let market_3 = read(MARKET_3_LEDGER, MARKET_3_MANIFEST);
    assert_eq!(MARKET_1_LEDGER.len(), MARKET_3_LEDGER.len());
    assert_eq!(market_1.rows.len(), 3, "three selected rows");
    assert_eq!(market_1, market_3);
    assert_eq!(market_1.remaining_native_total, 3);
}

/// The verdict flips on the rent rate alone, identically for both markets.
#[test]
fn a_rent_rate_change_moves_the_custody_verdict_for_both_ledgers() {
    for (market, ledger, manifest) in [
        ("market 1", MARKET_1_LEDGER, MARKET_1_MANIFEST),
        ("market 3", MARKET_3_LEDGER, MARKET_3_MANIFEST),
    ] {
        assert_eq!(
            custody_at(ledger, manifest, CREATION_RATE_LAMPORTS_PER_BYTE),
            Ok(()),
            "{market} was exact at the rate it was funded at"
        );
        assert_eq!(
            custody_at(ledger, manifest, EPOCH_1141_RATE_LAMPORTS_PER_BYTE),
            Err(Error::PresentNativeLamportsMismatch),
            "{market} is stranded by the rate it was not funded at"
        );
    }
}

/// The stranded amount, stated: what the rate change turned from rent into an
/// unaccounted surplus. It is the whole of the difference between the two
/// minimum balances, and no lamport of it left the account.
#[test]
fn the_stranded_lamports_are_exactly_the_rent_difference() {
    let creation_minimum =
        rent_at(CREATION_RATE_LAMPORTS_PER_BYTE).minimum_balance(MARKET_3_LEDGER.len());
    let epoch_1141_minimum =
        rent_at(EPOCH_1141_RATE_LAMPORTS_PER_BYTE).minimum_balance(MARKET_3_LEDGER.len());
    assert_eq!(creation_minimum, 2_482_536);
    assert_eq!(epoch_1141_minimum, 1_991_360);
    assert_eq!(
        OBSERVED_LEDGER_LAMPORTS - epoch_1141_minimum - 3,
        creation_minimum - epoch_1141_minimum,
        "the surplus the conjunct refuses is the rent difference and nothing else"
    );
}
