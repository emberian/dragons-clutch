//! Atomic encoder for the adapter-owned Structured V2 runtime projection.

use crate::abi::{
    Error, Result, StructuredCoordinateObservationV2, StructuredPhaseV2, StructuredTermsV2, put,
};
use crate::generated_abi::{
    STRUCTURED_PHASE_OPEN_V2, STRUCTURED_PHASE_RETIRED_V2, STRUCTURED_PHASE_TERMINAL_V2,
    STRUCTURED_PROJECTION_DENOMINATOR_OFFSET_V2, STRUCTURED_PROJECTION_HEADER_BYTES_V2,
    STRUCTURED_PROJECTION_MAGIC_OFFSET_V2, STRUCTURED_PROJECTION_MAGIC_V2,
    STRUCTURED_PROJECTION_MARKET_OFFSET_V2, STRUCTURED_PROJECTION_PHASE_OFFSET_V2,
    STRUCTURED_PROJECTION_RECEIPT_SUPPLY_OFFSET_V2,
    STRUCTURED_PROJECTION_REPRESENTATION_WIDTH_OFFSET_V2, STRUCTURED_PROJECTION_REVISION_OFFSET_V2,
    STRUCTURED_PROJECTION_ROW_BYTES_V2, STRUCTURED_PROJECTION_ROW_CUSTODY_OFFSET_V2,
    STRUCTURED_PROJECTION_ROW_PAYOUT_OFFSET_V2, STRUCTURED_PROJECTION_SHARD_TERMS_OFFSET_V2,
    STRUCTURED_PROJECTION_TERMS_OFFSET_V2, STRUCTURED_PROJECTION_VERSION_OFFSET_V2,
    STRUCTURED_SCHEMA_VERSION_V2,
};

/// Return the exact encoded projection width for `K` coordinates.
pub fn structured_projection_bytes_v2(representation_width: u32) -> Result<usize> {
    STRUCTURED_PROJECTION_HEADER_BYTES_V2
        .checked_add(
            usize::try_from(representation_width)
                .map_err(|_| Error::InvalidCoordinate)?
                .checked_mul(STRUCTURED_PROJECTION_ROW_BYTES_V2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)
}

/// Encode one adapter-owned projection atomically into equal-width buffers.
pub fn encode_structured_projection_v2(
    terms: StructuredTermsV2<'_>,
    phase: StructuredPhaseV2,
    receipt_supply: u64,
    revision: u64,
    rows: &[StructuredCoordinateObservationV2],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let width = structured_projection_bytes_v2(terms.representation_width())?;
    if scratch.len() != width
        || output.len() != width
        || rows.len()
            != usize::try_from(terms.representation_width())
                .map_err(|_| Error::InvalidCoordinate)?
    {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(
        scratch,
        STRUCTURED_PROJECTION_MAGIC_OFFSET_V2,
        &STRUCTURED_PROJECTION_MAGIC_V2,
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_VERSION_OFFSET_V2,
        &STRUCTURED_SCHEMA_VERSION_V2.to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_PHASE_OFFSET_V2,
        &[match phase {
            StructuredPhaseV2::Open => STRUCTURED_PHASE_OPEN_V2,
            StructuredPhaseV2::Terminal => STRUCTURED_PHASE_TERMINAL_V2,
            StructuredPhaseV2::Retired => STRUCTURED_PHASE_RETIRED_V2,
        }],
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_TERMS_OFFSET_V2,
        &terms.terms_id(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_MARKET_OFFSET_V2,
        &terms.market(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_SHARD_TERMS_OFFSET_V2,
        &terms.shard_terms(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_REPRESENTATION_WIDTH_OFFSET_V2,
        &terms.representation_width().to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_DENOMINATOR_OFFSET_V2,
        &terms.denominator().to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_RECEIPT_SUPPLY_OFFSET_V2,
        &receipt_supply.to_le_bytes(),
    )?;
    put(
        scratch,
        STRUCTURED_PROJECTION_REVISION_OFFSET_V2,
        &revision.to_le_bytes(),
    )?;
    for (index, row) in rows.iter().enumerate() {
        let base = STRUCTURED_PROJECTION_HEADER_BYTES_V2
            .checked_add(
                index
                    .checked_mul(STRUCTURED_PROJECTION_ROW_BYTES_V2)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        put(
            scratch,
            base.checked_add(STRUCTURED_PROJECTION_ROW_CUSTODY_OFFSET_V2)
                .ok_or(Error::InvalidLength)?,
            &row.observed_shard_custody.to_le_bytes(),
        )?;
        put(
            scratch,
            base.checked_add(STRUCTURED_PROJECTION_ROW_PAYOUT_OFFSET_V2)
                .ok_or(Error::InvalidLength)?,
            &row.payout_per_claim.to_le_bytes(),
        )?;
    }
    output.copy_from_slice(scratch);
    Ok(())
}
