//! Fractional consumption of the finalized K↔N composition exposure.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod support;

use dclutch_fractional_claim_contract::FractionalActionV1;
use dclutch_fractional_claim_operator::{
    Error, FractionalClaimsAccountRuleV1, decode_and_check_fractional_exposure_v3,
};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureExpectedV3, CompositionExposureInputV3, CompositionExposureRowInputV3,
    CompositionExposureTermV3, RecordAdmissionV3, composition_exposure_bytes_v3,
    encode_composition_exposure_v3_atomic,
};
use sha2::{Digest, Sha256};

use support::FractionalChainFixtureV1;

const REPRESENTATION_BASIS: [u8; 32] = [71; 32];
const GRAPH: [u8; 32] = [72; 32];
const EXPOSURE_ID: [u8; 32] = [73; 32];

fn claims_frame() -> [FractionalClaimsAccountRuleV1; 1] {
    [FractionalClaimsAccountRuleV1 {
        signer: false,
        writable: false,
        executable: true,
        data_length: 0,
    }]
}

fn expected(
    fixture: &FractionalChainFixtureV1,
    representation_width: u32,
) -> CompositionExposureExpectedV3 {
    let prepared = fixture.prepare();
    CompositionExposureExpectedV3 {
        market: prepared.request_context().market,
        result_domain: prepared.request_context().result_domain,
        release_set: prepared.request_context().release_set,
        product_basis: prepared.product_join().claim_basis_id.to_bytes(),
        representation_basis: REPRESENTATION_BASIS,
        graph_id: GRAPH,
        product_width: prepared.product_join().outcome_count,
        representation_width,
    }
}

fn encode(
    expected: CompositionExposureExpectedV3,
    rows: &[CompositionExposureRowInputV3<'_>],
) -> Vec<u8> {
    let term_count = rows.iter().map(|row| row.terms.len()).sum::<usize>();
    let width = composition_exposure_bytes_v3(
        u32::try_from(rows.len()).expect("K"),
        u32::try_from(term_count).expect("terms"),
    )
    .expect("exposure width");
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: expected.market,
            result_domain: expected.result_domain,
            release_set: expected.release_set,
            product_basis: expected.product_basis,
            representation_basis: expected.representation_basis,
            graph_id: expected.graph_id,
            product_width: expected.product_width,
            rows,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical exposure");
    output
}

fn admission(bytes: &[u8]) -> RecordAdmissionV3 {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    RecordAdmissionV3 {
        selected_id: EXPOSURE_ID,
        finalized_id: EXPOSURE_ID,
        recomputed_digest: digest,
        finalized_digest: digest,
        record_authenticated: true,
    }
}

#[test]
fn k2_n3_exact_exposure_is_retranslated_byte_identically() {
    let fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::WinningRedeem, [62; 32], &claims_frame());
    let expected = expected(&fixture, 2);
    let row0 = [
        CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        },
        CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        },
    ];
    let row1 = [CompositionExposureTermV3 {
        product_coordinate: 2,
        numerator: 1,
    }];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [81; 32],
            denominator: 1,
            terms: &row0,
        },
        CompositionExposureRowInputV3 {
            node_id: [82; 32],
            denominator: 1,
            terms: &row1,
        },
    ];
    let bytes = encode(expected, &rows);
    let checked = decode_and_check_fractional_exposure_v3(
        fixture.prepare(),
        &bytes,
        admission(&bytes),
        expected,
    )
    .expect("K2/N3 exposure");
    let mut scratch = [0_u64; 2];
    let mut output = [0_u64; 2];
    checked
        .translate_product_payouts(&[3, 5, 7], &mut scratch, &mut output)
        .expect("exact exposure");
    assert_eq!(output, [8, 7]);
    assert_eq!(checked.common_denominator(), Ok(1));
    assert_eq!(
        checked.product_record(),
        fixture.prepare().request_context().product_record
    );
    assert_eq!(checked.bundle().as_bytes(), bytes);
}

#[test]
fn n258_remains_runtime_width_while_k_is_three() {
    let fixture = FractionalChainFixtureV1::new_with_outcomes(
        FractionalActionV1::WinningRedeem,
        [62; 32],
        &claims_frame(),
        258,
    );
    let expected = expected(&fixture, 3);
    let t0 = [CompositionExposureTermV3 {
        product_coordinate: 0,
        numerator: 1,
    }];
    let t1 = [CompositionExposureTermV3 {
        product_coordinate: 128,
        numerator: 1,
    }];
    let t2 = [CompositionExposureTermV3 {
        product_coordinate: 257,
        numerator: 1,
    }];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [81; 32],
            denominator: 1,
            terms: &t0,
        },
        CompositionExposureRowInputV3 {
            node_id: [82; 32],
            denominator: 1,
            terms: &t1,
        },
        CompositionExposureRowInputV3 {
            node_id: [83; 32],
            denominator: 1,
            terms: &t2,
        },
    ];
    let bytes = encode(expected, &rows);
    let checked = decode_and_check_fractional_exposure_v3(
        fixture.prepare(),
        &bytes,
        admission(&bytes),
        expected,
    )
    .expect("K3/N258 exposure");
    let mut payouts = [0_u64; 258];
    payouts[0] = 11;
    payouts[128] = 22;
    payouts[257] = 33;
    let mut scratch = [0_u64; 3];
    let mut output = [0_u64; 3];
    checked
        .translate_product_payouts(&payouts, &mut scratch, &mut output)
        .expect("runtime-width exact exposure");
    assert_eq!(output, [11, 22, 33]);
}

#[test]
fn admission_domain_width_nonintegrality_and_overflow_refuse_atomically() {
    let fixture =
        FractionalChainFixtureV1::new(FractionalActionV1::WinningRedeem, [62; 32], &claims_frame());
    let expected = expected(&fixture, 2);
    let first = [CompositionExposureTermV3 {
        product_coordinate: 0,
        numerator: 1,
    }];
    let fractional = [CompositionExposureTermV3 {
        product_coordinate: 2,
        numerator: 1,
    }];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [81; 32],
            denominator: 1,
            terms: &first,
        },
        CompositionExposureRowInputV3 {
            node_id: [82; 32],
            denominator: 2,
            terms: &fractional,
        },
    ];
    let bytes = encode(expected, &rows);
    let checked = decode_and_check_fractional_exposure_v3(
        fixture.prepare(),
        &bytes,
        admission(&bytes),
        expected,
    )
    .expect("fractional exact record");
    assert_eq!(checked.common_denominator(), Ok(2));
    let mut scratch = [0_u64; 2];
    let mut output = [77_u64; 2];
    assert_eq!(
        checked.translate_product_payouts(&[1, 2, 3], &mut scratch, &mut output),
        Err(Error::Composition)
    );
    assert_eq!(output, [77; 2]);

    let mut wrong_expected = expected;
    wrong_expected.result_domain = [99; 32];
    assert!(matches!(
        decode_and_check_fractional_exposure_v3(
            fixture.prepare(),
            &bytes,
            admission(&bytes),
            wrong_expected,
        ),
        Err(Error::Composition)
    ));
    let mut unauthenticated = admission(&bytes);
    unauthenticated.record_authenticated = false;
    assert!(matches!(
        decode_and_check_fractional_exposure_v3(
            fixture.prepare(),
            &bytes,
            unauthenticated,
            expected,
        ),
        Err(Error::Composition)
    ));

    let huge = [CompositionExposureTermV3 {
        product_coordinate: 0,
        numerator: u64::MAX,
    }];
    let second = [CompositionExposureTermV3 {
        product_coordinate: 1,
        numerator: 1,
    }];
    let overflow_rows = [
        CompositionExposureRowInputV3 {
            node_id: [81; 32],
            denominator: 1,
            terms: &huge,
        },
        CompositionExposureRowInputV3 {
            node_id: [82; 32],
            denominator: 1,
            terms: &second,
        },
    ];
    let overflow = encode(expected, &overflow_rows);
    let checked_overflow = decode_and_check_fractional_exposure_v3(
        fixture.prepare(),
        &overflow,
        admission(&overflow),
        expected,
    )
    .expect("large exact coefficient");
    assert_eq!(
        checked_overflow.translate_product_payouts(&[2, 1, 0], &mut scratch, &mut output),
        Err(Error::Composition)
    );
    assert_eq!(output, [77; 2]);
}
