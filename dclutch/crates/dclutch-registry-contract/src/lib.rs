#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! SDK-free successor Registry admission semantics.
//!
//! The authority chain is singular:
//! `Market.capability_manifest_id` selects one execution-authority manifest;
//! that manifest selects one semantic capability manifest and one execution
//! release set; and that release set selects five exact artifact releases.
//! Activation checks current Loader observations and materializes one derived
//! cache.  Solana account ownership, finalized-record hashing, Loader parsing,
//! and PDA derivation remain explicitly outside this contract.

mod activation;
mod artifact;
mod authority;

pub use activation::*;
pub use artifact::*;
pub use authority::*;

/// Bytes in every identity and digest coordinate.
pub const IDENTITY_BYTES: usize = 32;

/// Stable refusal from Registry semantic admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte input did not have its one exact canonical width.
    InvalidLength,
    /// Magic bytes did not identify the requested record.
    InvalidMagic,
    /// A record named an unsupported semantic schema.
    UnsupportedSchema,
    /// A record named an unsupported fixed-layout profile.
    UnsupportedArtifactProfile,
    /// Reserved or inactive bytes were not canonically zero.
    NonCanonicalReservedBytes,
    /// An identity or digest used the reserved all-zero value.
    ZeroIdentity,
    /// Program, ProgramData, or Loader identities were improperly aliased.
    AliasedLoaderIdentity,
    /// Upgrade policy and authority bytes were not canonical.
    NonCanonicalUpgradeAuthority,
    /// The immutable Market selected a different authority-manifest identity.
    MarketAuthorityManifestMismatch,
    /// The authority manifest selected a different release-set identity.
    ReleaseSetSelectionMismatch,
    /// A role supplied a program other than its release-set selection.
    RoleProgramMismatch,
    /// A role supplied a different artifact-release content identity.
    RoleArtifactReleaseMismatch,
    /// Two aliased roles did not supply one identical activation input.
    AliasedRoleActivationMismatch,
    /// A Program account linked to a different ProgramData account.
    ProgramDataLinkMismatch,
    /// Program or ProgramData ownership did not match the selected Loader.
    LoaderOwnerMismatch,
    /// The selected Program account was not executable.
    ProgramNotExecutable,
    /// The selected ProgramData account was executable.
    ProgramDataExecutable,
    /// Program, ProgramData, or Loader identity differed from the release.
    DeploymentIdentityMismatch,
    /// The observed ProgramData deployment slot was stale or substituted.
    DeploymentSlotMismatch,
    /// The complete observed ELF digest differed from the release.
    ElfDigestMismatch,
    /// Current upgrade authority differed from the immutable release policy.
    UpgradeAuthorityMismatch,
    /// The referenced execution-release-set codec refused.
    ReleaseSet(dclutch_release_set_contract::Error),
}

impl From<dclutch_release_set_contract::Error> for Error {
    fn from(value: dclutch_release_set_contract::Error) -> Self {
        Self::ReleaseSet(value)
    }
}

/// Result alias for Registry admission.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn require_nonzero(value: &[u8; IDENTITY_BYTES]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentity);
    }
    Ok(())
}

pub(crate) fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

pub(crate) fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

pub(crate) fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn subslice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

pub(crate) fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(bytes, offset, width)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

pub(crate) fn copy_infallible(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

pub(crate) fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    copy_infallible(output, offset, &value.to_le_bytes());
}

pub(crate) fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    copy_infallible(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests;
