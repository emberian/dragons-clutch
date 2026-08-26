//! Schema-bound four-action program set for open Bearer and Structured routes.

use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v3::ProgramV3 as EffectProgramV3;
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, RepresentationActionV2,
};
use dclutch_rational_representation_v2_request_contract::generated::REQUEST_ACTION_OFFSET;
use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2};
use solana_program::hash::hash;

use crate::{
    Error, RationalOpenSelectedHotBundleV3, RationalOpenStructuredHotBundleV3, Result,
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3,
};

/// Four exact action descriptors sharing one admitted Token selection.
#[derive(Clone, Copy, Debug)]
pub struct RationalOpenCapabilityProgramSetInputV3<'a> {
    /// Finalized descriptor/Market/config Token behavior admission.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Exact Denominate artifact bundle.
    pub denominate: &'a RationalOpenSelectedHotBundleV3,
    /// Exact Reconstitute artifact bundle.
    pub reconstitute: &'a RationalOpenSelectedHotBundleV3,
    /// Exact IssueStructured artifact bundle.
    pub issue_structured: &'a RationalOpenStructuredHotBundleV3,
    /// Exact UnwrapStructured artifact bundle.
    pub unwrap_structured: &'a RationalOpenStructuredHotBundleV3,
}

/// Canonical config and schema-bound four-action CapabilityProgramSetV2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalOpenCapabilityProgramSetV3 {
    /// Exact Realm/release-selected Token behavior config bytes.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// SHA-256 content identity selected as Hot logical config coordinate 1.
    pub token_behavior_selection_id: [u8; 32],
    /// Exact schema-bound CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity selected as the Market capability release.
    pub program_set_id: [u8; 32],
}

/// Build the canonical schema-bound four-action set.
pub fn build_rational_open_capability_program_set_v3(
    input: RationalOpenCapabilityProgramSetInputV3<'_>,
) -> Result<RationalOpenCapabilityProgramSetV3> {
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        input.denominate,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        input.reconstitute,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        input.issue_structured,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        input.unwrap_structured,
        input.authenticated_token_behavior,
    )?;

    let selection = input.authenticated_token_behavior.selection().to_bytes();
    if hash(&selection).to_bytes() != input.authenticated_token_behavior.content_digest() {
        return Err(Error::ContentIdentity);
    }
    for observed in [
        input.denominate.token_behavior_selection.as_slice(),
        input.reconstitute.token_behavior_selection.as_slice(),
        input.issue_structured.token_behavior_selection.as_slice(),
        input.unwrap_structured.token_behavior_selection.as_slice(),
    ] {
        if observed != selection {
            return Err(Error::ArtifactGeometry);
        }
    }

    let descriptors = [
        descriptor_entry(input.denominate, RepresentationActionV2::Denominate)?,
        descriptor_entry(input.reconstitute, RepresentationActionV2::Reconstitute)?,
        structured_descriptor_entry(
            input.issue_structured,
            RepresentationActionV2::IssueStructured,
        )?,
        structured_descriptor_entry(
            input.unwrap_structured,
            RepresentationActionV2::UnwrapStructured,
        )?,
    ];
    let width =
        encoded_program_set_bytes_v2(descriptors.len()).map_err(Error::CapabilityProgramSet)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(REQUEST_ACTION_OFFSET).map_err(|_| Error::ArtifactGeometry)?,
        SelectorWidthV2::U8,
        &descriptors,
        &mut program_set,
    )
    .map_err(Error::CapabilityProgramSet)?;
    let output = RationalOpenCapabilityProgramSetV3 {
        token_behavior_selection: selection,
        token_behavior_selection_id: hash(&selection).to_bytes(),
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
    };
    validate_rational_open_capability_program_set_v3(&output, input)?;
    Ok(output)
}

/// Hostile-decode and bind one built set to independently authenticated
/// Realm/release authority and exact descriptor coordinates.
pub fn validate_rational_open_capability_program_set_v3(
    value: &RationalOpenCapabilityProgramSetV3,
    input: RationalOpenCapabilityProgramSetInputV3<'_>,
) -> Result<()> {
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        input.denominate,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        input.reconstitute,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        input.issue_structured,
        input.authenticated_token_behavior,
    )?;
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        input.unwrap_structured,
        input.authenticated_token_behavior,
    )?;
    if value.token_behavior_selection != input.authenticated_token_behavior.selection().to_bytes()
        || value.token_behavior_selection_id != input.authenticated_token_behavior.content_digest()
        || value.token_behavior_selection_id != hash(&value.token_behavior_selection).to_bytes()
        || value.program_set_id != hash(&value.program_set).to_bytes()
    {
        return Err(Error::ContentIdentity);
    }
    let set = CapabilityProgramSetV2::decode_selected(
        value.program_set_id,
        hash(&value.program_set).to_bytes(),
        &value.program_set,
    )
    .map_err(Error::CapabilityProgramSet)?;
    let expected_entries = [
        descriptor_entry(input.denominate, RepresentationActionV2::Denominate)?,
        descriptor_entry(input.reconstitute, RepresentationActionV2::Reconstitute)?,
        structured_descriptor_entry(
            input.issue_structured,
            RepresentationActionV2::IssueStructured,
        )?,
        structured_descriptor_entry(
            input.unwrap_structured,
            RepresentationActionV2::UnwrapStructured,
        )?,
    ];
    if usize::from(set.entry_count()) != expected_entries.len() {
        return Err(Error::ArtifactGeometry);
    }
    for (ordinal, expected) in expected_entries.iter().copied().enumerate() {
        let actual = set
            .entry(u16::try_from(ordinal).map_err(|_| Error::ArtifactGeometry)?)
            .map_err(Error::CapabilityProgramSet)?;
        if actual != expected {
            return Err(Error::ArtifactGeometry);
        }
        let mut request = [0_u8; REQUEST_ACTION_OFFSET + 1];
        request[REQUEST_ACTION_OFFSET] =
            u8::try_from(expected.selector()).map_err(|_| Error::ArtifactGeometry)?;
        set.require_descriptor(
            &request,
            expected.descriptor().schema(),
            expected.descriptor().program(),
        )
        .map_err(Error::CapabilityProgramSet)?;
    }
    Ok(())
}

fn descriptor_entry(
    bundle: &RationalOpenSelectedHotBundleV3,
    expected_action: RepresentationActionV2,
) -> Result<CapabilityProgramSetEntryV2> {
    descriptor_entry_inner(&bundle.descriptor, &bundle.effect, expected_action)
}

fn structured_descriptor_entry(
    bundle: &RationalOpenStructuredHotBundleV3,
    expected_action: RepresentationActionV2,
) -> Result<CapabilityProgramSetEntryV2> {
    descriptor_entry_inner(&bundle.descriptor, &bundle.effect, expected_action)
}

fn descriptor_entry_inner(
    descriptor_bytes: &[u8],
    effect_bytes: &[u8],
    expected_action: RepresentationActionV2,
) -> Result<CapabilityProgramSetEntryV2> {
    let descriptor =
        CapabilityProgramV4::decode(descriptor_bytes).map_err(Error::CapabilityDescriptor)?;
    if descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2 {
        return Err(Error::ArtifactGeometry);
    }
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect().program().to_bytes(),
        hash(effect_bytes).to_bytes(),
        effect_bytes,
    )
    .map_err(Error::EffectArtifact)?;
    let (fixed, _) = effect.route_template(0).map_err(Error::EffectArtifact)?;
    if fixed.get(REQUEST_ACTION_OFFSET).copied() != Some(expected_action as u8) {
        return Err(Error::ArtifactGeometry);
    }
    Ok(CapabilityProgramSetEntryV2::new(
        expected_action as u32,
        CapabilityDescriptorReferenceV2::new(
            content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?,
            content(hash(descriptor_bytes).to_bytes())?,
        ),
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::ContentIdentity)
}

pub(crate) fn require_open_program_selection_v3(
    value: &RationalOpenCapabilityProgramSetV3,
    authenticated: AuthenticatedTokenBehaviorV2,
    family_request: &[u8],
    descriptor_bytes: &[u8],
) -> Result<()> {
    if value.token_behavior_selection != authenticated.selection().to_bytes()
        || value.token_behavior_selection_id != authenticated.content_digest()
        || value.token_behavior_selection_id != hash(&value.token_behavior_selection).to_bytes()
        || value.program_set_id != hash(&value.program_set).to_bytes()
    {
        return Err(Error::ContentIdentity);
    }
    let descriptor =
        CapabilityProgramV4::decode(descriptor_bytes).map_err(Error::CapabilityDescriptor)?;
    if descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2 {
        return Err(Error::ArtifactGeometry);
    }
    let set = CapabilityProgramSetV2::decode_selected(
        value.program_set_id,
        hash(&value.program_set).to_bytes(),
        &value.program_set,
    )
    .map_err(Error::CapabilityProgramSet)?;
    set.require_descriptor(
        family_request,
        content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?,
        content(hash(descriptor_bytes).to_bytes())?,
    )
    .map_err(Error::CapabilityProgramSet)?;
    Ok(())
}
