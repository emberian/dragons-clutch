//! Exact General V3 release admission over a CLOSED, PROFILED program set.
//!
//! A General release is not one permissive descriptor with an internal action
//! switch. Its authenticated [`CapabilityProgramSetV2`] contains exactly one
//! schema-bound identity per coordinate, and this module validates that table
//! without allocating before joining every selected descriptor to its complete
//! finalized artifact bundle.
//!
//! # The exactly-seven relaxation
//!
//! Decision 0009 §3 and ADR-0006 §8 item 7 both need this table to grow: the
//! collection and candidate actions add coordinates, and General's activation
//! descriptor needs one of its own -- and it is a `CapabilityProgramV1`, not a
//! `CapabilityProgramV4`, so it is not merely one more of the same.
//!
//! **The rule relaxes without opening.** It would have been a single edit to
//! turn `entry_count == 7` into `entry_count >= 7`, and that would have been
//! the wrong shape: an open table admits a descriptor nobody enumerated, and
//! the whole argument for a program SET rather than a permissive descriptor is
//! that the reachable programs are named in advance. Instead the entry count
//! selects one of four NAMED PROFILES, each an exact table with a role per
//! coordinate. A release is still a closed enumeration; what changed is that
//! there are now four legal enumerations instead of one, and adding a fifth is
//! a visible edit here rather than a silent consequence of publishing a longer
//! set.
//!
//! | profile | entries | what it is |
//! |---|---|---|
//! | [`GeneralReleaseProfileV1::SettlementOnly`] | 7 | the seven settlement actions; what shipped before this |
//! | [`GeneralReleaseProfileV1::SettlementWithActivation`] | 8 | the same, plus the activation descriptor -- ADR-0006 §8 item 7 |
//! | [`GeneralReleaseProfileV1::Complete`] | 14 | every action, once the collection and candidate artifacts exist |
//! | [`GeneralReleaseProfileV1::CompleteWithActivation`] | 15 | both |

use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
};
use dclutch_core_contract::ContentId;
use dclutch_general_codec::Action;
use dclutch_sha256_adapter::digest;

use crate::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactErrorV3, GeneralArtifactSelectionV3,
    authenticate_general_artifacts_v3,
};

/// Number of exact action-selected programs in the settlement half.
pub const GENERAL_ACTION_PROGRAM_COUNT_V3: usize = 7;

/// Number of exact action-selected programs in a complete General release.
pub const GENERAL_ACTION_PROGRAM_COUNT_V4: usize = 14;

/// Canonical strictly increasing General action order for the settlement half.
///
/// This is a PREFIX of [`GENERAL_ACTIONS_V4`], not a separate table: the tags
/// are dense and ascending, so the settlement profile is exactly the first
/// seven coordinates of the complete one. A profile that reordered or renumbered
/// would put two meanings behind one request byte.
pub const GENERAL_ACTIONS_V3: [Action; GENERAL_ACTION_PROGRAM_COUNT_V3] = [
    Action::Consider,
    Action::Freeze,
    Action::InitializeSettlement,
    Action::Collect,
    Action::Materialize,
    Action::Distribute,
    Action::Close,
];

/// Canonical strictly increasing order of every General action.
pub const GENERAL_ACTIONS_V4: [Action; GENERAL_ACTION_PROGRAM_COUNT_V4] = [
    Action::Consider,
    Action::Freeze,
    Action::InitializeSettlement,
    Action::Collect,
    Action::Materialize,
    Action::Distribute,
    Action::Close,
    Action::OpenBatch,
    Action::PlaceOrder,
    Action::CancelOrder,
    Action::CloseBatch,
    Action::SubmitCandidate,
    Action::VerifyCandidateRow,
    Action::ReleaseOrder,
];

/// Selector reserved for the activation descriptor's set coordinate.
///
/// It is deliberately outside the action tag space and outside what any
/// controller request can carry: `Action::decode` refuses 255, so no Hot
/// execution can ever select this entry. The entry exists to be NAMED -- so the
/// capability seal and the ProgramSet identity cover the activation descriptor
/// -- not to be dispatched to.
pub const GENERAL_ACTIVATION_SELECTOR_V4: u32 = 255;

/// One legal shape of a General `CapabilityProgramSetV2`.
///
/// The entry count selects the profile and the profile fixes the table. There
/// is no "at least" anywhere in this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReleaseProfileV1 {
    /// The seven settlement actions, and nothing else.
    SettlementOnly,
    /// The seven settlement actions plus the activation descriptor.
    SettlementWithActivation,
    /// Every action, settlement and collection and candidate.
    Complete,
    /// Every action plus the activation descriptor.
    CompleteWithActivation,
}

impl GeneralReleaseProfileV1 {
    /// Select the sole profile with this exact entry count.
    pub const fn from_entry_count(count: usize) -> Result<Self> {
        match count {
            GENERAL_ACTION_PROGRAM_COUNT_V3 => Ok(Self::SettlementOnly),
            8 => Ok(Self::SettlementWithActivation),
            GENERAL_ACTION_PROGRAM_COUNT_V4 => Ok(Self::Complete),
            15 => Ok(Self::CompleteWithActivation),
            _ => Err(GeneralReleaseErrorV3::ProgramSet),
        }
    }

    /// Number of action-selected coordinates this profile declares.
    #[must_use]
    pub const fn action_count(self) -> usize {
        match self {
            Self::SettlementOnly | Self::SettlementWithActivation => {
                GENERAL_ACTION_PROGRAM_COUNT_V3
            }
            Self::Complete | Self::CompleteWithActivation => GENERAL_ACTION_PROGRAM_COUNT_V4,
        }
    }

    /// Whether this profile names the activation descriptor.
    #[must_use]
    pub const fn has_activation_entry(self) -> bool {
        matches!(
            self,
            Self::SettlementWithActivation | Self::CompleteWithActivation
        )
    }

    /// Exact total entry count.
    #[must_use]
    pub const fn entry_count(self) -> usize {
        self.action_count() + if self.has_activation_entry() { 1 } else { 0 }
    }
}

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
    /// The set's profile declares coordinates this admission cannot join.
    UnjoinedProfile,
    /// One complete action artifact join refused.
    Artifact(GeneralArtifactErrorV3),
}

/// Result alias for complete release admission.
pub type Result<T> = core::result::Result<T, GeneralReleaseErrorV3>;

/// Hostile-decode one exact profiled General ProgramSet.
///
/// Returns the set together with the profile its entry count selected, so a
/// caller that can only join some profiles says which rather than assuming.
pub fn authenticate_general_program_set_v3<'a>(
    selected_program_set: [u8; 32],
    authenticated_program_set: [u8; 32],
    bytes: &'a [u8],
) -> Result<(CapabilityProgramSetV2<'a>, GeneralReleaseProfileV1)> {
    let set = CapabilityProgramSetV2::decode_selected(
        selected_program_set,
        authenticated_program_set,
        bytes,
    )
    .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
    let profile = GeneralReleaseProfileV1::from_entry_count(usize::from(set.entry_count()))?;
    if set.selector_offset() != crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
    {
        return Err(GeneralReleaseErrorV3::ProgramSet);
    }
    let action_count = profile.action_count();
    let mut index = 0_usize;
    while index < profile.entry_count() {
        let entry = set
            .entry(u16::try_from(index).map_err(|_| GeneralReleaseErrorV3::ProgramSet)?)
            .map_err(|_| GeneralReleaseErrorV3::ProgramSet)?;
        if index < action_count {
            if entry.selector() != u32::from(GENERAL_ACTIONS_V4[index] as u8) {
                return Err(GeneralReleaseErrorV3::ProgramSet);
            }
            if entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID {
                return Err(GeneralReleaseErrorV3::ProgramSet);
            }
        } else {
            // The activation coordinate, and it is the reason this is a profile
            // table rather than a longer action table: it carries a schema no
            // action carries and a selector no request can produce.
            if entry.selector() != GENERAL_ACTIVATION_SELECTOR_V4 {
                return Err(GeneralReleaseErrorV3::ProgramSet);
            }
            if entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
                return Err(GeneralReleaseErrorV3::ProgramSet);
            }
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
    Ok((set, profile))
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
    let (set, profile) = authenticate_general_program_set_v3(
        selection.program_set,
        program_set_digest,
        release.program_set,
    )?;
    // Hot release admission joins the SEVEN settlement bundles and nothing
    // else. The wider profiles are legal sets, and they are not yet admissible
    // releases: the collection and candidate actions have no authored artifact
    // triple, so there is nothing for this function to join at those
    // coordinates, and admitting them unvalidated would be worse than refusing.
    // The activation coordinate is validated at the set level and joined by the
    // activation route, which owns a different schema.
    if profile.action_count() != GENERAL_ACTION_PROGRAM_COUNT_V3 {
        return Err(GeneralReleaseErrorV3::UnjoinedProfile);
    }
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

fn content(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(digest(bytes)).map_err(|_| GeneralReleaseErrorV3::ProgramSet)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;

    /// Build one canonical set for the given profile.
    fn profiled_set(profile: GeneralReleaseProfileV1) -> Vec<u8> {
        use dclutch_capability_program_contract::set_v2::{
            CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, encode_program_set_v2,
            encoded_program_set_bytes_v2,
        };

        let mut entries = Vec::new();
        for action in GENERAL_ACTIONS_V4.into_iter().take(profile.action_count()) {
            let byte = (action as u8).checked_add(1).expect("bounded action");
            entries.push(CapabilityProgramSetEntryV2::new(
                u32::from(action as u8),
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID).expect("schema"),
                    ContentId::new([byte; 32]).expect("descriptor"),
                ),
            ));
        }
        if profile.has_activation_entry() {
            entries.push(CapabilityProgramSetEntryV2::new(
                GENERAL_ACTIVATION_SELECTOR_V4,
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1).expect("schema"),
                    ContentId::new([0xa0; 32]).expect("activation descriptor"),
                ),
            ));
        }
        let mut output =
            vec![0_u8; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
        encode_program_set_v2(
            crate::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
            SelectorWidthV2::U8,
            &entries,
            &mut output,
        )
        .expect("V2 set");
        output
    }

    fn exact_set() -> Vec<u8> {
        profiled_set(GeneralReleaseProfileV1::SettlementOnly)
    }

    #[test]
    fn exact_seven_action_set_accepts() {
        let bytes = exact_set();
        let identity = digest(&bytes);
        let (set, profile) = authenticate_general_program_set_v3(identity, identity, &bytes)
            .expect("exact General set");
        assert_eq!(profile, GeneralReleaseProfileV1::SettlementOnly);
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
            CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2,
            CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
        };

        let mut missing = canonical.clone();
        missing[CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2
            ..CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2 + 2]
            .copy_from_slice(&6_u16.to_le_bytes());
        missing.truncate(
            CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + 6 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2,
        );
        assert_eq!(
            authenticate_general_program_set_v3(digest(&missing), digest(&missing), &missing).err(),
            Some(GeneralReleaseErrorV3::ProgramSet)
        );

        let mut reordered = canonical.clone();
        let reordered_selector = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + 3 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2
            + CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2;
        reordered[reordered_selector..reordered_selector + 4].copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(
            authenticate_general_program_set_v3(digest(&reordered), digest(&reordered), &reordered)
                .err(),
            Some(GeneralReleaseErrorV3::ProgramSet)
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
            authenticate_general_program_set_v3(digest(&aliased), digest(&aliased), &aliased).err(),
            Some(GeneralReleaseErrorV3::DuplicateDescriptor)
        );

        assert_eq!(
            authenticate_general_program_set_v3(identity, [0x55; 32], &exact_set()).err(),
            Some(GeneralReleaseErrorV3::ProgramSet)
        );
    }

    #[test]
    fn every_named_profile_is_accepted_and_nothing_between_them_is() {
        for profile in [
            GeneralReleaseProfileV1::SettlementOnly,
            GeneralReleaseProfileV1::SettlementWithActivation,
            GeneralReleaseProfileV1::Complete,
            GeneralReleaseProfileV1::CompleteWithActivation,
        ] {
            let bytes = profiled_set(profile);
            let identity = digest(&bytes);
            let accepted = authenticate_general_program_set_v3(identity, identity, &bytes);
            assert!(accepted.is_ok(), "profile {profile:?} must accept");
            let (set, selected) = accepted.expect("checked above");
            assert_eq!(selected, profile);
            assert_eq!(usize::from(set.entry_count()), profile.entry_count());
        }

        // The relaxation did NOT become "at least seven". Every entry count
        // that is not one of the four named profiles is refused, including the
        // counts that sit between them -- which is exactly what a `>=` rule
        // would have admitted, each one a table with an unenumerated
        // coordinate.
        for count in [0_usize, 1, 6, 9, 10, 13, 16, 20] {
            assert_eq!(
                GeneralReleaseProfileV1::from_entry_count(count).err(),
                Some(GeneralReleaseErrorV3::ProgramSet),
                "entry count {count} must name no profile"
            );
        }
    }

    #[test]
    fn hostile_the_activation_coordinate_admits_no_action_and_no_action_schema() {
        use dclutch_capability_program_contract::set_v2::{
            CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2,
            CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2,
            CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2,
            CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
        };

        let canonical = profiled_set(GeneralReleaseProfileV1::SettlementWithActivation);
        let activation_start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + GENERAL_ACTION_PROGRAM_COUNT_V3 * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;

        // The activation entry carrying an ACTION's schema: this is the exact
        // confusion the profile table exists to refuse, because a
        // `CapabilityProgramV4` at that coordinate would be a descriptor the
        // Hot executor could be persuaded to treat as a program.
        let mut action_schema = canonical.clone();
        let schema_at = activation_start + CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2;
        action_schema[schema_at..schema_at + 32]
            .copy_from_slice(&CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID);
        assert_eq!(
            authenticate_general_program_set_v3(
                digest(&action_schema),
                digest(&action_schema),
                &action_schema,
            )
            .err(),
            Some(GeneralReleaseErrorV3::ProgramSet)
        );

        // The activation entry claiming an action's selector, which would put
        // two descriptors behind one request byte.
        let mut action_selector = canonical;
        let selector_at = activation_start + CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2;
        action_selector[selector_at..selector_at + 4]
            .copy_from_slice(&u32::from(Action::Close as u8).to_le_bytes());
        assert_eq!(
            authenticate_general_program_set_v3(
                digest(&action_selector),
                digest(&action_selector),
                &action_selector,
            )
            .err(),
            Some(GeneralReleaseErrorV3::ProgramSet)
        );
    }

    #[test]
    fn no_controller_request_can_select_the_activation_coordinate() {
        // `Action::decode` refuses 255, so the selector the activation entry
        // carries cannot appear in a canonical controller request at all. The
        // entry is named by the set -- and therefore covered by the capability
        // seal and the ProgramSet identity -- without ever being dispatchable.
        assert!(u32::from(u8::MAX) == GENERAL_ACTIVATION_SELECTOR_V4);
        assert!(dclutch_general_codec::ControllerRequestV1::decode(&[0xff; 64]).is_err());
        for action in GENERAL_ACTIONS_V4 {
            assert_ne!(u32::from(action as u8), GENERAL_ACTIVATION_SELECTOR_V4);
        }
    }

    #[test]
    fn a_wider_profile_is_a_legal_set_and_not_yet_an_admissible_release() {
        // The relaxation lands ahead of the artifacts, deliberately: GEN-HOT's
        // bundle and ADR-0006 §8 item 7's activation entry both need the RULE,
        // and the collection and candidate actions need artifact triples nobody
        // has authored. A release naming those coordinates is refused with a
        // name that says which of the two is missing.
        let bytes = profiled_set(GeneralReleaseProfileV1::Complete);
        let identity = digest(&bytes);
        let (_, profile) = authenticate_general_program_set_v3(identity, identity, &bytes)
            .expect("a complete set is a legal set");
        assert_eq!(profile, GeneralReleaseProfileV1::Complete);
        assert!(profile.action_count() > GENERAL_ACTION_PROGRAM_COUNT_V3);
    }
}
