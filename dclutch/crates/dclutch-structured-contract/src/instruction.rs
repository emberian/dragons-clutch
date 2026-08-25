//! Exact-width hostile instruction codec for the future SBF vertical.

use crate::descriptor::{MAX_STRUCTURED_OUTCOMES, MIN_STRUCTURED_OUTCOMES};
use crate::{Error, Result, array, byte, put, require_zero};

/// Exact width shared by every Structured V1 instruction.
pub const STRUCTURED_INSTRUCTION_BYTES: usize = 32;
/// Canonical Structured instruction magic.
pub const STRUCTURED_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTSIX1";
/// Implemented instruction schema.
pub const STRUCTURED_INSTRUCTION_SCHEMA_VERSION: u16 = 1;

const ACTION_OFFSET: usize = 10;
const OUTCOME_COUNT_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;
const GENERATION_OFFSET: usize = 16;
const VALUE_OFFSET: usize = 24;

/// Public action selected by an exact Structured instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredActionV1 {
    /// Create the immutable descriptor and empty native custody child.
    Activate = 0,
    /// Debit an owner's native Position and issue structured units.
    Wrap = 1,
    /// Burn structured units and credit the owner's native Position.
    Unwrap = 2,
    /// Burn units and redeem every backed native claim after resolution.
    RedeemTerminal = 3,
    /// Close zero-supply, empty custody and decrement the Market child count.
    Retire = 4,
}

impl StructuredActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Activate),
            1 => Ok(Self::Wrap),
            2 => Ok(Self::Unwrap),
            3 => Ok(Self::RedeemTerminal),
            4 => Ok(Self::Retire),
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Activate => 0,
            Self::Wrap => 1,
            Self::Unwrap => 2,
            Self::RedeemTerminal => 3,
            Self::Retire => 4,
        }
    }
}

/// One decoded exact Structured V1 request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredInstructionV1 {
    action: StructuredActionV1,
    outcome_count: u8,
    generation: u64,
    value: u64,
}

impl StructuredInstructionV1 {
    /// Construct an activation request with child-count replay guard.
    pub fn activate(
        outcome_count: u8,
        generation: u64,
        expected_prior_child_count: u64,
    ) -> Result<Self> {
        Self::new(
            StructuredActionV1::Activate,
            outcome_count,
            generation,
            expected_prior_child_count,
        )
    }

    /// Construct a nonzero wrap request.
    pub fn wrap(outcome_count: u8, generation: u64, units: u64) -> Result<Self> {
        Self::new(StructuredActionV1::Wrap, outcome_count, generation, units)
    }

    /// Construct a nonzero unwrap request.
    pub fn unwrap(outcome_count: u8, generation: u64, units: u64) -> Result<Self> {
        Self::new(StructuredActionV1::Unwrap, outcome_count, generation, units)
    }

    /// Construct a nonzero terminal-redemption request.
    pub fn redeem_terminal(outcome_count: u8, generation: u64, units: u64) -> Result<Self> {
        Self::new(
            StructuredActionV1::RedeemTerminal,
            outcome_count,
            generation,
            units,
        )
    }

    /// Construct a retirement request with child-count replay guard.
    pub fn retire(
        outcome_count: u8,
        generation: u64,
        expected_prior_child_count: u64,
    ) -> Result<Self> {
        if expected_prior_child_count == 0 {
            return Err(Error::InvalidChildCount);
        }
        Self::new(
            StructuredActionV1::Retire,
            outcome_count,
            generation,
            expected_prior_child_count,
        )
    }

    fn new(
        action: StructuredActionV1,
        outcome_count: u8,
        generation: u64,
        value: u64,
    ) -> Result<Self> {
        validate_outcome_count(outcome_count)?;
        if matches!(
            action,
            StructuredActionV1::Wrap
                | StructuredActionV1::Unwrap
                | StructuredActionV1::RedeemTerminal
        ) && value == 0
        {
            return Err(Error::ZeroInstructionQuantity);
        }
        Ok(Self {
            action,
            outcome_count,
            generation,
            value,
        })
    }

    /// Decode one exact instruction, refusing both prefixes and trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != STRUCTURED_INSTRUCTION_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != STRUCTURED_INSTRUCTION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != STRUCTURED_INSTRUCTION_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let action = StructuredActionV1::decode(byte(bytes, ACTION_OFFSET)?)?;
        let value = u64::from_le_bytes(array(bytes, VALUE_OFFSET)?);
        let decoded = Self::new(
            action,
            byte(bytes, OUTCOME_COUNT_OFFSET)?,
            u64::from_le_bytes(array(bytes, GENERATION_OFFSET)?),
            value,
        )?;
        if action == StructuredActionV1::Retire && value == 0 {
            return Err(Error::InvalidChildCount);
        }
        Ok(decoded)
    }

    /// Encode atomically into one exact caller-owned output.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        let canonical = Self::new(self.action, self.outcome_count, self.generation, self.value)?;
        if canonical.action == StructuredActionV1::Retire && canonical.value == 0 {
            return Err(Error::InvalidChildCount);
        }
        if output.len() != STRUCTURED_INSTRUCTION_BYTES {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(output, 0, &STRUCTURED_INSTRUCTION_MAGIC);
        put(
            output,
            8,
            &STRUCTURED_INSTRUCTION_SCHEMA_VERSION.to_le_bytes(),
        );
        put(output, ACTION_OFFSET, &[self.action.byte()]);
        put(output, OUTCOME_COUNT_OFFSET, &[self.outcome_count]);
        put(output, GENERATION_OFFSET, &self.generation.to_le_bytes());
        put(output, VALUE_OFFSET, &self.value.to_le_bytes());
        Ok(())
    }

    /// Return the selected action.
    pub const fn action(self) -> StructuredActionV1 {
        self.action
    }

    /// Return the exact categorical width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Return the immutable Market generation replay guard.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return unit quantity or expected prior child count, according to action.
    pub const fn value(self) -> u64 {
        self.value
    }
}

fn validate_outcome_count(outcome_count: u8) -> Result<()> {
    if !(MIN_STRUCTURED_OUTCOMES..=MAX_STRUCTURED_OUTCOMES).contains(&usize::from(outcome_count)) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(())
    }
}
