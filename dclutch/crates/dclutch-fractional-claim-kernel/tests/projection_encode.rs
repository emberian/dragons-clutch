//! Failure-atomic reserve-projection encoder tests.

#![allow(clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_kernel::{
    Error, FRACTIONAL_TERMS_HEADER_BYTES_V1, FRACTIONAL_TERMS_MAGIC_V1,
    FRACTIONAL_TERMS_MINT_BYTES_V1, FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalPhaseV1,
    FractionalProjectionV1, FractionalTermsAdmissionV1, FractionalTermsV1, OutcomeReserveV1,
    SCHEMA_VERSION_V1, encode_fractional_projection_v1, fractional_projection_bytes_v1,
};

const OUTCOMES: u32 = 3;
const TERMS_ID: [u8; 32] = [91; 32];

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture destination")
        .copy_from_slice(value);
}

fn terms_bytes() -> Vec<u8> {
    let mut output = vec![
        0;
        FRACTIONAL_TERMS_HEADER_BYTES_V1
            + usize::try_from(OUTCOMES).unwrap() * FRACTIONAL_TERMS_MINT_BYTES_V1
    ];
    put(&mut output, 0, &FRACTIONAL_TERMS_MAGIC_V1);
    put(&mut output, 8, &SCHEMA_VERSION_V1.to_le_bytes());
    put(&mut output, 16, &[1; 32]);
    put(&mut output, 48, &[2; 32]);
    put(&mut output, 80, &[3; 32]);
    put(&mut output, 112, &[4; 32]);
    put(&mut output, 144, &[5; 32]);
    put(&mut output, 176, &OUTCOMES.to_le_bytes());
    put(&mut output, 184, &10_u64.to_le_bytes());
    for outcome in 0..OUTCOMES {
        let offset = FRACTIONAL_TERMS_HEADER_BYTES_V1 + usize::try_from(outcome).unwrap() * 32;
        put(
            &mut output,
            offset,
            &[u8::try_from(outcome + 11).unwrap(); 32],
        );
    }
    output
}

fn decode_terms(bytes: &[u8]) -> FractionalTermsV1<'_> {
    FractionalTermsV1::decode(
        bytes,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: TERMS_ID,
            finalized_terms_id: TERMS_ID,
            recomputed_terms_digest: TERMS_ID,
            finalized_terms_digest: TERMS_ID,
            record_authenticated: true,
        },
    )
    .unwrap()
}

#[test]
fn runtime_width_projection_round_trips_open_and_terminal_rows() {
    let bytes = terms_bytes();
    let terms = decode_terms(&bytes);
    let rows = [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 20,
        },
        OutcomeReserveV1 {
            locked_native_claims: 3,
            shard_supply: 30,
        },
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 40,
        },
    ];
    let width = fractional_projection_bytes_v1(OUTCOMES).unwrap();
    let mut scratch = vec![0; width];
    let mut output = vec![7; width];
    encode_fractional_projection_v1(
        terms,
        FractionalPhaseV1::Open,
        41,
        &rows,
        &mut scratch,
        &mut output,
    )
    .unwrap();
    let decoded = FractionalProjectionV1::decode(&output, terms).unwrap();
    assert_eq!(decoded.revision(), 41);
    assert_eq!(decoded.reserve(2).unwrap(), rows[2]);

    let terminal = [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 13,
        },
        rows[1],
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 0,
        },
    ];
    encode_fractional_projection_v1(
        terms,
        FractionalPhaseV1::Terminal { winning_outcome: 1 },
        42,
        &terminal,
        &mut scratch,
        &mut output,
    )
    .unwrap();
    assert_eq!(
        FractionalProjectionV1::decode(&output, terms)
            .unwrap()
            .phase(),
        FractionalPhaseV1::Terminal { winning_outcome: 1 }
    );
}

#[test]
fn every_refusal_preserves_the_caller_output() {
    let bytes = terms_bytes();
    let terms = decode_terms(&bytes);
    let rows = [
        OutcomeReserveV1 {
            locked_native_claims: 2,
            shard_supply: 19,
        },
        OutcomeReserveV1 {
            locked_native_claims: 3,
            shard_supply: 30,
        },
        OutcomeReserveV1 {
            locked_native_claims: 4,
            shard_supply: 40,
        },
    ];
    let width = fractional_projection_bytes_v1(OUTCOMES).unwrap();
    let mut scratch = vec![0; width];
    let before = vec![0xa5; width];
    let mut output = before.clone();
    assert_eq!(
        encode_fractional_projection_v1(
            terms,
            FractionalPhaseV1::Open,
            1,
            &rows,
            &mut scratch,
            &mut output,
        ),
        Err(Error::ReserveMismatch)
    );
    assert_eq!(output, before);
    assert_eq!(
        encode_fractional_projection_v1(
            terms,
            FractionalPhaseV1::Terminal {
                winning_outcome: OUTCOMES,
            },
            1,
            &rows,
            &mut scratch,
            &mut output,
        ),
        Err(Error::InvalidOutcome)
    );
    assert_eq!(output, before);
}
