//! Exact seven-action General V3 release admission.
//!
//! A General release is not one permissive descriptor with an internal action
//! switch. Its authenticated [`CapabilityProgramSetV2`] contains exactly one
//! schema-bound `CapabilityProgramV4` identity for each canonical General action.
//! This module validates that closed table without allocating and then joins
//! every selected descriptor to its complete finalized artifact bundle.

use dclutch_capability_program_contract::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
};
use dclutch_core_contract::ContentId;
use dclutch_general_codec::Action;
use sha2::{Digest, Sha256};

use crate::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactErrorV3, GeneralArtifactSelectionV3,
    authenticate_general_artifacts_v3,
};

/// Number of exact action-selected programs in one General V3 release.
pub const GENERAL_ACTION_PROGRAM_COUNT_V3: usize = 7;

/// Canonical strictly increasing General action order.
pub const GENERAL_ACTIONS_V3: [Action; GENERAL_ACTION_PROGRAM_COUNT_V3] = [
    Action::Consider,
    Action::Freeze,
    Action::InitializeSettlement,
    Action::Collect,
    Action::Materialize,
    Action::Distribute,
    Action::Close,
];

/// One action-specific artifact bundle and canonical admission request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActionArtifactsV3<'a> {
    /// Action this array coordinate must implement.
    pub action: Action,
    /// One canonical request accepted by the action's exact RequestProfile.
    pub admission_request: &'a [u8],
    /// Complete finalized artifact bytes selected for this action.
    pub artifacts: GeneralArtifactBytesV3<'a>,
}

/// Complete exact seven-action release input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactReleaseBytesV3<'a> {
    /// ProgramSet selected by the capability release and GeneralConfigV3.
    pub program_set: &'a [u8],
    /// Immutable GeneralConfigV3 shared by every action descriptor.
    pub config: &'a [u8],
    /// Exact action bundles in [`GENERAL_ACTIONS_V3`] order.
    pub actions: [GeneralActionArtifactsV3<'a>; GENERAL_ACTION_PROGRAM_COUNT_V3],
}

/// Accepted release summary. The artifact bytes remain content-addressed by
/// these exact identities; this value grants no account or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactReleaseV3 {
    /// Complete ProgramSet identity.
    pub program_set: ContentId,
    /// Immutable GeneralConfigV3 identity.
    pub config: ContentId,
    /// Exact descriptor identities in canonical action order.
    pub descriptors: [ContentId; GENERAL_ACTION_PROGRAM_COUNT_V3],
    /// Product-authenticated runtime outcome width used during admission.
    pub tail_count: u32,
}

/// Stable refusal from complete General V3 release admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReleaseErrorV3 {
    /// ProgramSet identity, selector geometry, count, or order refused.
    ProgramSet,
    /// Two action coordinates selected one descriptor identity.
    DuplicateDescriptor,
    /// An action bundle did not use the release's exact shared bytes.
    SharedArtifactMismatch,
    /// An action coordinate or its canonical request selected another action.
    ActionMismatch,
    /// One complete action artifact join refused.
    Artifact(GeneralArtifactErrorV3),
}

/// Result alias for complete release admission.
pub type Result<T> = core::result::Result<T, GeneralReleaseErrorV3>;

/// Hostile-decode one exact seven-action ProgramSet.
pub fn authenticate_general_program_set_v3<'a>(
    selected_program_set: [u8; 32],
    authenticated_program_set: [u8; 32],
    bytes: &'a [u8],
) -> Result<CapabilityProgramSetV2<'a>> {
    let set = CapabilityProgramSetV2::decode_selected(
        selected_program_set,
        authenticated_program_set,
        bytes,
    )
    .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
    if set.selector_offset() != crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != GENERAL_ACTION_PROGRAM_COUNT_V3
    {
        return Err(GeneralReleaseErrorV3::ProgramSet);
    }
    let mut index = 0_usize;
    while index < GENERAL_ACTION_PROGRAM_COUNT_V3 {
        let entry = set
            .entry(u16::try_from(index).map_err(|_| GeneralReleaseErrorV3::ProgramSet)?)
            .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
        if entry.selector() != u32::from(GENERAL_ACTIONS_V3[index] as u8) {
            return Err(GeneralReleaseErrorV3::ProgramSet);
        }
        if entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID {
            return Err(GeneralReleaseErrorV3::ProgramSet);
        }
        let mut prior = 0_usize;
        while prior < index {
            if set
                .entry(u16::try_from(prior).map_err(|_| GeneralReleaseErrorV3::ProgramSet)?)
                .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?
                .descriptor()
                .program()
                == entry.descriptor().program()
            {
                return Err(GeneralReleaseErrorV3::DuplicateDescriptor);
            }
            prior = prior
                .checked_add(1)
                .ok_or(GeneralReleaseErrorV3::ProgramSet)?;
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralReleaseErrorV3::ProgramSet)?;
    }
    Ok(set)
}

/// Join every action-selected descriptor and finalized artifact in one pass.
///
/// The seven supplied requests are admission witnesses only. Live execution
/// re-runs the selected RequestProfile against the actual family request.
pub fn authenticate_general_release_v3(
    selection: GeneralArtifactSelectionV3,
    release: GeneralArtifactReleaseBytesV3<'_>,
    tail_count: u32,
) -> Result<GeneralArtifactReleaseV3> {
    let program_set_digest = digest(release.program_set);
    let config_digest = digest(release.config);
    let set = authenticate_general_program_set_v3(
        selection.program_set,
        program_set_digest,
        release.program_set,
    )?;
    if selection.config != config_digest {
        return Err(GeneralReleaseErrorV3::SharedArtifactMismatch);
    }
    let mut descriptors = [ContentId::new([1; 32])
        .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
        GENERAL_ACTION_PROGRAM_COUNT_V3];
    let mut index = 0_usize;
    while index < GENERAL_ACTION_PROGRAM_COUNT_V3 {
        let action_artifacts = release.actions[index];
        let expected_action = GENERAL_ACTIONS_V3[index];
        if action_artifacts.action != expected_action
            || action_artifacts.artifacts.program_set != release.program_set
            || action_artifacts.artifacts.config != release.config
        {
            return Err(GeneralReleaseErrorV3::SharedArtifactMismatch);
        }
        let bundle = authenticate_general_artifacts_v3(
            selection,
            action_artifacts.artifacts,
            action_artifacts.admission_request,
            tail_count,
        )
        .map_err(GeneralReleaseErrorV3::Artifact)?;
        if bundle.request.action != expected_action {
            return Err(GeneralReleaseErrorV3::ActionMismatch);
        }
        let descriptor = set
            .entry(u16::try_from(index).map_err(|_| GeneralReleaseErrorV3::ProgramSet)?)
            .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?
            .descriptor()
            .program();
        if descriptor != content(action_artifacts.artifacts.descriptor)? {
            return Err(GeneralReleaseErrorV3::ActionMismatch);
        }
        descriptors[index] = descriptor;
        index = index
            .checked_add(1)
            .ok_or(GeneralReleaseErrorV3::ProgramSet)?;
    }
    Ok(GeneralArtifactReleaseV3 {
        program_set: content(release.program_set)?,
        config: content(release.config)?,
        descriptors,
        tail_count,
    })
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn content(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(digest(bytes)).map_err(|_| GeneralReleaseErrorV3::ProgramSet)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;

    fn exact_set() -> Vec<u8> {
        use dclutch_capability_program_contract::set_v2::{
            CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, encode_program_set_v2,
            encoded_program_set_bytes_v2,
        };

        let entries = GENERAL_ACTIONS_V3.map(|action| {
            let byte = (action as u8).checked_add(1).expect("bounded action");
            CapabilityProgramSetEntryV2::new(
                u32::from(action as u8),
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID).expect("schema"),
                    ContentId::new([byte; 32]).expect("descriptor"),
                ),
            )
        });
        let mut output = vec![
            0_u8;
            encoded_program_set_bytes_v2(GENERAL_ACTION_PROGRAM_COUNT_V3)
                .expect("set width")
        ];
        encode_program_set_v2(
            crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
            SelectorWidthV2::U8,
            &entries,
            &mut output,
        )
        .expect("V2 set");
        output
    }

    #[test]
    fn exact_seven_action_set_accepts() {
        let bytes = exact_set();
        let identity = digest(&bytes);
        let set = authenticate_general_program_set_v3(identity, identity, &bytes)
            .expect("exact General set");
        assert_eq!(
            usize::from(set.entry_count()),
            GENERAL_ACTION_PROGRAM_COUNT_V3
        );
        for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
            let mut request = [0_u8; 64];
            request[10] = action as u8;
            assert_eq!(
                set.select_descriptor(&request)
                    .expect("selected action")
                    .program(),
                set.entry(u16::try_from(index).expect("seven"))
                    .expect("entry")
                    .descriptor()
                    .program()
            );
        }
    }

    #[test]
    fn missing_reordered_and_aliased_actions_refuse() {
        let canonical = exact_set();
        let identity = digest(&canonical);

        use dclutch_capability_program_contract::set_v2::{
            CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2, CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2,
            CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2,
            CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2, CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
        };

        let mut missing = canonical.clone();
        missing[CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2..CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2 + 2]
            .copy_from_slice(&6_u16.to_le_bytes());
        missing.truncate(
            CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + 6 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2,
        );
        assert_eq!(
            authenticate_general_program_set_v3(digest(&missing), digest(&missing), &missing),
            Err(GeneralReleaseErrorV3::ProgramSet)
        );

        let mut reordered = canonical.clone();
        let reordered_selector = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + 3 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2
            + CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2;
        reordered[reordered_selector..reordered_selector + 4]
            .copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(
            authenticate_general_program_set_v3(digest(&reordered), digest(&reordered), &reordered,),
            Err(GeneralReleaseErrorV3::ProgramSet)
        );

        let mut aliased = canonical;
        let first_start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2;
        let first = aliased[first_start..first_start + 32].to_vec();
        let last_start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + 6 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2
            + CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2;
        aliased[last_start..last_start + 32].copy_from_slice(&first);
        assert_eq!(
            authenticate_general_program_set_v3(digest(&aliased), digest(&aliased), &aliased),
            Err(GeneralReleaseErrorV3::DuplicateDescriptor)
        );

        assert_eq!(
            authenticate_general_program_set_v3(identity, [0x55; 32], &exact_set()),
            Err(GeneralReleaseErrorV3::ProgramSet)
        );
    }
}
