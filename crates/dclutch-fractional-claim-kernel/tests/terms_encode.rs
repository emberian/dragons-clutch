//! Canonical immutable-terms compiler tests.

#![allow(clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_kernel::{
    Error, FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalTermsAdmissionV1, FractionalTermsInputV1,
    FractionalTermsV1, encode_fractional_terms_v1, fractional_terms_bytes_v1,
};

fn input<'a>(mints: &'a [[u8; 32]]) -> FractionalTermsInputV1<'a> {
    FractionalTermsInputV1 {
        market: [1; 32],
        result_domain: [2; 32],
        release_set: [3; 32],
        token_program: [4; 32],
        token_behavior: [5; 32],
        denominator: 10,
        shard_mints: mints,
    }
}

fn decode(bytes: &[u8]) -> FractionalTermsV1<'_> {
    FractionalTermsV1::decode(
        bytes,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: [99; 32],
            finalized_terms_id: [99; 32],
            recomputed_terms_digest: [99; 32],
            finalized_terms_digest: [99; 32],
            record_authenticated: true,
        },
    )
    .unwrap()
}

#[test]
fn runtime_width_terms_round_trip_at_258_outcomes() {
    let mints: Vec<[u8; 32]> = (0_u16..258)
        .map(|index| {
            let mut mint = [0_u8; 32];
            mint[..2].copy_from_slice(&index.saturating_add(1).to_le_bytes());
            mint
        })
        .collect();
    let width = fractional_terms_bytes_v1(mints.len()).unwrap();
    let mut scratch = vec![0; width];
    let mut output = vec![0xa5; width];
    encode_fractional_terms_v1(input(&mints), &mut scratch, &mut output).unwrap();
    let terms = decode(&output);
    assert_eq!(terms.outcome_count(), 258);
    assert_eq!(terms.denominator(), 10);
    assert_eq!(
        terms.shard_mint(257).unwrap(),
        *mints.get(257).expect("last runtime Mint")
    );
}

#[test]
fn every_refusal_preserves_output() {
    let mints = [[11; 32], [12; 32], [13; 32]];
    let width = fractional_terms_bytes_v1(mints.len()).unwrap();
    let mut scratch = vec![0; width];
    let before = vec![0xa5; width];
    let mut output = before.clone();

    let mut duplicate = mints;
    duplicate[2] = duplicate[0];
    assert_eq!(
        encode_fractional_terms_v1(input(&duplicate), &mut scratch, &mut output),
        Err(Error::DuplicateShardMint)
    );
    assert_eq!(output, before);

    let zero_program = FractionalTermsInputV1 {
        token_program: [0; 32],
        ..input(&mints)
    };
    assert_eq!(
        encode_fractional_terms_v1(zero_program, &mut scratch, &mut output),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(output, before);

    let nonfractional = FractionalTermsInputV1 {
        denominator: 1,
        ..input(&mints)
    };
    assert_eq!(
        encode_fractional_terms_v1(nonfractional, &mut scratch, &mut output),
        Err(Error::NonFractionalDenominator)
    );
    assert_eq!(output, before);
}
