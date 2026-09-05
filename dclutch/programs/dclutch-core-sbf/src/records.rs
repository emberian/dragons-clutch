//! Exact finalized-record authentication shared by Core actions.

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::CoreSbfError;

/// Canonical bumps of one finalized record's raw/staging PDA pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecordPdaBumpsV1 {
    pub(crate) raw: u8,
    pub(crate) staging: u8,
}

/// Derive one record's canonical raw/staging pair and the bumps that reach it.
pub(crate) fn derive_record_pdas(
    registry_program: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> (Pubkey, Pubkey, RecordPdaBumpsV1) {
    let (raw, raw_bump) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        registry_program,
    );
    let (staging, staging_bump) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        registry_program,
    );
    (
        raw,
        staging,
        RecordPdaBumpsV1 {
            raw: raw_bump,
            staging: staging_bump,
        },
    )
}

/// Authenticate one exact finalized headerless record and its finalized cursor absence.
///
/// Nonzero lamports on the vacant System-owned staging PDA are accepted as
/// unclassified dust; ownership, zero data, non-executable status, and exact
/// derivation are the finalized-absence authority.
pub(crate) fn authenticate_finalized_record<'a>(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &'a [u8],
) -> Result<&'a [u8], CoreSbfError> {
    let (bytes, _) = authenticate_finalized_record_with_bumps(
        registry_program,
        raw,
        staging,
        schema,
        expected_digest,
        bytes,
    )?;
    Ok(bytes)
}

/// Authenticate one finalized record and return the bumps the derivation used.
///
/// The search is the authentication, so a caller that needs to persist the
/// canonical bumps takes them from here rather than repeating the search.
pub(crate) fn authenticate_finalized_record_with_bumps<'a>(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &'a [u8],
) -> Result<(&'a [u8], RecordPdaBumpsV1), CoreSbfError> {
    if raw.owner != registry_program
        || raw.executable
        || raw.is_signer
        || raw.is_writable
        || hash(bytes).to_bytes() != expected_digest
        || !funded_rent_persists_v1(raw.lamports())
    {
        return Err(CoreSbfError::FinalizedRecord);
    }
    let (expected_raw, expected_staging, bumps) =
        derive_record_pdas(registry_program, schema, expected_digest);
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
    Ok((bytes, bumps))
}

/// Hash hostile bytes and authenticate the corresponding canonical record PDA.
pub(crate) fn authenticate_content_addressed_record<'a>(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<([u8; 32], &'a [u8], RecordPdaBumpsV1), CoreSbfError> {
    let digest = hash(bytes).to_bytes();
    let (bytes, bumps) = authenticate_finalized_record_with_bumps(
        registry_program,
        raw,
        staging,
        schema,
        digest,
        bytes,
    )?;
    Ok((digest, bytes, bumps))
}
