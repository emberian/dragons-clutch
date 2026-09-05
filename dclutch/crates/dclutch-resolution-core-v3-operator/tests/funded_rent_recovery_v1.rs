//! THE PLANNER RECOVERS THE RATE A COHORT ALREADY ON CHAIN WAS FUNDED AT.
//!
//! `FundingLedgerV2` now records the exemption-scaled rent rate its founding
//! paid, and a header carrying no rate fails closed at decode rather than
//! pricing every account at nothing. Every ledger cohort-15 founded was written
//! before that field existed, so every one of them carries zero there -- which
//! is correct, and which stops the host planner reading them at all.
//!
//! These fixtures are cohort-15's own account bytes, read off devnet at
//! finalized commitment and NOT edited. The wall and the repair are both
//! measured on them.
#![allow(clippy::panic)]

use dclutch_market::capability_manifest::{
    CapabilityManifestV1, Error as CapabilityError, FundingLedgerV2,
};
use dclutch_resolution_core_v3_operator::ResolutionCoreOperatorErrorV3;
use dclutch_resolution_core_v3_operator::funded_rent_recovery_v1::{
    FundedRentReadingV2, ledger_with_funded_rent_rate_v2, recover_funded_rent_rate_v2,
};

const MARKET_1_LEDGER: &[u8] = include_bytes!("fixtures/m1_ledger.bin");
const MARKET_1_MANIFEST: &[u8] = include_bytes!("fixtures/m1_manifest.bin");
const MARKET_3_LEDGER: &[u8] = include_bytes!("fixtures/m3_ledger.bin");
const MARKET_3_MANIFEST: &[u8] = include_bytes!("fixtures/m3_manifest.bin");

/// What both ledger accounts hold, read off devnet and unchanged since
/// activation. 2,482,536 of it is rent and 3 of it is Bounty principal.
const OBSERVED_LEDGER_LAMPORTS: u64 = 2_482_539;

/// The rate devnet charged while cohort-15 founded. Recovered below rather
/// than supplied: no test here tells the recovery what answer to reach.
const FOUNDING_RATE: u32 = 6_333;

/// The rate devnet charged from the epoch-1141 boundary onward.
const EPOCH_1141_RATE: u32 = 5_080;

fn manifest(bytes: &[u8]) -> CapabilityManifestV1<'_> {
    CapabilityManifestV1::decode(bytes).expect("manifest decodes")
}

/// The manifest's own content identity, computed from the manifest bytes rather
/// than read out of the ledger header -- a legacy header does not decode, and a
/// test that reached into it at a hand-written offset would be a second author
/// of a layout the ABI already owns. `authenticate` refuses any other identity,
/// so this is checked by every call below rather than asserted here.
fn manifest_id(manifest_bytes: &[u8]) -> dclutch_market::capability_manifest::ContentId {
    dclutch_market::capability_manifest::ContentId::new(
        solana_program::hash::hash(manifest_bytes).to_bytes(),
    )
    .expect("nonzero manifest identity")
}

/// (RED) THE WALL, ON THE REAL BYTES: a cohort-15 ledger does not decode.
///
/// This is the state of the tree before this module existed, and it is the
/// positive control for everything below -- without it, a recovery that did
/// nothing would look identical to a recovery that worked.
#[test]
fn a_cohort_fifteen_ledger_records_no_rate_and_does_not_decode() {
    for (market, raw) in [("market 1", MARKET_1_LEDGER), ("market 3", MARKET_3_LEDGER)] {
        assert_eq!(
            FundingLedgerV2::decode(raw).err(),
            Some(CapabilityError::FundedRentRateMissing),
            "{market}'s ledger was written before the header had a field for the rate"
        );
        assert!(
            raw.get(12..16)
                .expect("the reserved span")
                .iter()
                .all(|b| *b == 0),
            "{market}'s ledger carries zero where a cohort-16 founding writes its rate"
        );
    }
}

/// (GREEN) THE RATE IS RECOVERED FROM THE LEDGER'S OWN BYTES.
///
/// `rate = (lamports - remaining native principal) / (128 + len)`, and the
/// answer is not supplied by this test: it is compared against devnet's
/// measured founding rate only after the recovery has produced it.
#[test]
fn the_planner_recovers_the_founding_rate_from_the_ledgers_own_bytes() {
    for (market, raw, manifest_bytes) in [
        ("market 1", MARKET_1_LEDGER, MARKET_1_MANIFEST),
        ("market 3", MARKET_3_LEDGER, MARKET_3_MANIFEST),
    ] {
        let priced = ledger_with_funded_rent_rate_v2(
            raw,
            OBSERVED_LEDGER_LAMPORTS,
            manifest_id(manifest_bytes),
            manifest(manifest_bytes),
            &[],
        )
        .expect("the rate is recoverable from a ledger nothing has touched");
        assert!(priced.recovered, "{market}'s header recorded no rate");
        assert_eq!(
            priced.funded_rent_rate, FOUNDING_RATE,
            "{market} recovers the rate devnet charged when cohort-15 founded"
        );

        // The recovered ledger reads, and the conjunct that refused it passes
        // EXACTLY -- which is the whole claim, since a relaxation would also
        // have made it pass.
        let ledger = FundingLedgerV2::decode(&priced.bytes).expect("recovered ledger decodes");
        let authenticated = ledger
            .authenticate(manifest_id(manifest_bytes), manifest(manifest_bytes))
            .expect("the ledger binds its own manifest");
        authenticated
            .validate_recorded_native_custody(OBSERVED_LEDGER_LAMPORTS, raw.len(), false)
            .expect("exact against the rate it was funded at");
        assert_eq!(
            authenticated.remaining_native_lamports_total(),
            Ok(3),
            "{market}'s rows still owe three lamports of Bounty principal"
        );

        // And only the four reserved bytes moved. The account on chain is not
        // written by any of this.
        assert_eq!(priced.bytes.len(), raw.len());
        for (offset, (recovered, original)) in priced.bytes.iter().zip(raw).enumerate() {
            if !(12..16).contains(&offset) {
                assert_eq!(recovered, original, "{market} byte {offset} moved");
            }
        }
    }
}

/// A DONATED LAMPORT IS REFUSED BY NAME, NOT ROUNDED AWAY.
///
/// The hostile PROGRAMS-16 wrote for the recorded-rate path, asked one layer
/// earlier: with no record to compare against, a donation is a balance that no
/// rate reproduces, and the recovery refuses rather than picking the nearest.
#[test]
fn one_donated_lamport_refuses_the_recovery_in_either_direction() {
    for (delta, lamports) in [
        (1_i64, OBSERVED_LEDGER_LAMPORTS.wrapping_add(1)),
        (-1, OBSERVED_LEDGER_LAMPORTS.wrapping_sub(1)),
    ] {
        assert_eq!(
            ledger_with_funded_rent_rate_v2(
                MARKET_3_LEDGER,
                lamports,
                manifest_id(MARKET_3_MANIFEST),
                manifest(MARKET_3_MANIFEST),
                &[],
            )
            .err(),
            Some(ResolutionCoreOperatorErrorV3::FundedRentUnrecoverable),
            "a balance {delta} off the affine line names no rate, and says so"
        );
    }
    // The control: the untouched balance recovers.
    assert!(
        ledger_with_funded_rent_rate_v2(
            MARKET_3_LEDGER,
            OBSERVED_LEDGER_LAMPORTS,
            manifest_id(MARKET_3_MANIFEST),
            manifest(MARKET_3_MANIFEST),
            &[],
        )
        .is_ok()
    );
}

/// ONE FOUNDING IS ONE RATE, AT EVERY WIDTH -- `the_whole_cohort_is_one_rate`.
///
/// The sibling is not invented: market 3's founding created a second funding
/// ledger, Trading-owned, at `Gm5WFhDCa7CryyLkfPWBvcDQhAEcwszjDVAhJvdmq1tx` --
/// 120 bytes holding 1,570,584 lamports with no principal outstanding, read off
/// devnet at finalized commitment. Two widths pin the affine function, which is
/// the same shape `derive_funded_rent_rate_v2` requires of a founding that
/// records its rate rather than recovering it.
#[test]
fn a_sibling_ledger_of_the_same_founding_corroborates_the_rate() {
    let trading_sibling = FundedRentReadingV2 {
        account_bytes: 120,
        account_lamports: 1_570_584,
        remaining_native_principal: 0,
    };
    let priced = ledger_with_funded_rent_rate_v2(
        MARKET_3_LEDGER,
        OBSERVED_LEDGER_LAMPORTS,
        manifest_id(MARKET_3_MANIFEST),
        manifest(MARKET_3_MANIFEST),
        &[trading_sibling],
    )
    .expect("264 bytes and 120 bytes of one founding agree on one rate");
    assert_eq!(priced.funded_rent_rate, FOUNDING_RATE);

    // A sibling funded at the OTHER rate is a different founding, and saying so
    // is the point: the cohort boundary is what makes one rate meaningful.
    let epoch_1141_sibling = FundedRentReadingV2 {
        account_bytes: 264,
        account_lamports: 1_991_360,
        remaining_native_principal: 0,
    };
    assert_eq!(
        ledger_with_funded_rent_rate_v2(
            MARKET_3_LEDGER,
            OBSERVED_LEDGER_LAMPORTS,
            manifest_id(MARKET_3_MANIFEST),
            manifest(MARKET_3_MANIFEST),
            &[epoch_1141_sibling],
        )
        .err(),
        Some(ResolutionCoreOperatorErrorV3::FundedRentUnrecoverable),
        "two rates in one founding is a disagreement, never an average"
    );

    // And the two markets' own ledgers, folded together: cohort-15 is one rate.
    assert_eq!(
        recover_funded_rent_rate_v2(&[
            FundedRentReadingV2 {
                account_bytes: MARKET_1_LEDGER.len(),
                account_lamports: OBSERVED_LEDGER_LAMPORTS,
                remaining_native_principal: 3,
            },
            FundedRentReadingV2 {
                account_bytes: MARKET_3_LEDGER.len(),
                account_lamports: OBSERVED_LEDGER_LAMPORTS,
                remaining_native_principal: 3,
            },
            trading_sibling,
            // The Market account, 368 bytes at 3,141,168, and the certificate
            // seat, 312 bytes at 2,786,520. Both read off devnet.
            FundedRentReadingV2 {
                account_bytes: 368,
                account_lamports: 3_141_168,
                remaining_native_principal: 0,
            },
            FundedRentReadingV2 {
                account_bytes: 312,
                account_lamports: 2_786_520,
                remaining_native_principal: 0,
            },
        ]),
        Ok(FOUNDING_RATE),
        "five accounts of one cohort at five widths derive one rate"
    );
}

/// A RECORDED RATE IS NEVER SECOND-GUESSED.
///
/// Recovery is for a header that has nothing to say. A cohort-16 ledger records
/// its own rate and the recovery must return THAT, even when the balance would
/// have derived another -- otherwise the record stops being the authority and a
/// lying record becomes unfalsifiable.
#[test]
fn a_ledger_that_records_its_own_rate_is_returned_unchanged() {
    let mut recorded = MARKET_3_LEDGER.to_vec();
    recorded
        .get_mut(12..16)
        .expect("the span the founding writes")
        .copy_from_slice(&EPOCH_1141_RATE.to_le_bytes());
    let priced = ledger_with_funded_rent_rate_v2(
        &recorded,
        OBSERVED_LEDGER_LAMPORTS,
        manifest_id(MARKET_3_MANIFEST),
        manifest(MARKET_3_MANIFEST),
        &[],
    )
    .expect("a recorded rate needs no recovery");
    assert!(!priced.recovered, "the header spoke for itself");
    assert_eq!(
        priced.funded_rent_rate, EPOCH_1141_RATE,
        "the record is the authority, not the balance"
    );
    assert_eq!(priced.bytes, recorded, "and the bytes are untouched");

    // The balance then disagrees with the record, which is the recorded-rate
    // path's own refusal and not this module's business to paper over.
    assert_eq!(
        FundingLedgerV2::decode(&recorded)
            .expect("decodes")
            .authenticate(manifest_id(MARKET_3_MANIFEST), manifest(MARKET_3_MANIFEST))
            .expect("binds")
            .validate_recorded_native_custody(OBSERVED_LEDGER_LAMPORTS, recorded.len(), false),
        Err(CapabilityError::FundedRentNotEvidenced),
        "a record claiming a rent the balance does not evidence is refused, and named"
    );
}
