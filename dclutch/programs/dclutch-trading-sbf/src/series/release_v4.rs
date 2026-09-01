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
//! # The one input this module refuses to invent
//!
//! The generic assembler below still accepts `lifecycle` and `strategy` as
//! caller bytes — it is the mechanism the canonical completion drives, and
//! its tests exercise hostile substitutions through it. The canonical path is
//! [`series_consume_selected_release_v4`], which derives everything the
//! release set binds and leaves exactly one deployment fact as a parameter.
//!
//! **Lifecycle** is now EMITTED, not supplied: the 1b8228e9 ruling resolved
//! the open decisions this header used to carry, and
//! [`super::lifecycle_policy_v5`] encodes the root-only, derived,
//! lamport-silent policy. The Ticket's capability rent stays spoken for by
//! the funding path ([`super::commit_plans`]); a policy claiming it refuses
//! at [`super::artifacts_v4`]'s `TicketAuthorship` wall.
//!
//! **Strategy** keeps the one honest hole: the ShadowAot arm names the
//! *deployed* accelerator's certificate program. That identity is a fact
//! about a deployment (`dclutch-series-shadow-sbf` builds fail-closed until a
//! generated release is selected, so no local certificate exists to read),
//! and a builder that invented one would produce a release addressed to an
//! accelerator nobody runs. Every other strategy field is a schema constant
//! or the digest of the transition this module itself emits, so
//! [`encode_series_consume_strategy_v4`] takes precisely that one
//! [`ContentId`] and nothing else.

extern crate alloc;

use alloc::boxed::Box;
use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
    v2::SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2,
};
use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4,
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    shadow_v3::{SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3},
    v2::{
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        ExecutionStrategyProgramV2, StrategyDispositionV2,
    },
};
use dclutch_series_v3_kernel::generated::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3;
use dclutch_series_v3_kernel::request::SeriesActionV3;
use solana_program::hash::hash;

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
        SeriesConsumeAccountProfileInputV4, encode_series_consume_account_profile_v4_atomic,
        stamp_series_release_owned_widths_v4,
    },
    artifacts_v3::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ACTION_SELECTOR_OFFSET_V3,
        SERIES_ROOT_SCHEMA_PREIMAGE_V3, SERIES_SUCCESSOR_KIND_PREIMAGE_V3,
        SERIES_TICKET_DERIVATION_PREIMAGE_V3,
    },
    consume_artifacts_v4::{
        SERIES_CONSUME_BASE_EFFECT_BYTES_V4, SERIES_CONSUME_EFFECT_BYTES_V4,
        SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4, SERIES_CONSUME_TRANSITION_BYTES_V4,
        SeriesConsumeChildRequestsV4, encode_series_consume_effect_v4_from_requests_atomic,
        encode_series_consume_request_profile_v4_atomic,
        encode_series_consume_transition_v4_atomic,
    },
    lifecycle_policy_v5::{
        SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5, SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5,
        encode_series_consume_state_lifecycle_v5_atomic,
    },
    state::{SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3},
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
    // Each emitter owns its own scratch in its own #[inline(never)] frame:
    // holding all three scratch/output pairs in one frame overflowed the
    // 4096-byte SBF bound (measured at offset 4544 — 12 frame diagnostics
    // per compilation of this crate).
    let mut artifacts = SeriesConsumeEmittedArtifactsV4 {
        account_profile: [0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4],
        request_profile: [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4],
        transition: [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4],
    };
    emit_series_consume_account_profile_part_v4(profile, &mut artifacts.account_profile)?;
    emit_series_consume_request_profile_part_v4(&mut artifacts.request_profile)?;
    emit_series_consume_transition_part_v4(&mut artifacts.transition)?;
    Ok(artifacts)
}

#[inline(never)]
fn emit_series_consume_account_profile_part_v4(
    profile: SeriesConsumeAccountProfileInputV4<'_>,
    output: &mut [u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4],
) -> Result<()> {
    let mut scratch = [0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
    encode_series_consume_account_profile_v4_atomic(profile, &mut scratch, output)
        .map_err(|_| SeriesReleaseErrorV4::Encode)
}

#[inline(never)]
fn emit_series_consume_request_profile_part_v4(
    output: &mut [u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4],
) -> Result<()> {
    let mut scratch = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
    encode_series_consume_request_profile_v4_atomic(&mut scratch, output)
        .map_err(|_| SeriesReleaseErrorV4::Encode)
}

#[inline(never)]
fn emit_series_consume_transition_part_v4(
    output: &mut [u8; SERIES_CONSUME_TRANSITION_BYTES_V4],
) -> Result<()> {
    let mut scratch = [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4];
    encode_series_consume_transition_v4_atomic(&mut scratch, output)
        .map_err(|_| SeriesReleaseErrorV4::Encode)
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
            lifecycle: reference(
                supplied.lifecycle_schema,
                hash(supplied.lifecycle).to_bytes(),
            )?,
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

/// Stable refusal from the canonical selected-release completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesSelectedReleaseErrorV4 {
    /// One of the three fully determined artifact emitters refused.
    Emit,
    /// The canonical lifecycle-policy emitter refused.
    Lifecycle,
    /// The occurrence-specific Effect emitter refused.
    Effect,
    /// The strategy grammar refused the derived tuple.
    Strategy,
    /// Descriptor assembly refused.
    Descriptor,
    /// ProgramSetV2 encoding refused.
    ProgramSet,
    /// A supplied release differed from its own canonical recompilation.
    Publication,
}

/// Result alias for the selected-release completion.
pub type SelectedResult<T> = core::result::Result<T, SeriesSelectedReleaseErrorV4>;

/// Complete authenticated input for one selected Series Consume release.
#[derive(Clone, Copy, Debug)]
pub struct SeriesConsumeSelectedReleaseInputV4<'a> {
    /// Finalized Series Template record identity the descriptor's config binds.
    pub template: ContentId,
    /// The one deployment fact: the deployed Shadow accelerator's certificate.
    ///
    /// See this module's header — everything else in the strategy is a schema
    /// constant or a digest of bytes this compiler emits itself.
    pub shadow_certificate_program: ContentId,
    /// Exact canonical child requests the occurrence binds.
    pub child_requests: SeriesConsumeChildRequestsV4<'a>,
    /// Observed exact data widths at every fixed base coordinate.
    ///
    /// The two Trading-owned widths — the composite root and the Ticket
    /// replay account — are release constants and are DERIVED over whatever
    /// this array carries at those coordinates and their aliases; the caller
    /// owns only the genuinely observed remainder (wallets, mints, sysvars,
    /// role ProgramData).
    pub observed_data_lengths: &'a [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
}

/// Canonical Market-bindable publication for one selected Series release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeSelectedPublicationV4 {
    /// Finalized Series Template record identity.
    pub template: [u8; 32],
    /// SHA-256 identity of the one-entry ProgramSetV2 bytes.
    pub program_set_id: [u8; 32],
    /// Exact Consume descriptor identity the set selects.
    pub descriptor: [u8; 32],
    /// Profile13 account-profile identity.
    pub account_profile: [u8; 32],
    /// Request-profile identity.
    pub request_profile: [u8; 32],
    /// Root-only `StateLifecyclePolicyV5` identity.
    pub lifecycle: [u8; 32],
    /// Transition-program identity.
    pub transition: [u8; 32],
    /// Occurrence-specific DCE5 Effect identity.
    pub effect: [u8; 32],
    /// ShadowAot strategy identity.
    pub strategy: [u8; 32],
    /// Deployed Shadow accelerator certificate the strategy names.
    pub shadow_certificate_program: [u8; 32],
    /// Exact mutable family root tail width the descriptor binds.
    pub root_state_bytes: u32,
}

/// Canonical publication magic.
pub const SERIES_CONSUME_PUBLICATION_MAGIC_V4: [u8; 8] = *b"DCSRPB04";
/// Exact canonical publication width.
pub const SERIES_CONSUME_PUBLICATION_BYTES_V4: usize = 16 + PUBLICATION_IDENTITY_COUNT * 32 + 8;

const PUBLICATION_VERSION: u16 = 4;
const PUBLICATION_IDENTITY_COUNT: usize = 10;
const PUBLICATION_IDENTITY_START: usize = 16;
const PUBLICATION_SCALAR_START: usize =
    PUBLICATION_IDENTITY_START + PUBLICATION_IDENTITY_COUNT * 32;

impl SeriesConsumeSelectedPublicationV4 {
    /// Exact canonical publication bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; SERIES_CONSUME_PUBLICATION_BYTES_V4] {
        let mut output = [0_u8; SERIES_CONSUME_PUBLICATION_BYTES_V4];
        output[..8].copy_from_slice(&SERIES_CONSUME_PUBLICATION_MAGIC_V4);
        output[8..10].copy_from_slice(&PUBLICATION_VERSION.to_le_bytes());
        for (index, identity) in self.identities().iter().enumerate() {
            let start = PUBLICATION_IDENTITY_START + index * 32;
            output[start..start + 32].copy_from_slice(identity);
        }
        output[PUBLICATION_SCALAR_START..PUBLICATION_SCALAR_START + 4]
            .copy_from_slice(&self.root_state_bytes.to_le_bytes());
        output
    }

    /// SHA-256 identity of [`Self::to_bytes`] with no extra domain prefix.
    #[must_use]
    pub fn publication_id(&self) -> [u8; 32] {
        hash(&self.to_bytes()).to_bytes()
    }

    fn identities(&self) -> [[u8; 32]; PUBLICATION_IDENTITY_COUNT] {
        [
            self.template,
            self.program_set_id,
            self.descriptor,
            self.account_profile,
            self.request_profile,
            self.lifecycle,
            self.transition,
            self.effect,
            self.strategy,
            self.shadow_certificate_program,
        ]
    }
}

/// One complete selected Series Consume release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesConsumeSelectedReleaseV4 {
    /// The three fully determined artifacts, with their digests.
    ///
    /// Boxed: the three inline artifact arrays are frame-scale, and holding
    /// them by value put the compiler's 4 KiB SBF frame diagnostics on every
    /// call this builder makes (the checked release refuses any diagnostic).
    pub emitted: Box<SeriesConsumeEmittedArtifactsV4>,
    /// Exact canonical root-only `StateLifecyclePolicyV5` bytes.
    pub lifecycle: Vec<u8>,
    /// Exact occurrence-specific DCE5 Effect bytes.
    pub effect: Vec<u8>,
    /// Exact ShadowAot `ExecutionStrategyProgramV2` bytes.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// Exact `CapabilityProgramV4` descriptor bytes.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
    /// Exact one-entry `CapabilityProgramSetV2` bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity of `program_set`.
    pub program_set_id: [u8; 32],
    /// Canonical Market-bindable publication.
    pub publication: SeriesConsumeSelectedPublicationV4,
}

/// Emit the exact ShadowAot strategy from the one deployment fact.
///
/// `transition_program` is the digest of the transition bytes this module
/// emits; every schema below is a fixed constant of the strategy grammar for
/// the ShadowAot disposition, whose transport must be the Shadow transcript
/// pair. Nothing here is a field a human keeps in sync.
pub fn encode_series_consume_strategy_v4(
    shadow_certificate_program: ContentId,
    transition_program: ContentId,
) -> SelectedResult<[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2]> {
    let schema =
        |bytes: [u8; 32]| ContentId::new(bytes).map_err(|_| SeriesSelectedReleaseErrorV4::Strategy);
    Ok(ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::ShadowAot,
        schema(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        transition_program,
        schema(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        Some(shadow_certificate_program),
        schema(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        schema(SHADOW_REQUEST_SCHEMA_ID_V3)?,
        schema(SHADOW_ACK_SCHEMA_ID_V3)?,
    )
    .map_err(|_| SeriesSelectedReleaseErrorV4::Strategy)?
    .to_bytes())
}

/// Emit the three determined artifacts inside their own SBF frame.
///
/// The emitted struct is frame-scale (three inline artifact arrays); calling
/// the emitter from the release builder's frame put the builder over the
/// 4 KiB SBF bound. The emit call's return slot lives here, alone, and the
/// caller receives only a heap pointer.
#[inline(never)]
fn emit_series_consume_artifacts_boxed_v4(
    lengths: &[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> SelectedResult<Box<SeriesConsumeEmittedArtifactsV4>> {
    // One zero-initialized heap allocation, filled in place by the same
    // per-artifact framed emitters `emit_series_consume_artifacts_v4` uses.
    // Calling that function here instead would stack its return slot AND the
    // box argument copy in this frame - measured at 6,016 bytes against the
    // 4,096-byte SBF bound.
    let mut artifacts = Box::new(SeriesConsumeEmittedArtifactsV4 {
        account_profile: [0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4],
        request_profile: [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4],
        transition: [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4],
    });
    let profile = SeriesConsumeAccountProfileInputV4 {
        fixed_data_lengths: lengths,
    };
    emit_series_consume_account_profile_part_v4(profile, &mut artifacts.account_profile)
        .map_err(|_| SeriesSelectedReleaseErrorV4::Emit)?;
    emit_series_consume_request_profile_part_v4(&mut artifacts.request_profile)
        .map_err(|_| SeriesSelectedReleaseErrorV4::Emit)?;
    emit_series_consume_transition_part_v4(&mut artifacts.transition)
        .map_err(|_| SeriesSelectedReleaseErrorV4::Emit)?;
    Ok(artifacts)
}

/// Assemble and encode the descriptor inside its own SBF frame.
///
/// `CapabilityProgramV4` is a frame-scale temporary before `.encode()`
/// reduces it to its 600 canonical bytes; holding it in the release
/// builder's frame was the other half of the 4 KiB overflow.
#[inline(never)]
fn encode_series_consume_descriptor_framed_v4(
    emitted: &SeriesConsumeEmittedArtifactsV4,
    effect: &[u8],
    lifecycle: &[u8],
    strategy: &[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    template: ContentId,
) -> SelectedResult<[u8; CAPABILITY_PROGRAM_V4_BYTES]> {
    Ok(assemble_series_consume_descriptor_v4(
        emitted,
        SeriesConsumeSuppliedArtifactsV4 {
            effect,
            lifecycle,
            strategy,
            effect_schema: dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
            strategy_schema: EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
            lifecycle_schema: CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
            request_profile_schema: dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
            transition_schema: dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
            account_profile_schema: ACCOUNT_PROFILE_SCHEMA_ID_V2,
        },
        hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes(),
        hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes(),
        hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes(),
        hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes(),
        template.to_bytes(),
        u32::try_from(SERIES_STATE_BYTES_V3)
            .map_err(|_| SeriesSelectedReleaseErrorV4::Descriptor)?,
    )
    .map_err(|_| SeriesSelectedReleaseErrorV4::Descriptor)?
    .encode())
}

/// Compile the one canonical selected Series Consume release.
///
/// This is the completion the WAVE queue names: the artifacts assembled into
/// a descriptor, the descriptor named by a one-entry `CapabilityProgramSetV2`
/// selected by the Consume action byte, and the canonical publication a
/// Market manifest binds. Every schema, preimage digest, and Trading-owned
/// width is derived here; the inputs are the Template identity, the deployed
/// accelerator certificate, the occurrence's child requests, and the observed
/// external account widths.
pub fn series_consume_selected_release_v4(
    input: SeriesConsumeSelectedReleaseInputV4<'_>,
) -> SelectedResult<SeriesConsumeSelectedReleaseV4> {
    let mut lengths = *input.observed_data_lengths;
    stamp_series_release_owned_widths_v4(
        &mut lengths,
        u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| SeriesSelectedReleaseErrorV4::Emit)?,
        u32::try_from(SERIES_TICKET_STATE_BYTES_V3)
            .map_err(|_| SeriesSelectedReleaseErrorV4::Emit)?,
    );
    let emitted = emit_series_consume_artifacts_boxed_v4(&lengths)?;

    let mut lifecycle_scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
    let mut lifecycle = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
    encode_series_consume_state_lifecycle_v5_atomic(&mut lifecycle_scratch, &mut lifecycle)
        .map_err(|_| SeriesSelectedReleaseErrorV4::Lifecycle)?;

    let mut base_scratch = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
    let mut base = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
    let mut effect_scratch = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
    let mut effect = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
    encode_series_consume_effect_v4_from_requests_atomic(
        input.child_requests,
        &mut base_scratch,
        &mut base,
        &mut effect_scratch,
        &mut effect,
    )
    .map_err(|_| SeriesSelectedReleaseErrorV4::Effect)?;

    let strategy = encode_series_consume_strategy_v4(
        input.shadow_certificate_program,
        ContentId::new(emitted.transition_id())
            .map_err(|_| SeriesSelectedReleaseErrorV4::Strategy)?,
    )?;

    let descriptor = encode_series_consume_descriptor_framed_v4(
        &emitted,
        &effect,
        &lifecycle,
        &strategy,
        input.template,
    )?;

    let descriptor_id = hash(&descriptor).to_bytes();
    let entries = [CapabilityProgramSetEntryV2::new(
        SeriesActionV3::Consume as u32,
        CapabilityDescriptorReferenceV2::new(
            ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4)
                .map_err(|_| SeriesSelectedReleaseErrorV4::ProgramSet)?,
            ContentId::new(descriptor_id).map_err(|_| SeriesSelectedReleaseErrorV4::ProgramSet)?,
        ),
    )];
    let width = encoded_program_set_bytes_v2(entries.len())
        .map_err(|_| SeriesSelectedReleaseErrorV4::ProgramSet)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        SERIES_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| SeriesSelectedReleaseErrorV4::ProgramSet)?;
    let program_set_id = hash(&program_set).to_bytes();

    let publication = SeriesConsumeSelectedPublicationV4 {
        template: input.template.to_bytes(),
        program_set_id,
        descriptor: descriptor_id,
        account_profile: emitted.account_profile_id(),
        request_profile: emitted.request_profile_id(),
        lifecycle: hash(&lifecycle).to_bytes(),
        transition: emitted.transition_id(),
        effect: hash(&effect).to_bytes(),
        strategy: hash(&strategy).to_bytes(),
        shadow_certificate_program: input.shadow_certificate_program.to_bytes(),
        root_state_bytes: u32::try_from(SERIES_STATE_BYTES_V3)
            .map_err(|_| SeriesSelectedReleaseErrorV4::Descriptor)?,
    };
    Ok(SeriesConsumeSelectedReleaseV4 {
        emitted,
        lifecycle,
        effect,
        strategy,
        descriptor,
        program_set,
        program_set_id,
        publication,
    })
}

/// Hostile-rejoin one supplied selected release against its own inputs.
///
/// Recompiles the entire release from the same input and refuses any byte
/// that differs, so a substituted artifact, descriptor, selector, or
/// publication identity refuses even when each part is well formed on its
/// own. This is the same shape `validate_fractional_selected_release_v4`
/// gives the Fractional release.
pub fn validate_series_consume_selected_release_v4(
    release: &SeriesConsumeSelectedReleaseV4,
    input: SeriesConsumeSelectedReleaseInputV4<'_>,
) -> SelectedResult<()> {
    let canonical = series_consume_selected_release_v4(input)?;
    if *release != canonical
        || release.publication.publication_id() != canonical.publication.publication_id()
    {
        return Err(SeriesSelectedReleaseErrorV4::Publication);
    }
    Ok(())
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
        assert_eq!(emitted.transition.len(), SERIES_CONSUME_TRANSITION_BYTES_V4);
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

    /// The lifecycle spec is a checklist against the verifier, not prose.
    #[test]
    fn the_lifecycle_requirements_are_stated_as_a_checklist() {
        assert_eq!(SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4.len(), 5);
        assert!(
            SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4
                .iter()
                .all(|line| !line.is_empty())
        );
    }

    use crate::series::artifacts_v4::{
        SeriesArtifactErrorV4, SeriesConsumeArtifactBytesV4, SeriesConsumeArtifactRegistersV4,
        SeriesConsumeArtifactSelectionV4, authenticate_series_consume_artifacts_v4,
        tests::{CONSUME_IDENTITIES, CONSUME_SCALARS},
    };

    const TEMPLATE: [u8; 32] = [1; 32];

    fn template() -> ContentId {
        ContentId::new(TEMPLATE).expect("template identity")
    }

    fn certificate() -> ContentId {
        ContentId::new([0x77; 32]).expect("certificate identity")
    }

    fn canonical_release() -> SeriesConsumeSelectedReleaseV4 {
        let (lock, core, realize, claims) = crate::series::consume_artifacts_v4::tests::requests();
        let observed = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        series_consume_selected_release_v4(SeriesConsumeSelectedReleaseInputV4 {
            template: template(),
            shadow_certificate_program: certificate(),
            child_requests: SeriesConsumeChildRequestsV4 {
                lock: &lock,
                core: &core,
                realize: &realize,
                claims: &claims,
            },
            observed_data_lengths: &observed,
        })
        .expect("canonical selected Series release")
    }

    /// The exact register banks the Consume artifacts declare.
    ///
    /// Both widths are typed by the emitter's own constants: the identity bank
    /// has grown twice (1 -> 6 in `8f579821`, 6 -> 9 in `6121b131`) and a
    /// hand-carried width silently became a `validate_request_coverage`
    /// refusal rather than a compile error.
    fn registers() -> ([u64; CONSUME_SCALARS], [[u8; 32]; CONSUME_IDENTITIES]) {
        ([128, 64, 2, 32, 7, 9, 4], [[9_u8; 32]; CONSUME_IDENTITIES])
    }

    /// Authenticate the canonical release exactly as the on-chain join would,
    /// with the lifecycle policy (or the whole release) substituted.
    fn authenticate(
        release: &SeriesConsumeSelectedReleaseV4,
        lifecycle: &[u8],
        descriptor: &[u8],
        program_set: &[u8],
    ) -> core::result::Result<(), SeriesArtifactErrorV4> {
        let request = crate::series::effect_v4::tests::request();
        let (scalars, identities) = registers();
        authenticate_series_consume_artifacts_v4(
            SeriesConsumeArtifactSelectionV4 {
                program_set: hash(program_set).to_bytes(),
                template: template(),
            },
            SeriesConsumeArtifactBytesV4 {
                program_set,
                descriptor,
                account_profile: &release.emitted.account_profile,
                lifecycle_policy: lifecycle,
                request_profile: &release.emitted.request_profile,
                strategy: &release.strategy,
                transition: &release.emitted.transition,
                effect: &release.effect,
            },
            &request,
            SeriesConsumeArtifactRegistersV4 {
                tail_count: 0,
                scalars: &scalars,
                identities: &identities,
                funding_count_hint: 7,
            },
        )
        .map(|_| ())
    }

    /// Rebuild the descriptor and program set around substituted lifecycle
    /// bytes, so a hostile policy reaches the verifier's lifecycle conjuncts
    /// instead of dying at the content-identity wall.
    fn release_around_lifecycle(
        release: &SeriesConsumeSelectedReleaseV4,
        lifecycle: &[u8],
    ) -> ([u8; CAPABILITY_PROGRAM_V4_BYTES], Vec<u8>) {
        let descriptor = assemble_series_consume_descriptor_v4(
            &release.emitted,
            SeriesConsumeSuppliedArtifactsV4 {
                effect: &release.effect,
                lifecycle,
                strategy: &release.strategy,
                effect_schema: dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
                strategy_schema: EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                lifecycle_schema: CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
                request_profile_schema: dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                transition_schema: dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                account_profile_schema: ACCOUNT_PROFILE_SCHEMA_ID_V2,
            },
            hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes(),
            hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes(),
            hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes(),
            hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes(),
            TEMPLATE,
            u32::try_from(SERIES_STATE_BYTES_V3).expect("state width"),
        )
        .expect("hostile-lifecycle descriptor assembles")
        .encode();
        let entries = [CapabilityProgramSetEntryV2::new(
            SeriesActionV3::Consume as u32,
            CapabilityDescriptorReferenceV2::new(
                ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).expect("schema"),
                ContentId::new(hash(&descriptor).to_bytes()).expect("descriptor id"),
            ),
        )];
        let width = encoded_program_set_bytes_v2(entries.len()).expect("set width");
        let mut program_set = vec![0_u8; width];
        encode_program_set_v2(
            SERIES_ACTION_SELECTOR_OFFSET_V3,
            SelectorWidthV2::U8,
            &entries,
            &mut program_set,
        )
        .expect("program set");
        (descriptor, program_set)
    }

    /// The bar of the whole module: `authenticate_series_consume_artifacts_v4`
    /// ACCEPTS a real assembled bundle — the first admissible Series release —
    /// and the two ruled negative controls refuse at their exact conjuncts.
    #[test]
    fn the_first_admissible_series_release_authenticates_and_the_controls_refuse() {
        let release = canonical_release();
        assert_eq!(
            authenticate(
                &release,
                &release.lifecycle,
                &release.descriptor,
                &release.program_set,
            ),
            Ok(())
        );

        // Negative control 1: a Prepare-only or Expire-only policy decodes,
        // joins the profile, and is still refused at the nonzero-Consume-plan
        // conjunct.
        for action in [SeriesActionV3::Prepare, SeriesActionV3::Expire] {
            let single_action = crate::series::artifacts_v4::tests::hostile_policy(action, 0, None);
            let (descriptor, program_set) = release_around_lifecycle(&release, &single_action);
            assert_eq!(
                authenticate(&release, &single_action, &descriptor, &program_set),
                Err(SeriesArtifactErrorV4::Lifecycle)
            );
        }

        // Negative control 2: a Ticket-claiming policy is refused at the
        // second-author pin, with the pin's own code.
        let ticket_claiming =
            crate::series::artifacts_v4::tests::hostile_policy(SeriesActionV3::Consume, 59, None);
        let (descriptor, program_set) = release_around_lifecycle(&release, &ticket_claiming);
        assert_eq!(
            authenticate(&release, &ticket_claiming, &descriptor, &program_set),
            Err(SeriesArtifactErrorV4::TicketAuthorship)
        );
    }

    /// The canonical rejoin accepts the release it compiled and refuses any
    /// substituted byte, publication identities included.
    #[test]
    fn the_selected_release_rejoins_and_refuses_substitution() {
        let (lock, core, realize, claims) = crate::series::consume_artifacts_v4::tests::requests();
        let observed = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        let input = SeriesConsumeSelectedReleaseInputV4 {
            template: template(),
            shadow_certificate_program: certificate(),
            child_requests: SeriesConsumeChildRequestsV4 {
                lock: &lock,
                core: &core,
                realize: &realize,
                claims: &claims,
            },
            observed_data_lengths: &observed,
        };
        let release = series_consume_selected_release_v4(input).expect("release");
        assert_eq!(
            validate_series_consume_selected_release_v4(&release, input),
            Ok(())
        );
        assert_eq!(
            release.publication.to_bytes().len(),
            SERIES_CONSUME_PUBLICATION_BYTES_V4
        );
        assert_ne!(release.publication.publication_id(), [0_u8; 32]);

        let mut substituted = release.clone();
        let last = substituted.program_set.len() - 1;
        substituted.program_set[last] ^= 1;
        assert_eq!(
            validate_series_consume_selected_release_v4(&substituted, input),
            Err(SeriesSelectedReleaseErrorV4::Publication)
        );
        let mut relabeled = release.clone();
        relabeled.publication.template = [2; 32];
        assert_eq!(
            validate_series_consume_selected_release_v4(&relabeled, input),
            Err(SeriesSelectedReleaseErrorV4::Publication)
        );
    }

    /// The strategy derives everything except the one deployment fact.
    #[test]
    fn the_strategy_binds_the_emitted_transition_and_the_named_certificate() {
        let release = canonical_release();
        let strategy = dclutch_execution_strategy_contract::v2::ExecutionStrategyProgramV2::decode(
            &release.strategy,
        )
        .expect("strategy decodes");
        assert_eq!(
            strategy.transition_program().to_bytes(),
            release.emitted.transition_id()
        );
        assert_eq!(release.publication.shadow_certificate_program, [0x77; 32]);
        assert_eq!(
            release.publication.root_state_bytes,
            u32::try_from(SERIES_STATE_BYTES_V3).expect("state width")
        );
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
                &emitted, supplied, [9; 32], [10; 32], [11; 32], [12; 32], [13; 32], 64,
            ),
            Err(SeriesReleaseErrorV4::EmptyArtifact)
        );
    }
}

/// What a Series `StateLifecyclePolicyV5` must satisfy, read off the verifier.
///
/// This is not documentation of a decision that has been made. It is the
/// *specification the decision has to meet*, derived by reading what
/// [`super::artifacts_v4::authenticate_series_consume_artifacts_v4`] actually
/// does with the lifecycle bytes, so that whoever rules on the open questions
/// in this module's header can check their answer against the verifier rather
/// than against a description of it.
///
/// The verifier, in order:
///
/// 1. `require_artifact` — the descriptor's lifecycle reference must carry
///    schema `CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5` and name `sha256` of
///    the exact policy bytes. This is what [`assemble_series_consume_descriptor_v4`]
///    already guarantees for any bytes it is handed.
/// 2. `StateLifecyclePolicyV5::decode_selected` — the bytes must decode under
///    that schema at that digest. So the policy is content-addressed and a
///    substitution is caught here, not later.
/// 3. `validate_account_profile(account_profile)` — the policy is checked
///    against the **Series Consume account profile this module emits**. Any
///    state coordinate the policy names has to exist in Profile13 with the
///    matching rule. This is the conjunct a hand-written policy is most
///    likely to fail, and the reason the two artifacts cannot be authored
///    independently.
/// 4. `action_plan_count(SeriesActionV3::Consume as u32) != 0` — the policy
///    must declare at least one plan **for Consume specifically**. A policy
///    that covered only Prepare or Expire would decode, validate against the
///    profile, and still be refused here.
/// 5. `require_root_only_series_lifecycle` — no plan of any declared Series
///    action may name the Ticket replay coordinate as its state, payer, or
///    RentCredit, directly or through a route alias. The Ticket's lamport
///    flow has one author (the funding path's `ticket_capability_refund`),
///    and a second author refuses with its own code,
///    `SeriesArtifactErrorV4::TicketAuthorship`.
///
/// The questions this list once left open — which states step 3 covers and
/// who receives the refund — were ruled (WAVE.md, 1b8228e9) and are answered
/// by [`super::lifecycle_policy_v5`]: the root only, with the Ticket claim a
/// pinned refusal (step 5), every pin derived at emit time, and the refund
/// recipient a rule rather than an identity in the policy bytes.
pub const SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4: [&str; 5] = [
    "descriptor lifecycle reference carries CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 and sha256 of \
     the exact policy bytes",
    "the bytes decode via StateLifecyclePolicyV5::decode_selected at that digest",
    "validate_account_profile accepts it against the Series Consume Profile13 this module emits",
    "action_plan_count(SeriesActionV3::Consume) is nonzero, so a Prepare-only or Expire-only \
     policy is refused",
    "no plan names the Ticket replay coordinate as state, payer, or RentCredit - directly or via \
     route alias - so a second author for the Ticket's lamport flow refuses as TicketAuthorship",
];
