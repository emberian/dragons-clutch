//! Failure-atomic encoder for ephemeral chain-derived reserve projections.

use crate::{
    Error, FRACTIONAL_PROJECTION_HEADER_BYTES_V1, FRACTIONAL_PROJECTION_MAGIC_V1,
    FRACTIONAL_PROJECTION_ROW_BYTES_V1, FractionalPhaseV1, FractionalProjectionV1,
    FractionalTermsV1, OutcomeReserveV1, Result, SCHEMA_VERSION_V1,
    generated_abi::{
        NO_TERMINAL_OUTCOME, PROJECTION_MARKET_OFFSET, PROJECTION_OUTCOME_COUNT_OFFSET,
        PROJECTION_PHASE_OFFSET, PROJECTION_REVISION_OFFSET, PROJECTION_TERMINAL_OUTCOME_OFFSET,
        PROJECTION_TERMS_ID_OFFSET, PROJECTION_VERSION_OFFSET,
    },
};

/// Return the exact projection width for one Product-owned outcome count.
pub fn fractional_projection_bytes_v1(outcome_count: u32) -> Result<usize> {
    usize::try_from(outcome_count)
        .ok()
        .filter(|count| *count != 0)
        .and_then(|count| count.checked_mul(FRACTIONAL_PROJECTION_ROW_BYTES_V1))
        .and_then(|rows| FRACTIONAL_PROJECTION_HEADER_BYTES_V1.checked_add(rows))
        .ok_or(Error::InvalidLength)
}

/// Encode one ephemeral projection from exact Claims/Token observations.
///
/// The output is not persisted authority. It exists so host and SBF adapters
/// can feed the same hostile decoder without duplicating physical offsets.
/// Candidate bytes change only after the complete projection decodes and all
/// phase-dependent reserve invariants hold.
pub fn encode_fractional_projection_v1(
    terms: FractionalTermsV1<'_>,
    phase: FractionalPhaseV1,
    revision: u64,
    reserves: &[OutcomeReserveV1],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = fractional_projection_bytes_v1(terms.outcome_count())?;
    if scratch.len() != expected
        || output.len() != expected
        || reserves.len() != expected_rows(terms)?
    {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(scratch, 0, &FRACTIONAL_PROJECTION_MAGIC_V1)?;
    put(
        scratch,
        PROJECTION_VERSION_OFFSET,
        &SCHEMA_VERSION_V1.to_le_bytes(),
    )?;
    let (phase_byte, terminal_outcome) = match phase {
        FractionalPhaseV1::Open => (0, NO_TERMINAL_OUTCOME),
        FractionalPhaseV1::Terminal { winning_outcome }
            if winning_outcome < terms.outcome_count() =>
        {
            (1, winning_outcome)
        }
        FractionalPhaseV1::Terminal { .. } => return Err(Error::InvalidOutcome),
        FractionalPhaseV1::Retired => (2, NO_TERMINAL_OUTCOME),
    };
    *scratch
        .get_mut(PROJECTION_PHASE_OFFSET)
        .ok_or(Error::InvalidLength)? = phase_byte;
    put(scratch, PROJECTION_TERMS_ID_OFFSET, &terms.terms_id())?;
    put(scratch, PROJECTION_MARKET_OFFSET, &terms.market_id())?;
    put(
        scratch,
        PROJECTION_OUTCOME_COUNT_OFFSET,
        &terms.outcome_count().to_le_bytes(),
    )?;
    put(
        scratch,
        PROJECTION_TERMINAL_OUTCOME_OFFSET,
        &terminal_outcome.to_le_bytes(),
    )?;
    put(scratch, PROJECTION_REVISION_OFFSET, &revision.to_le_bytes())?;
    let mut index = 0_usize;
    while index < reserves.len() {
        let row = reserves.get(index).ok_or(Error::InvalidLength)?;
        let offset = FRACTIONAL_PROJECTION_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(FRACTIONAL_PROJECTION_ROW_BYTES_V1)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        put(scratch, offset, &row.locked_native_claims.to_le_bytes())?;
        put(scratch, offset + 8, &row.shard_supply.to_le_bytes())?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    FractionalProjectionV1::decode(scratch, terms)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn expected_rows(terms: FractionalTermsV1<'_>) -> Result<usize> {
    usize::try_from(terms.outcome_count()).map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
