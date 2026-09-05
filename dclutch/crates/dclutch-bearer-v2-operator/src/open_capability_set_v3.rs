//! Schema-bound five-action program set for Bearer and Structured routes.

use dclutch_market::capability_program::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_core_contract::ContentId;
use dclutch_vm::effect::v4::{
    ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4,
};
use dclutch_claims::rational::{
    AuthenticatedTokenBehaviorV2, RepresentationActionV2,
};
use dclutch_claims::rational_request::generated::REQUEST_ACTION_OFFSET_V3;
use dclutch_custody::token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use solana_program::hash::hash;

use crate::{
    Error, RationalOpenSelectedHotBundleV3, RationalOpenStructuredHotBundleV3,
    RationalTerminalHotBundleV3, Result,
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_selected_hot_bundle_v3,
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_structured_hot_bundle_v3,
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3,
    validate_rational_terminal_hot_bundle_v3,
};

/// Five exact action descriptors sharing one admitted Token selection.
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
    /// Exact RedeemTerminal artifact bundle.
    pub redeem_terminal: &'a RationalTerminalHotBundleV3,
}

/// Canonical config and schema-bound five-action CapabilityProgramSetV2.
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

/// Build the canonical schema-bound five-action set.
/// The same five bundles, joined with no Market in scope.
///
/// The V3 form takes an `AuthenticatedTokenBehaviorV2`, which can only be built
/// from a finalized `RepresentationDescriptorV2` and therefore not before the
/// Market exists. This form takes the immutable `TokenBehaviorSelectionV2` the
/// five bundles were compiled against, which is the only thing the join ever
/// needed: every check the V3 path runs against the admission is a check that
/// each bundle's config equals that selection.
#[derive(Clone, Copy, Debug)]
pub struct RationalOpenCapabilityProgramSetInputV6<'a> {
    /// Immutable Realm/release selection every bundle was compiled against.
    pub token_behavior_selection: TokenBehaviorSelectionV2,
    /// Exact Denominate artifact bundle.
    pub denominate: &'a RationalOpenSelectedHotBundleV3,
    /// Exact Reconstitute artifact bundle.
    pub reconstitute: &'a RationalOpenSelectedHotBundleV3,
    /// Exact IssueStructured artifact bundle.
    pub issue_structured: &'a RationalOpenStructuredHotBundleV3,
    /// Exact UnwrapStructured artifact bundle.
    pub unwrap_structured: &'a RationalOpenStructuredHotBundleV3,
    /// Exact RedeemTerminal artifact bundle.
    pub redeem_terminal: &'a RationalTerminalHotBundleV3,
}

/// Build the canonical five-action set before the Market that selects it.
///
/// This is the function that makes an open capability release compilable
/// pre-founding: its output `program_set_id` is the `release_id` a Market's
/// capability manifest entry names, and nothing reachable from this input can
/// observe a Market.
pub fn build_rational_open_capability_program_set_v6(
    input: RationalOpenCapabilityProgramSetInputV6<'_>,
) -> Result<RationalOpenCapabilityProgramSetV3> {
    let selection = input.token_behavior_selection.to_bytes();
    validate_rational_open_selected_hot_bundle_v3(input.denominate)?;
    validate_rational_open_selected_hot_bundle_v3(input.reconstitute)?;
    validate_rational_open_structured_hot_bundle_v3(input.issue_structured)?;
    validate_rational_open_structured_hot_bundle_v3(input.unwrap_structured)?;
    validate_rational_terminal_hot_bundle_v3(input.redeem_terminal)?;
    for observed in [
        input.denominate.token_behavior_selection.as_slice(),
        input.reconstitute.token_behavior_selection.as_slice(),
        input.issue_structured.token_behavior_selection.as_slice(),
        input.unwrap_structured.token_behavior_selection.as_slice(),
        input.redeem_terminal.token_behavior_selection.as_slice(),
    ] {
        if observed != selection {
            return Err(Error::ArtifactGeometry);
        }
    }
    let output = assemble_open_capability_program_set(
        selection,
        input.denominate,
        input.reconstitute,
        input.issue_structured,
        input.unwrap_structured,
        input.redeem_terminal,
    )?;
    validate_open_capability_program_set_core(
        &output,
        input.denominate,
        input.reconstitute,
        input.issue_structured,
        input.unwrap_structured,
        input.redeem_terminal,
    )?;
    Ok(output)
}

/// Build the canonical five-action set from a Token-behavior admission.
///
/// Identical bytes to [`build_rational_open_capability_program_set_v6`]: both
/// route through one assembler. What this entry point adds is the admission
/// join -- each bundle is required to match a finalized descriptor's Token
/// behavior, not merely to agree with the others.
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
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3(
        input.redeem_terminal,
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
        input.redeem_terminal.token_behavior_selection.as_slice(),
    ] {
        if observed != selection {
            return Err(Error::ArtifactGeometry);
        }
    }

    let output = assemble_open_capability_program_set(
        selection,
        input.denominate,
        input.reconstitute,
        input.issue_structured,
        input.unwrap_structured,
        input.redeem_terminal,
    )?;
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
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3(
        input.redeem_terminal,
        input.authenticated_token_behavior,
    )?;
    if value.token_behavior_selection != input.authenticated_token_behavior.selection().to_bytes()
        || value.token_behavior_selection_id != input.authenticated_token_behavior.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    validate_open_capability_program_set_core(
        value,
        input.denominate,
        input.reconstitute,
        input.issue_structured,
        input.unwrap_structured,
        input.redeem_terminal,
    )
}

/// Every set check that does not need a Token-behavior ADMISSION.
///
/// Both entry points join through this, so the pre-founding path and the
/// descriptor-bound path cannot drift into two readings of one set.
fn validate_open_capability_program_set_core(
    value: &RationalOpenCapabilityProgramSetV3,
    denominate: &RationalOpenSelectedHotBundleV3,
    reconstitute: &RationalOpenSelectedHotBundleV3,
    issue_structured: &RationalOpenStructuredHotBundleV3,
    unwrap_structured: &RationalOpenStructuredHotBundleV3,
    redeem_terminal: &RationalTerminalHotBundleV3,
) -> Result<()> {
    if value.token_behavior_selection_id != hash(&value.token_behavior_selection).to_bytes()
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
        descriptor_entry(denominate, RepresentationActionV2::Denominate)?,
        descriptor_entry(reconstitute, RepresentationActionV2::Reconstitute)?,
        structured_descriptor_entry(issue_structured, RepresentationActionV2::IssueStructured)?,
        structured_descriptor_entry(unwrap_structured, RepresentationActionV2::UnwrapStructured)?,
        terminal_descriptor_entry(redeem_terminal)?,
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
        let mut request = [0_u8; REQUEST_ACTION_OFFSET_V3 + 1];
        request[REQUEST_ACTION_OFFSET_V3] =
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

/// Encode the five-entry set from bundles whose config already agrees.
///
/// Sole author of the set bytes: both entry points route here, so the
/// pre-founding path and the descriptor-bound path cannot emit two different
/// encodings of one capability.
fn assemble_open_capability_program_set(
    selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    denominate: &RationalOpenSelectedHotBundleV3,
    reconstitute: &RationalOpenSelectedHotBundleV3,
    issue_structured: &RationalOpenStructuredHotBundleV3,
    unwrap_structured: &RationalOpenStructuredHotBundleV3,
    redeem_terminal: &RationalTerminalHotBundleV3,
) -> Result<RationalOpenCapabilityProgramSetV3> {
    let descriptors = [
        descriptor_entry(denominate, RepresentationActionV2::Denominate)?,
        descriptor_entry(reconstitute, RepresentationActionV2::Reconstitute)?,
        structured_descriptor_entry(issue_structured, RepresentationActionV2::IssueStructured)?,
        structured_descriptor_entry(unwrap_structured, RepresentationActionV2::UnwrapStructured)?,
        terminal_descriptor_entry(redeem_terminal)?,
    ];
    let width =
        encoded_program_set_bytes_v2(descriptors.len()).map_err(Error::CapabilityProgramSet)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(REQUEST_ACTION_OFFSET_V3).map_err(|_| Error::ArtifactGeometry)?,
        SelectorWidthV2::U8,
        &descriptors,
        &mut program_set,
    )
    .map_err(Error::CapabilityProgramSet)?;
    Ok(RationalOpenCapabilityProgramSetV3 {
        token_behavior_selection: selection,
        token_behavior_selection_id: hash(&selection).to_bytes(),
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
    })
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

fn terminal_descriptor_entry(
    bundle: &RationalTerminalHotBundleV3,
) -> Result<CapabilityProgramSetEntryV2> {
    descriptor_entry_inner(
        &bundle.descriptor,
        &bundle.effect,
        RepresentationActionV2::RedeemTerminal,
    )
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
    if descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4
        || descriptor.effect().program().to_bytes() != hash(effect_bytes).to_bytes()
    {
        return Err(Error::ArtifactGeometry);
    }
    let effect = EffectProgramV4::decode(effect_bytes).map_err(Error::EffectArtifactV4)?;
    let (fixed, _) = effect
        .base()
        .route_template(0)
        .map_err(Error::EffectArtifact)?;
    if fixed.get(REQUEST_ACTION_OFFSET_V3).copied() != Some(expected_action as u8) {
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
