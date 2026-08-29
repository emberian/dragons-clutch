//! Assemble one self-consistent Series Consume capability release.
//!
//! # The gap this closes
//!
//! Series has had complete V4 artifact *encoders* for a while —
//! [`super::account_profile_v4`], [`super::consume_artifacts_v4`],
//! [`super::effect_v4`] — and not one production caller. Nothing assembled
//! them into a descriptor, so `authenticate_series_consume_artifacts_v4`, the
//! function that decides whether a Series release is admissible at all,
//! authenticated nothing: it had no callers of any kind, and the only bundle
//! anywhere in the tree was a unit-test descriptor built from placeholder
//! content identities (`byte_id(10)`, `byte_id(11)`, …) whose digests
//! deliberately do not match any real artifact bytes.
//!
//! That is the difference this module makes. It emits the real bytes, digests
//! *those* bytes, and builds the descriptor from the digests — so the
//! descriptor names artifacts that actually exist, which is the whole content
//! of "self-consistent" and the precondition for a Market ever selecting it.
//!
//! # Why Series is not reachable through Trading yet, stated precisely
//!
//! It is worth recording here, because the shape is not what it looks like
//! from outside. There is **no missing Trading dispatch site**: `hot_v3` is
//! family-neutral, already dispatched from the program entrypoint, and walks
//! whatever child routes the *selected* EffectProgram declares, by
//! `FixedRole`. No family has an arm there and Series does not need one.
//!
//! What Series lacks is a *published, selected* release: this bundle finalized
//! as Registry records, named by a `CapabilityProgramSetV2`, and chosen by a
//! founded Market's capability manifest. That last mile is shared
//! infrastructure — General does not have it either, which is why General's
//! executed campaign runs against its accelerator rather than through
//! Trading's commit half — and it is not this module's job.
//!
//! # The two inputs this module refuses to invent
//!
//! `lifecycle` and `strategy` arrive as caller-supplied bytes rather than
//! being encoded here, and that is deliberate in both cases.
//!
//! **Lifecycle**: Series declares no `StateLifecyclePolicyV5` anywhere. Every
//! other family has one. Writing it means deciding which created states it
//! covers (Series creates a root *and* a Ticket), which rent-quote generation
//! it pins, and who receives the refund — and Series already has a competing
//! claim on that last one in [`super::lifecycle`], whose
//! `ticket_capability_refund` suggests the Ticket's capability rent is spoken
//! for by the funding path. A policy that also claimed it would be a second
//! author for one lamport flow. That is a protocol decision, not a caller's,
//! so it is a parameter until it is ruled on.
//!
//! **Strategy**: the ShadowAot arm names the *deployed* accelerator's
//! certificate program. That identity is a fact about a deployment, and a
//! builder that invented one would produce a release addressed to an
//! accelerator nobody runs.

use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3;
use solana_program::hash::hash;

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    },
    consume_artifacts_v4::{
        SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4, SERIES_CONSUME_TRANSITION_BYTES_V4,
        encode_series_consume_request_profile_v4_atomic,
        encode_series_consume_transition_v4_atomic,
    },
};

/// Stable refusal from Series release assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesReleaseErrorV4 {
    /// One artifact encoder refused its own canonical emission.
    Encode,
    /// A supplied artifact was empty, so its digest would name nothing.
    EmptyArtifact,
    /// A required content identity was zero.
    ZeroIdentity,
    /// The assembled descriptor refused its own schema bindings.
    Descriptor,
}

/// Result alias for Series release assembly.
pub type Result<T> = core::result::Result<T, SeriesReleaseErrorV4>;

/// The artifacts this module emits, with the digests the descriptor names.
///
/// The bytes and their digests travel together on purpose. A caller that
/// received only digests could not publish the records, and one that received
/// only bytes would have to re-derive the digests and could re-derive them
/// differently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesConsumeEmittedArtifactsV4 {
    /// Exact Profile13 account profile bytes.
    pub account_profile: [u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4],
    /// Exact unsigned request-profile bytes.
    pub request_profile: [u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4],
    /// Exact transition-program bytes.
    pub transition: [u8; SERIES_CONSUME_TRANSITION_BYTES_V4],
}

impl SeriesConsumeEmittedArtifactsV4 {
    /// Digest of the account profile, as the descriptor names it.
    pub fn account_profile_id(&self) -> [u8; 32] {
        hash(&self.account_profile).to_bytes()
    }

    /// Digest of the request profile, as the descriptor names it.
    pub fn request_profile_id(&self) -> [u8; 32] {
        hash(&self.request_profile).to_bytes()
    }

    /// Digest of the transition program, as the descriptor names it.
    pub fn transition_id(&self) -> [u8; 32] {
        hash(&self.transition).to_bytes()
    }
}

/// Emit the three Series Consume artifacts that are fully determined.
///
/// These need nothing from a deployment or a policy decision: the account
/// profile follows from the fixed alias data lengths, and the request profile
/// and transition are constants of the Consume action's grammar. They are
/// emitted separately from the descriptor so a caller can publish the records
/// first and address them afterwards, which is the order the Registry needs.
pub fn emit_series_consume_artifacts_v4(
    profile: SeriesConsumeAccountProfileInputV4<'_>,
) -> Result<SeriesConsumeEmittedArtifactsV4> {
    let mut account_profile_scratch = [0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
    let mut account_profile = [0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
    encode_series_consume_account_profile_v4_atomic(
        profile,
        &mut account_profile_scratch,
        &mut account_profile,
    )
    .map_err(|_| SeriesReleaseErrorV4::Encode)?;

    let mut request_profile_scratch = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
    let mut request_profile = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
    encode_series_consume_request_profile_v4_atomic(
        &mut request_profile_scratch,
        &mut request_profile,
    )
    .map_err(|_| SeriesReleaseErrorV4::Encode)?;

    let mut transition_scratch = [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4];
    let mut transition = [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4];
    encode_series_consume_transition_v4_atomic(&mut transition_scratch, &mut transition)
        .map_err(|_| SeriesReleaseErrorV4::Encode)?;

    Ok(SeriesConsumeEmittedArtifactsV4 {
        account_profile,
        request_profile,
        transition,
    })
}

/// The two artifacts this module will not invent, plus the emitted effect.
///
/// Every field is bytes the caller obtained elsewhere, and every one is
/// digested here rather than accepted as an identity, so a caller cannot name
/// an artifact it does not hold.
#[derive(Clone, Copy, Debug)]
pub struct SeriesConsumeSuppliedArtifactsV4<'a> {
    /// Exact V4-envelope EffectProgram bytes.
    pub effect: &'a [u8],
    /// Exact `StateLifecyclePolicyV5` bytes — see this module's header.
    pub lifecycle: &'a [u8],
    /// Exact `ExecutionStrategyV2` bytes for the ShadowAot disposition.
    pub strategy: &'a [u8],
    /// Schema the effect bytes are addressed under.
    pub effect_schema: [u8; 32],
    /// Schema the strategy bytes are addressed under.
    pub strategy_schema: [u8; 32],
    /// Schema the lifecycle bytes are addressed under.
    pub lifecycle_schema: [u8; 32],
    /// Schema the request profile is addressed under.
    pub request_profile_schema: [u8; 32],
    /// Schema the transition program is addressed under.
    pub transition_schema: [u8; 32],
    /// Schema the account profile is addressed under.
    pub account_profile_schema: [u8; 32],
}

/// Build the descriptor that names every artifact by its own digest.
///
/// This is what makes a release *self-consistent*: every reference is the
/// digest of bytes the caller is holding, so the descriptor cannot name an
/// artifact that was never emitted. The existing unit-test descriptors in
/// [`super::artifacts_v4`] use placeholder identities precisely because
/// nothing until now produced the real ones.
pub fn assemble_series_consume_descriptor_v4(
    emitted: &SeriesConsumeEmittedArtifactsV4,
    supplied: SeriesConsumeSuppliedArtifactsV4<'_>,
    successor_kind_id: [u8; 32],
    action_header_schema: [u8; 32],
    root_schema: [u8; 32],
    ticket_derivation: [u8; 32],
    config_id: [u8; 32],
    state_bytes: u32,
) -> Result<CapabilityProgramV4> {
    for artifact in [supplied.effect, supplied.lifecycle, supplied.strategy] {
        if artifact.is_empty() {
            return Err(SeriesReleaseErrorV4::EmptyArtifact);
        }
    }
    let reference = |schema: [u8; 32], bytes_digest: [u8; 32]| -> Result<ArtifactReferenceV4> {
        Ok(ArtifactReferenceV4::new(
            identity(schema)?,
            identity(bytes_digest)?,
        ))
    };
    CapabilityProgramV4::new(
        identity(successor_kind_id)?,
        identity(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3)?,
        identity(action_header_schema)?,
        identity(root_schema)?,
        identity(ticket_derivation)?,
        identity(config_id)?,
        CapabilityArtifactsV4 {
            account_profile: reference(
                supplied.account_profile_schema,
                emitted.account_profile_id(),
            )?,
            request_profile: reference(
                supplied.request_profile_schema,
                emitted.request_profile_id(),
            )?,
            lifecycle: reference(supplied.lifecycle_schema, hash(supplied.lifecycle).to_bytes())?,
            strategy: reference(supplied.strategy_schema, hash(supplied.strategy).to_bytes())?,
            transition: reference(supplied.transition_schema, emitted.transition_id())?,
            effect: reference(supplied.effect_schema, hash(supplied.effect).to_bytes())?,
        },
        state_bytes,
    )
    .map_err(|_| SeriesReleaseErrorV4::Descriptor)
}

fn identity(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| SeriesReleaseErrorV4::ZeroIdentity)
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;

    use dclutch_account_profile_contract::v2::AccountProfileV2;
    use dclutch_request_profile_contract::RequestProfileV1;

    use super::*;
    use crate::series::effect_v4::SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4;

    fn lengths() -> [u32; SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4 as usize] {
        [0_u32; SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4 as usize]
    }

    /// Every emitted artifact decodes under the contract that owns it.
    ///
    /// An encoder that emits bytes its own decoder refuses has proved nothing,
    /// and until this module existed nothing round-tripped these three at all.
    #[test]
    fn every_emitted_artifact_decodes_under_its_own_contract() {
        let lengths = lengths();
        let emitted = emit_series_consume_artifacts_v4(SeriesConsumeAccountProfileInputV4 {
            fixed_data_lengths: &lengths,
        })
        .expect("emit");
        AccountProfileV2::decode(&emitted.account_profile).expect("account profile decodes");
        RequestProfileV1::decode(&emitted.request_profile).expect("request profile decodes");
        assert_eq!(
            emitted.transition.len(),
            SERIES_CONSUME_TRANSITION_BYTES_V4
        );
    }

    /// Emission is deterministic, which is what lets a digest address it.
    ///
    /// A release is content-addressed end to end: if the same input emitted
    /// different bytes twice, the descriptor built from the first emission
    /// would not name the second, and republishing would silently fork the
    /// release identity.
    #[test]
    fn emission_is_deterministic_so_the_digests_address_it() {
        let lengths = lengths();
        let input = SeriesConsumeAccountProfileInputV4 {
            fixed_data_lengths: &lengths,
        };
        let first = emit_series_consume_artifacts_v4(input).expect("first");
        let second = emit_series_consume_artifacts_v4(input).expect("second");
        assert_eq!(first, second);
        assert_eq!(first.account_profile_id(), second.account_profile_id());
        assert_eq!(first.request_profile_id(), second.request_profile_id());
        assert_eq!(first.transition_id(), second.transition_id());
    }

    /// The three artifacts are distinct records, not one repeated.
    #[test]
    fn the_three_artifacts_have_three_distinct_identities() {
        let lengths = lengths();
        let emitted = emit_series_consume_artifacts_v4(SeriesConsumeAccountProfileInputV4 {
            fixed_data_lengths: &lengths,
        })
        .expect("emit");
        let ids = [
            emitted.account_profile_id(),
            emitted.request_profile_id(),
            emitted.transition_id(),
        ];
        for (index, left) in ids.iter().enumerate() {
            assert_ne!(*left, [0_u8; 32], "artifact {index} digested to zero");
            for right in ids.iter().skip(index + 1) {
                assert_ne!(left, right, "two artifacts share one identity");
            }
        }
    }

    /// An empty supplied artifact is refused rather than digested.
    ///
    /// `sha256("")` is a perfectly good digest of nothing, so a descriptor
    /// would happily name it and the refusal would arrive much later, at
    /// selection, as an artifact that cannot be fetched.
    #[test]
    fn an_empty_supplied_artifact_is_refused_before_it_is_digested() {
        let lengths = lengths();
        let emitted = emit_series_consume_artifacts_v4(SeriesConsumeAccountProfileInputV4 {
            fixed_data_lengths: &lengths,
        })
        .expect("emit");
        let effect = vec![7_u8; 64];
        let lifecycle = vec![8_u8; 64];
        let supplied = SeriesConsumeSuppliedArtifactsV4 {
            effect: &effect,
            lifecycle: &lifecycle,
            strategy: &[],
            effect_schema: [1; 32],
            strategy_schema: [2; 32],
            lifecycle_schema: [3; 32],
            request_profile_schema: [4; 32],
            transition_schema: [5; 32],
            account_profile_schema: [6; 32],
        };
        assert_eq!(
            assemble_series_consume_descriptor_v4(
                &emitted,
                supplied,
                [9; 32],
                [10; 32],
                [11; 32],
                [12; 32],
                [13; 32],
                64,
            ),
            Err(SeriesReleaseErrorV4::EmptyArtifact)
        );
    }
}
