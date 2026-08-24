//! Borrowed wire contract for the Pyth resolve instruction.

use crate::{Error, Result, array, zero};

/// Exact fixed header width, excluding its nonempty borrowed body.
pub const RESOLVE_HEADER_BYTES: usize = 40;
/// Resolve-instruction magic.
pub const RESOLVE_MAGIC: [u8; 8] = *b"DCLTIX01";
/// Implemented resolve-instruction schema.
pub const RESOLVE_SCHEMA_VERSION: u16 = 1;
/// The only instruction tag owned by this module.
pub const RESOLVE_TAG: u8 = 1;

/// A parsed resolve instruction whose opaque body is borrowed from its input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveCategoricalPythV1<'a> {
    generation: u64,
    child_count: u64,
    body: &'a [u8],
}

impl<'a> ResolveCategoricalPythV1<'a> {
    /// Construct a borrowed resolve instruction, rejecting an empty body.
    pub fn new(generation: u64, child_count: u64, body: &'a [u8]) -> Result<Self> {
        if body.is_empty() {
            return Err(Error::EmptyBody);
        }
        if body.len() > usize::from(u16::MAX) {
            return Err(Error::BodyTooLarge);
        }
        Ok(Self {
            generation,
            child_count,
            body,
        })
    }

    /// Decode one canonical instruction, requiring exact end of input.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < RESOLVE_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != RESOLVE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != RESOLVE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if *bytes.get(10).ok_or(Error::InvalidLength)? != RESOLVE_TAG {
            return Err(Error::InvalidInstructionTag);
        }
        if *bytes.get(11).ok_or(Error::InvalidLength)? != 0 {
            return Err(Error::InvalidInstructionFlags);
        }
        if !zero(bytes.get(12..16).ok_or(Error::InvalidLength)?)
            || !zero(bytes.get(34..40).ok_or(Error::InvalidLength)?)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let body_length = usize::from(u16::from_le_bytes(array::<2>(bytes, 32)?));
        if body_length == 0 {
            return Err(Error::EmptyBody);
        }
        let expected = RESOLVE_HEADER_BYTES
            .checked_add(body_length)
            .ok_or(Error::BodyLengthMismatch)?;
        if bytes.len() != expected {
            return Err(Error::BodyLengthMismatch);
        }
        Self::new(
            u64::from_le_bytes(array(bytes, 16)?),
            u64::from_le_bytes(array(bytes, 24)?),
            bytes
                .get(RESOLVE_HEADER_BYTES..)
                .ok_or(Error::InvalidLength)?,
        )
    }

    /// Encode exactly into a caller-owned buffer.  All validation occurs before
    /// mutation, so every refusal leaves `output` untouched.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let expected = RESOLVE_HEADER_BYTES
            .checked_add(self.body.len())
            .ok_or(Error::BodyTooLarge)?;
        if self.body.is_empty() {
            return Err(Error::EmptyBody);
        }
        if self.body.len() > usize::from(u16::MAX) {
            return Err(Error::BodyTooLarge);
        }
        if output.len() != expected {
            return Err(Error::OutputLength);
        }
        let body_length = u16::try_from(self.body.len()).map_err(|_| Error::BodyTooLarge)?;
        let header = output
            .get_mut(..RESOLVE_HEADER_BYTES)
            .ok_or(Error::OutputLength)?;
        header
            .get_mut(..8)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(&RESOLVE_MAGIC);
        header
            .get_mut(8..10)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(&RESOLVE_SCHEMA_VERSION.to_le_bytes());
        *header.get_mut(10).ok_or(Error::OutputLength)? = RESOLVE_TAG;
        *header.get_mut(11).ok_or(Error::OutputLength)? = 0;
        header.get_mut(12..16).ok_or(Error::OutputLength)?.fill(0);
        header
            .get_mut(16..24)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(&self.generation.to_le_bytes());
        header
            .get_mut(24..32)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(&self.child_count.to_le_bytes());
        header
            .get_mut(32..34)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(&body_length.to_le_bytes());
        header.get_mut(34..40).ok_or(Error::OutputLength)?.fill(0);
        output
            .get_mut(RESOLVE_HEADER_BYTES..)
            .ok_or(Error::OutputLength)?
            .copy_from_slice(self.body);
        Ok(())
    }

    /// Return immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return immutable direct-child count.
    pub const fn child_count(&self) -> u64 {
        self.child_count
    }
    /// Borrow the opaque nonempty adapter body.
    pub const fn body(&self) -> &'a [u8] {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_round_trip_and_borrowed_body() {
        let instruction = ResolveCategoricalPythV1::new(0x0807_0605_0403_0201, 9, &[4, 5])
            .expect("valid instruction");
        let mut encoded = [0; 42];
        assert_eq!(instruction.encode(&mut encoded), Ok(()));
        assert_eq!(encoded.get(16..24), Some(&[1, 2, 3, 4, 5, 6, 7, 8][..]));
        assert_eq!(ResolveCategoricalPythV1::decode(&encoded), Ok(instruction));
    }

    #[test]
    fn hostile_headers_and_lengths_refuse_without_mutation() {
        let instruction = ResolveCategoricalPythV1::new(1, 2, &[3]).expect("valid instruction");
        let mut encoded = [0; 41];
        assert_eq!(instruction.encode(&mut encoded), Ok(()));
        for length in 0..RESOLVE_HEADER_BYTES {
            if let Some(short) = encoded.get(..length) {
                assert_eq!(
                    ResolveCategoricalPythV1::decode(short),
                    Err(Error::InvalidLength)
                );
            }
        }
        let mut empty = encoded;
        if let Some(slot) = empty.get_mut(32) {
            *slot = 0;
        }
        if let Some(empty_body) = empty.get(..RESOLVE_HEADER_BYTES) {
            assert_eq!(
                ResolveCategoricalPythV1::decode(empty_body),
                Err(Error::EmptyBody)
            );
        }
        let mut long = [0; 42];
        if let Some(prefix) = long.get_mut(..41) {
            prefix.copy_from_slice(&encoded);
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&long),
            Err(Error::BodyLengthMismatch)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(0) {
            *slot = 0;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::InvalidMagic)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(8) {
            *slot = 2;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(10) {
            *slot = 0;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::InvalidInstructionTag)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(11) {
            *slot = 1;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::InvalidInstructionFlags)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(12) {
            *slot = 1;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut changed = encoded;
        if let Some(slot) = changed.get_mut(32) {
            *slot = 2;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&changed),
            Err(Error::BodyLengthMismatch)
        );
        let before = [0x5a; 40];
        let mut wrong = before;
        assert_eq!(instruction.encode(&mut wrong), Err(Error::OutputLength));
        assert_eq!(wrong, before);
        assert_eq!(
            ResolveCategoricalPythV1::new(1, 2, &[]),
            Err(Error::EmptyBody)
        );
    }

    #[test]
    fn excessive_declared_body_is_refused() {
        let instruction = ResolveCategoricalPythV1::new(1, 2, &[3]).expect("valid instruction");
        let mut encoded = [0; 41];
        assert_eq!(instruction.encode(&mut encoded), Ok(()));
        if let Some(slot) = encoded.get_mut(32) {
            *slot = 0xff;
        }
        if let Some(slot) = encoded.get_mut(33) {
            *slot = 0xff;
        }
        assert_eq!(
            ResolveCategoricalPythV1::decode(&encoded),
            Err(Error::BodyLengthMismatch)
        );
    }
}
