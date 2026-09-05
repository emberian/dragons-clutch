//! The de Boor port, checked against the retained kernel as a differential
//! reference.
//!
//! `BASIS_ABI_UNIFICATION_V1` keeps `dclutch-liability-basis-v2-kernel` as a
//! non-authoritative differential reference under `O-005`, on the grounds that
//! an independent handwritten implementation of the same mathematics is the
//! strongest cheap check available on a de Boor port. This is that check.
//!
//! Every case is driven through the kernel's **real decoder** from real
//! `DCLTLBV2` record bytes rather than through a hand-built request struct, so
//! the reference side is the shipping kernel path and not a reconstruction of
//! it.
//!
//! Three things are asserted per case:
//!
//! 1. **The exact rational weights agree**, integer for integer — same common
//!    denominator, same local numerators, same support offset, same located
//!    span. This is the port's correctness claim, and it is exact rather than
//!    approximate because both sides carry the weights as integers over one
//!    accumulating denominator.
//! 2. **The cumulative-floor apportionment agrees**, atom for atom. The port
//!    carries the kernel's boundary across unchanged, so this pins the rounding
//!    too and not merely the weights.
//! 3. **The blessed rule's two defining properties hold** — it partitions the
//!    scale exactly, and every claim lands within one atom of its exact
//!    rational share. Those are the grounds WAVE `76e2ca3f` ruled on, asserted
//!    as properties of the surviving rule rather than as a comparison against
//!    the deleted one.

#![allow(clippy::indexing_slicing, clippy::panic)]

use dclutch_liability_basis_v2_kernel::spline as kernel;
use dclutch_product::payoff::runtime_v3::Error;
use dclutch_product::payoff::spline_eval_v3::{
    SPLINE_COORDINATE_DENOMINATOR_CEILING_V3, SplineWeightsV3, apportion_cumulative_v3,
    evaluate_spline_weights_v3, spline_arithmetic_envelope_v3,
};

const SPLINE_REQUEST_BYTES: usize = 144;
const KNOTS_OFFSET: usize = 48;

/// One differential case, in the vocabulary both sides accept.
struct Case {
    note: &'static str,
    degree: u8,
    knots: &'static [i64],
    coordinate_numerator: i64,
    coordinate_denominator: u32,
    knot_denominator: u32,
    scale: u32,
}

/// Cases chosen to exercise the structural corners, not merely the happy path:
/// clamped and unclamped knot vectors at both degrees, coordinates below, on,
/// inside and above the domain, interior knot multiplicity (which is the whole
/// reason a spline kind exists), a coordinate denominator that makes the
/// coordinate a genuine fraction, and scales that force the rounding boundary
/// to actually round.
const CASES: &[Case] = &[
    Case {
        note: "degree 2, clamped uniform, coordinate mid-domain",
        degree: 2,
        knots: &[0, 0, 0, 1, 2, 3, 3, 3],
        coordinate_numerator: 3,
        coordinate_denominator: 2,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 2, clamped uniform, coordinate on an interior knot",
        degree: 2,
        knots: &[0, 0, 0, 1, 2, 3, 3, 3],
        coordinate_numerator: 2,
        coordinate_denominator: 1,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 2, coordinate below the domain clamps to the first span",
        degree: 2,
        knots: &[0, 0, 0, 1, 2, 3, 3, 3],
        coordinate_numerator: -50,
        coordinate_denominator: 1,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 2, coordinate above the domain clamps to the last span",
        degree: 2,
        knots: &[0, 0, 0, 1, 2, 3, 3, 3],
        coordinate_numerator: 500,
        coordinate_denominator: 1,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 2, unclamped uniform knots",
        degree: 2,
        knots: &[0, 1, 2, 3, 4, 5, 6, 7],
        coordinate_numerator: 7,
        coordinate_denominator: 2,
        knot_denominator: 1,
        scale: 1_000_000,
    },
    Case {
        note: "degree 2, interior multiplicity collapses a span",
        degree: 2,
        knots: &[0, 1, 2, 2, 3, 4, 5, 6],
        coordinate_numerator: 2,
        coordinate_denominator: 1,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 2, interior multiplicity, coordinate just inside the collapsed knot",
        degree: 2,
        knots: &[0, 1, 2, 2, 3, 4, 5, 6],
        coordinate_numerator: 9,
        coordinate_denominator: 4,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 3, clamped uniform, coordinate mid-domain",
        degree: 3,
        knots: &[0, 0, 0, 0, 1, 2, 3, 3, 3, 3],
        coordinate_numerator: 3,
        coordinate_denominator: 2,
        knot_denominator: 1,
        scale: 100,
    },
    Case {
        note: "degree 3, unclamped uniform, awkward scale forces the boundary to round",
        degree: 3,
        knots: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        coordinate_numerator: 9,
        coordinate_denominator: 2,
        knot_denominator: 1,
        scale: 7,
    },
    Case {
        note: "degree 3, knot denominator above one",
        degree: 3,
        knots: &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18],
        coordinate_numerator: 5,
        coordinate_denominator: 1,
        knot_denominator: 2,
        scale: 1_000,
    },
    Case {
        note: "degree 2, prime scale over a fractional coordinate",
        degree: 2,
        knots: &[0, 0, 0, 5, 9, 14, 14, 14],
        coordinate_numerator: 23,
        coordinate_denominator: 3,
        knot_denominator: 1,
        scale: 97,
    },
];

/// Build one canonical 144-byte `DCLTLBV2` spline request.
fn record(case: &Case) -> [u8; SPLINE_REQUEST_BYTES] {
    let mut bytes = [0_u8; SPLINE_REQUEST_BYTES];
    bytes[0..8].copy_from_slice(b"DCLTLBV2");
    bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&2_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&case.scale.to_le_bytes());
    bytes[16..20].copy_from_slice(&case.knot_denominator.to_le_bytes());
    bytes[20..24].copy_from_slice(&case.coordinate_denominator.to_le_bytes());
    bytes[24..32].copy_from_slice(&case.coordinate_numerator.to_le_bytes());
    bytes[32] = case.degree;
    bytes[33] = u8::try_from(case.knots.len()).expect("knot count fits a byte");
    for (slot, knot) in case.knots.iter().enumerate() {
        let start = KNOTS_OFFSET + slot * 8;
        bytes[start..start + 8].copy_from_slice(&knot.to_le_bytes());
    }
    bytes
}

fn width_of(case: &Case) -> u32 {
    u32::try_from(case.knots.len() - usize::from(case.degree) - 1).expect("width fits")
}

fn port_weights(case: &Case) -> SplineWeightsV3 {
    let knots: Vec<i128> = case.knots.iter().map(|knot| i128::from(*knot)).collect();
    evaluate_spline_weights_v3(
        knots.as_slice(),
        u64::from(case.knot_denominator),
        i128::from(case.coordinate_numerator),
        u64::from(case.coordinate_denominator),
        case.degree,
        width_of(case),
    )
    .unwrap_or_else(|error| panic!("port refused {}: {error:?}", case.note))
}

/// **The port's correctness claim.** The exact rational weights the port
/// computes are the same integers the kernel computes, on every case.
#[test]
fn the_port_and_the_kernel_agree_on_the_exact_weights() {
    for case in CASES {
        let bytes = record(case);
        let request = kernel::decode_spline_request_v2(&bytes)
            .unwrap_or_else(|error| panic!("kernel refused {}: {error:?}", case.note));
        let reference = kernel::evaluate_spline_basis_v2(&request)
            .unwrap_or_else(|error| panic!("kernel refused {}: {error:?}", case.note));
        let ported = port_weights(case);

        assert_eq!(
            ported.denominator_u128().expect("corpus denominator fits"),
            reference.denominator,
            "common denominator, {}",
            case.note
        );
        assert_eq!(ported.span, reference.span, "located span, {}", case.note);
        assert_eq!(
            ported.offset, reference.offset,
            "support offset, {}",
            case.note
        );
        assert_eq!(
            ported.local_len, reference.local_len,
            "support length, {}",
            case.note
        );
        assert_eq!(ported.width, reference.width, "width, {}", case.note);
        for claim in 0..reference.width {
            assert_eq!(
                ported
                    .numerator_u128_at(claim)
                    .expect("corpus numerator fits"),
                reference.numerator_at(claim),
                "weight numerator of claim {claim}, {}",
                case.note
            );
        }
    }
}

/// The ported cumulative boundary is the kernel's boundary, atom for atom. The
/// weights agreeing would not by itself pin the rounding.
#[test]
fn the_port_and_the_kernel_agree_on_the_cumulative_apportionment() {
    for case in CASES {
        let bytes = record(case);
        let reference = kernel::decode_and_evaluate_spline_v2(&bytes)
            .unwrap_or_else(|error| panic!("kernel refused {}: {error:?}", case.note));
        let ported = port_weights(case);
        let mut output = vec![0_u64; ported.width];
        apportion_cumulative_v3(&ported, u64::from(case.scale), &mut output)
            .unwrap_or_else(|error| panic!("port refused {}: {error:?}", case.note));
        assert_eq!(output.as_slice(), reference.active(), "{}", case.note);
    }
}

/// A realistic cubic at the published coordinate-denominator ceiling forces
/// the live recurrence above `u128`.  Its result is checked against the
/// independent one-span Bernstein identity, evaluated here with reduced exact
/// integers rather than by either de Boor implementation.
#[test]
fn a_wide_cubic_matches_the_independent_exact_bernstein_oracle() {
    let knots = [0_i128, 0, 0, 0, 3, 3, 3, 3];
    let coordinate_denominator = SPLINE_COORDINATE_DENOMINATOR_CEILING_V3;
    let scale = 1_000_003_u64;
    spline_arithmetic_envelope_v3(knots.as_slice(), 3, 4, scale)
        .expect("the realistic cubic is foundable");
    let evaluated =
        evaluate_spline_weights_v3(knots.as_slice(), 1, 1, coordinate_denominator, 3, 4)
            .expect("the admitted cubic evaluates at the closed ceiling");
    assert!(
        evaluated.denominator_u128().is_err(),
        "this vector must exercise the widened recurrence, not the old u128 path"
    );
    let mut actual = [0_u64; 4];
    apportion_cumulative_v3(&evaluated, scale, &mut actual)
        .expect("the widened cumulative floor apportions");

    // For one clamped cubic span [0, 3], at x = 1/d, the reduced exact
    // Bernstein numerators are `(3d-1)^3`, `3(3d-1)^2`, `3(3d-1)`, `1`
    // over `(3d)^3`. This is a different derivation from the live triangular
    // recurrence and all values fit u128 after reduction.
    let span = 3_u128 * u128::from(coordinate_denominator);
    let left = span - 1;
    let exact = [left.pow(3), 3 * left.pow(2), 3 * left, 1];
    let exact_denominator = span.pow(3);
    assert_eq!(exact.iter().sum::<u128>(), exact_denominator);
    let mut expected = [0_u64; 4];
    let mut running = 0_u128;
    let mut carried = 0_u128;
    for (slot, numerator) in exact.iter().enumerate() {
        running += *numerator;
        let boundary = u128::from(scale) * running / exact_denominator;
        expected[slot] = u64::try_from(boundary - carried).expect("payout is scale-bounded");
        carried = boundary;
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.iter().sum::<u64>(), scale);
}

/// **The blessed rule partitions the scale exactly**, on every case. This is
/// the property that makes an apportionment admissible at all: the claims sum
/// to `Q` with no residue placed anywhere by hand.
#[test]
fn the_blessed_boundary_partitions_the_scale_exactly() {
    for case in CASES {
        let weights = port_weights(case);
        let scale = u64::from(case.scale);
        let mut cumulative = vec![0_u64; weights.width];
        apportion_cumulative_v3(&weights, scale, &mut cumulative).expect("cumulative apportions");
        assert_eq!(
            cumulative.iter().sum::<u64>(),
            scale,
            "cumulative sums to Q, {}",
            case.note
        );
    }
}

/// The cumulative boundary keeps the exact zero outside the local support.
///
/// This is the property that makes it the safer of the two candidates, and it
/// is asserted rather than described.
#[test]
fn the_cumulative_boundary_pays_nothing_outside_the_support() {
    for case in CASES {
        let weights = port_weights(case);
        let mut output = vec![0_u64; weights.width];
        apportion_cumulative_v3(&weights, u64::from(case.scale), &mut output)
            .expect("cumulative apportions");
        for claim in 0..weights.width {
            if weights.is_zero_at(claim) {
                assert_eq!(
                    output.get(claim).copied(),
                    Some(0),
                    "claim {claim} is outside the support and must be paid nothing, {}",
                    case.note
                );
            }
        }
    }
}

/// **The first ground of the cumulative-floor ruling, kept as a property.**
///
/// WAVE `76e2ca3f` ruled cumulative-floor the spline rounding rule on a
/// measurement: over eleven cases at both degrees it kept every claim within
/// one atom of its exact rational share, and floor-plus-complement did not
/// (2 of 11 diverged, worst 2 atoms). The rejected implementation is deleted,
/// so the comparison that produced that number cannot be re-run here — but the
/// property it was measuring can be, and directly.
///
/// Each claim's exact share is `Q * w_i / D`. Cumulative-floor's telescoping
/// guarantees `|payout_i - share_i| < 1` for every claim; asserting that is
/// strictly stronger than asserting it beat the alternative, and it stays true
/// no matter what the alternative was.
#[test]
fn every_claim_lands_within_one_atom_of_its_exact_share() {
    for case in CASES {
        let weights = port_weights(case);
        let scale = u128::from(case.scale);
        let mut output = vec![0_u64; weights.width];
        apportion_cumulative_v3(&weights, u64::from(case.scale), &mut output)
            .expect("cumulative apportions");
        for (claim, apportioned) in output.iter().enumerate() {
            let paid = u128::from(*apportioned);
            // The exact share is `Q * w / D`. Compared without dividing, and
            // symmetrically: telescoping floors may land either side of the
            // exact value, so the claim is `|paid - share| < 1`, which scaled
            // by `D` is `|paid * D - Q * w| < D`.
            let numerator = weights
                .numerator_u128_at(claim)
                .expect("corpus numerator fits");
            let denominator = weights.denominator_u128().expect("corpus denominator fits");
            let share_numerator = scale.checked_mul(numerator).expect("share fits");
            let paid_numerator = paid.checked_mul(denominator).expect("bound fits");
            assert!(
                paid_numerator.abs_diff(share_numerator) < denominator,
                "claim {claim} paid {paid} is a full atom or more from its exact share, {}",
                case.note
            );
        }
    }
}

/// **Hostile 22, as the design states it.** Every case in the Lean-emitted
/// `SPLINE_AGREEMENT_CASES_V2` that is expressible in this family — that is,
/// every degree-2 and degree-3 case — is reproduced exactly by the port.
///
/// This is a stronger check than the hand-written table above, and it is the
/// one the design actually asks for. The expectations here were computed by
/// `DClutchSemantics.LiabilityBasisV2Spline`, not by the kernel and not by this
/// crate, and the file carrying them is byte-guarded by
/// `check-generated-spline.sh` with its case count pinned at 28. So a pass
/// chains the port to the specification: Lean emits the expectation, the guard
/// pins the emission, and this test says the port meets it.
///
/// # Why this is 19 of 28, and why 28 of 28 would be the wrong target
///
/// Nine cases are at degree 1, which this family does not claim: degree 1 is
/// the graded family's, reached through `BasisShapeV3`. It is tempting to route
/// them through the graded evaluator so the number reads 28, and that would be
/// **wrong**, not merely awkward.
///
/// A degree-1 B-spline basis function is exactly a tent, so the record is
/// constructible — `Tent { left: i, peak: i + 1, right: i + 2 }` per claim. But
/// the graded family rounds by flooring each term and handing the residue to
/// its structurally reserved last claim, and this corpus's expectations were
/// computed with **cumulative-floor**. WAVE `76e2ca3f` ruled those are
/// different functions, on a measurement showing they diverge on 2 of 11 cases.
/// So asserting the degree-1 cases against the graded evaluator would assert an
/// agreement that does not hold, and making it hold would mean changing the
/// live graded family's rounding — a money change nobody has ruled.
///
/// The corpus is fully executed across the tree regardless: the kernel's own
/// suite (`dclutch-liability-basis-v2-kernel/tests/spline.rs`) runs all 28 with
/// no skip, and it is the differential reference under `O-005`. What this test
/// says is the narrower and truer thing — every case the *ported* evaluator
/// claims, it reproduces exactly.
///
/// The split is pinned rather than floored, so a case cannot quietly move from
/// exercised to skipped.
#[test]
fn the_port_reproduces_the_lean_emitted_agreement_corpus() {
    use dclutch_liability_basis_v2_kernel::SPLINE_AGREEMENT_CASES_V2;

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        let mut buffer = [0_u8; 4];
        buffer.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_le_bytes(buffer)
    }
    fn i64_at(bytes: &[u8], offset: usize) -> i64 {
        let mut buffer = [0_u8; 8];
        buffer.copy_from_slice(&bytes[offset..offset + 8]);
        i64::from_le_bytes(buffer)
    }

    let mut exercised = 0_usize;
    let mut skipped_degree_one = 0_usize;
    for (index, case) in SPLINE_AGREEMENT_CASES_V2.iter().enumerate() {
        let bytes = &case.request;
        let degree = bytes[32];
        if degree < 2 {
            skipped_degree_one += 1;
            continue;
        }
        let scale = u32_at(bytes, 12);
        let knot_denominator = u32_at(bytes, 16);
        let coordinate_denominator = u32_at(bytes, 20);
        let coordinate_numerator = i64_at(bytes, 24);
        let knot_count = usize::from(bytes[33]);
        let knots: Vec<i128> = (0..knot_count)
            .map(|slot| i128::from(i64_at(bytes, KNOTS_OFFSET + slot * 8)))
            .collect();
        let width = u32::try_from(case.width).expect("width fits");

        let weights = evaluate_spline_weights_v3(
            knots.as_slice(),
            u64::from(knot_denominator),
            i128::from(coordinate_numerator),
            u64::from(coordinate_denominator),
            degree,
            width,
        )
        .unwrap_or_else(|error| panic!("port refused Lean case {index}: {error:?}"));

        let mut output = vec![0_u64; case.width];
        apportion_cumulative_v3(&weights, u64::from(scale), &mut output)
            .unwrap_or_else(|error| panic!("port refused Lean case {index}: {error:?}"));

        assert_eq!(
            output.as_slice(),
            &case.expected[..case.width],
            "Lean-emitted agreement case {index} (degree {degree})"
        );
        exercised += 1;
    }

    println!(
        "Lean corpus: {exercised} degree-2/3 cases reproduced exactly, \
         {skipped_degree_one} degree-1 cases outside this family"
    );
    assert_eq!(
        exercised + skipped_degree_one,
        SPLINE_AGREEMENT_CASES_V2.len(),
        "every emitted case must be either exercised or explicitly skipped"
    );
    // **Pinned exactly, not floored loosely.** This assertion used to read
    // `exercised >= 10`, which let nine currently-exercised cases turn into
    // skips -- or let the emitter reclassify them -- without going red. The
    // corpus is 28 cases: 9 at degree 1 (the graded family's, reached through
    // `BasisShapeV3` and executed in full by the kernel's own suite), 2 at
    // degree 2 and 17 at degree 3. Those numbers are facts about an emitted,
    // byte-guarded artifact, so pinning them costs nothing and catches a
    // silent shrink. If the emitter grows the corpus, this is the line that
    // says so.
    assert_eq!(
        (exercised, skipped_degree_one),
        (19, 9),
        "the Lean corpus split moved; a case was added, removed, or reclassified"
    );
}

/// The refusals are the port's, not the kernel's: a degree outside the
/// family's closed interval is refused before any evaluation happens.
#[test]
fn a_degree_outside_the_family_refuses() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    for degree in [0_u8, 1, 4, 255] {
        assert!(
            evaluate_spline_weights_v3(knots.as_slice(), 1, 1, 1, degree, 5).is_err(),
            "degree {degree} must refuse"
        );
    }
}

/// A knot vector that does not derive the declared width refuses, which is the
/// only binding between the degree and the knot vector.
#[test]
fn a_width_the_knot_vector_does_not_derive_refuses() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    assert!(evaluate_spline_weights_v3(knots.as_slice(), 1, 1, 1, 2, 4).is_err());
    assert!(evaluate_spline_weights_v3(knots.as_slice(), 1, 1, 1, 2, 6).is_err());
}

/// A knot vector with no non-degenerate span at all refuses rather than
/// dividing by zero.
#[test]
fn a_wholly_degenerate_knot_vector_refuses() {
    let knots: Vec<i128> = vec![5, 5, 5, 5, 5, 5, 5, 5];
    assert!(evaluate_spline_weights_v3(knots.as_slice(), 1, 5, 1, 2, 5).is_err());
}

/// **The strand the pre-scaling saturation closes, red-proofed.**
///
/// The coordinate used to be carried onto the common denominator with a
/// `checked_mul`, so a coordinate numerator large enough to overflow `i128`
/// when multiplied by the knot denominator refused with `ArithmeticOverflow` —
/// at *settlement*, on a Market that had already taken deposits. The multiply
/// is now saturating, and saturation is exact here because the clamp on the
/// next line discards the magnitude anyway.
///
/// The assertion is not merely "it no longer errors": it is that an enormous
/// coordinate lands on **exactly** the same weights as a coordinate sitting on
/// the top knot, which is what "clamped to the domain" has to mean.
#[test]
fn an_enormous_coordinate_saturates_onto_the_domain_instead_of_trapping() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    let denominator = 1_000_000_u64;

    // Large enough that `coordinate_numerator * knot_denominator` leaves i128.
    let enormous = evaluate_spline_weights_v3(knots.as_slice(), denominator, i128::MAX, 1, 2, 5)
        .expect("an out-of-domain coordinate must clamp, not trap");
    let at_top = evaluate_spline_weights_v3(knots.as_slice(), denominator, 3, 1, 2, 5)
        .expect("the top of the domain evaluates");
    assert_eq!(
        enormous, at_top,
        "a coordinate above the domain must evaluate exactly as the top knot does"
    );

    // And symmetrically below.
    let tiny = evaluate_spline_weights_v3(knots.as_slice(), denominator, i128::MIN, 1, 2, 5)
        .expect("an under-domain coordinate must clamp, not trap");
    let at_bottom = evaluate_spline_weights_v3(knots.as_slice(), denominator, 0, 1, 2, 5)
        .expect("the bottom of the domain evaluates");
    assert_eq!(
        tiny, at_bottom,
        "a coordinate below the domain must evaluate exactly as the first knot does"
    );
}

/// The residue, named rather than left to arrive as a generic overflow: a
/// coordinate denominator above the published ceiling refuses by its own code.
/// This is the boundary a `SignedU256` accumulation would retire, and the test
/// exists so that widening is visibly a *change of this constant* rather than a
/// silent relaxation.
#[test]
fn a_coordinate_denominator_above_the_ceiling_refuses_by_name() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    assert_eq!(
        evaluate_spline_weights_v3(
            knots.as_slice(),
            1,
            1,
            SPLINE_COORDINATE_DENOMINATOR_CEILING_V3 + 1,
            2,
            5
        ),
        Err(Error::SplineCoordinateOutOfEnvelope)
    );
    assert!(
        evaluate_spline_weights_v3(
            knots.as_slice(),
            1,
            1,
            SPLINE_COORDINATE_DENOMINATOR_CEILING_V3,
            2,
            5
        )
        .is_ok(),
        "the ceiling itself is admitted; it is a closed bound"
    );
}

/// **The envelope's promise, stated as a property rather than an example.**
/// Every basis the envelope admits evaluates without overflow at every
/// coordinate denominator up to the ceiling — that is the whole claim, and it
/// is what makes a founding-time check a substitute for a settlement-time one.
#[test]
fn what_the_envelope_admits_evaluates_everywhere_it_promised() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    let scale = 1_000_000_u64;
    spline_arithmetic_envelope_v3(knots.as_slice(), 2, 5, scale).expect("this basis is admissible");

    for denominator in [
        1_u64,
        2,
        1_000,
        1_000_000,
        SPLINE_COORDINATE_DENOMINATOR_CEILING_V3,
    ] {
        for numerator in [i128::MIN, -7, 0, 1, 2, 3, i128::MAX] {
            let weights =
                evaluate_spline_weights_v3(knots.as_slice(), 1, numerator, denominator, 2, 5)
                    .unwrap_or_else(|error| {
                        panic!("admitted basis refused at {numerator}/{denominator}: {error:?}")
                    });
            let mut output = vec![0_u64; 5];
            apportion_cumulative_v3(&weights, scale, &mut output).unwrap_or_else(|error| {
                panic!("admitted basis failed to apportion at {numerator}/{denominator}: {error:?}")
            });
            assert_eq!(
                output.iter().sum::<u64>(),
                scale,
                "apportionment must be exact at {numerator}/{denominator}"
            );
        }
    }
}
