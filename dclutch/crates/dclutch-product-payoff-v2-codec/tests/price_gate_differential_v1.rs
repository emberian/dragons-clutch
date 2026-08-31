//! The ported `DCLTPGT1` price gate, checked against the retained kernel and
//! against Lean's own corpora.
//!
//! `BASIS_ABI_UNIFICATION_V1` §6.3 puts the no-arbitrage gate at founding, in
//! `dclutch-core-sbf`, which reaches the basis through the payoff codec. The
//! kernel that first implemented the gate is a **dev-dependency** of that codec
//! and must stay one under `O-005` — so the gate is ported, and this is the
//! check that the port did not change what the gate decides.
//!
//! Two corpora, both Lean-emitted and byte-guarded by
//! `check-generated-price-gate.sh` with their counts pinned:
//!
//! - `PRICE_GATE_AGREEMENT_CASES_V1` (22) — certificates the specification
//!   admits, each carrying the price vector it certifies. **18 are at degree 2
//!   or 3 and the port admits every one and reproduces its prices exactly**;
//!   the other 4 declare degree 1 and are refused by name, because degree 1 is
//!   the graded family's and is exempt from this gate by proof.
//! - `PRICE_GATE_REFUSAL_CASES_V1` (45) — hostile certificates. The port must
//!   refuse every one, **for the same reason**: the kernel's stable
//!   `Error::tag` maps onto the codec's refusal one-for-one and the mapping is
//!   asserted per case, not merely "it errored". 23 match exactly; the other
//!   22 declare a degree outside this family and are refused at the earlier
//!   degree conjunct, which is asserted rather than skipped.
//!
//! The tag correspondence is the property worth having. A port that refused
//! everything would pass an accept/reject check on the refusal corpus and fail
//! the agreement corpus; a port that refused the right cases for the wrong
//! reasons would pass both. Only the correspondence catches a decoder whose
//! checks run in the wrong order.
//!
//! # The coverage hole the corpus leaves, and how it is filled
//!
//! Measured, the corpus reaches these kernel tags exactly against this codec:
//! `[0, 1, 2, 3, 4, 5, 15, 20, 29, 30]` — length, magic, schema, profile,
//! reserved, zero scale, degree, zero mass, basis mismatch, and the hull
//! identity itself. Every *other* guarded check — width, atom count, padding,
//! zero weight, atom order, weight mass, primitive scale, price partition —
//! is carried only by corpus cases that declare degree 1, so against this
//! family they would ship **untested**.
//!
//! `every_remaining_guarded_check_fires_at_degree_two` closes that by mutation:
//! one field of a known-good degree-2 certificate at a time, against a control
//! that asserts the unmutated record is admitted.

use dclutch_liability_basis_v2_kernel::{
    PRICE_GATE_AGREEMENT_CASES_V1, PRICE_GATE_REFUSAL_CASES_V1,
};
use dclutch_product_payoff_v2_codec::price_gate_v1::{decode_price_gate_v1, verify_price_gate_v1};
use dclutch_product_payoff_v2_codec::runtime_v3::Error;

/// Offset of the degree byte inside a `DCLTPGT1` certificate.
const CERTIFICATE_DEGREE_OFFSET: usize = 24;

/// The lowest degree the `SplineDegree2To3` family claims. Degree 0 and 1 are
/// the graded family's and are exempt from the price gate **by proof**, so a
/// certificate declaring one of them is not a certificate this codec has any
/// business verifying -- it is refused, by name, before anything else about it
/// is considered.
const FAMILY_MINIMUM_DEGREE: u8 = 2;

/// The degree a certificate declares, when it is long enough to declare one.
fn certificate_degree(bytes: &[u8]) -> Option<u8> {
    bytes.get(CERTIFICATE_DEGREE_OFFSET).copied()
}

const SCALE_OFFSET: usize = 12;
const KNOT_DENOMINATOR_OFFSET: usize = 16;
const DEGREE_OFFSET: usize = 32;
const KNOT_COUNT_OFFSET: usize = 33;
const KNOTS_OFFSET: usize = 48;

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

fn i64_at(bytes: &[u8], offset: usize) -> i64 {
    i64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
}

/// The founding-fixed quantities a certificate is verified against, read out of
/// the corpus case's own canonical basis record.
struct Basis {
    knots: Vec<i128>,
    knot_denominator: u64,
    payout_scale: u64,
    degree: u8,
    width: u32,
}

fn basis_of(request: &[u8]) -> Basis {
    let degree = request[DEGREE_OFFSET];
    let knot_count = usize::from(request[KNOT_COUNT_OFFSET]);
    let knots: Vec<i128> = (0..knot_count)
        .map(|slot| i128::from(i64_at(request, KNOTS_OFFSET + slot * 8)))
        .collect();
    let width = u32::try_from(knot_count - usize::from(degree) - 1).expect("width fits");
    Basis {
        knots,
        knot_denominator: u64::from(u32_at(request, KNOT_DENOMINATOR_OFFSET)),
        payout_scale: u64::from(u32_at(request, SCALE_OFFSET)),
        degree,
        width,
    }
}

/// The kernel's stable refusal tag, mapped onto this codec's refusal.
///
/// The correspondence is total over every tag the price-gate corpus reaches,
/// and it is written out rather than derived so that a port which collapsed two
/// distinct refusals into one would fail here instead of passing quietly.
fn expected_refusal(tag: u8) -> Option<Error> {
    Some(match tag {
        0 => Error::InvalidLength,
        1 => Error::InvalidMagic,
        2 => Error::UnsupportedSchema,
        3 => Error::PriceGateUnsupportedProfile,
        4 => Error::NonCanonicalReserved,
        5 => Error::ZeroScale,
        6 => Error::ZeroDenominator,
        11 => Error::ArithmeticOverflow,
        15 => Error::SplineDegreeOutOfProfile,
        19 => Error::SplineDegenerateSpan,
        20 => Error::PriceGateZeroMass,
        21 => Error::PriceGateWidthOutOfRange,
        22 => Error::PriceGateCapacity,
        23 => Error::PriceGateNonCanonicalPadding,
        24 => Error::PriceGateZeroAtomWeight,
        25 => Error::PriceGateNonCanonicalAtomOrder,
        26 => Error::PriceGateWeightMassMismatch,
        27 => Error::PriceGateNonPrimitiveWeightScale,
        28 => Error::PriceGatePriceNotPartition,
        29 => Error::PriceGateBasisMismatch,
        30 => Error::PriceGateHullRefused,
        _ => return None,
    })
}

/// **The port admits every certificate the specification admits, and
/// reproduces the price vector each one certifies.**
///
/// The prices are checked against `case.prices`, which Lean emitted — not
/// against anything the port computed — so this says the hull identity holds
/// with the payouts *this* evaluator produces at *this* rounding rule. That is
/// the load-bearing claim: the gate was proved against the kernel's evaluator,
/// and the whole point of the port is that the live one agrees.
#[test]
fn the_port_admits_every_certificate_the_specification_admits() {
    let mut admitted = 0_usize;
    let mut outside_the_family = 0_usize;
    for (index, case) in PRICE_GATE_AGREEMENT_CASES_V1.iter().enumerate() {
        let basis = basis_of(&case.basis);
        let declared = certificate_degree(&case.certificate).expect("canonical certificate");
        let outcome = verify_price_gate_v1(
            basis.knots.as_slice(),
            basis.knot_denominator,
            basis.payout_scale,
            basis.degree,
            basis.width,
            &case.certificate,
        );
        if declared < FAMILY_MINIMUM_DEGREE {
            // Not a skip -- an assertion. The specification admits this
            // certificate because the kernel's gate serves degrees 1 through 3;
            // this codec's gate serves the curved family only, and refuses a
            // degree-1 certificate by name rather than evaluating one it has no
            // evaluator for. `admit_basis_selection_v3` refuses the converse
            // too: a certificate offered alongside an exempt kind is
            // `PriceGateCertificateUnexpected`, never a harmless extra.
            assert_eq!(
                outcome,
                Err(Error::SplineDegreeOutOfProfile),
                "agreement case {index} declares degree {declared}, outside this family"
            );
            outside_the_family += 1;
            continue;
        }
        let certificate = outcome.unwrap_or_else(|error| {
            panic!("agreement case {index} (degree {declared}) refused: {error:?}")
        });
        assert_eq!(
            certificate.active_prices(),
            &case.prices[..case.width],
            "agreement case {index}: certified prices differ from Lean's"
        );
        assert_eq!(
            certificate.atom_count(),
            case.atom_count,
            "agreement case {index}: atom count"
        );
        admitted += 1;
    }
    println!(
        "price-gate agreement corpus: {admitted} curved certificates admitted and \
         reproduced, {outside_the_family} degree-1 certificates outside this family"
    );
    assert_eq!(
        admitted + outside_the_family,
        PRICE_GATE_AGREEMENT_CASES_V1.len(),
        "every emitted agreement case must be accounted for"
    );
    assert_eq!(
        (admitted, outside_the_family),
        (18, 4),
        "the emitted agreement corpus split moved"
    );
}

/// **The port refuses every hostile certificate, for the same reason.**
///
/// Not merely "it errored": the kernel's stable tag is mapped onto this codec's
/// refusal and compared per case. A decoder whose checks ran in a different
/// order would still refuse every case here and would still fail this test,
/// which is exactly what makes it worth running.
#[test]
fn the_port_refuses_every_hostile_certificate_for_the_same_reason() {
    let mut exact = 0_usize;
    let mut outside_the_family = 0_usize;
    let mut unmapped = Vec::new();
    let mut covered_tags = Vec::new();
    for (index, case) in PRICE_GATE_REFUSAL_CASES_V1.iter().enumerate() {
        let basis = basis_of(&case.basis);
        let offered = &case.certificate[..case.certificate_len];
        let outcome = verify_price_gate_v1(
            basis.knots.as_slice(),
            basis.knot_denominator,
            basis.payout_scale,
            basis.degree,
            basis.width,
            offered,
        );
        let error = match outcome {
            Ok(_) => panic!(
                "refusal case {index} (kernel tag {}) was ADMITTED by the port",
                case.error_tag
            ),
            Err(error) => error,
        };
        let expected = match expected_refusal(case.error_tag) {
            Some(expected) => expected,
            None => {
                unmapped.push(case.error_tag);
                continue;
            }
        };
        // A certificate declaring a degree outside the curved family is refused
        // for that, ahead of whatever else is wrong with it -- the degree check
        // precedes the width, atom-count and hull checks in the specification's
        // own order. Both refusals are correct; they are simply at different
        // conjuncts, and this codec's is the earlier one.
        let declared = certificate_degree(offered);
        if declared.is_some_and(|degree| degree < FAMILY_MINIMUM_DEGREE) && error != expected {
            assert_eq!(
                error,
                Error::SplineDegreeOutOfProfile,
                "refusal case {index} declares a degree outside the family, so the \
                 only admissible earlier refusal is the degree one"
            );
            outside_the_family += 1;
            continue;
        }
        assert_eq!(
            error, expected,
            "refusal case {index}: kernel tag {} maps to {expected:?}, port said {error:?}",
            case.error_tag
        );
        exact += 1;
        if !covered_tags.contains(&case.error_tag) {
            covered_tags.push(case.error_tag);
        }
    }
    covered_tags.sort_unstable();
    println!(
        "price-gate refusal corpus: {exact} refused for exactly the specified reason \
         (kernel tags {covered_tags:?}), {outside_the_family} refused earlier for a \
         degree outside this family"
    );
    assert!(
        unmapped.is_empty(),
        "the refusal corpus reached tags with no mapping: {unmapped:?}"
    );
    assert_eq!(
        exact + outside_the_family,
        PRICE_GATE_REFUSAL_CASES_V1.len(),
        "every emitted refusal case must be accounted for"
    );
    assert_eq!(
        (exact, outside_the_family),
        (23, 22),
        "the emitted refusal corpus split moved"
    );
}

/// The two emissions of one specification agree.
///
/// `generated_price_gate_v1.rs` in this crate and `generated_price_gate.rs` in
/// the kernel are emitted from the same Lean ABI owner into two destinations,
/// because founding cannot reach the kernel under `O-005`. Each is byte-guarded
/// against its emitter; this is the check that the two guards are guarding the
/// same numbers.
#[test]
fn the_two_price_gate_emissions_agree() {
    use dclutch_liability_basis_v2_kernel as kernel;
    use dclutch_product_payoff_v2_codec::price_gate_v1 as port;

    assert_eq!(
        port::PRICE_GATE_REQUEST_BYTES_V1,
        kernel::PRICE_GATE_REQUEST_BYTES_V1
    );
    assert_eq!(port::PRICE_GATE_MAGIC_V1, kernel::PRICE_GATE_MAGIC_V1);
    assert_eq!(
        port::PRICE_GATE_MAX_ATOMS_V1,
        kernel::PRICE_GATE_MAX_ATOMS_V1
    );
    assert_eq!(
        port::PRICE_GATE_MAX_WIDTH_V1,
        kernel::PRICE_GATE_MAX_WIDTH_V1
    );
    assert_eq!(
        port::PRICE_GATE_EXEMPT_DEGREE_V1,
        kernel::PRICE_GATE_EXEMPT_DEGREE_V1
    );
    assert_eq!(
        port::PRICE_GATE_RESERVED_OFFSET_V1,
        kernel::PRICE_GATE_RESERVED_OFFSET_V1
    );
    assert_eq!(
        port::PRICE_GATE_RESERVED_BYTES_V1,
        kernel::PRICE_GATE_RESERVED_BYTES_V1
    );
    assert_eq!(
        port::PRICE_GATE_PRICES_OFFSET_V1,
        kernel::PRICE_GATE_PRICES_OFFSET_V1
    );
    assert_eq!(
        port::PRICE_GATE_WEIGHTS_OFFSET_V1,
        kernel::PRICE_GATE_WEIGHTS_OFFSET_V1
    );
    assert_eq!(
        port::PRICE_GATE_NUMERATORS_OFFSET_V1,
        kernel::PRICE_GATE_NUMERATORS_OFFSET_V1
    );
    assert_eq!(
        port::PRICE_GATE_DENOMINATORS_OFFSET_V1,
        kernel::PRICE_GATE_DENOMINATORS_OFFSET_V1
    );
}

/// A certificate that decodes structurally is still refused against a basis it
/// was not issued for. The hull identity is only meaningful relative to a
/// specific basis, and this is the conjunct that says so.
#[test]
fn a_certificate_issued_against_another_basis_refuses() {
    let case = &PRICE_GATE_AGREEMENT_CASES_V1[0];
    let basis = basis_of(&case.basis);
    decode_price_gate_v1(&case.certificate).expect("the certificate itself is canonical");

    // Same certificate, a payout scale it was not issued against.
    assert_eq!(
        verify_price_gate_v1(
            basis.knots.as_slice(),
            basis.knot_denominator,
            basis.payout_scale + 1,
            basis.degree,
            basis.width,
            &case.certificate,
        ),
        Err(Error::PriceGateBasisMismatch)
    );
}

/// **The hostile checks the emitted corpus cannot reach, reached by mutation.**
///
/// The refusal corpus exercises kernel tags `[0, 1, 2, 3, 4, 5, 15, 20, 29, 30]`
/// against this codec exactly. The remaining guarded tags — width, atom count,
/// padding, zero weight, atom order, weight mass, primitive scale, price
/// partition — are all carried by corpus cases that declare **degree 1**, which
/// this family refuses at an earlier conjunct. Those checks would therefore
/// ship untested here, which is precisely the situation a hostile table exists
/// to prevent.
///
/// So each one is reached directly: take a certificate the specification
/// admits at degree 2, break exactly one field, and require the named refusal.
/// Every mutation is a single-field edit of a known-good record, so a passing
/// assertion says the check fired *because of that field* and not because the
/// record was malformed generally.
#[test]
fn every_remaining_guarded_check_fires_at_degree_two() {
    // The first curved case with at least two atoms, so the ordering check has
    // something to order.
    let case = PRICE_GATE_AGREEMENT_CASES_V1
        .iter()
        .find(|case| {
            certificate_degree(&case.certificate).is_some_and(|d| d >= FAMILY_MINIMUM_DEGREE)
                && case.atom_count >= 2
        })
        .expect("a curved multi-atom certificate");
    let basis = basis_of(&case.basis);

    let verify = |bytes: &[u8]| {
        verify_price_gate_v1(
            basis.knots.as_slice(),
            basis.knot_denominator,
            basis.payout_scale,
            basis.degree,
            basis.width,
            bytes,
        )
    };
    // The control: unmutated, it is admitted. Without this every assertion
    // below could be passing for the wrong reason.
    verify(&case.certificate).expect("the unmutated certificate is admitted");

    let mutate = |edit: &dyn Fn(&mut [u8; 320])| {
        let mut bytes = case.certificate;
        edit(&mut bytes);
        bytes
    };
    let put_u64 = |bytes: &mut [u8; 320], offset: usize, value: u64| {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    };
    let get_u64 = |bytes: &[u8; 320], offset: usize| -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
    };

    const WIDTH: usize = 25;
    const ATOMS: usize = 26;
    const MASS: usize = 16;
    const PRICES: usize = 40;
    const WEIGHTS: usize = 120;
    const NUMERATORS: usize = 200;
    const DENOMINATORS: usize = 280;

    // Width: zero, and past the affine-Caratheodory capacity.
    for width in [0_u8, 1, 11, 255] {
        assert_eq!(
            verify(&mutate(&|bytes| bytes[WIDTH] = width)),
            Err(Error::PriceGateWidthOutOfRange),
            "width {width}"
        );
    }
    // Atom count: zero, and past capacity.
    for atoms in [0_u8, 11, 255] {
        assert_eq!(
            verify(&mutate(&|bytes| bytes[ATOMS] = atoms)),
            Err(Error::PriceGateCapacity),
            "atom count {atoms}"
        );
    }
    // Padding past a declared width is not free space.
    assert_eq!(
        verify(&mutate(&|bytes| put_u64(bytes, PRICES + 9 * 8, 1))),
        Err(Error::PriceGateNonCanonicalPadding),
        "a nonzero price past the declared width"
    );
    assert_eq!(
        verify(&mutate(&|bytes| put_u64(bytes, WEIGHTS + 9 * 8, 1))),
        Err(Error::PriceGateNonCanonicalPadding),
        "a nonzero weight past the declared atom count"
    );
    // A zero denominator, and a zero atom weight.
    assert_eq!(
        verify(&mutate(
            &|bytes| bytes[DENOMINATORS..DENOMINATORS + 4].copy_from_slice(&0_u32.to_le_bytes())
        )),
        Err(Error::ZeroDenominator)
    );
    assert_eq!(
        verify(&mutate(&|bytes| put_u64(bytes, WEIGHTS, 0))),
        Err(Error::PriceGateZeroAtomWeight)
    );
    // Atom coordinates must strictly increase; swapping the first two breaks it.
    assert_eq!(
        verify(&mutate(&|bytes| {
            let first: [u8; 8] = bytes[NUMERATORS..NUMERATORS + 8].try_into().expect("eight");
            let second: [u8; 8] = bytes[NUMERATORS + 8..NUMERATORS + 16]
                .try_into()
                .expect("eight");
            bytes[NUMERATORS..NUMERATORS + 8].copy_from_slice(&second);
            bytes[NUMERATORS + 8..NUMERATORS + 16].copy_from_slice(&first);
        })),
        Err(Error::PriceGateNonCanonicalAtomOrder)
    );
    // The weights must sum to exactly the declared mass.
    assert_eq!(
        verify(&mutate(&|bytes| {
            let mass = get_u64(bytes, MASS);
            put_u64(bytes, MASS, mass + 1);
        })),
        Err(Error::PriceGateWeightMassMismatch)
    );
    // Doubling weights and mass together leaves the hull identity unchanged, so
    // only the primitive representative is canonical.
    assert_eq!(
        verify(&mutate(&|bytes| {
            put_u64(bytes, MASS, get_u64(bytes, MASS) * 2);
            for slot in 0..10 {
                let offset = WEIGHTS + slot * 8;
                put_u64(bytes, offset, get_u64(bytes, offset) * 2);
            }
        })),
        Err(Error::PriceGateNonPrimitiveWeightScale)
    );
    // The certified prices must partition the declared scale.
    assert_eq!(
        verify(&mutate(&|bytes| {
            put_u64(bytes, PRICES, get_u64(bytes, PRICES) + 1);
        })),
        Err(Error::PriceGatePriceNotPartition)
    );
}

/// **A valid certificate, against a real `DCLTPAY3` degree-2 record.**
///
/// Everything above drives the gate from the kernel's own corpus, which speaks
/// `DCLTLBV2`. This one closes the loop the cut actually needs: a certificate
/// verified against a basis decoded from the live wire, through the live
/// evaluator, at the blessed rounding.
///
/// The certificate is built the cheapest way that is genuinely valid rather
/// than contrived. Take **one** atom, with `weight == mass == 1`. Then the hull
/// identity collapses to `price_j * 1 == 1 * payout_j`, so the certified price
/// vector *is* the payout vector at that coordinate — and it partitions the
/// scale for free, because a payout vector already does. `gcd(1, 1) == 1`, so
/// the weight scale is primitive; one atom is trivially strictly increasing and
/// is inside the affine-Carathéodory capacity. Every structural conjunct holds
/// by construction, which is what makes this a test of the *hull* check rather
/// than of the decoder.
#[test]
fn a_single_atom_certificate_verifies_against_a_live_wire_degree_two_basis() {
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, basis_record_bytes_v3, compile_basis_v3,
    };

    const SCALE: u64 = 1_000_000;
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    let failure = vec![200_000_u64; 5];

    // The atom coordinate the certificate will witness.
    let (atom_numerator, atom_denominator) = (3_i64, 2_u32);

    // A real record on the live wire.
    let kind = BasisKindV3::SplineDegree2To3 {
        degree: 2,
        interior_multiplicity: false,
    };
    let width = basis_record_bytes_v3(kind, 5, knots.len(), 0).expect("record width");
    let mut record = vec![0_u8; width];
    compile_basis_v3(
        BasisInputV3 {
            kind,
            product_id: [1_u8; 32],
            result_domain_id: [2_u8; 32],
            coordinate_domain_id: [3_u8; 32],
            result_unit_id: [4_u8; 32],
            evaluator_release_id: [5_u8; 32],
            basis_width: 5,
            payout_scale: SCALE,
            knot_denominator: 1,
            knots: &knots,
            terms: &[],
            failure_payouts: &failure,
            price_gate_certificate_digest: [9_u8; 32],
        },
        &mut record,
    )
    .expect("a curved record compiles");
    let basis = ProductBasisV3::decode(&record).expect("and decodes");

    // The payouts the BASIS produces at that coordinate. The certificate never
    // supplies these; it only says where to look.
    let mut payouts = vec![0_u64; 5];
    basis
        .evaluate_rational(
            i128::from(atom_numerator),
            u64::from(atom_denominator),
            &mut payouts,
        )
        .expect("the live evaluator pays");
    assert_eq!(payouts.iter().sum::<u64>(), SCALE, "an exact partition");

    let mut certificate = [0_u8; 320];
    certificate[0..8].copy_from_slice(b"DCLTPGT1");
    certificate[8..10].copy_from_slice(&1_u16.to_le_bytes());
    certificate[10..12].copy_from_slice(&1_u16.to_le_bytes());
    certificate[12..16].copy_from_slice(&u32::try_from(SCALE).expect("scale").to_le_bytes());
    certificate[16..24].copy_from_slice(&1_u64.to_le_bytes()); // mass
    certificate[24] = 2; // degree
    certificate[25] = 5; // width
    certificate[26] = 1; // atom count
    for (claim, payout) in payouts.iter().enumerate() {
        certificate[40 + claim * 8..48 + claim * 8].copy_from_slice(&payout.to_le_bytes());
    }
    certificate[120..128].copy_from_slice(&1_u64.to_le_bytes()); // weight
    certificate[200..208].copy_from_slice(&atom_numerator.to_le_bytes());
    certificate[280..284].copy_from_slice(&atom_denominator.to_le_bytes());

    let verified = verify_price_gate_v1(
        &basis,
        basis.knot_denominator(),
        basis.payout_scale(),
        2,
        basis.basis_width(),
        &certificate,
    )
    .expect("the hull identity holds");
    assert_eq!(verified.active_prices(), payouts.as_slice());
    assert_eq!(verified.atom_count(), 1);

    // **Red-proof.** Move one certified price by a single atom and the hull
    // identity fails — so the check is load-bearing rather than vacuous.
    let mut forged = certificate;
    let bumped = payouts[0] + 1;
    forged[40..48].copy_from_slice(&bumped.to_le_bytes());
    assert_eq!(
        verify_price_gate_v1(
            &basis,
            basis.knot_denominator(),
            basis.payout_scale(),
            2,
            basis.basis_width(),
            &forged,
        ),
        // The prices no longer partition the scale, which is checked before the
        // hull; both are refusals of the same forgery.
        Err(Error::PriceGatePriceNotPartition)
    );

    // A forgery that DOES still partition the scale reaches the hull check and
    // is refused there, which is the assertion that matters.
    let mut balanced = certificate;
    balanced[40..48].copy_from_slice(&(payouts[0] + 1).to_le_bytes());
    balanced[48..56].copy_from_slice(&(payouts[1] - 1).to_le_bytes());
    assert_eq!(
        verify_price_gate_v1(
            &basis,
            basis.knot_denominator(),
            basis.payout_scale(),
            2,
            basis.basis_width(),
            &balanced,
        ),
        Err(Error::PriceGateHullRefused)
    );
}
