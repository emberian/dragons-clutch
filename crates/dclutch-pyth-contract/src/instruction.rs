//! Borrowed wire contract for the Pyth resolve instruction.

use crate::{Error, Result, array, zero};

/// Exact fixed header width, excluding its nonempty borrowed body.
pub const RESOLVE_HEADER_BYTES: usize = 40;
/// Resolve-instruction magic.
pub const RESOLVE_MAGIC: [u8; 8] = *b"DCLTIX01";
/// Implemented resolve-instruction schema.
pub const RESOLVE_SCHEMA_VERSION: u16 = 1;
/// Price-resolution instruction tag.
pub const RESOLVE_TAG: u8 = 1;
/// Permissionless failure-resolution instruction tag.
pub const RESOLVE_FAILURE_TAG: u8 = 2;
/// Exact width of a permissionless failure-resolution instruction.
pub const RESOLVE_FAILURE_BYTES: usize = 32;

/// One exactly decoded categorical-resolution instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveCategoricalInstructionV1<'a> {
    /// Post and fold a nonempty Pyth provider body.
    Pyth(ResolveCategoricalPythV1<'a>),
    /// Resolve to the failure outcome after the policy deadline.
    Failure(ResolveCategoricalFailureV1),
}

impl<'a> ResolveCategoricalInstructionV1<'a> {
    /// Decode one complete canonical instruction and dispatch by its wire tag.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        let tag = *bytes.get(10).ok_or(Error::InvalidLength)?;
        match tag {
            RESOLVE_TAG => ResolveCategoricalPythV1::decode(bytes).map(Self::Pyth),
            RESOLVE_FAILURE_TAG => ResolveCategoricalFailureV1::decode(bytes).map(Self::Failure),
            _ => Err(Error::InvalidInstructionTag),
        }
    }
}

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

/// Fixed, body-free instruction for permissionless resolution to failure.
///
/// The adapter derives the failure outcome and deadline from the Market's
/// immutable policy.  Consequently this wire carries only replay facts shared
/// with the price path and cannot name a provider, source, or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveCategoricalFailureV1 {
    generation: u64,
    child_count: u64,
}

impl ResolveCategoricalFailureV1 {
    /// Construct a permissionless failure-resolution instruction.
    pub const fn new(generation: u64, child_count: u64) -> Self {
        Self {
            generation,
            child_count,
        }
    }

    /// Decode one canonical body-free instruction, requiring exact input length.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RESOLVE_FAILURE_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != RESOLVE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != RESOLVE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if *bytes.get(10).ok_or(Error::InvalidLength)? != RESOLVE_FAILURE_TAG {
            return Err(Error::InvalidInstructionTag);
        }
        if *bytes.get(11).ok_or(Error::InvalidLength)? != 0 {
            return Err(Error::InvalidInstructionFlags);
        }
        if !zero(bytes.get(12..16).ok_or(Error::InvalidLength)?) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        Ok(Self::new(
            u64::from_le_bytes(array(bytes, 16)?),
            u64::from_le_bytes(array(bytes, 24)?),
        ))
    }

    /// Encode exactly into a caller-owned buffer.  Every refusal leaves
    /// `output` untouched.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != RESOLVE_FAILURE_BYTES {
            return Err(Error::OutputLength);
        }

        let mut encoded = [0; RESOLVE_FAILURE_BYTES];
        encoded[..8].copy_from_slice(&RESOLVE_MAGIC);
        encoded[8..10].copy_from_slice(&RESOLVE_SCHEMA_VERSION.to_le_bytes());
        encoded[10] = RESOLVE_FAILURE_TAG;
        encoded[16..24].copy_from_slice(&self.generation.to_le_bytes());
        encoded[24..32].copy_from_slice(&self.child_count.to_le_bytes());
        output.copy_from_slice(&encoded);
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

    #[test]
    fn failure_is_exact_body_free_and_round_trips() {
        let instruction =
            ResolveCategoricalFailureV1::new(0x0807_0605_0403_0201, 0x1817_1615_1413_1211);
        let mut encoded = [0xa5; RESOLVE_FAILURE_BYTES];
        assert_eq!(instruction.encode(&mut encoded), Ok(()));
        assert_eq!(encoded.get(..8), Some(&RESOLVE_MAGIC[..]));
        assert_eq!(encoded.get(8..10), Some(&[1, 0][..]));
        assert_eq!(encoded.get(10), Some(&RESOLVE_FAILURE_TAG));
        assert_eq!(encoded.get(11..16), Some(&[0; 5][..]));
        assert_eq!(encoded.get(16..24), Some(&[1, 2, 3, 4, 5, 6, 7, 8][..]));
        assert_eq!(
            encoded.get(24..32),
            Some(&[17, 18, 19, 20, 21, 22, 23, 24][..])
        );
        assert_eq!(
            ResolveCategoricalFailureV1::decode(&encoded),
            Ok(instruction)
        );
        assert_eq!(instruction.generation(), 0x0807_0605_0403_0201);
        assert_eq!(instruction.child_count(), 0x1817_1615_1413_1211);
    }

    #[test]
    fn failure_hostile_lengths_and_headers_refuse() {
        let instruction = ResolveCategoricalFailureV1::new(1, 2);
        let mut encoded = [0; RESOLVE_FAILURE_BYTES];
        assert_eq!(instruction.encode(&mut encoded), Ok(()));

        for length in 0..RESOLVE_FAILURE_BYTES {
            if let Some(short) = encoded.get(..length) {
                assert_eq!(
                    ResolveCategoricalFailureV1::decode(short),
                    Err(Error::InvalidLength)
                );
            }
        }
        let mut long = [0; RESOLVE_FAILURE_BYTES + 1];
        if let Some(prefix) = long.get_mut(..RESOLVE_FAILURE_BYTES) {
            prefix.copy_from_slice(&encoded);
        }
        assert_eq!(
            ResolveCategoricalFailureV1::decode(&long),
            Err(Error::InvalidLength)
        );

        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (10, Error::InvalidInstructionTag),
            (11, Error::InvalidInstructionFlags),
            (12, Error::NonCanonicalReservedBytes),
            (15, Error::NonCanonicalReservedBytes),
        ] {
            let mut changed = encoded;
            if let Some(slot) = changed.get_mut(offset) {
                *slot = slot.wrapping_add(1);
            }
            assert_eq!(ResolveCategoricalFailureV1::decode(&changed), Err(expected));
        }
    }

    #[test]
    fn failure_encode_refusals_are_atomic() {
        let instruction = ResolveCategoricalFailureV1::new(1, 2);
        let before_short = [0x5a; RESOLVE_FAILURE_BYTES - 1];
        let mut short = before_short;
        assert_eq!(instruction.encode(&mut short), Err(Error::OutputLength));
        assert_eq!(short, before_short);

        let before_long = [0x5a; RESOLVE_FAILURE_BYTES + 1];
        let mut long = before_long;
        assert_eq!(instruction.encode(&mut long), Err(Error::OutputLength));
        assert_eq!(long, before_long);
    }

    #[test]
    fn exact_dispatch_preserves_price_tag_one_and_accepts_failure_tag_two() {
        let price = ResolveCategoricalPythV1::new(7, 8, &[9]).expect("nonempty body");
        let mut price_bytes = [0; RESOLVE_HEADER_BYTES + 1];
        assert_eq!(price.encode(&mut price_bytes), Ok(()));
        assert_eq!(price_bytes.get(10), Some(&1));
        assert_eq!(
            ResolveCategoricalInstructionV1::decode(&price_bytes),
            Ok(ResolveCategoricalInstructionV1::Pyth(price))
        );

        let failure = ResolveCategoricalFailureV1::new(7, 8);
        let mut failure_bytes = [0; RESOLVE_FAILURE_BYTES];
        assert_eq!(failure.encode(&mut failure_bytes), Ok(()));
        assert_eq!(failure_bytes.get(10), Some(&2));
        assert_eq!(
            ResolveCategoricalInstructionV1::decode(&failure_bytes),
            Ok(ResolveCategoricalInstructionV1::Failure(failure))
        );

        let mut unknown = failure_bytes;
        if let Some(tag) = unknown.get_mut(10) {
            *tag = 3;
        }
        assert_eq!(
            ResolveCategoricalInstructionV1::decode(&unknown),
            Err(Error::InvalidInstructionTag)
        );
        if let Some(short) = unknown.get(..10) {
            assert_eq!(
                ResolveCategoricalInstructionV1::decode(short),
                Err(Error::InvalidLength)
            );
        }
    }
}
