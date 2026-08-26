//! Construction of schema-bound CapabilityProgramSetV2 artifacts.
//!
//! This host-only helper consumes exact descriptor schema/content coordinates,
//! emits the canonical Lean-owned set bytes, and reports their content digest.
//! It does not authenticate Registry finalization or choose a decoder; callers
//! must derive every coordinate from the same finalized chain observation.

use dclutch_capability_program_contract::set_v2::{
    CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
    CapabilityProgramSetEntryV2, CapabilityProgramSetV2, ProgramSetErrorV2, SelectorWidthV2,
    encode_program_set_v2, encoded_program_set_bytes_v2,
};
use dclutch_core_contract::ContentId;
use solana_program::hash::hash;

/// Stable schema-bound set construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityProgramSetBuildErrorV2 {
    /// The Lean-owned set codec refused the supplied table or output.
    ProgramSet(ProgramSetErrorV2),
    /// SHA-256 unexpectedly produced the forbidden zero content identity.
    ZeroContentIdentity,
}

impl From<ProgramSetErrorV2> for CapabilityProgramSetBuildErrorV2 {
    fn from(value: ProgramSetErrorV2) -> Self {
        Self::ProgramSet(value)
    }
}

/// Canonical bytes and exact Registry publication identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityProgramSetArtifactV2 {
    /// Exact canonical CapabilityProgramSetV2 bytes.
    pub bytes: Vec<u8>,
    /// Lean-owned finalized-record schema identity.
    pub schema_id: ContentId,
    /// SHA-256 identity of `bytes` with no extra domain prefix.
    pub content_id: ContentId,
}

/// Build one canonical schema-bound action table.
pub fn build_capability_program_set_v2(
    selector_offset: u32,
    selector_width: SelectorWidthV2,
    entries: &[CapabilityProgramSetEntryV2],
) -> Result<CapabilityProgramSetArtifactV2, CapabilityProgramSetBuildErrorV2> {
    let mut bytes = vec![0; encoded_program_set_bytes_v2(entries.len())?];
    encode_program_set_v2(selector_offset, selector_width, entries, &mut bytes)?;
    CapabilityProgramSetV2::decode(&bytes)?;
    let schema_id = ContentId::new(CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2)
        .map_err(|_| CapabilityProgramSetBuildErrorV2::ZeroContentIdentity)?;
    let content_id = ContentId::new(hash(&bytes).to_bytes())
        .map_err(|_| CapabilityProgramSetBuildErrorV2::ZeroContentIdentity)?;
    Ok(CapabilityProgramSetArtifactV2 {
        bytes,
        schema_id,
        content_id,
    })
}

/// Construct one entry without exposing physical offsets to the operator.
pub const fn capability_program_set_entry_v2(
    selector: u32,
    descriptor_schema: ContentId,
    descriptor_program: ContentId,
) -> CapabilityProgramSetEntryV2 {
    CapabilityProgramSetEntryV2::new(
        selector,
        CapabilityDescriptorReferenceV2::new(descriptor_schema, descriptor_program),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    #[test]
    fn builder_reports_exact_schema_content_and_selected_pair() {
        let entries = [
            capability_program_set_entry_v2(1, id(0x41), id(0x11)),
            capability_program_set_entry_v2(3, id(0x42), id(0x22)),
        ];
        let artifact =
            build_capability_program_set_v2(10, SelectorWidthV2::U8, &entries).expect("artifact");
        assert_eq!(
            artifact.schema_id.to_bytes(),
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2
        );
        assert_eq!(
            artifact.content_id.to_bytes(),
            hash(&artifact.bytes).to_bytes()
        );
        let set = CapabilityProgramSetV2::decode(&artifact.bytes).expect("decode artifact");
        let mut request = [0_u8; 11];
        request[10] = 3;
        assert_eq!(
            set.select_descriptor(&request).expect("selected"),
            CapabilityDescriptorReferenceV2::new(id(0x42), id(0x22))
        );
    }

    #[test]
    fn schema_substitution_changes_authority_and_cannot_match_expected_pair() {
        let expected = [capability_program_set_entry_v2(1, id(0x41), id(0x11))];
        let substituted = [capability_program_set_entry_v2(1, id(0x42), id(0x11))];
        let expected_artifact =
            build_capability_program_set_v2(0, SelectorWidthV2::U8, &expected).expect("expected");
        let substituted_artifact =
            build_capability_program_set_v2(0, SelectorWidthV2::U8, &substituted)
                .expect("substituted");
        assert_ne!(
            expected_artifact.content_id,
            substituted_artifact.content_id
        );
        let set = CapabilityProgramSetV2::decode(&substituted_artifact.bytes).expect("decode");
        assert_eq!(
            set.require_descriptor(&[1], id(0x41), id(0x11)),
            Err(ProgramSetErrorV2::DescriptorMismatch)
        );
    }
}
