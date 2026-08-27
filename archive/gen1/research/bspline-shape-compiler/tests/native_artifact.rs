use clutch_accumulator::MAX_VALUE;
use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_bspline_shape_compiler::{
    artifact::{
        basis_spec_from_terms_v1, build_market_creation_artifacts_v1, decode_basis_spec_v1,
        digest_basis_spec_v1, encode_basis_spec_v1, render_cross_language_fixture_v1,
        ArtifactError, NativeShapeCertificateV1, BASIS_SPEC_BYTES_V1, CERTIFICATE_FIXED_BYTES_V1,
        CERTIFICATE_MAGIC_V1, SEMANTIC_NATIVE_BSPLINE, WEIGHT_ROUND_VERSION_V1,
    },
    Shape,
};
use clutch_solana_layout::{
    Hash32, Intent, PayoutVectorBytes, TermsAccount, MAX_INTENT_BYTES, MAX_OUTCOMES, MAX_PAYOUTS,
    PAYOUT_MAP_UNUSED,
};
use sha2::{Digest, Sha256};

fn basis(degree: u8, denominator: u64) -> BasisSpec {
    let (outcome_count, knot_count, active) = match degree {
        0 => (3, 2, &[4_u128, 8][..]),
        1 => (3, 3, &[0_u128, 4, 8][..]),
        2 => (4, 3, &[0_u128, 4, 8][..]),
        3 => (5, 3, &[0_u128, 4, 8][..]),
        _ => unreachable!(),
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..active.len()].copy_from_slice(active);
    BasisSpec {
        outcome_count,
        degree,
        knot_count,
        uniform_log2_spacing: if degree >= 2 { 2 } else { UNIFORM_SPACING_NONE },
        denominator,
        domain_max: if degree == 0 { 12 } else { 8 },
        edge_policy: EdgePolicy::Clamp,
        knots,
    }
}

fn shape(degree: u8) -> Shape {
    match degree {
        0 => Shape::HardRange {
            low: 4,
            high: 8,
            height: 8,
        },
        1 => Shape::Triangle {
            left: 0,
            peak: 4,
            right: 8,
            height: 8,
        },
        2 | 3 => Shape::CappedCall {
            low: 0,
            high: 8,
            height: 8,
        },
        _ => unreachable!(),
    }
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32]).unwrap()
}

fn market_terms(degree: u8) -> TermsAccount {
    let (outcome_count, knot_count, active) = match degree {
        0 => (3, 2, &[MAX_VALUE / 3, (MAX_VALUE / 3) * 2][..]),
        1 => (2, 2, &[0, MAX_VALUE][..]),
        2 => (3, 2, &[0, 1_u128 << 32][..]),
        3 => (4, 2, &[0, 1_u128 << 32][..]),
        _ => unreachable!(),
    };
    let denominator = 8;
    let mut weights = [0_u64; MAX_OUTCOMES];
    weights[0] = denominator;
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    payouts[0] = PayoutVectorBytes {
        denominator,
        weights,
    };
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    if degree == 0 {
        payout_map[..usize::from(outcome_count)].fill(0);
    }
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..active.len()].copy_from_slice(active);
    let mut terms = TermsAccount {
        terms: Hash32::ZERO,
        realm: hash(1),
        profile: hash(2),
        feed: hash(3),
        price_grid: hash(4),
        outcome_count,
        payout_count: 1,
        payouts,
        grid_family_id: 1,
        grid_version: 1,
        bucket_seconds: 60,
        expected_start_bucket: 10,
        expected_end_bucket_exclusive: 20,
        maturity_horizon_buckets: 10,
        coverage_policy_id: 1,
        repair_policy_id: 1,
        failure_policy_id: 1,
        statistic_id: 1,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: degree,
        knot_count,
        uniform_log2_spacing: if degree >= 2 {
            32
        } else {
            UNIFORM_SPACING_NONE
        },
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 1,
        evaluator_version: 1,
        source_adapter_id: hash(5),
        payout_map,
        knots,
        collateral_cap: 1_000_000,
        stored_bump: 7,
        flags: 0,
    };
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms.validate().unwrap();
    terms
}

#[test]
fn basis_codec_roundtrips_degrees_zero_through_three_without_lowering() {
    for degree in 0..=3 {
        let spec = basis(degree, 257);
        let bytes = encode_basis_spec_v1(&spec).unwrap();
        assert_eq!(bytes.len(), BASIS_SPEC_BYTES_V1);
        assert_eq!(&bytes[..8], b"DCBASV01");
        assert_eq!(bytes[12], SEMANTIC_NATIVE_BSPLINE);
        assert_eq!(decode_basis_spec_v1(&bytes).unwrap(), spec);
        assert_ne!(digest_basis_spec_v1(&bytes), [0; 32]);
    }
}

#[test]
fn basis_digest_and_bytes_are_golden_and_domain_separated() {
    let spec = basis(2, 2);
    let bytes = encode_basis_spec_v1(&spec).unwrap();
    assert_eq!(&bytes[0..8], b"DCBASV01");
    assert_eq!(&bytes[8..12], &[1, 0, 1, 0]);
    assert_eq!(&bytes[12..18], &[1, 4, 2, 3, 2, 1]);
    assert_eq!(&bytes[18..24], &[0; 6]);
    assert_eq!(&bytes[24..32], &2_u64.to_le_bytes());
    assert_eq!(&bytes[32..48], &8_u128.to_le_bytes());
    assert_eq!(&bytes[48..64], &0_u128.to_le_bytes());
    assert_eq!(&bytes[64..80], &4_u128.to_le_bytes());
    assert_eq!(&bytes[80..96], &8_u128.to_le_bytes());
    assert!(bytes[96..].iter().all(|byte| *byte == 0));
    assert_eq!(
        hex(&digest_basis_spec_v1(&bytes)),
        "5a386d58164af5dc9759fd14e4fd24742e71072fd2e9a4598d65853a83004ee6"
    );
    let mut plain = Sha256::new();
    plain.update(bytes);
    assert_ne!(
        digest_basis_spec_v1(&bytes),
        <[u8; 32]>::from(plain.finalize())
    );
}

#[test]
fn basis_codec_refuses_hostile_headers_padding_counts_and_lengths() {
    let canonical = encode_basis_spec_v1(&basis(2, 2)).unwrap();
    for (offset, replacement) in [
        (0, b'X'),
        (8, 2),
        (12, 2),
        (13, 1),
        (14, 4),
        (15, 2),
        (16, 3),
        (17, 9),
        (18, 1),
        (24, 0),
        (32, 3),
        (64, 0),
    ] {
        let mut mutant = canonical;
        mutant[offset] = replacement;
        assert!(decode_basis_spec_v1(&mutant).is_err(), "offset {offset}");
    }
    let mut reserved = canonical;
    reserved[20] = 1;
    assert_eq!(
        decode_basis_spec_v1(&reserved),
        Err(ArtifactError::NonCanonicalPadding)
    );
    assert_eq!(
        decode_basis_spec_v1(&canonical[..canonical.len() - 1]),
        Err(ArtifactError::Truncated)
    );
    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_basis_spec_v1(&trailing),
        Err(ArtifactError::TrailingBytes)
    );
}

#[test]
fn certificate_roundtrips_and_recompiles_every_native_degree() {
    for degree in 0..=3 {
        let value =
            NativeShapeCertificateV1::compile([degree + 1; 32], basis(degree, 257), shape(degree))
                .unwrap();
        let bytes = value.encode().unwrap();
        assert_eq!(&bytes[..8], &CERTIFICATE_MAGIC_V1);
        assert_eq!(bytes[16], SEMANTIC_NATIVE_BSPLINE);
        assert_eq!(NativeShapeCertificateV1::decode(&bytes).unwrap(), value);
        assert_ne!(value.digest().unwrap(), [0; 32]);
    }
}

#[test]
fn certificate_roundtrips_all_seven_shape_tags() {
    let basis = basis(1, 257);
    for shape in [
        Shape::HardRange {
            low: 2,
            high: 6,
            height: 8,
        },
        Shape::UpperTail {
            strike: 4,
            height: 8,
        },
        Shape::LowerTail {
            strike: 4,
            height: 8,
        },
        Shape::Triangle {
            left: 0,
            peak: 4,
            right: 8,
            height: 8,
        },
        Shape::CappedCall {
            low: 2,
            high: 6,
            height: 8,
        },
        Shape::CappedPut {
            low: 2,
            high: 6,
            height: 8,
        },
        Shape::Gaussian {
            center: 4,
            sigma: 1,
            height: 8,
        },
    ] {
        let certificate = NativeShapeCertificateV1::compile([11; 32], basis, shape).unwrap();
        let bytes = certificate.encode().unwrap();
        assert_eq!(
            NativeShapeCertificateV1::decode(&bytes).unwrap(),
            certificate
        );
    }
}

#[test]
fn certificate_refuses_digest_shape_compilation_and_rational_malleability() {
    let value = NativeShapeCertificateV1::compile([9; 32], basis(1, 257), shape(1)).unwrap();
    let canonical = value.encode().unwrap();

    let mut zero_terms = canonical.clone();
    zero_terms[20..52].fill(0);
    assert_eq!(
        NativeShapeCertificateV1::decode(&zero_terms),
        Err(ArtifactError::DigestMismatch)
    );

    let mut wrong_basis_digest = canonical.clone();
    wrong_basis_digest[52] ^= 1;
    assert_eq!(
        NativeShapeCertificateV1::decode(&wrong_basis_digest),
        Err(ArtifactError::DigestMismatch)
    );

    for offset in [8, 10, 12, 14] {
        let mut wrong_version = canonical.clone();
        wrong_version[offset] = 2;
        assert_eq!(
            NativeShapeCertificateV1::decode(&wrong_version),
            Err(ArtifactError::InvalidDiscriminant),
            "version offset {offset}"
        );
    }

    let mut lowered_tag = canonical.clone();
    lowered_tag[16] = 2;
    assert_eq!(
        NativeShapeCertificateV1::decode(&lowered_tag),
        Err(ArtifactError::InvalidDiscriminant)
    );

    let mut wrong_status = canonical.clone();
    wrong_status[17] = 2;
    assert_eq!(
        NativeShapeCertificateV1::decode(&wrong_status),
        Err(ArtifactError::CertificateMismatch)
    );

    let mut wrong_construction = canonical.clone();
    wrong_construction[18] = 3;
    assert_eq!(
        NativeShapeCertificateV1::decode(&wrong_construction),
        Err(ArtifactError::CertificateMismatch)
    );

    let mut wrong_depth = canonical.clone();
    wrong_depth[19] = wrong_depth[19].wrapping_add(1);
    assert_eq!(
        NativeShapeCertificateV1::decode(&wrong_depth),
        Err(ArtifactError::CertificateMismatch)
    );

    let mut shape_padding = canonical.clone();
    let shape_offset = 84 + BASIS_SPEC_BYTES_V1;
    shape_padding[shape_offset + 1] = 1;
    assert_eq!(
        NativeShapeCertificateV1::decode(&shape_padding),
        Err(ArtifactError::NonCanonicalPadding)
    );

    // The first coefficient is zero: canonical `0/1` is lengths 0,1 and byte
    // 1.  Inserting a leading zero in the denominator must not create a second
    // representation of the same rational.
    let first_rational = CERTIFICATE_FIXED_BYTES_V1;
    assert_eq!(
        &canonical[first_rational..first_rational + 5],
        &[0, 0, 1, 0, 1]
    );
    let mut leading_zero = canonical.clone();
    leading_zero[first_rational + 2..first_rational + 4].copy_from_slice(&2_u16.to_le_bytes());
    leading_zero.insert(first_rational + 4, 0);
    assert_eq!(
        NativeShapeCertificateV1::decode(&leading_zero),
        Err(ArtifactError::NonCanonicalRational)
    );

    // The second triangle coefficient is 8/1.  16/2 is mathematically equal
    // but is not the canonical reduced encoding.
    let second_rational = first_rational + 5;
    assert_eq!(
        &canonical[second_rational..second_rational + 6],
        &[1, 0, 1, 0, 8, 1]
    );
    let mut unreduced = canonical.clone();
    unreduced[second_rational + 4] = 16;
    unreduced[second_rational + 5] = 2;
    assert_eq!(
        NativeShapeCertificateV1::decode(&unreduced),
        Err(ArtifactError::NonCanonicalRational)
    );

    let mut zero_denominator = canonical.clone();
    zero_denominator[first_rational + 4] = 0;
    assert_eq!(
        NativeShapeCertificateV1::decode(&zero_denominator),
        Err(ArtifactError::NonCanonicalRational)
    );

    let mut overwide_rational = canonical.clone();
    overwide_rational[first_rational..first_rational + 2].copy_from_slice(&4097_u16.to_le_bytes());
    assert_eq!(
        NativeShapeCertificateV1::decode(&overwide_rational),
        Err(ArtifactError::InvalidLength)
    );

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(
        NativeShapeCertificateV1::decode(&trailing),
        Err(ArtifactError::TrailingBytes)
    );
    assert_eq!(
        NativeShapeCertificateV1::decode(&canonical[..canonical.len() - 1]),
        Err(ArtifactError::Truncated)
    );
    assert_eq!(
        NativeShapeCertificateV1::decode(&vec![0; 256 * 1024 + 1]),
        Err(ArtifactError::InvalidLength)
    );
}

#[test]
fn weight_round_version_pins_largest_remainder_and_low_index_ties() {
    assert_eq!(WEIGHT_ROUND_VERSION_V1, 1);
    let quadratic = basis(2, 2);
    assert_eq!(&quadratic.evaluate(2).unwrap().weights[..4], &[1, 1, 0, 0]);

    let mut version_mutant = NativeShapeCertificateV1::compile(
        [7; 32],
        quadratic,
        Shape::CappedCall {
            low: 0,
            high: 8,
            height: 8,
        },
    )
    .unwrap()
    .encode()
    .unwrap();
    version_mutant[14..16].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        NativeShapeCertificateV1::decode(&version_mutant),
        Err(ArtifactError::InvalidDiscriminant)
    );
}

#[test]
fn terms_projection_and_market_intents_use_the_frozen_layout_codec() {
    for degree in 0..=3 {
        let terms = market_terms(degree);
        let projected = basis_spec_from_terms_v1(&terms).unwrap();
        assert_eq!(projected.degree, degree);
        assert_eq!(projected.domain_max, MAX_VALUE);
        assert_eq!(projected.knots, terms.knots);

        let shape = if degree == 0 {
            Shape::UpperTail {
                strike: terms.knots[0],
                height: 8,
            }
        } else {
            Shape::CappedCall {
                low: terms.knots[0],
                high: terms.knots[usize::from(terms.knot_count) - 1],
                height: 8,
            }
        };
        let bundle = build_market_creation_artifacts_v1(&terms, 42, 1_000, 2_000, shape).unwrap();
        assert_eq!(bundle.terms_digest, terms.terms.0);
        assert_eq!(
            bundle.basis_spec_digest,
            digest_basis_spec_v1(&bundle.basis_spec_bytes)
        );
        assert_eq!(
            NativeShapeCertificateV1::decode(&bundle.shape_certificate_bytes)
                .unwrap()
                .terms_digest,
            terms.terms.0
        );

        let mut scratch = [0_u8; MAX_INTENT_BYTES];
        let begin = Intent::decode(&bundle.terms_upload.begin_intent).unwrap();
        assert_eq!(
            begin,
            Intent::BeginArtifact {
                kind: clutch_solana_layout::artifact::ArtifactKind::Terms,
                context: terms.realm,
                digest: terms.terms,
                exact_len: 1656,
                expires_slot: 2_000,
            }
        );
        assert_eq!(bundle.terms_upload.write_intents.len(), 9);
        let mut reconstructed = Vec::new();
        for (index, encoded) in bundle.terms_upload.write_intents.iter().enumerate() {
            let Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor,
                chunk_len,
                chunk,
            } = Intent::decode(encoded).unwrap()
            else {
                panic!("write {index} decoded as the wrong intent");
            };
            assert_eq!(kind, clutch_solana_layout::artifact::ArtifactKind::Terms);
            assert_eq!(context, terms.realm);
            assert_eq!(digest, terms.terms);
            assert_eq!(usize::from(cursor), index * 192);
            assert_eq!(usize::from(chunk_len), if index == 8 { 120 } else { 192 });
            assert!(chunk[usize::from(chunk_len)..]
                .iter()
                .all(|byte| *byte == 0));
            reconstructed.extend_from_slice(&chunk[..usize::from(chunk_len)]);
        }
        assert_eq!(reconstructed, bundle.terms_account_bytes);
        assert_eq!(
            Intent::decode(&bundle.terms_upload.seal_intent).unwrap(),
            Intent::SealArtifact {
                kind: clutch_solana_layout::artifact::ArtifactKind::Terms,
                context: terms.realm,
                digest: terms.terms,
                exact_len: 1656,
            }
        );
        let expected_create = Intent::CreateMarket {
            realm: terms.realm,
            profile: terms.profile,
            market_nonce: 42,
            outcome_count: terms.outcome_count,
            terms: terms.terms,
            feed: terms.feed,
        };
        let written = expected_create.encode(&mut scratch).unwrap();
        assert_eq!(bundle.create_market_intent, scratch[..written]);
        assert_eq!(
            Intent::decode(&bundle.create_market_intent).unwrap(),
            expected_create
        );
    }
}

#[test]
fn terms_upload_plan_refuses_expiry_and_codec_mutants() {
    let terms = market_terms(1);
    let shape = Shape::CappedCall {
        low: 0,
        high: MAX_VALUE,
        height: 8,
    };
    for (current, expiry) in [(100, 99), (100, 100), (100, 107), (100, 432_101)] {
        assert_eq!(
            build_market_creation_artifacts_v1(&terms, 42, current, expiry, shape),
            Err(ArtifactError::InvalidLength)
        );
    }
    for expiry in [108, 432_100] {
        build_market_creation_artifacts_v1(&terms, 42, 100, expiry, shape).unwrap();
    }

    let bundle = build_market_creation_artifacts_v1(&terms, 42, 100, 200, shape).unwrap();
    let mut wrong_cursor = bundle.terms_upload.write_intents[1].clone();
    wrong_cursor[67..69].copy_from_slice(&0_u16.to_le_bytes());
    let mut wrong_cursor_plan = bundle.terms_upload.clone();
    wrong_cursor_plan.write_intents[1] = wrong_cursor;
    assert_eq!(
        wrong_cursor_plan.verify_and_reconstruct(&terms),
        Err(ArtifactError::UploadPlanMismatch)
    );

    let mut wrong_length_plan = bundle.terms_upload.clone();
    wrong_length_plan.write_intents[0][69..71].copy_from_slice(&191_u16.to_le_bytes());
    assert_eq!(
        wrong_length_plan.verify_and_reconstruct(&terms),
        Err(ArtifactError::UploadPlanMismatch)
    );

    let mut wrong_padding = bundle.terms_upload.write_intents[8].clone();
    *wrong_padding.last_mut().unwrap() = 1;
    assert_eq!(
        {
            let mut plan = bundle.terms_upload.clone();
            plan.write_intents[8] = wrong_padding;
            plan.verify_and_reconstruct(&terms)
        },
        Err(ArtifactError::Layout(
            clutch_solana_layout::CodecError::NonCanonicalPadding
        ))
    );

    for offset in [3, 35] {
        let mut wrong_binding = bundle.terms_upload.write_intents[0].clone();
        wrong_binding[offset] ^= 1;
        let mut plan = bundle.terms_upload.clone();
        plan.write_intents[0] = wrong_binding;
        assert_eq!(
            plan.verify_and_reconstruct(&terms),
            Err(ArtifactError::UploadPlanMismatch)
        );
    }

    for offset in [3, 35, 67, 69] {
        let mut plan = bundle.terms_upload.clone();
        plan.begin_intent[offset] ^= 1;
        assert!(plan.verify_and_reconstruct(&terms).is_err());
    }
    for offset in [3, 35, 67] {
        let mut plan = bundle.terms_upload.clone();
        plan.seal_intent[offset] ^= 1;
        assert!(plan.verify_and_reconstruct(&terms).is_err());
    }
}

#[test]
fn checked_cross_language_fixture_equals_the_rust_renderer_byte_for_byte() {
    let terms = market_terms(1);
    let artifacts = build_market_creation_artifacts_v1(
        &terms,
        42,
        1_000,
        2_000,
        Shape::CappedCall {
            low: 0,
            high: MAX_VALUE,
            height: 8,
        },
    )
    .unwrap();
    let rendered = render_cross_language_fixture_v1(&artifacts, 42);
    assert_eq!(
        include_str!("../fixtures/native-v1-degree1.json"),
        format!("{rendered}\n")
    );
}

#[test]
fn certificate_terms_binding_refuses_each_independent_native_identity() {
    let terms = market_terms(1);
    let basis = basis_spec_from_terms_v1(&terms).unwrap();
    let certificate = NativeShapeCertificateV1::compile(
        terms.terms.0,
        basis,
        Shape::CappedCall {
            low: 0,
            high: MAX_VALUE,
            height: 8,
        },
    )
    .unwrap();
    certificate.verify_terms(&terms).unwrap();

    let other_degree = market_terms(2);
    assert_eq!(
        certificate.verify_terms(&other_degree),
        Err(ArtifactError::TermsBasisMismatch)
    );

    let mut other_digest = terms;
    other_digest.source_adapter_id = hash(99);
    other_digest.terms = other_digest.recomputed_terms_digest().unwrap();
    other_digest.validate().unwrap();
    assert_eq!(
        certificate.verify_terms(&other_digest),
        Err(ArtifactError::TermsBasisMismatch)
    );

    let mut other_edge = terms;
    other_edge.edge_policy_id = 2;
    other_edge.terms = other_edge.recomputed_terms_digest().unwrap();
    other_edge.validate().unwrap();
    assert_eq!(
        certificate.verify_terms(&other_edge),
        Err(ArtifactError::TermsBasisMismatch)
    );

    let mut other_knot = terms;
    other_knot.knots[1] -= 1;
    other_knot.terms = other_knot.recomputed_terms_digest().unwrap();
    other_knot.validate().unwrap();
    assert_eq!(
        certificate.verify_terms(&other_knot),
        Err(ArtifactError::TermsBasisMismatch)
    );
}

#[test]
fn terms_projection_refuses_noncanonical_edge_policy_for_degree_zero() {
    let mut terms = market_terms(0);
    terms.edge_policy_id = 2;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    terms.validate().unwrap();
    assert_eq!(
        basis_spec_from_terms_v1(&terms),
        Err(ArtifactError::TermsBasisMismatch)
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
