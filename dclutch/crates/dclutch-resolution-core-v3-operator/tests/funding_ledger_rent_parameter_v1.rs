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

/// THE FIXTURES ARE CHAIN EVIDENCE AND ARE NOT EDITED.
///
/// These four `.bin` files are cohort-15's own account bytes, read off devnet at
/// finalized commitment, and they were written by programs that had no field for
/// the rate they were funded at. So every read below splices `6,333` into a COPY
/// of the header, at the four bytes that used to be reserved -- which is exactly
/// and only what a cohort-16 founding writes there itself. The splice is the
/// whole content of the repair: nothing else about these accounts changes, and
/// the tests below are the before and after of that one field existing.
fn recorded(ledger_bytes: &[u8]) -> Vec<u8> {
    let rate = u32::try_from(CREATION_RATE_LAMPORTS_PER_BYTE).expect("rate fits");
    let mut bytes = ledger_bytes.to_vec();
    // Located by decode rather than by a hand-written offset: the span is the one
    // whose four zero bytes, filled with the rate, make a header decode at all.
    let offset = (0..44)
        .find(|start| {
            let mut probe = ledger_bytes.to_vec();
            let Some(span) = probe.get_mut(*start..start + 4) else {
                return false;
            };
            span.copy_from_slice(&rate.to_le_bytes());
            FundingLedgerV2::decode(&probe).is_ok()
        })
        .expect("exactly one header span carries the funded rate");
    bytes
        .get_mut(offset..offset + 4)
        .expect("the span the search found")
        .copy_from_slice(&rate.to_le_bytes());
    bytes
}

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

fn read(raw_ledger_bytes: &[u8], manifest_bytes: &[u8]) -> LedgerReading {
    let ledger_bytes = recorded(raw_ledger_bytes);
    let ledger = FundingLedgerV2::decode(&ledger_bytes).expect("ledger decodes");
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

fn custody_at(raw_ledger_bytes: &[u8], manifest_bytes: &[u8], rate: u64) -> Result<(), Error> {
    let ledger_bytes = recorded(raw_ledger_bytes);
    let ledger = FundingLedgerV2::decode(&ledger_bytes).expect("ledger decodes");
    let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest decodes");
    ledger
        .authenticate(ledger.manifest_content_id(), manifest)
        .expect("the ledger binds its own manifest")
        .validate_native_custody(
            OBSERVED_LEDGER_LAMPORTS,
            rent_at(rate).minimum_balance(raw_ledger_bytes.len()),
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

/// THE PAYOFF, ON THE SAME BYTES: THE VERDICT STOPS MOVING.
///
/// Everything above is the defect. This is the repair, measured on cohort-15's
/// own ledgers rather than on a fixture built to agree with it: once the header
/// records the rate the account was funded at, the custody conjunct gives the
/// same answer no matter what the cluster charges today, and it is still EXACT.
/// The three assertions are the ruling's three hostiles in order.
#[test]
fn a_recorded_rate_makes_the_verdict_stop_moving_and_stay_exact() {
    for (market, raw, manifest_bytes) in [
        ("market 1", MARKET_1_LEDGER, MARKET_1_MANIFEST),
        ("market 3", MARKET_3_LEDGER, MARKET_3_MANIFEST),
    ] {
        let bytes = recorded(raw);
        let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest decodes");
        let ledger = FundingLedgerV2::decode(&bytes).expect("ledger decodes");
        let authenticated = ledger
            .authenticate(ledger.manifest_content_id(), manifest)
            .expect("the ledger binds its own manifest");

        // The record reproduces the cluster reading that funded it, to the lamport.
        assert_eq!(
            authenticated.funded_rent_minimum(raw.len()),
            Ok(rent_at(CREATION_RATE_LAMPORTS_PER_BYTE).minimum_balance(raw.len())),
            "{market}'s recorded rate rederives the minimum it was funded at"
        );

        // (a) exact after the cluster moved.
        assert_eq!(
            authenticated.validate_recorded_native_custody(
                OBSERVED_LEDGER_LAMPORTS,
                raw.len(),
                false
            ),
            Ok(()),
            "{market} is exact against the rate it was funded at, whatever devnet charges now"
        );

        // (b) a donation of one lamport still refuses.
        assert_eq!(
            authenticated.validate_recorded_native_custody(
                OBSERVED_LEDGER_LAMPORTS + 1,
                raw.len(),
                false
            ),
            Err(Error::FundedRentNotEvidenced),
            "{market} still refuses one lamport nobody can account for"
        );

        // (c) and the old rule, run beside it on the same bytes, still refuses --
        //     which is the positive control that this fixture straddles a real
        //     rate change rather than proving nothing.
        assert_eq!(
            custody_at(raw, manifest_bytes, EPOCH_1141_RATE_LAMPORTS_PER_BYTE),
            Err(Error::PresentNativeLamportsMismatch),
            "{market} is still stranded by the rule this repair replaces"
        );
    }
}
