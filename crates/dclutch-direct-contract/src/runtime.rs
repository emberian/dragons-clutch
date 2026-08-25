//! Runtime-width Direct projection of canonical Realm Position bytes.
//!
//! The generic Realm type remains the schema owner. This projection exists so
//! one SBF execution path can serve every admitted width without emitting
//! fifteen copies of each Direct transition.

#![allow(clippy::indexing_slicing)]

use core::convert::TryInto;

use dclutch_realm_contract::{
    MAX_OUTCOMES, MIN_OUTCOMES, POSITION_BASE_BYTES, POSITION_MAGIC, POSITION_SCHEMA_VERSION,
};

use crate::{Error, Result};

const OUTCOME_COUNT_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 11;
const RESERVED_BYTES: usize = 5;
const MARKET_OFFSET: usize = 16;
const OWNER_OFFSET: usize = 48;
const GENERATION_OFFSET: usize = 80;
const BALANCES_OFFSET: usize = POSITION_BASE_BYTES;

/// One bounded runtime-width Position value used by Direct execution.
///
/// Only `balances[..outcome_count]` is active. The inactive tail is always
/// zero and is never serialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPositionV2 {
    market: [u8; 32],
    owner: [u8; 32],
    generation: u64,
    outcome_count: u8,
    balances: [u64; MAX_OUTCOMES],
}

impl DirectPositionV2 {
    /// Decode one exact canonical Realm Position at any admitted width.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < POSITION_BASE_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != POSITION_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != POSITION_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        let outcome_count = read_byte(bytes, OUTCOME_COUNT_OFFSET)?;
        let active = usize::from(outcome_count);
        if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&active) {
            return Err(Error::InvalidOutcomeWidth);
        }
        let expected = active
            .checked_mul(8)
            .and_then(|width| POSITION_BASE_BYTES.checked_add(width))
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
        let market = read_array(bytes, MARKET_OFFSET)?;
        let owner = read_array(bytes, OWNER_OFFSET)?;
        require_nonzero(&market)?;
        require_nonzero(&owner)?;
        let mut balances = [0u64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < active {
            let offset = BALANCES_OFFSET
                .checked_add(index.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            balances[index] = u64::from_le_bytes(read_array(bytes, offset)?);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(Self {
            market,
            owner,
            generation: u64::from_le_bytes(read_array(bytes, GENERATION_OFFSET)?),
            outcome_count,
            balances,
        })
    }

    /// Encode the exact canonical Realm Position into caller-owned bytes.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let active = usize::from(self.outcome_count);
        let expected = active
            .checked_mul(8)
            .and_then(|width| POSITION_BASE_BYTES.checked_add(width))
            .ok_or(Error::ArithmeticOverflow)?;
        if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&active) || output.len() != expected {
            return Err(Error::OutputLength);
        }
        require_nonzero(&self.market)?;
        require_nonzero(&self.owner)?;
        if self.balances[active..].iter().any(|value| *value != 0) {
            return Err(Error::InvalidOutcomeWidth);
        }
        output.fill(0);
        put(output, 0, &POSITION_MAGIC)?;
        put(output, 8, &POSITION_SCHEMA_VERSION.to_le_bytes())?;
        output[OUTCOME_COUNT_OFFSET] = self.outcome_count;
        put(output, MARKET_OFFSET, &self.market)?;
        put(output, OWNER_OFFSET, &self.owner)?;
        put(output, GENERATION_OFFSET, &self.generation.to_le_bytes())?;
        for (index, balance) in self.balances[..active].iter().enumerate() {
            let offset = BALANCES_OFFSET
                .checked_add(index.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            put(output, offset, &balance.to_le_bytes())?;
        }
        Ok(())
    }

    /// Return the exact active outcome width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Return the bound Market address.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the Position owner.
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Return the bound Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return one active balance.
    pub fn balance(self, outcome: usize) -> Result<u64> {
        if outcome >= usize::from(self.outcome_count) {
            return Err(Error::InvalidOutcome);
        }
        self.balances
            .get(outcome)
            .copied()
            .ok_or(Error::InvalidOutcome)
    }

    /// Credit one selected outcome atomically.
    pub fn credit_outcome(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        if quantity == 0 || outcome >= usize::from(self.outcome_count) {
            return Err(if quantity == 0 {
                Error::ZeroQuantity
            } else {
                Error::InvalidOutcome
            });
        }
        let next = self
            .balance(outcome)?
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        self.balances[outcome] = next;
        Ok(())
    }

    /// Debit one selected outcome atomically.
    pub fn debit_outcome(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        if quantity == 0 || outcome >= usize::from(self.outcome_count) {
            return Err(if quantity == 0 {
                Error::ZeroQuantity
            } else {
                Error::InvalidOutcome
            });
        }
        let next = self
            .balance(outcome)?
            .checked_sub(quantity)
            .ok_or(Error::InsufficientPositionBalance)?;
        self.balances[outcome] = next;
        Ok(())
    }
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
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
        .any(|value| *value != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset.checked_add(value.len()).ok_or(Error::OutputLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::OutputLength)?
        .copy_from_slice(value);
    Ok(())
}
