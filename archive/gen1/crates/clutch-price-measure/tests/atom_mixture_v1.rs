use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS};
use clutch_price_measure::{
    verify_quantized_atom_mixture_v1, BoundQuantizedSplineV1, ErrorV1, IdentityFieldV1,
    QuantizedAtomMixtureBindingsV1, QuantizedAtomMixtureCertificateV1,
    QuantizedPayoutPriceVectorV1, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1,
};

fn bindings() -> QuantizedAtomMixtureBindingsV1 {
    QuantizedAtomMixtureBindingsV1 {
        market_id: [1; 32],
        terms_id: [2; 32],
        basis_id: [3; 32],
        price_id: [4; 32],
    }
}

fn basis(degree: u8, edge_policy: EdgePolicy) -> BasisSpec {
    let outcome_count = 4_u8;
    let knot_count = outcome_count + 1 - degree;
    let mut knots = [0_u128; MAX_KNOTS];
    let mut knot = 0_u8;
    while knot < knot_count {
        knots[usize::from(knot)] = u128::from(knot) * 2;
        knot += 1;
    }
    BasisSpec {
        outcome_count,
        degree,
        knot_count,
        uniform_log2_spacing: 1,
        denominator: 12,
        domain_max: u128::from(knot_count - 1) * 2,
        edge_policy,
        knots,
    }
}

fn bound(degree: u8) -> BoundQuantizedSplineV1 {
    let basis = basis(degree, EdgePolicy::Clamp);
    BoundQuantizedSplineV1 {
        bindings: bindings(),
        coordinate_domain_min: 0,
        coordinate_domain_max: basis.domain_max,
        basis,
    }
}

fn certificate(
    degree: u8,
    coordinates: &[u128],
    weights_in: &[u64],
) -> QuantizedAtomMixtureCertificateV1 {
    let mut observation_coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0_u64; MAX_QUANTIZED_ATOMS];
    observation_coordinates[..coordinates.len()].copy_from_slice(coordinates);
    weights[..weights_in.len()].copy_from_slice(weights_in);
    QuantizedAtomMixtureCertificateV1::new(
        bindings(),
        degree,
        4,
        12,
        weights_in.iter().copied().sum(),
        u8::try_from(coordinates.len()).unwrap(),
        observation_coordinates,
        weights,
    )
    .unwrap()
}

fn exact_prices(
    bound: &BoundQuantizedSplineV1,
    certificate: &QuantizedAtomMixtureCertificateV1,
) -> QuantizedPayoutPriceVectorV1 {
    let mut numerators = [0_u128; MAX_OUTCOMES];
    let mut witness = 0_usize;
    while witness < usize::from(certificate.witness_count) {
        let atom = bound
            .basis
            .evaluate(certificate.observation_coordinates[witness])
            .unwrap();
        let mut outcome = 0_usize;
        while outcome < usize::from(bound.basis.outcome_count) {
            numerators[outcome] +=
                u128::from(certificate.weights[witness]) * u128::from(atom.weights[outcome]);
            outcome += 1;
        }
        witness += 1;
    }
    let mut prices = [0_u64; MAX_OUTCOMES];
    let mut outcome = 0_usize;
    while outcome < usize::from(bound.basis.outcome_count) {
        let quotient = numerators[outcome] / u128::from(certificate.weight_denominator);
        assert_eq!(
            quotient * u128::from(certificate.weight_denominator),
            numerators[outcome]
        );
        prices[outcome] = u64::try_from(quotient).unwrap();
        outcome += 1;
    }
    QuantizedPayoutPriceVectorV1 {
        price_id: bindings().price_id,
        outcome_count: bound.basis.outcome_count,
        prices,
    }
}

fn encoded(
    certificate: QuantizedAtomMixtureCertificateV1,
) -> [u8; QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1] {
    let mut bytes = [0_u8; QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1];
    certificate.encode_into(&mut bytes).unwrap();
    bytes
}

#[test]
fn degree_two_and_three_exact_atoms_are_positive_certificates() {
    for degree in [2, 3] {
        let bound = bound(degree);
        let certificate = certificate(degree, &[1], &[1]);
        let prices = exact_prices(&bound, &certificate);
        let verified = verify_quantized_atom_mixture_v1(&bound, &prices, &certificate).unwrap();
        assert_eq!(verified.bindings(), bindings());
        assert_eq!(verified.basis_degree(), degree);
        assert_eq!(verified.outcome_count(), 4);
        assert_eq!(verified.witness_count(), 1);
        assert_eq!(verified.payout_denominator(), 12);
        assert_eq!(verified.weight_denominator(), 1);
    }
}

#[test]
fn endpoint_mixture_uses_direct_denominator_scale_equations() {
    for degree in [2, 3] {
        let bound = bound(degree);
        let certificate = certificate(degree, &[0, bound.coordinate_domain_max], &[1, 1]);
        let prices = exact_prices(&bound, &certificate);
        assert_eq!(
            prices.prices,
            [6, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        verify_quantized_atom_mixture_v1(&bound, &prices, &certificate).unwrap();
    }
}

#[test]
fn fixed_codec_round_trips_and_refuses_hostile_headers() {
    let certificate = certificate(2, &[1], &[1]);
    let bytes = encoded(certificate);
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bytes),
        Ok(certificate)
    );
    assert_eq!(
        core::mem::size_of::<QuantizedAtomMixtureCertificateV1>(),
        544
    );

    for length in [0, 543] {
        assert_eq!(
            QuantizedAtomMixtureCertificateV1::decode(&bytes[..length]),
            Err(ErrorV1::InvalidEncodedLength)
        );
    }
    let mut bad = bytes;
    bad[0] ^= 1;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bad),
        Err(ErrorV1::InvalidMagic)
    );
    let mut bad = bytes;
    bad[8] = 2;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bad),
        Err(ErrorV1::UnsupportedSchemaVersion)
    );
    let mut bad = bytes;
    bad[9] = 2;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bad),
        Err(ErrorV1::UnsupportedSemanticsVersion)
    );
    let mut bad = bytes;
    bad[10] = 2;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bad),
        Err(ErrorV1::UnsupportedCaratheodoryProfile)
    );
    let mut bad = bytes;
    bad[14] = 1;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::decode(&bad),
        Err(ErrorV1::NonCanonicalReserved)
    );
}

#[test]
fn caratheodory_profile_refuses_support_above_affine_bound() {
    let mut coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0_u64; MAX_QUANTIZED_ATOMS];
    coordinates[..4].copy_from_slice(&[1, 2, 3, 4]);
    weights[..4].fill(1);
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::new(bindings(), 2, 3, 12, 4, 4, coordinates, weights,),
        Err(ErrorV1::InvalidWitnessCount)
    );
}

#[test]
fn sparse_support_is_sorted_positive_primitive_and_zero_padded() {
    let mut coordinates = [0_u128; MAX_QUANTIZED_ATOMS];
    let mut weights = [0_u64; MAX_QUANTIZED_ATOMS];
    coordinates[..2].copy_from_slice(&[2, 2]);
    weights[..2].copy_from_slice(&[1, 1]);
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::new(bindings(), 2, 4, 12, 2, 2, coordinates, weights),
        Err(ErrorV1::NonCanonicalObservationOrder { witness: 1 })
    );

    coordinates[..2].copy_from_slice(&[1, 2]);
    weights[..2].copy_from_slice(&[2, 0]);
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::new(bindings(), 2, 4, 12, 2, 2, coordinates, weights),
        Err(ErrorV1::ZeroWitnessWeight { witness: 1 })
    );

    weights[..2].copy_from_slice(&[2, 2]);
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::new(bindings(), 2, 4, 12, 4, 2, coordinates, weights),
        Err(ErrorV1::NonPrimitiveWeightScale)
    );

    weights[..2].copy_from_slice(&[1, 1]);
    coordinates[2] = 9;
    assert_eq!(
        QuantizedAtomMixtureCertificateV1::new(bindings(), 2, 4, 12, 2, 2, coordinates, weights),
        Err(ErrorV1::NonCanonicalWitnessPadding { witness: 2 })
    );
}

#[test]
fn exact_market_terms_basis_and_price_ids_are_all_bound() {
    let bound = bound(2);
    let certificate = certificate(2, &[1], &[1]);
    let prices = exact_prices(&bound, &certificate);
    for field in [
        IdentityFieldV1::Market,
        IdentityFieldV1::Terms,
        IdentityFieldV1::Basis,
        IdentityFieldV1::Price,
    ] {
        let mut changed = bound;
        match field {
            IdentityFieldV1::Market => changed.bindings.market_id = [9; 32],
            IdentityFieldV1::Terms => changed.bindings.terms_id = [9; 32],
            IdentityFieldV1::Basis => changed.bindings.basis_id = [9; 32],
            IdentityFieldV1::Price => changed.bindings.price_id = [9; 32],
        }
        assert_eq!(
            verify_quantized_atom_mixture_v1(&changed, &prices, &certificate),
            Err(ErrorV1::BindingMismatch { field })
        );
    }
}

#[test]
fn exact_knots_domain_denominator_and_price_equations_are_behavioral() {
    let bound = bound(2);
    let certificate = certificate(2, &[1], &[1]);
    let prices = exact_prices(&bound, &certificate);

    let mut changed_knots = bound;
    changed_knots.basis.knots[1] = 3;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&changed_knots, &prices, &certificate),
        Err(ErrorV1::InvalidBasis)
    );

    let mut changed_domain = bound;
    changed_domain.coordinate_domain_max += 1;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&changed_domain, &prices, &certificate),
        Err(ErrorV1::InvalidTermsDomain)
    );

    let mut changed_denominator = bound;
    changed_denominator.basis.denominator = 13;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&changed_denominator, &prices, &certificate),
        Err(ErrorV1::CertificateBasisMismatch)
    );

    let mut changed_price = prices;
    changed_price.prices[0] += 1;
    changed_price.prices[1] -= 1;
    assert!(matches!(
        verify_quantized_atom_mixture_v1(&bound, &changed_price, &certificate),
        Err(ErrorV1::PriceReconstructionMismatch { .. })
    ));
}

#[test]
fn full_terms_domain_controls_observations_and_refusing_totality() {
    let mut clamping = bound(2);
    clamping.coordinate_domain_min = 0;
    clamping.coordinate_domain_max = 6;
    clamping.basis.domain_max = 6;
    let certificate = certificate(2, &[5], &[1]);
    let prices = exact_prices(&clamping, &certificate);
    verify_quantized_atom_mixture_v1(&clamping, &prices, &certificate).unwrap();

    let mut refusing = clamping;
    refusing.basis.edge_policy = EdgePolicy::Refuse;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&refusing, &prices, &certificate),
        Err(ErrorV1::IncompleteRefusingDomain)
    );

    let ordinary = bound(2);
    let out_of_domain = certificate(2, &[5], &[1]);
    let endpoint_prices = QuantizedPayoutPriceVectorV1 {
        price_id: bindings().price_id,
        outcome_count: 4,
        prices: [0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    assert_eq!(
        verify_quantized_atom_mixture_v1(&ordinary, &endpoint_prices, &out_of_domain),
        Err(ErrorV1::ObservationOutOfDomain { witness: 0 })
    );
}

#[test]
fn price_simplex_and_padding_are_hostile_checked() {
    let bound = bound(3);
    let certificate = certificate(3, &[1], &[1]);
    let prices = exact_prices(&bound, &certificate);

    let mut wrong_sum = prices;
    wrong_sum.prices[0] += 1;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&bound, &wrong_sum, &certificate),
        Err(ErrorV1::PriceSimplexMismatch)
    );
    let mut padded = prices;
    padded.prices[4] = 1;
    assert_eq!(
        verify_quantized_atom_mixture_v1(&bound, &padded, &certificate),
        Err(ErrorV1::NonCanonicalPricePadding { outcome: 4 })
    );
    let mut wrong_id = prices;
    wrong_id.price_id = [8; 32];
    assert_eq!(
        verify_quantized_atom_mixture_v1(&bound, &wrong_id, &certificate),
        Err(ErrorV1::PriceBindingMismatch)
    );
}
