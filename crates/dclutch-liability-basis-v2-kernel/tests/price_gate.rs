//! Differential and hostile tests for the Lean-owned degree-`>= 2` price gate.
//!
//! Every expectation here is either Lean-emitted
//! (`PRICE_GATE_AGREEMENT_CASES_V1`, `PRICE_GATE_REFUSAL_CASES_V1`) or derived
//! from generation two's own adversarial pair on `dragons-clutch`
//! (`crates/clutch-price-measure/tests/adversarial.rs`). Nothing is read back
//! out of the kernel and asserted against itself.

use dclutch_liability_basis_v2_kernel::{
    Error, PRICE_GATE_AGREEMENT_CASES_V1, PRICE_GATE_ATOM_COUNT_OFFSET_V1,
    PRICE_GATE_DEGREE_OFFSET_V1, PRICE_GATE_DENOMINATOR_BYTES_V1,
    PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_EXEMPT_DEGREE_V1, PRICE_GATE_MAGIC_OFFSET_V1,
    PRICE_GATE_MAGIC_V1, PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_MAX_WIDTH_V1,
    PRICE_GATE_NUMERATOR_BYTES_V1, PRICE_GATE_NUMERATORS_OFFSET_V1, PRICE_GATE_PRICE_BYTES_V1,
    PRICE_GATE_PRICES_OFFSET_V1, PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1,
    PRICE_GATE_REFUSAL_CASES_V1, PRICE_GATE_REQUEST_BYTES_V1, PRICE_GATE_REQUIRED_TAG_V1,
    PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1, PRICE_GATE_VERSION_OFFSET_V1,
    PRICE_GATE_WEIGHT_BYTES_V1, PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1,
    SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2, SPLINE_COORDINATE_NUMERATOR_OFFSET_V2,
    SPLINE_DEGREE_OFFSET_V2, SPLINE_KNOT_BYTES_V2, SPLINE_KNOT_COUNT_OFFSET_V2,
    SPLINE_KNOT_DENOMINATOR_OFFSET_V2, SPLINE_KNOTS_OFFSET_V2, SPLINE_MAGIC_OFFSET_V2,
    SPLINE_MAGIC_V2, SPLINE_PROFILE_OFFSET_V2, SPLINE_PROFILE_V2, SPLINE_REQUEST_BYTES_V2,
    SPLINE_SCALE_OFFSET_V2, SPLINE_SCHEMA_VERSION_V2, SPLINE_VERSION_OFFSET_V2,
    admit_and_evaluate_spline_v2, decode_and_evaluate_spline_v2, decode_price_gate_v1,
    decode_spline_request_v2, verify_price_gate_v1,
};

/// Every guarded refusal `PhysicalAbi.decodeChecks` names, in its own order.
///
/// Two reachable tags are deliberately absent. Tag 31 is the admission
/// conjunct, which no record can carry because it fires when there is no
/// record at all. Tag 11 is the spline evaluator's `ArithmeticOverflow`, which
/// has no Lean counterpart at all — Lean is unbounded, this kernel evaluates in
/// `u128`, and it fails closed — so no Lean-emitted case can reach it.
const GUARDED_TAGS: [u8; 20] = [
    0, 1, 2, 3, 4, 5, 20, 15, 21, 22, 23, 6, 24, 25, 26, 27, 28, 29, 19, 30,
];

/// Generation one's pinned counterexample basis: degree two, five claims,
/// breakpoints `[0,1,2,3]`, written here with clamped end multiplicity.
const GEN1_KNOTS: [i64; 8] = [0, 0, 0, 1, 2, 3, 3, 3];

/// Its payout scale.
const GEN1_SCALE: u32 = 12;

/// The price generation one's moment cone wrongly admitted:
/// `(1/3, 2/3, 0, 0, 0)` on the scale above.
/// `dragons-clutch crates/clutch-price-measure/tests/adversarial.rs:262`.
const GEN1_PRICE: [u64; 5] = [4, 8, 0, 0, 0];

/// The arbitrage portfolio: the B-spline coefficients of `(3x-1)^2` over the
/// knot vector above, so its payoff is a square and cannot be negative.
const GEN1_ARBITRAGE: [i64; 5] = [1, -2, 10, 40, 64];

/// The denominators `Examples.grid` sweeps, over the domain `[0, 3]`.
const GRID_DENOMINATORS: [u32; 6] = [1, 2, 3, 4, 6, 12];

#[test]
fn handwritten_gate_exactly_matches_the_lean_agreement_corpus() {
    for case in &PRICE_GATE_AGREEMENT_CASES_V1 {
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let certificate =
            verify_price_gate_v1(&basis, &case.certificate).expect("Lean-admitted certificate");
        assert_eq!(certificate.width(), case.width);
        assert_eq!(certificate.atom_count(), case.atom_count);
        assert_eq!(
            certificate.active_prices(),
            case.prices.get(..case.width).expect("width in capacity")
        );
        for padded in case.prices.get(case.width..).expect("width in capacity") {
            assert_eq!(*padded, 0);
        }
    }
    assert_eq!(PRICE_GATE_AGREEMENT_CASES_V1.len(), 22);
}

#[test]
fn every_certified_price_is_a_partition_of_the_collateral_scale() {
    // Lean `Certificate.price_sum`: the simplex condition is a *consequence* of
    // hull membership, not an extra premise, so a certified price can never
    // fail it. Asserted against the scale decoded from the basis bytes rather
    // than from the certificate, so a certificate that lied about its scale and
    // a kernel that believed it would both be caught here.
    for case in &PRICE_GATE_AGREEMENT_CASES_V1 {
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let certificate =
            verify_price_gate_v1(&basis, &case.certificate).expect("Lean-admitted certificate");
        let mut total = 0_u128;
        for price in certificate.active_prices() {
            total += u128::from(*price);
        }
        assert_eq!(total, u128::from(basis.scale()));
        assert_eq!(certificate.scale(), basis.scale());
        assert!(certificate.mass() > 0);
        for weight in certificate.active_weights() {
            assert!(*weight > 0);
        }
    }
}

#[test]
fn hostile_gate_decoder_exactly_matches_the_lean_refusal_corpus() {
    for case in &PRICE_GATE_REFUSAL_CASES_V1 {
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let offered = case
            .certificate
            .get(..case.certificate_len)
            .expect("refusal case inside its buffer");
        let error = verify_price_gate_v1(&basis, offered).expect_err("hostile certificate");
        assert_eq!(error.tag(), case.error_tag);
    }
    assert_eq!(PRICE_GATE_REFUSAL_CASES_V1.len(), 45);
}

#[test]
fn the_refusal_corpus_reaches_every_guarded_tag() {
    // A corpus edit that silently dropped a guard would still pass the refusal
    // test above; it fails here.
    for tag in GUARDED_TAGS {
        assert!(
            PRICE_GATE_REFUSAL_CASES_V1
                .iter()
                .any(|case| case.error_tag == tag),
            "no refusal case reaches tag {tag}"
        );
    }
    for case in &PRICE_GATE_REFUSAL_CASES_V1 {
        assert!(GUARDED_TAGS.contains(&case.error_tag));
    }
}

#[test]
fn the_generated_tags_agree_with_the_kernels_own_error_numbering() {
    // The corpus names tags as bare integers; the kernel names them as
    // variants. A renumbering on either side has to fail somewhere.
    assert_eq!(Error::ZeroMass.tag(), 20);
    assert_eq!(Error::WidthOutOfRange.tag(), 21);
    assert_eq!(Error::AtomCountOutOfRange.tag(), 22);
    assert_eq!(Error::NonCanonicalGatePadding.tag(), 23);
    assert_eq!(Error::ZeroAtomWeight.tag(), 24);
    assert_eq!(Error::NonCanonicalAtomOrder.tag(), 25);
    assert_eq!(Error::WeightMassMismatch.tag(), 26);
    assert_eq!(Error::NonPrimitiveWeightScale.tag(), 27);
    assert_eq!(Error::PriceNotPartition.tag(), 28);
    assert_eq!(Error::PriceGateBasisMismatch.tag(), 29);
    assert_eq!(Error::PriceReconstructionMismatch.tag(), 30);
    assert_eq!(Error::PriceGateRequired.tag(), PRICE_GATE_REQUIRED_TAG_V1);
    assert_eq!(PRICE_GATE_EXEMPT_DEGREE_V1, 1);
}

/// # Direction one of generation two's adversarial pair
///
/// `adversarial.rs:262`: generation one's gate **accepts** the price
/// `(4,8,0,0,0)/12`, and the portfolio `(1,-2,10,40,64)` costs exactly `-S`
/// there. Generation one was unsound. The sweep reproduces both halves against
/// this tree's evaluator and then shows the gate refusing the price.
#[test]
fn the_gen_one_false_acceptance_is_an_executable_arbitrage_this_gate_refuses() {
    // The price is simplex-admissible, so nothing weaker than a hull gate could
    // have refused it: it passes the partition guard and dies later.
    let mut total = 0_u64;
    for price in GEN1_PRICE {
        total += price;
    }
    assert_eq!(total, u64::from(GEN1_SCALE));

    // And it costs exactly minus one complete set.
    assert_eq!(
        portfolio_value(&GEN1_ARBITRAGE, &GEN1_PRICE),
        -i128::from(GEN1_SCALE)
    );

    let mut swept = 0_usize;
    for coordinate in grid() {
        let (numerator, denominator) = coordinate;
        let payouts = decode_and_evaluate_spline_v2(&gen1_basis_at(numerator, denominator))
            .expect("swept coordinate is admitted");

        // The payoff is a square in the exact rational basis; integer
        // apportionment never pushes it below zero on this grid.
        assert!(
            portfolio_value(&GEN1_ARBITRAGE, payouts.active()) >= 0,
            "arbitrage portfolio pays negatively at {numerator}/{denominator}"
        );

        // So the gate cannot certify the price at that atom.
        let basis = decode_spline_request_v2(&gen1_basis_at(0, 1)).expect("well-formed basis");
        let offered = gate_request(
            GEN1_SCALE,
            1,
            2,
            5,
            &GEN1_PRICE,
            &[(numerator, denominator, 1)],
        );
        assert_eq!(
            verify_price_gate_v1(&basis, &offered)
                .expect_err("the false acceptance has no single-atom certificate")
                .tag(),
            30
        );
        swept += 1;
    }
    assert_eq!(swept, 90);
}

/// # Direction two of the same pair
///
/// `adversarial.rs:281`: a live quantized point generation one's gate
/// **refuses** has an exact single-atom certificate. Generation one also
/// over-refused. The corpus carries the point and its mirror as agreement cases
/// zero and one; this pins the payout vectors generation two recorded.
#[test]
fn the_gen_one_over_refusal_is_a_point_this_gate_admits_by_a_single_atom() {
    // Generation two recorded `basis(2, [0,128,256,384], 10_000).evaluate(85) =
    // [1128, 6667, 2205, 0, 0]`, rounding by largest remainder. This tree
    // floors a running cumulative sum and returns the same vector.
    let live = PRICE_GATE_AGREEMENT_CASES_V1
        .first()
        .expect("the corpus leads with the live point");
    assert_eq!(
        live.prices.get(..5).expect("five claims"),
        &[1128, 6667, 2205, 0, 0]
    );
    assert_eq!(live.atom_count, 1);

    let mirror = PRICE_GATE_AGREEMENT_CASES_V1
        .get(1)
        .expect("and its reflection");
    assert_eq!(
        mirror.prices.get(..5).expect("five claims"),
        &[0, 0, 2204, 6667, 1129]
    );
    assert_eq!(mirror.atom_count, 1);

    for case in [live, mirror] {
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let certificate =
            verify_price_gate_v1(&basis, &case.certificate).expect("single-atom certificate");
        assert_eq!(certificate.mass(), 1);
        assert_eq!(certificate.active_weights(), &[1]);
    }
}

#[test]
fn every_attainable_payout_vector_on_the_swept_grid_certifies_itself() {
    // Lean `singleAtom_valid`, executable: a market trading at exactly what
    // some coordinate pays is never refused. This is the completeness side of
    // the same grid the refutation above sweeps, so the two directions are
    // measured against one another rather than each against its own basis.
    let basis = decode_spline_request_v2(&gen1_basis_at(0, 1)).expect("well-formed basis");
    for (numerator, denominator) in grid() {
        let payouts = decode_and_evaluate_spline_v2(&gen1_basis_at(numerator, denominator))
            .expect("swept coordinate is admitted");
        let offered = gate_request(
            GEN1_SCALE,
            1,
            2,
            5,
            payouts.active(),
            &[(numerator, denominator, 1)],
        );
        let certificate = verify_price_gate_v1(&basis, &offered)
            .expect("an attainable payout vector is its own certificate");
        assert_eq!(certificate.active_prices(), payouts.active());
    }
}

#[test]
fn one_price_can_have_two_accepted_supports() {
    // Generation two pinned the same fact about its own certificate at
    // `adversarial.rs:321`. It is worth an executable test because the
    // primitivity guard is easy to over-read: it canonicalizes the *scale* of
    // one support so that one support has one encoding, and it says nothing
    // about the support being unique. Lean `mixture_valid` is the positive
    // form — the checker admits every honest mixture, not a distinguished
    // normal form — so a certificate digest is not a price identity.
    let basis = decode_spline_request_v2(&gen1_basis_at(0, 1)).expect("well-formed basis");
    let price = [0_u64, 7, 5, 0, 0];

    // 5/6 pays the price outright.
    let single = gate_request(GEN1_SCALE, 1, 2, 5, &price, &[(5, 6, 1)]);
    // And 3/4 mixed evenly with 1 reaches the same point from two others.
    let mixed = gate_request(GEN1_SCALE, 2, 2, 5, &price, &[(3, 4, 1), (1, 1, 1)]);

    let first = verify_price_gate_v1(&basis, &single).expect("the single-atom support");
    let second = verify_price_gate_v1(&basis, &mixed).expect("the two-atom support");
    assert_eq!(first.active_prices(), second.active_prices());
    assert_eq!(first.active_prices(), &price);
    assert_ne!(first.mass(), second.mass());
    assert_ne!(first.atom_count(), second.atom_count());
    assert_ne!(single, mixed);
}

#[test]
fn a_mixture_is_admitted_only_at_its_primitive_scale() {
    // `Certificate.Valid` is invariant under scaling the weights and the mass
    // together; the physical boundary refuses the scaled forms so that one
    // support has one encoding. This is canonicalization, not a uniqueness
    // claim about the support.
    let basis = decode_spline_request_v2(&gen1_basis_at(0, 1)).expect("well-formed basis");
    // Coordinates 1 and 2 pay [0,6,6,0,0] and [0,0,6,6,0]; mixed evenly that is
    // the price [0,3,6,3,0] at mass two.
    let primitive = gate_request(
        GEN1_SCALE,
        2,
        2,
        5,
        &[0, 3, 6, 3, 0],
        &[(1, 1, 1), (2, 1, 1)],
    );
    verify_price_gate_v1(&basis, &primitive).expect("the primitive representative is admitted");

    let scaled = gate_request(
        GEN1_SCALE,
        6,
        2,
        5,
        &[0, 3, 6, 3, 0],
        &[(1, 1, 3), (2, 1, 3)],
    );
    assert_eq!(
        verify_price_gate_v1(&basis, &scaled)
            .expect_err("the same mixture at three times the mass")
            .tag(),
        27
    );
}

/// # The admission conjunct
///
/// Lean `PhysicalAbi.admitEvaluation`. Degree `>= 2` is evaluated for sale only
/// alongside a certificate this gate accepts against that same request.
#[test]
fn nothing_at_degree_two_or_above_is_evaluated_without_a_certificate() {
    for case in &PRICE_GATE_AGREEMENT_CASES_V1 {
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let admitted = admit_and_evaluate_spline_v2(&case.basis, Some(&case.certificate))
            .expect("certificate admits the evaluation");
        assert!(admitted.certificate.is_some());
        // The evaluation is the production evaluator's, unchanged by the gate.
        let bare = decode_and_evaluate_spline_v2(&case.basis).expect("admitted coordinate");
        assert_eq!(admitted.weights, bare);

        let bare_attempt = admit_and_evaluate_spline_v2(&case.basis, None);
        if basis.degree() > PRICE_GATE_EXEMPT_DEGREE_V1 {
            assert_eq!(
                bare_attempt
                    .expect_err("a graded basis with no certificate")
                    .tag(),
                PRICE_GATE_REQUIRED_TAG_V1
            );
        } else {
            // Degree one is exempt by LB-SPLINE's proof: every claim attains a
            // whole complete set at its own knot, so
            // `no_cap_of_attained_scale` leaves the capped-claim refusal with
            // no instance and the simplex condition is still the whole
            // no-arbitrage condition.
            let exempt = bare_attempt.expect("degree one needs no certificate");
            assert!(exempt.certificate.is_none());
            assert_eq!(exempt.weights, bare);
        }
    }
}

#[test]
fn a_certificate_offered_at_an_exempt_degree_is_still_checked() {
    // An input that is present is never silently ignored, whatever the degree.
    let degree_one = PRICE_GATE_AGREEMENT_CASES_V1
        .iter()
        .find(|case| {
            decode_spline_request_v2(&case.basis)
                .expect("Lean-admitted basis")
                .degree()
                == 1
        })
        .expect("the corpus carries a degree-one case");
    let mut broken = degree_one.certificate;
    let slot = broken
        .get_mut(PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8)
        .expect("mass field inside the record");
    slot.copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        admit_and_evaluate_spline_v2(&degree_one.basis, Some(&broken))
            .expect_err("a broken certificate at an exempt degree")
            .tag(),
        20
    );
}

#[test]
fn a_certificate_never_binds_to_a_basis_it_does_not_name() {
    // The gate authenticates nothing: it compares the certificate's own scale,
    // degree and width against an already authenticated request. Crossing the
    // corpus's bases must refuse rather than certify.
    let mut bound = 0_usize;
    let mut unbound = 0_usize;
    for case in &PRICE_GATE_AGREEMENT_CASES_V1 {
        for other in &PRICE_GATE_AGREEMENT_CASES_V1 {
            if other.basis == case.basis {
                continue;
            }
            let basis = decode_spline_request_v2(&other.basis).expect("Lean-admitted basis");
            let certificate = decode_price_gate_v1(&case.certificate).expect("canonical record");
            let tag = verify_price_gate_v1(&basis, &case.certificate)
                .expect_err("a certificate for another basis")
                .tag();
            if u64::from(certificate.scale()) == u64::from(basis.scale())
                && certificate.degree() == basis.degree()
                && certificate.width() == basis.width()
            {
                // A different knot vector at the same scale, degree and width.
                // The binding cannot separate these — that is exactly the case
                // the hull equation has to carry, either because a named
                // coordinate is outside the other basis's domain (19) or
                // because the recomputed mixture disagrees (30).
                assert!(tag == 19 || tag == 30, "unbound crossing reported {tag}");
                unbound += 1;
            } else {
                assert_eq!(tag, 29);
                bound += 1;
            }
        }
    }
    // Both arms are exercised: the corpus carries bases that the binding
    // separates and bases it cannot.
    assert!(bound > 0);
    assert!(unbound > 0);
}

/// Exact integer value of one signed portfolio against one payout vector.
fn portfolio_value(portfolio: &[i64], payouts: &[u64]) -> i128 {
    let mut total = 0_i128;
    for (holding, payout) in portfolio.iter().zip(payouts.iter()) {
        total += i128::from(*holding) * i128::from(*payout);
    }
    total
}

/// `Examples.grid`: every coordinate `n/d` in `[0, 3]` for each swept
/// denominator. Ninety of them.
fn grid() -> impl Iterator<Item = (i64, u32)> {
    GRID_DENOMINATORS
        .into_iter()
        .flat_map(|denominator| (0..=3 * denominator).map(move |n| (i64::from(n), denominator)))
}

/// Encode generation one's counterexample basis at one coordinate.
fn gen1_basis_at(numerator: i64, denominator: u32) -> [u8; SPLINE_REQUEST_BYTES_V2] {
    let mut request = [0_u8; SPLINE_REQUEST_BYTES_V2];
    put(&mut request, SPLINE_MAGIC_OFFSET_V2, &SPLINE_MAGIC_V2);
    put(
        &mut request,
        SPLINE_VERSION_OFFSET_V2,
        &SPLINE_SCHEMA_VERSION_V2.to_le_bytes(),
    );
    put(
        &mut request,
        SPLINE_PROFILE_OFFSET_V2,
        &SPLINE_PROFILE_V2.to_le_bytes(),
    );
    put(
        &mut request,
        SPLINE_SCALE_OFFSET_V2,
        &GEN1_SCALE.to_le_bytes(),
    );
    put(
        &mut request,
        SPLINE_KNOT_DENOMINATOR_OFFSET_V2,
        &1_u32.to_le_bytes(),
    );
    put(
        &mut request,
        SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2,
        &denominator.to_le_bytes(),
    );
    put(
        &mut request,
        SPLINE_COORDINATE_NUMERATOR_OFFSET_V2,
        &numerator.to_le_bytes(),
    );
    put(&mut request, SPLINE_DEGREE_OFFSET_V2, &[2]);
    put(
        &mut request,
        SPLINE_KNOT_COUNT_OFFSET_V2,
        &[u8::try_from(GEN1_KNOTS.len()).expect("knot count inside the record")],
    );
    for (slot, knot) in GEN1_KNOTS.iter().enumerate() {
        put(
            &mut request,
            SPLINE_KNOTS_OFFSET_V2 + slot * SPLINE_KNOT_BYTES_V2,
            &knot.to_le_bytes(),
        );
    }
    request
}

/// Encode one canonical price-gate certificate. Named offsets only, so a layout
/// change in the Lean-emitted constants moves this with it. `atoms` are
/// `(numerator, denominator, weight)` in the record's own increasing order.
fn gate_request(
    scale: u32,
    mass: u64,
    degree: u8,
    width: u8,
    prices: &[u64],
    atoms: &[(i64, u32, u64)],
) -> [u8; PRICE_GATE_REQUEST_BYTES_V1] {
    let mut request = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
    put(
        &mut request,
        PRICE_GATE_MAGIC_OFFSET_V1,
        &PRICE_GATE_MAGIC_V1,
    );
    put(
        &mut request,
        PRICE_GATE_VERSION_OFFSET_V1,
        &PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes(),
    );
    put(
        &mut request,
        PRICE_GATE_PROFILE_OFFSET_V1,
        &PRICE_GATE_PROFILE_V1.to_le_bytes(),
    );
    put(
        &mut request,
        PRICE_GATE_SCALE_OFFSET_V1,
        &scale.to_le_bytes(),
    );
    put(&mut request, PRICE_GATE_MASS_OFFSET_V1, &mass.to_le_bytes());
    put(&mut request, PRICE_GATE_DEGREE_OFFSET_V1, &[degree]);
    put(&mut request, PRICE_GATE_WIDTH_OFFSET_V1, &[width]);
    put(
        &mut request,
        PRICE_GATE_ATOM_COUNT_OFFSET_V1,
        &[u8::try_from(atoms.len()).expect("atom count inside the record")],
    );
    for (slot, price) in prices.iter().enumerate() {
        put(
            &mut request,
            PRICE_GATE_PRICES_OFFSET_V1 + slot * PRICE_GATE_PRICE_BYTES_V1,
            &price.to_le_bytes(),
        );
    }
    for (slot, (numerator, denominator, weight)) in atoms.iter().enumerate() {
        put(
            &mut request,
            PRICE_GATE_WEIGHTS_OFFSET_V1 + slot * PRICE_GATE_WEIGHT_BYTES_V1,
            &weight.to_le_bytes(),
        );
        put(
            &mut request,
            PRICE_GATE_NUMERATORS_OFFSET_V1 + slot * PRICE_GATE_NUMERATOR_BYTES_V1,
            &numerator.to_le_bytes(),
        );
        put(
            &mut request,
            PRICE_GATE_DENOMINATORS_OFFSET_V1 + slot * PRICE_GATE_DENOMINATOR_BYTES_V1,
            &denominator.to_le_bytes(),
        );
    }
    request
}

fn put(request: &mut [u8], offset: usize, value: &[u8]) {
    let end = offset + value.len();
    request
        .get_mut(offset..end)
        .expect("field inside the record")
        .copy_from_slice(value);
}

#[test]
fn the_physical_capacity_bounds_hold_across_the_corpus() {
    for case in &PRICE_GATE_AGREEMENT_CASES_V1 {
        assert!(case.width >= 2 && case.width <= PRICE_GATE_MAX_WIDTH_V1);
        // Affine Caratheodory: a hull point's support is bounded by the
        // dimension of the hyperplane `sum = Q` plus one, which is the width.
        assert!(case.atom_count >= 1 && case.atom_count <= case.width);
        let basis = decode_spline_request_v2(&case.basis).expect("Lean-admitted basis");
        let certificate = decode_price_gate_v1(&case.certificate).expect("canonical record");
        assert_eq!(certificate.width(), basis.width());
        assert_eq!(certificate.degree(), basis.degree());
    }
}
