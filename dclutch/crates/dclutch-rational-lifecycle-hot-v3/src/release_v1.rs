//! Admission for one complete four-action Rational lifecycle release.
//!
//! # What this is, and what the family had instead
//!
//! Rational had no release authenticator at all. The closest thing,
//! [`crate::validate_rational_lifecycle_program_set_v6`], is a ROUND TRIP: it
//! rebuilds the entry table from bundles the caller already holds and requires
//! agreement. That proves an author did not fumble their own build. It proves
//! nothing about bytes that arrived from somewhere else, because it needs the
//! typed inputs to run at all.
//!
//! This is the other kind. It takes only two content identities and a pile of
//! BYTES, and decides whether those bytes are one coherent Rational release.
//! Every joined fact is re-derived here; nothing is taken on the caller's word,
//! and there is no parameter through which a caller could assert a conclusion.
//! It is the shape General's `authenticate_general_release_v3` has, supplied
//! for the family that lacked both halves.
//!
//! # What "coherent" is required to mean
//!
//! - the two selection identities digest the supplied ProgramSet and config;
//! - the config is a hostile-decodable `TokenBehaviorSelectionV2`, which is the
//!   record the descriptors themselves name as `config_schema`;
//! - the set's selector geometry is the one the family's Hot layouts dispatch
//!   through, and it carries exactly the four canonical actions in order;
//! - each entry selects the descriptor whose bytes were supplied for that
//!   action, by digest;
//! - each descriptor claims the Rational kind and the Token-behavior config
//!   schema, and names the request schema its own action actually uses -- the
//!   three fixed-cardinality actions the V6 schema, complete retirement the
//!   compact V4 one;
//! - every artifact the descriptor references is present, DECODES under its own
//!   type, and digests to the identity the descriptor names;
//! - all four descriptors agree on capacity profile, root schema, root width
//!   and derivation policy, because they are one selectable capability rather
//!   than four;
//! - the four descriptors are distinct.
//!
//! The artifacts are decoded rather than merely digested on purpose. A release
//! whose digests all agree but whose effect program is not a decodable effect
//! program would authenticate under a digest-only check and fail at the first
//! instruction that ran it.

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::AccountProfileV2,
};
use dclutch_capability_program_contract::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::{
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_effect_kernel::v4::ProgramV4 as EffectProgramV4;
use dclutch_execution_strategy_contract::v2::ExecutionStrategyProgramV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2, RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
    compact_hot_v4::{
        RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4, RationalLifecycleCompactHotLayoutV4,
    },
    hot_v3::RationalLifecycleHotLayoutV3,
    hot_v6::RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6,
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2};
use solana_program::hash::hash;

use crate::{Error, RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6, Result};

/// The two content identities a Market's manifest entry names.
///
/// `program_set` is the entry's `release_id` and `config` its `config_id`, so
/// authenticating against this structure is authenticating against exactly what
/// a founded Market bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalArtifactSelectionV1 {
    /// SHA-256 the Market selected as the capability release.
    pub program_set: [u8; 32],
    /// SHA-256 the Market selected as the capability config.
    pub config: [u8; 32],
}

/// Untrusted artifact bytes for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalActionArtifactBytesV1<'a> {
    /// Action these artifacts claim to implement.
    pub action: LifecycleActionV2,
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
pub struct RationalArtifactReleaseBytesV1<'a> {
    /// CapabilityProgramSetV2 bytes.
    pub program_set: &'a [u8],
    /// TokenBehaviorSelectionV2 config-record bytes.
    pub config: &'a [u8],
    /// Action artifacts in canonical action order.
    pub actions: [RationalActionArtifactBytesV1<'a>; 4],
}

/// Facts the admission established, for a caller that needs them downstream.
///
/// Returned rather than accepted: a caller cannot supply these, so nothing here
/// can be asserted into existence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalJoinedReleaseV1 {
    /// Descriptor identities in canonical action order.
    pub descriptors: [[u8; 32]; 4],
    /// Capability kind every descriptor claimed.
    pub kind: [u8; 32],
    /// Capacity profile all four agreed on.
    pub capacity_profile: [u8; 32],
    /// Root schema all four agreed on.
    pub root_schema: [u8; 32],
    /// Derivation policy all four agreed on.
    pub derivation_policy: [u8; 32],
    /// Mutable root width all four agreed on.
    pub root_state_bytes: u32,
    /// Immutable Realm the config record binds.
    pub realm: [u8; 32],
    /// Immutable release set the config record binds.
    pub release_set: [u8; 32],
}

/// Authenticate one complete Rational release from bytes alone.
pub fn authenticate_rational_release_v1(
    selection: RationalArtifactSelectionV1,
    bytes: RationalArtifactReleaseBytesV1<'_>,
) -> Result<RationalJoinedReleaseV1> {
    if hash(bytes.program_set).to_bytes() != selection.program_set
        || hash(bytes.config).to_bytes() != selection.config
    {
        return Err(Error::ContentIdentity);
    }

    // The config is the record the descriptors name as `config_schema`, so it
    // must be a real one rather than 144 bytes that happen to digest correctly.
    let config =
        TokenBehaviorSelectionV2::decode(bytes.config).map_err(Error::TokenBehavior)?;

    let set = CapabilityProgramSetV2::decode(bytes.program_set)
        .map_err(|_| Error::ArtifactGeometry)?;
    let selector_offset =
        u32::try_from(RationalLifecycleHotLayoutV3::ACTION).map_err(|_| Error::ArtifactGeometry)?;
    if RationalLifecycleHotLayoutV3::ACTION != RationalLifecycleCompactHotLayoutV4::ACTION
        || set.selector_offset() != selector_offset
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6.len()
    {
        return Err(Error::ArtifactGeometry);
    }

    let mut descriptors = [[0_u8; 32]; 4];
    let mut agreed: Option<(([u8; 32], [u8; 32]), ([u8; 32], u32))> = None;
    for (ordinal, action) in RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6.into_iter().enumerate() {
        let supplied = *bytes
            .actions
            .get(ordinal)
            .ok_or(Error::ArtifactGeometry)?;
        if supplied.action != action {
            return Err(Error::ActionGeometry);
        }
        let entry = set
            .entry(u16::try_from(ordinal).map_err(|_| Error::ArtifactGeometry)?)
            .map_err(|_| Error::ArtifactGeometry)?;
        let descriptor_id = hash(supplied.descriptor).to_bytes();
        if entry.selector() != u32::from(action.tag())
            || entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4
            || entry.descriptor().program().to_bytes() != descriptor_id
        {
            return Err(Error::ArtifactGeometry);
        }

        let descriptor =
            CapabilityProgramV4::decode(supplied.descriptor).map_err(Error::Descriptor)?;
        if descriptor.kind().to_bytes() != RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1
            || descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
            || descriptor.request_schema().to_bytes() != request_schema(action)
        {
            return Err(Error::ContentIdentity);
        }

        join_artifacts(descriptor, supplied)?;

        // One selectable capability, not four: the coordinates a manifest entry
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

        *descriptors
            .get_mut(ordinal)
            .ok_or(Error::ArtifactGeometry)? = descriptor_id;
    }

    // Four actions pointing at one descriptor would encode, digest and join
    // cleanly while routing three of them to the wrong program.
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
    Ok(RationalJoinedReleaseV1 {
        descriptors,
        kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
        capacity_profile,
        root_schema,
        derivation_policy,
        root_state_bytes,
        realm: config.realm(),
        release_set: config.release_set(),
    })
}

/// Request schema the given action's Hot family actually dispatches through.
const fn request_schema(action: LifecycleActionV2) -> [u8; 32] {
    match action {
        LifecycleActionV2::RetireReceipt => RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4,
        _ => RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6,
    }
}

/// Require every referenced artifact to be present, typed, and named.
///
/// Each artifact is decoded under its own type BEFORE its digest is compared,
/// so a release cannot pass by carrying well-digested rubbish.
fn join_artifacts(
    descriptor: CapabilityProgramV4,
    supplied: RationalActionArtifactBytesV1<'_>,
) -> Result<()> {
    AccountProfileV2::decode(supplied.account_profile).map_err(Error::AccountProfile)?;
    RequestProfileV1::decode(supplied.request_profile).map_err(Error::RequestProfile)?;
    let lifecycle_id = hash(supplied.lifecycle_policy).to_bytes();
    StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        supplied.lifecycle_policy,
    )
    .map_err(Error::LifecycleArtifact)?;
    ExecutionStrategyProgramV2::decode(supplied.strategy).map_err(Error::Strategy)?;
    dclutch_transition_vm::v3::ProgramV3::decode(supplied.transition)
        .map_err(Error::Transition)?;
    EffectProgramV4::decode(supplied.effect).map_err(Error::EffectV4)?;

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
    // The descriptor's derivation policy IS its lifecycle identity for this
    // family; a descriptor naming another would derive its root differently
    // from the policy it publishes.
    if descriptor.derivation_policy().to_bytes() != lifecycle_id {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RationalLifecycleCompactBundleV4, RationalLifecycleProgramSetInputV6,
        RationalLifecycleSelectedBundleV6, build_rational_lifecycle_program_set_v6,
        selected_set_v6::tests::{basis, compact, id, lifecycle_policy, selected, selection},
    };

    /// Everything one release needs, owned so the byte slices outlive the join.
    struct Release {
        set: crate::RationalLifecycleProgramSetV6,
        fixed: Vec<RationalLifecycleSelectedBundleV6>,
        compact: RationalLifecycleCompactBundleV4,
    }

    fn release() -> Release {
        let basis = basis();
        let lifecycle = lifecycle_policy();
        let fixed = [
            LifecycleActionV2::ActivateReceipt,
            LifecycleActionV2::ActivateCoordinate,
            LifecycleActionV2::RetireCoordinate,
        ]
        .map(|action| selected(action, &basis, &lifecycle))
        .to_vec();
        let compact_bundle = compact(&basis, &lifecycle);
        let set = build_rational_lifecycle_program_set_v6(RationalLifecycleProgramSetInputV6 {
            token_behavior_selection: selection(),
            activate_receipt: fixed.first().expect("receipt"),
            activate_coordinate: fixed.get(1).expect("coordinate"),
            retire_coordinate: fixed.get(2).expect("retire"),
            retire_receipt: &compact_bundle,
        })
        .expect("ProgramSet");
        Release {
            set,
            fixed,
            compact: compact_bundle,
        }
    }

    impl Release {
        fn selection(&self) -> RationalArtifactSelectionV1 {
            RationalArtifactSelectionV1 {
                program_set: self.set.program_set_id,
                config: self.set.token_behavior_selection_id,
            }
        }

        fn actions(&self) -> [RationalActionArtifactBytesV1<'_>; 4] {
            let fixed = |index: usize| {
                let bundle = self.fixed.get(index).expect("fixed bundle");
                RationalActionArtifactBytesV1 {
                    action: bundle.action,
                    descriptor: &bundle.descriptor,
                    account_profile: &bundle.account_profile,
                    request_profile: &bundle.request_profile,
                    lifecycle_policy: &bundle.lifecycle_policy,
                    strategy: &bundle.strategy,
                    transition: &bundle.transition,
                    effect: &bundle.effect,
                }
            };
            [
                fixed(0),
                fixed(1),
                fixed(2),
                RationalActionArtifactBytesV1 {
                    action: LifecycleActionV2::RetireReceipt,
                    descriptor: &self.compact.descriptor,
                    account_profile: &self.compact.account_profile,
                    request_profile: &self.compact.request_profile,
                    lifecycle_policy: &self.compact.lifecycle_policy,
                    strategy: &self.compact.strategy,
                    transition: &self.compact.transition,
                    effect: &self.compact.effect,
                },
            ]
        }

        fn bytes(&self) -> RationalArtifactReleaseBytesV1<'_> {
            RationalArtifactReleaseBytesV1 {
                program_set: &self.set.program_set,
                config: &self.set.token_behavior_selection,
                actions: self.actions(),
            }
        }
    }

    /// The release the family's own builders emit is one this admission accepts.
    ///
    /// The direction matters: the builder is not trusted to be right because it
    /// is ours, it is required to produce bytes that survive a join performed
    /// from the bytes alone.
    #[test]
    fn the_compiled_release_is_admitted_and_reports_what_it_joined() {
        let release = release();
        let joined = authenticate_rational_release_v1(release.selection(), release.bytes())
            .expect("admission");
        assert_eq!(joined.kind, RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1);
        assert_eq!(joined.realm, id(18));
        assert_eq!(joined.release_set, id(15));
        assert_eq!(joined.capacity_profile, id(43));
        assert_eq!(joined.root_schema, id(42));
        assert_eq!(joined.root_state_bytes, 64);
        // The reported descriptors are the ones the set actually selects.
        for (ordinal, descriptor) in joined.descriptors.into_iter().enumerate() {
            let entry = CapabilityProgramSetV2::decode(&release.set.program_set)
                .expect("set")
                .entry(u16::try_from(ordinal).expect("ordinal"))
                .expect("entry");
            assert_eq!(entry.descriptor().program().to_bytes(), descriptor);
        }
        // Four distinct programs, which is what the distinctness check buys.
        let mut seen = joined.descriptors.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4);
    }

    /// A selection naming other bytes refuses before anything is decoded.
    #[test]
    fn a_selection_that_does_not_digest_the_supplied_bytes_refuses() {
        let release = release();
        for selection in [
            RationalArtifactSelectionV1 {
                program_set: id(90),
                config: release.set.token_behavior_selection_id,
            },
            RationalArtifactSelectionV1 {
                program_set: release.set.program_set_id,
                config: id(91),
            },
        ] {
            assert_eq!(
                authenticate_rational_release_v1(selection, release.bytes()),
                Err(Error::ContentIdentity)
            );
        }
    }

    /// Substituted artifact bytes refuse even though the descriptor is intact.
    ///
    /// This is the check a digest-only join gets right and a "looks plausible"
    /// join gets wrong: the descriptor still decodes and still names its
    /// artifacts, but one of the bodies is no longer the one it named.
    #[test]
    fn an_artifact_body_the_descriptor_does_not_name_refuses() {
        let release = release();
        let other = release.fixed.get(1).expect("another action");
        let mut actions = release.actions();
        // Action 0's effect replaced by action 1's: same type, decodes fine,
        // wrong identity.
        actions.get_mut(0).expect("first").effect = &other.effect;
        assert_eq!(
            authenticate_rational_release_v1(
                release.selection(),
                RationalArtifactReleaseBytesV1 {
                    program_set: &release.set.program_set,
                    config: &release.set.token_behavior_selection,
                    actions,
                }
            ),
            Err(Error::ContentIdentity)
        );
    }

    /// A well-digested artifact that is not its own type refuses.
    ///
    /// The reason artifacts are DECODED and not merely digested: bytes whose
    /// digest agrees with the descriptor but which are not a decodable program
    /// would pass a digest-only admission and fail at the first instruction.
    #[test]
    fn an_artifact_that_is_not_its_own_type_refuses() {
        let release = release();
        // Hand the effect coordinate the account-profile bytes, and rebuild the
        // descriptor's expectation around them so the DIGEST agrees. Only the
        // typed decode can catch this.
        let first = release.fixed.first().expect("first");
        let mut actions = release.actions();
        actions.get_mut(0).expect("first").effect = &first.account_profile;
        let result = authenticate_rational_release_v1(
            release.selection(),
            RationalArtifactReleaseBytesV1 {
                program_set: &release.set.program_set,
                config: &release.set.token_behavior_selection,
                actions,
            },
        );
        assert!(matches!(
            result,
            Err(Error::EffectV4(_)) | Err(Error::ContentIdentity)
        ));
    }

    /// Actions presented out of canonical order refuse.
    #[test]
    fn actions_out_of_canonical_order_refuse() {
        let release = release();
        let mut actions = release.actions();
        actions.swap(0, 1);
        assert_eq!(
            authenticate_rational_release_v1(
                release.selection(),
                RationalArtifactReleaseBytesV1 {
                    program_set: &release.set.program_set,
                    config: &release.set.token_behavior_selection,
                    actions,
                }
            ),
            Err(Error::ActionGeometry)
        );
    }

    /// A config that digests correctly but is not a Token behavior selection.
    #[test]
    fn a_config_that_is_not_a_token_behavior_selection_refuses() {
        let release = release();
        let forged = vec![0_u8; release.set.token_behavior_selection.len()];
        let selection = RationalArtifactSelectionV1 {
            program_set: release.set.program_set_id,
            config: hash(&forged).to_bytes(),
        };
        assert!(matches!(
            authenticate_rational_release_v1(
                selection,
                RationalArtifactReleaseBytesV1 {
                    program_set: &release.set.program_set,
                    config: &forged,
                    actions: release.actions(),
                }
            ),
            Err(Error::TokenBehavior(_))
        ));
    }
}
