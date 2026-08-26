//! Canonical immutable selection record for Token behavior V2.
//!
//! The record is deliberately not inferred from a token name, symbol, Mint
//! address, or caller hint. A composing adapter must source `realm` from the
//! authenticated immutable Realm and `release_set` from the authenticated
//! Market/capability release selection before accepting this record.

use core::convert::TryInto;

use crate::{Address, Error, Result, TOKEN_2022_BEHAVIOR_PROFILE_ID_V2, TOKEN_2022_PROGRAM_ID};

/// Exact byte width of [`TokenBehaviorSelectionV2`].
pub const TOKEN_BEHAVIOR_SELECTION_BYTES_V2: usize = 144;
/// Canonical selection-record magic.
pub const TOKEN_BEHAVIOR_SELECTION_MAGIC_V2: [u8; 8] = *b"DCLTTBS2";
/// Implemented selection-record schema.
pub const TOKEN_BEHAVIOR_SELECTION_SCHEMA_V2: u16 = 2;
/// Canonical semantic schema preimage for selection records.
pub const TOKEN_BEHAVIOR_SELECTION_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/token-behavior-selection-schema/v2|bytes=144|fields=magic,version,reserved,realm,release-set,profile-id,token-program|authority=authenticated-immutable-realm+release-set";
/// SHA-256 identity of [`TOKEN_BEHAVIOR_SELECTION_SCHEMA_PREIMAGE_V2`].
pub const TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2: Address = [
    0xb4, 0x92, 0xdc, 0x12, 0x85, 0x7a, 0x10, 0xe9, 0xad, 0x28, 0xc5, 0x85, 0x5c, 0x69, 0x55, 0xa0,
    0x10, 0x7f, 0x58, 0x17, 0x35, 0x72, 0x71, 0x34, 0xc0, 0x21, 0xd4, 0xdf, 0xff, 0x9e, 0xa0, 0xe8,
];

const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const REALM_OFFSET: usize = 16;
const RELEASE_SET_OFFSET: usize = 48;
const PROFILE_OFFSET: usize = 80;
const PROGRAM_OFFSET: usize = 112;

/// Immutable Realm/release-selected Token behavior identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenBehaviorSelectionV2 {
    realm: Address,
    release_set: Address,
}

impl TokenBehaviorSelectionV2 {
    /// Construct the sole implemented V2 selection from already-authenticated
    /// immutable Realm and release-set identities.
    pub fn new(realm: Address, release_set: Address) -> Result<Self> {
        if realm == [0; 32] || release_set == [0; 32] || realm == release_set {
            return Err(Error::InvalidAdapterRelease);
        }
        Ok(Self { realm, release_set })
    }

    /// Decode the exact canonical selection. Profile and program substitutions,
    /// reserved bits, and trailing storage are refused.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != TOKEN_BEHAVIOR_SELECTION_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(TOKEN_BEHAVIOR_SELECTION_MAGIC_V2.as_slice())
            || read_u16(bytes, 8)? != TOKEN_BEHAVIOR_SELECTION_SCHEMA_V2
            || bytes
                .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
                .ok_or(Error::InvalidAdapterRelease)?
                .iter()
                .any(|byte| *byte != 0)
            || bytes.get(PROFILE_OFFSET..PROFILE_OFFSET + 32)
                != Some(TOKEN_2022_BEHAVIOR_PROFILE_ID_V2.as_slice())
            || bytes.get(PROGRAM_OFFSET..PROGRAM_OFFSET + 32)
                != Some(TOKEN_2022_PROGRAM_ID.as_slice())
        {
            return Err(Error::InvalidAdapterRelease);
        }
        Self::new(
            read_address(bytes, REALM_OFFSET)?,
            read_address(bytes, RELEASE_SET_OFFSET)?,
        )
    }

    /// Decode and bind the record to the Realm and release-set identities
    /// already authenticated by the composing adapter.
    ///
    /// This is the admission entry point. [`Self::decode`] exists for tooling
    /// that needs to inspect a record, but does not itself establish where the
    /// two authority identities came from.
    pub fn decode_for_authenticated_selection(
        bytes: &[u8],
        authenticated_realm: Address,
        authenticated_release_set: Address,
    ) -> Result<Self> {
        let selection = Self::decode(bytes)?;
        if selection.realm != authenticated_realm
            || selection.release_set != authenticated_release_set
        {
            return Err(Error::InvalidAdapterRelease);
        }
        Ok(selection)
    }

    /// Encode the canonical selection for embedding in an immutable descriptor
    /// or release-selected admission artifact.
    pub fn to_bytes(self) -> [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2] {
        let mut output = [0; TOKEN_BEHAVIOR_SELECTION_BYTES_V2];
        put(&mut output, 0, &TOKEN_BEHAVIOR_SELECTION_MAGIC_V2);
        put(
            &mut output,
            8,
            &TOKEN_BEHAVIOR_SELECTION_SCHEMA_V2.to_le_bytes(),
        );
        put(&mut output, REALM_OFFSET, &self.realm);
        put(&mut output, RELEASE_SET_OFFSET, &self.release_set);
        put(
            &mut output,
            PROFILE_OFFSET,
            &TOKEN_2022_BEHAVIOR_PROFILE_ID_V2,
        );
        put(&mut output, PROGRAM_OFFSET, &TOKEN_2022_PROGRAM_ID);
        output
    }

    /// Return the authenticated immutable Realm identity.
    pub const fn realm(self) -> Address {
        self.realm
    }

    /// Return the authenticated release-set identity.
    pub const fn release_set(self) -> Address {
        self.release_set
    }

    /// Return the exact behavior profile selected by this schema.
    pub const fn profile_id(self) -> Address {
        TOKEN_2022_BEHAVIOR_PROFILE_ID_V2
    }

    /// Return the exact Token program selected by this schema.
    pub const fn token_program(self) -> Address {
        TOKEN_2022_PROGRAM_ID
    }
}

fn read_address(bytes: &[u8], offset: usize) -> Result<Address> {
    bytes
        .get(offset..offset.checked_add(32).ok_or(Error::InvalidAdapterRelease)?)
        .ok_or(Error::InvalidAdapterRelease)?
        .try_into()
        .map_err(|_| Error::InvalidAdapterRelease)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    bytes
        .get(offset..offset.checked_add(2).ok_or(Error::InvalidAdapterRelease)?)
        .ok_or(Error::InvalidAdapterRelease)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| Error::InvalidAdapterRelease)
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use sha2::Digest;

    use super::*;

    fn id(value: u8) -> Address {
        [value; 32]
    }

    #[test]
    fn immutable_realm_release_selection_round_trips() {
        let selection = TokenBehaviorSelectionV2::new(id(1), id(2)).expect("selection");
        assert_eq!(
            TokenBehaviorSelectionV2::decode(&selection.to_bytes()),
            Ok(selection)
        );
        assert_eq!(selection.profile_id(), TOKEN_2022_BEHAVIOR_PROFILE_ID_V2);
        assert_eq!(selection.token_program(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(
            sha2::Sha256::digest(TOKEN_BEHAVIOR_SELECTION_SCHEMA_PREIMAGE_V2).as_slice(),
            TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        );
    }

    #[test]
    fn caller_profile_program_and_reserved_substitutions_refuse() {
        let canonical = TokenBehaviorSelectionV2::new(id(1), id(2))
            .expect("selection")
            .to_bytes();
        for offset in [RESERVED_OFFSET, PROFILE_OFFSET, PROGRAM_OFFSET] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("field byte") ^= 0xff;
            assert!(TokenBehaviorSelectionV2::decode(&hostile).is_err());
        }
        for offset in [REALM_OFFSET, RELEASE_SET_OFFSET] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("authority byte") ^= 0xff;
            assert_eq!(
                TokenBehaviorSelectionV2::decode_for_authenticated_selection(
                    &hostile,
                    id(1),
                    id(2),
                ),
                Err(Error::InvalidAdapterRelease)
            );
        }
        assert_eq!(
            TokenBehaviorSelectionV2::decode_for_authenticated_selection(&canonical, id(1), id(2),),
            TokenBehaviorSelectionV2::new(id(1), id(2))
        );
        assert_eq!(
            TokenBehaviorSelectionV2::new([0; 32], id(2)),
            Err(Error::InvalidAdapterRelease)
        );
        assert_eq!(
            TokenBehaviorSelectionV2::new(id(1), id(1)),
            Err(Error::InvalidAdapterRelease)
        );
        assert_eq!(
            TokenBehaviorSelectionV2::decode(canonical.get(..canonical.len() - 1).expect("prefix")),
            Err(Error::InvalidLength)
        );
    }
}
