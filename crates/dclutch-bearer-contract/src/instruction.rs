//! Exact-width hostile instruction codecs.

use core::convert::TryInto;

use crate::state::{MAX_OUTCOMES, MIN_OUTCOMES};
use crate::{Error, Result};

/// Canonical bearer instruction magic.
pub const BEARER_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTBIX1";
/// Implemented instruction schema.
pub const BEARER_INSTRUCTION_SCHEMA_VERSION: u16 = 1;
/// Exact common instruction header width.
pub const INSTRUCTION_HEADER_BYTES: usize = 16;
/// Exact activation instruction width.
pub const ACTIVATE_INSTRUCTION_BYTES: usize = 32;
/// Exact audit instruction width.
pub const AUDIT_INSTRUCTION_BYTES: usize = 24;
/// Exact split, merge, and retire instruction width.
pub const SET_INSTRUCTION_BYTES: usize = 32;
/// Exact outcome-specific instruction width.
pub const OUTCOME_INSTRUCTION_BYTES: usize = 40;

const ACTION_OFFSET: usize = 10;
const OUTCOME_COUNT_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;
const GENERATION_OFFSET: usize = 16;
const VALUE_OFFSET: usize = 24;
const OUTCOME_OFFSET: usize = 32;
const OUTCOME_RESERVED_OFFSET: usize = 33;
const OUTCOME_RESERVED_BYTES: usize = 7;

/// Canonical public action discriminators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ActionV1 {
    /// Create the direct child and every canonical Mint.
    Activate = 0,
    /// Audit full Mint supply/profile state without economic mutation.
    Audit = 1,
    /// Deposit a complete set into a native Position.
    SplitNative = 2,
    /// Merge a complete set from a native Position.
    MergeNative = 3,
    /// Move native claims into one bearer Mint.
    Materialize = 4,
    /// Burn bearer claims into a native Position.
    Dematerialize = 5,
    /// Transfer bearer ownership without changing supply.
    Transfer = 6,
    /// Deposit a complete set directly into all bearer Mints.
    SplitBearer = 7,
    /// Burn a complete bearer set and withdraw collateral.
    MergeBearer = 8,
    /// Redeem one native outcome claim.
    RedeemNative = 9,
    /// Burn and redeem one bearer outcome claim.
    RedeemBearer = 10,
    /// Close zero-supply Mints and retire the direct child.
    Retire = 11,
}

impl ActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Activate),
            1 => Ok(Self::Audit),
            2 => Ok(Self::SplitNative),
            3 => Ok(Self::MergeNative),
            4 => Ok(Self::Materialize),
            5 => Ok(Self::Dematerialize),
            6 => Ok(Self::Transfer),
            7 => Ok(Self::SplitBearer),
            8 => Ok(Self::MergeBearer),
            9 => Ok(Self::RedeemNative),
            10 => Ok(Self::RedeemBearer),
            11 => Ok(Self::Retire),
            _ => Err(Error::UnsupportedSchema),
        }
    }
}

/// Decoded exact bearer instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionV1 {
    /// Atomic manifest-funded physical activation.
    Activate {
        /// Exact categorical width.
        outcome_count: u8,
        /// Immutable Market generation replay guard.
        generation: u64,
        /// Market child-count replay guard.
        expected_prior_child_count: u64,
    },
    /// Read-only full-Mint audit.
    Audit {
        /// Exact categorical width.
        outcome_count: u8,
        /// Immutable Market generation replay guard.
        generation: u64,
    },
    /// One quantity action with no selected outcome.
    Set {
        /// Exact action among native/bearer split or merge.
        action: ActionV1,
        /// Exact categorical width.
        outcome_count: u8,
        /// Immutable Market generation replay guard.
        generation: u64,
        /// Exact raw claim/collateral atoms.
        quantity: u64,
    },
    /// One outcome-specific quantity action.
    Outcome {
        /// Exact materialize, dematerialize, transfer, or redemption action.
        action: ActionV1,
        /// Exact categorical width.
        outcome_count: u8,
        /// Immutable Market generation replay guard.
        generation: u64,
        /// Exact raw claim atoms.
        quantity: u64,
        /// Zero-based canonical outcome.
        outcome: u8,
    },
    /// Atomic close and direct-child decrement.
    Retire {
        /// Exact categorical width.
        outcome_count: u8,
        /// Immutable Market generation replay guard.
        generation: u64,
        /// Market child-count replay guard.
        expected_prior_child_count: u64,
    },
}

impl InstructionV1 {
    /// Decode one exact canonical instruction, refusing trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < INSTRUCTION_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != BEARER_INSTRUCTION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != BEARER_INSTRUCTION_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let action = ActionV1::decode(byte(bytes, ACTION_OFFSET)?)?;
        let outcome_count = byte(bytes, OUTCOME_COUNT_OFFSET)?;
        validate_count(outcome_count)?;
        let expected = instruction_bytes(action);
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let generation = read_u64(bytes, GENERATION_OFFSET)?;
        match action {
            ActionV1::Activate => Ok(Self::Activate {
                outcome_count,
                generation,
                expected_prior_child_count: read_u64(bytes, VALUE_OFFSET)?,
            }),
            ActionV1::Audit => Ok(Self::Audit {
                outcome_count,
                generation,
            }),
            ActionV1::SplitNative
            | ActionV1::MergeNative
            | ActionV1::SplitBearer
            | ActionV1::MergeBearer => Ok(Self::Set {
                action,
                outcome_count,
                generation,
                quantity: read_u64(bytes, VALUE_OFFSET)?,
            }),
            ActionV1::Materialize
            | ActionV1::Dematerialize
            | ActionV1::Transfer
            | ActionV1::RedeemNative
            | ActionV1::RedeemBearer => {
                require_zero(bytes, OUTCOME_RESERVED_OFFSET, OUTCOME_RESERVED_BYTES)?;
                let outcome = byte(bytes, OUTCOME_OFFSET)?;
                if outcome >= outcome_count {
                    return Err(Error::InvalidOutcome);
                }
                Ok(Self::Outcome {
                    action,
                    outcome_count,
                    generation,
                    quantity: read_u64(bytes, VALUE_OFFSET)?,
                    outcome,
                })
            }
            ActionV1::Retire => Ok(Self::Retire {
                outcome_count,
                generation,
                expected_prior_child_count: read_u64(bytes, VALUE_OFFSET)?,
            }),
        }
    }

    /// Return the one exact encoded width.
    pub const fn encoded_len(self) -> usize {
        instruction_bytes(self.action())
    }

    /// Return the public action discriminator.
    pub const fn action(self) -> ActionV1 {
        match self {
            Self::Activate { .. } => ActionV1::Activate,
            Self::Audit { .. } => ActionV1::Audit,
            Self::Set { action, .. } | Self::Outcome { action, .. } => action,
            Self::Retire { .. } => ActionV1::Retire,
        }
    }

    /// Return the exact categorical width.
    pub const fn outcome_count(self) -> u8 {
        match self {
            Self::Activate { outcome_count, .. }
            | Self::Audit { outcome_count, .. }
            | Self::Set { outcome_count, .. }
            | Self::Outcome { outcome_count, .. }
            | Self::Retire { outcome_count, .. } => outcome_count,
        }
    }

    /// Return the immutable Market generation replay guard.
    pub const fn generation(self) -> u64 {
        match self {
            Self::Activate { generation, .. }
            | Self::Audit { generation, .. }
            | Self::Set { generation, .. }
            | Self::Outcome { generation, .. }
            | Self::Retire { generation, .. } => generation,
        }
    }

    /// Encode atomically into an exact caller-owned buffer.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        validate_count(self.outcome_count())?;
        if output.len() != self.encoded_len() {
            return Err(Error::OutputLength);
        }
        if let Self::Set { action, .. } = self
            && !matches!(
                action,
                ActionV1::SplitNative
                    | ActionV1::MergeNative
                    | ActionV1::SplitBearer
                    | ActionV1::MergeBearer
            )
        {
            return Err(Error::UnsupportedSchema);
        }
        if let Self::Outcome {
            action,
            outcome,
            outcome_count,
            ..
        } = self
        {
            if !matches!(
                action,
                ActionV1::Materialize
                    | ActionV1::Dematerialize
                    | ActionV1::Transfer
                    | ActionV1::RedeemNative
                    | ActionV1::RedeemBearer
            ) {
                return Err(Error::UnsupportedSchema);
            }
            if outcome >= outcome_count {
                return Err(Error::InvalidOutcome);
            }
        }
        output.fill(0);
        put(output, 0, &BEARER_INSTRUCTION_MAGIC);
        put(output, 8, &BEARER_INSTRUCTION_SCHEMA_VERSION.to_le_bytes());
        put(output, ACTION_OFFSET, &[self.action() as u8]);
        put(output, OUTCOME_COUNT_OFFSET, &[self.outcome_count()]);
        put(output, GENERATION_OFFSET, &self.generation().to_le_bytes());
        match self {
            Self::Activate {
                expected_prior_child_count,
                ..
            } => {
                put(
                    output,
                    VALUE_OFFSET,
                    &expected_prior_child_count.to_le_bytes(),
                );
            }
            Self::Audit { .. } => {}
            Self::Set { quantity, .. } => put(output, VALUE_OFFSET, &quantity.to_le_bytes()),
            Self::Outcome {
                quantity, outcome, ..
            } => {
                put(output, VALUE_OFFSET, &quantity.to_le_bytes());
                put(output, OUTCOME_OFFSET, &[outcome]);
            }
            Self::Retire {
                expected_prior_child_count,
                ..
            } => put(
                output,
                VALUE_OFFSET,
                &expected_prior_child_count.to_le_bytes(),
            ),
        }
        Ok(())
    }
}

const fn instruction_bytes(action: ActionV1) -> usize {
    match action {
        ActionV1::Activate => ACTIVATE_INSTRUCTION_BYTES,
        ActionV1::Audit => AUDIT_INSTRUCTION_BYTES,
        ActionV1::SplitNative
        | ActionV1::MergeNative
        | ActionV1::SplitBearer
        | ActionV1::MergeBearer
        | ActionV1::Retire => SET_INSTRUCTION_BYTES,
        ActionV1::Materialize
        | ActionV1::Dematerialize
        | ActionV1::Transfer
        | ActionV1::RedeemNative
        | ActionV1::RedeemBearer => OUTCOME_INSTRUCTION_BYTES,
    }
}

fn validate_count(count: u8) -> Result<()> {
    if (MIN_OUTCOMES..=MAX_OUTCOMES).contains(&usize::from(count)) {
        Ok(())
    } else {
        Err(Error::InvalidOutcomeCount)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}
