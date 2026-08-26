//! Fixed Capability Program V4 descriptor.
//!
//! V4 gives every executable artifact class an explicit finalized-record
//! schema identity and an exact content identity. Adapters must authenticate
//! each raw/staging pair under the descriptor-selected schema before choosing
//! an implemented hostile decoder. Raw magic and caller hints are never schema
//! authority.

use core::convert::TryInto;

pub use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5 as SELECTED_LIFECYCLE_SCHEMA_RELEASE_PREIMAGE_V5,
};
use dclutch_capability_contract::CapabilityEntryV1;
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;

use super::{CAPABILITY_ROOT_HEADER_BYTES_V1, CAPABILITY_ROOT_STATE_MAX_BYTES_V1, Error, Result};

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_v4.rs"]
mod generated;

pub use generated::*;

/// Schema label for finalized [`CapabilityProgramV4`] records.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/capability-program-v4";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0x2d, 0x85, 0xb2, 0x21, 0x7c, 0x9b, 0x58, 0xbb, 0x59, 0xb8, 0x5d, 0x43, 0x7f, 0xf4, 0xd1, 0x7f,
    0xa0, 0x70, 0x58, 0xd9, 0x5d, 0xee, 0xb7, 0xd2, 0x58, 0x43, 0xa6, 0xea, 0x31, 0x30, 0x11, 0x62,
];

/// One independently finalized executable artifact coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactReferenceV4 {
    schema: ContentId,
    program: ContentId,
}

impl ArtifactReferenceV4 {
    /// Bind one exact schema release and exact content digest.
    pub const fn new(schema: ContentId, program: ContentId) -> Self {
        Self { schema, program }
    }

    /// Finalized-record schema identity.
    pub const fn schema(self) -> ContentId {
        self.schema
    }

    /// SHA-256 identity of the exact finalized bytes.
    pub const fn program(self) -> ContentId {
        self.program
    }
}

/// Complete authenticated executable-artifact projection for one descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityArtifactsV4 {
    /// AccountProfile interpreter artifact.
    pub account_profile: ArtifactReferenceV4,
    /// RequestProfile interpreter artifact.
    pub request_profile: ArtifactReferenceV4,
    /// StateLifecyclePolicy artifact.
    pub lifecycle: ArtifactReferenceV4,
    /// ExecutionStrategy artifact.
    pub strategy: ArtifactReferenceV4,
    /// Underlying Transition interpreter artifact.
    pub transition: ArtifactReferenceV4,
    /// Effect interpreter artifact.
    pub effect: ArtifactReferenceV4,
}

/// Hostile-decoded fixed V4 descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramV4 {
    kind: ContentId,
    config_schema: ContentId,
    request_schema: ContentId,
    root_schema: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    artifacts: CapabilityArtifactsV4,
    root_state_bytes: u32,
}

impl CapabilityProgramV4 {
    /// Construct one descriptor from eighteen nonzero identities.
    ///
    /// The lifecycle artifact must select the sole production V5 schema. A
    /// capability with no lifecycle actions selects the canonical empty V5
    /// artifact instead of naming an older or parallel lifecycle schema.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: ContentId,
        config_schema: ContentId,
        request_schema: ContentId,
        root_schema: ContentId,
        derivation_policy: ContentId,
        capacity_profile: ContentId,
        artifacts: CapabilityArtifactsV4,
        root_state_bytes: u32,
    ) -> Result<Self> {
        if artifacts.lifecycle.schema.to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5 {
            return Err(Error::UnsupportedSchema);
        }
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
            derivation_policy,
            capacity_profile,
            artifacts,
            root_state_bytes,
        })
    }

    /// Hostile-decode one exact 600-byte V4 descriptor.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_PROGRAM_V4_BYTES {
            return Err(Error::InvalidLength);
        }
        if slice(bytes, CAPABILITY_PROGRAM_V4_MAGIC_OFFSET, 8)? != CAPABILITY_PROGRAM_V4_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET)?
            != CAPABILITY_PROGRAM_V4_SCHEMA_VERSION
            || read_u16(bytes, CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET)?
                != CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET, 4)?;
        require_zero(bytes, CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET, 4)?;
        Self::new(
            content(bytes, CAPABILITY_PROGRAM_V4_KIND_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET)?,
            content(bytes, CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET)?,
            CapabilityArtifactsV4 {
                account_profile: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
                )?,
                request_profile: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET,
                )?,
                lifecycle: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET,
                )?,
                strategy: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET,
                )?,
                transition: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET,
                )?,
                effect: artifact(
                    bytes,
                    CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
                    CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET,
                )?,
            },
            read_u32(bytes, CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET)?,
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

    /// Join every independently authenticated executable artifact exactly.
    pub fn validate_artifacts(self, authenticated: CapabilityArtifactsV4) -> Result<()> {
        if self.artifacts == authenticated {
            Ok(())
        } else {
            Err(Error::UnsupportedContent)
        }
    }

    /// Require the authenticated Strategy artifact and its selected underlying
    /// Transition pair to equal the descriptor's two acyclic edges.
    pub fn validate_strategy_transition(
        self,
        strategy: ArtifactReferenceV4,
        strategy_transition: ArtifactReferenceV4,
    ) -> Result<()> {
        if strategy == self.artifacts.strategy && strategy_transition == self.artifacts.transition {
            Ok(())
        } else {
            Err(Error::UnsupportedContent)
        }
    }

    /// Selected capability kind.
    pub const fn kind(self) -> ContentId {
        self.kind
    }

    /// Config record schema identity. The selected config digest remains in
    /// the authenticated execution selection.
    pub const fn config_schema(self) -> ContentId {
        self.config_schema
    }

    /// Family request semantic schema identity.
    pub const fn request_schema(self) -> ContentId {
        self.request_schema
    }

    /// Mutable root-tail semantic schema identity.
    pub const fn root_schema(self) -> ContentId {
        self.root_schema
    }

    /// Manifest-joined child derivation policy identity.
    pub const fn derivation_policy(self) -> ContentId {
        self.derivation_policy
    }

    /// Physical capacity-profile content identity.
    pub const fn capacity_profile(self) -> ContentId {
        self.capacity_profile
    }

    /// Complete executable-artifact coordinate set.
    pub const fn artifacts(self) -> CapabilityArtifactsV4 {
        self.artifacts
    }

    /// AccountProfile schema/content pair.
    pub const fn account_profile(self) -> ArtifactReferenceV4 {
        self.artifacts.account_profile
    }

    /// RequestProfile schema/content pair.
    pub const fn request_profile(self) -> ArtifactReferenceV4 {
        self.artifacts.request_profile
    }

    /// StateLifecyclePolicy schema/content pair.
    pub const fn lifecycle(self) -> ArtifactReferenceV4 {
        self.artifacts.lifecycle
    }

    /// ExecutionStrategy schema/content pair.
    pub const fn strategy(self) -> ArtifactReferenceV4 {
        self.artifacts.strategy
    }

    /// Underlying Transition schema/content pair.
    pub const fn transition(self) -> ArtifactReferenceV4 {
        self.artifacts.transition
    }

    /// Effect schema/content pair.
    pub const fn effect(self) -> ArtifactReferenceV4 {
        self.artifacts.effect
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
    pub fn encode(self) -> [u8; CAPABILITY_PROGRAM_V4_BYTES] {
        let mut output = [0_u8; CAPABILITY_PROGRAM_V4_BYTES];
        write(
            &mut output,
            CAPABILITY_PROGRAM_V4_MAGIC_OFFSET,
            &CAPABILITY_PROGRAM_V4_MAGIC,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V4_SCHEMA_VERSION,
        );
        write_u16(
            &mut output,
            CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE,
        );
        for (offset, value) in [
            (CAPABILITY_PROGRAM_V4_KIND_OFFSET, self.kind),
            (
                CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET,
                self.config_schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET,
                self.request_schema,
            ),
            (CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET, self.root_schema),
            (
                CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET,
                self.derivation_policy,
            ),
            (
                CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET,
                self.capacity_profile,
            ),
            (
                CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET,
                self.artifacts.account_profile.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
                self.artifacts.account_profile.program,
            ),
            (
                CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET,
                self.artifacts.request_profile.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET,
                self.artifacts.request_profile.program,
            ),
            (
                CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
                self.artifacts.lifecycle.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET,
                self.artifacts.lifecycle.program,
            ),
            (
                CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET,
                self.artifacts.strategy.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET,
                self.artifacts.strategy.program,
            ),
            (
                CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET,
                self.artifacts.transition.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET,
                self.artifacts.transition.program,
            ),
            (
                CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
                self.artifacts.effect.schema,
            ),
            (
                CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET,
                self.artifacts.effect.program,
            ),
        ] {
            write(&mut output, offset, &value.to_bytes());
        }
        write(
            &mut output,
            CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET,
            &self.root_state_bytes.to_le_bytes(),
        );
        output
    }
}

/// Borrowed exact view of one header plus V4-descriptor-sized family tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRootAccountV4<'a> {
    header: super::CapabilityRootHeaderV1,
    state: &'a [u8],
}

impl<'a> CapabilityRootAccountV4<'a> {
    /// Hostile-decode one exact composite Trading root.
    pub fn decode(bytes: &'a [u8], program: CapabilityProgramV4) -> Result<Self> {
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
pub fn initialize_root_account_v4(
    output: &mut [u8],
    header: super::CapabilityRootHeaderV1,
    program: CapabilityProgramV4,
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

/// Authenticate a V4 composite root and expose only its mutable family tail.
pub fn split_root_account_mut_v4(
    bytes: &mut [u8],
    program: CapabilityProgramV4,
) -> Result<(super::CapabilityRootHeaderV1, &mut [u8])> {
    if bytes.len() != program.root_account_bytes()? {
        return Err(Error::InvalidLength);
    }
    let (header, state) = bytes.split_at_mut(CAPABILITY_ROOT_HEADER_BYTES_V1);
    Ok((super::CapabilityRootHeaderV1::decode(header)?, state))
}

fn artifact(bytes: &[u8], schema: usize, program: usize) -> Result<ArtifactReferenceV4> {
    Ok(ArtifactReferenceV4::new(
        content(bytes, schema)?,
        content(bytes, program)?,
    ))
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

    fn artifacts() -> CapabilityArtifactsV4 {
        CapabilityArtifactsV4 {
            account_profile: ArtifactReferenceV4::new(id(7), id(8)),
            request_profile: ArtifactReferenceV4::new(id(9), id(10)),
            lifecycle: ArtifactReferenceV4::new(
                ContentId::new(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)
                    .expect("selected lifecycle schema"),
                id(12),
            ),
            strategy: ArtifactReferenceV4::new(id(13), id(14)),
            transition: ArtifactReferenceV4::new(id(15), id(16)),
            effect: ArtifactReferenceV4::new(id(17), id(18)),
        }
    }

    fn canonical() -> CapabilityProgramV4 {
        CapabilityProgramV4::new(id(1), id(2), id(3), id(4), id(5), id(6), artifacts(), 128)
            .expect("descriptor")
    }

    #[test]
    fn exact_descriptor_and_root_round_trip() {
        let value = canonical();
        let bytes = value.encode();
        assert_eq!(CapabilityProgramV4::decode(&bytes), Ok(value));
        assert_eq!(value.root_account_bytes(), Ok(360));
        let header = super::super::CapabilityRootHeaderV1::new(
            id(19),
            [20; 32],
            21,
            CapabilityExecutionSelectionV1::new(0, id(22), id(1), id(23), id(24))
                .expect("selection"),
        )
        .expect("header");
        let mut root = [0_u8; 360];
        initialize_root_account_v4(&mut root, header, value, &[0; 128]).expect("initialize");
        let decoded = CapabilityRootAccountV4::decode(&root, value).expect("root");
        assert_eq!(decoded.header(), header);
        assert_eq!(decoded.state(), &[0; 128]);
        assert_eq!(
            split_root_account_mut_v4(&mut root, value).map(|pair| pair.0),
            Ok(header)
        );
    }

    #[test]
    fn hostile_header_reserved_ids_width_and_root_size_refuse() {
        let canonical = canonical().encode();
        for offset in [
            CAPABILITY_PROGRAM_V4_MAGIC_OFFSET,
            CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET,
            CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET,
            CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET,
        ] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(CapabilityProgramV4::decode(&hostile).is_err());
        }
        for offset in [
            CAPABILITY_PROGRAM_V4_KIND_OFFSET,
            CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET,
            CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET,
            CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET,
            CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
            CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET,
        ] {
            let mut hostile = canonical;
            hostile
                .get_mut(offset..offset + 32)
                .expect("identity")
                .fill(0);
            assert_eq!(
                CapabilityProgramV4::decode(&hostile),
                Err(Error::ZeroIdentity)
            );
        }
        let mut zero_root = canonical;
        zero_root
            .get_mut(
                CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET
                    ..CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET + 4,
            )
            .expect("root size")
            .fill(0);
        assert_eq!(
            CapabilityProgramV4::decode(&zero_root),
            Err(Error::InvalidRootStateBytes)
        );
        assert_eq!(
            CapabilityProgramV4::decode(&canonical[..CAPABILITY_PROGRAM_V4_BYTES - 1]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn every_schema_program_swap_and_strategy_transition_mismatch_refuses() {
        let value = canonical();
        assert_eq!(value.validate_artifacts(artifacts()), Ok(()));
        for index in 0_u8..6 {
            let mut hostile = artifacts();
            let selected = match index {
                0 => &mut hostile.account_profile,
                1 => &mut hostile.request_profile,
                2 => &mut hostile.lifecycle,
                3 => &mut hostile.strategy,
                4 => &mut hostile.transition,
                _ => &mut hostile.effect,
            };
            *selected = ArtifactReferenceV4::new(selected.program(), selected.schema());
            assert_eq!(
                value.validate_artifacts(hostile),
                Err(Error::UnsupportedContent)
            );
        }
        assert_eq!(
            value.validate_strategy_transition(artifacts().strategy, artifacts().transition),
            Ok(())
        );
        assert_eq!(
            value.validate_strategy_transition(artifacts().transition, artifacts().strategy),
            Err(Error::UnsupportedContent)
        );
    }

    #[test]
    fn only_v5_lifecycle_schema_can_be_selected() {
        let mut legacy = artifacts();
        legacy.lifecycle = ArtifactReferenceV4::new(
            ContentId::new(
                dclutch_account_profile_contract::lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID,
            )
            .expect("legacy V4 lifecycle schema"),
            id(12),
        );
        assert_eq!(
            CapabilityProgramV4::new(id(1), id(2), id(3), id(4), id(5), id(6), legacy, 128),
            Err(Error::UnsupportedSchema)
        );

        let mut hostile = canonical().encode();
        hostile[CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET
            ..CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET + 32]
            .copy_from_slice(
                &dclutch_account_profile_contract::lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID,
            );
        assert_eq!(
            CapabilityProgramV4::decode(&hostile),
            Err(Error::UnsupportedSchema)
        );
    }
}
