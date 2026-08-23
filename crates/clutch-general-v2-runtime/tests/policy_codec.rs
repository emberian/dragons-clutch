// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_general_v2_contract::Id32;
use clutch_general_v2_runtime::{
    relation_v2_policy_id_v1, score_v2_q_policy_id_v1, GeneralV2RuntimeError,
    QuantizedWitnessBodyV1, QUANTIZED_WITNESS_BODY_BYTES_V1, RELATION_V2_POLICY_BODY_V1,
    SCORE_V2_Q_POLICY_BODY_V1,
};
use clutch_price_measure::{
    PRICE_MEASURE_WITNESS_VERSION_V3, QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};

fn id(byte: u8) -> Id32 {
    Id32::new([byte; 32]).expect("nonzero fixture identity")
}

fn body() -> QuantizedWitnessBodyV1 {
    let mut atom_coordinates = [0u128; 16];
    atom_coordinates[0] = 4;
    atom_coordinates[1] = 8;
    let mut atom_masses = [0u64; 16];
    atom_masses[0] = 1;
    atom_masses[1] = 2;
    QuantizedWitnessBodyV1 {
        schema_version: PRICE_MEASURE_WITNESS_VERSION_V3,
        quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
        candidate_feed: id(1),
        relation_domain_digest: id(2),
        basis_digest: id(3),
        candidate_price_digest: id(4),
        basis_degree: 2,
        outcome_count: 4,
        atom_count: 2,
        common_denominator: 3,
        atom_coordinates,
        atom_masses,
    }
}

#[test]
fn canonical_policy_ids_are_live_distinct_and_semantically_pinned() {
    let relation = relation_v2_policy_id_v1().expect("fixed relation policy hashes");
    let score = score_v2_q_policy_id_v1().expect("fixed score policy hashes");
    assert!(!relation.is_zero());
    assert!(!score.is_zero());
    assert_ne!(relation, score);

    assert_eq!(&RELATION_V2_POLICY_BODY_V1[..8], b"DCRELV2\0");
    assert_eq!(RELATION_V2_POLICY_BODY_V1[9], 2);
    assert_eq!(RELATION_V2_POLICY_BODY_V1[10], 0);
    assert_eq!(RELATION_V2_POLICY_BODY_V1[11], 16);
    assert_eq!(RELATION_V2_POLICY_BODY_V1[12], 64);
    assert_eq!(&SCORE_V2_Q_POLICY_BODY_V1[..8], b"DCSV2Q1\0");
    assert_eq!(SCORE_V2_Q_POLICY_BODY_V1[9], 2);
    assert_eq!(&SCORE_V2_Q_POLICY_BODY_V1[10..], &[0; 6]);
}

#[test]
fn witness_codec_is_fixed_width_and_every_binding_changes_its_identity() {
    let original = body();
    let encoded = original.encode().expect("canonical body encodes");
    assert_eq!(encoded.len(), QUANTIZED_WITNESS_BODY_BYTES_V1);
    assert_eq!(&encoded[..2], &[3, 1]);
    assert_eq!(&encoded[2..34], &id(1).bytes());

    let original_digest = original.digest().expect("canonical body hashes");
    let mut mutation = original;
    mutation.candidate_feed = id(9);
    assert_ne!(mutation.digest(), Ok(original_digest));
    mutation = original;
    mutation.relation_domain_digest = id(9);
    assert_ne!(mutation.digest(), Ok(original_digest));
    mutation = original;
    mutation.basis_digest = id(9);
    assert_ne!(mutation.digest(), Ok(original_digest));
    mutation = original;
    mutation.candidate_price_digest = id(9);
    assert_ne!(mutation.digest(), Ok(original_digest));
    mutation = original;
    mutation.atom_coordinates[1] = 9;
    assert_ne!(mutation.digest(), Ok(original_digest));
    mutation = original;
    mutation.atom_masses = [0; 16];
    mutation.atom_masses[0] = 2;
    mutation.atom_masses[1] = 1;
    assert_ne!(mutation.digest(), Ok(original_digest));
}

#[test]
fn witness_codec_refuses_noncanonical_or_nonprimitive_support() {
    let mut mutation = body();
    mutation.atom_coordinates[2] = 12;
    assert_eq!(
        mutation.encode(),
        Err(GeneralV2RuntimeError::NonCanonicalWitnessPadding)
    );

    mutation = body();
    mutation.atom_coordinates[1] = mutation.atom_coordinates[0];
    assert_eq!(
        mutation.encode(),
        Err(GeneralV2RuntimeError::InvalidWitnessShape)
    );

    mutation = body();
    mutation.common_denominator = 6;
    mutation.atom_masses[0] = 2;
    mutation.atom_masses[1] = 4;
    assert_eq!(
        mutation.encode(),
        Err(GeneralV2RuntimeError::InvalidWitnessShape)
    );
}
