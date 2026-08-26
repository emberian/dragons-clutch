//! Canonical chain-observed Rational Representation replay state.

use crate::{Error, Result, array_at, is_zero, put, require_zero, u64_at};

/// Exact Rational replay account width.
pub const RATIONAL_REPLAY_BYTES_V2: usize = 88;
/// Canonical Rational replay account magic.
pub const RATIONAL_REPLAY_MAGIC_V2: [u8; 8] = *b"DCRRREP2";
/// Implemented Rational replay account version.
pub const RATIONAL_REPLAY_VERSION_V2: u16 = 2;

const VERSION_OFFSET: usize = 8;
const RESERVED_OFFSET: usize = 10;
const DESCRIPTOR_OFFSET: usize = 16;
const ACTOR_OFFSET: usize = 48;
const REVISION_OFFSET: usize = 80;

/// One canonical per-descriptor, per-actor replay cursor.
///
/// The Claims adapter owns the account and advances `revision` exactly once
/// after all Claims, Token, and Custody postconditions pass. Operators decode
/// this state rather than accepting an independently supplied replay scalar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalReplayV2 {
    descriptor: [u8; 32],
    actor: [u8; 32],
    revision: u64,
}

impl RationalReplayV2 {
    /// Construct one canonical replay observation.
    pub fn new(descriptor: [u8; 32], actor: [u8; 32], revision: u64) -> Result<Self> {
        if is_zero(descriptor) || is_zero(actor) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            descriptor,
            actor,
            revision,
        })
    }

    /// Hostile-decode one exact replay account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RATIONAL_REPLAY_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(bytes, 0)? != RATIONAL_REPLAY_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array_at(bytes, VERSION_OFFSET)?) != RATIONAL_REPLAY_VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, RESERVED_OFFSET, 6)?;
        Self::new(
            array_at(bytes, DESCRIPTOR_OFFSET)?,
            array_at(bytes, ACTOR_OFFSET)?,
            u64_at(bytes, REVISION_OFFSET)?,
        )
    }

    /// Encode into one exact caller-owned replay buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != RATIONAL_REPLAY_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &RATIONAL_REPLAY_MAGIC_V2)?;
        put(
            output,
            VERSION_OFFSET,
            &RATIONAL_REPLAY_VERSION_V2.to_le_bytes(),
        )?;
        put(output, DESCRIPTOR_OFFSET, &self.descriptor)?;
        put(output, ACTOR_OFFSET, &self.actor)?;
        put(output, REVISION_OFFSET, &self.revision.to_le_bytes())
    }

    /// Return exact canonical replay bytes.
    pub fn to_bytes(self) -> [u8; RATIONAL_REPLAY_BYTES_V2] {
        let mut output = [0; RATIONAL_REPLAY_BYTES_V2];
        output[..8].copy_from_slice(&RATIONAL_REPLAY_MAGIC_V2);
        output[VERSION_OFFSET..VERSION_OFFSET + 2]
            .copy_from_slice(&RATIONAL_REPLAY_VERSION_V2.to_le_bytes());
        output[DESCRIPTOR_OFFSET..DESCRIPTOR_OFFSET + 32].copy_from_slice(&self.descriptor);
        output[ACTOR_OFFSET..ACTOR_OFFSET + 32].copy_from_slice(&self.actor);
        output[REVISION_OFFSET..REVISION_OFFSET + 8].copy_from_slice(&self.revision.to_le_bytes());
        output
    }

    /// Return the immutable descriptor identity.
    pub const fn descriptor(self) -> [u8; 32] {
        self.descriptor
    }

    /// Return the immutable actor identity.
    pub const fn actor(self) -> [u8; 32] {
        self.actor
    }

    /// Return the current optimistic replay revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Authenticate this observation against its expected PDA coordinates.
    pub fn authenticate(self, descriptor: [u8; 32], actor: [u8; 32]) -> Result<Self> {
        if self.descriptor != descriptor || self.actor != actor {
            return Err(Error::ProjectionMismatch);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn replay_roundtrip_and_coordinate_join_are_exact() {
        let replay = RationalReplayV2::new(id(1), id(2), 7).expect("replay");
        let bytes = replay.to_bytes();
        assert_eq!(RationalReplayV2::decode(&bytes), Ok(replay));
        assert_eq!(replay.authenticate(id(1), id(2)), Ok(replay));
        assert_eq!(
            replay.authenticate(id(3), id(2)),
            Err(Error::ProjectionMismatch)
        );
    }

    #[test]
    fn replay_hostile_header_and_identity_mutations_refuse() {
        let canonical = RationalReplayV2::new(id(1), id(2), u64::MAX)
            .expect("replay")
            .to_bytes();
        for offset in [0_usize, VERSION_OFFSET, RESERVED_OFFSET] {
            let mut mutated = canonical;
            *mutated.get_mut(offset).expect("fixture offset") ^= 1;
            assert!(RationalReplayV2::decode(&mutated).is_err(), "offset {offset}");
        }
        for offset in [DESCRIPTOR_OFFSET, ACTOR_OFFSET] {
            let mut mutated = canonical;
            mutated
                .get_mut(offset..offset + 32)
                .expect("fixture identity")
                .fill(0);
            assert!(RationalReplayV2::decode(&mutated).is_err(), "offset {offset}");
        }
        assert_eq!(
            RationalReplayV2::decode(
                canonical
                    .get(..canonical.len() - 1)
                    .expect("truncated fixture"),
            ),
            Err(Error::InvalidLength)
        );
    }
}
