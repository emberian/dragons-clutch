//! Shared authentication of chain observations, owned by no family.
//!
//! These four things are what every builder in this crate needs before it can
//! trust an account it was handed: the canonical Rent and Clock sysvars, and a
//! record proved finalized at its own content-derived address with a vacant
//! staging cursor at the paired one. They lived in `foundation`, the DCLTCAT1
//! Realm/Market founding builder, which is banished; the builders that need
//! them -- `direct_inline_v3`, `series_hot_v3` -- are not that family and never
//! were. A shared fact does not belong to the first module that happened to
//! need it.

use dclutch_record_contract::{
    ContentDigest, RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1, SchemaReleaseId,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, pubkey::Pubkey, rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{sysvar, system_program};

use crate::ObservedAccount;

/// Refusal from authenticating one observed account against its stated role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationError {
    /// Records or vacancy proofs came from different observations.
    ObservationMismatch,
    /// A protocol PDA or canonical program/sysvar key differed.
    AddressMismatch,
    /// An account owner or executable bit was incompatible with its role.
    InvalidOwner,
    /// Rent sysvar bytes or identity were invalid.
    InvalidRent,
    /// Clock sysvar bytes or identity were invalid.
    InvalidClock,
    /// An existing immutable account was not rent exempt.
    AccountNotRentExempt,
    /// A content digest or cross-record semantic link differed.
    ContentLinkMismatch,
}

/// Chain-observed finalization proof paired with one immutable raw record.
///
/// The schema/release identifier and content digest derive both the raw record
/// and its now-vacant staging cursor.  A builder never treats a decoded record
/// at an arbitrary program-owned address as finalized evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedRecordProof {
    /// Schema/release identity used in the raw and cursor PDA derivations.
    pub schema_release_id: [u8; 32],
    /// Full finalized observation of the paired, vacant staging cursor.
    pub staging_cursor: ObservedAccount,
}

/// Authenticate one raw record as finalized at its own content-derived address.
///
/// The record must be program-owned, non-executable, and rent exempt; its
/// address must be the raw-record PDA for the stated schema/release and the
/// digest of its own bytes; and the paired staging cursor must be observably
/// vacant at the same observation. A builder never treats a decoded record at
/// an arbitrary program-owned address as finalized evidence.
pub fn authenticate_finalized_record(
    program_id: Pubkey,
    rent: &Rent,
    account: &ObservedAccount,
    proof: &FinalizedRecordProof,
) -> Result<(), ObservationError> {
    if account.owner != program_id || account.executable {
        return Err(ObservationError::InvalidOwner);
    }
    require_rent_exempt(rent, account)?;
    let schema = SchemaReleaseId::new(proof.schema_release_id)
        .map_err(|_| ObservationError::AddressMismatch)?;
    let digest = ContentDigest::new(hash(&account.data).to_bytes())
        .map_err(|_| ObservationError::ContentLinkMismatch)?;
    let schema_bytes = schema.to_bytes();
    let digest_bytes = digest.to_bytes();
    let (expected_raw, _) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            schema_bytes.as_slice(),
            digest_bytes.as_slice(),
        ],
        &program_id,
    );
    let (expected_cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema_bytes.as_slice(),
            digest_bytes.as_slice(),
        ],
        &program_id,
    );
    let cursor = &proof.staging_cursor;
    if account.key != expected_raw
        || cursor.key != expected_cursor
        || cursor.owner != system_program::ID
        || cursor.executable
        || !cursor.data.is_empty()
    {
        return Err(ObservationError::AddressMismatch);
    }
    if cursor.observation != account.observation {
        return Err(ObservationError::ObservationMismatch);
    }
    Ok(())
}

/// Authenticate and decode the canonical Rent sysvar from one observation.
///
/// The account identity, owner, executable bit, and exact length are checked
/// before the bytes are deserialized, so a caller cannot substitute a
/// hand-built account for the runtime's own rent.
pub fn decode_rent(account: &ObservedAccount) -> Result<Rent, ObservationError> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(ObservationError::InvalidRent);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        account.executable,
    );
    Rent::from_account_info(&info).map_err(|_| ObservationError::InvalidRent)
}

/// Authenticate and decode the canonical Clock sysvar from one observation.
///
/// The account identity, owner, executable bit, and exact length are all
/// checked before the bytes are deserialized, so a caller cannot substitute a
/// hand-built account for the runtime's own clock.
pub fn decode_clock(account: &ObservedAccount) -> Result<Clock, ObservationError> {
    if account.key != sysvar::clock::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Clock::size_of()
    {
        return Err(ObservationError::InvalidClock);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        account.executable,
    );
    Clock::from_account_info(&info).map_err(|_| ObservationError::InvalidClock)
}

fn require_rent_exempt(rent: &Rent, account: &ObservedAccount) -> Result<(), ObservationError> {
    if !rent.is_exempt(account.lamports, account.data.len()) {
        return Err(ObservationError::AccountNotRentExempt);
    }
    Ok(())
}
