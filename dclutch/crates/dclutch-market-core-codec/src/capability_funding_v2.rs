//! Canonical routing header for physical capability-funding ledgers.
//!
//! The header counts one to sixteen physical ledgers whose disjoint subsets
//! cover one to sixteen logical funding entries. The union mask may be sparse
//! over its sixteen-bit domain, but its population count is exactly
//! `logical_count`. A manifest-bound consumer separately checks index range.

use crate::Error;

#[path = "generated_capability_funding_header_v2.rs"]
mod generated;

pub use generated::CAPABILITY_FUNDING_HEADER_BYTES_V2;
use generated::{
    CAPABILITY_FUNDING_HEADER_MAGIC_V2, CAPABILITY_FUNDING_HEADER_VERSION_V2,
    CAPABILITY_FUNDING_LOGICAL_COUNT_OFFSET_V2, CAPABILITY_FUNDING_MAGIC_OFFSET_V2,
    CAPABILITY_FUNDING_MAX_LOGICAL_COUNT_V2, CAPABILITY_FUNDING_MAX_PHYSICAL_COUNT_V2,
    CAPABILITY_FUNDING_PHYSICAL_COUNT_OFFSET_V2, CAPABILITY_FUNDING_RESERVED_OFFSET_V2,
    CAPABILITY_FUNDING_SELECTED_MASK_OFFSET_V2, CAPABILITY_FUNDING_VERSION_OFFSET_V2,
};

/// Exact V2 header routing a partition of logical entries over physical ledgers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingHeaderV2 {
    physical_count: u8,
    logical_count: u8,
    selected_mask: u16,
}

impl CapabilityFundingHeaderV2 {
    /// Construct one canonical nonempty logical-entry selection.
    pub fn new(physical_count: u8, logical_count: u8, selected_mask: u16) -> Result<Self, Error> {
        if physical_count == 0 || physical_count > CAPABILITY_FUNDING_MAX_PHYSICAL_COUNT_V2 {
            return Err(Error::InvalidLength);
        }
        if logical_count == 0 || logical_count > CAPABILITY_FUNDING_MAX_LOGICAL_COUNT_V2 {
            return Err(Error::InvalidLength);
        }
        if physical_count > logical_count {
            return Err(Error::InvalidLength);
        }
        if selected_mask == 0 || selected_mask.count_ones() != u32::from(logical_count) {
            return Err(Error::InvalidFunding);
        }
        Ok(Self {
            physical_count,
            logical_count,
            selected_mask,
        })
    }

    /// Hostile-decode one exact sixteen-byte header.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != CAPABILITY_FUNDING_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        exact(
            input,
            CAPABILITY_FUNDING_MAGIC_OFFSET_V2,
            &CAPABILITY_FUNDING_HEADER_MAGIC_V2,
        )?;
        if read_u16(input, CAPABILITY_FUNDING_VERSION_OFFSET_V2)?
            != CAPABILITY_FUNDING_HEADER_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, CAPABILITY_FUNDING_RESERVED_OFFSET_V2, 2)?;
        Self::new(
            read_u8(input, CAPABILITY_FUNDING_PHYSICAL_COUNT_OFFSET_V2)?,
            read_u8(input, CAPABILITY_FUNDING_LOGICAL_COUNT_OFFSET_V2)?,
            read_u16(input, CAPABILITY_FUNDING_SELECTED_MASK_OFFSET_V2)?,
        )
    }

    /// Encode one exact sixteen-byte canonical header.
    #[must_use]
    pub fn encode(self) -> [u8; CAPABILITY_FUNDING_HEADER_BYTES_V2] {
        let mut output = [0_u8; CAPABILITY_FUNDING_HEADER_BYTES_V2];
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_MAGIC_OFFSET_V2,
            &CAPABILITY_FUNDING_HEADER_MAGIC_V2,
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_VERSION_OFFSET_V2,
            &CAPABILITY_FUNDING_HEADER_VERSION_V2.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_PHYSICAL_COUNT_OFFSET_V2,
            &[self.physical_count],
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_LOGICAL_COUNT_OFFSET_V2,
            &[self.logical_count],
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_SELECTED_MASK_OFFSET_V2,
            &self.selected_mask.to_le_bytes(),
        );
        output
    }

    /// Return the exact number of physical subset-ledger accounts.
    #[must_use]
    pub const fn physical_count(self) -> u8 {
        self.physical_count
    }

    /// Return the required union's selected logical-entry count.
    #[must_use]
    pub const fn logical_count(self) -> u8 {
        self.logical_count
    }

    /// Return the canonical nonempty mask selecting logical funding entries.
    #[must_use]
    pub const fn selected_mask(self) -> u16 {
        self.selected_mask
    }

    /// Return whether the canonical mask selects `entry_index`.
    #[must_use]
    pub const fn selects(self, entry_index: u8) -> bool {
        entry_index < CAPABILITY_FUNDING_MAX_LOGICAL_COUNT_V2
            && (self.selected_mask & (1_u16 << entry_index)) != 0
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(Error::ArithmeticOverflow)?;
    if input.get(offset..end) != Some(expected) {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, length: usize) -> Result<(), Error> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonzeroReserved);
    }
    Ok(())
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}
