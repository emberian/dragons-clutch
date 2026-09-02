//! Admission for one complete five-action open-capability release.
//!
//! # What this is, and what the layer had instead
//!
//! The open capability layer had no release authenticator. The closest thing,
//! [`crate::validate_rational_open_capability_program_set_v3`], is a ROUND TRIP:
//! it rebuilds the entry table from bundles the caller already holds and
//! requires agreement. That proves an author did not fumble their own build. It
//! proves nothing about bytes that arrived from somewhere else, because it needs
//! the typed inputs to run at all -- and one of those inputs, an
//! `AuthenticatedTokenBehaviorV2`, cannot even be constructed before the Market
//! exists.
//!
//! This is the other kind. It takes three content identities and a pile of
//! BYTES, and decides whether those bytes are one coherent open-capability
//! release. Every joined fact is re-derived here; nothing is taken on the
//! caller's word. It is the shape `authenticate_rational_release_v1` has,
//! supplied for the layer that lacked it.
//!
//! # The capability kind, which nothing checked
//!
//! [`crate::validate_rational_open_capability_program_set_v3`] does not read the
//! descriptor's `kind` at all. That is how the only test in the tree building a
//! Structured `CapabilityProgramV4` passed a placeholder `identity(0x10)` for
//! months while two Lean-generated kind constants sat unread beside it.
//!
//! The Rational lifecycle admission closes the same hole by HARDCODING its
//! family constant. This layer cannot: one set builder serves Bearer and
//! Structured, so a hardcode would be wrong for one of them or would invent a
//! second encoder. So the kind travels in [`OpenCapabilityArtifactSelectionV1`]
//! beside the release and config identities -- which is the honest place for it,
//! because those are exactly the three identities a Market's capability manifest
//! entry names. Authenticating against this structure is authenticating against
//! what a founded Market bound, and the kind is a fact the MANIFEST states and
//! this admission refuses to contradict, not a conclusion a caller asserts.
//!
//! # What "coherent" is required to mean
//!
//! - the two selection identities digest the supplied ProgramSet and config;
//! - the config is a hostile-decodable `TokenBehaviorSelectionV2`, which is the
//!   record the descriptors themselves name as `config_schema`;
//! - the set's selector geometry is the one the open Hot layouts dispatch
//!   through, and it carries exactly the five canonical actions in order;
//! - each entry selects the descriptor whose bytes were supplied for that
//!   action, by digest;
//! - each descriptor claims the SELECTED kind, the Token-behavior config schema,
//!   and the request schema its own action actually dispatches through -- the
//!   four open actions the open-representation schema, terminal redemption the
//!   terminal one;
//! - every artifact the descriptor references is present, DECODES under its own
//!   type, and digests to the identity the descriptor names;
//! - each action's effect really routes that action, read out of the effect's
//!   own route template rather than out of the entry that claims it;
//! - all five descriptors agree on capacity profile, root schema, root width and
//!   derivation policy, because they are one selectable capability rather than
//!   five;
//! - the five descriptors are distinct.
//!
//! The artifacts are decoded rather than merely digested on purpose. A release
//! whose digests all agree but whose effect program is not a decodable effect
//! program would authenticate under a digest-only check and fail at the first
//! instruction that ran it.
//!
//! # Nothing here can observe a Market
//!
//! Every input is bytes or a content identity. There is no type in this module's
//! surface that carries a Market address, which is what makes the admission
//! runnable against a release compiled before its Market exists.

use dclutch_account_profile_contract::{lifecycle_v3::StateLifecyclePolicyV5, v2::AccountProfileV2};
use dclutch_capability_program_contract::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_effect_kernel::v4::ProgramV4 as EffectProgramV4;
use dclutch_execution_strategy_contract::v2::ExecutionStrategyProgramV2;
use dclutch_rational_representation_v2_contract::RepresentationActionV2;
use dclutch_rational_representation_v2_request_contract::generated::REQUEST_ACTION_OFFSET_V3;
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2};
use solana_program::hash::hash;

use crate::{Error, Result};

/// Number of action bundles one selectable open-capability release carries.
pub const OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1: usize = 5;

/// The canonical action order an open-capability release publishes.
///
/// This is the order [`crate::build_rational_open_capability_program_set_v6`]
/// assembles, restated here as the admission's expectation so a release whose
/// entries were permuted refuses rather than authenticating five actions into
/// four wrong selectors.
pub const OPEN_CAPABILITY_SELECTED_ACTIONS_V1: [RepresentationActionV2;
    OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1] = [
    RepresentationActionV2::Denominate,
    RepresentationActionV2::Reconstitute,
    RepresentationActionV2::IssueStructured,
    RepresentationActionV2::UnwrapStructured,
    RepresentationActionV2::RedeemTerminal,
];

/// The three content identities a Market's manifest entry names.
///
/// `kind` is the entry's `kind_id`, `program_set` its `release_id` and `config`
/// its `config_id`, so authenticating against this structure is authenticating
/// against exactly what a founded Market bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCapabilityArtifactSelectionV1 {
    /// Capability kind the Market selected.
    pub kind: [u8; 32],
    /// SHA-256 the Market selected as the capability release.
    pub program_set: [u8; 32],
    /// SHA-256 the Market selected as the capability config.
    pub config: [u8; 32],
}

/// Untrusted artifact bytes for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCapabilityActionArtifactBytesV1<'a> {
    /// Action these artifacts claim to implement.
    pub action: RepresentationActionV2,
    /// CapabilityProgramV4 descriptor bytes.
    pub descriptor: &'a [u8],
    /// Profile13 account interpreter bytes.
    pub account_profile: &'a [u8],
    /// RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// StateLifecyclePolicyV5 bytes.
    pub lifecycle_policy: &'a [u8],
    /// ExecutionStrategyProgramV2 bytes.
    pub strategy: &'a [u8],
    /// TransitionVM program bytes.
    pub transition: &'a [u8],
    /// EffectV4 program bytes.
    pub effect: &'a [u8],
}

/// Untrusted bytes for one complete release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCapabilityArtifactReleaseBytesV1<'a> {
    /// CapabilityProgramSetV2 bytes.
    pub program_set: &'a [u8],
    /// TokenBehaviorSelectionV2 config-record bytes.
    pub config: &'a [u8],
    /// Action artifacts in canonical action order.
    pub actions:
        [OpenCapabilityActionArtifactBytesV1<'a>; OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1],
}

/// Facts the admission established, for a caller that needs them downstream.
///
/// Returned rather than accepted: a caller cannot supply these, so nothing here
/// can be asserted into existence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCapabilityJoinedReleaseV1 {
    /// Descriptor identities in canonical action order.
    pub descriptors: [[u8; 32]; OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1],
    /// Capability kind every descriptor claimed, read off the descriptors.
    pub kind: [u8; 32],
    /// Capacity profile all five agreed on.
    pub capacity_profile: [u8; 32],
    /// Root schema all five agreed on.
    pub root_schema: [u8; 32],
    /// Derivation policy all five agreed on.
    pub derivation_policy: [u8; 32],
    /// Mutable root width all five agreed on.
    pub root_state_bytes: u32,
    /// Immutable Realm the config record binds.
    pub realm: [u8; 32],
    /// Immutable release set the config record binds.
    pub release_set: [u8; 32],
}

/// Authenticate one complete open-capability release from bytes alone.
pub fn authenticate_open_capability_release_v1(
    selection: OpenCapabilityArtifactSelectionV1,
    bytes: OpenCapabilityArtifactReleaseBytesV1<'_>,
) -> Result<OpenCapabilityJoinedReleaseV1> {
    // An all-zero kind is the shape a placeholder takes when nobody looks. The
    // manifest cannot select it, so neither can this.
    if selection.kind == [0; 32] {
        return Err(Error::ContentIdentity);
    }
    if hash(bytes.program_set).to_bytes() != selection.program_set
        || hash(bytes.config).to_bytes() != selection.config
    {
        return Err(Error::ContentIdentity);
    }

    // The config is the record the descriptors name as `config_schema`, so it
    // must be a real one rather than bytes that happen to digest correctly.
    let config = TokenBehaviorSelectionV2::decode(bytes.config).map_err(Error::TokenBehavior)?;

    let set = CapabilityProgramSetV2::decode(bytes.program_set).map_err(Error::CapabilityProgramSet)?;
    let selector_offset =
        u32::try_from(REQUEST_ACTION_OFFSET_V3).map_err(|_| Error::ArtifactGeometry)?;
    if set.selector_offset() != selector_offset
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1
    {
        return Err(Error::ArtifactGeometry);
    }

    let mut descriptors = [[0_u8; 32]; OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1];
    let mut agreed: Option<(([u8; 32], [u8; 32]), ([u8; 32], u32))> = None;
    for (ordinal, action) in OPEN_CAPABILITY_SELECTED_ACTIONS_V1.into_iter().enumerate() {
        let supplied = *bytes.actions.get(ordinal).ok_or(Error::ArtifactGeometry)?;
        if supplied.action != action {
            return Err(Error::ArtifactGeometry);
        }
        let entry = set
            .entry(u16::try_from(ordinal).map_err(|_| Error::ArtifactGeometry)?)
            .map_err(Error::CapabilityProgramSet)?;
        let descriptor_id = hash(supplied.descriptor).to_bytes();
        if entry.selector() != action as u32
            || entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4
            || entry.descriptor().program().to_bytes() != descriptor_id
        {
            return Err(Error::ArtifactGeometry);
        }

        let descriptor =
            CapabilityProgramV4::decode(supplied.descriptor).map_err(Error::CapabilityDescriptor)?;
        if descriptor.kind().to_bytes() != selection.kind
            || descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
            || descriptor.request_schema().to_bytes() != request_schema(action)
        {
            return Err(Error::ContentIdentity);
        }

        join_artifacts(descriptor, supplied, action)?;

        // One selectable capability, not five: the coordinates a manifest entry
        // reads off a descriptor must be the same whichever action supplies it.
        let observed = (
            (
                descriptor.capacity_profile().to_bytes(),
                descriptor.root_schema().to_bytes(),
            ),
            (
                descriptor.derivation_policy().to_bytes(),
                descriptor.root_state_bytes(),
            ),
        );
        match agreed {
            None => agreed = Some(observed),
            Some(expected) if expected == observed => {}
            Some(_) => return Err(Error::ContentIdentity),
        }

        *descriptors.get_mut(ordinal).ok_or(Error::ArtifactGeometry)? = descriptor_id;
    }

    // Five actions pointing at one descriptor would encode, digest and join
    // cleanly while routing four of them to the wrong program.
    for (index, descriptor) in descriptors.iter().enumerate() {
        if descriptors
            .get(index.checked_add(1).ok_or(Error::ArtifactGeometry)?..)
            .ok_or(Error::ArtifactGeometry)?
            .contains(descriptor)
        {
            return Err(Error::ContentIdentity);
        }
    }

    let ((capacity_profile, root_schema), (derivation_policy, root_state_bytes)) =
        agreed.ok_or(Error::ArtifactGeometry)?;
    Ok(OpenCapabilityJoinedReleaseV1 {
        descriptors,
        kind: selection.kind,
        capacity_profile,
        root_schema,
        derivation_policy,
        root_state_bytes,
        realm: config.realm(),
        release_set: config.release_set(),
    })
}

/// Request schema the given action's Hot family actually dispatches through.
fn request_schema(action: RepresentationActionV2) -> [u8; 32] {
    use dclutch_rational_representation_v2_contract::{
        OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3, RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
    };
    match action {
        RepresentationActionV2::RedeemTerminal => RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
        _ => OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3,
    }
}

/// Require every referenced artifact to be present, typed, and named.
///
/// Each artifact is decoded under its own type BEFORE its digest is compared,
/// so a release cannot pass by carrying well-digested rubbish.
fn join_artifacts(
    descriptor: CapabilityProgramV4,
    supplied: OpenCapabilityActionArtifactBytesV1<'_>,
    action: RepresentationActionV2,
) -> Result<()> {
    AccountProfileV2::decode(supplied.account_profile).map_err(Error::AccountProfileArtifact)?;
    RequestProfileV1::decode(supplied.request_profile).map_err(Error::RequestProfileArtifact)?;
    let lifecycle_id = hash(supplied.lifecycle_policy).to_bytes();
    StateLifecyclePolicyV5::decode_selected(lifecycle_id, lifecycle_id, supplied.lifecycle_policy)
        .map_err(Error::LifecycleArtifact)?;
    ExecutionStrategyProgramV2::decode(supplied.strategy).map_err(Error::ExecutionStrategy)?;
    dclutch_transition_vm::v3::ProgramV3::decode(supplied.transition)
        .map_err(Error::TransitionArtifact)?;
    let effect = EffectProgramV4::decode(supplied.effect).map_err(Error::EffectArtifactV4)?;

    let artifacts = descriptor.artifacts();
    for (reference, body) in [
        (artifacts.account_profile, supplied.account_profile),
        (artifacts.request_profile, supplied.request_profile),
        (artifacts.lifecycle, supplied.lifecycle_policy),
        (artifacts.strategy, supplied.strategy),
        (artifacts.transition, supplied.transition),
        (artifacts.effect, supplied.effect),
    ] {
        if reference.program().to_bytes() != hash(body).to_bytes() {
            return Err(Error::ContentIdentity);
        }
    }

    // The action an entry SELECTS and the action its effect actually ROUTES are
    // two different facts. A set whose entries were permuted keeps every digest
    // intact and every join green while dispatching each selector at another
    // action's effect, so the routed action is read out of the effect's own
    // template rather than out of the entry that claims it.
    let (fixed, _) = effect
        .base()
        .route_template(0)
        .map_err(Error::EffectArtifact)?;
    if fixed.get(REQUEST_ACTION_OFFSET_V3).copied() != Some(action as u8) {
        return Err(Error::ArtifactGeometry);
    }

    // The descriptor's derivation policy IS its lifecycle identity for this
    // family; a descriptor naming another would derive its root differently from
    // the policy it publishes.
    if descriptor.derivation_policy().to_bytes() != lifecycle_id {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}
