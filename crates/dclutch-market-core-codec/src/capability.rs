//! Canonical funding-account-count prefix for Core-routed capabilities.
//!
//! The role bytes are `CapabilityExecutionSelectionV1 || header || child
//! request`. The selected child request remains the sole byte-level owner of
//! the funding coordinates it uses. Core obtains authoritative entry indices
//! from the child-owned FundingState accounts, requires strict entry-index
//! order and pairwise-distinct derived keys, and binds the whole composite in
//! the existing effect digest. This keeps the maximum on-wire overhead fixed
//! at sixteen bytes instead of repeating up to sixteen account keys.
//!
//! Profile 1 inherits the capability-manifest maximum of sixteen entries.
//! Lifting that semantic bound requires a new manifest and physical ABI
//! profile; this decoder never truncates an oversized count.

use crate::{
    Error,
    generated_physical::{
        CAPABILITY_FUNDING_COUNT_OFFSET, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1,
        CAPABILITY_FUNDING_LIST_MAGIC_V1, CAPABILITY_FUNDING_MAGIC_OFFSET,
        CAPABILITY_FUNDING_MAX_ENTRIES_V1, CAPABILITY_FUNDING_RESERVED_OFFSET,
        CAPABILITY_FUNDING_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1,
    },
};

/// Exact profile-1 header preceding one child-owned capability request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingHeaderV1 {
    funding_count: u8,
}

impl CapabilityFundingHeaderV1 {
    /// Construct a bounded, nonempty funding-account count.
    pub fn new(funding_count: u8) -> Result<Self, Error> {
        if funding_count == 0 || usize::from(funding_count) > CAPABILITY_FUNDING_MAX_ENTRIES_V1 {
            return Err(Error::InvalidLength);
        }
        Ok(Self { funding_count })
    }

    /// Hostile-decode one exact sixteen-byte header.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        exact(
            input,
            CAPABILITY_FUNDING_MAGIC_OFFSET,
            &CAPABILITY_FUNDING_LIST_MAGIC_V1,
        )?;
        if read_u16(input, CAPABILITY_FUNDING_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, CAPABILITY_FUNDING_RESERVED_OFFSET, 5)?;
        Self::new(read_u8(input, CAPABILITY_FUNDING_COUNT_OFFSET)?)
    }

    /// Encode one exact sixteen-byte header.
    pub fn encode(self) -> [u8; CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1] {
        let mut output = [0_u8; CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1];
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_MAGIC_OFFSET,
            &CAPABILITY_FUNDING_LIST_MAGIC_V1,
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V1.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            CAPABILITY_FUNDING_COUNT_OFFSET,
            &[self.funding_count],
        );
        output
    }

    /// Return the exact number of leading child-owned FundingState accounts.
    #[must_use]
    pub const fn funding_count(self) -> u8 {
        self.funding_count
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
