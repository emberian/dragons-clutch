//! Shared fixture builders for the Structured V2 kernel tests.
//!
//! The running instrument is a two-coordinate receipt over a shard layer with
//! denominator `4` and coefficients `[1, 3]`: one receipt atom denotes exactly
//! `1/4` native claims of coordinate `0` and `3/4` of coordinate `1`.

#![allow(dead_code)]

use dclutch_fractional_claim_kernel::{
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2, FractionalExposureTermsV2,
    encode_fractional_exposure_terms_v2, fractional_exposure_terms_bytes_v2,
};
use dclutch_structured_v2_kernel::{
    StructuredCoordinateObservationV2, StructuredPhaseV2, StructuredTermsAdmissionV2,
    StructuredTermsInputV2, StructuredTermsV2, encode_structured_projection_v2,
    encode_structured_terms_v2, structured_projection_bytes_v2, structured_terms_bytes_v2,
};
use sha2::{Digest, Sha256};

/// Distinct nonzero test identity.
pub fn identity(tag: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = tag;
    value[31] = 0xa5;
    value
}

pub const MARKET: u8 = 0x11;
pub const PRODUCT_RECORD: u8 = 0x12;
pub const RESULT_DOMAIN: u8 = 0x13;
pub const RELEASE_SET: u8 = 0x14;
pub const TOKEN_PROGRAM: u8 = 0x15;
pub const SHARD_TOKEN_BEHAVIOR: u8 = 0x16;
pub const RECEIPT_TOKEN_BEHAVIOR: u8 = 0x17;
pub const SHARD_EXPOSURE: u8 = 0x18;
pub const PRODUCT_BASIS: u8 = 0x19;
pub const REPRESENTATION_BASIS: u8 = 0x1a;
pub const GRAPH_ID: u8 = 0x1b;
pub const RECEIPT_MINT: u8 = 0x1c;
pub const SHARD_MINT_BASE: u8 = 0x40;

/// Running fixture denominator: one native claim splits into four shard atoms.
pub const DENOMINATOR_FIXTURE: u64 = 4;

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Exact claim-shard terms bytes for `width` coordinates at `denominator`.
pub fn shard_terms_bytes(width: usize, denominator: u64) -> Vec<u8> {
    shard_terms_bytes_with_mints(&shard_mints(width), denominator)
}

/// Ordered distinct shard Mints.
pub fn shard_mints(width: usize) -> Vec<[u8; 32]> {
    (0..width)
        .map(|index| {
            identity(
                SHARD_MINT_BASE
                    .checked_add(u8::try_from(index).expect("test width fits a byte"))
                    .expect("test mint tag fits a byte"),
            )
        })
        .collect()
}

pub fn shard_terms_bytes_with_mints(mints: &[[u8; 32]], denominator: u64) -> Vec<u8> {
    let size = fractional_exposure_terms_bytes_v2(mints.len()).expect("shard terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: identity(MARKET),
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: identity(TOKEN_PROGRAM),
            token_behavior: identity(SHARD_TOKEN_BEHAVIOR),
            exposure_id: identity(SHARD_EXPOSURE),
            product_basis: identity(PRODUCT_BASIS),
            representation_basis: identity(REPRESENTATION_BASIS),
            graph_id: identity(GRAPH_ID),
            product_width: u32::try_from(mints.len()).expect("product width"),
            denominator,
            shard_mints: mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode shard terms");
    output
}

pub fn shard_terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
    let content = digest(bytes);
    FractionalExposureTermsV2::decode(
        bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id:
                dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id:
                dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: content,
            finalized_terms_id: content,
            recomputed_terms_digest: content,
            finalized_terms_digest: content,
            record_authenticated: true,
        },
    )
    .expect("decode shard terms")
}

/// Structured terms bytes for the given coefficients and denominator.
pub fn structured_terms_bytes(coefficients: &[u64], denominator: u64) -> Vec<u8> {
    structured_terms_bytes_with(coefficients, denominator, identity(RECEIPT_MINT))
}

pub fn structured_terms_bytes_with(
    coefficients: &[u64],
    denominator: u64,
    receipt_mint: [u8; 32],
) -> Vec<u8> {
    let size = structured_terms_bytes_v2(coefficients.len()).expect("structured terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_structured_terms_v2(
        StructuredTermsInputV2 {
            market: identity(MARKET),
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: identity(TOKEN_PROGRAM),
            token_behavior: identity(RECEIPT_TOKEN_BEHAVIOR),
            shard_terms: digest(&shard_terms_bytes(coefficients.len(), denominator)),
            shard_exposure: identity(SHARD_EXPOSURE),
            receipt_mint,
            graph_id: identity(GRAPH_ID),
            denominator,
            coefficients,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode structured terms");
    output
}

pub fn structured_admission(bytes: &[u8]) -> StructuredTermsAdmissionV2 {
    let content = digest(bytes);
    StructuredTermsAdmissionV2 {
        selected_schema_id: dclutch_structured_v2_kernel::STRUCTURED_TERMS_SCHEMA_ID_V2,
        finalized_schema_id: dclutch_structured_v2_kernel::STRUCTURED_TERMS_SCHEMA_ID_V2,
        selected_terms_id: content,
        finalized_terms_id: content,
        recomputed_terms_digest: content,
        finalized_terms_digest: content,
        record_authenticated: true,
    }
}

pub fn structured_terms<'a>(
    bytes: &'a [u8],
    shard: FractionalExposureTermsV2<'_>,
) -> StructuredTermsV2<'a> {
    StructuredTermsV2::decode(bytes, structured_admission(bytes), shard)
        .expect("decode structured terms")
}

/// Encode one adapter projection into caller-owned bytes.
pub fn projection_bytes(
    terms: StructuredTermsV2<'_>,
    phase: StructuredPhaseV2,
    receipt_supply: u64,
    revision: u64,
    rows: &[StructuredCoordinateObservationV2],
) -> Vec<u8> {
    let size =
        structured_projection_bytes_v2(terms.representation_width()).expect("projection width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_structured_projection_v2(
        terms,
        phase,
        receipt_supply,
        revision,
        rows,
        &mut scratch,
        &mut output,
    )
    .expect("encode projection");
    output
}

/// Exactly backed rows: `observed = supply * coefficient` with no surplus.
pub fn exact_rows(
    coefficients: &[u64],
    receipt_supply: u64,
    payouts: &[u64],
) -> Vec<StructuredCoordinateObservationV2> {
    coefficients
        .iter()
        .zip(payouts)
        .map(|(coefficient, payout)| StructuredCoordinateObservationV2 {
            observed_shard_custody: receipt_supply * coefficient,
            payout_per_claim: *payout,
        })
        .collect()
}

/* --- operator-specific fixtures --- */

pub const ROOT: u8 = 0x21;
pub const RENT_BENEFICIARY: u8 = 0x22;
pub const RECEIPT_SOURCE: u8 = 0x23;
pub const RECEIPT_DESTINATION: u8 = 0x24;
pub const RENT_CREDIT: u8 = 0x25;
pub const RENT_PROGRAM: u8 = 0x26;
pub const OWNER: u8 = 0x27;
pub const TERMINAL_DIGEST: u8 = 0x28;
pub const HOLDER_SHARD_BASE: u8 = 0x60;
pub const CUSTODY_SHARD_BASE: u8 = 0x70;
