//! Canonical five-action Series successor release and selected authenticator.
//!
//! One ProgramSetV2 binds the complete Prepare, Consume, Expire, Retire, and
//! Close descriptor bank. Every supplied artifact is hostile-decoded under its
//! current schema, cross-joined on register/account geometry, digested here,
//! and recompiled byte-for-byte before selection is returned to an operator.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    lifecycle_v3::{CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, StateLifecyclePolicyV5},
    v3::{
        AccountProfileV3, HEADER_BYTES_V3 as ACCOUNT_PROFILE_HEADER_BYTES_V3,
        SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_ID_V3, encode_account_profile_v3_atomic,
    },
};
use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4,
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{ProjectedCustodyOperationV1, ProjectedCustodyRequestV1};
use dclutch_effect_kernel::v5::{
    FundingOperationV5, HEADER_BYTES_V5 as EFFECT_HEADER_BYTES_V5, ProgramV5 as EffectProgramV5,
    encode_program_v5_atomic,
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market_core_codec::{SeriesCoreActionV1, SeriesCoreRequestV1};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_series_v3_kernel::{
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use solana_program::hash::hash;

use super::{
    account_profile_v4::SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4,
    account_profile_v4::{
        SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4, SeriesConsumeAccountProfileInputV4,
        stamp_series_release_owned_widths_v4,
    },
    artifacts_v3::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ACTION_SELECTOR_OFFSET_V3,
        SERIES_ROOT_SCHEMA_PREIMAGE_V3, SERIES_SUCCESSOR_KIND_PREIMAGE_V3,
        SERIES_TICKET_DERIVATION_PREIMAGE_V3,
    },
    consume_artifacts_v4::{
        SERIES_CONSUME_BASE_EFFECT_BYTES_V4, SERIES_CONSUME_EFFECT_BYTES_V4,
        SeriesConsumeChildRequestsV4, encode_series_consume_effect_v4_from_requests_atomic,
    },
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5, SERIES_EXPIRE_TICKET_COORDINATE_V5,
        SeriesExpireAccountProfileInputV5, SeriesExpireChildRequestsV5,
        emit_series_expire_funding_artifacts_v5,
    },
    funding_artifacts_v5::{
        SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5, emit_series_close_funding_artifacts_v5,
    },
    lifecycle_policy_v5::{
        SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5, SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
        SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5, encode_series_empty_state_lifecycle_v5_atomic,
    },
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_PAYER_COORDINATE_V5, SERIES_PREPARE_REFUND_COORDINATE_V5,
        SERIES_PREPARE_SYSTEM_COORDINATE_V5, SERIES_PREPARE_TICKET_COORDINATE_V5,
        SeriesPrepareAccountProfileInputV5, emit_series_prepare_funding_artifacts_v5,
    },
    release_v4::{emit_series_consume_artifacts_v4, encode_series_consume_strategy_v4},
    retire_funding_artifacts_v5::{
        SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5, SERIES_RETIRE_TICKET_COORDINATE_V5,
        emit_series_retire_funding_artifacts_v5,
    },
    state::{SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3},
};

/// Exact action count in the canonical successor set.
pub const SERIES_RELEASE_ACTION_COUNT_V5: usize = 5;

/// Complete bytes for one action-selected artifact tuple.
#[derive(Clone, Copy, Debug)]
pub struct SeriesActionArtifactSourceV5<'a> {
    /// Current AccountProfileV3 bytes.
    account_profile: &'a [u8],
    /// Current RequestProfileV1 bytes.
    request_profile: &'a [u8],
    /// Current StateLifecyclePolicyV5 bytes.
    lifecycle: &'a [u8],
    /// Current TransitionV3 bytes.
    transition: &'a [u8],
    /// Current EffectV5 bytes.
    effect: &'a [u8],
    /// Exact dynamic fixed-span counts; empty for fixed profiles.
    dynamic_fixed_span_counts: &'a [u32],
    /// Exact semantic authority decoded by the action-specific emitter seam.
    authority: SeriesOccurrenceAuthorityV5,
}

/// Complete current-source five-action release input.
#[derive(Clone, Copy, Debug)]
pub struct SeriesReleaseSourceV5<'a> {
    /// Finalized Series Template content identity.
    template: ContentId,
    /// Deployed Consume-only Shadow certificate program.
    consume_shadow_certificate_program: ContentId,
    /// Artifacts ordered exactly Prepare, Consume, Expire, Retire, Close.
    actions: [SeriesActionArtifactSourceV5<'a>; SERIES_RELEASE_ACTION_COUNT_V5],
}

/// Inputs owned by the five current action-specific emitters.
#[derive(Clone, Copy, Debug)]
pub struct SeriesCurrentReleaseInputV5<'a> {
    /// Finalized Series Template identity.
    pub template: ContentId,
    /// Deployed Consume-only Shadow certificate program identity.
    pub consume_shadow_certificate_program: ContentId,
    /// Prepare fixed prestate widths.
    pub prepare_profile: SeriesPrepareAccountProfileInputV5<'a>,
    /// Prepare child request bank.
    pub prepare_requests: SeriesPrepareChildRequestsV4<'a>,
    /// Current exact Ticket rent target.
    pub prepare_ticket_rent_lamports: u64,
    /// Consume fixed base widths before its FundingState span.
    pub consume_observed_data_lengths: &'a [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    /// Consume child request bank.
    pub consume_requests: SeriesConsumeChildRequestsV4<'a>,
    /// Exact nonzero Consume FundingState span count.
    pub consume_funding_count: u32,
    /// Expire fixed prestate widths.
    pub expire_profile: SeriesExpireAccountProfileInputV5<'a>,
    /// Expire child request bank.
    pub expire_requests: SeriesExpireChildRequestsV5<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SeriesOwnedActionArtifactsV5 {
    account_profile: Vec<u8>,
    request_profile: Vec<u8>,
    lifecycle: Vec<u8>,
    transition: Vec<u8>,
    effect: Vec<u8>,
    dynamic_fixed_span_counts: Vec<u32>,
    authority: SeriesOccurrenceAuthorityV5,
}

/// Opaque owned five-action bank emitted only by current semantic owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesOwnedReleaseSourceV5 {
    template: ContentId,
    consume_shadow_certificate_program: ContentId,
    actions: [SeriesOwnedActionArtifactsV5; SERIES_RELEASE_ACTION_COUNT_V5],
}

impl SeriesOwnedReleaseSourceV5 {
    /// Borrow the exact current artifact bank for compile/authentication.
    pub fn as_source(&self) -> SeriesReleaseSourceV5<'_> {
        SeriesReleaseSourceV5 {
            template: self.template,
            consume_shadow_certificate_program: self.consume_shadow_certificate_program,
            actions: core::array::from_fn(|index| SeriesActionArtifactSourceV5 {
                account_profile: &self.actions[index].account_profile,
                request_profile: &self.actions[index].request_profile,
                lifecycle: &self.actions[index].lifecycle,
                transition: &self.actions[index].transition,
                effect: &self.actions[index].effect,
                dynamic_fixed_span_counts: &self.actions[index].dynamic_fixed_span_counts,
                authority: self.actions[index].authority,
            }),
        }
    }

    /// Borrow one exact action body for Registry publication without exposing
    /// any constructor that could replace the current semantic owner.
    pub fn action_artifacts(&self, action: SeriesActionV3) -> SeriesActionArtifactViewV5<'_> {
        SeriesActionArtifactViewV5 {
            action: &self.actions[action as usize],
        }
    }
}

/// Read-only publication view over one current action's complete artifact body.
#[derive(Clone, Copy, Debug)]
pub struct SeriesActionArtifactViewV5<'a> {
    action: &'a SeriesOwnedActionArtifactsV5,
}

impl<'a> SeriesActionArtifactViewV5<'a> {
    /// Current AccountProfileV3 body.
    pub fn account_profile(self) -> &'a [u8] {
        self.action.account_profile.as_slice()
    }

    /// Current RequestProfileV1 body.
    pub fn request_profile(self) -> &'a [u8] {
        self.action.request_profile.as_slice()
    }

    /// Current StateLifecyclePolicyV5 body.
    pub fn lifecycle(self) -> &'a [u8] {
        self.action.lifecycle.as_slice()
    }

    /// Current TransitionV3 body.
    pub fn transition(self) -> &'a [u8] {
        self.action.transition.as_slice()
    }

    /// Current EffectV5 body.
    pub fn effect(self) -> &'a [u8] {
        self.action.effect.as_slice()
    }

    /// Exact dynamic fixed-span counts; empty for fixed profiles.
    pub fn dynamic_fixed_span_counts(self) -> &'a [u32] {
        self.action.dynamic_fixed_span_counts.as_slice()
    }
}

/// Exact occurrence authority authored by the selected action's typed child
/// requests. Terminal actions intentionally carry no occurrence authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesOccurrenceAuthorityV5 {
    /// Prepare commits a future Market before a founding permit exists.
    Prepare {
        /// Future Market identity.
        market: [u8; 32],
        /// Nonzero future Market generation.
        generation: u64,
        /// Selected Registry release set.
        release_set: [u8; 32],
        /// Parent Trading capability root.
        parent_root: [u8; 32],
    },
    /// Consume commits the same future Market before Core returns its permit.
    Consume {
        /// Future Market identity.
        market: [u8; 32],
        /// Nonzero future Market generation.
        generation: u64,
        /// Selected Registry release set.
        release_set: [u8; 32],
        /// Parent Trading capability root.
        parent_root: [u8; 32],
    },
    /// Expire commits the future Market selected by its authenticated request.
    Expire {
        /// Future Market identity.
        market: [u8; 32],
        /// Nonzero future Market generation.
        generation: u64,
        /// Selected Registry release set.
        release_set: [u8; 32],
        /// Parent Trading capability root.
        parent_root: [u8; 32],
    },
    /// Retire and Close have no occurrence child authority.
    Terminal,
}

/// One action's descriptor-bound artifact identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesActionArtifactIdsV5 {
    /// AccountProfile identity.
    pub account_profile: [u8; 32],
    /// RequestProfile identity.
    pub request_profile: [u8; 32],
    /// Lifecycle identity.
    pub lifecycle: [u8; 32],
    /// Strategy identity.
    pub strategy: [u8; 32],
    /// Transition identity.
    pub transition: [u8; 32],
    /// Effect identity.
    pub effect: [u8; 32],
}

/// Complete canonical five-action release bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesReleaseV5 {
    /// Five exact action descriptors in selector order.
    pub descriptors: [[u8; CAPABILITY_PROGRAM_V4_BYTES]; SERIES_RELEASE_ACTION_COUNT_V5],
    /// Five exact derived strategies in selector order.
    pub strategies: [[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2]; SERIES_RELEASE_ACTION_COUNT_V5],
    /// One exact five-entry ProgramSetV2.
    pub program_set: Vec<u8>,
    /// SHA-256 identity of `program_set`.
    pub program_set_id: [u8; 32],
    /// Descriptor-bound artifact identities in selector order.
    pub artifact_ids: [SeriesActionArtifactIdsV5; SERIES_RELEASE_ACTION_COUNT_V5],
    /// Action-specific typed authority in selector order.
    pub authorities: [SeriesOccurrenceAuthorityV5; SERIES_RELEASE_ACTION_COUNT_V5],
}

/// Typed selected physical role coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesSelectedRolesV5 {
    /// Sole composite root coordinate.
    pub root: u16,
    /// Ticket coordinate when selected.
    pub ticket: Option<u16>,
    /// RentCredit coordinate when selected.
    pub rent_credit: Option<u16>,
    /// Funding Create payer coordinate when selected.
    pub payer: Option<u16>,
    /// Funding Create surplus-refund coordinate when selected.
    pub refund: Option<u16>,
    /// Funding Create System-program coordinate when selected.
    pub system_program: Option<u16>,
}

/// Selected exact account/register/route geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesSelectedGeometryV5 {
    /// Logical fixed-account count.
    pub logical_fixed_accounts: u16,
    /// Expanded logical-account count after dynamic fixed spans.
    pub logical_accounts: usize,
    /// Exact physical-account count after authenticated alias packing.
    pub physical_accounts: usize,
    /// Common scalar-bank width.
    pub common_scalars: u16,
    /// Common identity-bank width.
    pub common_identities: u16,
    /// Child route count.
    pub route_count: u16,
}

/// Exact binding from one expanded logical coordinate to its packed physical
/// representative and runtime account ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLogicalPhysicalBindingV5 {
    /// Expanded logical coordinate.
    pub logical: usize,
    /// Authenticated logical representative.
    pub representative: usize,
    /// Packed runtime account ordinal.
    pub physical_ordinal: usize,
}

/// Owned selected bodies, copied only after complete release reauthentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesSelectedArtifactBodiesV5 {
    /// Current AccountProfileV3 body.
    pub account_profile: Vec<u8>,
    /// Current RequestProfileV1 body.
    pub request_profile: Vec<u8>,
    /// Current StateLifecyclePolicyV5 body.
    pub lifecycle: Vec<u8>,
    /// Exact selected strategy body.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// Current TransitionV3 body.
    pub transition: Vec<u8>,
    /// Current EffectV5 body.
    pub effect: Vec<u8>,
}

/// Reauthenticated operator-facing selected action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesSelectedActionV5 {
    /// Selected semantic action.
    pub action: SeriesActionV3,
    /// Complete planner-authenticated family request bytes.
    pub request_bytes: Vec<u8>,
    /// Exact descriptor bytes selected by ProgramSetV2.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
    /// Descriptor and selected-authority artifact identities.
    pub artifact_ids: SeriesActionArtifactIdsV5,
    /// Selected physical geometry.
    pub geometry: SeriesSelectedGeometryV5,
    /// Selected physical roles.
    pub roles: SeriesSelectedRolesV5,
    /// Exact action-specific occurrence authority.
    pub authority: SeriesOccurrenceAuthorityV5,
    /// Exact expanded logical-to-physical account bindings.
    pub account_bindings: Vec<SeriesLogicalPhysicalBindingV5>,
    /// Complete selected artifact bodies after byte-for-byte reauthentication.
    pub artifacts: SeriesSelectedArtifactBodiesV5,
    /// Exact route request templates in authenticated order.
    pub route_requests: Vec<Vec<u8>>,
}

/// Stable refusal from five-action release compilation or selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesReleaseErrorV5 {
    /// One artifact refused its current hostile decoder.
    Artifact,
    /// Cross-artifact account/register topology differed.
    Geometry,
    /// Funding or lifecycle authority differed from the selected action.
    Authority,
    /// Strategy derivation refused.
    Strategy,
    /// Descriptor assembly or self-decode refused.
    Descriptor,
    /// ProgramSet emission or selection refused.
    ProgramSet,
    /// Supplied release bytes differed from canonical recompilation.
    ReleaseSubstitution,
    /// Family request or selected Template differed.
    Request,
}

/// Result returned by the current five-action release seam.
pub type Result<T> = core::result::Result<T, SeriesReleaseErrorV5>;

/// Invoke every current action-specific semantic owner and return one opaque
/// bank that cannot be populated from arbitrary alternate artifact bytes.
pub fn emit_current_series_release_source_v5(
    input: SeriesCurrentReleaseInputV5<'_>,
) -> Result<SeriesOwnedReleaseSourceV5> {
    let prepare_authority = prepare_authority(input.prepare_requests.projected_initialize)?;
    let consume_authority = consume_authority(
        input.template,
        input.consume_requests.lock,
        input.consume_requests.core,
    )?;
    let expire_authority = expire_authority(input.template, input.expire_requests)?;
    let prepare = emit_series_prepare_funding_artifacts_v5(
        input.prepare_profile,
        input.prepare_requests,
        input.prepare_ticket_rent_lamports,
    )
    .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let consume = emit_current_consume_v5(
        input.consume_observed_data_lengths,
        input.consume_requests,
        input.consume_funding_count,
        consume_authority,
    )?;
    let expire =
        emit_series_expire_funding_artifacts_v5(input.expire_profile, input.expire_requests)
            .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let retire =
        emit_series_retire_funding_artifacts_v5().map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let close =
        emit_series_close_funding_artifacts_v5().map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    Ok(SeriesOwnedReleaseSourceV5 {
        template: input.template,
        consume_shadow_certificate_program: input.consume_shadow_certificate_program,
        actions: [
            SeriesOwnedActionArtifactsV5 {
                account_profile: prepare.account_profile,
                request_profile: prepare.request_profile,
                lifecycle: prepare.lifecycle,
                transition: prepare.transition,
                effect: prepare.effect,
                dynamic_fixed_span_counts: vec![],
                authority: prepare_authority,
            },
            consume,
            SeriesOwnedActionArtifactsV5 {
                account_profile: expire.account_profile,
                request_profile: expire.request_profile,
                lifecycle: empty_lifecycle()?,
                transition: expire.transition,
                effect: expire.effect,
                dynamic_fixed_span_counts: vec![],
                authority: expire_authority,
            },
            SeriesOwnedActionArtifactsV5 {
                account_profile: retire.account_profile,
                request_profile: retire.request_profile,
                lifecycle: retire.lifecycle,
                transition: retire.transition,
                effect: retire.effect,
                dynamic_fixed_span_counts: vec![],
                authority: SeriesOccurrenceAuthorityV5::Terminal,
            },
            SeriesOwnedActionArtifactsV5 {
                account_profile: close.account_profile,
                request_profile: close.request_profile,
                lifecycle: close.lifecycle,
                transition: close.transition,
                effect: close.effect,
                dynamic_fixed_span_counts: vec![],
                authority: SeriesOccurrenceAuthorityV5::Terminal,
            },
        ],
    })
}

fn prepare_authority(request: &[u8]) -> Result<SeriesOccurrenceAuthorityV5> {
    let projected =
        ProjectedCustodyRequestV1::decode(request).map_err(|_| SeriesReleaseErrorV5::Authority)?;
    if projected.operation != ProjectedCustodyOperationV1::Initialize {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    Ok(SeriesOccurrenceAuthorityV5::Prepare {
        market: projected.market,
        generation: projected.generation,
        release_set: projected.release_set,
        parent_root: projected.parent_capability_root,
    })
}

fn consume_authority(
    template: ContentId,
    projected_bytes: &[u8],
    core_bytes: &[u8],
) -> Result<SeriesOccurrenceAuthorityV5> {
    let projected = ProjectedCustodyRequestV1::decode(projected_bytes)
        .map_err(|_| SeriesReleaseErrorV5::Authority)?;
    let core =
        SeriesCoreRequestV1::decode(core_bytes).map_err(|_| SeriesReleaseErrorV5::Authority)?;
    let market = core.market().ok_or(SeriesReleaseErrorV5::Authority)?;
    let generation = core
        .market_generation()
        .ok_or(SeriesReleaseErrorV5::Authority)?;
    if projected.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
        || core.action() != SeriesCoreActionV1::Consume
        || core.template().to_bytes() != template.to_bytes()
        || market.to_bytes() != projected.market
        || generation != projected.generation
        || core.release_set().to_bytes() != projected.release_set
    {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    Ok(SeriesOccurrenceAuthorityV5::Consume {
        market: projected.market,
        generation: projected.generation,
        release_set: projected.release_set,
        parent_root: projected.parent_capability_root,
    })
}

fn expire_authority(
    template: ContentId,
    requests: SeriesExpireChildRequestsV5<'_>,
) -> Result<SeriesOccurrenceAuthorityV5> {
    let permit = requests.permit_expiry.permit();
    let intent = permit.intent();
    let core = requests.core_expire;
    if core.action() != SeriesCoreActionV1::Expire
        || core.template().to_bytes() != template.to_bytes()
        || core.market().map(|value| value.to_bytes()) != Some(intent.market().to_bytes())
        || core.market_generation() != Some(intent.generation())
        || core.release_set() != intent.release_set()
    {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    Ok(SeriesOccurrenceAuthorityV5::Expire {
        market: intent.market().to_bytes(),
        generation: intent.generation(),
        release_set: intent.release_set().to_bytes(),
        parent_root: intent.parent_root().to_bytes(),
    })
}

/// Compile and hostile-decode one complete five-entry canonical release.
pub fn compile_series_release_v5(source: SeriesReleaseSourceV5<'_>) -> Result<SeriesReleaseV5> {
    let mut descriptors = [[0_u8; CAPABILITY_PROGRAM_V4_BYTES]; SERIES_RELEASE_ACTION_COUNT_V5];
    let mut strategies =
        [[0_u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2]; SERIES_RELEASE_ACTION_COUNT_V5];
    let mut artifact_ids = [empty_ids(); SERIES_RELEASE_ACTION_COUNT_V5];
    for index in 0..SERIES_RELEASE_ACTION_COUNT_V5 {
        let action = action_from_index(index)?;
        let validated = validate_action_source(action, source.actions[index])?;
        let transition_id = content(validated.ids.transition)?;
        strategies[index] = encode_action_strategy(
            action,
            source.consume_shadow_certificate_program,
            transition_id,
        )?;
        ExecutionStrategyProgramV2::decode(&strategies[index])
            .map_err(|_| SeriesReleaseErrorV5::Strategy)?;
        artifact_ids[index] = SeriesActionArtifactIdsV5 {
            strategy: hash(&strategies[index]).to_bytes(),
            ..validated.ids
        };
        descriptors[index] = encode_descriptor(source.template, artifact_ids[index])?;
    }
    let entries: [CapabilityProgramSetEntryV2; SERIES_RELEASE_ACTION_COUNT_V5] =
        core::array::from_fn(|index| {
            CapabilityProgramSetEntryV2::new(
                index as u32,
                CapabilityDescriptorReferenceV2::new(
                    content(CAPABILITY_PROGRAM_SCHEMA_ID_V4).expect("nonzero schema"),
                    content(hash(&descriptors[index]).to_bytes()).expect("nonzero descriptor"),
                ),
            )
        });
    let mut program_set = vec![
        0_u8;
        encoded_program_set_bytes_v2(entries.len())
            .map_err(|_| SeriesReleaseErrorV5::ProgramSet)?
    ];
    encode_program_set_v2(
        SERIES_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| SeriesReleaseErrorV5::ProgramSet)?;
    let decoded = CapabilityProgramSetV2::decode(&program_set)
        .map_err(|_| SeriesReleaseErrorV5::ProgramSet)?;
    if decoded.entry_count() != SERIES_RELEASE_ACTION_COUNT_V5 as u16 {
        return Err(SeriesReleaseErrorV5::ProgramSet);
    }
    Ok(SeriesReleaseV5 {
        descriptors,
        strategies,
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
        artifact_ids,
        authorities: core::array::from_fn(|index| source.actions[index].authority),
    })
}

fn emit_current_consume_v5(
    observed: &[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    requests: SeriesConsumeChildRequestsV4<'_>,
    funding_count: u32,
    authority: SeriesOccurrenceAuthorityV5,
) -> Result<SeriesOwnedActionArtifactsV5> {
    if funding_count == 0 {
        return Err(SeriesReleaseErrorV5::Geometry);
    }
    let mut lengths = *observed;
    stamp_series_release_owned_widths_v4(
        &mut lengths,
        SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32,
        SERIES_TICKET_STATE_BYTES_V3 as u32,
    );
    let emitted = emit_series_consume_artifacts_v4(SeriesConsumeAccountProfileInputV4 {
        fixed_data_lengths: &lengths,
    })
    .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let mut account_scratch =
        vec![0_u8; ACCOUNT_PROFILE_HEADER_BYTES_V3 + emitted.account_profile.len()];
    let mut account_profile = vec![0_u8; account_scratch.len()];
    encode_account_profile_v3_atomic(
        &emitted.account_profile,
        &[],
        &mut account_scratch,
        &mut account_profile,
    )
    .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    AccountProfileV3::decode(&account_profile).map_err(|_| SeriesReleaseErrorV5::Artifact)?;

    let mut base_scratch = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
    let mut base = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
    let mut v4_scratch = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
    let mut v4 = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
    encode_series_consume_effect_v4_from_requests_atomic(
        requests,
        &mut base_scratch,
        &mut base,
        &mut v4_scratch,
        &mut v4,
    )
    .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let mut effect_scratch = vec![0_u8; EFFECT_HEADER_BYTES_V5 + v4.len()];
    let mut effect = vec![0_u8; effect_scratch.len()];
    encode_program_v5_atomic(&v4, &[], &[], &mut effect_scratch, &mut effect)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    EffectProgramV5::decode(&effect).map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    Ok(SeriesOwnedActionArtifactsV5 {
        account_profile,
        request_profile: emitted.request_profile.to_vec(),
        lifecycle: empty_lifecycle()?,
        transition: emitted.transition.to_vec(),
        effect,
        dynamic_fixed_span_counts: vec![funding_count],
        authority,
    })
}

fn empty_lifecycle() -> Result<Vec<u8>> {
    let mut scratch = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    let mut output = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
    encode_series_empty_state_lifecycle_v5_atomic(&mut scratch, &mut output)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let id = hash(&output).to_bytes();
    StateLifecyclePolicyV5::decode_selected(id, id, &output)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    Ok(output)
}

fn encode_interpreted_strategy(
    transition_program: ContentId,
) -> Result<[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2]> {
    let schema = |bytes| content(bytes).map_err(|_| SeriesReleaseErrorV5::Strategy);
    let strategy = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        schema(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        transition_program,
        schema(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        schema(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        schema(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        schema(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(|_| SeriesReleaseErrorV5::Strategy)?;
    Ok(strategy.to_bytes())
}

fn encode_action_strategy(
    action: SeriesActionV3,
    consume_shadow_certificate_program: ContentId,
    transition_program: ContentId,
) -> Result<[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2]> {
    if action == SeriesActionV3::Consume {
        encode_series_consume_strategy_v4(consume_shadow_certificate_program, transition_program)
            .map_err(|_| SeriesReleaseErrorV5::Strategy)
    } else {
        encode_interpreted_strategy(transition_program)
    }
}

/// Recompile the release, select by the authenticated family request, and
/// return exact physical geometry/authority without re-deriving client DTOs.
pub fn authenticate_series_selected_action_v5(
    release: &SeriesReleaseV5,
    source: SeriesReleaseSourceV5<'_>,
    family_request: &[u8],
) -> Result<SeriesSelectedActionV5> {
    let canonical = compile_series_release_v5(source)?;
    if release != &canonical {
        return Err(SeriesReleaseErrorV5::ReleaseSubstitution);
    }
    let request =
        SeriesActionRequestV3::decode(family_request).map_err(|_| SeriesReleaseErrorV5::Request)?;
    if request.template() != source.template {
        return Err(SeriesReleaseErrorV5::Request);
    }
    let action = request.action();
    let index = action as usize;
    let set = CapabilityProgramSetV2::decode_selected(
        release.program_set_id,
        hash(&release.program_set).to_bytes(),
        &release.program_set,
    )
    .map_err(|_| SeriesReleaseErrorV5::ProgramSet)?;
    let selected = set
        .select_entry(family_request)
        .map_err(|_| SeriesReleaseErrorV5::ProgramSet)?;
    if selected.descriptor().program().to_bytes() != hash(&release.descriptors[index]).to_bytes() {
        return Err(SeriesReleaseErrorV5::Descriptor);
    }
    let descriptor = CapabilityProgramV4::decode(&release.descriptors[index])
        .map_err(|_| SeriesReleaseErrorV5::Descriptor)?;
    if descriptor.capacity_profile() != source.template {
        return Err(SeriesReleaseErrorV5::Descriptor);
    }
    let validated = validate_action_source(action, source.actions[index])?;
    let mut expected_ids = validated.ids;
    expected_ids.strategy = hash(&release.strategies[index]).to_bytes();
    if expected_ids != release.artifact_ids[index] {
        return Err(SeriesReleaseErrorV5::ReleaseSubstitution);
    }
    Ok(SeriesSelectedActionV5 {
        action,
        request_bytes: family_request.to_vec(),
        descriptor: release.descriptors[index],
        artifact_ids: release.artifact_ids[index],
        geometry: validated.geometry,
        roles: roles(action),
        authority: release.authorities[index],
        account_bindings: validated.account_bindings,
        artifacts: SeriesSelectedArtifactBodiesV5 {
            account_profile: source.actions[index].account_profile.to_vec(),
            request_profile: source.actions[index].request_profile.to_vec(),
            lifecycle: source.actions[index].lifecycle.to_vec(),
            strategy: release.strategies[index],
            transition: source.actions[index].transition.to_vec(),
            effect: source.actions[index].effect.to_vec(),
        },
        route_requests: route_requests(validated.effect)?,
    })
}

struct ValidatedAction<'a> {
    ids: SeriesActionArtifactIdsV5,
    geometry: SeriesSelectedGeometryV5,
    account_bindings: Vec<SeriesLogicalPhysicalBindingV5>,
    effect: EffectProgramV5<'a>,
}

fn validate_action_source<'a>(
    action: SeriesActionV3,
    source: SeriesActionArtifactSourceV5<'a>,
) -> Result<ValidatedAction<'a>> {
    if !matches!(
        (action, source.authority),
        (
            SeriesActionV3::Prepare,
            SeriesOccurrenceAuthorityV5::Prepare { .. }
        ) | (
            SeriesActionV3::Consume,
            SeriesOccurrenceAuthorityV5::Consume { .. }
        ) | (
            SeriesActionV3::Expire,
            SeriesOccurrenceAuthorityV5::Expire { .. }
        ) | (
            SeriesActionV3::Retire | SeriesActionV3::Close,
            SeriesOccurrenceAuthorityV5::Terminal
        )
    ) {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    let profile = AccountProfileV3::decode(source.account_profile)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let request = RequestProfileV1::decode(source.request_profile)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let lifecycle_id = hash(source.lifecycle).to_bytes();
    let lifecycle =
        StateLifecyclePolicyV5::decode_selected(lifecycle_id, lifecycle_id, source.lifecycle)
            .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let transition = TransitionProgramV3::decode(source.transition)
        .map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let effect =
        EffectProgramV5::decode(source.effect).map_err(|_| SeriesReleaseErrorV5::Artifact)?;
    let base = effect.base().base();
    if request.common_scalar_count() != transition.common_scalar_count()
        || request.common_identity_count() != transition.common_identity_count()
        || profile.base().common_scalar_count() != transition.common_scalar_count()
        || profile.base().common_identity_count() != transition.common_identity_count()
        || base.common_scalar_count() != transition.common_scalar_count()
        || base.common_identity_count() != transition.common_identity_count()
        || base.fixed_account_count() != profile.base().fixed_account_count()
    {
        return Err(SeriesReleaseErrorV5::Geometry);
    }
    lifecycle
        .validate_account_profile_with_external_funding_join(profile)
        .map_err(|_| SeriesReleaseErrorV5::Authority)?;
    validate_action_authority(action, profile, lifecycle, effect)?;
    let logical_accounts = if profile.base().uses_dynamic_fixed_spans() {
        profile
            .base()
            .logical_account_count_with_dynamic_spans(0, source.dynamic_fixed_span_counts)
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?
    } else {
        usize::from(profile.base().fixed_account_count())
    };
    let physical_accounts = if profile.base().uses_dynamic_fixed_spans() {
        profile
            .base()
            .physical_account_count_with_dynamic_spans(0, source.dynamic_fixed_span_counts)
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?
    } else {
        if !source.dynamic_fixed_span_counts.is_empty() {
            return Err(SeriesReleaseErrorV5::Geometry);
        }
        profile
            .base()
            .physical_account_count(0)
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?
    };
    let mut account_bindings = Vec::with_capacity(logical_accounts);
    for logical in 0..logical_accounts {
        let representative = profile
            .base()
            .representative_with_dynamic_spans(0, source.dynamic_fixed_span_counts, logical)
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?;
        let physical_ordinal = profile
            .base()
            .physical_account_ordinal_with_dynamic_spans(
                0,
                source.dynamic_fixed_span_counts,
                logical,
            )
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?;
        account_bindings.push(SeriesLogicalPhysicalBindingV5 {
            logical,
            representative,
            physical_ordinal,
        });
    }
    Ok(ValidatedAction {
        ids: SeriesActionArtifactIdsV5 {
            account_profile: hash(source.account_profile).to_bytes(),
            request_profile: hash(source.request_profile).to_bytes(),
            lifecycle: lifecycle_id,
            strategy: [0; 32],
            transition: hash(source.transition).to_bytes(),
            effect: hash(source.effect).to_bytes(),
        },
        geometry: SeriesSelectedGeometryV5 {
            logical_fixed_accounts: base.fixed_account_count(),
            logical_accounts,
            physical_accounts,
            common_scalars: base.common_scalar_count(),
            common_identities: base.common_identity_count(),
            route_count: base.route_count(),
        },
        account_bindings,
        effect,
    })
}

fn validate_action_authority(
    action: SeriesActionV3,
    profile: AccountProfileV3<'_>,
    lifecycle: StateLifecyclePolicyV5<'_>,
    effect: EffectProgramV5<'_>,
) -> Result<()> {
    let expected_lifecycle = usize::from(matches!(action, SeriesActionV3::Close));
    if usize::from(
        lifecycle
            .action_plan_count(action as u32)
            .map_err(|_| SeriesReleaseErrorV5::Authority)?,
    ) != expected_lifecycle
    {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    let expected_funding = usize::from(matches!(
        action,
        SeriesActionV3::Prepare | SeriesActionV3::Retire
    ));
    if effect.funding_action_count() as usize != expected_funding
        || profile.funding_bound_count() as usize != expected_funding
    {
        return Err(SeriesReleaseErrorV5::Authority);
    }
    if expected_funding == 1 {
        let funding = effect
            .funding_action(0)
            .map_err(|_| SeriesReleaseErrorV5::Authority)?;
        let expected = if action == SeriesActionV3::Prepare {
            FundingOperationV5::Create
        } else {
            FundingOperationV5::Close
        };
        if funding.operation() != expected || funding.state() != 5 {
            return Err(SeriesReleaseErrorV5::Authority);
        }
    }
    Ok(())
}

fn encode_descriptor(
    template: ContentId,
    ids: SeriesActionArtifactIdsV5,
) -> Result<[u8; CAPABILITY_PROGRAM_V4_BYTES]> {
    let reference = |schema, program| {
        Ok::<_, SeriesReleaseErrorV5>(ArtifactReferenceV4::new(
            content(schema)?,
            content(program)?,
        ))
    };
    let descriptor = CapabilityProgramV4::new(
        content(hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes())?,
        content(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3)?,
        content(hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes())?,
        content(hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes())?,
        content(hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes())?,
        template,
        CapabilityArtifactsV4 {
            account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V3, ids.account_profile)?,
            request_profile: reference(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                ids.request_profile,
            )?,
            lifecycle: reference(CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, ids.lifecycle)?,
            strategy: reference(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ids.strategy)?,
            transition: reference(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID, ids.transition)?,
            effect: reference(dclutch_effect_kernel::v5::SCHEMA_RELEASE_ID_V5, ids.effect)?,
        },
        SERIES_STATE_BYTES_V3 as u32,
    )
    .map_err(|_| SeriesReleaseErrorV5::Descriptor)?;
    let bytes = descriptor.encode();
    CapabilityProgramV4::decode(&bytes).map_err(|_| SeriesReleaseErrorV5::Descriptor)?;
    Ok(bytes)
}

fn route_requests(effect: EffectProgramV5<'_>) -> Result<Vec<Vec<u8>>> {
    let base = effect.base().base();
    let mut requests = Vec::with_capacity(base.route_count() as usize);
    for index in 0..base.route_count() {
        let (fixed, _) = base
            .route_template(index)
            .map_err(|_| SeriesReleaseErrorV5::Geometry)?;
        requests.push(fixed.to_vec());
    }
    Ok(requests)
}

const fn roles(action: SeriesActionV3) -> SeriesSelectedRolesV5 {
    match action {
        SeriesActionV3::Prepare => SeriesSelectedRolesV5 {
            root: 0,
            ticket: Some(SERIES_PREPARE_TICKET_COORDINATE_V5),
            rent_credit: None,
            payer: Some(SERIES_PREPARE_PAYER_COORDINATE_V5),
            refund: Some(SERIES_PREPARE_REFUND_COORDINATE_V5),
            system_program: Some(SERIES_PREPARE_SYSTEM_COORDINATE_V5),
        },
        SeriesActionV3::Consume => SeriesSelectedRolesV5 {
            root: 0,
            ticket: Some(SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4),
            rent_credit: None,
            payer: None,
            refund: None,
            system_program: None,
        },
        SeriesActionV3::Expire => SeriesSelectedRolesV5 {
            root: 0,
            ticket: Some(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            rent_credit: Some(SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5),
            payer: None,
            refund: None,
            system_program: None,
        },
        SeriesActionV3::Retire => SeriesSelectedRolesV5 {
            root: 0,
            ticket: Some(SERIES_RETIRE_TICKET_COORDINATE_V5),
            rent_credit: Some(SERIES_RETIRE_RENT_CREDIT_COORDINATE_V5),
            payer: None,
            refund: None,
            system_program: None,
        },
        SeriesActionV3::Close => SeriesSelectedRolesV5 {
            root: 0,
            ticket: None,
            rent_credit: Some(SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5),
            payer: None,
            refund: None,
            system_program: None,
        },
    }
}

const fn empty_ids() -> SeriesActionArtifactIdsV5 {
    SeriesActionArtifactIdsV5 {
        account_profile: [0; 32],
        request_profile: [0; 32],
        lifecycle: [0; 32],
        strategy: [0; 32],
        transition: [0; 32],
        effect: [0; 32],
    }
}

fn action_from_index(index: usize) -> Result<SeriesActionV3> {
    match index {
        0 => Ok(SeriesActionV3::Prepare),
        1 => Ok(SeriesActionV3::Consume),
        2 => Ok(SeriesActionV3::Expire),
        3 => Ok(SeriesActionV3::Retire),
        4 => Ok(SeriesActionV3::Close),
        _ => Err(SeriesReleaseErrorV5::ProgramSet),
    }
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| SeriesReleaseErrorV5::Descriptor)
}

const _: () = assert!(SERIES_CLOSE_FIXED_ACCOUNT_COUNT_V5 == 7);

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims_svm::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
    use dclutch_custody_contract::{CompartmentV1, ProjectedCallerRoleV1};
    use dclutch_market_core_codec::{
        FoundingIntentV5, Identity, SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
    };
    use dclutch_series_v3_kernel::request::encode_series_action_header_v3;

    use crate::series::{
        artifacts_v3::{
            SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        },
        expire_funding_artifacts_v5::SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5,
        prepare_funding_artifacts_v5::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5,
    };

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero identity")
    }

    fn mid(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("nonzero identity")
    }

    fn projected(
        operation: ProjectedCustodyOperationV1,
        parent_root: [u8; 32],
    ) -> ProjectedCustodyRequestV1 {
        let (expected_revision, resulting_revision, amount) = match operation {
            ProjectedCustodyOperationV1::Initialize => (0, 1, 0),
            ProjectedCustodyOperationV1::OpenHoard => (1, 2, 0),
            _ => (2, 3, 9),
        };
        ProjectedCustodyRequestV1 {
            operation,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: [2; 32],
            generation: 8,
            realm: [20; 32],
            product_record: [3; 32],
            product: [21; 32],
            source: [4; 32],
            release_set: [1; 32],
            projection_receipt_digest: [11; 32],
            parent_capability_root: parent_root,
            context_digest: [12; 32],
            caller_program: [13; 32],
            payer: [14; 32],
            core_program: [15; 32],
            rent_program: [16; 32],
            refund_owner: [17; 32],
            rent_credit: [18; 32],
            hoard_vault: [19; 32],
            funding_source_vault: [22; 32],
            funding_source_context: [23; 32],
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: [24; 32],
            token_program: [25; 32],
            collateral_release: [26; 32],
            expiry_slot: 100,
            expected_revision,
            resulting_revision,
            amount,
            state_rent_lamports: 41,
            vault_rent_lamports: 42,
            funding_source_replay_revision: 1,
            funding_source_state_rent_lamports: 44,
            funding_source_vault_rent_lamports: 45,
        }
    }

    fn claims(dynamic: u8) -> [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3] {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: [1; 32],
            market: [2; 32],
            product_record_digest: [3; 32],
            product_instance_id: [4; 32],
            linked_basis_record_digest: [5; 32],
            semantic_basis_id: [6; 32],
            founder: [7; 32],
            founding_intent_digest: [dynamic; 32],
            aggregate: [9; 32],
            position: [10; 32],
            admission: [11; 32],
            funding_source: [12; 32],
            hoard: [13; 32],
            custody_replay: [14; 32],
            rent_credit: [15; 32],
            rent_program: [16; 32],
            claims_program: [17; 32],
            trading_program: [18; 32],
            custody_request_digest: [dynamic.wrapping_add(1); 32],
            custody_receipt_digest: [dynamic.wrapping_add(2); 32],
            generation: 8,
            claim_count: 3,
            quantity: 3,
            basis_scale: 3,
            pre_source_amount: 9,
            post_source_amount: 0,
            pre_hoard_amount: 0,
            post_hoard_amount: 9,
            pre_custody_revision: 2,
            post_custody_revision: 3,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("Claims request")
        .to_bytes()
    }

    fn core(action: SeriesCoreActionV1) -> [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3] {
        SeriesCoreRequestV1::occurrence(
            action,
            mid(1),
            mid(18),
            mid(19),
            mid(2),
            mid(20),
            mid(3),
            mid(21),
            mid(5),
            7,
            3,
            1,
            22,
            23,
            24,
            25,
        )
        .expect("Core request")
        .encode()
        .expect("Core bytes")
    }

    fn current_source_with_root(parent_root: u8) -> SeriesOwnedReleaseSourceV5 {
        let root = [parent_root; 32];
        let prepare_initialize = projected(ProjectedCustodyOperationV1::Initialize, root)
            .encode()
            .expect("Prepare projected initialize");
        let prepare_open = projected(ProjectedCustodyOperationV1::OpenHoard, root)
            .encode()
            .expect("Prepare projected open");
        let replay_initialize = [32; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let escrow_open = [33; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let escrow_lock = [34; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let consume_lock = projected(ProjectedCustodyOperationV1::LockHoardAndCloseSource, root)
            .encode()
            .expect("Consume projected lock");
        let consume_core = core(SeriesCoreActionV1::Consume);
        let consume_realize = projected(ProjectedCustodyOperationV1::RealizeAndClose, root)
            .encode()
            .expect("Consume projected realize");
        let consume_claims = claims(parent_root.wrapping_add(40));
        let refund = [37; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_vault = [38; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_replay = [39; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let projected_abort = [40; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let intent = FoundingIntentV5::new(
            255,
            mid(1),
            mid(2),
            mid(3),
            mid(4),
            mid(5),
            mid(6),
            Identity::new(root).expect("root"),
            mid(8),
            mid(9),
            mid(10),
            mid(11),
            mid(12),
            mid(13),
            mid(14),
            mid(15),
            8,
            1,
            1,
            100,
            4,
            1,
        )
        .expect("founding intent");
        let permit_expiry = SeriesPermitExpiryRequestV1::new(
            SeriesFoundingPermitV1::new(intent, mid(16), mid(17)).expect("permit"),
        );
        let expire_core =
            SeriesCoreRequestV1::decode(&core(SeriesCoreActionV1::Expire)).expect("Expire Core");
        let prepare_lengths = [0; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let consume_lengths = [0; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        let expire_lengths = [0; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
        emit_current_series_release_source_v5(SeriesCurrentReleaseInputV5 {
            template: id(18),
            consume_shadow_certificate_program: id(90),
            prepare_profile: SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &prepare_lengths,
            },
            prepare_requests: SeriesPrepareChildRequestsV4 {
                projected_initialize: &prepare_initialize,
                projected_open: &prepare_open,
                replay_initialize: &replay_initialize,
                escrow_open: &escrow_open,
                escrow_lock: &escrow_lock,
            },
            prepare_ticket_rent_lamports: 123,
            consume_observed_data_lengths: &consume_lengths,
            consume_requests: SeriesConsumeChildRequestsV4 {
                lock: &consume_lock,
                core: &consume_core,
                realize: &consume_realize,
                claims: &consume_claims,
            },
            consume_funding_count: 3,
            expire_profile: SeriesExpireAccountProfileInputV5 {
                fixed_data_lengths: &expire_lengths,
            },
            expire_requests: SeriesExpireChildRequestsV5 {
                refund: &refund,
                close_vault: &close_vault,
                close_replay: &close_replay,
                projected_abort: &projected_abort,
                permit_expiry,
                core_expire: expire_core,
            },
        })
        .expect("complete current source")
    }

    fn current_source() -> SeriesOwnedReleaseSourceV5 {
        current_source_with_root(7)
    }

    fn family_request(action: SeriesActionV3) -> Vec<u8> {
        let occurrence = action.occurrence_bound().then(|| id(30));
        let ticket = (action != SeriesActionV3::Close).then(|| id(31));
        encode_series_action_header_v3(
            action,
            id(18),
            occurrence,
            ticket,
            7,
            if matches!(action, SeriesActionV3::Prepare | SeriesActionV3::Close) {
                0
            } else {
                1
            },
            0,
        )
        .expect("family request")
        .to_vec()
    }

    #[test]
    fn complete_current_bank_compiles_and_reauthenticates_all_five_actions() {
        let owned = current_source();
        let release = compile_series_release_v5(owned.as_source()).expect("five-entry release");
        assert_eq!(
            CapabilityProgramSetV2::decode(&release.program_set)
                .expect("SetV2")
                .entry_count(),
            SERIES_RELEASE_ACTION_COUNT_V5 as u16
        );
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            let request = family_request(action);
            let selected =
                authenticate_series_selected_action_v5(&release, owned.as_source(), &request)
                    .expect("selected action");
            let index = action as usize;
            assert_eq!(selected.action, action);
            assert_eq!(selected.request_bytes, request);
            assert_eq!(selected.descriptor, release.descriptors[index]);
            assert_eq!(selected.artifact_ids, release.artifact_ids[index]);
            assert_eq!(selected.authority, release.authorities[index]);
            assert_eq!(selected.roles, roles(action));
            assert!(matches!(
                (action, selected.authority),
                (
                    SeriesActionV3::Prepare,
                    SeriesOccurrenceAuthorityV5::Prepare { .. }
                ) | (
                    SeriesActionV3::Consume,
                    SeriesOccurrenceAuthorityV5::Consume { .. }
                ) | (
                    SeriesActionV3::Expire,
                    SeriesOccurrenceAuthorityV5::Expire { .. }
                ) | (
                    SeriesActionV3::Retire | SeriesActionV3::Close,
                    SeriesOccurrenceAuthorityV5::Terminal
                )
            ));
            let descriptor =
                CapabilityProgramV4::decode(&selected.descriptor).expect("selected descriptor");
            assert_eq!(
                [
                    descriptor.account_profile().program().to_bytes(),
                    descriptor.request_profile().program().to_bytes(),
                    descriptor.lifecycle().program().to_bytes(),
                    descriptor.strategy().program().to_bytes(),
                    descriptor.transition().program().to_bytes(),
                    descriptor.effect().program().to_bytes(),
                ],
                [
                    selected.artifact_ids.account_profile,
                    selected.artifact_ids.request_profile,
                    selected.artifact_ids.lifecycle,
                    selected.artifact_ids.strategy,
                    selected.artifact_ids.transition,
                    selected.artifact_ids.effect,
                ]
            );
            assert_eq!(
                selected.account_bindings.len(),
                selected.geometry.logical_accounts
            );
            assert!(selected.account_bindings.iter().all(|binding| {
                binding.logical < selected.geometry.logical_accounts
                    && binding.representative < selected.geometry.logical_accounts
                    && binding.physical_ordinal < selected.geometry.physical_accounts
            }));
            let profile = AccountProfileV3::decode(&selected.artifacts.account_profile)
                .expect("selected profile");
            let spans = owned.action_artifacts(action).dynamic_fixed_span_counts();
            for binding in &selected.account_bindings {
                assert_eq!(
                    profile
                        .base()
                        .representative_with_dynamic_spans(0, spans, binding.logical),
                    Ok(binding.representative)
                );
                assert_eq!(
                    profile.base().physical_account_ordinal_with_dynamic_spans(
                        0,
                        spans,
                        binding.logical
                    ),
                    Ok(binding.physical_ordinal)
                );
            }
            assert_eq!(
                hash(&selected.artifacts.account_profile).to_bytes(),
                selected.artifact_ids.account_profile
            );
            assert_eq!(
                hash(&selected.artifacts.request_profile).to_bytes(),
                selected.artifact_ids.request_profile
            );
            assert_eq!(
                hash(&selected.artifacts.lifecycle).to_bytes(),
                selected.artifact_ids.lifecycle
            );
            assert_eq!(
                hash(&selected.artifacts.strategy).to_bytes(),
                selected.artifact_ids.strategy
            );
            assert_eq!(
                hash(&selected.artifacts.transition).to_bytes(),
                selected.artifact_ids.transition
            );
            assert_eq!(
                hash(&selected.artifacts.effect).to_bytes(),
                selected.artifact_ids.effect
            );
            assert_eq!(selected.roles.root, 0);
            for coordinate in [selected.roles.ticket, selected.roles.rent_credit]
                .into_iter()
                .flatten()
            {
                assert!(
                    selected
                        .account_bindings
                        .iter()
                        .any(|binding| binding.logical == usize::from(coordinate))
                );
            }
            let expected_disposition = if action == SeriesActionV3::Consume {
                StrategyDispositionV2::ShadowAot
            } else {
                StrategyDispositionV2::Interpreted
            };
            assert_eq!(
                ExecutionStrategyProgramV2::decode(&selected.artifacts.strategy)
                    .expect("selected strategy")
                    .disposition(),
                expected_disposition
            );
        }
    }

    #[test]
    fn two_parent_roots_compile_one_program_set_with_exact_selected_authority() {
        let first = current_source_with_root(7);
        let second = current_source_with_root(70);
        let first_release =
            compile_series_release_v5(first.as_source()).expect("first root release");
        let second_release =
            compile_series_release_v5(second.as_source()).expect("second root release");
        assert_eq!(first_release.program_set, second_release.program_set);
        assert_eq!(first_release.program_set_id, second_release.program_set_id);
        assert_eq!(first_release.descriptors, second_release.descriptors);
        assert_eq!(first_release.artifact_ids, second_release.artifact_ids);
        for index in 0..SERIES_RELEASE_ACTION_COUNT_V5 {
            assert_eq!(
                first.actions[index].account_profile,
                second.actions[index].account_profile
            );
            assert_eq!(
                first.actions[index].request_profile,
                second.actions[index].request_profile
            );
            assert_eq!(
                first.actions[index].lifecycle,
                second.actions[index].lifecycle
            );
            assert_eq!(
                first.actions[index].transition,
                second.actions[index].transition
            );
            assert_eq!(first.actions[index].effect, second.actions[index].effect);
        }
        for (action, expected_first, expected_second) in [
            (SeriesActionV3::Prepare, [7; 32], [70; 32]),
            (SeriesActionV3::Consume, [7; 32], [70; 32]),
            (SeriesActionV3::Expire, [7; 32], [70; 32]),
        ] {
            let parent_root = |authority| match authority {
                SeriesOccurrenceAuthorityV5::Prepare { parent_root, .. }
                | SeriesOccurrenceAuthorityV5::Consume { parent_root, .. }
                | SeriesOccurrenceAuthorityV5::Expire { parent_root, .. } => parent_root,
                SeriesOccurrenceAuthorityV5::Terminal => panic!("occurrence authority"),
            };
            assert_eq!(
                parent_root(first_release.authorities[action as usize]),
                expected_first
            );
            assert_eq!(
                parent_root(second_release.authorities[action as usize]),
                expected_second
            );
        }
    }

    #[test]
    fn complete_bank_refuses_release_artifact_and_authority_substitution() {
        let owned = current_source();
        let release = compile_series_release_v5(owned.as_source()).expect("five-entry release");
        let request = family_request(SeriesActionV3::Expire);
        let refuses_release = |hostile: &SeriesReleaseV5| {
            assert_eq!(
                authenticate_series_selected_action_v5(hostile, owned.as_source(), &request),
                Err(SeriesReleaseErrorV5::ReleaseSubstitution)
            );
        };

        let mut hostile = release.clone();
        hostile.program_set[0] ^= 1;
        refuses_release(&hostile);
        let mut hostile = release.clone();
        hostile.descriptors[SeriesActionV3::Expire as usize][0] ^= 1;
        refuses_release(&hostile);
        let mut hostile = release.clone();
        hostile.strategies[SeriesActionV3::Expire as usize][0] ^= 1;
        refuses_release(&hostile);
        let mut hostile = release.clone();
        hostile.artifact_ids[SeriesActionV3::Expire as usize].effect[0] ^= 1;
        refuses_release(&hostile);

        for field in 0..4 {
            let mut hostile = release.clone();
            let SeriesOccurrenceAuthorityV5::Expire {
                market,
                generation,
                release_set,
                parent_root,
            } = &mut hostile.authorities[SeriesActionV3::Expire as usize]
            else {
                panic!("Expire authority")
            };
            match field {
                0 => market[0] ^= 1,
                1 => *generation += 1,
                2 => release_set[0] ^= 1,
                3 => parent_root[0] ^= 1,
                _ => unreachable!(),
            }
            refuses_release(&hostile);
        }

        for artifact in 0..5 {
            let mut hostile_source = owned.clone();
            let selected = &mut hostile_source.actions[SeriesActionV3::Expire as usize];
            match artifact {
                0 => selected.account_profile[0] ^= 1,
                1 => selected.request_profile[0] ^= 1,
                2 => selected.lifecycle[0] ^= 1,
                3 => selected.transition[0] ^= 1,
                4 => selected.effect[0] ^= 1,
                _ => unreachable!(),
            }
            assert_eq!(
                authenticate_series_selected_action_v5(
                    &release,
                    hostile_source.as_source(),
                    &request
                ),
                Err(SeriesReleaseErrorV5::Artifact)
            );
        }

        let mut crossed = owned.clone();
        crossed.actions.swap(
            SeriesActionV3::Prepare as usize,
            SeriesActionV3::Consume as usize,
        );
        assert_eq!(
            compile_series_release_v5(crossed.as_source()),
            Err(SeriesReleaseErrorV5::Authority)
        );
    }

    #[test]
    fn exact_action_strategy_map_keeps_expire_interpreted() {
        let certificate_program = id(91);
        let transition = id(92);
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            let bytes = encode_action_strategy(action, certificate_program, transition)
                .expect("interpreted action strategy");
            assert_eq!(
                ExecutionStrategyProgramV2::decode(&bytes)
                    .expect("strategy")
                    .disposition(),
                StrategyDispositionV2::Interpreted
            );
        }
        let shadow =
            encode_action_strategy(SeriesActionV3::Consume, certificate_program, transition)
                .expect("Consume Shadow strategy");
        assert_eq!(
            ExecutionStrategyProgramV2::decode(&shadow)
                .expect("strategy")
                .disposition(),
            StrategyDispositionV2::ShadowAot
        );
        let expire =
            encode_action_strategy(SeriesActionV3::Expire, certificate_program, transition)
                .expect("Expire interpreted strategy");
        assert_ne!(
            shadow, expire,
            "a Shadow substitution changes canonical bytes"
        );
    }

    #[test]
    fn terminal_cross_action_artifacts_refuse_before_selection() {
        let retire = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let close = emit_series_close_funding_artifacts_v5().expect("Close artifacts");
        let retire_source = SeriesActionArtifactSourceV5 {
            account_profile: &retire.account_profile,
            request_profile: &retire.request_profile,
            lifecycle: &retire.lifecycle,
            transition: &retire.transition,
            effect: &retire.effect,
            dynamic_fixed_span_counts: &[],
            authority: SeriesOccurrenceAuthorityV5::Terminal,
        };
        let close_source = SeriesActionArtifactSourceV5 {
            account_profile: &close.account_profile,
            request_profile: &close.request_profile,
            lifecycle: &close.lifecycle,
            transition: &close.transition,
            effect: &close.effect,
            dynamic_fixed_span_counts: &[],
            authority: SeriesOccurrenceAuthorityV5::Terminal,
        };
        assert!(validate_action_source(SeriesActionV3::Retire, retire_source).is_ok());
        assert!(validate_action_source(SeriesActionV3::Close, close_source).is_ok());
        assert_eq!(
            validate_action_source(SeriesActionV3::Close, retire_source).err(),
            Some(SeriesReleaseErrorV5::Authority)
        );
        assert_eq!(
            validate_action_source(SeriesActionV3::Retire, close_source).err(),
            Some(SeriesReleaseErrorV5::Authority)
        );
    }

    #[test]
    fn action_authority_variants_are_not_cross_action_dtos() {
        let retire = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let hostile = SeriesActionArtifactSourceV5 {
            account_profile: &retire.account_profile,
            request_profile: &retire.request_profile,
            lifecycle: &retire.lifecycle,
            transition: &retire.transition,
            effect: &retire.effect,
            dynamic_fixed_span_counts: &[],
            authority: SeriesOccurrenceAuthorityV5::Prepare {
                market: [1; 32],
                generation: 1,
                release_set: [2; 32],
                parent_root: [3; 32],
            },
        };
        assert_eq!(
            validate_action_source(SeriesActionV3::Retire, hostile).err(),
            Some(SeriesReleaseErrorV5::Authority)
        );
    }
}
