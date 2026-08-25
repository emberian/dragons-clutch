//! Exact finalized-record authentication shared by Core actions.

use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::system_program;

use crate::CoreSbfError;

/// Authenticate one exact finalized headerless record and its finalized cursor absence.
///
/// Nonzero lamports on the vacant System-owned staging PDA are accepted as
/// unclassified dust; ownership, zero data, non-executable status, and exact
/// derivation are the finalized-absence authority.
pub(crate) fn authenticate_finalized_record<'a>(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &'a [u8],
) -> Result<&'a [u8], CoreSbfError> {
    if raw.owner != registry_program
        || raw.executable
        || raw.is_signer
        || raw.is_writable
        || hash(bytes).to_bytes() != expected_digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
    {
        return Err(CoreSbfError::FinalizedRecord);
    }
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &expected_digest],
        registry_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &expected_digest],
        registry_program,
    )
    .0;
    if raw.key != &expected_raw
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.executable
        || staging.is_signer
        || staging.is_writable
    {
        return Err(CoreSbfError::FinalizedRecord);
    }
    Ok(bytes)
}

/// Hash hostile bytes and authenticate the corresponding canonical record PDA.
pub(crate) fn authenticate_content_addressed_record<'a>(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<([u8; 32], &'a [u8]), CoreSbfError> {
    let digest = hash(bytes).to_bytes();
    authenticate_finalized_record(registry_program, raw, staging, rent, schema, digest, bytes)?;
    Ok((digest, bytes))
}
