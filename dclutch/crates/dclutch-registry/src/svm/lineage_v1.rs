//! The declaration that names one release set's successor.
//!
//! This wire carries **no arguments at all**. Both endpoints, the moved-role
//! mask and every consenting authority are read out of the accounts, so there
//! is nothing supplied that could disagree with the chain.
//!
//! It needs its own magic rather than a `RegistryInstructionV1` action,
//! because that magic is shared byte-for-byte with the record family, which
//! owns every action from `2` upward -- the Registry side of the split has
//! exactly actions `0` and `1`, and both are spent.

use crate::release_set::EXECUTION_ROLE_COUNT_V1;

use crate::svm::{Error, Result};

/// Canonical successor-declaration magic.
pub const DECLARE_SUCCESSOR_MAGIC_V1: [u8; 8] = *b"DCLRLND1";
/// Exact successor-declaration instruction width.
pub const DECLARE_SUCCESSOR_BYTES_V1: usize = 16;
/// Implemented successor-declaration wire schema.
pub const DECLARE_SUCCESSOR_SCHEMA_V1: u16 = 1;

/// Payer: signs, funds the lineage record.
pub const DECLARE_SUCCESSOR_PAYER_ACCOUNT_V1: usize = 0;
/// The pristine lineage record this declaration creates.
pub const DECLARE_SUCCESSOR_LINEAGE_ACCOUNT_V1: usize = 1;
/// The predecessor's activation cache: read for its bindings, never admitted.
pub const DECLARE_SUCCESSOR_PREDECESSOR_CACHE_ACCOUNT_V1: usize = 2;
/// The successor's activation cache: bindings, slots and authorities.
pub const DECLARE_SUCCESSOR_SUCCESSOR_CACHE_ACCOUNT_V1: usize = 3;
/// First per-role consenting-authority slot, in canonical role order.
pub const DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1: usize = 4;
/// System program, for the record's creation.
pub const DECLARE_SUCCESSOR_SYSTEM_ACCOUNT_V1: usize =
    DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1 + EXECUTION_ROLE_COUNT_V1;
/// Rent sysvar.
pub const DECLARE_SUCCESSOR_RENT_ACCOUNT_V1: usize = DECLARE_SUCCESSOR_SYSTEM_ACCOUNT_V1 + 1;
/// Exact account count consumed by one successor declaration.
pub const DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1: usize = DECLARE_SUCCESSOR_RENT_ACCOUNT_V1 + 1;

// The authority block is one slot per role, so its width is the role count and
// never a literal beside it. A sixth role moves System and Rent by itself.
const _: () = assert!(
    DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1 == 11,
    "the declaration frame is eleven accounts under the five-role profile"
);

const SCHEMA_OFFSET: usize = 8;
const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;

const _: () = assert!(
    RESERVED_OFFSET + RESERVED_BYTES == DECLARE_SUCCESSOR_BYTES_V1,
    "the declaration wire must be fully described by its own layout"
);

/// The one argument-free instruction that declares a release set's successor.
///
/// It is a unit type on purpose. A field here would be a fact the caller
/// chooses, and every fact this route needs is already on the chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclareSuccessorV1;

impl DeclareSuccessorV1 {
    /// Hostile-decode one exact successor declaration.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DECLARE_SUCCESSOR_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(DECLARE_SUCCESSOR_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if crate::svm::read_u16(bytes, SCHEMA_OFFSET)? != DECLARE_SUCCESSOR_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        Ok(Self)
    }

    /// Encode the one canonical successor declaration.
    #[must_use]
    pub fn to_bytes() -> [u8; DECLARE_SUCCESSOR_BYTES_V1] {
        let mut output = [0_u8; DECLARE_SUCCESSOR_BYTES_V1];
        if let Some(magic) = output.get_mut(..8) {
            magic.copy_from_slice(&DECLARE_SUCCESSOR_MAGIC_V1);
        }
        if let Some(schema) = output.get_mut(SCHEMA_OFFSET..SCHEMA_OFFSET + 2) {
            schema.copy_from_slice(&DECLARE_SUCCESSOR_SCHEMA_V1.to_le_bytes());
        }
        output
    }
}
