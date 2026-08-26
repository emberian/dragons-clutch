//! Fixed Capability Program V3 descriptor.
//!
//! V3 carries content identities for three independently finalized physical
//! artifacts: AccountProfile, Effect, and Transition. It does not embed VM
//! bytes, select a family tag, or treat any schema identity as arbitrary byte
//! authority.

use core::convert::TryInto;

use dclutch_capability_contract::CapabilityEntryV1;
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;

use super::{CAPABILITY_ROOT_HEADER_BYTES_V1, CAPABILITY_ROOT_STATE_MAX_BYTES_V1, Error, Result};

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_v3.rs"]
mod generated;

pub use generated::*;

/// Schema label for finalized [`CapabilityProgramV3`] records.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/capability-program-v3";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0x0e, 0x33, 0xb2, 0x5f, 0x91, 0x03, 0xd4, 0x84, 0x3e, 0x6f, 0x2f, 0xdc, 0x7b, 0x31, 0x86, 0x46,
    0x57, 0x3f, 0x45, 0xad, 0x71, 0xb5, 0x4a, 0xa2, 0x35, 0x24, 0x37, 0x0b, 0x22, 0xa0, 0xbb, 0x84,
];

/// Hostile-decoded fixed V3 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramV3 {
    kind: ContentId,
    config_schema: ContentId,
    request_schema: ContentId,
    root_schema: ContentId,
    account_profile: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    effect_schema: ContentId,
    request_profile_schema: ContentId,
    request_profile_program: ContentId,
    transition_schema: ContentId,
    transition_program: ContentId,
    root_state_bytes: u32,
}

impl CapabilityProgramV3 {
    /// Construct one descriptor from twelve nonzero finalized content identities.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: ContentId,
        config_schema: ContentId,
        request_schema: ContentId,
        root_schema: ContentId,
        account_profile: ContentId,
        derivation_policy: ContentId,
        capacity_profile: ContentId,
        effect_schema: ContentId,
        request_profile_schema: ContentId,
        request_profile_program: ContentId,
        transition_schema: ContentId,
        transition_program: ContentId,
        root_state_bytes: u32,
    ) -> Result<Self> {
        if root_state_bytes == 0
            || usize::try_from(root_state_bytes).map_err(|_| Error::InvalidRootStateBytes)?
                > CAPABILITY_ROOT_STATE_MAX_BYTES_V1
        {
            return Err(Error::InvalidRootStateBytes);
        }
        Ok(Self {
            kind,
            config_schema,
            request_schema,
            root_schema,
            account_profile,
            derivation_policy,
            capacity_profile,
            effect_schema,
            request_profile_schema,
            request_profile_program,
            transition_schema,
            transition_program,
            root_state_bytes,
        })
    }

    /// Hostile-decode one exact 408-byte V3 descriptor.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_PROGRAM_V3_BYTES {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, CAPABILITY_PROGRAM_V3_MAGIC_OFFSET, 8)? != CAPABILITY_PROGRAM_V3_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET)?
            != CAPABILITY_PROGRAM_V3_SCHEMA_VERSION
            || read_u16(bytes, CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET)?
                != CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE
            || read_u16(
                bytes,
                CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET,
            )? != CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION
            || read_u16(
                bytes,
                CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET,
            )? != CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET, 4)?;
        let root_state_bytes = read_u32(bytes, CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET)?;
        if root_state_bytes == 0
            || usize::try_from(root_state_bytes).map_err(|_| Error::InvalidRootStateBytes)?
                > CAPABILITY_ROOT_STATE_MAX_BYTES_V1
        {
            return Err(Error::InvalidRootStateBytes);
        }
        Self::new(
            content(bytes, CAPABILITY_PROGRAM_V3_KIND_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_EFFECT_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET)?,
            root_state_bytes,
        )
    }

    /// Require the complete manifest selection/entry projection.
    pub fn validate_selection(
        self,
        selection: CapabilityExecutionSelectionV1,
        entry: CapabilityEntryV1,
    ) -> Result<()> {
        if self.kind != selection.kind() || entry.kind_id() != selection.kind() {
            return Err(Error::SelectionMismatch);
        }
        if entry.release_id() != selection.capability_release()
            || entry.config_id() != selection.config()
            || self.capacity_profile != entry.capacity_profile_id()
            || self.root_schema != entry.child_schema_id()
            || self.derivation_policy != entry.child_derivation_id()
        {
            return Err(Error::ManifestEntryMismatch);
        }
        Ok(())
    }

    /// Require the kind stored in an already authenticated root selector.
    pub fn validate_persisted_selection(
        self,
        selection: CapabilityExecutionSelectionV1,
    ) -> Result<()> {
        if self.kind == selection.kind() {
            Ok(())
        } else {
            Err(Error::SelectionMismatch)
        }
    }

    /// Join both independently authenticated interpreter artifacts exactly.
    pub fn validate_interpreter_artifacts(
        self,
        authenticated_request_profile_schema: ContentId,
        authenticated_request_profile_program: ContentId,
        authenticated_transition_schema: ContentId,
        authenticated_transition_program: ContentId,
    ) -> Result<()> {
        if self.request_profile_schema != authenticated_request_profile_schema
            || self.request_profile_program != authenticated_request_profile_program
            || self.transition_schema != authenticated_transition_schema
            || self.transition_program != authenticated_transition_program
        {
            Err(Error::UnsupportedContent)
        } else {
            Ok(())
        }
    }

    /// Selected capability kind.
    pub const fn kind(self) -> ContentId {
        self.kind
    }

    /// Config record schema identity.
    pub const fn config_schema(self) -> ContentId {
        self.config_schema
    }

    /// Family request schema identity.
    pub const fn request_schema(self) -> ContentId {
        self.request_schema
    }

    /// Mutable root-tail schema identity.
    pub const fn root_schema(self) -> ContentId {
        self.root_schema
    }

    /// AccountProfile finalized content identity.
    pub const fn account_profile(self) -> ContentId {
        self.account_profile
    }

    /// Child-root derivation policy identity.
    pub const fn derivation_policy(self) -> ContentId {
        self.derivation_policy
    }

    /// Physical capacity-profile content identity.
    pub const fn capacity_profile(self) -> ContentId {
        self.capacity_profile
    }

    /// Effect finalized content identity.
    pub const fn effect_schema(self) -> ContentId {
        self.effect_schema
    }

    /// RequestProfile finalized-record schema identity.
    pub const fn request_profile_schema(self) -> ContentId {
        self.request_profile_schema
    }

    /// SHA-256 identity of the exact finalized RequestProfile bytes.
    pub const fn request_profile_program(self) -> ContentId {
        self.request_profile_program
    }

    /// Transition finalized-record schema identity.
    pub const fn transition_schema(self) -> ContentId {
        self.transition_schema
    }

    /// SHA-256 identity of the exact finalized Transition bytes.
    pub const fn transition_program(self) -> ContentId {
        self.transition_program
    }

    /// Explicit Transition artifact schema version.
    pub const fn transition_schema_version(self) -> u16 {
        CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION
    }

    /// Explicit RequestProfile artifact schema version.
    pub const fn request_profile_schema_version(self) -> u16 {
        CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION
    }

    /// Exact mutable root-tail width.
    pub const fn root_state_bytes(self) -> u32 {
        self.root_state_bytes
    }

    /// Exact composite Trading root-account width.
    pub fn root_account_bytes(self) -> Result<usize> {
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(
                usize::try_from(self.root_state_bytes).map_err(|_| Error::InvalidRootStateBytes)?,
            )
            .ok_or(Error::InvalidRootStateBytes)
    }

    /// Encode exact canonical descriptor bytes.
    pub fn encode(self) -> [u8; CAPABILITY_PROGRAM_V3_BYTES] {
        let mut output = [0_u8; CAPABILITY_PROGRAM_V3_BYTES];
        write(
            &mut output,
            CAPABILITY_PROGRAM_V3_MAGIC_OFFSET,
            &CAPABILITY_PROGRAM_V3_MAGIC,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_SCHEMA_VERSION,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION,
        );
        for (offset, value) in [
            (CAPABILITY_PROGRAM_V3_KIND_OFFSET, self.kind),
            (
                CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET,
                self.config_schema,
            ),
            (
                CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET,
                self.request_schema,
            ),
            (CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET, self.root_schema),
            (
                CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET,
                self.account_profile,
            ),
            (
                CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET,
                self.derivation_policy,
            ),
            (
                CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET,
                self.capacity_profile,
            ),
            (
                CAPABILITY_PROGRAM_V3_EFFECT_SCHEMA_OFFSET,
                self.effect_schema,
            ),
            (
                CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET,
                self.request_profile_schema,
            ),
            (
                CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET,
                self.request_profile_program,
            ),
            (
                CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET,
                self.transition_schema,
            ),
            (
                CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET,
                self.transition_program,
            ),
        ] {
            write(&mut output, offset, &value.to_bytes());
        }
        write(
            &mut output,
            CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET,
            &self.root_state_bytes.to_le_bytes(),
        );
        output
    }
}

/// Borrowed exact view of one header plus V3-descriptor-sized family tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRootAccountV3<'a> {
    header: super::CapabilityRootHeaderV1,
    state: &'a [u8],
}

impl<'a> CapabilityRootAccountV3<'a> {
    /// Hostile-decode one exact composite Trading root.
    pub fn decode(bytes: &'a [u8], program: CapabilityProgramV3) -> Result<Self> {
        if bytes.len() != program.root_account_bytes()? {
            return Err(Error::InvalidLength);
        }
        let (header, state) = bytes.split_at(CAPABILITY_ROOT_HEADER_BYTES_V1);
        Ok(Self {
            header: super::CapabilityRootHeaderV1::decode(header)?,
            state,
        })
    }

    /// Immutable common activation header.
    pub const fn header(self) -> super::CapabilityRootHeaderV1 {
        self.header
    }

    /// Exact descriptor-schema-owned mutable family state.
    pub const fn state(self) -> &'a [u8] {
        self.state
    }
}

/// Initialize a caller-owned candidate composite root without partial output.
pub fn initialize_root_account_v3(
    output: &mut [u8],
    header: super::CapabilityRootHeaderV1,
    program: CapabilityProgramV3,
    initial_state: &[u8],
) -> Result<()> {
    let expected = program.root_account_bytes()?;
    let state_bytes =
        usize::try_from(program.root_state_bytes).map_err(|_| Error::InvalidRootStateBytes)?;
    if output.len() != expected || initial_state.len() != state_bytes {
        return Err(Error::InvalidLength);
    }
    let (header_output, state_output) = output.split_at_mut(CAPABILITY_ROOT_HEADER_BYTES_V1);
    header_output.copy_from_slice(&header.to_bytes());
    state_output.copy_from_slice(initial_state);
    Ok(())
}

/// Authenticate a V3 composite root and expose only its mutable family tail.
pub fn split_root_account_mut_v3(
    bytes: &mut [u8],
    program: CapabilityProgramV3,
) -> Result<(super::CapabilityRootHeaderV1, &mut [u8])> {
    if bytes.len() != program.root_account_bytes()? {
        return Err(Error::InvalidLength);
    }
    let (header, state) = bytes.split_at_mut(CAPABILITY_ROOT_HEADER_BYTES_V1);
    Ok((super::CapabilityRootHeaderV1::decode(header)?, state))
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?).map_err(|_| Error::ZeroIdentity)
}

fn array(bytes: &[u8], offset: usize) -> Result<[u8; 32]> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonCanonicalReservedBytes)
    }
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) {
    let end = offset.saturating_add(value.len());
    if let Some(destination) = output.get_mut(offset..end) {
        destination.copy_from_slice(value);
    }
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    write(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero")
    }

    fn canonical() -> CapabilityProgramV3 {
        CapabilityProgramV3::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            id(11),
            id(12),
            128,
        )
        .expect("descriptor")
    }

    #[test]
    fn exact_descriptor_round_trips() {
        let value = canonical();
        let bytes = value.encode();
        assert_eq!(CapabilityProgramV3::decode(&bytes), Ok(value));
        assert_eq!(value.root_account_bytes(), Ok(360));
        let header = super::super::CapabilityRootHeaderV1::new(
            id(11),
            [12; 32],
            13,
            CapabilityExecutionSelectionV1::new(0, id(14), id(1), id(15), id(16))
                .expect("selection"),
        )
        .expect("header");
        let mut root = [0_u8; 360];
        initialize_root_account_v3(&mut root, header, value, &[0; 128]).expect("initialize");
        let decoded = CapabilityRootAccountV3::decode(&root, value).expect("root");
        assert_eq!(decoded.header(), header);
        assert_eq!(decoded.state(), &[0; 128]);
    }

    #[test]
    fn hostile_versions_reserved_ids_and_width_refuse() {
        let canonical = canonical().encode();
        for offset in [
            CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET,
        ] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(CapabilityProgramV3::decode(&hostile).is_err());
        }
        for offset in [
            CAPABILITY_PROGRAM_V3_KIND_OFFSET,
            CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET,
            CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V3_EFFECT_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile
                .get_mut(offset..offset + 32)
                .expect("identity")
                .fill(0);
            assert_eq!(
                CapabilityProgramV3::decode(&hostile),
                Err(Error::ZeroIdentity)
            );
        }
        assert_eq!(
            CapabilityProgramV3::decode(&canonical[..CAPABILITY_PROGRAM_V3_BYTES - 1]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn swapped_schema_and_program_identities_refuse_exact_join() {
        let value = canonical();
        assert_eq!(
            value.validate_interpreter_artifacts(id(9), id(10), id(11), id(12)),
            Ok(())
        );
        assert_eq!(
            value.validate_interpreter_artifacts(id(10), id(9), id(11), id(12)),
            Err(Error::UnsupportedContent)
        );
        assert_eq!(
            value.validate_interpreter_artifacts(id(9), id(10), id(12), id(11)),
            Err(Error::UnsupportedContent)
        );
    }
}
