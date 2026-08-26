//! Hostile corpus for exposure-bound Fractional V2 terms and terminal evaluation.

use dclutch_fractional_claim_kernel::{
    Error, ExposureTranslationBuffersV2, FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2,
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_PREIMAGE_V2,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2, FractionalExposureTermsV2,
    check_fractional_exposure_bundle_v2, divide_exposure_shards_v2,
    encode_fractional_exposure_terms_v2, evaluate_exposure_terminal_v2,
    fractional_exposure_terms_bytes_v2, require_categorical_embedding_v2,
};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, CompositionExposureInputV3, CompositionExposureRowInputV3,
    CompositionExposureTermV3, RecordAdmissionV3, composition_exposure_bytes_v3,
    encode_composition_exposure_v3_atomic,
};
use sha2::{Digest, Sha256};

const MARKET: [u8; 32] = [1; 32];
const PRODUCT_RECORD: [u8; 32] = [2; 32];
const DOMAIN: [u8; 32] = [3; 32];
const RELEASE: [u8; 32] = [4; 32];
const TOKEN_PROGRAM: [u8; 32] = [5; 32];
const TOKEN_BEHAVIOR: [u8; 32] = [6; 32];
const EXPOSURE: [u8; 32] = [7; 32];
const PRODUCT_BASIS: [u8; 32] = [8; 32];
const REPRESENTATION_BASIS: [u8; 32] = [9; 32];
const GRAPH: [u8; 32] = [10; 32];
const TERMS: [u8; 32] = [11; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];

fn terms_admission() -> FractionalExposureTermsAdmissionV2 {
    FractionalExposureTermsAdmissionV2 {
        selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        selected_terms_id: TERMS,
        finalized_terms_id: TERMS,
        recomputed_terms_digest: TERMS,
        finalized_terms_digest: TERMS,
        record_authenticated: true,
    }
}

fn terms_bytes(product_width: u32, denominator: u64, mints: &[[u8; 32]]) -> Vec<u8> {
    let length = fractional_exposure_terms_bytes_v2(mints.len()).expect("terms width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0_u8; length];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: MARKET,
            product_record: PRODUCT_RECORD,
            result_domain: DOMAIN,
            release_set: RELEASE,
            token_program: TOKEN_PROGRAM,
            token_behavior: TOKEN_BEHAVIOR,
            exposure_id: EXPOSURE,
            product_basis: PRODUCT_BASIS,
            representation_basis: REPRESENTATION_BASIS,
            graph_id: GRAPH,
            product_width,
            denominator,
            shard_mints: mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical terms");
    output
}

fn decode_terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
    FractionalExposureTermsV2::decode(bytes, terms_admission()).expect("admitted terms")
}

fn exposure_bytes(product_width: u32, rows: &[CompositionExposureRowInputV3<'_>]) -> Vec<u8> {
    let term_count = rows.iter().map(|row| row.terms.len()).sum::<usize>();
    let length = composition_exposure_bytes_v3(
        u32::try_from(rows.len()).expect("row count"),
        u32::try_from(term_count).expect("term count"),
    )
    .expect("exposure width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0_u8; length];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: MARKET,
            result_domain: DOMAIN,
            release_set: RELEASE,
            product_basis: PRODUCT_BASIS,
            representation_basis: REPRESENTATION_BASIS,
            graph_id: GRAPH,
            product_width,
            rows,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical exposure");
    output
}

fn decode_exposure(bytes: &[u8], selected_id: [u8; 32]) -> CompositionExposureBundleV3<'_> {
    CompositionExposureBundleV3::decode(
        bytes,
        RecordAdmissionV3 {
            selected_id,
            finalized_id: selected_id,
            recomputed_digest: [31; 32],
            finalized_digest: [31; 32],
            record_authenticated: true,
        },
    )
    .expect("admitted exposure")
}

fn n258_rows<'a>(
    terms: &'a [[CompositionExposureTermV3; 1]; 3],
    denominators: [u64; 3],
) -> [CompositionExposureRowInputV3<'a>; 3] {
    [
        CompositionExposureRowInputV3 {
            node_id: [41; 32],
            denominator: denominators[0],
            terms: &terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [42; 32],
            denominator: denominators[1],
            terms: &terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [43; 32],
            denominator: denominators[2],
            terms: &terms[2],
        },
    ]
}

#[test]
fn schema_and_runtime_width_layout_are_exact() {
    assert_eq!(
        Sha256::digest(FRACTIONAL_EXPOSURE_TERMS_SCHEMA_PREIMAGE_V2).as_slice(),
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
    );
    let bytes = terms_bytes(258, 6, &MINTS);
    assert_eq!(
        bytes.len(),
        FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2 + 3 * 32
    );
    let terms = decode_terms(&bytes);
    assert_eq!(terms.product_width(), 258);
    assert_eq!(terms.representation_width(), 3);
    assert_eq!(terms.denominator(), 6);
    assert_eq!(terms.shard_mint(2), Ok(MINTS[2]));
}

#[test]
fn k3_n258_terminal_uses_exposure_and_exact_same_mint_change() {
    let sparse = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 128,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 257,
            numerator: 1,
        }],
    ];
    let rows = n258_rows(&sparse, [1, 1, 1]);
    let encoded_exposure = exposure_bytes(258, &rows);
    let bundle = decode_exposure(&encoded_exposure, EXPOSURE);
    let terms_bytes = terms_bytes(258, 6, &MINTS);
    let terms = decode_terms(&terms_bytes);
    check_fractional_exposure_bundle_v2(terms, bundle).expect("exact Product/Claims join");

    let mut payouts = vec![0_u64; 258];
    *payouts.get_mut(0).expect("coordinate zero") = 5;
    *payouts.get_mut(128).expect("coordinate 128") = 7;
    *payouts.get_mut(257).expect("coordinate 257") = 11;
    let mut scratch = [0_u64; 3];
    let mut candidate = [0_u64; 3];
    let mut translated = [91_u64; 3];
    let plan = evaluate_exposure_terminal_v2(
        terms,
        bundle,
        &payouts,
        1,
        26,
        ExposureTranslationBuffersV2 {
            scratch: &mut scratch,
            candidate: &mut candidate,
            output: &mut translated,
        },
    )
    .expect("exact terminal plan");
    assert_eq!(translated, [5, 7, 11]);
    assert_eq!(plan.division.whole_claims, 4);
    assert_eq!(plan.division.consumed.shard_atoms, 24);
    assert_eq!(plan.division.change.shard_atoms, 2);
    assert_eq!(plan.division.change.shard_mint, MINTS[1]);
    assert_eq!(plan.collateral_atoms_per_claim, 7);
    assert_eq!(plan.collateral_atoms, 28);
}

#[test]
fn nonintegral_and_overflow_refuse_without_changing_terminal_output() {
    let nonintegral_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 128,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 257,
            numerator: 3,
        }],
    ];
    let rows = n258_rows(&nonintegral_terms, [1, 2, 1]);
    let encoded_exposure = exposure_bytes(258, &rows);
    let bundle = decode_exposure(&encoded_exposure, EXPOSURE);
    let terms_bytes = terms_bytes(258, 2, &MINTS);
    let terms = decode_terms(&terms_bytes);
    let mut payouts = vec![0_u64; 258];
    *payouts.get_mut(128).expect("coordinate 128") = 7;
    let mut scratch = [0_u64; 3];
    let mut candidate = [61_u64; 3];
    let mut output = [71_u64; 3];
    assert_eq!(
        evaluate_exposure_terminal_v2(
            terms,
            bundle,
            &payouts,
            1,
            4,
            ExposureTranslationBuffersV2 {
                scratch: &mut scratch,
                candidate: &mut candidate,
                output: &mut output,
            },
        ),
        Err(Error::NonIntegralTranslation)
    );
    assert_eq!(candidate, [61; 3]);
    assert_eq!(output, [71; 3]);

    let exact_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 128,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 257,
            numerator: 1,
        }],
    ];
    let exact_rows = n258_rows(&exact_terms, [1, 1, 1]);
    let exact_bytes = exposure_bytes(258, &exact_rows);
    let exact_bundle = decode_exposure(&exact_bytes, EXPOSURE);
    *payouts.get_mut(128).expect("coordinate 128") = u64::MAX;
    candidate = [61; 3];
    assert_eq!(
        evaluate_exposure_terminal_v2(
            terms,
            exact_bundle,
            &payouts,
            1,
            4,
            ExposureTranslationBuffersV2 {
                scratch: &mut scratch,
                candidate: &mut candidate,
                output: &mut output,
            },
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(output, [71; 3]);
}

#[test]
fn categorical_embedding_is_explicit_k_equals_n_one_hot() {
    let one_hot_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }],
    ];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [41; 32],
            denominator: 1,
            terms: &one_hot_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [42; 32],
            denominator: 1,
            terms: &one_hot_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [43; 32],
            denominator: 1,
            terms: &one_hot_terms[2],
        },
    ];
    let encoded_exposure = exposure_bytes(3, &rows);
    let bundle = decode_exposure(&encoded_exposure, EXPOSURE);
    let terms_bytes = terms_bytes(3, 10, &MINTS);
    let terms = decode_terms(&terms_bytes);
    require_categorical_embedding_v2(terms, bundle).expect("categorical embeds in V2");

    let non_one_hot_terms = [
        one_hot_terms[0],
        [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }],
        one_hot_terms[2],
    ];
    let non_one_hot_rows = [
        CompositionExposureRowInputV3 {
            node_id: [41; 32],
            denominator: 1,
            terms: &non_one_hot_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [42; 32],
            denominator: 1,
            terms: &non_one_hot_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [43; 32],
            denominator: 1,
            terms: &non_one_hot_terms[2],
        },
    ];
    let non_one_hot_bytes = exposure_bytes(3, &non_one_hot_rows);
    let non_one_hot = decode_exposure(&non_one_hot_bytes, EXPOSURE);
    assert_eq!(
        require_categorical_embedding_v2(terms, non_one_hot),
        Err(Error::AdmissionMismatch)
    );
}

#[test]
fn hostile_terms_and_exposure_substitutions_refuse() {
    let duplicate = [MINTS[0], MINTS[0], MINTS[2]];
    let length = fractional_exposure_terms_bytes_v2(duplicate.len()).expect("width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0_u8; length];
    let input = FractionalExposureTermsInputV2 {
        market: MARKET,
        product_record: PRODUCT_RECORD,
        result_domain: DOMAIN,
        release_set: RELEASE,
        token_program: TOKEN_PROGRAM,
        token_behavior: TOKEN_BEHAVIOR,
        exposure_id: EXPOSURE,
        product_basis: PRODUCT_BASIS,
        representation_basis: REPRESENTATION_BASIS,
        graph_id: GRAPH,
        product_width: 258,
        denominator: 6,
        shard_mints: &duplicate,
    };
    assert_eq!(
        encode_fractional_exposure_terms_v2(input, &mut scratch, &mut output),
        Err(Error::DuplicateShardMint)
    );
    assert_eq!(output, vec![0; length]);

    let mut bytes = terms_bytes(258, 6, &MINTS);
    *bytes
        .get_mut(FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2 - 1)
        .expect("reserved tail byte") = 1;
    assert_eq!(
        FractionalExposureTermsV2::decode(&bytes, terms_admission()),
        Err(Error::NonCanonical)
    );

    let sparse = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 128,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 257,
            numerator: 1,
        }],
    ];
    let rows = n258_rows(&sparse, [1, 1, 1]);
    let exposure_bytes = exposure_bytes(258, &rows);
    let wrong_id_bundle = decode_exposure(&exposure_bytes, [99; 32]);
    let clean_terms_bytes = terms_bytes(258, 6, &MINTS);
    let terms = decode_terms(&clean_terms_bytes);
    assert_eq!(
        check_fractional_exposure_bundle_v2(terms, wrong_id_bundle),
        Err(Error::AdmissionMismatch)
    );
    let division = divide_exposure_shards_v2(terms, 0, 13).expect("division");
    assert_eq!(division.whole_claims, 2);
    assert_eq!(division.change.shard_atoms, 1);
    assert_eq!(
        divide_exposure_shards_v2(terms, 3, 13),
        Err(Error::InvalidOutcome)
    );
}
