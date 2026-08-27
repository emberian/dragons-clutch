//! Failure-atomic compiler for immutable runtime-width Fractional terms.

use crate::{
    Error, FRACTIONAL_TERMS_HEADER_BYTES_V1, FRACTIONAL_TERMS_MAGIC_V1,
    FRACTIONAL_TERMS_MINT_BYTES_V1, FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalTermsAdmissionV1,
    FractionalTermsV1, Result, SCHEMA_VERSION_V1,
    generated_abi::{
        TERMS_DENOMINATOR_OFFSET, TERMS_MARKET_OFFSET, TERMS_OUTCOME_COUNT_OFFSET,
        TERMS_RELEASE_SET_OFFSET, TERMS_RESULT_DOMAIN_OFFSET, TERMS_TOKEN_BEHAVIOR_OFFSET,
        TERMS_TOKEN_PROGRAM_OFFSET, TERMS_VERSION_OFFSET,
    },
};

/// Data-defined immutable Fractional terms before canonical byte emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTermsInputV1<'a> {
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Product-owned ResultDomain digest and outcome ordering.
    pub result_domain: [u8; 32],
    /// Immutable execution release-set identity.
    pub release_set: [u8; 32],
    /// Release-selected Token program.
    pub token_program: [u8; 32],
    /// Finalized TokenBehaviorV2 selection identity.
    pub token_behavior: [u8; 32],
    /// Exact number of shard atoms representing one native claim.
    pub denominator: u64,
    /// One nonzero, unique, terms-owned shard Mint per Product outcome.
    pub shard_mints: &'a [[u8; 32]],
}

/// Return the exact immutable-terms width for one runtime-selected outcome count.
pub fn fractional_terms_bytes_v1(outcome_count: usize) -> Result<usize> {
    if outcome_count == 0 || u32::try_from(outcome_count).is_err() {
        return Err(Error::InvalidOutcome);
    }
    outcome_count
        .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
        .and_then(|tail| FRACTIONAL_TERMS_HEADER_BYTES_V1.checked_add(tail))
        .ok_or(Error::InvalidLength)
}

/// Emit one canonical runtime-width immutable terms body.
///
/// Every semantic check completes against `scratch` before `output` changes.
/// The kernel performs no hashing; the Registry adapter remains responsible
/// for content identity and finalized raw/staging authentication.
pub fn encode_fractional_terms_v1(
    input: FractionalTermsInputV1<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let expected = fractional_terms_bytes_v1(input.shard_mints.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    if [
        input.market,
        input.result_domain,
        input.release_set,
        input.token_program,
        input.token_behavior,
    ]
    .iter()
    .any(is_zero)
    {
        return Err(Error::ZeroIdentity);
    }
    if input.denominator <= 1 {
        return Err(Error::NonFractionalDenominator);
    }
    for (index, mint) in input.shard_mints.iter().enumerate() {
        if is_zero(mint)
            || input
                .shard_mints
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other == mint)
        {
            return Err(if is_zero(mint) {
                Error::ZeroIdentity
            } else {
                Error::DuplicateShardMint
            });
        }
    }

    scratch.fill(0);
    put(scratch, 0, &FRACTIONAL_TERMS_MAGIC_V1)?;
    put(
        scratch,
        TERMS_VERSION_OFFSET,
        &SCHEMA_VERSION_V1.to_le_bytes(),
    )?;
    put(scratch, TERMS_MARKET_OFFSET, &input.market)?;
    put(scratch, TERMS_RESULT_DOMAIN_OFFSET, &input.result_domain)?;
    put(scratch, TERMS_RELEASE_SET_OFFSET, &input.release_set)?;
    put(scratch, TERMS_TOKEN_PROGRAM_OFFSET, &input.token_program)?;
    put(scratch, TERMS_TOKEN_BEHAVIOR_OFFSET, &input.token_behavior)?;
    let outcome_count =
        u32::try_from(input.shard_mints.len()).map_err(|_| Error::InvalidOutcome)?;
    put(
        scratch,
        TERMS_OUTCOME_COUNT_OFFSET,
        &outcome_count.to_le_bytes(),
    )?;
    put(
        scratch,
        TERMS_DENOMINATOR_OFFSET,
        &input.denominator.to_le_bytes(),
    )?;
    for (index, mint) in input.shard_mints.iter().enumerate() {
        let offset = FRACTIONAL_TERMS_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        put(scratch, offset, mint)?;
    }

    // Content hashing is intentionally outside this kernel. A nonzero local
    // identity lets the hostile decoder recheck the complete emitted body.
    let compiler_identity = [1; 32];
    FractionalTermsV1::decode(
        scratch,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: compiler_identity,
            finalized_terms_id: compiler_identity,
            recomputed_terms_digest: compiler_identity,
            finalized_terms_digest: compiler_identity,
            record_authenticated: true,
        },
    )?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
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
