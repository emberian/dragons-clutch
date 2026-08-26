//! Action-selected sets of exact Capability Program V3 bundles.
//!
//! A set contains no family tag and no executable switch. It defines one
//! bounded little-endian selector in the raw family request and a strictly
//! increasing map from selector values to exact finalized
//! `CapabilityProgramV3` content identities. Trading may use the minimal read
//! only to select a bundle; the selected RequestProfile must then revalidate
//! the same action and the complete request before any transition or effect.

use core::convert::TryInto;

use dclutch_core_contract::ContentId;

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_set_v1.rs"]
mod generated;

pub use generated::*;

/// Stable hostile-decode or selector refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramSetErrorV1 {
    /// Selected or authenticated set identity was zero.
    ZeroSetIdentity,
    /// Selected and independently authenticated set identities differed.
    SetIdentityMismatch,
    /// Bytes did not have the exact count-derived width.
    InvalidLength,
    /// Magic selected another finalized record.
    InvalidMagic,
    /// Schema or physical profile was unsupported.
    UnsupportedSchema,
    /// Selector width was not exactly one, two, or four bytes.
    InvalidSelectorWidth,
    /// Endian or reserved bytes were noncanonical.
    NonCanonicalReserved,
    /// The table was empty or exceeded the record-derived entry capacity.
    InvalidEntryCount,
    /// Selectors were duplicated, descending, or not representable at the selected width.
    NonCanonicalSelectorOrder,
    /// A mapped CapabilityProgramV3 content identity was zero.
    ZeroProgramIdentity,
    /// The selector field exceeded the supplied request.
    SelectorOutOfBounds,
    /// No exact entry admitted the selected request action.
    MissingSelector,
}

/// Result alias for V1 program sets.
pub type ProgramSetResultV1<T> = core::result::Result<T, ProgramSetErrorV1>;

/// Canonical selector width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorWidthV1 {
    /// One-byte action value.
    U8,
    /// Little-endian two-byte action value.
    U16,
    /// Little-endian four-byte action value.
    U32,
}

impl SelectorWidthV1 {
    fn decode(value: u8) -> ProgramSetResultV1<Self> {
        match value {
            1 => Ok(Self::U8),
            2 => Ok(Self::U16),
            4 => Ok(Self::U32),
            _ => Err(ProgramSetErrorV1::InvalidSelectorWidth),
        }
    }

    /// Exact selector byte width.
    pub const fn bytes(self) -> u8 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }

    const fn maximum(self) -> u32 {
        match self {
            Self::U8 => 0xff,
            Self::U16 => 0xffff,
            Self::U32 => u32::MAX,
        }
    }
}

/// One decoded strictly ordered selector entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramSetEntryV1 {
    selector: u32,
    program: ContentId,
}

impl CapabilityProgramSetEntryV1 {
    /// Exact request selector value.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    /// Exact finalized CapabilityProgramV3 content identity.
    pub const fn program(self) -> ContentId {
        self.program
    }
}

/// Hostile-decoded, allocation-free capability-program set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramSetV1<'a> {
    selector_offset: u32,
    selector_width: SelectorWidthV1,
    entry_count: u16,
    bytes: &'a [u8],
}

impl<'a> CapabilityProgramSetV1<'a> {
    /// Decode after joining the selected set identity to independently authenticated bytes.
    pub fn decode_selected(
        selected_set_id: [u8; 32],
        authenticated_set_id: [u8; 32],
        bytes: &'a [u8],
    ) -> ProgramSetResultV1<Self> {
        if selected_set_id == [0; 32] || authenticated_set_id == [0; 32] {
            return Err(ProgramSetErrorV1::ZeroSetIdentity);
        }
        if selected_set_id != authenticated_set_id {
            return Err(ProgramSetErrorV1::SetIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode and prevalidate one complete program set.
    pub fn decode(bytes: &'a [u8]) -> ProgramSetResultV1<Self> {
        if bytes.len() < CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1
            || bytes.len() > CAPABILITY_PROGRAM_SET_MAX_BYTES_V1
        {
            return Err(ProgramSetErrorV1::InvalidLength);
        }
        if slice(bytes, CAPABILITY_PROGRAM_SET_MAGIC_OFFSET_V1, 8)?
            != CAPABILITY_PROGRAM_SET_MAGIC_V1
        {
            return Err(ProgramSetErrorV1::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_OFFSET_V1)?
            != CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V1
            || read_u16(bytes, CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_OFFSET_V1)?
                != CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V1
        {
            return Err(ProgramSetErrorV1::UnsupportedSchema);
        }
        if byte(bytes, CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V1)?
            != CAPABILITY_PROGRAM_SET_CANONICAL_ENDIAN_V1
            || !all_zero(slice(
                bytes,
                CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V1,
                CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1
                    .checked_sub(CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V1)
                    .ok_or(ProgramSetErrorV1::InvalidLength)?,
            )?)
        {
            return Err(ProgramSetErrorV1::NonCanonicalReserved);
        }
        let selector_width = SelectorWidthV1::decode(byte(
            bytes,
            CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V1,
        )?)?;
        let entry_count = read_u16(bytes, CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V1)?;
        if entry_count == 0 || usize::from(entry_count) > CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V1 {
            return Err(ProgramSetErrorV1::InvalidEntryCount);
        }
        let expected = usize::from(entry_count)
            .checked_mul(CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1)
            .and_then(|body| CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1.checked_add(body))
            .ok_or(ProgramSetErrorV1::InvalidLength)?;
        if bytes.len() != expected {
            return Err(ProgramSetErrorV1::InvalidLength);
        }
        let value = Self {
            selector_offset: read_u32(bytes, CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V1)?,
            selector_width,
            entry_count,
            bytes,
        };
        let mut prior = None;
        let mut index = 0_u16;
        while index < entry_count {
            let entry = value.entry(index)?;
            if entry.selector > selector_width.maximum()
                || prior.is_some_and(|selector| selector >= entry.selector)
            {
                return Err(ProgramSetErrorV1::NonCanonicalSelectorOrder);
            }
            prior = Some(entry.selector);
            index = index
                .checked_add(1)
                .ok_or(ProgramSetErrorV1::InvalidEntryCount)?;
        }
        Ok(value)
    }

    /// Byte offset of the minimally parsed action selector.
    pub const fn selector_offset(self) -> u32 {
        self.selector_offset
    }

    /// Canonical selector width.
    pub const fn selector_width(self) -> SelectorWidthV1 {
        self.selector_width
    }

    /// Number of strictly ordered action bundles.
    pub const fn entry_count(self) -> u16 {
        self.entry_count
    }

    /// Decode one exact table entry.
    pub fn entry(self, index: u16) -> ProgramSetResultV1<CapabilityProgramSetEntryV1> {
        if index >= self.entry_count {
            return Err(ProgramSetErrorV1::InvalidEntryCount);
        }
        let offset = usize::from(index)
            .checked_mul(CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1)
            .and_then(|body| CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1.checked_add(body))
            .ok_or(ProgramSetErrorV1::InvalidLength)?;
        if !all_zero(slice(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_RESERVED_OFFSET_V1)
                .ok_or(ProgramSetErrorV1::InvalidLength)?,
            4,
        )?) {
            return Err(ProgramSetErrorV1::NonCanonicalReserved);
        }
        let selector = read_u32(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V1)
                .ok_or(ProgramSetErrorV1::InvalidLength)?,
        )?;
        let program = ContentId::new(read_array(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_PROGRAM_OFFSET_V1)
                .ok_or(ProgramSetErrorV1::InvalidLength)?,
        )?)
        .map_err(|_| ProgramSetErrorV1::ZeroProgramIdentity)?;
        Ok(CapabilityProgramSetEntryV1 { selector, program })
    }

    /// Select one exact full CapabilityProgramV3 bundle from raw request bytes.
    ///
    /// This does not validate the family request. The selected RequestProfile
    /// must independently require the same action before execution.
    pub fn select(self, request: &[u8]) -> ProgramSetResultV1<ContentId> {
        let start = usize::try_from(self.selector_offset)
            .map_err(|_| ProgramSetErrorV1::SelectorOutOfBounds)?;
        let width = usize::from(self.selector_width.bytes());
        let selector_bytes = request
            .get(
                start
                    ..start
                        .checked_add(width)
                        .ok_or(ProgramSetErrorV1::SelectorOutOfBounds)?,
            )
            .ok_or(ProgramSetErrorV1::SelectorOutOfBounds)?;
        let selector = match self.selector_width {
            SelectorWidthV1::U8 => u32::from(
                selector_bytes
                    .first()
                    .copied()
                    .ok_or(ProgramSetErrorV1::SelectorOutOfBounds)?,
            ),
            SelectorWidthV1::U16 => u32::from(u16::from_le_bytes(
                selector_bytes
                    .try_into()
                    .map_err(|_| ProgramSetErrorV1::SelectorOutOfBounds)?,
            )),
            SelectorWidthV1::U32 => u32::from_le_bytes(
                selector_bytes
                    .try_into()
                    .map_err(|_| ProgramSetErrorV1::SelectorOutOfBounds)?,
            ),
        };
        let mut index = 0_u16;
        while index < self.entry_count {
            let entry = self.entry(index)?;
            if entry.selector == selector {
                return Ok(entry.program);
            }
            if entry.selector > selector {
                break;
            }
            index = index
                .checked_add(1)
                .ok_or(ProgramSetErrorV1::InvalidEntryCount)?;
        }
        Err(ProgramSetErrorV1::MissingSelector)
    }

    /// Borrow exact canonical finalized-record bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn byte(bytes: &[u8], offset: usize) -> ProgramSetResultV1<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ProgramSetErrorV1::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> ProgramSetResultV1<&[u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(ProgramSetErrorV1::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(ProgramSetErrorV1::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> ProgramSetResultV1<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| ProgramSetErrorV1::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> ProgramSetResultV1<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| ProgramSetErrorV1::InvalidLength)?,
    ))
}

fn read_array(bytes: &[u8], offset: usize) -> ProgramSetResultV1<[u8; 32]> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| ProgramSetErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    #[test]
    fn generated_set_selects_exact_full_program_bundle() {
        let set = CapabilityProgramSetV1::decode(&CAPABILITY_PROGRAM_SET_EXAMPLE_V1)
            .expect("generated set");
        assert_eq!(set.selector_offset(), 10);
        assert_eq!(set.selector_width(), SelectorWidthV1::U8);
        assert_eq!(set.entry_count(), 3);
        let mut request = [0_u8; 11];
        request[10] = 3;
        assert_eq!(
            set.select(&request),
            Ok(ContentId::new([0x22; 32]).expect("id"))
        );
        request[10] = 2;
        assert_eq!(
            set.select(&request),
            Err(ProgramSetErrorV1::MissingSelector)
        );
        assert_eq!(
            set.select(&request[..10]),
            Err(ProgramSetErrorV1::SelectorOutOfBounds)
        );
    }

    #[test]
    fn generated_hostile_corpus_and_identity_substitution_refuse() {
        for hostile in CAPABILITY_PROGRAM_SET_HOSTILE_CORPUS_V1 {
            assert!(CapabilityProgramSetV1::decode(&hostile).is_err());
        }
        assert_eq!(
            CapabilityProgramSetV1::decode_selected(
                [1; 32],
                [2; 32],
                &CAPABILITY_PROGRAM_SET_EXAMPLE_V1,
            ),
            Err(ProgramSetErrorV1::SetIdentityMismatch)
        );
    }

    #[test]
    fn every_canonical_width_and_singleton_set_accept() {
        for (width, request) in [
            (1_u8, [3_u8, 0, 0, 0]),
            (2, [3, 0, 0, 0]),
            (4, [3, 0, 0, 0]),
        ] {
            let mut bytes = CAPABILITY_PROGRAM_SET_EXAMPLE_V1;
            bytes[CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V1] = width;
            let set = CapabilityProgramSetV1::decode(&bytes).expect("canonical width");
            let mut family_request = [0_u8; 14];
            let used = usize::from(width);
            family_request
                .get_mut(10..10 + used)
                .expect("bounded request selector")
                .copy_from_slice(request.get(..used).expect("bounded selector bytes"));
            assert_eq!(
                set.select(&family_request),
                Ok(ContentId::new([0x22; 32]).expect("id"))
            );
        }

        let mut singleton: Vec<u8> = CAPABILITY_PROGRAM_SET_EXAMPLE_V1
            [..CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 + CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1]
            .to_vec();
        singleton
            .get_mut(
                CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V1
                    ..CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V1 + 2,
            )
            .expect("entry count field")
            .copy_from_slice(&1_u16.to_le_bytes());
        let set = CapabilityProgramSetV1::decode(&singleton).expect("singleton");
        let mut request = [0_u8; 11];
        request[10] = 1;
        assert_eq!(
            set.select(&request),
            Ok(ContentId::new([0x11; 32]).expect("id"))
        );
    }
}
