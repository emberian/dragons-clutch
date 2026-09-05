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

/// Number of record and Market accounts before the physical funding slice.
pub const CAPABILITY_ROUTE_PREFIX_ACCOUNTS_V1: usize = 5;
/// Number of Core-owned fixed accounts between funding and the child tail.
pub const CAPABILITY_ROUTE_FIXED_ACCOUNTS_V1: usize = 11;
/// Number of exact infrastructure aliases admitted by a close route.
pub const CAPABILITY_CLOSE_ALIAS_COUNT_V1: usize = 7;

/// Single semantic owner for one Core capability route's dynamic coordinates.
///
/// The layout is independent of any Trading family's child-tail schema.  It
/// owns the Core prefix, the dynamic physical-funding slice, the fixed route,
/// and the seven infrastructure coordinates which a close child authenticates
/// again at child-tail offsets 8 through 14.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRouteLayoutV1 {
    funding_count: u8,
    child_tail_count: usize,
}

impl CapabilityRouteLayoutV1 {
    /// Construct one bounded route layout.
    pub fn new(funding_count: u8, child_tail_count: usize) -> Result<Self, Error> {
        if funding_count == 0 || usize::from(funding_count) > CAPABILITY_FUNDING_MAX_ENTRIES_V1 {
            return Err(Error::InvalidLength);
        }
        CAPABILITY_ROUTE_PREFIX_ACCOUNTS_V1
            .checked_add(usize::from(funding_count))
            .and_then(|value| value.checked_add(CAPABILITY_ROUTE_FIXED_ACCOUNTS_V1))
            .and_then(|value| value.checked_add(child_tail_count))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            funding_count,
            child_tail_count,
        })
    }

    /// Core Market account.
    pub const fn market(self) -> usize {
        0
    }
    /// Finalized Realm raw record.
    pub const fn realm_raw(self) -> usize {
        1
    }
    /// Vacant Realm staging cursor.
    pub const fn realm_staging(self) -> usize {
        2
    }
    /// Finalized capability-manifest raw record.
    pub const fn manifest_raw(self) -> usize {
        3
    }
    /// Vacant capability-manifest staging cursor.
    pub const fn manifest_staging(self) -> usize {
        4
    }
    /// First physical FundingLedger account.
    pub const fn funding_start(self) -> usize {
        CAPABILITY_ROUTE_PREFIX_ACCOUNTS_V1
    }
    /// Exclusive end of the physical FundingLedger slice and root coordinate.
    pub const fn funding_end(self) -> usize {
        CAPABILITY_ROUTE_PREFIX_ACCOUNTS_V1 + self.funding_count as usize
    }
    /// Capability root account.
    pub const fn root(self) -> usize {
        self.funding_end()
    }
    /// Registry activation cache.
    pub const fn activation_cache(self) -> usize {
        self.funding_end() + 1
    }
    /// Core program.
    pub const fn core_program(self) -> usize {
        self.funding_end() + 2
    }
    /// Core ProgramData.
    pub const fn core_programdata(self) -> usize {
        self.funding_end() + 3
    }
    /// Selected Trading child program.
    pub const fn trading_program(self) -> usize {
        self.funding_end() + 4
    }
    /// Selected Trading ProgramData.
    pub const fn trading_programdata(self) -> usize {
        self.funding_end() + 5
    }
    /// Selected Resolution program.
    pub const fn resolution_program(self) -> usize {
        self.funding_end() + 6
    }
    /// Selected Resolution ProgramData.
    pub const fn resolution_programdata(self) -> usize {
        self.funding_end() + 7
    }
    /// Registry program.
    pub const fn registry_program(self) -> usize {
        self.funding_end() + 8
    }
    /// Rent sysvar.
    pub const fn rent_sysvar(self) -> usize {
        self.funding_end() + 9
    }
    /// Core caller-authority PDA.
    pub const fn caller_authority(self) -> usize {
        self.funding_end() + 10
    }
    /// First child-tail account.
    pub const fn child_start(self) -> usize {
        self.funding_end() + CAPABILITY_ROUTE_FIXED_ACCOUNTS_V1
    }
    /// Exact top-level account count.
    pub const fn account_count(self) -> usize {
        self.child_start() + self.child_tail_count
    }
    /// Exact seven fixed-to-child authenticated-suffix alias pairs.
    pub const fn close_alias_pairs(self) -> [(usize, usize); CAPABILITY_CLOSE_ALIAS_COUNT_V1] {
        [
            (self.activation_cache(), self.child_start() + 8),
            (self.core_program(), self.child_start() + 9),
            (self.core_programdata(), self.child_start() + 10),
            (self.trading_program(), self.child_start() + 11),
            (self.trading_programdata(), self.child_start() + 12),
            (self.registry_program(), self.child_start() + 13),
            (self.rent_sysvar(), self.child_start() + 14),
        ]
    }
}

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

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn direct_close_geometry_has_one_exact_dynamic_owner() {
        let layout = CapabilityRouteLayoutV1::new(1, 20).expect("Direct close layout");
        assert_eq!(layout.funding_start(), 5);
        assert_eq!(layout.funding_end(), 6);
        assert_eq!(layout.child_start(), 17);
        assert_eq!(layout.account_count(), 37);
        assert_eq!(
            layout.close_alias_pairs(),
            [
                (7, 25),
                (8, 26),
                (9, 27),
                (10, 28),
                (11, 29),
                (14, 30),
                (15, 31),
            ]
        );
        assert_eq!(
            CapabilityRouteLayoutV1::new(0, 20),
            Err(Error::InvalidLength)
        );
        assert_eq!(
            CapabilityRouteLayoutV1::new(17, 20),
            Err(Error::InvalidLength)
        );
    }
}
