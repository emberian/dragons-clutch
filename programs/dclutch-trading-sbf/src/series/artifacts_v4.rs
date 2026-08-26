//! Schema-bound artifact admission for the global Series Consume plan.
//!
//! `CapabilityProgramSetV2` selects one exact `CapabilityProgramV4` schema and
//! content pair from the authenticated action byte.  The descriptor then binds
//! every independently finalized interpreter artifact, including lifecycle and
//! the DCE5 Effect successor.  This module grants no account, receipt, scalar,
//! CPI, or state-write authority; the projected outer and common Hot executor
//! retain those physical responsibilities.

use dclutch_account_profile_contract::{
    lifecycle_v3::{SUCCESSOR_SCHEMA_RELEASE_ID as LIFECYCLE_SCHEMA_ID_V4, StateLifecyclePolicyV4},
    v2::{
        AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
        SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2,
    },
};
use dclutch_capability_program_contract::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::{
        ArtifactReferenceV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2,
};
use dclutch_request_profile_contract::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_series_v3_kernel::{
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_transition_vm::{MAX_IDENTITIES, MAX_SCALARS, v3::ProgramV3 as TransitionProgramV3};
use solana_program::hash::hash;

use crate::projected_market_v2::AuthenticatedFoundSpanV2;

use super::{
    artifacts_v3::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ACTION_SELECTOR_OFFSET_V3,
        SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
        SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TICKET_DERIVATION_PREIMAGE_V3,
        SERIES_WITNESS_ITEM_BYTES_V3, SeriesRequestSlicesV3,
    },
    effect_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4, SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4,
        SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4, SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4,
        SeriesConsumeEffectV4, SeriesConsumeRouteWindowV4,
    },
    instruction::SERIES_ACTION_HEADER_BYTES_V3,
    state::SERIES_STATE_BYTES_V3,
};

/// Exact finalized artifacts selected for one Series Consume execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeArtifactBytesV4<'a> {
    /// Canonical action-to-schema/content descriptor set.
    pub program_set: &'a [u8],
    /// Exact selected `CapabilityProgramV4` bytes.
    pub descriptor: &'a [u8],
    /// Exact runtime AccountProfile bytes.
    pub account_profile: &'a [u8],
    /// Exact successor StateLifecyclePolicy bytes.
    pub lifecycle_policy: &'a [u8],
    /// Exact fixed-header RequestProfile bytes.
    pub request_profile: &'a [u8],
    /// Exact generic ExecutionStrategy V2 bytes.
    pub strategy: &'a [u8],
    /// Exact underlying TransitionVM V3 bytes.
    pub transition: &'a [u8],
    /// Exact global DCE5 Effect bytes.
    pub effect: &'a [u8],
}

/// Immutable selections authenticated before the Series artifact join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeArtifactSelectionV4 {
    /// Capability release selecting the exact ProgramSet record.
    pub program_set: [u8; 32],
    /// Manifest config selecting the exact Series Template.
    pub template: ContentId,
}

/// Post-RequestProfile register banks used to resolve the global DCE5 plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeArtifactRegistersV4<'a> {
    /// Product-authenticated runtime tail width.
    pub tail_count: u32,
    /// Complete common scalar bank after RequestProfile and Transition.
    pub scalars: &'a [u64],
    /// Complete common identity bank after RequestProfile and Transition.
    pub identities: &'a [[u8; 32]],
    /// Bounded pre-Core routing hint; not an attested continuation scalar.
    pub funding_count_hint: u16,
}

/// Stable refusal from the Series V4 artifact join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesArtifactErrorV4 {
    /// A selected content identity was zero or differed from exact bytes.
    ContentIdentity,
    /// ProgramSet selection geometry or descriptor selection refused.
    ProgramSet,
    /// Family request, Template, or exact proof boundary refused.
    Request,
    /// Descriptor semantic identity, schema, or root geometry differed.
    Descriptor,
    /// AccountProfile schema, content, hostile decoding, or geometry refused.
    AccountProfile,
    /// Lifecycle schema, content, hostile decoding, or action plan refused.
    Lifecycle,
    /// RequestProfile schema, content, hostile decoding, or projection refused.
    RequestProfile,
    /// ExecutionStrategy schema, content, or transport refused.
    Strategy,
    /// Transition schema, content, hostile decoding, or geometry refused.
    Transition,
    /// DCE5 schema, content, topology, request coverage, or geometry refused.
    Effect,
    /// Cross-artifact register or account geometry differed.
    Geometry,
    /// Current Core did not attest the exact FundingState span selected by the artifact.
    FoundAcknowledgement,
}

/// Result alias for Series V4 artifact admission.
pub type Result<T> = core::result::Result<T, SeriesArtifactErrorV4>;

/// Fully joined schema-bound Series Consume artifact bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeArtifactBundleV4<'a> {
    /// Hostile-decoded complete semantic request.
    pub request: SeriesActionRequestV3<'a>,
    /// Explicit header and exact no-leftover proof suffix.
    pub slices: SeriesRequestSlicesV3<'a>,
    /// Exact selected schema-bound descriptor.
    pub descriptor: CapabilityProgramV4,
    /// Exact runtime AccountProfile.
    pub account_profile: AccountProfileV2<'a>,
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: StateLifecyclePolicyV4<'a>,
    /// Exact header RequestProfile.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact selected execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
    /// Exact underlying transition program.
    pub transition: TransitionProgramV3<'a>,
    /// Exact global DCE5 Consume plan.
    pub effect: SeriesConsumeEffectV4<'a>,
}

/// Live-Market continuation authority after current Core attests the affine span.
///
/// The contained generic witness is the sole promotion boundary for the
/// pre-Core routing hint. Constructing this value neither executes a child nor
/// grants write authority; the common outer must still reauthenticate the
/// exact global artifact and execute only routes `[2, 5)` commit-last.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeContinuationBundleV4<'a> {
    /// Exact globally admitted Series artifact bundle.
    pub artifacts: SeriesConsumeArtifactBundleV4<'a>,
    /// Current-Core attestation of F and the ordered FundingState-list identity.
    pub found_span: AuthenticatedFoundSpanV2,
}

/// Join one already-admitted global plan to the current Core Found attestation.
///
/// This does not infer F from account count or the compact projected header.
/// `AuthenticatedFoundSpanV2` can originate only from the generic Core-return
/// verifier that binds the exact Core request, permit, FundingState list, and
/// post-resource digest.
pub fn authenticate_series_consume_continuation_v4<'a>(
    artifacts: SeriesConsumeArtifactBundleV4<'a>,
    found_span: AuthenticatedFoundSpanV2,
) -> Result<SeriesConsumeContinuationBundleV4<'a>> {
    if u16::from(found_span.funding_count()) != artifacts.effect.funding_count_hint() {
        return Err(SeriesArtifactErrorV4::FoundAcknowledgement);
    }
    artifacts
        .effect
        .require_window(SeriesConsumeRouteWindowV4::LiveMarketContinuation)
        .map_err(|_| SeriesArtifactErrorV4::Effect)?;
    Ok(SeriesConsumeContinuationBundleV4 {
        artifacts,
        found_span,
    })
}

/// Authenticate and join one complete schema-bound Series Consume bundle.
pub fn authenticate_series_consume_artifacts_v4<'a>(
    selection: SeriesConsumeArtifactSelectionV4,
    artifacts: SeriesConsumeArtifactBytesV4<'a>,
    family_request: &'a [u8],
    registers: SeriesConsumeArtifactRegistersV4<'_>,
) -> Result<SeriesConsumeArtifactBundleV4<'a>> {
    require_selected(selection.program_set, artifacts.program_set)?;
    let set = CapabilityProgramSetV2::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| SeriesArtifactErrorV4::ProgramSet)?;
    if set.selector_offset() != SERIES_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
    {
        return Err(SeriesArtifactErrorV4::ProgramSet);
    }

    let request = SeriesActionRequestV3::decode(family_request)
        .map_err(|_| SeriesArtifactErrorV4::Request)?;
    if request.action() != SeriesActionV3::Consume || request.template() != selection.template {
        return Err(SeriesArtifactErrorV4::Request);
    }
    let slices = split_request(request, family_request)?;
    let selected_descriptor = set
        .select_descriptor(slices.header)
        .map_err(|_| SeriesArtifactErrorV4::ProgramSet)?;
    if selected_descriptor.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4
        || selected_descriptor.program().to_bytes() != digest(artifacts.descriptor)
    {
        return Err(SeriesArtifactErrorV4::ContentIdentity);
    }
    let descriptor = CapabilityProgramV4::decode(artifacts.descriptor)
        .map_err(|_| SeriesArtifactErrorV4::Descriptor)?;
    validate_descriptor(descriptor)?;

    require_artifact(
        descriptor.account_profile(),
        ACCOUNT_PROFILE_SCHEMA_ID_V2,
        artifacts.account_profile,
        SeriesArtifactErrorV4::AccountProfile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| SeriesArtifactErrorV4::AccountProfile)?;

    require_artifact(
        descriptor.lifecycle(),
        LIFECYCLE_SCHEMA_ID_V4,
        artifacts.lifecycle_policy,
        SeriesArtifactErrorV4::Lifecycle,
    )?;
    let lifecycle_policy = StateLifecyclePolicyV4::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        digest(artifacts.lifecycle_policy),
        artifacts.lifecycle_policy,
    )
    .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?;
    lifecycle_policy
        .validate_account_profile(account_profile)
        .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?;
    if lifecycle_policy
        .action_plan_count(request.action() as u32)
        .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?
        == 0
    {
        return Err(SeriesArtifactErrorV4::Lifecycle);
    }

    require_artifact(
        descriptor.request_profile(),
        dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
        artifacts.request_profile,
        SeriesArtifactErrorV4::RequestProfile,
    )?;
    let request_profile = RequestProfileV1::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        digest(artifacts.request_profile),
        artifacts.request_profile,
    )
    .map_err(|_| SeriesArtifactErrorV4::RequestProfile)?;
    validate_and_execute_header(request_profile, slices.header)?;

    require_artifact(
        descriptor.strategy(),
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        artifacts.strategy,
        SeriesArtifactErrorV4::Strategy,
    )?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| SeriesArtifactErrorV4::Strategy)?;
    if strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
    {
        return Err(SeriesArtifactErrorV4::Strategy);
    }

    require_artifact(
        descriptor.transition(),
        dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
        artifacts.transition,
        SeriesArtifactErrorV4::Transition,
    )?;
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| SeriesArtifactErrorV4::Transition)?;

    require_artifact(
        descriptor.effect(),
        dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4,
        artifacts.effect,
        SeriesArtifactErrorV4::Effect,
    )?;
    let effect = SeriesConsumeEffectV4::decode(
        artifacts.effect,
        family_request,
        registers.tail_count,
        registers.scalars,
        registers.identities,
        registers.funding_count_hint,
    )
    .map_err(|_| SeriesArtifactErrorV4::Effect)?;

    validate_geometry(
        account_profile,
        request_profile,
        transition,
        effect,
        registers,
    )?;
    Ok(SeriesConsumeArtifactBundleV4 {
        request,
        slices,
        descriptor,
        account_profile,
        lifecycle_policy,
        request_profile,
        strategy,
        transition,
        effect,
    })
}

fn validate_descriptor(descriptor: CapabilityProgramV4) -> Result<()> {
    if descriptor.kind().to_bytes() != digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)
        || descriptor.config_schema().to_bytes() != SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        || descriptor.request_schema().to_bytes() != digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)
        || descriptor.root_schema().to_bytes() != digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)
        || descriptor.derivation_policy().to_bytes() != digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| SeriesArtifactErrorV4::Geometry)?
            != SERIES_STATE_BYTES_V3
        || descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_ID_V2
        || descriptor.request_profile().schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.lifecycle().schema().to_bytes() != LIFECYCLE_SCHEMA_ID_V4
        || descriptor.strategy().schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition().schema().to_bytes()
            != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || descriptor.effect().schema().to_bytes()
            != dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4
    {
        return Err(SeriesArtifactErrorV4::Descriptor);
    }
    Ok(())
}

fn validate_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: SeriesConsumeEffectV4<'_>,
    registers: SeriesConsumeArtifactRegistersV4<'_>,
) -> Result<()> {
    let base = effect.program().base();
    let common_scalars = account.common_scalar_count();
    let common_identities = account.common_identity_count();
    if account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.fixed_account_count() != SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4
        || account.item_account_stride() != 1
        || account.item_scalar_stride() != 0
        || account.item_identity_stride() != 0
        || request.common_scalar_count() != common_scalars
        || request.common_identity_count() != common_identities
        || transition.common_scalar_count() != common_scalars
        || transition.common_identity_count() != common_identities
        || transition.item_scalar_stride() != 0
        || transition.item_identity_stride() != 0
        || base.fixed_account_count() != SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4
        || base.common_scalar_count() != common_scalars
        || base.common_identity_count() != common_identities
        || base.item_account_stride() != 0
        || base.item_scalar_stride() != 0
        || base.item_identity_stride() != 0
        || base.item_operation_count() != 0
    {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    validate_dynamic_account_span(account, effect, registers)?;
    Ok(())
}

fn validate_dynamic_account_span(
    account: AccountProfileV2<'_>,
    effect: SeriesConsumeEffectV4<'_>,
    registers: SeriesConsumeArtifactRegistersV4<'_>,
) -> Result<()> {
    if account.dynamic_fixed_span_count() != 1
        || account.trusted_current_slot_scalar().is_some()
        || account.trusted_current_executing_program_identity() != Some(0)
        || account.trusted_system_program_identity().is_some()
        || SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4
            .checked_add(SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4)
            != Some(SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4)
    {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    let span = account
        .dynamic_fixed_span(0)
        .map_err(|_| SeriesArtifactErrorV4::Geometry)?;
    if span.insertion_coordinate() != SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4
        || span.count_scalar() != SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4
        || span.rule_start() != 0
        || span.rule_stride() != 1
        || span.minimum() != 1
        || span.maximum() != u32::from(SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3)
        || span.step() != 1
    {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    let mut span_counts = [0_u32; 1];
    account
        .dynamic_span_widths_from_scalars(registers.scalars, &mut span_counts)
        .map_err(|_| SeriesArtifactErrorV4::Geometry)?;
    if span_counts[0] != u32::from(registers.funding_count_hint)
        || account
            .logical_account_count_with_dynamic_spans(registers.tail_count, &span_counts)
            .map_err(|_| SeriesArtifactErrorV4::Geometry)?
            != effect
                .program()
                .account_count(registers.tail_count, registers.scalars)
                .map_err(|_| SeriesArtifactErrorV4::Geometry)?
    {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    Ok(())
}

fn validate_and_execute_header(profile: RequestProfileV1<'_>, header: &[u8]) -> Result<()> {
    if profile
        .request_bytes(0)
        .map_err(|_| SeriesArtifactErrorV4::RequestProfile)?
        != SERIES_ACTION_HEADER_BYTES_V3
        || profile.item_request_bytes() != 0
        || profile.item_scalar_stride() != 0
        || profile.item_identity_stride() != 0
    {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    let scalars = usize::from(profile.common_scalar_count());
    let identities = usize::from(profile.common_identity_count());
    if scalars > MAX_SCALARS || identities > MAX_IDENTITIES {
        return Err(SeriesArtifactErrorV4::Geometry);
    }
    let input_scalars = [0_u64; MAX_SCALARS];
    let input_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut scratch_scalars = [0_u64; MAX_SCALARS];
    let mut scratch_identities = [[0_u8; 32]; MAX_IDENTITIES];
    let mut output_scalars = [0_u64; MAX_SCALARS];
    let mut output_identities = [[0_u8; 32]; MAX_IDENTITIES];
    project_atomic(
        profile,
        0,
        header,
        ProjectionRegistersV1 {
            input_scalars: input_scalars
                .get(..scalars)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
            input_identities: input_identities
                .get(..identities)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
            scratch_scalars: scratch_scalars
                .get_mut(..scalars)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
            scratch_identities: scratch_identities
                .get_mut(..identities)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
            output_scalars: output_scalars
                .get_mut(..scalars)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
            output_identities: output_identities
                .get_mut(..identities)
                .ok_or(SeriesArtifactErrorV4::Geometry)?,
        },
    )
    .map_err(|_| SeriesArtifactErrorV4::RequestProfile)
}

fn split_request<'a>(
    request: SeriesActionRequestV3<'_>,
    bytes: &'a [u8],
) -> Result<SeriesRequestSlicesV3<'a>> {
    let header = bytes
        .get(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesArtifactErrorV4::Request)?;
    let witness = bytes
        .get(SERIES_ACTION_HEADER_BYTES_V3..)
        .ok_or(SeriesArtifactErrorV4::Request)?;
    let expected = usize::from(request.proof_count())
        .checked_mul(SERIES_WITNESS_ITEM_BYTES_V3)
        .ok_or(SeriesArtifactErrorV4::Request)?;
    if witness.len() != expected || witness != request.proof_bytes() {
        return Err(SeriesArtifactErrorV4::Request);
    }
    Ok(SeriesRequestSlicesV3 { header, witness })
}

fn require_artifact(
    selected: ArtifactReferenceV4,
    expected_schema: [u8; 32],
    bytes: &[u8],
    error: SeriesArtifactErrorV4,
) -> Result<()> {
    if selected.schema().to_bytes() != expected_schema
        || selected.program().to_bytes() != digest(bytes)
    {
        Err(error)
    } else {
        Ok(())
    }
}

fn require_selected(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    if selected == [0; 32] || selected != digest(bytes) {
        Err(SeriesArtifactErrorV4::ContentIdentity)
    } else {
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;
    use dclutch_capability_program_contract::v4::CapabilityArtifactsV4;

    use super::*;
    use crate::series::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    };

    fn id(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero identity")
    }

    fn byte_id(byte: u8) -> ContentId {
        id([byte; 32])
    }

    fn reference(schema: [u8; 32], program: u8) -> ArtifactReferenceV4 {
        ArtifactReferenceV4::new(id(schema), byte_id(program))
    }

    fn descriptor(effect_schema: [u8; 32]) -> CapabilityProgramV4 {
        CapabilityProgramV4::new(
            id(digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)),
            id(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3),
            id(digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3)),
            id(digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)),
            id(digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)),
            byte_id(9),
            CapabilityArtifactsV4 {
                account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V2, 10),
                request_profile: reference(dclutch_request_profile_contract::SCHEMA_RELEASE_ID, 11),
                lifecycle: reference(LIFECYCLE_SCHEMA_ID_V4, 12),
                strategy: reference(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, 13),
                transition: reference(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID, 14),
                effect: reference(effect_schema, 15),
            },
            SERIES_STATE_BYTES_V3 as u32,
        )
        .expect("descriptor")
    }

    #[test]
    fn descriptor_requires_every_successor_schema() {
        assert_eq!(
            validate_descriptor(descriptor(dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4)),
            Ok(())
        );
        assert_eq!(
            validate_descriptor(descriptor(dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID)),
            Err(SeriesArtifactErrorV4::Descriptor)
        );
    }

    #[test]
    fn artifact_reference_rejects_schema_and_content_substitution() {
        let bytes = b"exact artifact";
        let exact = ArtifactReferenceV4::new(byte_id(21), id(digest(bytes)));
        assert_eq!(
            require_artifact(exact, [21; 32], bytes, SeriesArtifactErrorV4::Effect),
            Ok(())
        );
        assert_eq!(
            require_artifact(exact, [22; 32], bytes, SeriesArtifactErrorV4::Effect),
            Err(SeriesArtifactErrorV4::Effect)
        );
        assert_eq!(
            require_artifact(
                exact,
                [21; 32],
                b"substitution",
                SeriesArtifactErrorV4::Effect
            ),
            Err(SeriesArtifactErrorV4::Effect)
        );
    }

    #[test]
    fn dynamic_account_profile_geometry_is_exact() {
        let lengths = [0_u32; SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4 as usize];
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut bytes = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &lengths,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("Series Profile13");
        let exact = AccountProfileV2::decode(&bytes).expect("decode Profile13");
        let scalars = [128, 64, 2, 32, 7];
        let registers = SeriesConsumeArtifactRegistersV4 {
            tail_count: 258,
            scalars: &scalars,
            identities: &[[9_u8; 32]],
            funding_count_hint: 7,
        };
        let effect_program = crate::series::effect_v4::tests::successor();
        let request = crate::series::effect_v4::tests::request();
        let effect = SeriesConsumeEffectV4::decode(
            &effect_program,
            &request,
            registers.tail_count,
            registers.scalars,
            registers.identities,
            registers.funding_count_hint,
        )
        .expect("Series DCE5");
        assert_eq!(
            validate_dynamic_account_span(exact, effect, registers),
            Ok(())
        );

        let insertion = dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_HEADER_BYTES;
        bytes[insertion..insertion + 2].copy_from_slice(&62_u16.to_le_bytes());
        let substituted = AccountProfileV2::decode(&bytes).expect("valid substituted Profile13");
        assert_eq!(
            validate_dynamic_account_span(substituted, effect, registers),
            Err(SeriesArtifactErrorV4::Geometry)
        );
    }
}
