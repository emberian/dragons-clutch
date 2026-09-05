//! Production acquisition of the exact physical bank selected by Series V5.
//!
//! RPC supplies account observations, never privileges or aliases.  This
//! module authenticates the current release bodies and finalized records,
//! derives every meta from the selected `AccountProfileV3`, and returns the
//! existing [`SeriesCurrentHotStateV5`] consumed by the Series inspector.  It
//! owns no action choice, artifact DTO, or persisted protocol fact.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::*,
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_market::execution_strategy::{
    shadow_digest_v3::{
        AcceleratorCallerKindV1, accelerator_caller_authority_digest_v1, family_request_digest_v3,
    },
    shadow_v3::{SHADOW_CALLER_AUTHORITY_INDEX_V1, ShadowArtifactTupleV3, ShadowRequestV3},
    v2::{
        AuthenticatedInterpreterArtifactsV2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyCertificateV2,
        ExecutionStrategyProgramV2, StrategyDispositionV2,
    },
};
use dclutch_registry::release_set::{ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1};
use dclutch_trading::series::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
};
use dclutch_trading_sbf::series::{
    instruction::SeriesActionV3,
    release_v5::{SeriesActionArtifactViewV5, SeriesOccurrenceAuthorityV5, SeriesSelectedActionV5},
};
use dclutch_vm::account_profile::{
    lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
    v2::PhysicalAccountDataGeometryV2,
    v3::{AccountProfileV3, SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_ID_V3},
};
use dclutch_vm::effect::v5::SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_ID_V5;
use dclutch_vm::request_profile::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1;
use dclutch_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3;
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Finality, Observation, ObservedAccount,
    direct_inline_route_v3::{DirectHotFixedRouteV3, FinalizedRecordRouteV3},
    direct_inline_v3::ObservedAccountMetaV3,
    observation::{FinalizedRecordProof, authenticate_finalized_record},
    series_hot_v3::{
        CheckedSeriesShadowAcceleratorV3, SeriesCurrentHotStateV5, SeriesSelectedRoleKeysV5,
        authenticate_expire_permit_v5,
    },
    series_lifecycle_v3::{
        SeriesLifecycleSnapshotV3, SeriesNextActV3, inspect_series_lifecycle_v3,
    },
};

/// Finalized action records and mutable economic roles observed with the bank.
#[derive(Clone, Copy)]
pub struct SeriesSelectedRecordObservationsV5<'a> {
    /// Current occurrence record; required only for Prepare/Consume/Expire.
    pub occurrence: Option<&'a FinalizedRecordRouteV3>,
    /// Current or terminal immutable Ticket record; absent only for Close.
    pub ticket: Option<&'a FinalizedRecordRouteV3>,
    /// Lifecycle RentCredit selected by Expire/Retire/Close.
    pub rent_credit: Option<&'a ObservedAccount>,
    /// Same-snapshot vacant permit PDA selected only by Expire.
    pub expire_permit: Option<&'a ObservedAccount>,
}

/// Consume-only checked Shadow deployment and request-bound caller authority.
#[derive(Clone, Copy)]
pub struct SeriesConsumeShadowObservationsV5<'a> {
    /// Finalized translation Certificate and vacant cursor.
    pub certificate: &'a FinalizedRecordRouteV3,
    /// Finalized accelerator ArtifactRelease and vacant cursor.
    pub artifact: &'a FinalizedRecordRouteV3,
    /// Current accelerator executable.
    pub accelerator_program: &'a ObservedAccount,
    /// Current accelerator ProgramData.
    pub accelerator_programdata: &'a ObservedAccount,
    /// Request-bound Trading caller-authority PDA.
    pub caller_authority: &'a ObservedAccount,
    /// Release-checker result for the same deployment.
    pub checked: CheckedSeriesShadowAcceleratorV3,
    /// Exact request derived from the interpreted candidate/effect transcript.
    pub request: ShadowRequestV3<'a>,
}

/// Live observations sufficient to assemble one current selected Hot bank.
pub struct SeriesCurrentAcquisitionInputV5<'a> {
    /// Common 39-account Hot frame as named RPC observations.
    pub fixed: &'a DirectHotFixedRouteV3,
    /// Expanded logical AccountProfile observations in exact coordinate order.
    pub runtime_logical_accounts: &'a [ObservedAccount],
    /// Selected immutable and economic role observations.
    pub records: SeriesSelectedRecordObservationsV5<'a>,
    /// Consume-only Shadow selection; absent for interpreted actions.
    pub shadow: Option<SeriesConsumeShadowObservationsV5<'a>>,
    /// Same-slot lifecycle snapshot which selected the action.
    pub lifecycle: SeriesLifecycleSnapshotV3<'a>,
}

/// Named observed accounts resolved from authenticated physical coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesAcquiredRoleObservationsV5 {
    /// Controller Series root.
    pub root: ObservedAccount,
    /// Finalized Template record.
    pub template: ObservedAccount,
    /// Finalized current occurrence, when occurrence-bound.
    pub occurrence: Option<ObservedAccount>,
    /// Finalized immutable Ticket, absent only for Close.
    pub ticket_record: Option<ObservedAccount>,
    /// Selected mutable Ticket replay coordinate, when present.
    pub ticket_state: Option<ObservedAccount>,
    /// Selected lifecycle RentCredit, when present.
    pub rent_credit: Option<ObservedAccount>,
    /// Distinct future occurrence Market, when occurrence-bound.
    pub future_market: Option<ObservedAccount>,
    /// Expire's exact still-vacant, prefunded permit PDA.
    pub expire_permit: Option<ObservedAccount>,
}

/// Existing inspector state plus the named accounts acquisition resolved.
pub struct AcquiredSeriesCurrentHotV5<'a> {
    /// Exact input consumed by [`crate::series_hot_v3::inspect_current_series_hot_v5`].
    pub state: SeriesCurrentHotStateV5<'a>,
    /// Named accounts for operator and terminal reporting.
    pub roles: SeriesAcquiredRoleObservationsV5,
}

/// Stable refusal from production account acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesCurrentAcquisitionErrorV5 {
    /// Lifecycle did not select this exact ready action.
    Lifecycle,
    /// Current artifact bodies or descriptor identities differed.
    Artifact,
    /// Finalized record PDA, cursor, owner, rent, or digest differed.
    FinalizedRecord,
    /// Accounts did not share one finalized observation.
    Observation,
    /// Common Hot account identity, width, owner, or privilege differed.
    FixedFrame,
    /// Logical aliases, representatives, widths, or privileges differed.
    RuntimeProfile,
    /// Occurrence/Ticket/RentCredit/permit roles differed.
    Role,
    /// Consume Shadow Certificate, deployment, request, or caller differed.
    Strategy,
    /// `dclutch_trading_sbf` refused; the cause is its own.
    SeriesOperator(dclutch_trading_sbf::series::operator::SeriesOperatorErrorV3),
    /// `dclutch_market::execution_strategy` refused; the cause is its own.
    ExecutionStrategy(dclutch_market::execution_strategy::v2::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_operator` refused; the cause is its own.
    ObservationError(crate::observation::ObservationError),
    /// `dclutch_vm::account_profile` refused; the cause is its own.
    AccountProfileV3(dclutch_vm::account_profile::v3::ErrorV3),
    /// `dclutch_vm::account_profile` refused; the cause is its own.
    AccountProfile(dclutch_vm::account_profile::v2::Error),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_market::execution_strategy` refused; the cause is its own.
    ShadowDigest(dclutch_market::execution_strategy::shadow_digest_v3::ShadowDigestErrorV3),
    /// `dclutch_operator` refused; the cause is its own.
    SeriesHotOperator(crate::series_hot_v3::SeriesHotOperatorErrorV3),
}

/// Assemble current live RPC observations into the existing Series V5 state.
pub fn acquire_current_series_hot_v5<'a>(
    selected: &SeriesSelectedActionV5,
    artifacts: SeriesActionArtifactViewV5<'_>,
    input: SeriesCurrentAcquisitionInputV5<'a>,
) -> Result<AcquiredSeriesCurrentHotV5<'a>, SeriesCurrentAcquisitionErrorV5> {
    require_selected_lifecycle(selected.action, input.lifecycle)?;
    require_artifact_view(selected, artifacts)?;
    let observation = input.fixed.market.observation;
    if observation.finality != Finality::Finalized {
        return Err(SeriesCurrentAcquisitionErrorV5::Observation);
    }
    let fixed_accounts =
        assemble_fixed_accounts(selected, input.fixed, input.lifecycle, observation)?;
    let runtime_physical_accounts = pack_runtime_accounts(
        selected,
        artifacts.account_profile(),
        artifacts.dynamic_fixed_span_counts(),
        input.runtime_logical_accounts,
        &fixed_accounts,
        observation,
    )?;
    let strategy_accounts =
        assemble_strategy_accounts(selected, input.fixed, input.shadow, observation)?;
    require_one_observation(
        strategy_accounts
            .iter()
            .chain(runtime_physical_accounts.iter())
            .map(|value| &value.account),
        observation,
    )?;
    let role_keys = resolve_role_keys(selected, &runtime_physical_accounts)?;
    let roles = authenticate_selected_records(
        selected,
        input.fixed,
        input.records,
        input.lifecycle,
        &fixed_accounts,
        &runtime_physical_accounts,
        role_keys,
        observation,
    )?;
    Ok(AcquiredSeriesCurrentHotV5 {
        state: SeriesCurrentHotStateV5 {
            fixed_accounts,
            strategy_accounts,
            runtime_physical_accounts,
            lifecycle: input.lifecycle,
            permit: roles.expire_permit.clone(),
        },
        roles,
    })
}

fn require_selected_lifecycle(
    action: SeriesActionV3,
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    match inspect_series_lifecycle_v3(lifecycle)
        .map_err(SeriesCurrentAcquisitionErrorV5::SeriesOperator)?
        .next()
    {
        SeriesNextActV3::Ready(plan) if plan.action() == action => Ok(()),
        _ => Err(SeriesCurrentAcquisitionErrorV5::Lifecycle),
    }
}

fn require_artifact_view(
    selected: &SeriesSelectedActionV5,
    artifacts: SeriesActionArtifactViewV5<'_>,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    if artifacts.account_profile() != selected.artifacts.account_profile
        || artifacts.request_profile() != selected.artifacts.request_profile
        || artifacts.lifecycle() != selected.artifacts.lifecycle
        || artifacts.transition() != selected.artifacts.transition
        || artifacts.effect() != selected.artifacts.effect
        || hash(artifacts.account_profile()).to_bytes() != selected.artifact_ids.account_profile
        || hash(artifacts.request_profile()).to_bytes() != selected.artifact_ids.request_profile
        || hash(artifacts.lifecycle()).to_bytes() != selected.artifact_ids.lifecycle
        || hash(artifacts.transition()).to_bytes() != selected.artifact_ids.transition
        || hash(artifacts.effect()).to_bytes() != selected.artifact_ids.effect
        || hash(&selected.artifacts.strategy).to_bytes() != selected.artifact_ids.strategy
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Artifact);
    }
    let strategy = ExecutionStrategyProgramV2::decode(&selected.artifacts.strategy)
        .map_err(SeriesCurrentAcquisitionErrorV5::ExecutionStrategy)?;
    let expected = if selected.action == SeriesActionV3::Consume {
        StrategyDispositionV2::ShadowAot
    } else {
        StrategyDispositionV2::Interpreted
    };
    if strategy.disposition() != expected {
        return Err(SeriesCurrentAcquisitionErrorV5::Artifact);
    }
    Ok(())
}

fn assemble_fixed_accounts(
    selected: &SeriesSelectedActionV5,
    fixed: &DirectHotFixedRouteV3,
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
    observation: Observation,
) -> Result<Vec<ObservedAccountMetaV3>, SeriesCurrentAcquisitionErrorV5> {
    let mut output = vec![None; HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut put = |index: usize, account: &ObservedAccount, writable: bool| {
        if account.observation != observation || account.observation.finality != Finality::Finalized
        {
            return Err(SeriesCurrentAcquisitionErrorV5::Observation);
        }
        let slot = output
            .get_mut(index)
            .ok_or(SeriesCurrentAcquisitionErrorV5::FixedFrame)?;
        if slot.is_some() {
            return Err(SeriesCurrentAcquisitionErrorV5::FixedFrame);
        }
        *slot = Some(ObservedAccountMetaV3 {
            account: account.clone(),
            is_signer: false,
            is_writable: writable,
        });
        Ok(())
    };
    put(HOT_MARKET_ACCOUNT_V3, &fixed.market, false)?;
    put(HOT_ROOT_ACCOUNT_V3, &fixed.root, true)?;
    for (raw, staging, record) in [
        (
            HOT_MANIFEST_RAW_ACCOUNT_V3,
            HOT_MANIFEST_STAGING_ACCOUNT_V3,
            &fixed.manifest,
        ),
        (
            HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
            HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
            &fixed.program_set,
        ),
        (
            HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            &fixed.descriptor,
        ),
        (
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_CONFIG_STAGING_ACCOUNT_V3,
            &fixed.config,
        ),
        (
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
            &fixed.account_profile,
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
            &fixed.request_profile,
        ),
        (
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            HOT_TRANSITION_STAGING_ACCOUNT_V3,
            &fixed.transition,
        ),
        (
            HOT_EFFECT_RAW_ACCOUNT_V3,
            HOT_EFFECT_STAGING_ACCOUNT_V3,
            &fixed.effect,
        ),
        (
            HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
            &fixed.lifecycle,
        ),
        (
            HOT_STRATEGY_RAW_ACCOUNT_V3,
            HOT_STRATEGY_STAGING_ACCOUNT_V3,
            &fixed.strategy,
        ),
        (
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PRODUCT_STAGING_ACCOUNT_V3,
            &fixed.product,
        ),
        (
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3,
            &fixed.result_domain,
        ),
        (
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3,
            &fixed.portfolio,
        ),
        (
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
            &fixed.linked_basis,
        ),
    ] {
        put(raw, &record.raw, false)?;
        put(staging, &record.staging, false)?;
    }
    for (index, account) in [
        (HOT_ACTIVATION_CACHE_ACCOUNT_V3, &fixed.activation_cache),
        (HOT_CORE_PROGRAM_ACCOUNT_V3, &fixed.core_program),
        (HOT_CORE_PROGRAMDATA_ACCOUNT_V3, &fixed.core_programdata),
        (HOT_TRADING_PROGRAM_ACCOUNT_V3, &fixed.trading_program),
        (
            HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            &fixed.trading_programdata,
        ),
        (HOT_REGISTRY_PROGRAM_ACCOUNT_V3, &fixed.registry_program),
        (HOT_RENT_SYSVAR_ACCOUNT_V3, &fixed.rent_sysvar),
        (
            HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
            &fixed.instructions_sysvar,
        ),
        (HOT_CAPABILITY_SEAL_ACCOUNT_V3, &fixed.capability_seal),
    ] {
        put(index, account, false)?;
    }
    drop(put);
    let output = output
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(SeriesCurrentAcquisitionErrorV5::FixedFrame)?;
    require_distinct_keys(
        output.iter().map(|value| value.account.key),
        SeriesCurrentAcquisitionErrorV5::FixedFrame,
    )?;
    authenticate_fixed_release(selected, fixed, lifecycle)?;
    Ok(output)
}

fn authenticate_fixed_release(
    selected: &SeriesSelectedActionV5,
    fixed: &DirectHotFixedRouteV3,
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    if fixed.root.owner != fixed.trading_program.key
        || fixed.root.executable
        || !fixed.trading_program.executable
        || fixed.trading_program.owner != bpf_loader_upgradeable::ID
        || fixed.trading_programdata.owner != bpf_loader_upgradeable::ID
        || fixed.trading_programdata.executable
        || !fixed.core_program.executable
        || fixed.core_program.owner != bpf_loader_upgradeable::ID
        || fixed.core_programdata.owner != bpf_loader_upgradeable::ID
        || fixed.core_programdata.executable
        || !fixed.registry_program.executable
        || fixed.rent_sysvar.key != sysvar::rent::ID
        || fixed.instructions_sysvar.key != sysvar::instructions::ID
        || fixed.config.raw.data.as_slice() != lifecycle.template_bytes
        || fixed.account_profile.raw.data != selected.artifacts.account_profile
        || fixed.request_profile.raw.data != selected.artifacts.request_profile
        || fixed.transition.raw.data != selected.artifacts.transition
        || fixed.effect.raw.data != selected.artifacts.effect
        || fixed.lifecycle.raw.data != selected.artifacts.lifecycle
        || fixed.strategy.raw.data.as_slice() != selected.artifacts.strategy
        || fixed.descriptor.raw.data.as_slice() != selected.descriptor
    {
        return Err(SeriesCurrentAcquisitionErrorV5::FixedFrame);
    }
    let header = CapabilityRootHeaderV1::decode(
        fixed
            .root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesCurrentAcquisitionErrorV5::FixedFrame)?,
    )
    .map_err(SeriesCurrentAcquisitionErrorV5::CapabilityProgram)?;
    if header.market() != fixed.market.key.to_bytes()
        || header.selection().manifest().to_bytes() != hash(&fixed.manifest.raw.data).to_bytes()
        || header.selection().capability_release().to_bytes()
            != hash(&fixed.program_set.raw.data).to_bytes()
        || header.selection().config().to_bytes() != hash(&fixed.config.raw.data).to_bytes()
    {
        return Err(SeriesCurrentAcquisitionErrorV5::FixedFrame);
    }
    for (record, schema, digest) in [
        (
            &fixed.manifest,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            header.selection().manifest().to_bytes(),
        ),
        (
            &fixed.program_set,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            header.selection().capability_release().to_bytes(),
        ),
        (
            &fixed.descriptor,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            hash(&selected.descriptor).to_bytes(),
        ),
        (
            &fixed.config,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            hash(lifecycle.template_bytes).to_bytes(),
        ),
        (
            &fixed.account_profile,
            ACCOUNT_PROFILE_SCHEMA_ID_V3,
            selected.artifact_ids.account_profile,
        ),
        (
            &fixed.request_profile,
            REQUEST_PROFILE_SCHEMA_ID_V1,
            selected.artifact_ids.request_profile,
        ),
        (
            &fixed.transition,
            TRANSITION_SCHEMA_ID_V3,
            selected.artifact_ids.transition,
        ),
        (
            &fixed.effect,
            EFFECT_SCHEMA_ID_V5,
            selected.artifact_ids.effect,
        ),
        (
            &fixed.lifecycle,
            CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
            selected.artifact_ids.lifecycle,
        ),
        (
            &fixed.strategy,
            EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
            selected.artifact_ids.strategy,
        ),
    ] {
        authenticate_record(fixed.registry_program.key, record, schema, digest)?;
    }
    Ok(())
}

fn authenticate_record(
    registry: Pubkey,
    record: &FinalizedRecordRouteV3,
    schema: [u8; 32],
    expected_digest: [u8; 32],
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    if hash(&record.raw.data).to_bytes() != expected_digest {
        return Err(SeriesCurrentAcquisitionErrorV5::FinalizedRecord);
    }
    authenticate_finalized_record(
        registry,
        &record.raw,
        &FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: record.staging.clone(),
        },
    )
    .map_err(SeriesCurrentAcquisitionErrorV5::ObservationError)
}

fn pack_runtime_accounts(
    selected: &SeriesSelectedActionV5,
    account_profile: &[u8],
    dynamic_fixed_span_counts: &[u32],
    logical: &[ObservedAccount],
    fixed: &[ObservedAccountMetaV3],
    observation: Observation,
) -> Result<Vec<ObservedAccountMetaV3>, SeriesCurrentAcquisitionErrorV5> {
    if logical.len() != selected.geometry.logical_accounts
        || selected.account_bindings.len() != logical.len()
    {
        return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
    }
    let profile = AccountProfileV3::decode(account_profile)
        .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfileV3)?;
    let spans = dynamic_fixed_span_counts;
    let base = profile.base();
    let logical_count = base
        .logical_account_count_with_dynamic_spans(0, spans)
        .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfile)?;
    let physical_count = base
        .physical_account_count_with_dynamic_spans(0, spans)
        .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfile)?;
    if logical_count != logical.len() || physical_count != selected.geometry.physical_accounts {
        return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
    }
    let mut packed: Vec<Option<ObservedAccount>> = vec![None; physical_count];
    for (coordinate, account) in logical.iter().enumerate() {
        if account.observation != observation || account.observation.finality != Finality::Finalized
        {
            return Err(SeriesCurrentAcquisitionErrorV5::Observation);
        }
        let representative = base
            .representative_with_dynamic_spans(0, spans, coordinate)
            .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfile)?;
        let ordinal = base
            .physical_account_ordinal_with_dynamic_spans(0, spans, coordinate)
            .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfile)?;
        if selected.account_bindings.get(coordinate).map(|binding| {
            (
                binding.logical,
                binding.representative,
                binding.physical_ordinal,
            )
        }) != Some((coordinate, representative, ordinal))
        {
            return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
        }
        let slot = packed
            .get_mut(ordinal)
            .ok_or(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)?;
        if slot.as_ref().is_some_and(|existing| existing != account) {
            return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
        }
        if slot.is_none() {
            *slot = Some(account.clone());
        }
    }
    let packed = packed
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)?;
    require_distinct_keys(
        packed.iter().map(|value| value.key),
        SeriesCurrentAcquisitionErrorV5::RuntimeProfile,
    )?;
    let mut output = Vec::with_capacity(physical_count);
    for (ordinal, account) in packed.into_iter().enumerate() {
        let geometry = base
            .physical_account_geometry_with_dynamic_spans(0, spans, ordinal)
            .map_err(SeriesCurrentAcquisitionErrorV5::AccountProfile)?;
        let privileges = geometry.privileges();
        let width_ok = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes } => account.data.len() == bytes,
            PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
                account.data.is_empty() || account.data.len() == live_bytes
            }
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                !account.data.is_empty() && account.data.len() >= minimum_bytes
            }
            PhysicalAccountDataGeometryV2::Opaque => true,
        };
        if account.executable != privileges.executable() || !width_ok {
            return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
        }
        output.push(ObservedAccountMetaV3 {
            account,
            is_signer: privileges.signer(),
            is_writable: privileges.writable(),
        });
    }
    for (ordinal, fixed_coordinate) in [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ]
    .into_iter()
    .enumerate()
    {
        if output.get(ordinal) != fixed.get(fixed_coordinate) {
            return Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile);
        }
    }
    Ok(output)
}

fn resolve_role_keys(
    selected: &SeriesSelectedActionV5,
    physical: &[ObservedAccountMetaV3],
) -> Result<SeriesSelectedRoleKeysV5, SeriesCurrentAcquisitionErrorV5> {
    let key = |coordinate: u16| {
        selected
            .account_bindings
            .get(usize::from(coordinate))
            .and_then(|binding| physical.get(binding.physical_ordinal))
            .map(|value| value.account.key)
            .ok_or(SeriesCurrentAcquisitionErrorV5::Role)
    };
    let optional = |coordinate: Option<u16>| coordinate.map(key).transpose();
    let (occurrence_market, occurrence_generation) = match selected.authority {
        SeriesOccurrenceAuthorityV5::Prepare {
            market, generation, ..
        }
        | SeriesOccurrenceAuthorityV5::Consume {
            market, generation, ..
        }
        | SeriesOccurrenceAuthorityV5::Expire {
            market, generation, ..
        } => (Some(Pubkey::new_from_array(market)), Some(generation)),
        SeriesOccurrenceAuthorityV5::Terminal => (None, None),
    };
    Ok(SeriesSelectedRoleKeysV5 {
        root: key(selected.roles.root)?,
        ticket: optional(selected.roles.ticket)?,
        rent_credit: optional(selected.roles.rent_credit)?,
        payer: optional(selected.roles.payer)?,
        refund: optional(selected.roles.refund)?,
        system_program: optional(selected.roles.system_program)?,
        occurrence_market,
        occurrence_generation,
        permit: None,
    })
}

fn assemble_strategy_accounts(
    selected: &SeriesSelectedActionV5,
    fixed: &DirectHotFixedRouteV3,
    shadow: Option<SeriesConsumeShadowObservationsV5<'_>>,
    observation: Observation,
) -> Result<Vec<ObservedAccountMetaV3>, SeriesCurrentAcquisitionErrorV5> {
    if selected.action != SeriesActionV3::Consume {
        if shadow.is_some() {
            return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
        }
        return Ok(Vec::new());
    }
    let shadow = shadow.ok_or(SeriesCurrentAcquisitionErrorV5::Strategy)?;
    let accounts = [
        &shadow.certificate.raw,
        &shadow.certificate.staging,
        &shadow.artifact.raw,
        &shadow.artifact.staging,
        shadow.accelerator_program,
        shadow.accelerator_programdata,
        shadow.caller_authority,
    ];
    require_one_observation(accounts.into_iter(), observation)?;
    require_distinct_keys(
        accounts.into_iter().map(|account| account.key),
        SeriesCurrentAcquisitionErrorV5::Strategy,
    )?;
    let strategy = ExecutionStrategyProgramV2::decode(&selected.artifacts.strategy)
        .map_err(SeriesCurrentAcquisitionErrorV5::ExecutionStrategy)?;
    let descriptor = CapabilityProgramV4::decode(&selected.descriptor)
        .map_err(SeriesCurrentAcquisitionErrorV5::CapabilityProgram)?;
    let strategy_id = content(&selected.artifacts.strategy)?;
    let certificate_id = content(&shadow.certificate.raw.data)?;
    if strategy.disposition() != StrategyDispositionV2::ShadowAot
        || strategy.certificate_program() != Some(certificate_id)
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
    }
    authenticate_record(
        fixed.registry_program.key,
        shadow.certificate,
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        certificate_id.to_bytes(),
    )?;
    let certificate = ExecutionStrategyCertificateV2::decode(&shadow.certificate.raw.data)
        .map_err(SeriesCurrentAcquisitionErrorV5::ExecutionStrategy)?;
    certificate
        .validate_v4(
            certificate_id,
            strategy_id,
            strategy,
            descriptor,
            AuthenticatedInterpreterArtifactsV2 {
                account_profile_program: content(&selected.artifacts.account_profile)?,
                request_profile_schema: descriptor.request_profile().schema(),
                request_profile_program: content(&selected.artifacts.request_profile)?,
                transition_schema: strategy.transition_schema(),
                transition_program: content(&selected.artifacts.transition)?,
                effect_program: content(&selected.artifacts.effect)?,
            },
        )
        .map_err(SeriesCurrentAcquisitionErrorV5::ExecutionStrategy)?;
    let artifact_digest = hash(&shadow.artifact.raw.data).to_bytes();
    if shadow.checked.artifact_release != artifact_digest
        || shadow.checked.artifact_release == [0; 32]
        || shadow.checked.checked_manifest_digest == [0; 32]
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
    }
    authenticate_record(
        fixed.registry_program.key,
        shadow.artifact,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        artifact_digest,
    )?;
    let artifact_id = ArtifactReleaseIdV1::new(artifact_digest)
        .map_err(SeriesCurrentAcquisitionErrorV5::ReleaseSet)?;
    certificate
        .validate_artifact(artifact_id)
        .map_err(SeriesCurrentAcquisitionErrorV5::ExecutionStrategy)?;
    let artifact = ArtifactReleaseV1::decode(&shadow.artifact.raw.data)
        .map_err(SeriesCurrentAcquisitionErrorV5::Registry)?;
    if shadow.accelerator_program.key != shadow.checked.accelerator_program
        || shadow.accelerator_programdata.key != shadow.checked.accelerator_programdata
        || artifact.program().to_bytes() != shadow.accelerator_program.key.to_bytes()
        || artifact.programdata() != shadow.accelerator_programdata.key.to_bytes()
        || artifact.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || shadow.accelerator_program.owner != bpf_loader_upgradeable::ID
        || !shadow.accelerator_program.executable
        || shadow.accelerator_programdata.owner != bpf_loader_upgradeable::ID
        || shadow.accelerator_programdata.executable
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
    }
    require_shadow_request(
        selected,
        fixed,
        shadow,
        descriptor,
        strategy_id,
        certificate_id,
    )?;
    Ok(accounts
        .into_iter()
        .map(|account| ObservedAccountMetaV3 {
            account: account.clone(),
            is_signer: false,
            is_writable: false,
        })
        .collect())
}

fn require_shadow_request(
    selected: &SeriesSelectedActionV5,
    fixed: &DirectHotFixedRouteV3,
    shadow: SeriesConsumeShadowObservationsV5<'_>,
    descriptor: CapabilityProgramV4,
    strategy_id: ContentId,
    certificate_id: ContentId,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    let header = CapabilityRootHeaderV1::decode(
        fixed
            .root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesCurrentAcquisitionErrorV5::Strategy)?,
    )
    .map_err(SeriesCurrentAcquisitionErrorV5::CapabilityProgram)?;
    let expected_artifacts = ShadowArtifactTupleV3 {
        capability_program: content(&selected.descriptor)?,
        account_profile: descriptor.account_profile().program(),
        request_profile: descriptor.request_profile().program(),
        transition: descriptor.transition().program(),
        effect: descriptor.effect().program(),
        strategy: strategy_id,
        certificate: certificate_id,
    };
    let request = shadow.request;
    if request.release_set.to_bytes() != header.release_set().to_bytes()
        || request.market.to_bytes() != header.market()
        || request.root.to_bytes() != fixed.root.key.to_bytes()
        || request.registry_program.to_bytes() != fixed.registry_program.key.to_bytes()
        || request.trading_program.to_bytes() != fixed.trading_program.key.to_bytes()
        || request.accelerator_program.to_bytes() != shadow.accelerator_program.key.to_bytes()
        || request.artifacts != expected_artifacts
        || request.family_request != selected.request_bytes
        || usize::try_from(request.shape.account_count)
            .map_err(|_| SeriesCurrentAcquisitionErrorV5::Strategy)?
            != selected.geometry.physical_accounts
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
    }
    // THE SIGNED FAMILY REQUEST, not the encoded `ShadowRequestV3`. This host
    // used to encode the whole request and hash it, which meant it could only
    // state this address for a request whose register bank it had already
    // reproduced byte-for-byte -- and for a window-gated profile that bank
    // carries `Clock::get().slot`, so the address it stated was valid for one
    // slot. See `accelerator_caller_authority_digest_v1`.
    let seeds = CallerAuthoritySeedsV1::new(
        request.release_set,
        request.market.to_bytes(),
        ExecutionRoleV1::Trading,
        request.root.to_bytes(),
        accelerator_caller_authority_digest_v1(
            AcceleratorCallerKindV1::Shadow,
            family_request_digest_v3(request.family_request)
                .map_err(SeriesCurrentAcquisitionErrorV5::ShadowDigest)?,
            SHADOW_CALLER_AUTHORITY_INDEX_V1,
        )
        .map_err(SeriesCurrentAcquisitionErrorV5::ShadowDigest)?
        .to_bytes(),
    )
    .map_err(SeriesCurrentAcquisitionErrorV5::ReleaseSet)?;
    if Pubkey::find_program_address(&seeds.as_slices(), &fixed.trading_program.key).0
        != shadow.caller_authority.key
        || shadow.caller_authority.executable
        || !shadow.caller_authority.data.is_empty()
        || shadow.caller_authority.owner != system_program::ID
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Strategy);
    }
    Ok(())
}

fn content(bytes: &[u8]) -> Result<ContentId, SeriesCurrentAcquisitionErrorV5> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| SeriesCurrentAcquisitionErrorV5::Strategy)
}

fn authenticate_selected_records(
    selected: &SeriesSelectedActionV5,
    fixed: &DirectHotFixedRouteV3,
    records: SeriesSelectedRecordObservationsV5<'_>,
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
    fixed_accounts: &[ObservedAccountMetaV3],
    physical: &[ObservedAccountMetaV3],
    role_keys: SeriesSelectedRoleKeysV5,
    observation: Observation,
) -> Result<SeriesAcquiredRoleObservationsV5, SeriesCurrentAcquisitionErrorV5> {
    let occurrence_bound = matches!(
        selected.action,
        SeriesActionV3::Prepare | SeriesActionV3::Consume | SeriesActionV3::Expire
    );
    let (expected_occurrence, expected_ticket) = match selected.action {
        SeriesActionV3::Prepare | SeriesActionV3::Consume | SeriesActionV3::Expire => {
            let current = lifecycle
                .current
                .ok_or(SeriesCurrentAcquisitionErrorV5::Role)?;
            (Some(current.occurrence_bytes), Some(current.ticket_bytes))
        }
        SeriesActionV3::Retire => {
            let terminal = lifecycle
                .terminal_ticket
                .ok_or(SeriesCurrentAcquisitionErrorV5::Role)?;
            (None, Some(terminal.ticket_bytes))
        }
        SeriesActionV3::Close => (None, None),
    };
    let occurrence = match (records.occurrence, expected_occurrence) {
        (Some(record), Some(expected)) if record.raw.data.as_slice() == expected => {
            authenticate_record(
                fixed.registry_program.key,
                record,
                SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
                hash(expected).to_bytes(),
            )?;
            require_physical_account(physical, &record.raw)?;
            Some(record.raw.clone())
        }
        (None, None) => None,
        _ => return Err(SeriesCurrentAcquisitionErrorV5::Role),
    };
    let ticket_record = match (records.ticket, expected_ticket) {
        (Some(record), Some(expected)) if record.raw.data.as_slice() == expected => {
            authenticate_record(
                fixed.registry_program.key,
                record,
                SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
                hash(expected).to_bytes(),
            )?;
            require_physical_account(physical, &record.raw)?;
            Some(record.raw.clone())
        }
        (None, None) => None,
        _ => return Err(SeriesCurrentAcquisitionErrorV5::Role),
    };
    if occurrence_bound != occurrence.is_some()
        || (selected.action != SeriesActionV3::Close) != ticket_record.is_some()
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Role);
    }
    let root = physical_account_by_key(physical, role_keys.root)?;
    if root != &fixed.root {
        return Err(SeriesCurrentAcquisitionErrorV5::Role);
    }
    let ticket_state = role_keys
        .ticket
        .map(|key| physical_account_by_key(physical, key).cloned())
        .transpose()?;
    let rent_credit = match (role_keys.rent_credit, records.rent_credit) {
        (Some(key), Some(account))
            if key == account.key
                && account.observation == observation
                && lifecycle
                    .rent_sink
                    .map(|sink| sink.credit_account().to_bytes())
                    == Some(key.to_bytes()) =>
        {
            require_physical_account(physical, account)?;
            Some(account.clone())
        }
        (None, None) => None,
        _ => return Err(SeriesCurrentAcquisitionErrorV5::Role),
    };
    let header = CapabilityRootHeaderV1::decode(
        fixed
            .root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesCurrentAcquisitionErrorV5::FixedFrame)?,
    )
    .map_err(SeriesCurrentAcquisitionErrorV5::CapabilityProgram)?;
    let (future_market, expire_authority) = match selected.authority {
        SeriesOccurrenceAuthorityV5::Prepare {
            market,
            generation,
            release_set,
            parent_root,
        }
        | SeriesOccurrenceAuthorityV5::Consume {
            market,
            generation,
            release_set,
            parent_root,
        } => {
            require_occurrence_authority(
                fixed,
                physical,
                header,
                market,
                generation,
                release_set,
                parent_root,
            )?;
            (
                Some(physical_account_by_key(physical, Pubkey::new_from_array(market))?.clone()),
                None,
            )
        }
        SeriesOccurrenceAuthorityV5::Expire {
            market,
            generation,
            release_set,
            parent_root,
        } => {
            require_occurrence_authority(
                fixed,
                physical,
                header,
                market,
                generation,
                release_set,
                parent_root,
            )?;
            (
                Some(physical_account_by_key(physical, Pubkey::new_from_array(market))?.clone()),
                Some((market, release_set)),
            )
        }
        SeriesOccurrenceAuthorityV5::Terminal => {
            if role_keys.occurrence_market.is_some() || role_keys.occurrence_generation.is_some() {
                return Err(SeriesCurrentAcquisitionErrorV5::Role);
            }
            (None, None)
        }
    };
    let expire_permit = match (records.expire_permit, expire_authority) {
        (Some(observed), Some((market, release_set))) => {
            let SeriesOccurrenceAuthorityV5::Expire {
                market: selected_market,
                release_set: selected_release,
                ..
            } = selected.authority
            else {
                return Err(SeriesCurrentAcquisitionErrorV5::Role);
            };
            if market != selected_market || release_set != selected_release {
                return Err(SeriesCurrentAcquisitionErrorV5::Role);
            }
            authenticate_expire_permit_v5(
                selected,
                fixed_accounts
                    .get(HOT_CORE_PROGRAM_ACCOUNT_V3)
                    .ok_or(SeriesCurrentAcquisitionErrorV5::FixedFrame)?,
                &fixed.rent_sysvar,
                observation,
                observed,
                physical,
            )
            .map_err(SeriesCurrentAcquisitionErrorV5::SeriesHotOperator)?;
            Some(observed.clone())
        }
        (None, None) => None,
        _ => return Err(SeriesCurrentAcquisitionErrorV5::Role),
    };
    Ok(SeriesAcquiredRoleObservationsV5 {
        root: fixed.root.clone(),
        template: fixed.config.raw.clone(),
        occurrence,
        ticket_record,
        ticket_state,
        rent_credit,
        future_market,
        expire_permit,
    })
}

fn require_occurrence_authority(
    fixed: &DirectHotFixedRouteV3,
    physical: &[ObservedAccountMetaV3],
    header: CapabilityRootHeaderV1,
    market: [u8; 32],
    generation: u64,
    release_set: [u8; 32],
    parent_root: [u8; 32],
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    let future = physical_account_by_key(physical, Pubkey::new_from_array(market))?;
    if parent_root != fixed.root.key.to_bytes()
        || release_set != header.release_set().to_bytes()
        || generation == 0
        || market == header.market()
        || future.owner != system_program::ID
        || future.executable
        || !future.data.is_empty()
    {
        return Err(SeriesCurrentAcquisitionErrorV5::Role);
    }
    Ok(())
}

fn require_physical_account(
    physical: &[ObservedAccountMetaV3],
    expected: &ObservedAccount,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    let mut matches = physical.iter().filter(|value| value.account == *expected);
    if matches.next().is_none() || matches.next().is_some() {
        return Err(SeriesCurrentAcquisitionErrorV5::Role);
    }
    Ok(())
}

fn physical_account_by_key(
    physical: &[ObservedAccountMetaV3],
    key: Pubkey,
) -> Result<&ObservedAccount, SeriesCurrentAcquisitionErrorV5> {
    let mut matches = physical.iter().filter(|value| value.account.key == key);
    let value = matches
        .next()
        .ok_or(SeriesCurrentAcquisitionErrorV5::Role)?;
    if matches.next().is_some() {
        return Err(SeriesCurrentAcquisitionErrorV5::Role);
    }
    Ok(&value.account)
}

fn require_one_observation<'a>(
    accounts: impl IntoIterator<Item = &'a ObservedAccount>,
    observation: Observation,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    if accounts.into_iter().any(|account| {
        account.observation != observation || account.observation.finality != Finality::Finalized
    }) {
        Err(SeriesCurrentAcquisitionErrorV5::Observation)
    } else {
        Ok(())
    }
}

fn require_distinct_keys(
    keys: impl IntoIterator<Item = Pubkey>,
    error: SeriesCurrentAcquisitionErrorV5,
) -> Result<(), SeriesCurrentAcquisitionErrorV5> {
    let mut seen = Vec::new();
    for key in keys {
        if seen.contains(&key) {
            return Err(error);
        }
        seen.push(key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
    use dclutch_custody::{
        CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyOperationV1,
        ProjectedCustodyRequestV1,
    };
    use dclutch_market::rent::{
        RefundAuthority,
        lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
    };
    use dclutch_market::{
        FoundingIntentV5, Identity as CoreIdentity, SERIES_FOUNDING_PERMIT_BYTES_V1,
        SeriesCoreActionV1, SeriesCoreRequestV1, SeriesFoundingPermitSeedsV1,
        SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
    };
    use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
    use dclutch_trading::series::{
        AccountKeyV3, SERIES_OCCURRENCE_BYTES_V3, SERIES_TICKET_BYTES_V3, admit_occurrence,
        admit_ticket, generated, occurrence_content_id,
        replay::{SeriesStateV3, TicketStateV3},
        template_content_id,
        terminal::SeriesLifecycleRentSinkV3,
    };
    use dclutch_trading_sbf::series::{
        account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
        artifacts_v3::{
            SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        },
        consume_artifacts_v4::SeriesConsumeChildRequestsV4,
        expire_funding_artifacts_v5::{
            SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SeriesExpireAccountProfileInputV5,
            SeriesExpireChildRequestsV5,
        },
        occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
        prepare_funding_artifacts_v5::{
            SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
        },
        release_v5::{
            SeriesActionArtifactIdsV5, SeriesCurrentReleaseInputV5, SeriesLogicalPhysicalBindingV5,
            SeriesSelectedArtifactBodiesV5, SeriesSelectedGeometryV5, SeriesSelectedRolesV5,
            authenticate_series_selected_action_v5, compile_series_release_v5,
            emit_current_series_release_source_v5,
        },
        retire_funding_artifacts_v5::emit_series_retire_funding_artifacts_v5,
    };
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};

    fn observation() -> Observation {
        Observation {
            slot: 90,
            unix_timestamp: 91,
            finality: Finality::Finalized,
        }
    }

    fn content_id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("fixture identity")
    }

    fn core_identity(bytes: [u8; 32]) -> CoreIdentity {
        CoreIdentity::new(bytes).expect("fixture Core identity")
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

    fn core_request(
        action: SeriesCoreActionV1,
        template: ContentId,
        ticket: ContentId,
    ) -> [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3] {
        SeriesCoreRequestV1::occurrence(
            action,
            core_identity([1; 32]),
            core_identity(template.to_bytes()),
            core_identity(ticket.to_bytes()),
            core_identity([2; 32]),
            core_identity([20; 32]),
            core_identity([3; 32]),
            core_identity([21; 32]),
            core_identity([5; 32]),
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

    fn current_source(
        template: ContentId,
        ticket: ContentId,
        parent_root: [u8; 32],
    ) -> dclutch_trading_sbf::series::release_v5::SeriesOwnedReleaseSourceV5 {
        let prepare_initialize = projected(ProjectedCustodyOperationV1::Initialize, parent_root)
            .encode()
            .expect("Prepare projected initialize");
        let prepare_open = projected(ProjectedCustodyOperationV1::OpenHoard, parent_root)
            .encode()
            .expect("Prepare projected open");
        let replay_initialize = [32; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let escrow_open = [33; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let escrow_lock = [34; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let consume_lock = projected(
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            parent_root,
        )
        .encode()
        .expect("Consume projected lock");
        let consume_core = core_request(SeriesCoreActionV1::Consume, template, ticket);
        let consume_realize = projected(ProjectedCustodyOperationV1::RealizeAndClose, parent_root)
            .encode()
            .expect("Consume projected realize");
        let consume_claims = claims(47);
        let refund = [37; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_vault = [38; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_replay = [39; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let projected_abort = [40; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let intent = FoundingIntentV5::new(
            255,
            core_identity([1; 32]),
            core_identity([2; 32]),
            core_identity([3; 32]),
            core_identity([4; 32]),
            core_identity([5; 32]),
            core_identity(ticket.to_bytes()),
            core_identity(parent_root),
            core_identity([8; 32]),
            core_identity([9; 32]),
            core_identity([10; 32]),
            core_identity([11; 32]),
            core_identity([12; 32]),
            core_identity([13; 32]),
            core_identity([14; 32]),
            core_identity([15; 32]),
            8,
            1,
            1,
            100,
            4,
            1,
        )
        .expect("founding intent");
        let permit_expiry = SeriesPermitExpiryRequestV1::new(
            SeriesFoundingPermitV1::new(intent, core_identity([16; 32]), core_identity([17; 32]))
                .expect("permit"),
        );
        let expire_core = SeriesCoreRequestV1::decode(&core_request(
            SeriesCoreActionV1::Expire,
            template,
            ticket,
        ))
        .expect("Expire Core");
        let prepare_lengths = [0; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let consume_lengths = [0; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        let mut expire_lengths = [0; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
        expire_lengths[73] = SERIES_OCCURRENCE_BYTES_V3 as u32;
        expire_lengths[75] = SERIES_TICKET_BYTES_V3 as u32;
        emit_current_series_release_source_v5(SeriesCurrentReleaseInputV5 {
            template,
            template_occurrence_count: 1,
            consume_shadow_certificate_program: content_id(90),
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
        .expect("current source")
    }

    fn finalized_record(
        registry: Pubkey,
        rent: &Rent,
        schema: [u8; 32],
        data: Vec<u8>,
    ) -> FinalizedRecordRouteV3 {
        let digest = hash(&data).to_bytes();
        let raw =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &registry,
        )
        .0;
        FinalizedRecordRouteV3 {
            raw: ObservedAccount {
                observation: observation(),
                key: raw,
                owner: registry,
                lamports: rent.minimum_balance(data.len()),
                executable: false,
                data,
            },
            staging: ObservedAccount {
                observation: observation(),
                key: staging,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
        }
    }

    fn rent_account(rent: &Rent) -> ObservedAccount {
        let mut lamports = 1;
        let mut data = vec![0; Rent::size_of()];
        let key = sysvar::rent::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        assert_eq!(rent.clone().to_account_info(&mut info), Some(()));
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    struct ExpireAcquisitionFixture {
        owned: dclutch_trading_sbf::series::release_v5::SeriesOwnedReleaseSourceV5,
        selected: SeriesSelectedActionV5,
        fixed: DirectHotFixedRouteV3,
        logical: Vec<ObservedAccount>,
        occurrence_record: FinalizedRecordRouteV3,
        ticket_record: FinalizedRecordRouteV3,
        rent_credit: ObservedAccount,
        permit: ObservedAccount,
        template: Vec<u8>,
        occurrence: Vec<u8>,
        ticket: Vec<u8>,
        siblings: Vec<[u8; 32]>,
        series: SeriesStateV3,
        ticket_state: TicketStateV3,
        now_slot: u64,
        rent_sink: SeriesLifecycleRentSinkV3,
    }

    impl ExpireAcquisitionFixture {
        fn lifecycle(&self) -> SeriesLifecycleSnapshotV3<'_> {
            SeriesLifecycleSnapshotV3 {
                template_bytes: &self.template,
                series: self.series,
                now_slot: self.now_slot,
                current: Some(crate::series_lifecycle_v3::SeriesCurrentOccurrenceV3 {
                    occurrence_bytes: &self.occurrence,
                    ticket_bytes: &self.ticket,
                    siblings: &self.siblings,
                    ticket_state: Some(self.ticket_state),
                }),
                terminal_ticket: None,
                observed_root_lamports: self.fixed.root.lamports,
                exact_root_rent: 0,
                rent_sink: Some(self.rent_sink),
            }
        }

        fn acquire<'a>(
            &'a self,
            fixed: &'a DirectHotFixedRouteV3,
            logical: &'a [ObservedAccount],
            occurrence: &'a FinalizedRecordRouteV3,
            ticket: &'a FinalizedRecordRouteV3,
            rent_credit: &'a ObservedAccount,
            permit: &'a ObservedAccount,
        ) -> Result<AcquiredSeriesCurrentHotV5<'a>, SeriesCurrentAcquisitionErrorV5> {
            acquire_current_series_hot_v5(
                &self.selected,
                self.owned.action_artifacts(SeriesActionV3::Expire),
                SeriesCurrentAcquisitionInputV5 {
                    fixed,
                    runtime_logical_accounts: logical,
                    records: SeriesSelectedRecordObservationsV5 {
                        occurrence: Some(occurrence),
                        ticket: Some(ticket),
                        rent_credit: Some(rent_credit),
                        expire_permit: Some(permit),
                    },
                    shadow: None,
                    lifecycle: self.lifecycle(),
                },
            )
        }
    }

    fn expire_acquisition_fixture() -> ExpireAcquisitionFixture {
        use dclutch_market::capability_program::SelectedRecordBumpsV1;
        use dclutch_trading::series::TemplateV3;

        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        let siblings = [[90; 32], [91; 32]];
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let mut node = occurrence_id.to_bytes();
        let mut index = 1_u32;
        for sibling in siblings {
            node = if index & 1 == 0 {
                solana_program::hash::hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &[0],
                    &node,
                    &sibling,
                ])
            } else {
                solana_program::hash::hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &[0],
                    &sibling,
                    &node,
                ])
            }
            .to_bytes();
            index >>= 1;
        }
        template[generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3
            ..generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3 + 32]
            .copy_from_slice(&node);
        let template_id = template_content_id(&template).expect("template ID");
        ticket[generated::SERIES_TICKET_TEMPLATE_OFFSET_V3
            ..generated::SERIES_TICKET_TEMPLATE_OFFSET_V3 + 32]
            .copy_from_slice(&template_id.to_bytes());
        ticket[generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3
            ..generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3 + 32]
            .copy_from_slice(&occurrence_id.to_bytes());
        let admitted_occurrence =
            admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
        let ticket_id = admit_ticket(&ticket).expect("ticket").content_id();
        let template_body = TemplateV3::decode(&template).expect("template");
        let now_slot = template_body
            .retry_through(admitted_occurrence.occurrence().occurrence())
            .expect("retry")
            + 1;
        let at_second_occurrence = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("first prepared")
            .settle_current(1, template_body.occurrence_count())
            .expect("first settled")
            .retire_ticket(2)
            .expect("first retired");
        let series = at_second_occurrence
            .prepare_ticket(at_second_occurrence.revision())
            .expect("second prepared");
        let ticket_state = TicketStateV3::prepared(ticket_id);
        let parent_root = Pubkey::new_from_array([200; 32]);
        let owned = current_source(template_id, ticket_id, parent_root.to_bytes());
        let release = compile_series_release_v5(owned.as_source()).expect("release");
        let lifecycle = SeriesLifecycleSnapshotV3 {
            template_bytes: &template,
            series,
            now_slot,
            current: Some(crate::series_lifecycle_v3::SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &occurrence,
                ticket_bytes: &ticket,
                siblings: &siblings,
                ticket_state: Some(ticket_state),
            }),
            terminal_ticket: None,
            observed_root_lamports: 1,
            exact_root_rent: 0,
            rent_sink: None,
        };
        let request = match inspect_series_lifecycle_v3(lifecycle)
            .expect("lifecycle")
            .next()
        {
            SeriesNextActV3::Ready(plan) if plan.action() == SeriesActionV3::Expire => {
                plan.request().as_bytes().to_vec()
            }
            _ => panic!("Expire lifecycle"),
        };
        let selected =
            authenticate_series_selected_action_v5(&release, owned.as_source(), &request)
                .expect("selected Expire");
        let rent = Rent::default();
        let registry = Pubkey::new_from_array([100; 32]);
        let trading = Pubkey::new_from_array([13; 32]);
        let core = Pubkey::new_from_array([15; 32]);
        let controller_market = Pubkey::new_from_array([111; 32]);
        let manifest = finalized_record(
            registry,
            &rent,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            vec![201],
        );
        let program_set = finalized_record(
            registry,
            &rent,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            release.program_set.clone(),
        );
        let descriptor = finalized_record(
            registry,
            &rent,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            selected.descriptor.to_vec(),
        );
        let config = finalized_record(
            registry,
            &rent,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            template.to_vec(),
        );
        let account_profile = finalized_record(
            registry,
            &rent,
            ACCOUNT_PROFILE_SCHEMA_ID_V3,
            selected.artifacts.account_profile.clone(),
        );
        let request_profile = finalized_record(
            registry,
            &rent,
            REQUEST_PROFILE_SCHEMA_ID_V1,
            selected.artifacts.request_profile.clone(),
        );
        let transition = finalized_record(
            registry,
            &rent,
            TRANSITION_SCHEMA_ID_V3,
            selected.artifacts.transition.clone(),
        );
        let effect = finalized_record(
            registry,
            &rent,
            EFFECT_SCHEMA_ID_V5,
            selected.artifacts.effect.clone(),
        );
        let lifecycle_record = finalized_record(
            registry,
            &rent,
            CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
            selected.artifacts.lifecycle.clone(),
        );
        let strategy = finalized_record(
            registry,
            &rent,
            EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
            selected.artifacts.strategy.to_vec(),
        );
        let selection = CapabilityExecutionSelectionV1::from_bytes(
            0,
            hash(&manifest.raw.data).to_bytes(),
            [202; 32],
            release.program_set_id,
            hash(&template).to_bytes(),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            content_id(1),
            controller_market.to_bytes(),
            9,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("root header");
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(
            &series
                .encode(template_body.occurrence_count())
                .expect("root tail"),
        );

        let profile =
            AccountProfileV3::decode(&selected.artifacts.account_profile).expect("Expire profile");
        let base = profile.base();
        let mut physical = (0..selected.geometry.physical_accounts)
            .map(|ordinal| {
                exact_account(
                    ordinal,
                    base.physical_account_geometry_with_dynamic_spans(0, &[], ordinal)
                        .expect("geometry"),
                )
            })
            .collect::<Vec<_>>();
        let set_logical = |physical: &mut [ObservedAccountMetaV3],
                           coordinate: usize,
                           account: ObservedAccount| {
            let ordinal = selected.account_bindings[coordinate].physical_ordinal;
            physical[ordinal].account = account;
        };
        set_logical(
            &mut physical,
            0,
            ObservedAccount {
                observation: observation(),
                key: parent_root,
                owner: trading,
                lamports: 1,
                executable: false,
                data: root_data,
            },
        );
        set_logical(&mut physical, 1, config.raw.clone());
        let product = FinalizedRecordRouteV3 {
            raw: physical[selected.account_bindings[2].physical_ordinal]
                .account
                .clone(),
            staging: exact_account(
                130,
                base.physical_account_geometry_with_dynamic_spans(0, &[], 0)
                    .expect("dummy geometry"),
            )
            .account,
        };
        let portfolio = FinalizedRecordRouteV3 {
            raw: physical[selected.account_bindings[3].physical_ordinal]
                .account
                .clone(),
            staging: exact_account(
                131,
                base.physical_account_geometry_with_dynamic_spans(0, &[], 0)
                    .expect("dummy geometry"),
            )
            .account,
        };
        let linked_basis = FinalizedRecordRouteV3 {
            raw: physical[selected.account_bindings[4].physical_ordinal]
                .account
                .clone(),
            staging: exact_account(
                132,
                base.physical_account_geometry_with_dynamic_spans(0, &[], 0)
                    .expect("dummy geometry"),
            )
            .account,
        };
        let result_domain = FinalizedRecordRouteV3 {
            raw: exact_account(
                133,
                base.physical_account_geometry_with_dynamic_spans(0, &[], 0)
                    .expect("dummy geometry"),
            )
            .account,
            staging: exact_account(
                134,
                base.physical_account_geometry_with_dynamic_spans(0, &[], 0)
                    .expect("dummy geometry"),
            )
            .account,
        };
        let occurrence_record = finalized_record(
            registry,
            &rent,
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            occurrence.to_vec(),
        );
        let ticket_record = finalized_record(
            registry,
            &rent,
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            ticket.to_vec(),
        );
        set_logical(&mut physical, 73, occurrence_record.raw.clone());
        set_logical(&mut physical, 74, occurrence_record.staging.clone());
        set_logical(&mut physical, 75, ticket_record.raw.clone());
        set_logical(&mut physical, 76, ticket_record.staging.clone());
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new([17; 32]).expect("refund"),
            LifecycleAccountIdV2::new([2; 32]).expect("future Market"),
            LifecycleAccountIdV2::new([1; 32]).expect("release"),
            8,
            4,
        )
        .expect("RentCredit");
        let rent_credit = ObservedAccount {
            observation: observation(),
            key: Pubkey::new_from_array([88; 32]),
            owner: Pubkey::new_from_array([89; 32]),
            lamports: 1,
            executable: false,
            data: credit.to_bytes().to_vec(),
        };
        set_logical(&mut physical, 33, rent_credit.clone());
        let rent_sink = SeriesLifecycleRentSinkV3::admit(
            AccountKeyV3::new(rent_credit.key.to_bytes()).expect("credit"),
            &rent_credit.data,
            AccountKeyV3::new([2; 32]).expect("future Market"),
            content_id(1),
            8,
            AccountKeyV3::new([17; 32]).expect("refund"),
        )
        .expect("rent sink");
        set_logical(
            &mut physical,
            54,
            ObservedAccount {
                observation: observation(),
                key: Pubkey::new_from_array([2; 32]),
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
        );
        let permit_seeds = SeriesFoundingPermitSeedsV1::new(
            core_identity([1; 32]),
            core_identity([2; 32]),
            core_identity(ticket_id.to_bytes()),
        );
        let permit = ObservedAccount {
            observation: observation(),
            key: Pubkey::find_program_address(&permit_seeds.as_slices(), &core).0,
            owner: system_program::ID,
            lamports: rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1),
            executable: false,
            data: Vec::new(),
        };
        set_logical(&mut physical, 55, permit.clone());
        let logical = selected
            .account_bindings
            .iter()
            .map(|binding| physical[binding.physical_ordinal].account.clone())
            .collect::<Vec<_>>();
        let program = |key: Pubkey| ObservedAccount {
            observation: observation(),
            key,
            owner: bpf_loader_upgradeable::ID,
            lamports: 1,
            executable: true,
            data: vec![1],
        };
        let data_account = |key: Pubkey, owner: Pubkey| ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports: 1,
            executable: false,
            data: vec![1],
        };
        let fixed = DirectHotFixedRouteV3 {
            market: ObservedAccount {
                observation: observation(),
                key: controller_market,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
            root: physical[selected.account_bindings[0].physical_ordinal]
                .account
                .clone(),
            manifest,
            program_set,
            descriptor,
            config,
            account_profile,
            request_profile,
            transition,
            effect,
            lifecycle: lifecycle_record,
            strategy,
            activation_cache: data_account(Pubkey::new_from_array([120; 32]), trading),
            core_program: program(core),
            core_programdata: data_account(
                Pubkey::new_from_array([121; 32]),
                bpf_loader_upgradeable::ID,
            ),
            trading_program: program(trading),
            trading_programdata: data_account(
                Pubkey::new_from_array([122; 32]),
                bpf_loader_upgradeable::ID,
            ),
            registry_program: program(registry),
            rent_sysvar: rent_account(&rent),
            instructions_sysvar: ObservedAccount {
                observation: observation(),
                key: sysvar::instructions::ID,
                owner: sysvar::ID,
                lamports: 1,
                executable: false,
                data: Vec::new(),
            },
            product,
            result_domain,
            portfolio,
            linked_basis,
            capability_seal: data_account(Pubkey::new_from_array([123; 32]), trading),
        };
        ExpireAcquisitionFixture {
            owned,
            selected,
            fixed,
            logical,
            occurrence_record,
            ticket_record,
            rent_credit,
            permit,
            template: template.to_vec(),
            occurrence: occurrence.to_vec(),
            ticket: ticket.to_vec(),
            siblings: siblings.to_vec(),
            series,
            ticket_state,
            now_slot,
            rent_sink,
        }
    }

    fn selected_for_profile(
        profile: &AccountProfileV3<'_>,
    ) -> (SeriesSelectedActionV5, Vec<SeriesLogicalPhysicalBindingV5>) {
        let base = profile.base();
        let logical_accounts = base
            .logical_account_count_with_dynamic_spans(0, &[])
            .expect("logical accounts");
        let physical_accounts = base
            .physical_account_count_with_dynamic_spans(0, &[])
            .expect("physical accounts");
        let bindings = (0..logical_accounts)
            .map(|logical| SeriesLogicalPhysicalBindingV5 {
                logical,
                representative: base
                    .representative_with_dynamic_spans(0, &[], logical)
                    .expect("representative"),
                physical_ordinal: base
                    .physical_account_ordinal_with_dynamic_spans(0, &[], logical)
                    .expect("physical ordinal"),
            })
            .collect::<Vec<_>>();
        (
            SeriesSelectedActionV5 {
                action: SeriesActionV3::Retire,
                request_bytes: Vec::new(),
                descriptor: [0;
                    dclutch_market::capability_program::v4::CAPABILITY_PROGRAM_V4_BYTES],
                artifact_ids: SeriesActionArtifactIdsV5 {
                    account_profile: [1; 32],
                    request_profile: [2; 32],
                    lifecycle: [3; 32],
                    strategy: [4; 32],
                    transition: [5; 32],
                    effect: [6; 32],
                },
                geometry: SeriesSelectedGeometryV5 {
                    logical_fixed_accounts: u16::try_from(logical_accounts)
                        .expect("logical fixed accounts"),
                    logical_accounts,
                    physical_accounts,
                    common_scalars: 0,
                    common_identities: 0,
                    route_count: 0,
                },
                roles: SeriesSelectedRolesV5 {
                    root: 0,
                    ticket: Some(5),
                    rent_credit: Some(6),
                    payer: None,
                    refund: None,
                    system_program: None,
                },
                authority: SeriesOccurrenceAuthorityV5::Terminal,
                account_bindings: bindings.clone(),
                artifacts: SeriesSelectedArtifactBodiesV5 {
                    account_profile: Vec::new(),
                    request_profile: Vec::new(),
                    lifecycle: Vec::new(),
                    strategy: [0;
                        dclutch_market::execution_strategy::v2::EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
                    transition: Vec::new(),
                    effect: Vec::new(),
                },
                route_requests: Vec::new(),
            },
            bindings,
        )
    }

    fn exact_account(
        ordinal: usize,
        geometry: dclutch_vm::account_profile::v2::PhysicalAccountGeometryV2,
    ) -> ObservedAccountMetaV3 {
        let data = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes } => vec![0; bytes],
            PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => vec![0; live_bytes],
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                vec![0; minimum_bytes.max(1)]
            }
            PhysicalAccountDataGeometryV2::Opaque => Vec::new(),
        };
        let privileges = geometry.privileges();
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: Pubkey::new_from_array(
                    [u8::try_from(ordinal + 1).expect("small physical bank"); 32],
                ),
                owner: system_program::ID,
                lamports: 1,
                executable: privileges.executable(),
                data,
            },
            is_signer: privileges.signer(),
            is_writable: privileges.writable(),
        }
    }

    #[test]
    fn public_expire_acquisition_authenticates_the_complete_observed_bank() {
        let fixture = expire_acquisition_fixture();
        let acquired = fixture
            .acquire(
                &fixture.fixed,
                &fixture.logical,
                &fixture.occurrence_record,
                &fixture.ticket_record,
                &fixture.rent_credit,
                &fixture.permit,
            )
            .expect("honest acquisition");
        assert_eq!(
            acquired.state.fixed_accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            acquired.state.runtime_physical_accounts.len(),
            fixture.selected.geometry.physical_accounts
        );
        assert_eq!(acquired.roles.root, fixture.fixed.root);
        assert_eq!(
            acquired.roles.occurrence,
            Some(fixture.occurrence_record.raw.clone())
        );
        assert_eq!(
            acquired.roles.ticket_record,
            Some(fixture.ticket_record.raw.clone())
        );
        assert_eq!(
            acquired.roles.future_market.as_ref().map(|value| value.key),
            Some(Pubkey::new_from_array([2; 32]))
        );
        assert_eq!(
            acquired.roles.expire_permit.as_ref().map(|value| value.key),
            Some(fixture.permit.key)
        );

        let mut wrong_core = fixture.fixed.clone();
        wrong_core.core_program.key = Pubkey::new_unique();
        assert_eq!(
            fixture
                .acquire(
                    &wrong_core,
                    &fixture.logical,
                    &fixture.occurrence_record,
                    &fixture.ticket_record,
                    &fixture.rent_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::SeriesHotOperator(
                crate::series_hot_v3::SeriesHotOperatorErrorV3::ActionMismatch
            ))
        );
        let mut occurrence_proof = fixture.occurrence_record.clone();
        occurrence_proof.staging.key = Pubkey::new_unique();
        assert_eq!(
            fixture
                .acquire(
                    &fixture.fixed,
                    &fixture.logical,
                    &occurrence_proof,
                    &fixture.ticket_record,
                    &fixture.rent_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::ObservationError(
                crate::observation::ObservationError::AddressMismatch
            ))
        );
        let mut ticket_proof = fixture.ticket_record.clone();
        ticket_proof.staging.key = Pubkey::new_unique();
        assert_eq!(
            fixture
                .acquire(
                    &fixture.fixed,
                    &fixture.logical,
                    &fixture.occurrence_record,
                    &ticket_proof,
                    &fixture.rent_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::ObservationError(
                crate::observation::ObservationError::AddressMismatch
            ))
        );
        let mut wrong_credit = fixture.rent_credit.clone();
        wrong_credit.key = Pubkey::new_unique();
        assert_eq!(
            fixture
                .acquire(
                    &fixture.fixed,
                    &fixture.logical,
                    &fixture.occurrence_record,
                    &fixture.ticket_record,
                    &wrong_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::Role)
        );
        let mut wrong_future = fixture.logical.clone();
        wrong_future[54].key = Pubkey::new_unique();
        assert_eq!(
            fixture
                .acquire(
                    &fixture.fixed,
                    &wrong_future,
                    &fixture.occurrence_record,
                    &fixture.ticket_record,
                    &fixture.rent_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );
        let mut stale = fixture.fixed.clone();
        stale.market.observation.slot += 1;
        assert_eq!(
            fixture
                .acquire(
                    &stale,
                    &fixture.logical,
                    &fixture.occurrence_record,
                    &fixture.ticket_record,
                    &fixture.rent_credit,
                    &fixture.permit,
                )
                .err(),
            Some(SeriesCurrentAcquisitionErrorV5::Observation)
        );
    }

    #[test]
    fn runtime_packing_owns_aliases_widths_privileges_and_injected_prefix() {
        let artifacts = emit_series_retire_funding_artifacts_v5().expect("Retire artifacts");
        let profile = AccountProfileV3::decode(&artifacts.account_profile).expect("profile");
        let base = profile.base();
        let (selected, bindings) = selected_for_profile(&profile);
        let expected = (0..selected.geometry.physical_accounts)
            .map(|ordinal| {
                exact_account(
                    ordinal,
                    base.physical_account_geometry_with_dynamic_spans(0, &[], ordinal)
                        .expect("physical geometry"),
                )
            })
            .collect::<Vec<_>>();
        let logical = bindings
            .iter()
            .map(|binding| expected[binding.physical_ordinal].account.clone())
            .collect::<Vec<_>>();
        let mut fixed = vec![expected[0].clone(); HOT_FIXED_ACCOUNT_COUNT_V3];
        for (ordinal, coordinate) in [
            HOT_ROOT_ACCOUNT_V3,
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ]
        .into_iter()
        .enumerate()
        {
            fixed[coordinate] = expected[ordinal].clone();
        }
        let pack = |selected: &SeriesSelectedActionV5, logical: &[ObservedAccount]| {
            pack_runtime_accounts(
                selected,
                &artifacts.account_profile,
                &[],
                logical,
                &fixed,
                observation(),
            )
        };
        assert_eq!(pack(&selected, &logical), Ok(expected.clone()));
        for (ordinal, account) in expected.iter().enumerate() {
            let geometry = base
                .physical_account_geometry_with_dynamic_spans(0, &[], ordinal)
                .expect("physical geometry");
            assert_eq!(account.is_signer, geometry.privileges().signer());
            assert_eq!(account.is_writable, geometry.privileges().writable());
            assert_eq!(
                account.account.executable,
                geometry.privileges().executable()
            );
        }

        let alias = bindings
            .iter()
            .find(|binding| binding.logical != binding.representative)
            .expect("Retire route alias");
        let mut substituted_alias = logical.clone();
        substituted_alias[alias.logical].key = Pubkey::new_unique();
        assert_eq!(
            pack(&selected, &substituted_alias),
            Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );
        let mut hostile_binding = selected.clone();
        hostile_binding.account_bindings[alias.logical].representative = alias.logical;
        assert_eq!(
            pack(&hostile_binding, &logical),
            Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );

        let self_coordinates = bindings
            .iter()
            .filter(|binding| binding.logical == binding.representative)
            .map(|binding| binding.logical)
            .collect::<Vec<_>>();
        let mut nonalias = logical.clone();
        nonalias[self_coordinates[6]].key = nonalias[self_coordinates[5]].key;
        assert_eq!(
            pack(&selected, &nonalias),
            Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );
        let mut wrong_width = logical.clone();
        wrong_width[1].data.push(1);
        assert_eq!(
            pack(&selected, &wrong_width),
            Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );
        let mut wrong_executable = logical.clone();
        wrong_executable[7].executable = !wrong_executable[7].executable;
        assert_eq!(
            pack(&selected, &wrong_executable),
            Err(SeriesCurrentAcquisitionErrorV5::RuntimeProfile)
        );
        let mut wrong_observation = logical.clone();
        wrong_observation[0].observation.slot += 1;
        assert_eq!(
            pack(&selected, &wrong_observation),
            Err(SeriesCurrentAcquisitionErrorV5::Observation)
        );
    }
}
