//! Exact hostile-decodable wires for transient Market-opening readiness.
//!
//! These wires contain only replay facts.  They deliberately carry neither a
//! caller readiness assertion nor funding/allocation amounts: the adapter
//! obtains readiness from the canonical manifest and physical funding account.

use core::convert::TryInto;

/// Shared header width for Market-opening readiness instructions.
pub const READINESS_INSTRUCTION_HEADER_BYTES: usize = 16;
/// Exact byte width of a readiness Begin instruction.
pub const BEGIN_MARKET_OPENING_READINESS_BYTES: usize = 32;
/// Exact byte width of a readiness Advance instruction.
pub const ADVANCE_MARKET_OPENING_READINESS_BYTES: usize = 32;
/// Canonical Market-opening readiness instruction magic.
pub const READINESS_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTRDY1";
/// Implemented readiness-instruction schema version.
pub const READINESS_INSTRUCTION_SCHEMA_VERSION: u16 = 1;

const SCHEMA_OFFSET: usize = 8;
const TAG_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const HEADER_RESERVED_BYTES: usize = 4;
const GENERATION_OFFSET: usize = READINESS_INSTRUCTION_HEADER_BYTES;
const BEGIN_CHILD_COUNT_OFFSET: usize = 24;
const ADVANCE_ENTRY_INDEX_OFFSET: usize = 24;
const ADVANCE_RESERVED_OFFSET: usize = 26;
const ADVANCE_RESERVED_BYTES: usize = 6;

/// Refusal from the exact readiness-instruction decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessInstructionError {
    /// Input did not have the one exact semantic width.
    InvalidLength,
    /// Magic bytes did not identify this instruction family.
    InvalidMagic,
    /// The encoded instruction schema is not implemented.
    UnsupportedSchema,
    /// The action discriminator is not defined by this instruction family.
    UnknownAction,
    /// Flags or reserved bytes were not canonical zeroes.
    NonCanonicalReservedBytes,
}

/// Result alias for readiness instruction decoding and encoding.
pub type Result<T> = core::result::Result<T, ReadinessInstructionError>;

/// Canonical Market-opening readiness instruction action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessInstructionTagV1 {
    /// Create the one transient readiness child for a founding Market.
    Begin,
    /// Validate and consume exactly one canonical capability funding state.
    Advance,
}

impl ReadinessInstructionTagV1 {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Begin),
            2 => Ok(Self::Advance),
            _ => Err(ReadinessInstructionError::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Begin => 1,
            Self::Advance => 2,
        }
    }
}

/// One exact decoded readiness instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessInstructionV1 {
    /// Begin one transient readiness child.
    Begin(BeginMarketOpeningReadinessV1),
    /// Advance that child by one manifest entry.
    Advance(AdvanceMarketOpeningReadinessV1),
}

impl ReadinessInstructionV1 {
    /// Hostile-decode one exact Begin or Advance instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match decode_header(bytes)? {
            ReadinessInstructionTagV1::Begin => {
                BeginMarketOpeningReadinessV1::decode(bytes).map(Self::Begin)
            }
            ReadinessInstructionTagV1::Advance => {
                AdvanceMarketOpeningReadinessV1::decode(bytes).map(Self::Advance)
            }
        }
    }

    /// Return this instruction's canonical action tag.
    pub const fn tag(self) -> ReadinessInstructionTagV1 {
        match self {
            Self::Begin(_) => ReadinessInstructionTagV1::Begin,
            Self::Advance(_) => ReadinessInstructionTagV1::Advance,
        }
    }
}

/// Begin one transient Market-opening readiness child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginMarketOpeningReadinessV1 {
    generation: u64,
    expected_market_child_count: u64,
}

impl BeginMarketOpeningReadinessV1 {
    /// Construct from the two immutable/replay facts required for Begin.
    pub const fn new(generation: u64, expected_market_child_count: u64) -> Self {
        Self {
            generation,
            expected_market_child_count,
        }
    }

    /// Hostile-decode the exact canonical Begin wire.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            BEGIN_MARKET_OPENING_READINESS_BYTES,
            ReadinessInstructionTagV1::Begin,
        )?;
        Ok(Self::new(
            read_u64(bytes, GENERATION_OFFSET)?,
            read_u64(bytes, BEGIN_CHILD_COUNT_OFFSET)?,
        ))
    }

    /// Return the exact canonical Begin wire.
    pub fn to_bytes(self) -> [u8; BEGIN_MARKET_OPENING_READINESS_BYTES] {
        let mut output = header(ReadinessInstructionTagV1::Begin);
        put_u64(&mut output, GENERATION_OFFSET, self.generation);
        put_u64(
            &mut output,
            BEGIN_CHILD_COUNT_OFFSET,
            self.expected_market_child_count,
        );
        output
    }

    /// Return the immutable Market generation replay guard.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the exact direct-child count required before creation.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }
}

/// Advance one transient Market-opening readiness child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceMarketOpeningReadinessV1 {
    generation: u64,
    expected_entry_index: u16,
}

impl AdvanceMarketOpeningReadinessV1 {
    /// Construct from the Market generation and one canonical manifest index.
    pub const fn new(generation: u64, expected_entry_index: u16) -> Self {
        Self {
            generation,
            expected_entry_index,
        }
    }

    /// Hostile-decode the exact canonical Advance wire.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            ADVANCE_MARKET_OPENING_READINESS_BYTES,
            ReadinessInstructionTagV1::Advance,
        )?;
        require_zero(bytes, ADVANCE_RESERVED_OFFSET, ADVANCE_RESERVED_BYTES)?;
        Ok(Self::new(
            read_u64(bytes, GENERATION_OFFSET)?,
            read_u16(bytes, ADVANCE_ENTRY_INDEX_OFFSET)?,
        ))
    }

    /// Return the exact canonical Advance wire.
    pub fn to_bytes(self) -> [u8; ADVANCE_MARKET_OPENING_READINESS_BYTES] {
        let mut output = header(ReadinessInstructionTagV1::Advance);
        put_u64(&mut output, GENERATION_OFFSET, self.generation);
        put_u16(
            &mut output,
            ADVANCE_ENTRY_INDEX_OFFSET,
            self.expected_entry_index,
        );
        output
    }

    /// Return the immutable Market generation replay guard.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the only manifest entry index this advance may accept.
    pub const fn expected_entry_index(self) -> u16 {
        self.expected_entry_index
    }
}

fn decode_header(bytes: &[u8]) -> Result<ReadinessInstructionTagV1> {
    if bytes.len() < READINESS_INSTRUCTION_HEADER_BYTES {
        return Err(ReadinessInstructionError::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != READINESS_INSTRUCTION_MAGIC {
        return Err(ReadinessInstructionError::InvalidMagic);
    }
    if read_u16(bytes, SCHEMA_OFFSET)? != READINESS_INSTRUCTION_SCHEMA_VERSION {
        return Err(ReadinessInstructionError::UnsupportedSchema);
    }
    if read_byte(bytes, FLAGS_OFFSET)? != 0 {
        return Err(ReadinessInstructionError::NonCanonicalReservedBytes);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
    ReadinessInstructionTagV1::decode(read_byte(bytes, TAG_OFFSET)?)
}

fn require_header(bytes: &[u8], length: usize, tag: ReadinessInstructionTagV1) -> Result<()> {
    if bytes.len() != length {
        return Err(ReadinessInstructionError::InvalidLength);
    }
    if decode_header(bytes)? != tag {
        return Err(ReadinessInstructionError::UnknownAction);
    }
    Ok(())
}

fn header(tag: ReadinessInstructionTagV1) -> [u8; READINESS_INSTRUCTION_HEADER_BYTES + 16] {
    let mut output = [0u8; READINESS_INSTRUCTION_HEADER_BYTES + 16];
    output[..8].copy_from_slice(&READINESS_INSTRUCTION_MAGIC);
    put_u16(
        &mut output,
        SCHEMA_OFFSET,
        READINESS_INSTRUCTION_SCHEMA_VERSION,
    );
    output[TAG_OFFSET] = tag.byte();
    output
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(ReadinessInstructionError::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(ReadinessInstructionError::InvalidLength)?
        .try_into()
        .map_err(|_| ReadinessInstructionError::InvalidLength)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ReadinessInstructionError::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    let Some(destination) = bytes.get_mut(offset..offset.saturating_add(2)) else {
        return;
    };
    destination.copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    let Some(destination) = bytes.get_mut(offset..offset.saturating_add(8)) else {
        return;
    };
    destination.copy_from_slice(&value.to_le_bytes());
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    let end = offset
        .checked_add(length)
        .ok_or(ReadinessInstructionError::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(ReadinessInstructionError::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ReadinessInstructionError::NonCanonicalReservedBytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_wires_round_trip_without_client_status_or_allocation() {
        let begin = BeginMarketOpeningReadinessV1::new(7, 1);
        assert_eq!(
            ReadinessInstructionV1::decode(&begin.to_bytes()),
            Ok(ReadinessInstructionV1::Begin(begin))
        );

        let advance = AdvanceMarketOpeningReadinessV1::new(7, 3);
        assert_eq!(
            ReadinessInstructionV1::decode(&advance.to_bytes()),
            Ok(ReadinessInstructionV1::Advance(advance))
        );
    }

    #[test]
    fn hostile_wires_refuse_trailing_reserved_and_unknown_bytes() {
        let begin = BeginMarketOpeningReadinessV1::new(7, 1).to_bytes();
        let mut trailing = [0u8; BEGIN_MARKET_OPENING_READINESS_BYTES + 1];
        trailing[..BEGIN_MARKET_OPENING_READINESS_BYTES].copy_from_slice(&begin);
        assert_eq!(
            BeginMarketOpeningReadinessV1::decode(&trailing),
            Err(ReadinessInstructionError::InvalidLength)
        );

        let mut dirty_advance = AdvanceMarketOpeningReadinessV1::new(7, 3).to_bytes();
        dirty_advance[ADVANCE_RESERVED_OFFSET] = 1;
        assert_eq!(
            AdvanceMarketOpeningReadinessV1::decode(&dirty_advance),
            Err(ReadinessInstructionError::NonCanonicalReservedBytes)
        );

        let mut unknown = begin;
        unknown[TAG_OFFSET] = 99;
        assert_eq!(
            ReadinessInstructionV1::decode(&unknown),
            Err(ReadinessInstructionError::UnknownAction)
        );
    }
}
