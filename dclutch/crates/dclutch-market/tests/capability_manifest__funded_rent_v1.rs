//! THE RENT AN ACCOUNT WAS FUNDED AT IS A FACT FIXED WHEN IT WAS FUNDED.
//!
//! Devnet lowered its rent-exempt rate from 6,333 to 5,080 lamports per byte at
//! the epoch-1141 boundary (slot 492,912,000, 2026-09-04 07:31:40 UTC) with
//! cohort-15 live on it. Every exactness check that re-derived a funded account's
//! rent from the Rent sysvar of the moment then refused an account nobody had
//! touched, by exactly the rate difference scaled by the account's own footprint:
//! 491,176 lamports on a 264-byte funding ledger, which is `392 * (6333 - 5080)`.
//!
//! A `FundingLedgerV2` header now records the exemption-scaled rate its founding
//! paid, in the four bytes it used to reserve. These are the three properties the
//! ruling names, and the fourth that says the recording costs nothing.

#![allow(clippy::panic)]

use dclutch_market::capability_manifest::funding::{
    ACCOUNT_STORAGE_OVERHEAD_BYTES, FundingAmountsV1, FundingLedgerV2, FundingQuoteV1,
    derive_funded_rent_rate_v2, funded_rent_minimum_v2, funded_rent_rate_from_minimum_v1,
    funding_ledger_bytes_v2,
};
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1, ContentId,
    Error, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};

/// The rate devnet charged when cohort-15 was founded.
const FUNDED_RATE: u32 = 6333;
/// The rate devnet charged after the epoch-1141 boundary.
const LATER_RATE: u32 = 5080;
/// One selected row: 48 header bytes plus one 72-byte slot.
const ONE_ROW_BYTES: usize = 120;
/// The Bounty principal the fixture quote parks in the ledger account.
const PRINCIPAL: u64 = 3;

fn id(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero fixture identity")
}

fn quote() -> FundingQuoteV1 {
    let native = |value: u64| {
        dclutch_market::capability_manifest::funding::CompartmentFundingV1::native_lamports(value)
            .expect("positive lamports")
    };
    let absent =
        dclutch_market::capability_manifest::funding::CompartmentFundingV1::not_applicable();
    FundingQuoteV1::new(
        FundingAmountsV1::new(
            native(1),
            native(1),
            absent,
            absent,
            native(PRINCIPAL),
            absent,
            absent,
        )
        .expect("amounts"),
        None,
    )
    .expect("quote")
}

fn manifest(
    storage: &mut [u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES],
) -> CapabilityManifestV1<'_> {
    let entry = CapabilityEntryV1::new(
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote(),
    )
    .expect("entry");
    CapabilityManifestV1::encode_into(&[entry], storage).expect("manifest")
}

/// One Pending ledger funded at `rate`, and the lamports it was funded with.
fn funded_ledger(rate: u32) -> (Vec<u8>, u64) {
    let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let view = manifest(&mut storage);
    let manifest_id =
        ContentId::new(dclutch_sha256_adapter::digest(view.as_bytes())).expect("manifest identity");
    let width = funding_ledger_bytes_v2(1).expect("width");
    assert_eq!(width, ONE_ROW_BYTES);
    let mut bytes = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut bytes, manifest_id, view, 1, rate).expect("initialize");
    let funded = funded_rent_minimum_v2(rate, width).expect("funded minimum");
    // Rent + Creation + Bounty are all native and all still Pending, so the
    // account holds its rent plus the whole quote's native principal.
    (bytes, funded + PRINCIPAL + 2)
}

fn authenticate<'a>(
    bytes: &'a [u8],
    storage: &'a mut [u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES],
) -> (dclutch_market::capability_manifest::funding::AuthenticatedFundingLedgerV2<'a, 'a>,) {
    let view = manifest(storage);
    let manifest_id =
        ContentId::new(dclutch_sha256_adapter::digest(view.as_bytes())).expect("manifest identity");
    (FundingLedgerV2::decode(bytes)
        .expect("decode")
        .authenticate(manifest_id, view)
        .expect("authenticate"),)
}

/// (a) AN ACCOUNT FUNDED AT 6,333 AND CHECKED AFTER THE RATE FELL TO 5,080
/// PASSES EXACTLY.
///
/// The rate never appears at the check; only the record does. The proof that
/// the cluster really has moved is the `assert_ne!` on what the old code would
/// have compared against.
#[test]
fn an_account_funded_at_the_old_rate_passes_exactly_after_the_rate_falls() {
    let (bytes, lamports) = funded_ledger(FUNDED_RATE);
    let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let (authenticated,) = authenticate(&bytes, &mut storage);

    let today = funded_rent_minimum_v2(LATER_RATE, ONE_ROW_BYTES).expect("today's minimum");
    let funded = authenticated
        .funded_rent_minimum(ONE_ROW_BYTES)
        .expect("funded minimum");
    assert_ne!(
        today, funded,
        "the fixture must actually straddle a rate change or it proves nothing"
    );

    authenticated
        .validate_recorded_native_custody(lamports, ONE_ROW_BYTES, false)
        .expect("an account priced by the rate it records is exact after the cluster moves");

    // And what the old code did: compare against the sysvar of the moment.
    assert_eq!(
        authenticated.validate_native_custody(lamports, today, false),
        Err(Error::PresentNativeLamportsMismatch),
        "re-deriving from a moved cluster is exactly what used to refuse a whole cohort"
    );
}

/// (b) A DONATION OF ONE LAMPORT STILL REFUSES.
///
/// The repair is not a relaxation. `>=` would admit a real donation as custody,
/// which the ledger census laws forbid; the recorded figure keeps the check
/// exact in both directions.
#[test]
fn one_donated_lamport_still_refuses_in_either_direction() {
    let (bytes, lamports) = funded_ledger(FUNDED_RATE);
    let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let (authenticated,) = authenticate(&bytes, &mut storage);

    assert_eq!(
        authenticated.validate_recorded_native_custody(lamports + 1, ONE_ROW_BYTES, false),
        Err(Error::FundedRentNotEvidenced),
        "one lamport more than the record accounts for is a donation, not custody"
    );
    assert_eq!(
        authenticated.validate_recorded_native_custody(lamports - 1, ONE_ROW_BYTES, false),
        Err(Error::FundedRentNotEvidenced),
        "one lamport less than the record accounts for is a leak"
    );
    authenticated
        .validate_recorded_native_custody(lamports + 1, ONE_ROW_BYTES, true)
        .expect("the close path classifies a surplus and still admits it");
}

/// (c) A RECORD CLAIMING A RENT THE ACCOUNT'S LAMPORTS DO NOT EVIDENCE REFUSES
/// BY NAME.
///
/// `PresentNativeLamportsMismatch` covered both this and an ordinary donation.
/// Split under decision 0007: when the term that disagrees is the PERSISTED
/// rate, the reader is told so, and a zero rate -- which is what every account
/// funded before the field existed carries -- is its own refusal rather than a
/// silent price of nothing.
#[test]
fn a_record_claiming_a_rent_the_account_does_not_evidence_refuses_by_name() {
    let (bytes, lamports) = funded_ledger(FUNDED_RATE);
    let mut storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let (authenticated,) = authenticate(&bytes, &mut storage);
    assert_eq!(
        authenticated.validate_recorded_native_custody(lamports, ONE_ROW_BYTES, false),
        Ok(()),
        "control: the unmodified record prices its own account"
    );

    // The rate is really IN the header, and exactly once: found by searching
    // the 48 header bytes for its own little-endian encoding rather than by
    // naming an offset this test would then be a second author of.
    let needle = FUNDED_RATE.to_le_bytes();
    let offsets = bytes
        .get(..48)
        .expect("header span")
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == needle.as_slice())
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    assert_eq!(
        offsets.len(),
        1,
        "the funded rate occupies exactly one header span"
    );
    let offset = *offsets.first().expect("one span");

    // A record claiming a rate the account was not funded at.
    let (lying, _) = funded_ledger(LATER_RATE);
    let mut liar_storage = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let (liar,) = authenticate(&lying, &mut liar_storage);
    assert_eq!(
        liar.validate_recorded_native_custody(lamports, ONE_ROW_BYTES, false),
        Err(Error::FundedRentNotEvidenced),
        "a record claiming a rent the balance does not evidence is refused, and named"
    );

    // A header carrying no rate at all -- every account funded before the field
    // existed. Zero prices every account at nothing, so it fails at DECODE
    // rather than reaching an arithmetic that would silently succeed.
    let mut absent = bytes.clone();
    absent
        .get_mut(offset..offset + 4)
        .expect("the span the search found")
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        FundingLedgerV2::decode(&absent).err(),
        Some(Error::FundedRentRateMissing),
        "an unrecorded rate fails closed at decode, never at a comparison against zero"
    );
    let mut storage_zero = [0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let view = manifest(&mut storage_zero);
    let manifest_id =
        ContentId::new(dclutch_sha256_adapter::digest(view.as_bytes())).expect("identity");
    let mut fresh = vec![0_u8; ONE_ROW_BYTES];
    assert_eq!(
        FundingLedgerV2::initialize(&mut fresh, manifest_id, view, 1, 0),
        Err(Error::FundedRentRateMissing),
        "and no founding may write a ledger that records no rate"
    );
}

/// THE RECORDING COSTS NOTHING, AND ONE RATE PRICES THE WHOLE COHORT.
///
/// Addendum E's nine chain readings, at both rates. Every one of them is
/// `(128 + len) * rate` to the lamport, which is why the persisted fact is the
/// rate: a `u64` minimum does not fit four reserved bytes and a `u32` rate does,
/// and one rate prices accounts of every width -- including a lookup table whose
/// width GROWS between the transaction that funded it and the one that reads it.
#[test]
fn one_rate_prices_every_account_cohort_fifteen_created() {
    assert_eq!(ACCOUNT_STORAGE_OVERHEAD_BYTES, 128);
    let cases = [
        (FUNDED_RATE, 264_usize, 2_482_536_u64), // funding ledger
        (FUNDED_RATE, 312, 2_786_520),           // certificate seat
        (FUNDED_RATE, 170, 1_887_234),           // payout ATA
        (FUNDED_RATE, 368, 3_141_168),           // Market
        (FUNDED_RATE, 2_128, 14_287_248),        // capability manifest
        (FUNDED_RATE, 416, 3_445_152),           // terminal session receipt
        (FUNDED_RATE, 1_720, 11_703_384),        // terminal lookup table
        (LATER_RATE, 264, 1_991_360),
        (LATER_RATE, 416, 2_763_520),
        (LATER_RATE, 1_720, 9_387_840),
        (LATER_RATE, 0, 650_240),
    ];
    for (rate, bytes, expected) in cases {
        assert_eq!(
            funded_rent_minimum_v2(rate, bytes),
            Ok(expected),
            "{rate} lamports per byte must price {bytes} bytes at {expected}"
        );
    }
    assert_eq!(
        funded_rent_minimum_v2(FUNDED_RATE, 264).expect("funded")
            - funded_rent_minimum_v2(LATER_RATE, 264).expect("today"),
        491_176,
        "the stranded amount is the rate gap times the footprint, and nothing moved it"
    );
}

/// A CLUSTER WHOSE RENT IS NOT AFFINE IN THE LENGTH IS REFUSED, NOT ROUNDED.
///
/// The derivation takes two readings, which pin an affine function, and then
/// checks both. A recorded rate that reproduces one length and not another
/// would price some other account of the same founding wrong, silently.
#[test]
fn a_rent_no_single_rate_reproduces_is_refused_rather_than_approximated() {
    assert_eq!(
        derive_funded_rent_rate_v2(128 * 6333, 264, 392 * 6333),
        Ok(FUNDED_RATE),
        "two agreeing readings derive the rate they agree on"
    );
    assert_eq!(
        derive_funded_rent_rate_v2(128 * 6333, 264, 392 * 6333 + 1),
        Err(Error::UnrepresentableRentRate),
        "a second reading one lamport off the affine line is refused"
    );
    assert_eq!(
        derive_funded_rent_rate_v2(128 * 6333 + 1, 264, 392 * 6333),
        Err(Error::UnrepresentableRentRate),
        "a zero-length reading that is not a multiple of the overhead is refused"
    );
    assert_eq!(
        derive_funded_rent_rate_v2(0, 264, 0),
        Err(Error::UnrepresentableRentRate),
        "a cluster charging no rent records no rate"
    );
    assert_eq!(
        funded_rent_minimum_v2(0, 264),
        Err(Error::FundedRentRateMissing),
        "and a zero rate never prices an account at nothing"
    );
}

/// A RECORDED PRINCIPAL CARRIES THE RATE THAT WROTE IT, AND ONE RATE PRICES
/// EVERY WIDTH THAT FOUNDING TOUCHED.
///
/// The floors over pre-existing accounts became `funded_rent_persists_v1`,
/// which is rate-free. The EXACTNESS checks over a persisted principal cannot
/// be: they must compare against a number. Comparing against
/// `Rent::minimum_balance` of the moment is what stranded cohort-15 in one
/// direction and would strand cohort-16's redeploy in the other, so the number
/// to compare against is recovered from the record itself.
///
/// The readings are cohort-15's, off devnet at finalized slot 493,000,156: its
/// seven Program accounts hold 1,038,612 lamports over 36 bytes and its seven
/// ProgramData accounts hold their own widths' minima -- fourteen accounts,
/// three widths shown here, one rate.
#[test]
fn a_recorded_principal_recovers_the_rate_that_wrote_it() {
    assert_eq!(
        funded_rent_rate_from_minimum_v1(1_038_612, 36),
        Ok(FUNDED_RATE),
        "cohort-15's Program accounts were funded at 6,333, which 164 x 6,333 says exactly"
    );
    assert_eq!(
        funded_rent_rate_from_minimum_v1(1_523_802_129, 240_485),
        Ok(FUNDED_RATE),
        "and its Registry ProgramData, at a width six thousand times larger"
    );
    assert_eq!(
        funded_rent_rate_from_minimum_v1(14_828_523_177, 2_341_341),
        Ok(FUNDED_RATE),
        "and its Trading ProgramData, the widest account the cohort deployed"
    );

    // The point of the recovery: one recorded principal prices the OTHER
    // account of the same founding, at a different width, with no sysvar in the
    // arithmetic at all. This is the shape `user_position_close_v1` now uses
    // over its two admission principals.
    let rate = funded_rent_rate_from_minimum_v1(1_038_612, 36).expect("rate");
    assert_eq!(
        funded_rent_minimum_v2(rate, 240_485),
        Ok(1_523_802_129),
        "one rate prices every width that founding touched"
    );

    // A DONATED LAMPORT PUTS THE READING OFF THE AFFINE LINE, and no rate
    // reproduces it. That is refused by name, never rounded to the nearest
    // plausible cluster -- the same hostile the recorded-rate path answers with
    // `FundedRentNotEvidenced`, asked of a principal instead of a balance.
    assert_eq!(
        funded_rent_rate_from_minimum_v1(1_038_613, 36),
        Err(Error::UnrepresentableRentRate),
        "one lamport above the minimum is not a rent-exempt minimum at any rate"
    );
    assert_eq!(
        funded_rent_rate_from_minimum_v1(1_038_611, 36),
        Err(Error::UnrepresentableRentRate),
        "and one lamport below it is not either"
    );
    assert_eq!(
        funded_rent_rate_from_minimum_v1(0, 36),
        Err(Error::UnrepresentableRentRate),
        "a zero principal records no rate rather than pricing every account at nothing"
    );
    assert_eq!(
        funded_rent_rate_from_minimum_v1(u64::from(u32::MAX) * 164 + 164, 36),
        Err(Error::UnrepresentableRentRate),
        "a principal implying a rate no u32 holds is refused, not truncated"
    );

    // THE POSITIVE CONTROL, two-sided: the rate really is what distinguishes
    // these numbers. Devnet's own live rate at that slot was 5,080 and the
    // genesis default prices a byte at 6,960; neither reproduces the balances
    // above, and the recovery says so rather than picking the closest.
    assert_eq!(funded_rent_minimum_v2(5_080, 36), Ok(833_120));
    assert_eq!(funded_rent_minimum_v2(6_960, 36), Ok(1_141_440));
    assert!(
        funded_rent_minimum_v2(5_080, 36) != Ok(1_038_612)
            && funded_rent_minimum_v2(6_960, 36) != Ok(1_038_612),
        "cohort-15 was funded at neither the rate of the moment nor the genesis constant"
    );
}
