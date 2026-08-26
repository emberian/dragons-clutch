//! Exact seven-action General V3 release admission.
//!
//! A General release is not one permissive descriptor with an internal action
//! switch.  Its authenticated [`CapabilityProgramSetV1`] contains exactly one
//! complete `CapabilityProgramV3` identity for each canonical General action.
//! This module validates that closed table without allocating and then joins
//! every selected descriptor to its complete finalized artifact bundle.

use dclutch_capability_program_contract::set_v1::{CapabilityProgramSetV1, SelectorWidthV1};
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
) -> Result<CapabilityProgramSetV1<'a>> {
    let set = CapabilityProgramSetV1::decode_selected(
        selected_program_set,
        authenticated_program_set,
        bytes,
    )
    .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
    if set.selector_offset() != crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U8
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
        let mut prior = 0_usize;
        while prior < index {
            if set
                .entry(u16::try_from(prior).map_err(|_| GeneralReleaseErrorV3::ProgramSet)?)
                .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?
                .program()
                == entry.program()
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

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture range")
            .copy_from_slice(value);
    }

    fn exact_set() -> Vec<u8> {
        const HEADER: usize = 32;
        const ENTRY: usize = 40;
        let mut output = vec![0_u8; HEADER + GENERAL_ACTION_PROGRAM_COUNT_V3 * ENTRY];
        put(&mut output, 0, b"DCLTCPS1");
        put(&mut output, 8, &1_u16.to_le_bytes());
        put(&mut output, 10, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        output[16] = 1;
        put(
            &mut output,
            18,
            &u16::try_from(GENERAL_ACTION_PROGRAM_COUNT_V3)
                .expect("seven")
                .to_le_bytes(),
        );
        for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
            let start = HEADER + index * ENTRY;
            put(&mut output, start, &u32::from(action as u8).to_le_bytes());
            let byte = u8::try_from(index + 1).expect("seven descriptors");
            put(&mut output, start + 4, &[byte; 32]);
        }
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
                set.select(&request).expect("selected action"),
                set.entry(u16::try_from(index).expect("seven"))
                    .expect("entry")
                    .program()
            );
        }
    }

    #[test]
    fn missing_reordered_and_aliased_actions_refuse() {
        let canonical = exact_set();
        let identity = digest(&canonical);

        let mut missing = canonical.clone();
        put(&mut missing, 18, &6_u16.to_le_bytes());
        missing.truncate(32 + 6 * 40);
        assert_eq!(
            authenticate_general_program_set_v3(digest(&missing), digest(&missing), &missing),
            Err(GeneralReleaseErrorV3::ProgramSet)
        );

        let mut reordered = canonical.clone();
        put(&mut reordered, 32 + 3 * 40, &9_u32.to_le_bytes());
        assert_eq!(
            authenticate_general_program_set_v3(digest(&reordered), digest(&reordered), &reordered,),
            Err(GeneralReleaseErrorV3::ProgramSet)
        );

        let mut aliased = canonical;
        let first = aliased[36..68].to_vec();
        put(&mut aliased, 32 + 6 * 40 + 4, &first);
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
