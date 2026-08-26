//! Schema-bound action selection for exact capability descriptors.
//!
//! Each V2 entry binds both the finalized descriptor schema and the SHA-256
//! identity of its exact bytes.  An adapter must authenticate that raw/staging
//! record under the selected schema before choosing a V3 migration decoder or
//! the production V4 decoder.  Descriptor magic is never selection authority.

use core::convert::TryInto;

use dclutch_core_contract::ContentId;

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_set_v2.rs"]
mod generated;

pub use generated::*;

/// Stable hostile-decode, construction, or selection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramSetErrorV2 {
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
    /// The table was empty or exceeded the fixed entry capacity.
    InvalidEntryCount,
    /// Selectors were duplicated, descending, or too wide.
    NonCanonicalSelectorOrder,
    /// A descriptor schema identity was zero.
    ZeroDescriptorSchema,
    /// A descriptor content identity was zero.
    ZeroDescriptorProgram,
    /// The selector field exceeded the supplied request.
    SelectorOutOfBounds,
    /// No exact entry admitted the selected request action.
    MissingSelector,
    /// The selected schema/content pair differed from authenticated evidence.
    DescriptorMismatch,
}

/// Result alias for V2 program sets.
pub type ProgramSetResultV2<T> = core::result::Result<T, ProgramSetErrorV2>;

/// Canonical little-endian selector width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorWidthV2 {
    /// One-byte action value.
    U8,
    /// Little-endian two-byte action value.
    U16,
    /// Little-endian four-byte action value.
    U32,
}

impl SelectorWidthV2 {
    fn decode(value: u8) -> ProgramSetResultV2<Self> {
        match value {
            1 => Ok(Self::U8),
            2 => Ok(Self::U16),
            4 => Ok(Self::U32),
            _ => Err(ProgramSetErrorV2::InvalidSelectorWidth),
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

/// Exact schema/content coordinate of one selected descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptorReferenceV2 {
    schema: ContentId,
    program: ContentId,
}

impl CapabilityDescriptorReferenceV2 {
    /// Construct an exact nonzero descriptor coordinate.
    pub const fn new(schema: ContentId, program: ContentId) -> Self {
        Self { schema, program }
    }

    /// Finalized-record schema identity selecting the hostile decoder.
    pub const fn schema(self) -> ContentId {
        self.schema
    }

    /// SHA-256 identity of the exact finalized descriptor bytes.
    pub const fn program(self) -> ContentId {
        self.program
    }
}

/// One decoded strictly ordered selector entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramSetEntryV2 {
    selector: u32,
    descriptor: CapabilityDescriptorReferenceV2,
}

impl CapabilityProgramSetEntryV2 {
    /// Construct one exact operator input entry.
    pub const fn new(selector: u32, descriptor: CapabilityDescriptorReferenceV2) -> Self {
        Self {
            selector,
            descriptor,
        }
    }

    /// Exact request selector value.
    pub const fn selector(self) -> u32 {
        self.selector
    }

    /// Exact selected descriptor schema/content pair.
    pub const fn descriptor(self) -> CapabilityDescriptorReferenceV2 {
        self.descriptor
    }
}

/// Hostile-decoded, allocation-free capability-program set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramSetV2<'a> {
    selector_offset: u32,
    selector_width: SelectorWidthV2,
    entry_count: u16,
    bytes: &'a [u8],
}

impl<'a> CapabilityProgramSetV2<'a> {
    /// Decode after joining the selected set identity to authenticated bytes.
    pub fn decode_selected(
        selected_set_id: [u8; 32],
        authenticated_set_id: [u8; 32],
        bytes: &'a [u8],
    ) -> ProgramSetResultV2<Self> {
        if selected_set_id == [0; 32] || authenticated_set_id == [0; 32] {
            return Err(ProgramSetErrorV2::ZeroSetIdentity);
        }
        if selected_set_id != authenticated_set_id {
            return Err(ProgramSetErrorV2::SetIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode and prevalidate one complete schema-bound set.
    pub fn decode(bytes: &'a [u8]) -> ProgramSetResultV2<Self> {
        if bytes.len() < CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            || bytes.len() > CAPABILITY_PROGRAM_SET_MAX_BYTES_V2
        {
            return Err(ProgramSetErrorV2::InvalidLength);
        }
        if slice(bytes, CAPABILITY_PROGRAM_SET_MAGIC_OFFSET_V2, 8)?
            != CAPABILITY_PROGRAM_SET_MAGIC_V2
        {
            return Err(ProgramSetErrorV2::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_OFFSET_V2)?
            != CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V2
            || read_u16(bytes, CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_OFFSET_V2)?
                != CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V2
        {
            return Err(ProgramSetErrorV2::UnsupportedSchema);
        }
        if byte(bytes, CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2)?
            != CAPABILITY_PROGRAM_SET_CANONICAL_ENDIAN_V2
            || !all_zero(slice(
                bytes,
                CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2,
                CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
                    .checked_sub(CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2)
                    .ok_or(ProgramSetErrorV2::InvalidLength)?,
            )?)
        {
            return Err(ProgramSetErrorV2::NonCanonicalReserved);
        }
        let selector_width = SelectorWidthV2::decode(byte(
            bytes,
            CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2,
        )?)?;
        let entry_count = read_u16(bytes, CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2)?;
        if entry_count == 0 || usize::from(entry_count) > CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2 {
            return Err(ProgramSetErrorV2::InvalidEntryCount);
        }
        let expected = encoded_program_set_bytes_v2(usize::from(entry_count))?;
        if bytes.len() != expected {
            return Err(ProgramSetErrorV2::InvalidLength);
        }
        let value = Self {
            selector_offset: read_u32(bytes, CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2)?,
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
                return Err(ProgramSetErrorV2::NonCanonicalSelectorOrder);
            }
            prior = Some(entry.selector);
            index = index
                .checked_add(1)
                .ok_or(ProgramSetErrorV2::InvalidEntryCount)?;
        }
        Ok(value)
    }

    /// Byte offset of the minimally parsed action selector.
    pub const fn selector_offset(self) -> u32 {
        self.selector_offset
    }

    /// Canonical selector width.
    pub const fn selector_width(self) -> SelectorWidthV2 {
        self.selector_width
    }

    /// Number of strictly ordered action descriptors.
    pub const fn entry_count(self) -> u16 {
        self.entry_count
    }

    /// Decode one exact schema-bound table entry.
    pub fn entry(self, index: u16) -> ProgramSetResultV2<CapabilityProgramSetEntryV2> {
        if index >= self.entry_count {
            return Err(ProgramSetErrorV2::InvalidEntryCount);
        }
        let offset = entry_offset(index)?;
        if !all_zero(slice(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_RESERVED_OFFSET_V2)
                .ok_or(ProgramSetErrorV2::InvalidLength)?,
            4,
        )?) {
            return Err(ProgramSetErrorV2::NonCanonicalReserved);
        }
        let selector = read_u32(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2)
                .ok_or(ProgramSetErrorV2::InvalidLength)?,
        )?;
        let schema = ContentId::new(read_array(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2)
                .ok_or(ProgramSetErrorV2::InvalidLength)?,
        )?)
        .map_err(|_| ProgramSetErrorV2::ZeroDescriptorSchema)?;
        let program = ContentId::new(read_array(
            self.bytes,
            offset
                .checked_add(CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2)
                .ok_or(ProgramSetErrorV2::InvalidLength)?,
        )?)
        .map_err(|_| ProgramSetErrorV2::ZeroDescriptorProgram)?;
        Ok(CapabilityProgramSetEntryV2::new(
            selector,
            CapabilityDescriptorReferenceV2::new(schema, program),
        ))
    }

    /// Select one exact descriptor schema/content coordinate.
    pub fn select_descriptor(
        self,
        request: &[u8],
    ) -> ProgramSetResultV2<CapabilityDescriptorReferenceV2> {
        self.select_entry(request)
            .map(CapabilityProgramSetEntryV2::descriptor)
    }

    /// Select the exact action entry from untrusted request bytes.
    ///
    /// The selected RequestProfile must independently validate the same action
    /// and whole request before any transition or effect executes.
    pub fn select_entry(self, request: &[u8]) -> ProgramSetResultV2<CapabilityProgramSetEntryV2> {
        let selector = read_request_selector(self.selector_offset, self.selector_width, request)?;
        let mut index = 0_u16;
        while index < self.entry_count {
            let entry = self.entry(index)?;
            if entry.selector == selector {
                return Ok(entry);
            }
            if entry.selector > selector {
                break;
            }
            index = index
                .checked_add(1)
                .ok_or(ProgramSetErrorV2::InvalidEntryCount)?;
        }
        Err(ProgramSetErrorV2::MissingSelector)
    }

    /// Require independently authenticated descriptor coordinates.
    pub fn require_descriptor(
        self,
        request: &[u8],
        authenticated_schema: ContentId,
        authenticated_program: ContentId,
    ) -> ProgramSetResultV2<CapabilityDescriptorReferenceV2> {
        let selected = self.select_descriptor(request)?;
        if selected.schema != authenticated_schema || selected.program != authenticated_program {
            return Err(ProgramSetErrorV2::DescriptorMismatch);
        }
        Ok(selected)
    }

    /// Borrow exact canonical finalized-record bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Exact encoded width for a nonempty bounded entry set.
pub fn encoded_program_set_bytes_v2(entry_count: usize) -> ProgramSetResultV2<usize> {
    if entry_count == 0 || entry_count > CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2 {
        return Err(ProgramSetErrorV2::InvalidEntryCount);
    }
    entry_count
        .checked_mul(CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2)
        .and_then(|body| CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2.checked_add(body))
        .ok_or(ProgramSetErrorV2::InvalidLength)
}

/// Encode a canonical schema-bound set into an exact caller-owned buffer.
pub fn encode_program_set_v2(
    selector_offset: u32,
    selector_width: SelectorWidthV2,
    entries: &[CapabilityProgramSetEntryV2],
    output: &mut [u8],
) -> ProgramSetResultV2<()> {
    let expected = encoded_program_set_bytes_v2(entries.len())?;
    if output.len() != expected {
        return Err(ProgramSetErrorV2::InvalidLength);
    }
    output.fill(0);
    put(
        output,
        CAPABILITY_PROGRAM_SET_MAGIC_OFFSET_V2,
        &CAPABILITY_PROGRAM_SET_MAGIC_V2,
    )?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_OFFSET_V2,
        &CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V2.to_le_bytes(),
    )?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_OFFSET_V2,
        &CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V2.to_le_bytes(),
    )?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2,
        &selector_offset.to_le_bytes(),
    )?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2,
        &[selector_width.bytes()],
    )?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2,
        &[CAPABILITY_PROGRAM_SET_CANONICAL_ENDIAN_V2],
    )?;
    let count = u16::try_from(entries.len()).map_err(|_| ProgramSetErrorV2::InvalidEntryCount)?;
    put(
        output,
        CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2,
        &count.to_le_bytes(),
    )?;
    let mut prior = None;
    for (index, entry) in entries.iter().copied().enumerate() {
        if entry.selector > selector_width.maximum()
            || prior.is_some_and(|selector| selector >= entry.selector)
        {
            return Err(ProgramSetErrorV2::NonCanonicalSelectorOrder);
        }
        prior = Some(entry.selector);
        let offset =
            entry_offset(u16::try_from(index).map_err(|_| ProgramSetErrorV2::InvalidEntryCount)?)?;
        put(
            output,
            offset + CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2,
            &entry.selector.to_le_bytes(),
        )?;
        put(
            output,
            offset + CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2,
            &entry.descriptor.schema.to_bytes(),
        )?;
        put(
            output,
            offset + CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2,
            &entry.descriptor.program.to_bytes(),
        )?;
    }
    CapabilityProgramSetV2::decode(output).map(|_| ())
}

fn read_request_selector(
    selector_offset: u32,
    selector_width: SelectorWidthV2,
    request: &[u8],
) -> ProgramSetResultV2<u32> {
    let start =
        usize::try_from(selector_offset).map_err(|_| ProgramSetErrorV2::SelectorOutOfBounds)?;
    let width = usize::from(selector_width.bytes());
    let selector_bytes = request
        .get(
            start
                ..start
                    .checked_add(width)
                    .ok_or(ProgramSetErrorV2::SelectorOutOfBounds)?,
        )
        .ok_or(ProgramSetErrorV2::SelectorOutOfBounds)?;
    match selector_width {
        SelectorWidthV2::U8 => selector_bytes
            .first()
            .copied()
            .map(u32::from)
            .ok_or(ProgramSetErrorV2::SelectorOutOfBounds),
        SelectorWidthV2::U16 => Ok(u32::from(u16::from_le_bytes(
            selector_bytes
                .try_into()
                .map_err(|_| ProgramSetErrorV2::SelectorOutOfBounds)?,
        ))),
        SelectorWidthV2::U32 => Ok(u32::from_le_bytes(
            selector_bytes
                .try_into()
                .map_err(|_| ProgramSetErrorV2::SelectorOutOfBounds)?,
        )),
    }
}

fn entry_offset(index: u16) -> ProgramSetResultV2<usize> {
    usize::from(index)
        .checked_mul(CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2)
        .and_then(|body| CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2.checked_add(body))
        .ok_or(ProgramSetErrorV2::InvalidLength)
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn byte(bytes: &[u8], offset: usize) -> ProgramSetResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ProgramSetErrorV2::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> ProgramSetResultV2<&[u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(ProgramSetErrorV2::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(ProgramSetErrorV2::InvalidLength)
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> ProgramSetResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProgramSetErrorV2::InvalidLength)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ProgramSetErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> ProgramSetResultV2<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| ProgramSetErrorV2::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> ProgramSetResultV2<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| ProgramSetErrorV2::InvalidLength)?,
    ))
}

fn read_array(bytes: &[u8], offset: usize) -> ProgramSetResultV2<[u8; 32]> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| ProgramSetErrorV2::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero identity")
    }

    #[test]
    fn generated_set_selects_exact_schema_and_program() {
        let set = CapabilityProgramSetV2::decode(&CAPABILITY_PROGRAM_SET_EXAMPLE_V2)
            .expect("generated set");
        assert_eq!(set.selector_offset(), 10);
        assert_eq!(set.selector_width(), SelectorWidthV2::U8);
        assert_eq!(set.entry_count(), 3);
        let mut request = [0_u8; 11];
        request[10] = 3;
        let selected = set.select_descriptor(&request).expect("selected pair");
        assert_eq!(selected.schema(), id(0x42));
        assert_eq!(selected.program(), id(0x22));
        assert_eq!(
            set.require_descriptor(&request, id(0x42), id(0x22)),
            Ok(selected)
        );
        assert_eq!(
            set.require_descriptor(&request, id(0x41), id(0x22)),
            Err(ProgramSetErrorV2::DescriptorMismatch)
        );
        assert_eq!(
            set.require_descriptor(&request, id(0x42), id(0x23)),
            Err(ProgramSetErrorV2::DescriptorMismatch)
        );
    }

    #[test]
    fn generated_hostile_corpus_and_set_substitution_refuse() {
        for hostile in CAPABILITY_PROGRAM_SET_HOSTILE_CORPUS_V2 {
            assert!(CapabilityProgramSetV2::decode(&hostile).is_err());
        }
        assert_eq!(
            CapabilityProgramSetV2::decode_selected(
                [1; 32],
                [2; 32],
                &CAPABILITY_PROGRAM_SET_EXAMPLE_V2,
            ),
            Err(ProgramSetErrorV2::SetIdentityMismatch)
        );
    }

    #[test]
    fn operator_encoder_is_exact_and_refuses_reordered_entries() {
        let entries = [
            CapabilityProgramSetEntryV2::new(
                1,
                CapabilityDescriptorReferenceV2::new(id(0x41), id(0x11)),
            ),
            CapabilityProgramSetEntryV2::new(
                3,
                CapabilityDescriptorReferenceV2::new(id(0x42), id(0x22)),
            ),
            CapabilityProgramSetEntryV2::new(
                7,
                CapabilityDescriptorReferenceV2::new(id(0x43), id(0x33)),
            ),
        ];
        let mut output = vec![0; encoded_program_set_bytes_v2(entries.len()).expect("width")];
        encode_program_set_v2(10, SelectorWidthV2::U8, &entries, &mut output).expect("encode");
        assert_eq!(output, CAPABILITY_PROGRAM_SET_EXAMPLE_V2);

        let reordered = [entries[1], entries[0]];
        let mut invalid = vec![0; encoded_program_set_bytes_v2(reordered.len()).expect("width")];
        assert_eq!(
            encode_program_set_v2(10, SelectorWidthV2::U8, &reordered, &mut invalid),
            Err(ProgramSetErrorV2::NonCanonicalSelectorOrder)
        );
    }
}
