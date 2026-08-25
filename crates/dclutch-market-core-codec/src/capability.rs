//! Canonical runtime-width funding-list prefix for Core-routed capabilities.
//!
//! The prefix and the following child-owned request are hashed together by
//! [`crate::CoreEffectEnvelopeV1`]. Core and the selected child therefore
//! decode one byte string while each retains sole ownership of its facts.
//! Profile 1 inherits the capability-manifest maximum of sixteen entries.
//! Lifting that semantic bound requires a new manifest and physical ABI
//! profile; this decoder never truncates an oversized list.

use crate::{
    Error,
    generated_physical::{
        CAPABILITY_FUNDING_COUNT_OFFSET, CAPABILITY_FUNDING_DESCRIPTOR_ACCOUNT_OFFSET,
        CAPABILITY_FUNDING_DESCRIPTOR_BYTES_V1, CAPABILITY_FUNDING_DESCRIPTOR_ENTRY_INDEX_OFFSET,
        CAPABILITY_FUNDING_DESCRIPTOR_RESERVED_OFFSET, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1,
        CAPABILITY_FUNDING_LIST_MAGIC_V1, CAPABILITY_FUNDING_MAGIC_OFFSET,
        CAPABILITY_FUNDING_MAX_ENTRIES_V1, CAPABILITY_FUNDING_RESERVED_BODY_OFFSET,
        CAPABILITY_FUNDING_RESERVED_HEADER_OFFSET, CAPABILITY_FUNDING_SELECTED_ENTRY_INDEX_OFFSET,
        CAPABILITY_FUNDING_VERSION_OFFSET, PHYSICAL_ABI_VERSION_V1,
    },
};

/// One manifest-entry/FundingState account coordinate from the authenticated list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingDescriptorV1 {
    entry_index: u16,
    funding_account: [u8; 32],
}

impl CapabilityFundingDescriptorV1 {
    /// Construct one nonzero FundingState coordinate.
    pub fn new(entry_index: u16, funding_account: [u8; 32]) -> Result<Self, Error> {
        if funding_account.iter().all(|byte| *byte == 0) {
            return Err(Error::InvalidAccount);
        }
        Ok(Self {
            entry_index,
            funding_account,
        })
    }

    /// Return the canonical manifest entry index.
    #[must_use]
    pub const fn entry_index(self) -> u16 {
        self.entry_index
    }

    /// Return the exact child-owned FundingState account key.
    #[must_use]
    pub const fn funding_account(self) -> [u8; 32] {
        self.funding_account
    }
}

/// Borrowed, hostile-decoded profile-1 capability funding-list prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFundingListV1<'a> {
    bytes: &'a [u8],
    count: u8,
    selected_entry_index: u16,
}

impl<'a> CapabilityFundingListV1<'a> {
    /// Encode one exact canonical prefix into caller-owned fixed storage.
    pub fn encode_into(
        descriptors: &[CapabilityFundingDescriptorV1],
        selected_entry_index: u16,
        output: &'a mut [u8],
    ) -> Result<Self, Error> {
        let count = u8::try_from(descriptors.len()).map_err(|_| Error::InvalidLength)?;
        if count == 0 || usize::from(count) > CAPABILITY_FUNDING_MAX_ENTRIES_V1 {
            return Err(Error::InvalidLength);
        }
        if output.len() != bytes_for_count(count)? {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(
            output,
            CAPABILITY_FUNDING_MAGIC_OFFSET,
            &CAPABILITY_FUNDING_LIST_MAGIC_V1,
        )?;
        put(
            output,
            CAPABILITY_FUNDING_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V1.to_le_bytes(),
        )?;
        put(output, CAPABILITY_FUNDING_COUNT_OFFSET, &[count])?;
        put(
            output,
            CAPABILITY_FUNDING_SELECTED_ENTRY_INDEX_OFFSET,
            &selected_entry_index.to_le_bytes(),
        )?;
        for (position, descriptor) in descriptors.iter().enumerate() {
            let offset = descriptor_offset(position)?;
            put(
                output,
                offset
                    .checked_add(CAPABILITY_FUNDING_DESCRIPTOR_ENTRY_INDEX_OFFSET)
                    .ok_or(Error::ArithmeticOverflow)?,
                &descriptor.entry_index.to_le_bytes(),
            )?;
            put(
                output,
                offset
                    .checked_add(CAPABILITY_FUNDING_DESCRIPTOR_ACCOUNT_OFFSET)
                    .ok_or(Error::ArithmeticOverflow)?,
                &descriptor.funding_account,
            )?;
        }
        Self::decode_exact(output)
    }

    /// Decode one exact list prefix without a child request tail.
    pub fn decode_exact(input: &'a [u8]) -> Result<Self, Error> {
        let (count, selected_entry_index, prefix_bytes) = decode_header(input)?;
        if input.len() != prefix_bytes {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            bytes: input,
            count,
            selected_entry_index,
        };
        value.validate()?;
        Ok(value)
    }

    /// Decode the prefix and return the untouched child-owned request tail.
    ///
    /// The tail must be nonempty because the Core envelope binds one real
    /// role-owned request, not merely a funding observation.
    pub fn decode_prefix(input: &'a [u8]) -> Result<(Self, &'a [u8]), Error> {
        let (count, selected_entry_index, prefix_bytes) = decode_header(input)?;
        if input.len() <= prefix_bytes {
            return Err(Error::InvalidLength);
        }
        let bytes = input.get(..prefix_bytes).ok_or(Error::InvalidLength)?;
        let child_request = input.get(prefix_bytes..).ok_or(Error::InvalidLength)?;
        let value = Self {
            bytes,
            count,
            selected_entry_index,
        };
        value.validate()?;
        Ok((value, child_request))
    }

    /// Return the exact prefix bytes committed by the enclosing effect digest.
    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Return the number of exact funding coordinates.
    #[must_use]
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Return the manifest entry selecting the child request semantics.
    #[must_use]
    pub const fn selected_entry_index(self) -> u16 {
        self.selected_entry_index
    }

    /// Decode one descriptor by canonical position.
    pub fn descriptor(self, position: usize) -> Result<CapabilityFundingDescriptorV1, Error> {
        if position >= usize::from(self.count) {
            return Err(Error::InvalidCoordinates);
        }
        let offset = descriptor_offset(position)?;
        require_zero(
            self.bytes,
            offset
                .checked_add(CAPABILITY_FUNDING_DESCRIPTOR_RESERVED_OFFSET)
                .ok_or(Error::ArithmeticOverflow)?,
            2,
        )?;
        CapabilityFundingDescriptorV1::new(
            read_u16(
                self.bytes,
                offset
                    .checked_add(CAPABILITY_FUNDING_DESCRIPTOR_ENTRY_INDEX_OFFSET)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?,
            read_array(
                self.bytes,
                offset
                    .checked_add(CAPABILITY_FUNDING_DESCRIPTOR_ACCOUNT_OFFSET)
                    .ok_or(Error::ArithmeticOverflow)?,
            )?,
        )
    }

    fn validate(self) -> Result<(), Error> {
        let mut selected_present = false;
        let mut position = 0usize;
        let mut previous: Option<u16> = None;
        while position < usize::from(self.count) {
            let descriptor = self.descriptor(position)?;
            if previous.is_some_and(|value| value >= descriptor.entry_index) {
                return Err(Error::InvalidCoordinates);
            }
            if descriptor.entry_index == self.selected_entry_index {
                selected_present = true;
            }
            let mut earlier = 0usize;
            while earlier < position {
                if self.descriptor(earlier)?.funding_account == descriptor.funding_account {
                    return Err(Error::InvalidAlias);
                }
                earlier = earlier.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            previous = Some(descriptor.entry_index);
            position = position.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !selected_present {
            return Err(Error::InvalidCoordinates);
        }
        Ok(())
    }
}

/// Return the exact encoded prefix width for a valid profile-1 count.
pub fn capability_funding_list_bytes_v1(count: u8) -> Result<usize, Error> {
    bytes_for_count(count)
}

fn decode_header(input: &[u8]) -> Result<(u8, u16, usize), Error> {
    if input.len() < CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1 {
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
    require_zero(input, CAPABILITY_FUNDING_RESERVED_HEADER_OFFSET, 1)?;
    require_zero(input, CAPABILITY_FUNDING_RESERVED_BODY_OFFSET, 2)?;
    let count = read_u8(input, CAPABILITY_FUNDING_COUNT_OFFSET)?;
    let prefix_bytes = bytes_for_count(count)?;
    Ok((
        count,
        read_u16(input, CAPABILITY_FUNDING_SELECTED_ENTRY_INDEX_OFFSET)?,
        prefix_bytes,
    ))
}

fn bytes_for_count(count: u8) -> Result<usize, Error> {
    if count == 0 || usize::from(count) > CAPABILITY_FUNDING_MAX_ENTRIES_V1 {
        return Err(Error::InvalidLength);
    }
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
        .checked_add(
            usize::from(count)
                .checked_mul(CAPABILITY_FUNDING_DESCRIPTOR_BYTES_V1)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn descriptor_offset(position: usize) -> Result<usize, Error> {
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
        .checked_add(
            position
                .checked_mul(CAPABILITY_FUNDING_DESCRIPTOR_BYTES_V1)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
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

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
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
