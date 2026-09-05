//! Schema-bound artifact admission for the global Series Consume plan.
//!
//! `CapabilityProgramSetV2` selects one exact `CapabilityProgramV4` schema and
//! content pair from the authenticated action byte.  The descriptor then binds
//! every independently finalized interpreter artifact, including lifecycle and
//! the DCE5 Effect successor.  This module grants no account, receipt, scalar,
//! CPI, or state-write authority; the projected outer and common Hot executor
//! retain those physical responsibilities.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_program::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::{
        ArtifactReferenceV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_market::execution_strategy::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2,
};
use dclutch_trading::series::{
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_vm::account_profile::{
    lifecycle_v3::{
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5, StateLifecyclePolicyV5,
    },
    v2::{
        AccountProfileV2, AliasKindV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
        SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2,
    },
};
use dclutch_vm::request_profile::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_vm::{MAX_IDENTITIES, MAX_SCALARS, v3::ProgramV3 as TransitionProgramV3};
use solana_program::hash::hash;

use crate::projected_market_v2::AuthenticatedFoundSpanV2;

use super::{
    account_profile_v4::SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4,
    artifacts_v3::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ACTION_SELECTOR_OFFSET_V3,
        SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
        SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TICKET_DERIVATION_PREIMAGE_V3,
        SERIES_WITNESS_ITEM_BYTES_V3, SeriesRequestSlicesV3,
    },
    effect_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4, SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4,
        SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4, SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4,
        SeriesConsumeEffectV4,
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
    /// A lifecycle plan claimed the Ticket replay coordinate.
    ///
    /// The Ticket's lamport flow has exactly one author — the funding path
    /// (`super::commit_plans::PendingFundingPlanV3::ticket_capability_refund`).
    /// A policy whose plan names the Ticket as its state, payer, or RentCredit
    /// — directly or through a route alias — would be a second author for that
    /// flow, so it is refused here as its own named wall rather than folded
    /// into the generic lifecycle refusal.
    TicketAuthorship,
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
    pub lifecycle_policy: StateLifecyclePolicyV5<'a>,
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
        LIFECYCLE_SCHEMA_ID_V5,
        artifacts.lifecycle_policy,
        SeriesArtifactErrorV4::Lifecycle,
    )?;
    let lifecycle_policy = StateLifecyclePolicyV5::decode_selected(
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
    require_root_only_series_lifecycle(lifecycle_policy, account_profile)?;

    require_artifact(
        descriptor.request_profile(),
        dclutch_vm::request_profile::SCHEMA_RELEASE_ID,
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
        dclutch_vm::v3::SCHEMA_RELEASE_ID,
        artifacts.transition,
        SeriesArtifactErrorV4::Transition,
    )?;
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| SeriesArtifactErrorV4::Transition)?;

    require_artifact(
        descriptor.effect(),
        dclutch_vm::effect::v4::SCHEMA_RELEASE_ID_V4,
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

/// Refuse any lifecycle plan that claims the Ticket replay coordinate.
///
/// The 1b8228e9 ruling: the policy covers the states Series routes create and
/// own — the root — while the Ticket appears in the Consume frame as a
/// referenced coordinate only, its lamport flow authored solely by the
/// funding path. This wall makes a second author *refused*, not merely
/// unwritten: every plan of every declared Series action is walked, and a
/// state, payer, or RentCredit coordinate that is the Ticket — or a route
/// alias resolving to it — refuses with its own code.
///
/// The walk runs after `validate_account_profile` has already joined the
/// policy to this exact profile, so item-scoped plans (impossible against a
/// zero-stride profile) and out-of-range coordinates are refused before this
/// is reached; the per-plan re-join inside `project_account_indices` is one
/// extra pass over a policy this small.
fn require_root_only_series_lifecycle(
    policy: StateLifecyclePolicyV5<'_>,
    profile: AccountProfileV2<'_>,
) -> Result<()> {
    let is_ticket = |index: usize| -> Result<bool> {
        if index == usize::from(SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4) {
            return Ok(true);
        }
        let coordinate =
            u16::try_from(index).map_err(|_| SeriesArtifactErrorV4::TicketAuthorship)?;
        let rule = profile
            .rule(false, coordinate)
            .map_err(|_| SeriesArtifactErrorV4::TicketAuthorship)?;
        Ok(rule.alias_kind() == AliasKindV2::Fixed
            && rule.alias_index() == SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4)
    };
    for action in [
        SeriesActionV3::Prepare,
        SeriesActionV3::Consume,
        SeriesActionV3::Expire,
        SeriesActionV3::Retire,
        SeriesActionV3::Close,
    ] {
        let plans = policy
            .action_plan_count(action as u32)
            .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?;
        let mut ordinal = 0_u16;
        while ordinal < plans {
            let selected = policy
                .action_plan(action as u32, ordinal)
                .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?;
            let indices = selected
                .project_account_indices(profile, 0, None)
                .map_err(|_| SeriesArtifactErrorV4::Lifecycle)?;
            for coordinate in [
                Some(indices.state()),
                indices.payer(),
                indices.rent_credit(),
            ]
            .into_iter()
            .flatten()
            {
                if is_ticket(coordinate)? {
                    return Err(SeriesArtifactErrorV4::TicketAuthorship);
                }
            }
            ordinal = ordinal
                .checked_add(1)
                .ok_or(SeriesArtifactErrorV4::Lifecycle)?;
        }
    }
    Ok(())
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
            != dclutch_vm::request_profile::SCHEMA_RELEASE_ID
        || descriptor.lifecycle().schema().to_bytes() != LIFECYCLE_SCHEMA_ID_V5
        || descriptor.strategy().schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition().schema().to_bytes() != dclutch_vm::v3::SCHEMA_RELEASE_ID
        || descriptor.effect().schema().to_bytes() != dclutch_vm::effect::v4::SCHEMA_RELEASE_ID_V4
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
pub(super) mod tests {
    extern crate alloc;

    use alloc::vec;
    use dclutch_market::capability_program::v4::CapabilityArtifactsV4;

    use super::*;
    use crate::series::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    };
    use crate::series::consume_artifacts_v4::{
        SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4, SERIES_CONSUME_COMMON_SCALAR_COUNT_V4,
    };

    /// Exact common scalar bank width the Consume artifacts declare.
    pub(crate) const CONSUME_SCALARS: usize = SERIES_CONSUME_COMMON_SCALAR_COUNT_V4 as usize;
    /// Exact common identity bank width the Consume artifacts declare.
    pub(crate) const CONSUME_IDENTITIES: usize = SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4 as usize;

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
                request_profile: reference(dclutch_vm::request_profile::SCHEMA_RELEASE_ID, 11),
                lifecycle: reference(LIFECYCLE_SCHEMA_ID_V5, 12),
                strategy: reference(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, 13),
                transition: reference(dclutch_vm::v3::SCHEMA_RELEASE_ID, 14),
                effect: reference(effect_schema, 15),
            },
            u32::try_from(SERIES_STATE_BYTES_V3).expect("Series state size fits in u32"),
        )
        .expect("descriptor")
    }

    #[test]
    fn descriptor_requires_every_successor_schema() {
        assert_eq!(
            validate_descriptor(descriptor(dclutch_vm::effect::v4::SCHEMA_RELEASE_ID_V4)),
            Ok(())
        );
        assert_eq!(
            validate_descriptor(descriptor(dclutch_vm::effect::v3::SCHEMA_RELEASE_ID)),
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

    /// Encode one hostile V5 policy whose `action` plan names `state` and,
    /// optionally, a RentCredit coordinate. Everything else is minimal and
    /// well-formed, so the only wall left to refuse it is the Ticket pin —
    /// or, for a non-Consume action, the nonzero-Consume-plan conjunct.
    pub(crate) fn hostile_policy(
        action: SeriesActionV3,
        state: u16,
        rent_credit: Option<u16>,
    ) -> alloc::vec::Vec<u8> {
        use dclutch_vm::account_profile::lifecycle_v3::{
            ACTION_PLAN_BYTES, HEADER_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
            encode::{
                LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
                LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
                LifecycleRegisterCoordinateV3, LifecycleSeedInputV3,
                encode_lifecycle_policy_v5_atomic,
            },
        };

        let recipes = [LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(state),
            seed_start: 0,
            seed_count: 2,
            bump_offset: 1,
            data_base: 64,
            data_stride: 0,
        }];
        let seeds = [
            LifecycleSeedInputV3::Literal(b"hostile-second-author"),
            LifecycleSeedInputV3::CanonicalBump,
        ];
        // A refund claim needs a Close plan: the wire format itself refuses a
        // RentCredit on an Authenticate plan, so the claiming shape is "close
        // the state to the Ticket".
        let plans = [match rent_credit {
            None => LifecyclePlanInputV3 {
                action: action as u32,
                operation: LifecycleOperationInputV3::Authenticate,
                recipe: 0,
                payer: None,
                rent_credit: None,
                principal: None,
                beneficiary: None,
                refund_source: LifecycleRefundSourceInputV3::Credit,
                guard: LifecycleGuardInputV3::Always,
            },
            Some(credit) => LifecyclePlanInputV3 {
                action: action as u32,
                operation: LifecycleOperationInputV3::Close,
                recipe: 0,
                payer: None,
                rent_credit: Some(LifecycleAccountCoordinateV3::fixed(credit)),
                principal: Some(LifecycleRegisterCoordinateV3::common(0)),
                beneficiary: Some(LifecycleRegisterCoordinateV3::common(0)),
                refund_source: LifecycleRefundSourceInputV3::Credit,
                guard: LifecycleGuardInputV3::Always,
            },
        }];
        let width = HEADER_BYTES
            + RECIPE_BYTES
            + 2 * SEED_BYTES
            + ACTION_PLAN_BYTES
            + PROTECTED_OUTPUT_BYTES;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_lifecycle_policy_v5_atomic(
            &recipes,
            &seeds,
            &plans,
            &[None],
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("hostile policy encodes");
        output
    }

    fn decoded_profile() -> alloc::vec::Vec<u8> {
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
        bytes
    }

    /// A policy claiming the Ticket refuses at its own named wall — as the
    /// plan's state and through the route alias — while the canonical
    /// root-only policy passes the same wall.
    ///
    /// The refusal CODE is asserted, not just failure: a hostile that only
    /// proved "something refused" would stay green if the policy died at the
    /// generic lifecycle join instead of at the second-author pin.
    ///
    /// **The RentCredit shape is no longer reachable against this profile,
    /// and the third assertion below is what measures that rather than
    /// assuming it.** `validate_plan_permissions` (added to the generic
    /// profile join in `73ffb010`) requires a `Close` plan's state to carry
    /// `DEBIT_LAMPORTS | WRITE_DATA` and its RentCredit to carry
    /// `CREDIT_LAMPORTS`; `Create`/`AuthenticateOrCreate` want the mirrored
    /// set. The Series Consume profile grants `WRITE_DATA` to the root and the
    /// Ticket and nothing at all to every other coordinate
    /// (`account_profile_v4::fixed_rule`), so **no** Consume policy can carry
    /// a Create or Close plan, and the wire format already refuses `payer` and
    /// `rent_credit` on an `Authenticate` plan. Only the state coordinate can
    /// still name the Ticket. The pin's payer and RentCredit arms are
    /// therefore defense in depth against a profile that does grant a lamport
    /// permission; they are kept, and the control below records that today
    /// they are shadowed by the generic wall rather than exercised by it.
    #[test]
    fn a_ticket_claiming_policy_refuses_at_the_ticket_authorship_wall() {
        let profile_bytes = decoded_profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let wall = |bytes: &[u8]| {
            let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], bytes)
                .expect("hostile policy decodes: the pin, not the decoder, must refuse it");
            require_root_only_series_lifecycle(policy, profile)
        };
        // Claimed as the plan's own state.
        assert_eq!(
            wall(&hostile_policy(SeriesActionV3::Consume, 59, None)),
            Err(SeriesArtifactErrorV4::TicketAuthorship)
        );
        // Claimed as the refund destination of a root plan: refused, but by
        // the generic permission wall ahead of the pin. The two arms of this
        // assertion are what keep it honest — the Ticket claim and a claim
        // naming a coordinate the pin does not care about refuse IDENTICALLY,
        // which is the measurement that this shape no longer discriminates.
        // If the Consume profile ever grants a lamport permission, the first
        // arm becomes `TicketAuthorship` and this assertion fails loudly.
        assert_eq!(
            wall(&hostile_policy(SeriesActionV3::Consume, 0, Some(59))),
            Err(SeriesArtifactErrorV4::Lifecycle)
        );
        assert_eq!(
            wall(&hostile_policy(SeriesActionV3::Consume, 0, Some(1))),
            Err(SeriesArtifactErrorV4::Lifecycle)
        );
        // Claimed through the authenticated route alias (140 -> 59).
        assert_eq!(
            wall(&hostile_policy(SeriesActionV3::Consume, 140, None)),
            Err(SeriesArtifactErrorV4::TicketAuthorship)
        );
        // Control: the canonical root-only policy passes this exact wall.
        let mut scratch =
            vec![0_u8; crate::series::lifecycle_policy_v5::SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        let mut canonical =
            vec![0_u8; crate::series::lifecycle_policy_v5::SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        crate::series::lifecycle_policy_v5::encode_series_consume_state_lifecycle_v5_atomic(
            &mut scratch,
            &mut canonical,
        )
        .expect("canonical policy");
        assert_eq!(wall(&canonical), Ok(()));
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
        // Both banks are typed by the emitter's own width constants, so a
        // future widening is a compile error here rather than a runtime
        // `Successor` refusal from `validate_request_coverage`.
        let scalars: [u64; CONSUME_SCALARS] = [128, 64, 2, 32, 7, 9, 4];
        let identities = [[9_u8; 32]; CONSUME_IDENTITIES];
        let registers = SeriesConsumeArtifactRegistersV4 {
            tail_count: 258,
            scalars: &scalars,
            identities: &identities,
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

        let insertion = dclutch_vm::account_profile::v2::DYNAMIC_FIXED_SPAN_HEADER_BYTES;
        bytes
            .get_mut(insertion..insertion + 2)
            .expect("dynamic span header offset in bounds")
            .copy_from_slice(&62_u16.to_le_bytes());
        let substituted = AccountProfileV2::decode(&bytes).expect("valid substituted Profile13");
        assert_eq!(
            validate_dynamic_account_span(substituted, effect, registers),
            Err(SeriesArtifactErrorV4::Geometry)
        );
    }
}
