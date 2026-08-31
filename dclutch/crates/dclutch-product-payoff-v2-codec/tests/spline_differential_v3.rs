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
//! 3. **Both candidate boundaries partition the scale exactly**, and the gap
//!    between them is *measured* rather than asserted away — see
//!    `the_two_candidate_boundaries_diverge_and_here_is_by_how_much`.

use dclutch_liability_basis_v2_kernel::spline as kernel;
use dclutch_product_payoff_v2_codec::spline_eval_v3::{
    SplineWeightsV3, apportion_cumulative_v3, apportion_floor_complement_v3,
    apportionment_divergence_v3, evaluate_spline_weights_v3,
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
        &knots,
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
            ported.denominator, reference.denominator,
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
                ported.numerator_at(claim),
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

/// Both candidate boundaries partition the scale exactly. This is the property
/// that makes either of them admissible at all.
#[test]
fn both_candidate_boundaries_partition_the_scale_exactly() {
    for case in CASES {
        let weights = port_weights(case);
        let scale = u64::from(case.scale);
        let mut cumulative = vec![0_u64; weights.width];
        let mut floored = vec![0_u64; weights.width];
        apportion_cumulative_v3(&weights, scale, &mut cumulative).expect("cumulative apportions");
        apportion_floor_complement_v3(&weights, scale, &mut floored)
            .expect("floor-complement apportions");
        assert_eq!(
            cumulative.iter().sum::<u64>(),
            scale,
            "cumulative sums to Q, {}",
            case.note
        );
        assert_eq!(
            floored.iter().sum::<u64>(),
            scale,
            "floor-complement sums to Q, {}",
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
            if weights.numerator_at(claim) == 0 {
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

/// **The measured divergence.** The two candidate boundaries are different
/// functions, and this records by how much rather than leaving it to a comment.
///
/// If this ever reports zero across every case, the rounding question this
/// module raises would be moot — so the test asserts that at least one case
/// genuinely diverges, which is what keeps the open question honest.
#[test]
fn the_two_candidate_boundaries_diverge_and_here_is_by_how_much() {
    let mut worst_overall = 0_u64;
    let mut diverging = 0_usize;
    for case in CASES {
        let weights = port_weights(case);
        let mut cumulative = vec![0_u64; weights.width];
        let mut floored = vec![0_u64; weights.width];
        let gap = apportionment_divergence_v3(
            &weights,
            u64::from(case.scale),
            &mut cumulative,
            &mut floored,
        )
        .expect("both boundaries apportion");
        if gap > 0 {
            diverging += 1;
        }
        worst_overall = worst_overall.max(gap);
        println!("{:<70} worst per-claim gap {gap}", case.note);
    }
    println!("cases diverging: {diverging} of {}", CASES.len());
    println!("worst per-claim gap across the corpus: {worst_overall} atoms");
    assert!(
        diverging > 0,
        "the two boundaries must actually differ somewhere, or the open rounding \
         question in spline_eval_v3 is not a real question"
    );
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
/// Degree-1 cases are skipped because the degree-2-to-3 family does not claim
/// them — degree 1 is the graded family's, reached through `BasisShapeV3`. The
/// count of cases actually exercised is asserted, so the skip cannot quietly
/// grow until the test covers nothing.
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
            &knots,
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
    assert!(
        exercised >= 10,
        "only {exercised} cases exercised; the corpus link has gone vacuous"
    );
}

/// The refusals are the port's, not the kernel's: a degree outside the
/// family's closed interval is refused before any evaluation happens.
#[test]
fn a_degree_outside_the_family_refuses() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    for degree in [0_u8, 1, 4, 255] {
        assert!(
            evaluate_spline_weights_v3(&knots, 1, 1, 1, degree, 5).is_err(),
            "degree {degree} must refuse"
        );
    }
}

/// A knot vector that does not derive the declared width refuses, which is the
/// only binding between the degree and the knot vector.
#[test]
fn a_width_the_knot_vector_does_not_derive_refuses() {
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    assert!(evaluate_spline_weights_v3(&knots, 1, 1, 1, 2, 4).is_err());
    assert!(evaluate_spline_weights_v3(&knots, 1, 1, 1, 2, 6).is_err());
}

/// A knot vector with no non-degenerate span at all refuses rather than
/// dividing by zero.
#[test]
fn a_wholly_degenerate_knot_vector_refuses() {
    let knots: Vec<i128> = vec![5, 5, 5, 5, 5, 5, 5, 5];
    assert!(evaluate_spline_weights_v3(&knots, 1, 5, 1, 2, 5).is_err());
}
