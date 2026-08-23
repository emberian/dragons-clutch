use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS};
use clutch_price_measure::{
    transfer_span_v2, verify_continuous_price_measure_v2, verify_quantized_price_measure_v2,
    AdapterBindingsV2, BasisSemanticsV2, BindingFieldV2, ContinuousPriceMeasureWitnessV2,
    CubicConstraintV2, ErrorV2, PayoutRoundingBoundaryV2, PriceRoundingBoundaryV2, PriceVectorV2,
    QuantizedAtomWitnessV2, QuantizedPriceMeasureAccumulatorV2, VerifiedPriceMeasureV2,
    MAX_COMMON_DENOMINATOR, MAX_MOMENTS, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    PRICE_MEASURE_WITNESS_VERSION_V2, TRANSFER_TABLE_VERSION_V1,
};

const FROZEN_CONTINUOUS: &str = include_str!("../fixtures/continuous_v2_vectors.txt");

fn bindings() -> AdapterBindingsV2 {
    AdapterBindingsV2 {
        candidate_feed: [1; 32],
        relation_domain_digest: [2; 32],
        basis_digest: [3; 32],
        candidate_price_digest: [4; 32],
        observed_body_digest: [5; 32],
    }
}

fn price_vector(degree: u8, active: &[u64], scale: u64) -> PriceVectorV2 {
    let mut prices = [0_u64; MAX_OUTCOMES];
    prices[..active.len()].copy_from_slice(active);
    PriceVectorV2 {
        basis_degree: degree,
        outcome_count: u8::try_from(active.len()).unwrap(),
        price_scale: scale,
        prices,
    }
}

fn continuous_witness(
    degree: u8,
    outcome_count: u8,
    denominator: u64,
    packed_moments: &[u64],
) -> ContinuousPriceMeasureWitnessV2 {
    let bindings = bindings();
    let spans = outcome_count - degree;
    let width = usize::from(degree) + 1;
    assert_eq!(packed_moments.len(), usize::from(spans) * width);
    let mut moments = [0_u64; MAX_MOMENTS];
    let mut span = 0_usize;
    while span < usize::from(spans) {
        let source = span * width;
        let target = span * 4;
        moments[target..target + width].copy_from_slice(&packed_moments[source..source + width]);
        span += 1;
    }
    ContinuousPriceMeasureWitnessV2 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V2,
        transfer_table_version: TRANSFER_TABLE_VERSION_V1,
        basis_semantics: BasisSemanticsV2::ContinuousOpenClampedUniformV1,
        price_rounding_boundary: PriceRoundingBoundaryV2::UpstreamExactSimplexV1,
        payout_rounding_boundary: PayoutRoundingBoundaryV2::ExactUnquantizedV1,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        basis_degree: degree,
        outcome_count,
        span_count: spans,
        common_denominator: denominator,
        moments,
    }
}

fn basis(degree: u8, knots_in: &[u128], denominator: u64, spacing_shift: u8) -> BasisSpec {
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..knots_in.len()].copy_from_slice(knots_in);
    BasisSpec {
        outcome_count: u8::try_from(knots_in.len() - 1 + usize::from(degree)).unwrap(),
        degree,
        knot_count: u8::try_from(knots_in.len()).unwrap(),
        uniform_log2_spacing: spacing_shift,
        denominator,
        domain_max: *knots_in.last().unwrap(),
        edge_policy: EdgePolicy::Clamp,
        knots,
    }
}

fn quantized_witness(
    atom_coordinates: &[u128],
    atom_masses: &[u64],
    denominator: u64,
) -> QuantizedAtomWitnessV2 {
    assert_eq!(atom_coordinates.len(), atom_masses.len());
    let bindings = bindings();
    let mut coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
    let mut masses = [0_u64; MAX_QUANTIZED_ATOMS];
    coordinates[..atom_coordinates.len()].copy_from_slice(atom_coordinates);
    masses[..atom_masses.len()].copy_from_slice(atom_masses);
    QuantizedAtomWitnessV2 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V2,
        basis_semantics: BasisSemanticsV2::QuantizedIntegerGridV1,
        price_rounding_boundary: PriceRoundingBoundaryV2::UpstreamExactSimplexV1,
        payout_rounding_boundary: PayoutRoundingBoundaryV2::LargestRemainderLowestIndexV1,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        atom_count: u8::try_from(atom_coordinates.len()).unwrap(),
        common_denominator: denominator,
        atom_coordinates: coordinates,
        atom_masses: masses,
    }
}

fn quantized_mixture_prices(
    basis: &BasisSpec,
    coordinates: &[u128],
    masses: &[u64],
) -> PriceVectorV2 {
    let denominator: u64 = masses.iter().copied().sum();
    let scale = basis.denominator.checked_mul(denominator).unwrap();
    let mut prices = [0_u64; MAX_OUTCOMES];
    for (&coordinate, &mass) in coordinates.iter().zip(masses) {
        let weights = basis.evaluate(coordinate).unwrap();
        let mut outcome = 0_usize;
        while outcome < usize::from(basis.outcome_count) {
            prices[outcome] = prices[outcome]
                .checked_add(weights.weights[outcome].checked_mul(mass).unwrap())
                .unwrap();
            outcome += 1;
        }
    }
    PriceVectorV2 {
        basis_degree: basis.degree,
        outcome_count: basis.outcome_count,
        price_scale: scale,
        prices,
    }
}

fn verify_quantized_staged(
    expected: &AdapterBindingsV2,
    basis: &BasisSpec,
    prices: &PriceVectorV2,
    witness: &QuantizedAtomWitnessV2,
) -> clutch_price_measure::Result<VerifiedPriceMeasureV2> {
    let mut accumulator =
        QuantizedPriceMeasureAccumulatorV2::begin(expected, basis, prices, witness)?;
    while accumulator.atom_cursor() < accumulator.atom_count() {
        let atom = accumulator.atom_cursor();
        accumulator.accumulate_atom(atom)?;
    }
    accumulator.finish()
}

fn verify_quantized_both(
    expected: &AdapterBindingsV2,
    basis: &BasisSpec,
    prices: &PriceVectorV2,
    witness: &QuantizedAtomWitnessV2,
) -> clutch_price_measure::Result<VerifiedPriceMeasureV2> {
    let monolithic = verify_quantized_price_measure_v2(expected, basis, prices, witness);
    let staged = verify_quantized_staged(expected, basis, prices, witness);
    assert_eq!(staged, monolithic);
    staged
}

#[test]
fn frozen_continuous_vectors_remain_exact() {
    let mut seen = 0_usize;
    for line in FROZEN_CONTINUOUS.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('|');
        let name = fields.next().unwrap();
        let degree = fields.next().unwrap().parse::<u8>().unwrap();
        let outcomes = fields.next().unwrap().parse::<u8>().unwrap();
        let scale = fields.next().unwrap().parse::<u64>().unwrap();
        let prices = parse_u64s(fields.next().unwrap());
        let denominator = fields.next().unwrap().parse::<u64>().unwrap();
        let moments = parse_u64s(fields.next().unwrap());
        assert!(fields.next().is_none(), "{name}: trailing field");
        assert_eq!(prices.len(), usize::from(outcomes));
        let price = price_vector(degree, &prices, scale);
        let witness = continuous_witness(degree, outcomes, denominator, &moments);
        verify_continuous_price_measure_v2(&bindings(), &price, &witness)
            .unwrap_or_else(|error| panic!("{name}: refused with {error:?}"));
        seen += 1;
    }
    assert_eq!(seen, 6);
}

#[test]
fn generated_tables_match_the_production_basis_at_exact_polynomial_nodes() {
    const EVALUATION_SCALE: u64 = 768;
    let mut degree = 2_u8;
    while degree <= 3 {
        let mut outcomes = degree + 1;
        while usize::from(outcomes) <= MAX_OUTCOMES {
            let knot_count = usize::from(outcomes + 1 - degree);
            let mut knots = [0_u128; MAX_KNOTS];
            let mut knot = 0_usize;
            while knot < knot_count {
                knots[knot] = u128::try_from(knot).unwrap() * 4;
                knot += 1;
            }
            let basis = BasisSpec {
                outcome_count: outcomes,
                degree,
                knot_count: u8::try_from(knot_count).unwrap(),
                uniform_log2_spacing: 2,
                denominator: EVALUATION_SCALE,
                domain_max: knots[knot_count - 1],
                edge_policy: EdgePolicy::Clamp,
                knots,
            };
            basis.validate().unwrap();
            let spans = outcomes - degree;
            let mut span = 0_u8;
            while span < spans {
                let table = transfer_span_v2(degree, outcomes, span).unwrap();
                let mut column = 0_usize;
                while column <= usize::from(degree) {
                    let sum: u16 = table
                        .numerators
                        .iter()
                        .map(|row| u16::from(row[column]))
                        .sum();
                    assert_eq!(sum, u16::from(table.denominator));
                    column += 1;
                }
                let mut quarter = 0_u8;
                while quarter <= 4 {
                    let coordinate = u128::from(span) * 4 + u128::from(quarter);
                    let actual = basis.evaluate(coordinate).unwrap();
                    let mut outcome = 0_usize;
                    while outcome < usize::from(outcomes) {
                        let expected = expected_transfer_weight(
                            &table,
                            degree,
                            outcome,
                            quarter,
                            EVALUATION_SCALE,
                        );
                        assert_eq!(
                            actual.weights[outcome], expected,
                            "degree={degree} outcomes={outcomes} span={span} q={quarter} outcome={outcome}"
                        );
                        outcome += 1;
                    }
                    quarter += 1;
                }
                span += 1;
            }
            outcomes += 1;
        }
        degree += 1;
    }
}

#[test]
fn continuous_checker_refuses_the_named_v1b_false_acceptance() {
    let price = price_vector(2, &[4, 8, 0, 0, 0], 12);
    assert!(v1b_degree_two_accepts(&price.prices[..5], 12));
    let witness = continuous_witness(2, 5, 3, &[1, 2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &price, &witness),
        Err(ErrorV2::QuadraticMomentOutsideCone { span: 0 })
    );
    // The corresponding nonnegative portfolio is `(3x-1)^2` and costs -S.
    let portfolio = [1_i64, -2, 10, 40, 64];
    let cost: i64 = portfolio
        .iter()
        .zip(price.prices)
        .map(|(&coefficient, price)| coefficient * i64::try_from(price).unwrap())
        .sum();
    assert_eq!(cost, -12);
}

#[test]
fn quantized_live_point_that_v1b_refuses_has_an_exact_runtime_certificate() {
    let basis = basis(2, &[0, 128, 256, 384], 10_000, 7);
    let weights = basis.evaluate(85).unwrap();
    assert_eq!(&weights.weights[..5], &[1_128, 6_667, 2_205, 0, 0]);
    assert!(!v1b_degree_two_accepts(&weights.weights[..5], 10_000));
    let price = price_vector(2, &weights.weights[..5], 10_000);
    let witness = quantized_witness(&[85], &[1], 1);
    verify_quantized_both(&bindings(), &basis, &price, &witness).unwrap();
}

#[test]
fn quantization_can_make_a_coherent_knot_point_one_hot() {
    let basis = basis(2, &[0, 1, 2, 3], 1, 0);
    let weights = basis.evaluate(1).unwrap();
    assert_eq!(&weights.weights[..5], &[0, 1, 0, 0, 0]);
    assert!(!v1b_degree_two_accepts(&weights.weights[..5], 1));
    let price = price_vector(2, &weights.weights[..5], 1);
    let witness = quantized_witness(&[1], &[1], 1);
    verify_quantized_both(&bindings(), &basis, &price, &witness).unwrap();
}

#[test]
fn continuous_single_span_hankel_acceptance_can_be_a_runtime_false_acceptance() {
    let price = price_vector(2, &[1, 2, 1], 4);
    let continuous = continuous_witness(2, 3, 4, &[1, 2, 1]);
    verify_continuous_price_measure_v2(&bindings(), &price, &continuous).unwrap();

    // On the integer grid [0,1], the only runtime vertices are the endpoints,
    // so every runtime mixture has a zero middle coordinate.
    let basis = basis(2, &[0, 1], 4, 0);
    assert_eq!(&basis.evaluate(0).unwrap().weights[..3], &[4, 0, 0]);
    assert_eq!(&basis.evaluate(1).unwrap().weights[..3], &[0, 0, 4]);
    let quantized = quantized_witness(&[0, 1], &[1, 1], 2);
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &quantized),
        Err(ErrorV2::PriceReconstructionMismatch { outcome: 0 })
    );
}

#[test]
fn canonical_atom_encoding_is_not_a_unique_witness_for_one_price() {
    let basis = basis(2, &[0, 4, 8, 12], 8, 2);
    let at_five = basis.evaluate(5).unwrap();
    let at_six = basis.evaluate(6).unwrap();
    let at_seven = basis.evaluate(7).unwrap();
    assert_eq!(&at_five.weights[..5], &[0, 2, 6, 0, 0]);
    assert_eq!(&at_six.weights[..5], &[0, 1, 6, 1, 0]);
    assert_eq!(&at_seven.weights[..5], &[0, 0, 6, 2, 0]);
    let price = price_vector(2, &at_six.weights[..5], 8);

    let one_atom = quantized_witness(&[6], &[1], 1);
    let one_verified = verify_quantized_both(&bindings(), &basis, &price, &one_atom).unwrap();

    let mut two_atoms = quantized_witness(&[5, 7], &[1, 1], 2);
    two_atoms.body_digest = [6; 32];
    let second_binding = AdapterBindingsV2 {
        observed_body_digest: [6; 32],
        ..bindings()
    };
    let two_verified = verify_quantized_both(&second_binding, &basis, &price, &two_atoms).unwrap();
    assert_eq!(
        one_atom.candidate_price_digest,
        two_atoms.candidate_price_digest
    );
    assert_eq!(
        (
            one_verified.basis_degree(),
            one_verified.outcome_count,
            one_verified.span_count,
        ),
        (
            two_verified.basis_degree(),
            two_verified.outcome_count,
            two_verified.span_count,
        )
    );
    assert_ne!(
        one_verified.common_denominator(),
        two_verified.common_denominator()
    );
    assert_ne!(one_atom.body_digest, two_atoms.body_digest);
}

#[test]
fn staged_atom_cursor_and_finish_are_exact_and_transactional() {
    let basis = basis(3, &[8, 16, 24], 10_000, 3);
    let coordinates = [8_u128, 13, 24];
    let masses = [1_u64, 2, 4];
    let price = quantized_mixture_prices(&basis, &coordinates, &masses);
    let witness = quantized_witness(&coordinates, &masses, 7);
    let mut accumulator =
        QuantizedPriceMeasureAccumulatorV2::begin(&bindings(), &basis, &price, &witness).unwrap();

    assert_eq!(accumulator.atom_cursor(), 0);
    assert_eq!(accumulator.atom_count(), 3);
    let initial = accumulator.clone();
    assert_eq!(
        accumulator.accumulate_atom(1),
        Err(ErrorV2::AtomCursorMismatch {
            expected: 0,
            provided: 1,
        })
    );
    assert_eq!(accumulator, initial);
    assert_eq!(
        accumulator.clone().finish(),
        Err(ErrorV2::IncompleteAtomAccumulation {
            cursor: 0,
            atom_count: 3,
        })
    );

    accumulator.accumulate_atom(0).unwrap();
    let after_first = accumulator.clone();
    assert_eq!(
        accumulator.accumulate_atom(0),
        Err(ErrorV2::AtomCursorMismatch {
            expected: 1,
            provided: 0,
        })
    );
    assert_eq!(accumulator, after_first);
    assert_eq!(
        accumulator.accumulate_atom(2),
        Err(ErrorV2::AtomCursorMismatch {
            expected: 1,
            provided: 2,
        })
    );
    assert_eq!(accumulator, after_first);

    accumulator.accumulate_atom(1).unwrap();
    assert_eq!(
        accumulator.clone().finish(),
        Err(ErrorV2::IncompleteAtomAccumulation {
            cursor: 2,
            atom_count: 3,
        })
    );
    accumulator.accumulate_atom(2).unwrap();
    let complete = accumulator.clone();
    assert_eq!(
        accumulator.accumulate_atom(3),
        Err(ErrorV2::AtomCursorExhausted)
    );
    assert_eq!(accumulator, complete);
    assert_eq!(
        accumulator.finish(),
        verify_quantized_price_measure_v2(&bindings(), &basis, &price, &witness)
    );
}

#[test]
fn staged_begin_enforces_degree_width_count_mass_and_primitive_bounds() {
    let basis = basis(2, &[0, 4, 8, 12], 8, 2);
    let coordinates = [0_u128, 6, 12];
    let masses = [1_u64, 2, 4];
    let price = quantized_mixture_prices(&basis, &coordinates, &masses);
    let witness = quantized_witness(&coordinates, &masses, 7);

    for (mutated, expected) in [
        (
            QuantizedAtomWitnessV2 {
                atom_count: 0,
                ..witness
            },
            ErrorV2::InvalidAtomCount,
        ),
        (
            QuantizedAtomWitnessV2 {
                atom_count: price.outcome_count + 1,
                ..witness
            },
            ErrorV2::InvalidAtomCount,
        ),
        (
            QuantizedAtomWitnessV2 {
                common_denominator: 0,
                ..witness
            },
            ErrorV2::InvalidCommonDenominator,
        ),
        (
            QuantizedAtomWitnessV2 {
                common_denominator: 8,
                ..witness
            },
            ErrorV2::AtomMassMismatch,
        ),
        (
            QuantizedAtomWitnessV2 {
                common_denominator: 14,
                atom_masses: {
                    let mut values = witness.atom_masses;
                    values[..3].copy_from_slice(&[2, 4, 8]);
                    values
                },
                ..witness
            },
            ErrorV2::NonPrimitiveAtomScale,
        ),
    ] {
        assert_eq!(
            verify_quantized_both(&bindings(), &basis, &price, &mutated),
            Err(expected)
        );
    }

    let invalid_degree = PriceVectorV2 {
        basis_degree: 1,
        ..price
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &invalid_degree, &witness),
        Err(ErrorV2::InvalidDegree)
    );
    let invalid_width = PriceVectorV2 {
        outcome_count: 17,
        ..price
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &invalid_width, &witness),
        Err(ErrorV2::InvalidOutcomeCount)
    );
    let mismatched_basis_shape = PriceVectorV2 {
        basis_degree: 3,
        ..price
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &mismatched_basis_shape, &witness),
        Err(ErrorV2::InvalidBasis)
    );
}

#[test]
fn quantized_cross_span_mixtures_verify_for_every_shape() {
    for degree in [2_u8, 3] {
        let mut outcomes = degree + 1;
        while usize::from(outcomes) <= MAX_OUTCOMES {
            let knot_count = usize::from(outcomes + 1 - degree);
            let mut knots = [0_u128; MAX_KNOTS];
            let mut knot = 0_usize;
            while knot < knot_count {
                knots[knot] = u128::try_from(knot).unwrap() * 8;
                knot += 1;
            }
            let basis = basis(degree, &knots[..knot_count], 10_000, 3);
            let last = knots[knot_count - 1];
            let coordinates = [0_u128, last / 2, last];
            let masses = [1_u64, 2, 4];
            let price = quantized_mixture_prices(&basis, &coordinates, &masses);
            let witness = quantized_witness(&coordinates, &masses, 7);
            verify_quantized_both(&bindings(), &basis, &price, &witness).unwrap();
            outcomes += 1;
        }
    }
}

#[test]
fn quantized_arithmetic_accepts_the_full_u64_denominator_boundary() {
    let basis = basis(3, &[0, 4], u64::MAX, 2);
    let denominator = u64::MAX;
    let masses = [denominator - 1, 1];
    let coordinates = [0_u128, 4];
    let price = price_vector(3, &[denominator - 1, 0, 0, 1], denominator);
    let witness = quantized_witness(&coordinates, &masses, denominator);
    verify_quantized_both(&bindings(), &basis, &price, &witness).unwrap();
    assert_eq!(MAX_COMMON_DENOMINATOR, denominator);
}

#[test]
fn full_u64_continuous_denominator_is_exact_and_basis_overflow_refuses() {
    let denominator = u64::MAX;
    let price = price_vector(3, &[denominator - 1, 0, 0, 1], denominator);
    let witness = continuous_witness(3, 4, denominator, &[denominator - 1, 0, 0, 1]);
    verify_continuous_price_measure_v2(&bindings(), &price, &witness).unwrap();

    let huge_gap = 1_u128 << 100;
    let invalid_basis = basis(3, &[0, huge_gap], 1, 100);
    let quantized = quantized_witness(&[0], &[1], 1);
    let endpoint_price = price_vector(3, &[1, 0, 0, 0], 1);
    assert_eq!(
        verify_quantized_both(&bindings(), &invalid_basis, &endpoint_price, &quantized),
        Err(ErrorV2::InvalidBasis)
    );
}

#[test]
fn quantized_certificate_mutations_refuse_without_rounding_or_clamping() {
    let basis = basis(3, &[8, 16, 24], 10_000, 3);
    let coordinates = [8_u128, 13, 24];
    let masses = [1_u64, 2, 4];
    let price = quantized_mixture_prices(&basis, &coordinates, &masses);
    let witness = quantized_witness(&coordinates, &masses, 7);

    let wrong_price = {
        let mut value = price;
        value.prices[0] += 1;
        value.prices[1] -= 1;
        value
    };
    assert!(matches!(
        verify_quantized_both(&bindings(), &basis, &wrong_price, &witness),
        Err(ErrorV2::PriceReconstructionMismatch { .. })
    ));

    let out_of_range = QuantizedAtomWitnessV2 {
        atom_coordinates: {
            let mut values = witness.atom_coordinates;
            values[0] = 7;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &out_of_range),
        Err(ErrorV2::AtomCoordinateOutOfRange { atom: 0 })
    );

    let duplicate = QuantizedAtomWitnessV2 {
        atom_coordinates: {
            let mut values = witness.atom_coordinates;
            values[1] = values[0];
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &duplicate),
        Err(ErrorV2::NonCanonicalAtomOrder { atom: 1 })
    );

    let zero_mass = QuantizedAtomWitnessV2 {
        atom_masses: {
            let mut values = witness.atom_masses;
            values[1] = 0;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &zero_mass),
        Err(ErrorV2::ZeroAtomMass { atom: 1 })
    );

    let padded = QuantizedAtomWitnessV2 {
        atom_coordinates: {
            let mut values = witness.atom_coordinates;
            values[3] = 17;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &padded),
        Err(ErrorV2::NonCanonicalAtomPadding { atom: 3 })
    );

    let nonprimitive = QuantizedAtomWitnessV2 {
        common_denominator: 14,
        atom_masses: {
            let mut values = witness.atom_masses;
            values[..3].copy_from_slice(&[2, 4, 8]);
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_both(&bindings(), &basis, &price, &nonprimitive),
        Err(ErrorV2::NonPrimitiveAtomScale)
    );
}

#[test]
fn policies_bindings_shapes_padding_and_cubic_sides_refuse_exactly() {
    let price = price_vector(3, &[8, 12, 6, 1], 27);
    let witness = continuous_witness(3, 4, 27, &[8, 12, 6, 1]);

    let wrong_binding = AdapterBindingsV2 {
        basis_digest: [99; 32],
        ..bindings()
    };
    assert_eq!(
        verify_continuous_price_measure_v2(&wrong_binding, &price, &witness),
        Err(ErrorV2::BindingMismatch {
            field: BindingFieldV2::BasisDigest
        })
    );

    let rounded = ContinuousPriceMeasureWitnessV2 {
        payout_rounding_boundary: PayoutRoundingBoundaryV2::LargestRemainderLowestIndexV1,
        ..witness
    };
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &price, &rounded),
        Err(ErrorV2::UnsupportedPayoutRoundingBoundary)
    );

    let wrong_shape = ContinuousPriceMeasureWitnessV2 {
        outcome_count: 5,
        ..witness
    };
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &price, &wrong_shape),
        Err(ErrorV2::ContinuousWitnessShapeMismatch)
    );

    let mut padded_price = price;
    padded_price.prices[4] = 1;
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &padded_price, &witness),
        Err(ErrorV2::NonCanonicalPricePadding { outcome: 4 })
    );

    let padded_moment = ContinuousPriceMeasureWitnessV2 {
        moments: {
            let mut values = witness.moments;
            values[4] = 1;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &price, &padded_moment),
        Err(ErrorV2::NonCanonicalMomentPadding { cell: 4 })
    );

    let left_failure = continuous_witness(3, 4, 3, &[1, 2, 0, 0]);
    let left_price = price_vector(3, &[1, 2, 0, 0], 3);
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &left_price, &left_failure),
        Err(ErrorV2::CubicMomentOutsideCone {
            span: 0,
            constraint: CubicConstraintV2::Left,
        })
    );

    let right_failure = continuous_witness(3, 4, 3, &[0, 0, 2, 1]);
    let right_price = price_vector(3, &[0, 0, 2, 1], 3);
    assert_eq!(
        verify_continuous_price_measure_v2(&bindings(), &right_price, &right_failure),
        Err(ErrorV2::CubicMomentOutsideCone {
            span: 0,
            constraint: CubicConstraintV2::Right,
        })
    );
}

#[test]
fn versions_semantics_and_transfer_shape_are_explicit_refusals() {
    let price = price_vector(2, &[1, 2, 1], 4);
    let witness = continuous_witness(2, 3, 4, &[1, 2, 1]);

    for (mutated, expected) in [
        (
            ContinuousPriceMeasureWitnessV2 {
                schema_version: 0,
                ..witness
            },
            ErrorV2::UnsupportedSchemaVersion,
        ),
        (
            ContinuousPriceMeasureWitnessV2 {
                transfer_table_version: 0,
                ..witness
            },
            ErrorV2::UnsupportedTransferTableVersion,
        ),
        (
            ContinuousPriceMeasureWitnessV2 {
                basis_semantics: BasisSemanticsV2::CallerSuppliedTransferForbidden,
                ..witness
            },
            ErrorV2::UnsupportedBasisSemantics,
        ),
        (
            ContinuousPriceMeasureWitnessV2 {
                price_rounding_boundary: PriceRoundingBoundaryV2::VerifierSideRoundingForbidden,
                ..witness
            },
            ErrorV2::UnsupportedPriceRoundingBoundary,
        ),
    ] {
        assert_eq!(
            verify_continuous_price_measure_v2(&bindings(), &price, &mutated),
            Err(expected)
        );
    }

    assert_eq!(transfer_span_v2(1, 3, 0), Err(ErrorV2::InvalidDegree));
    assert_eq!(transfer_span_v2(2, 2, 0), Err(ErrorV2::InvalidOutcomeCount));
    assert_eq!(transfer_span_v2(2, 3, 1), Err(ErrorV2::InvalidSpanCount));
}

fn expected_transfer_weight(
    table: &clutch_price_measure::TransferSpanV2,
    degree: u8,
    outcome: usize,
    quarter: u8,
    scale: u64,
) -> u64 {
    let first = usize::from(table.first_outcome);
    let width = usize::from(degree) + 1;
    if outcome < first || outcome >= first + width {
        return 0;
    }
    let local = outcome - first;
    let mut numerator = 0_u128;
    let mut bernstein = 0_u8;
    while bernstein <= degree {
        let coefficient = u128::from(table.numerators[local][usize::from(bernstein)]);
        let basis_numerator = u128::from(binomial(degree, bernstein))
            * u128::from(quarter).pow(u32::from(bernstein))
            * u128::from(4 - quarter).pow(u32::from(degree - bernstein));
        numerator += coefficient * basis_numerator;
        bernstein += 1;
    }
    let denominator = u128::from(table.denominator) * 4_u128.pow(u32::from(degree));
    let scaled = numerator * u128::from(scale);
    assert_eq!(scaled % denominator, 0);
    u64::try_from(scaled / denominator).unwrap()
}

fn binomial(degree: u8, index: u8) -> u8 {
    match (degree, index) {
        (_, 0) => 1,
        (2, 1) => 2,
        (2, 2) => 1,
        (3, 1) | (3, 2) => 3,
        (3, 3) => 1,
        _ => panic!("unsupported binomial"),
    }
}

fn v1b_degree_two_accepts(prices: &[u64], scale: u64) -> bool {
    if prices.len() < 3 || prices.iter().copied().sum::<u64>() != scale {
        return false;
    }
    let outcomes = prices.len();
    let mut claim = 1_usize;
    while claim + 1 < outcomes {
        let (ceiling_num, ceiling_den) = if outcomes == 3 {
            (1_u64, 2_u64)
        } else if claim == 1 || claim + 2 == outcomes {
            (2, 3)
        } else {
            (3, 4)
        };
        if u128::from(prices[claim]) * u128::from(ceiling_den)
            > u128::from(scale) * u128::from(ceiling_num)
        {
            return false;
        }
        let weight = if outcomes == 3 {
            1_u128
        } else if claim == 1 || claim + 2 == outcomes {
            2
        } else {
            3
        };
        if u128::from(prices[claim]) > weight * u128::from(prices[claim - 1] + prices[claim + 1]) {
            return false;
        }
        claim += 1;
    }
    if outcomes == 3
        && u128::from(prices[1]) * u128::from(prices[1])
            > 4 * u128::from(prices[0]) * u128::from(prices[2])
    {
        return false;
    }
    true
}

fn parse_u64s(text: &str) -> Vec<u64> {
    text.split(',')
        .map(|value| value.parse::<u64>().unwrap())
        .collect()
}
