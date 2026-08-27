#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, hostile-decodable contracts for the `RelayedMainnetStateV1`
//! Source provider family.
//!
//! The family carries **observations** of another cluster's account bytes and
//! never an interpretation of them.  A signed message names pubkeys the relayer
//! did not choose, each account's `owner`, `lamports`, `executable` and exact
//! `data_len` as read, a release-pinned inline prefix, a SHA-256 digest over the
//! remainder, the finalized slot the read was taken at, and the genesis hash of
//! the cluster read.  Nothing is selected, scaled, compared, thresholded or
//! named by the relayer.
//!
//! This crate owns no Solana accounts, no CPI, no rent lookup, and **computes no
//! digests**: it hands a caller the exact canonical preimage to hash and then
//! compares the caller's result by equality.  Adapters supply the hash function,
//! the account privileges, and the clocks.
//!
//! Every byte coordinate in [`generated_relayed_abi`] is emitted from
//! `formal/dclutch-semantics/DClutchSemantics/RelayedMainnetStateV1Abi.lean`,
//! where the layout is specialized once, proved pairwise byte-disjoint, and
//! exercised against an accepted example and a refusal corpus.  Nothing in this
//! crate re-declares an offset.

// The generated module carries the Lean-derived coordinates verbatim.  The
// emitter writes constants, not prose; the documentation for what each
// coordinate means lives in the Lean module that specializes it and in the
// types below that consume it.
#[allow(missing_docs)]
mod generated_relayed_abi;

pub mod frame;
pub mod identity;
pub mod instruction;
pub mod record;
pub mod release;
pub mod signature;
pub mod wire;

pub use generated_relayed_abi::*;

/// Refusal returned by this crate's total parsers, constructors and evaluators.
///
/// The variants are deliberately narrow.  A cross-cluster substitution has to be
/// refusable *on the cluster identity specifically* — nothing else distinguishes
/// a mainnet venue `Program` account from its byte-identical devnet twin — so
/// [`Error::ObservedClusterMismatch`] exists rather than folding into a generic
/// binding refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The input did not have its required exact length.
    InvalidLength,
    /// The contract magic did not match.
    InvalidMagic,
    /// The schema release is not implemented.
    UnsupportedSchema,
    /// A reserved byte was nonzero.
    NonCanonicalReservedBytes,
    /// A required opaque identifier was all zero.
    ZeroIdentifier,
    /// An exact checked arithmetic operation overflowed.
    ArithmeticOverflow,
    /// The declared `message_len` did not equal the verified message length.
    MessageLengthMismatch,
    /// The declared inline width exceeded the account's own `data_len` or the
    /// release ceiling.
    InvalidInlineWidth,
    /// A body's recomputed tail digest did not equal the attested one.
    TailDigestMismatch,
    /// The observed cluster was not the release-pinned cluster.
    ObservedClusterMismatch,
    /// The attested account set was not the founding-time pinned set.
    AccountSetMismatch,
    /// An attested account's owner was not the pinned expected owner.
    ObservedOwnerMismatch,
    /// An attested account's key was not the pinned key for its position.
    ObservedKeyMismatch,
    /// The provider family named by the message was not this family.
    ProviderFamilyMismatch,
    /// The decoding-rules identity named by the message was not the pinned one.
    DecodingRulesMismatch,
    /// A set index or count was outside the admitted profile bound.
    InvalidSetGeometry,
    /// An observation slot was filled out of strictly increasing order, or twice.
    InvalidAppendOrder,
    /// The observed slot did not equal the record's own observed slot.
    ObservedSlotMismatch,
    /// The relayer key set was not canonical.
    NonCanonicalKeySet,
    /// The signing key was not a member of the pinned relayer key set.
    UnknownSigner,
    /// A key-set member sealed the same record twice.
    DuplicateSeal,
    /// The accumulated seal count was below the release threshold.
    SealThresholdNotReached,
    /// The record was not in a phase that admits the requested transition.
    InvalidRecordTransition,
    /// The record's persisted binding did not match the supplied authority.
    RecordBindingMismatch,
    /// The recomputed running set digest did not equal the sealed one.
    SetDigestMismatch,
    /// A persisted record's fields were not canonical for its phase.
    NonCanonicalRecord,
    /// The attested observation was older than the configured staleness bound.
    ObservationTooStale,
    /// The attested observation carried a time ahead of the admitted skew.
    ObservationFromTheFuture,
    /// The window's liveness grace did not cover the declared cluster skew.
    ClusterSkewExceedsWindowGrace,
    /// The immediately preceding instruction was not the native Ed25519 program.
    InvalidSignatureProgram,
    /// The Ed25519 instruction was not immediately before the current one.
    InvalidSignatureInstructionOrder,
    /// The Ed25519 instruction data did not have its exact descriptor shape.
    InvalidSignatureInstruction,
    /// A descriptor's message slice did not lie inside the current instruction.
    SignatureMessageMismatch,
    /// A descriptor named a signer other than the expected key-set member.
    SignatureSignerMismatch,
    /// A descriptor's signature bytes were entirely zero.
    ForgedSignature,
    /// An account frame had the wrong count, privileges, or an alias.
    InvalidAccountFrame,
    /// An instruction tag was not a recognized relay action.
    UnknownInstructionAction,
    /// The Loader V3 account bytes were not the expected variant.
    InvalidLoaderVariant,
    /// The Loader V3 upgrade-authority tag was neither zero nor one.
    InvalidUpgradeAuthorityTag,
    /// The supplied output buffer did not have the required exact length.
    OutputLength,
}

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact width of an opaque nonzero content identity.
pub const CONTENT_ID_BYTES: usize = 32;
/// [`MAX_RELAYER_KEYS_V1`] as the byte-width the record's counters use.
pub const MAX_RELAYER_KEYS_V1_U8: u8 = 5;
/// Exact width of a Solana address.
pub const ADDRESS_BYTES: usize = 32;

pub(crate) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    let source = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
    source.try_into().map_err(|_| Error::InvalidLength)
}

pub(crate) fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset.checked_add(length).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

pub(crate) fn one(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

pub(crate) fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

pub(crate) fn i64_at(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

pub(crate) fn i32_at(bytes: &[u8], offset: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(array(bytes, offset)?))
}

pub(crate) fn require_zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    if slice(bytes, offset, length)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

pub(crate) fn require_nonzero(identity: &[u8; 32]) -> Result<()> {
    if identity.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(())
}

pub(crate) fn is_zero(identity: &[u8; 32]) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

/// Exact-width copy into a caller-owned buffer.
///
/// Unlike the silently truncating `put` used elsewhere in this repository, an
/// out-of-range write is a refusal.  A generated offset should never be out of
/// range; if one ever is, a hard error is the only outcome that surfaces it.
pub(crate) fn put(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = offset.checked_add(input.len()).ok_or(Error::OutputLength)?;
    let destination = output.get_mut(offset..end).ok_or(Error::OutputLength)?;
    destination.copy_from_slice(input);
    Ok(())
}

pub(crate) fn header(bytes: &[u8], expected: usize, magic: [u8; 8]) -> Result<()> {
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    variable_header(bytes, magic)
}

pub(crate) fn variable_header(bytes: &[u8], magic: [u8; 8]) -> Result<()> {
    if bytes.get(..8) != Some(&magic) {
        return Err(Error::InvalidMagic);
    }
    if u16_at(bytes, 8)? != RELAYED_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

pub(crate) fn base<const N: usize>(magic: [u8; 8]) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    put(&mut out, 0, &magic)?;
    put(&mut out, 8, &RELAYED_SCHEMA_VERSION.to_le_bytes())?;
    Ok(out)
}

pub(crate) fn u16_from(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

pub(crate) fn u32_from(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

#[cfg(test)]
mod generated_layout_tests {
    use super::*;

    #[test]
    fn generated_widths_are_the_lean_specialized_widths() {
        assert_eq!(RELAYED_OBSERVATION_HEAD_BYTES, 112);
        assert_eq!(RELAYED_ATTESTATION_HEAD_BYTES, 156);
        assert_eq!(RELAYED_SEAL_BYTES, 156);
        assert_eq!(RELAYER_KEY_SET_BYTES, 176);
        assert_eq!(RELAYED_ADAPTER_CONFIG_BYTES, 80);
        assert_eq!(RELAYED_RECORD_HEADER_BYTES, 312);
        assert_eq!(RELAYED_RECORD_SLOT_BYTES, 560);
        assert_eq!(RELAYED_RECORD_MAX_BYTES, 4_792);
        assert_eq!(MAX_RELAYED_INLINE_BYTES_V1, 448);
        assert_eq!(MAX_RELAYED_ACCOUNTS_V1, 8);
        assert_eq!(MAX_RELAYER_KEYS_V1, 5);
    }

    #[test]
    fn the_two_clusters_are_distinguishable_only_by_genesis_hash() {
        assert_ne!(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            SOLANA_DEVNET_GENESIS_HASH_V1
        );
    }

    #[test]
    fn a_fully_inline_body_commits_to_the_empty_string_digest() {
        // Pinned rather than computed: this crate hashes nothing.  The value is
        // SHA-256 of the empty string and the adapter recomputes it for real.
        assert_eq!(
            SHA256_EMPTY_DIGEST.first(),
            Some(&0xe3),
            "the empty-string digest constant moved"
        );
    }

    #[test]
    fn out_of_range_writes_refuse_instead_of_truncating() {
        let mut buffer = [0u8; 4];
        assert_eq!(put(&mut buffer, 2, &[1, 2, 3]), Err(Error::OutputLength));
        assert_eq!(buffer, [0, 0, 0, 0], "a refused write changed bytes");
    }
}
