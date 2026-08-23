use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_price_measure::{
    verify_quantized_price_measure_v2, verify_quantized_price_measure_v3_degree_zero,
    verify_quantized_price_measure_v3_smooth, AdapterBindingsV2, AdapterBindingsV3,
    BasisSemanticsV2, BindingFieldV3, DegreeZeroPayoutTableV3, ErrorV2, ErrorV3,
    PayoutRoundingBoundaryV2, PriceRoundingBoundaryV2, PriceVectorV2, PriceVectorV3,
    QuantizedAtomWitnessV2, QuantizedAtomWitnessV3, QuantizedPriceMeasureAccumulatorV3,
    VerifiedPriceMeasureV2, VerifiedPriceMeasureV3, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    PAYOUT_MAP_UNUSED_V3, PRICE_MEASURE_WITNESS_VERSION_V2, PRICE_MEASURE_WITNESS_VERSION_V3,
    QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V2, QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3,
    QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V2, QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V3,
    QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};

fn bindings_v3() -> AdapterBindingsV3 {
    AdapterBindingsV3 {
        candidate_feed: [1; 32],
        relation_domain_digest: [2; 32],
        basis_digest: [3; 32],
        candidate_price_digest: [4; 32],
        observed_body_digest: [5; 32],
    }
}

fn bindings_v2() -> AdapterBindingsV2 {
    AdapterBindingsV2 {
        candidate_feed: [1; 32],
        relation_domain_digest: [2; 32],
        basis_digest: [3; 32],
        candidate_price_digest: [4; 32],
        observed_body_digest: [5; 32],
    }
}

fn finite_table() -> DegreeZeroPayoutTableV3 {
    let mut payout_weights = [[0_u64; MAX_OUTCOMES]; MAX_OUTCOMES];
    payout_weights[0][..4].copy_from_slice(&[3, 2, 1, 1]);
    payout_weights[1][..4].copy_from_slice(&[0, 4, 2, 1]);
    payout_weights[2][..4].copy_from_slice(&[1, 0, 3, 3]);
    let mut payout_map = [PAYOUT_MAP_UNUSED_V3; MAX_OUTCOMES];
    payout_map[..4].copy_from_slice(&[0, 1, 0, 2]);
    let mut knots = [0_u128; MAX_OUTCOMES];
    knots[..3].copy_from_slice(&[2, 5, 7]);
    DegreeZeroPayoutTableV3 {
        native_outcome_count: 4,
        payout_count: 3,
        knot_count: 3,
        payout_denominator: 7,
        domain_min: 0,
        domain_max: 9,
        payout_weights,
        payout_map,
        knots,
    }
}

fn constant_finite_table() -> DegreeZeroPayoutTableV3 {
    let mut payout_weights = [[0_u64; MAX_OUTCOMES]; MAX_OUTCOMES];
    payout_weights[0][..3].copy_from_slice(&[2, 3, 5]);
    let mut payout_map = [PAYOUT_MAP_UNUSED_V3; MAX_OUTCOMES];
    payout_map[..3].fill(0);
    let mut knots = [0_u128; MAX_OUTCOMES];
    knots[..2].copy_from_slice(&[4, 8]);
    DegreeZeroPayoutTableV3 {
        native_outcome_count: 3,
        payout_count: 1,
        knot_count: 2,
        payout_denominator: 10,
        domain_min: 2,
        domain_max: 10,
        payout_weights,
        payout_map,
        knots,
    }
}

fn linear_basis(outcomes: u8, denominator: u64, first: u128) -> BasisSpec {
    let mut knots = [0_u128; MAX_KNOTS];
    let mut knot = 0_u8;
    while knot < outcomes {
        knots[usize::from(knot)] = first + u128::from(knot) * 2;
        knot += 1;
    }
    BasisSpec {
        outcome_count: outcomes,
        degree: 1,
        knot_count: outcomes,
        uniform_log2_spacing: 1,
        denominator,
        domain_max: knots[usize::from(outcomes - 1)],
        edge_policy: EdgePolicy::Clamp,
        knots,
    }
}

fn uniform_smooth_basis(outcomes: u8, degree: u8, denominator: u64) -> BasisSpec {
    let knot_count = outcomes + 1 - degree;
    let mut knots = [0_u128; MAX_KNOTS];
    let mut knot = 0_u8;
    while knot < knot_count {
        knots[usize::from(knot)] = u128::from(knot) * 2;
        knot += 1;
    }
    BasisSpec {
        outcome_count: outcomes,
        degree,
        knot_count,
        uniform_log2_spacing: 1,
        denominator,
        domain_max: knots[usize::from(knot_count - 1)],
        edge_policy: EdgePolicy::Clamp,
        knots,
    }
}

fn witness_v3(
    degree: u8,
    native_outcome_count: u8,
    coordinates: &[u128],
    masses: &[u64],
) -> QuantizedAtomWitnessV3 {
    assert_eq!(coordinates.len(), masses.len());
    let mut atom_coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
    let mut atom_masses = [0_u64; MAX_QUANTIZED_ATOMS];
    atom_coordinates[..coordinates.len()].copy_from_slice(coordinates);
    atom_masses[..masses.len()].copy_from_slice(masses);
    let bindings = bindings_v3();
    QuantizedAtomWitnessV3 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V3,
        quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        basis_degree: degree,
        native_outcome_count,
        atom_count: u8::try_from(coordinates.len()).unwrap(),
        common_denominator: masses.iter().copied().sum(),
        atom_coordinates,
        atom_masses,
    }
}

fn finite_prices(
    table: &DegreeZeroPayoutTableV3,
    coordinates: &[u128],
    masses: &[u64],
) -> PriceVectorV3 {
    let mass_total: u64 = masses.iter().copied().sum();
    let mut prices = [0_u64; MAX_OUTCOMES];
    for (&coordinate, &mass) in coordinates.iter().zip(masses) {
        let payout = table.evaluate(coordinate).unwrap();
        let mut outcome = 0_usize;
        while outcome < usize::from(table.native_outcome_count) {
            prices[outcome] = prices[outcome]
                .checked_add(payout.weights[outcome].checked_mul(mass).unwrap())
                .unwrap();
            outcome += 1;
        }
    }
    PriceVectorV3 {
        basis_degree: 0,
        native_outcome_count: table.native_outcome_count,
        price_scale: table.payout_denominator.checked_mul(mass_total).unwrap(),
        prices,
    }
}

fn smooth_prices(basis: &BasisSpec, coordinates: &[u128], masses: &[u64]) -> PriceVectorV3 {
    let mass_total: u64 = masses.iter().copied().sum();
    let mut prices = [0_u64; MAX_OUTCOMES];
    for (&coordinate, &mass) in coordinates.iter().zip(masses) {
        let payout = basis.evaluate(coordinate).unwrap();
        let mut outcome = 0_usize;
        while outcome < usize::from(basis.outcome_count) {
            prices[outcome] = prices[outcome]
                .checked_add(payout.weights[outcome].checked_mul(mass).unwrap())
                .unwrap();
            outcome += 1;
        }
    }
    PriceVectorV3 {
        basis_degree: basis.degree,
        native_outcome_count: basis.outcome_count,
        price_scale: basis.denominator.checked_mul(mass_total).unwrap(),
        prices,
    }
}

fn verify_finite_both(
    table: &DegreeZeroPayoutTableV3,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> Result<VerifiedPriceMeasureV3, ErrorV3> {
    let monolithic =
        verify_quantized_price_measure_v3_degree_zero(&bindings_v3(), table, prices, witness);
    let staged = (|| {
        let mut accumulator = QuantizedPriceMeasureAccumulatorV3::begin_degree_zero(
            &bindings_v3(),
            table,
            prices,
            witness,
        )?;
        while accumulator.atom_cursor() < accumulator.atom_count() {
            let atom = accumulator.atom_cursor();
            accumulator.accumulate_atom(atom)?;
        }
        accumulator.finish()
    })();
    assert_eq!(monolithic, staged);
    monolithic
}

fn verify_smooth_both(
    basis: &BasisSpec,
    prices: &PriceVectorV3,
    witness: &QuantizedAtomWitnessV3,
) -> Result<VerifiedPriceMeasureV3, ErrorV3> {
    let monolithic =
        verify_quantized_price_measure_v3_smooth(&bindings_v3(), basis, prices, witness);
    let staged = (|| {
        let mut accumulator = QuantizedPriceMeasureAccumulatorV3::begin_smooth(
            &bindings_v3(),
            basis,
            prices,
            witness,
        )?;
        while accumulator.atom_cursor() < accumulator.atom_count() {
            let atom = accumulator.atom_cursor();
            accumulator.accumulate_atom(atom)?;
        }
        accumulator.finish()
    })();
    assert_eq!(monolithic, staged);
    monolithic
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[test]
fn v3_staged_layout_borrows_large_inputs_and_stays_below_the_sbf_frame_budget() {
    assert!(core::mem::size_of::<DegreeZeroPayoutTableV3>() <= 2_400);
    assert!(core::mem::size_of::<QuantizedPriceMeasureAccumulatorV3<'static>>() <= 1_536);
    assert!(core::mem::size_of::<QuantizedAtomWitnessV3>() <= 768);
}

#[test]
fn degree_zero_reconstructs_exhaustive_small_domain_non_one_hot_geometry() {
    for table in [finite_table(), constant_finite_table()] {
        table.validate().unwrap();
        let mut coordinate = table.domain_min;
        while coordinate <= table.domain_max {
            let coordinates = [coordinate];
            let masses = [1];
            let prices = finite_prices(&table, &coordinates, &masses);
            let witness = witness_v3(0, table.native_outcome_count, &coordinates, &masses);
            verify_finite_both(&table, &prices, &witness).unwrap();
            coordinate += 1;
        }

        let mut left = table.domain_min;
        while left < table.domain_max {
            let mut right = left + 1;
            while right <= table.domain_max {
                for left_mass in 1..=3 {
                    for right_mass in 1..=3 {
                        if gcd(left_mass, right_mass) == 1 {
                            let coordinates = [left, right];
                            let masses = [left_mass, right_mass];
                            let prices = finite_prices(&table, &coordinates, &masses);
                            let witness =
                                witness_v3(0, table.native_outcome_count, &coordinates, &masses);
                            verify_finite_both(&table, &prices, &witness).unwrap();
                        }
                    }
                }
                right += 1;
            }
            left += 1;
        }
    }

    let table = finite_table();
    assert_ne!(table.payout_count, table.native_outcome_count);
    assert_eq!(table.payout_map[..4], [0, 1, 0, 2]);
    assert_eq!(table.evaluate(1).unwrap().weights[..4], [3, 2, 1, 1]);
    assert_eq!(table.evaluate(2).unwrap().weights[..4], [0, 4, 2, 1]);
    assert_eq!(table.evaluate(5).unwrap().weights[..4], [3, 2, 1, 1]);
    assert_eq!(table.evaluate(7).unwrap().weights[..4], [1, 0, 3, 3]);
}

#[test]
fn degree_one_reconstructs_every_small_point_and_primitive_two_atom_measure() {
    let mut outcomes = 2_u8;
    while outcomes <= 5 {
        let mut denominator = 1_u64;
        while denominator <= 4 {
            let basis = linear_basis(outcomes, denominator, 0);
            let first = basis.knots[0];
            let last = basis.knots[usize::from(basis.knot_count) - 1];
            let mut coordinate = first;
            while coordinate <= last {
                let coordinates = [coordinate];
                let masses = [1];
                let prices = smooth_prices(&basis, &coordinates, &masses);
                let witness = witness_v3(1, outcomes, &coordinates, &masses);
                verify_smooth_both(&basis, &prices, &witness).unwrap();
                coordinate += 1;
            }
            let mut left = first;
            while left < last {
                let mut right = left + 1;
                while right <= last {
                    for left_mass in 1..=3 {
                        for right_mass in 1..=3 {
                            if gcd(left_mass, right_mass) == 1 {
                                let coordinates = [left, right];
                                let masses = [left_mass, right_mass];
                                let prices = smooth_prices(&basis, &coordinates, &masses);
                                let witness = witness_v3(1, outcomes, &coordinates, &masses);
                                verify_smooth_both(&basis, &prices, &witness).unwrap();
                            }
                        }
                    }
                    right += 1;
                }
                left += 1;
            }
            denominator += 1;
        }
        outcomes += 1;
    }

    let nonuniform = BasisSpec {
        outcome_count: 3,
        degree: 1,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        denominator: 7,
        domain_max: 6,
        edge_policy: EdgePolicy::Refuse,
        knots: {
            let mut values = [0; MAX_KNOTS];
            values[..3].copy_from_slice(&[1, 3, 6]);
            values
        },
    };
    for coordinate in 1..=6 {
        let prices = smooth_prices(&nonuniform, &[coordinate], &[1]);
        let witness = witness_v3(1, 3, &[coordinate], &[1]);
        verify_smooth_both(&nonuniform, &prices, &witness).unwrap();
    }
}

#[test]
fn v3_smooth_degrees_two_and_three_retain_exact_production_evaluation() {
    for degree in [2_u8, 3] {
        let mut outcomes = degree + 1;
        while outcomes <= 6 {
            let basis = uniform_smooth_basis(outcomes, degree, 11);
            let last = basis.knots[usize::from(basis.knot_count) - 1];
            let mut coordinate = 0_u128;
            while coordinate <= last {
                let prices = smooth_prices(&basis, &[coordinate], &[1]);
                let witness = witness_v3(degree, outcomes, &[coordinate], &[1]);
                verify_smooth_both(&basis, &prices, &witness).unwrap();
                coordinate += 1;
            }
            outcomes += 1;
        }
    }
}

#[test]
fn finite_table_refuses_noncanonical_shape_rows_map_knots_and_domain() {
    let base = finite_table();
    for (mutated, expected) in [
        (
            DegreeZeroPayoutTableV3 {
                payout_count: 0,
                ..base
            },
            ErrorV3::InvalidDegreeZeroShape,
        ),
        (
            DegreeZeroPayoutTableV3 {
                knot_count: 2,
                ..base
            },
            ErrorV3::InvalidDegreeZeroShape,
        ),
        (
            DegreeZeroPayoutTableV3 {
                domain_min: 9,
                ..base
            },
            ErrorV3::InvalidDegreeZeroDomain,
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_denominator: 0,
                ..base
            },
            ErrorV3::InvalidPayoutDenominator,
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_weights: {
                    let mut values = base.payout_weights;
                    values[0][0] = 8;
                    values
                },
                ..base
            },
            ErrorV3::PayoutWeightExceedsDenominator { row: 0, outcome: 0 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_weights: {
                    let mut values = base.payout_weights;
                    values[0][0] = 2;
                    values
                },
                ..base
            },
            ErrorV3::PayoutRowSimplexMismatch { row: 0 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_weights: {
                    let mut values = base.payout_weights;
                    values[0][4] = 1;
                    values
                },
                ..base
            },
            ErrorV3::NonCanonicalPayoutPadding { row: 0, outcome: 4 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_weights: {
                    let mut values = base.payout_weights;
                    values[2] = values[0];
                    values
                },
                ..base
            },
            ErrorV3::DuplicatePayoutRow {
                first: 0,
                second: 2,
            },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_map: {
                    let mut values = base.payout_map;
                    values[0] = 3;
                    values
                },
                ..base
            },
            ErrorV3::PayoutMapOutOfRange { cell: 0 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_map: {
                    let mut values = base.payout_map;
                    values[0] = 1;
                    values
                },
                ..base
            },
            ErrorV3::NonCanonicalPayoutMapOrder { cell: 0 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                payout_map: {
                    let mut values = base.payout_map;
                    values[4] = 0;
                    values
                },
                ..base
            },
            ErrorV3::NonCanonicalPayoutMapPadding { cell: 4 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                knots: {
                    let mut values = base.knots;
                    values[1] = 2;
                    values
                },
                ..base
            },
            ErrorV3::InvalidDegreeZeroKnot { knot: 1 },
        ),
        (
            DegreeZeroPayoutTableV3 {
                knots: {
                    let mut values = base.knots;
                    values[3] = 8;
                    values
                },
                ..base
            },
            ErrorV3::NonCanonicalKnotPadding { knot: 3 },
        ),
    ] {
        assert_eq!(mutated.validate(), Err(expected));
    }
}

#[test]
fn versions_modes_bindings_support_and_padding_refuse() {
    assert_eq!(PRICE_MEASURE_WITNESS_VERSION_V2, 2);
    assert_eq!(PRICE_MEASURE_WITNESS_VERSION_V3, 3);
    assert_eq!(QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1, 1);
    assert_eq!(QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V2, 2);
    assert_eq!(QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V2, 3);
    assert_eq!(QUANTIZED_PRICE_MEASURE_MIN_DEGREE_V3, 0);
    assert_eq!(QUANTIZED_PRICE_MEASURE_MAX_DEGREE_V3, 3);
    assert_eq!(PAYOUT_MAP_UNUSED_V3, u8::MAX);

    let table = finite_table();
    let coordinates = [0, 3, 8];
    let masses = [1, 2, 4];
    let prices = finite_prices(&table, &coordinates, &masses);
    let witness = witness_v3(0, 4, &coordinates, &masses);

    let wrong_version = QuantizedAtomWitnessV3 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V2,
        ..witness
    };
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &prices,
            &wrong_version,
        ),
        Err(ErrorV3::UnsupportedSchemaVersion)
    );
    let wrong_semantics = QuantizedAtomWitnessV3 {
        quantized_semantics_version: 0,
        ..witness
    };
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &prices,
            &wrong_semantics,
        ),
        Err(ErrorV3::UnsupportedQuantizedSemanticsVersion)
    );
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &prices,
            &QuantizedAtomWitnessV3 {
                native_outcome_count: 3,
                ..witness
            },
        ),
        Err(ErrorV3::WitnessShapeMismatch)
    );
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &PriceVectorV3 {
                basis_degree: 4,
                ..prices
            },
            &QuantizedAtomWitnessV3 {
                basis_degree: 4,
                ..witness
            },
        ),
        Err(ErrorV3::InvalidDegree)
    );
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &PriceVectorV3 {
                native_outcome_count: 1,
                ..prices
            },
            &QuantizedAtomWitnessV3 {
                native_outcome_count: 1,
                ..witness
            },
        ),
        Err(ErrorV3::InvalidNativeOutcomeCount)
    );

    for (expected, field) in [
        (
            AdapterBindingsV3 {
                relation_domain_digest: [99; 32],
                ..bindings_v3()
            },
            BindingFieldV3::RelationDomainDigest,
        ),
        (
            AdapterBindingsV3 {
                basis_digest: [99; 32],
                ..bindings_v3()
            },
            BindingFieldV3::BasisDigest,
        ),
    ] {
        assert_eq!(
            verify_quantized_price_measure_v3_degree_zero(&expected, &table, &prices, &witness,),
            Err(ErrorV3::BindingMismatch { field })
        );
    }
    for (mutated, field) in [
        (
            QuantizedAtomWitnessV3 {
                relation_domain_digest: [98; 32],
                ..witness
            },
            BindingFieldV3::RelationDomainDigest,
        ),
        (
            QuantizedAtomWitnessV3 {
                basis_digest: [97; 32],
                ..witness
            },
            BindingFieldV3::BasisDigest,
        ),
    ] {
        assert_eq!(
            verify_quantized_price_measure_v3_degree_zero(
                &bindings_v3(),
                &table,
                &prices,
                &mutated,
            ),
            Err(ErrorV3::BindingMismatch { field })
        );
    }
    assert_eq!(
        table.evaluate(table.domain_max + 1),
        Err(ErrorV3::DegreeZeroCoordinateOutOfRange)
    );

    let linear = linear_basis(4, 7, 0);
    assert_eq!(
        verify_quantized_price_measure_v3_smooth(&bindings_v3(), &linear, &prices, &witness),
        Err(ErrorV3::InvalidDegree)
    );
    let mode_smooth_prices = smooth_prices(&linear, &[0], &[1]);
    let smooth_witness = witness_v3(1, 4, &[0], &[1]);
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &mode_smooth_prices,
            &smooth_witness,
        ),
        Err(ErrorV3::InvalidDegree)
    );

    let out_of_domain = QuantizedAtomWitnessV3 {
        atom_coordinates: {
            let mut values = witness.atom_coordinates;
            values[2] = table.domain_max + 1;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &prices,
            &out_of_domain,
        ),
        Err(ErrorV3::AtomCoordinateOutOfRange { atom: 2 })
    );
    let padded = QuantizedAtomWitnessV3 {
        atom_coordinates: {
            let mut values = witness.atom_coordinates;
            values[3] = 1;
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(&bindings_v3(), &table, &prices, &padded,),
        Err(ErrorV3::NonCanonicalAtomPadding { atom: 3 })
    );
    let nonprimitive = QuantizedAtomWitnessV3 {
        common_denominator: 14,
        atom_masses: {
            let mut values = witness.atom_masses;
            values[..3].copy_from_slice(&[2, 4, 8]);
            values
        },
        ..witness
    };
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &prices,
            &nonprimitive,
        ),
        Err(ErrorV3::NonPrimitiveAtomScale)
    );

    let constant = constant_finite_table();
    let support_prices = finite_prices(&constant, &[2], &[1]);
    let support_witness = witness_v3(0, 3, &[2, 4, 6, 8], &[1, 1, 1, 1]);
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &constant,
            &support_prices,
            &support_witness,
        ),
        Err(ErrorV3::InvalidAtomCount)
    );

    let mut padded_price = prices;
    padded_price.prices[4] = 1;
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &table,
            &padded_price,
            &witness,
        ),
        Err(ErrorV3::NonCanonicalPricePadding { outcome: 4 })
    );

    let constant_prices = finite_prices(&constant, &[2], &[1]);
    let constant_witness = witness_v3(0, 3, &[2], &[1]);
    let mut wrong_price = constant_prices;
    wrong_price.prices[0] += 1;
    wrong_price.prices[1] -= 1;
    assert_eq!(
        verify_quantized_price_measure_v3_degree_zero(
            &bindings_v3(),
            &constant,
            &wrong_price,
            &constant_witness,
        ),
        Err(ErrorV3::PriceReconstructionMismatch { outcome: 0 })
    );

    let bounded_linear = linear_basis(2, 10, 2);
    let bounded_prices = smooth_prices(&bounded_linear, &[2], &[1]);
    let bounded_witness = witness_v3(1, 2, &[1], &[1]);
    assert_eq!(
        verify_quantized_price_measure_v3_smooth(
            &bindings_v3(),
            &bounded_linear,
            &bounded_prices,
            &bounded_witness,
        ),
        Err(ErrorV3::AtomCoordinateOutOfRange { atom: 0 })
    );
}

#[test]
fn v3_staging_is_transactional_and_full_u64_mass_scale_is_exact() {
    let basis = linear_basis(2, 1, 0);
    let coordinates = [0_u128, 2];
    let masses = [u64::MAX - 1, 1];
    let prices = smooth_prices(&basis, &coordinates, &masses);
    let witness = witness_v3(1, 2, &coordinates, &masses);
    verify_smooth_both(&basis, &prices, &witness).unwrap();

    let mut accumulator =
        QuantizedPriceMeasureAccumulatorV3::begin_smooth(&bindings_v3(), &basis, &prices, &witness)
            .unwrap();
    let initial = accumulator.clone();
    assert_eq!(
        accumulator.accumulate_atom(1),
        Err(ErrorV3::AtomCursorMismatch {
            expected: 0,
            provided: 1,
        })
    );
    assert_eq!(accumulator, initial);
    accumulator.accumulate_atom(0).unwrap();
    assert_eq!(
        accumulator.clone().finish(),
        Err(ErrorV3::IncompleteAtomAccumulation {
            cursor: 1,
            atom_count: 2,
        })
    );
    accumulator.accumulate_atom(1).unwrap();
    assert_eq!(
        accumulator.finish(),
        verify_quantized_price_measure_v3_smooth(&bindings_v3(), &basis, &prices, &witness)
    );
}

#[test]
fn frozen_v2_valid_and_low_degree_refusal_behavior_is_unchanged() {
    let basis = BasisSpec {
        outcome_count: 3,
        degree: 2,
        knot_count: 2,
        uniform_log2_spacing: 1,
        denominator: 10,
        domain_max: 2,
        edge_policy: EdgePolicy::Clamp,
        knots: {
            let mut values = [0; MAX_KNOTS];
            values[1] = 2;
            values
        },
    };
    let prices = PriceVectorV2 {
        basis_degree: 2,
        outcome_count: 3,
        price_scale: 10,
        prices: {
            let mut values = [0; MAX_OUTCOMES];
            values[0] = 10;
            values
        },
    };
    let witness = QuantizedAtomWitnessV2 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V2,
        basis_semantics: BasisSemanticsV2::QuantizedIntegerGridV1,
        price_rounding_boundary: PriceRoundingBoundaryV2::UpstreamExactSimplexV1,
        payout_rounding_boundary: PayoutRoundingBoundaryV2::LargestRemainderLowestIndexV1,
        candidate_feed: bindings_v2().candidate_feed,
        relation_domain_digest: bindings_v2().relation_domain_digest,
        basis_digest: bindings_v2().basis_digest,
        candidate_price_digest: bindings_v2().candidate_price_digest,
        body_digest: bindings_v2().observed_body_digest,
        atom_count: 1,
        common_denominator: 1,
        atom_coordinates: [0; MAX_QUANTIZED_ATOMS],
        atom_masses: {
            let mut values = [0; MAX_QUANTIZED_ATOMS];
            values[0] = 1;
            values
        },
    };
    assert_eq!(
        verify_quantized_price_measure_v2(&bindings_v2(), &basis, &prices, &witness),
        Ok(VerifiedPriceMeasureV2 {
            basis_degree: 2,
            outcome_count: 3,
            span_count: 1,
            common_denominator: 1,
            body_digest: [5; 32],
        })
    );
    assert_eq!(
        verify_quantized_price_measure_v2(
            &bindings_v2(),
            &basis,
            &prices,
            &QuantizedAtomWitnessV2 {
                schema_version: PRICE_MEASURE_WITNESS_VERSION_V3,
                ..witness
            },
        ),
        Err(ErrorV2::UnsupportedSchemaVersion)
    );
    for degree in [0_u8, 1] {
        let low_basis = if degree == 0 {
            BasisSpec {
                outcome_count: 3,
                degree: 0,
                knot_count: 2,
                uniform_log2_spacing: UNIFORM_SPACING_NONE,
                denominator: 10,
                domain_max: 6,
                edge_policy: EdgePolicy::Clamp,
                knots: {
                    let mut values = [0; MAX_KNOTS];
                    values[..2].copy_from_slice(&[2, 4]);
                    values
                },
            }
        } else {
            linear_basis(3, 10, 0)
        };
        assert_eq!(
            verify_quantized_price_measure_v2(
                &bindings_v2(),
                &low_basis,
                &PriceVectorV2 {
                    basis_degree: degree,
                    ..prices
                },
                &witness,
            ),
            Err(ErrorV2::InvalidDegree)
        );
    }
}
