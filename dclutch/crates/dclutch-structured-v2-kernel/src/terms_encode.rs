//! Atomic encoder for immutable Structured V2 terms.

use crate::abi::{Error, Result, is_zero, put};
use crate::generated_abi::{
    STRUCTURED_MAX_COORDINATES_V2, STRUCTURED_MIN_COORDINATES_V2, STRUCTURED_MIN_DENOMINATOR_V2,
    STRUCTURED_RECEIPT_DECIMALS_V2, STRUCTURED_SCHEMA_VERSION_V2,
    STRUCTURED_TERMS_COEFFICIENT_BYTES_V2, STRUCTURED_TERMS_DENOMINATOR_OFFSET_V2,
    STRUCTURED_TERMS_GRAPH_ID_OFFSET_V2, STRUCTURED_TERMS_HEADER_BYTES_V2,
    STRUCTURED_TERMS_MAGIC_OFFSET_V2, STRUCTURED_TERMS_MAGIC_V2, STRUCTURED_TERMS_MARKET_OFFSET_V2,
    STRUCTURED_TERMS_PRODUCT_RECORD_OFFSET_V2, STRUCTURED_TERMS_RECEIPT_DECIMALS_OFFSET_V2,
    STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2, STRUCTURED_TERMS_RELEASE_SET_OFFSET_V2,
    STRUCTURED_TERMS_REPRESENTATION_WIDTH_OFFSET_V2, STRUCTURED_TERMS_RESULT_DOMAIN_OFFSET_V2,
    STRUCTURED_TERMS_SHARD_EXPOSURE_OFFSET_V2, STRUCTURED_TERMS_SHARD_TERMS_OFFSET_V2,
    STRUCTURED_TERMS_TOKEN_BEHAVIOR_OFFSET_V2, STRUCTURED_TERMS_TOKEN_PROGRAM_OFFSET_V2,
    STRUCTURED_TERMS_VERSION_OFFSET_V2,
};

/// Atomic encoder input for immutable Structured V2 terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTermsInputV2<'a> {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product root digest.
    pub product_record: [u8; 32],
    /// Product-owned result domain.
    pub result_domain: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Selected Token program.
    pub token_program: [u8; 32],
    /// Finalized receipt Token behavior selection identity.
    pub token_behavior: [u8; 32],
    /// Finalized exact claim-shard terms owning the `K` shard Mints.
    pub shard_terms: [u8; 32],
    /// Finalized Product-N to Claims-K exposure identity.
    pub shard_exposure: [u8; 32],
    /// Token-owned Structured receipt Mint.
    pub receipt_mint: [u8; 32],
    /// Stable representation-composition graph identity.
    pub graph_id: [u8; 32],
    /// Exact shard atoms per whole native claim, owned by the shard terms.
    pub denominator: u64,
    /// Exactly `K` ordered backing coefficients; at least one must be positive.
    pub coefficients: &'a [u64],
}

/// Return the exact encoded terms width for `K` coordinates.
pub fn structured_terms_bytes_v2(representation_width: usize) -> Result<usize> {
    let minimum =
        usize::try_from(STRUCTURED_MIN_COORDINATES_V2).map_err(|_| Error::InvalidCoordinate)?;
    let maximum =
        usize::try_from(STRUCTURED_MAX_COORDINATES_V2).map_err(|_| Error::InvalidCoordinate)?;
    if !(minimum..=maximum).contains(&representation_width) {
        return Err(Error::InvalidCoordinate);
    }
    STRUCTURED_TERMS_HEADER_BYTES_V2
        .checked_add(
            representation_width
                .checked_mul(STRUCTURED_TERMS_COEFFICIENT_BYTES_V2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)
}

/// Encode immutable Structured V2 terms atomically into equal-width buffers.
///
/// `output` is written only after every field validates, so a refused encode
/// leaves the caller's accepted bytes intact.
pub fn encode_structured_terms_v2(
    input: StructuredTermsInputV2<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let width = structured_terms_bytes_v2(input.coefficients.len())?;
    if scratch.len() != width || output.len() != width {
        return Err(Error::InvalidLength);
    }
    if input.denominator < STRUCTURED_MIN_DENOMINATOR_V2 {
        return Err(Error::NonFractionalDenominator);
    }
    let identities = [
        input.market,
        input.product_record,
        input.result_domain,
        input.release_set,
        input.token_program,
        input.token_behavior,
        input.shard_terms,
        input.shard_exposure,
        input.receipt_mint,
        input.graph_id,
    ];
    if identities.iter().any(is_zero) {
        return Err(Error::ZeroIdentity);
    }
    if !input
        .coefficients
        .iter()
        .any(|coefficient| *coefficient != 0)
    {
        return Err(Error::UnbackedBasis);
    }
    let representation_width =
        u32::try_from(input.coefficients.len()).map_err(|_| Error::InvalidCoordinate)?;
    scratch.fill(0);
    put(
        scratch,
        STRUCTURED_TERMS_MAGIC_OFFSET_V2,
        &STRUCTURED_TERMS_MAGIC_V2,
    )?;
    put(
        scratch,
        STRUCTURED_TERMS_VERSION_OFFSET_V2,
        &STRUCTURED_SCHEMA_VERSION_V2.to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_TERMS_RECEIPT_DECIMALS_OFFSET_V2,
        &[STRUCTURED_RECEIPT_DECIMALS_V2],
    )?;
    for (offset, value) in [
        (STRUCTURED_TERMS_MARKET_OFFSET_V2, input.market),
        (
            STRUCTURED_TERMS_PRODUCT_RECORD_OFFSET_V2,
            input.product_record,
        ),
        (
            STRUCTURED_TERMS_RESULT_DOMAIN_OFFSET_V2,
            input.result_domain,
        ),
        (STRUCTURED_TERMS_RELEASE_SET_OFFSET_V2, input.release_set),
        (
            STRUCTURED_TERMS_TOKEN_PROGRAM_OFFSET_V2,
            input.token_program,
        ),
        (
            STRUCTURED_TERMS_TOKEN_BEHAVIOR_OFFSET_V2,
            input.token_behavior,
        ),
        (STRUCTURED_TERMS_SHARD_TERMS_OFFSET_V2, input.shard_terms),
        (
            STRUCTURED_TERMS_SHARD_EXPOSURE_OFFSET_V2,
            input.shard_exposure,
        ),
        (STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2, input.receipt_mint),
        (STRUCTURED_TERMS_GRAPH_ID_OFFSET_V2, input.graph_id),
    ] {
        put(scratch, offset, &value)?;
    }
    put(
        scratch,
        STRUCTURED_TERMS_REPRESENTATION_WIDTH_OFFSET_V2,
        &representation_width.to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_TERMS_DENOMINATOR_OFFSET_V2,
        &input.denominator.to_le_bytes(),
    )?;
    for (index, coefficient) in input.coefficients.iter().enumerate() {
        let offset = STRUCTURED_TERMS_HEADER_BYTES_V2
            .checked_add(
                index
                    .checked_mul(STRUCTURED_TERMS_COEFFICIENT_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        put(scratch, offset, &coefficient.to_le_bytes())?;
    }
    output.copy_from_slice(scratch);
    Ok(())
}
