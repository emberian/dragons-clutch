#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Provider-neutral, fixed-layout Product contracts for dClutch.
//!
//! This crate commits Product truth without committing source transport. A
//! [`product::TermsV1`] names the reusable semantics and canonical partition;
//! an [`product::OccurrenceV1`] names one event under those terms; and a
//! [`product::InstanceV1`] binds that event to one finite claim basis. Source
//! accounts, oracle messages, resolver incentives, RPC observations, and SVM
//! account policy deliberately do not appear in any preimage here.
//!
//! Large finite artifacts are content-addressed and bounded by an immutable
//! [`capacity::CapacityProfileV1`]. The crate does not execute an arbitrary VM:
//! a nonzero verifier/evaluator release identity selects separately reviewed
//! exact semantics. New capacity-profile identities can lift measured or
//! provisional size envelopes without changing Product ontology.

use core::convert::TryInto;

/// Immutable capacity-envelope contracts.
pub mod capacity;
/// Finite exact claim-basis profile contracts.
pub mod claim;
/// Terms, occurrence, and Product-instance contracts.
pub mod product;
/// Compact provider-neutral terminal result contract.
pub mod terminal;

/// Exact byte width of an opaque content identity.
pub const CONTENT_ID_BYTES: usize = 32;

/// Refusal returned by a Product contract parser or constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input did not have its one exact canonical length.
    InvalidLength,
    /// A canonical record had the wrong magic.
    InvalidMagic,
    /// A record selected an unsupported schema release.
    UnsupportedSchema,
    /// Reserved bytes were not all zero.
    NonCanonicalReservedBytes,
    /// A required content identity was all zero.
    ZeroIdentifier,
    /// A capacity envelope byte was not defined.
    UnknownEnvelopeKind,
    /// An exact coefficient word width is not supported by this schema.
    UnsupportedWordWidth,
    /// A capacity quantity that must be positive was zero.
    ZeroCapacity,
    /// Page capacity was not the unique minimal cover for the artifact bound.
    NonCanonicalPaging,
    /// Checked exact integer arithmetic overflowed.
    ArithmeticOverflow,
    /// An artifact exceeded the selected capacity profile.
    ArtifactExceedsCapacity,
    /// An artifact page count was not its unique minimal canonical count.
    PageCountMismatch,
    /// A state partition had fewer than two cells.
    PartitionTooSmall,
    /// A state partition exceeded the selected capacity profile.
    PartitionExceedsCapacity,
    /// A coefficient artifact exceeded the profile entry envelope.
    CoefficientEntriesExceedCapacity,
    /// A partition requirement byte did not name the mandatory contract.
    UnknownPartitionRequirement,
    /// A claim-basis kind byte was not defined.
    UnknownClaimBasisKind,
    /// A redemption-rounding byte was not defined.
    UnknownRoundingMode,
    /// A payout denominator was zero or incompatible with the selected profile.
    InvalidPayoutDenominator,
    /// A coefficient degree was outside the supported zero-through-three profile.
    UnsupportedCoefficientDegree,
    /// Fields selected a semantically unsupported profile combination.
    UnsupportedProfileCombination,
    /// A coefficient count was not canonical for partition width and degree.
    NonCanonicalCoefficientCount,
    /// An artifact byte length did not match coefficient count and word width.
    ArtifactWidthMismatch,
    /// Linked Product records did not share the required content identity.
    IdentityMismatch,
    /// A terminal payoff-state kind byte was not defined.
    UnknownTerminalResultKind,
    /// A terminal resolution-kind byte was not defined.
    UnknownResolutionKind,
    /// A finite terminal selector was outside its committed partition.
    InvalidFiniteSelector,
    /// Tagged terminal-result fields were not in their unique canonical form.
    NonCanonicalTerminalResult,
}

/// Result alias for this contract crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Validated nonzero, opaque content identity.
///
/// Hash choice and content-address derivation are adapter policy. This type
/// only prevents the all-zero sentinel from becoming Product authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; CONTENT_ID_BYTES]);

impl ContentId {
    /// Validate and construct an opaque content identity.
    pub fn new(bytes: [u8; CONTENT_ID_BYTES]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentifier);
        }
        Ok(Self(bytes))
    }

    /// Decode one exact-width content identity.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CONTENT_ID_BYTES {
            return Err(Error::InvalidLength);
        }
        Self::new(array(bytes, 0)?)
    }

    /// Return the exact identity bytes.
    pub const fn to_bytes(self) -> [u8; CONTENT_ID_BYTES] {
        self.0
    }

    /// Borrow the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; CONTENT_ID_BYTES] {
        &self.0
    }
}

pub(crate) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn content_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

pub(crate) fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

pub(crate) fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    let reserved = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
    if reserved.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

pub(crate) fn canonical_pages(bytes: u32, page_payload_bytes: u32) -> Result<u32> {
    if bytes == 0 || page_payload_bytes == 0 {
        return Err(Error::ZeroCapacity);
    }
    let adjusted = bytes
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_div(page_payload_bytes)
        .ok_or(Error::ArithmeticOverflow)?;
    adjusted.checked_add(1).ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
pub(crate) fn id(fill: u8) -> ContentId {
    ContentId::new([fill; CONTENT_ID_BYTES]).expect("nonzero test identity")
}
