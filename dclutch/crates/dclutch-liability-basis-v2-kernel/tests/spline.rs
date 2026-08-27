//! Differential and hostile tests for the Lean-owned B-spline profile.
//!
//! Every expectation here is either Lean-emitted (`SPLINE_AGREEMENT_CASES_V2`,
//! `SPLINE_REFUSAL_CASES_V2`) or hand-derived from exact Bernstein and B-spline
//! values. Nothing is read back out of the kernel and asserted against itself.

use dclutch_liability_basis_v2_kernel::{
    SPLINE_AGREEMENT_CASES_V2, SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2,
    SPLINE_COORDINATE_NUMERATOR_OFFSET_V2, SPLINE_DEGREE_OFFSET_V2, SPLINE_KNOT_BYTES_V2,
    SPLINE_KNOT_COUNT_OFFSET_V2, SPLINE_KNOT_DENOMINATOR_OFFSET_V2, SPLINE_KNOTS_OFFSET_V2,
    SPLINE_MAGIC_OFFSET_V2, SPLINE_MAGIC_V2, SPLINE_MAX_WIDTH_V2, SPLINE_PROFILE_OFFSET_V2,
    SPLINE_PROFILE_V2, SPLINE_REFUSAL_CASES_V2, SPLINE_REQUEST_BYTES_V2, SPLINE_SCALE_OFFSET_V2,
    SPLINE_SCHEMA_VERSION_V2, SPLINE_VERSION_OFFSET_V2, SplineBasisV2,
    decode_and_evaluate_spline_v2, decode_spline_request_v2, evaluate_spline_basis_v2,
};

/// Every refusal tag the spline decoder and evaluator can name.
const GUARDED_TAGS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 15, 16, 17, 18, 19];

/// The clamped cubic knot vector: a single span carrying the cubic Bernstein
/// basis exactly.
const CLAMPED_CUBIC: [i64; 8] = [0, 0, 0, 0, 1, 1, 1, 1];

/// The uniform clamped cubic knot vector, five interior spans wide.
const UNIFORM_CUBIC: [i64; 12] = [0, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5];

#[test]
fn handwritten_spline_exactly_matches_the_lean_agreement_corpus() {
    for case in SPLINE_AGREEMENT_CASES_V2 {
        let weights =
            decode_and_evaluate_spline_v2(&case.request).expect("Lean-admitted spline request");
        assert_eq!(weights.width, case.width);
        assert_eq!(weights.active(), head(&case.expected, case.width));
        for padded in tail(&case.expected, case.width) {
            assert_eq!(*padded, 0);
        }
        for padded in tail(&weights.payouts, case.width) {
            assert_eq!(*padded, 0);
        }
    }
    assert_eq!(SPLINE_AGREEMENT_CASES_V2.len(), 28);
}

#[test]
fn every_agreement_case_apportions_exactly_the_named_scale() {
    // Asserted against the scale decoded from the request bytes rather than
    // against `expected`, so this would still catch a corpus that agreed with
    // a kernel losing atoms to a remainder.
    for case in SPLINE_AGREEMENT_CASES_V2 {
        let request = decode_spline_request_v2(&case.request).expect("Lean-admitted request");
        let weights =
            decode_and_evaluate_spline_v2(&case.request).expect("Lean-admitted evaluation");
        let mut total = 0_u128;
        for payout in weights.active() {
            assert!(u64::from(request.scale()) >= *payout);
            total += u128::from(*payout);
        }
        assert_eq!(total, u128::from(request.scale()));
    }
}

#[test]
fn hostile_spline_decoder_exactly_matches_the_lean_refusal_corpus() {
    for case in SPLINE_REFUSAL_CASES_V2 {
        let offered = case
            .request
            .get(..case.request_len)
            .expect("refusal case inside its buffer");
        let error = decode_and_evaluate_spline_v2(offered).expect_err("hostile spline request");
        assert_eq!(error.tag(), case.error_tag);
    }
    assert_eq!(SPLINE_REFUSAL_CASES_V2.len(), 32);
}

#[test]
fn the_refusal_corpus_reaches_every_guarded_tag() {
    // A corpus edit that silently dropped a guard would still pass the refusal
    // test above; it fails here.
    for tag in GUARDED_TAGS {
        assert!(
            SPLINE_REFUSAL_CASES_V2
                .iter()
                .any(|case| case.error_tag == tag),
            "no refusal case reaches tag {tag}"
        );
    }
    for case in SPLINE_REFUSAL_CASES_V2 {
        assert!(GUARDED_TAGS.contains(&case.error_tag));
    }
}

#[test]
fn the_clamped_cubic_triangle_is_the_exact_bernstein_basis() {
    // Cubic Bernstein at t = 1/2 is [1/8, 3/8, 3/8, 1/8]; over the triangle's
    // own denominator 64 that is [8, 24, 24, 8].
    let request = decode_spline_request_v2(&spline_request(1_000, 1, 2, 1, 3, &CLAMPED_CUBIC))
        .expect("well-formed clamped cubic");
    let basis = evaluate_spline_basis_v2(&request).expect("admitted coordinate");
    assert_eq!(basis.denominator, 64);
    assert_eq!(basis.local_len, 4);
    assert_eq!(basis.offset, 0);
    assert_eq!(basis.span, 3);
    assert_eq!(local(&basis), &[8_u128, 24, 24, 8]);
}

#[test]
fn the_uniform_cubic_triangle_is_the_exact_b_spline_basis() {
    // The uniform cubic at t = 5/2 is [1/48, 23/48, 23/48, 1/48]; over the
    // triangle's own denominator 6912 that is [144, 3312, 3312, 144].
    let request = decode_spline_request_v2(&spline_request(1_000, 1, 2, 5, 3, &UNIFORM_CUBIC))
        .expect("well-formed uniform cubic");
    let basis = evaluate_spline_basis_v2(&request).expect("admitted coordinate");
    assert_eq!(basis.denominator, 6_912);
    assert_eq!(basis.local_len, 4);
    assert_eq!(basis.span, 5);
    assert_eq!(basis.offset, 2);
    assert_eq!(local(&basis), &[144_u128, 3312, 3312, 144]);
}

#[test]
fn cubic_sweeps_stay_an_exact_partition_with_local_support() {
    // The cheapest thing that would catch an off-by-one in the span locator: a
    // located span one claim off still sums to the scale, but pays a claim
    // outside the four the coordinate is really supported on.
    let mut admitted = 0_usize;
    for knots in [&CLAMPED_CUBIC[..], &UNIFORM_CUBIC[..]] {
        for scale in [1_u32, 7, 100, 1_000, 999_983] {
            for denominator in [1_u32, 2, 3, 7] {
                for numerator in -6_i64..=30 {
                    let bytes = spline_request(scale, 1, denominator, numerator, 3, knots);
                    let weights =
                        decode_and_evaluate_spline_v2(&bytes).expect("admitted cubic coordinate");
                    assert_eq!(weights.width, knots.len() - 4);
                    let mut total = 0_u128;
                    let mut supported = 0_usize;
                    for payout in weights.active() {
                        assert!(*payout <= u64::from(scale));
                        if *payout != 0 {
                            supported += 1;
                        }
                        total += u128::from(*payout);
                    }
                    assert_eq!(total, u128::from(scale));
                    assert!(supported <= 4, "degree-three support is four claims");
                    admitted += 1;
                }
            }
        }
    }
    assert_eq!(admitted, 2 * 5 * 4 * 37);
}

#[test]
fn a_degenerate_knot_vector_refuses_rather_than_paying_a_nonsense_basis() {
    // Every span collapsed: no candidate span exists at all.
    let collapsed = spline_request(100, 1, 1, 0, 1, &[0, 0, 0, 0]);
    assert_eq!(
        decode_and_evaluate_spline_v2(&collapsed)
            .expect_err("no non-degenerate span")
            .tag(),
        19
    );
    // Interior multiplicity is admitted, not refused: the collapsed span at
    // the doubled knot is skipped and the next real span carries the
    // coordinate, which is how a corner lives inside a smooth basis at all.
    let multiple = spline_request(100, 1, 1, 1, 1, &[0, 1, 1, 2, 3]);
    let weights = decode_and_evaluate_spline_v2(&multiple).expect("interior multiplicity admitted");
    assert_eq!(weights.active(), &[0, 100, 0]);
}

/// Encode one canonical B-spline request. Named offsets only, so a layout
/// change in the Lean-emitted constants moves this with it.
fn spline_request(
    scale: u32,
    knot_denominator: u32,
    coordinate_denominator: u32,
    coordinate_numerator: i64,
    degree: u8,
    knots: &[i64],
) -> [u8; SPLINE_REQUEST_BYTES_V2] {
    let mut request = [0_u8; SPLINE_REQUEST_BYTES_V2];
    write_bytes(&mut request, SPLINE_MAGIC_OFFSET_V2, &SPLINE_MAGIC_V2);
    write_bytes(
        &mut request,
        SPLINE_VERSION_OFFSET_V2,
        &SPLINE_SCHEMA_VERSION_V2.to_le_bytes(),
    );
    write_bytes(
        &mut request,
        SPLINE_PROFILE_OFFSET_V2,
        &SPLINE_PROFILE_V2.to_le_bytes(),
    );
    write_bytes(&mut request, SPLINE_SCALE_OFFSET_V2, &scale.to_le_bytes());
    write_bytes(
        &mut request,
        SPLINE_KNOT_DENOMINATOR_OFFSET_V2,
        &knot_denominator.to_le_bytes(),
    );
    write_bytes(
        &mut request,
        SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2,
        &coordinate_denominator.to_le_bytes(),
    );
    write_bytes(
        &mut request,
        SPLINE_COORDINATE_NUMERATOR_OFFSET_V2,
        &coordinate_numerator.to_le_bytes(),
    );
    write_bytes(&mut request, SPLINE_DEGREE_OFFSET_V2, &[degree]);
    write_bytes(
        &mut request,
        SPLINE_KNOT_COUNT_OFFSET_V2,
        &[u8::try_from(knots.len()).expect("knot count inside the record")],
    );
    for (slot, knot) in knots.iter().enumerate() {
        write_bytes(
            &mut request,
            SPLINE_KNOTS_OFFSET_V2 + slot * SPLINE_KNOT_BYTES_V2,
            &knot.to_le_bytes(),
        );
    }
    request
}

fn write_bytes(request: &mut [u8; SPLINE_REQUEST_BYTES_V2], offset: usize, value: &[u8]) {
    let end = offset + value.len();
    request
        .get_mut(offset..end)
        .expect("field inside the record")
        .copy_from_slice(value);
}

/// The valid prefix of one zero-padded corpus vector.
fn head(values: &[u64; SPLINE_MAX_WIDTH_V2], width: usize) -> &[u64] {
    values.get(..width).expect("width inside the capacity")
}

/// The zero padding beyond one corpus vector's runtime width.
fn tail(values: &[u64; SPLINE_MAX_WIDTH_V2], width: usize) -> &[u64] {
    values.get(width..).expect("width inside the capacity")
}

/// The valid prefix of one evaluated local de Boor triangle.
fn local(basis: &SplineBasisV2) -> &[u128] {
    basis
        .local
        .get(..basis.local_len)
        .expect("local support inside the capacity")
}

#[test]
fn the_physical_capacity_bounds_hold_across_the_corpus() {
    for case in SPLINE_AGREEMENT_CASES_V2 {
        assert!(case.width >= 2 && case.width <= SPLINE_MAX_WIDTH_V2);
        let request = decode_spline_request_v2(&case.request).expect("Lean-admitted request");
        assert_eq!(request.width(), case.width);
        assert_eq!(
            request.active_knots().len(),
            usize::from(request.knot_count())
        );
        assert!(request.degree() >= 1 && request.degree() <= 3);
    }
}
