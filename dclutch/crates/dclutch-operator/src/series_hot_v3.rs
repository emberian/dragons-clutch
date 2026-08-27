//! Chain-derived unsigned recurring-Series V3 hot execution construction.
//!
//! The builder derives the immutable Hot envelope, Series replay coordinates,
//! action-selected artifact bundle, and Shadow accelerator selection from one
//! finalized observation. It never performs RPC, signs, submits, or treats a
//! client projection as onchain authority. The canonical Trading interpreter
//! and its child receipt chain remain authoritative at execution time.

use crate::{
    Finality, Observation, ObservedAccount,
    direct_inline_v3::{CheckedHotOuterReleaseV3, ObservedAccountMetaV3},
    foundation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
    verticals::decode_clock,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_CONFIG_COORDINATE_V3,
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, HOT_RUNTIME_PRODUCT_COORDINATE_V3,
        HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AcceleratorTransportProfileV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2, ExecutionStrategyCertificateV2,
    StrategyDispositionV2,
};
use dclutch_registry_contract::{ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1};
use dclutch_release_set_contract::{ArtifactReleaseIdV1, ExecutionRoleV1};
use dclutch_series_v3_kernel::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, TemplateV3, admit_ticket,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3, TicketStateSeedsV3, TicketStateV3},
};
use dclutch_trading_sbf::series::{
    accounts::SERIES_ROOT_ACCOUNT_BYTES_V3,
    artifacts_v3::{
        SeriesArtifactBundleV3, SeriesArtifactBytesV3, SeriesArtifactSelectionV3,
        authenticate_series_artifacts_v3,
    },
    instruction::SeriesActionV3,
    operator::{
        SeriesOccurrenceSnapshotV3, UnsignedSeriesActionV3, build_consume_v3, build_expire_v3,
        build_prepare_v3,
    },
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

const SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3: usize = 7;
const SHADOW_CERTIFICATE_RAW_V3: usize = 0;
const SHADOW_CERTIFICATE_STAGING_V3: usize = 1;
const SHADOW_ARTIFACT_RAW_V3: usize = 2;
const SHADOW_ARTIFACT_STAGING_V3: usize = 3;
const SHADOW_ACCELERATOR_PROGRAM_V3: usize = 4;
const SHADOW_ACCELERATOR_PROGRAMDATA_V3: usize = 5;
const SHADOW_CALLER_AUTHORITY_V3: usize = 6;

/// One Registry-finalized immutable Series record and its vacant staging cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFinalizedRecordV3 {
    /// Exact raw record observation.
    pub raw: ObservedAccount,
    /// Exact schema and same-snapshot vacant staging cursor.
    pub finalization: FinalizedRecordProof,
}

/// Current checked Shadow accelerator selected by the immutable strategy chain.
///
/// A release checker constructs this value only after matching the finalized
/// ArtifactRelease to current Loader metadata and a user-supplied checked
/// multiprogram manifest. This host builder rechecks its identities against the
/// exact record and physical accounts but does not reproduce Loader inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedSeriesShadowAcceleratorV3 {
    /// Exact finalized ArtifactRelease content identity.
    pub artifact_release: [u8; 32],
    /// Current accelerator Program identity.
    pub accelerator_program: Pubkey,
    /// Current accelerator ProgramData identity.
    pub accelerator_programdata: Pubkey,
    /// Digest of the checked multiprogram manifest used by the release checker.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized Series occurrence state and exact Hot physical projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesOccurrenceHotStateV3 {
    /// Exact 38-account family-neutral Hot prefix.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Six Shadow strategy extras plus the Trading caller-authority PDA.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact logical AccountProfile vector, including all five injected prefix
    /// observations at coordinates zero through four.
    pub runtime_accounts: Vec<ObservedAccountMetaV3>,
    /// Finalized realized occurrence record.
    pub occurrence: SeriesFinalizedRecordV3,
    /// Finalized immutable Ticket record.
    pub ticket: SeriesFinalizedRecordV3,
    /// Current or vacant Trading-owned Ticket replay PDA observation.
    pub ticket_replay: ObservedAccount,
    /// Canonical Clock sysvar included in the selected runtime profile.
    pub clock: ObservedAccount,
    /// Ordered occurrence-projection Merkle siblings.
    pub occurrence_proof: Vec<[u8; 32]>,
    /// Checked current common Trading hot outer.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
    /// Checked current Shadow accelerator release.
    pub shadow_accelerator: Option<CheckedSeriesShadowAcceleratorV3>,
}

/// Complete unsigned Hot instruction plus the exact authority observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesOccurrenceHotReportV3 {
    /// Sole unsigned Trading Hot instruction. No signature or submission occurs.
    pub instruction: Instruction,
    /// Exact encoded Hot envelope followed by the Series family request.
    pub instruction_data: Vec<u8>,
    /// Same finalized observation selecting every input.
    pub observation: Observation,
    /// Constructed recurring-Series action.
    pub action: SeriesActionV3,
    /// Action-selected CapabilityProgramV3 content digest.
    pub selected_program: [u8; 32],
    /// Checked Shadow ArtifactRelease content identity.
    pub accelerator_artifact_release: [u8; 32],
}

/// Stable refusal from finalized state, replay, artifact, or account projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesHotOperatorErrorV3 {
    /// The common outer or Shadow accelerator was not checked for this snapshot.
    CheckedReleaseUnavailable,
    /// A required identity or checked-manifest digest was zero.
    ZeroIdentity,
    /// Inputs did not share one finalized observation.
    ObservationMismatch,
    /// The common prefix, root, Market, Registry, or sysvar frame differed.
    FixedFrameMismatch,
    /// Template, occurrence, Ticket, or staging finalization refused.
    RecordMismatch,
    /// Root or Ticket replay state/PDA refused.
    ReplayMismatch,
    /// Action-selected generic artifacts refused.
    ArtifactMismatch,
    /// Shadow Certificate, ArtifactRelease, or deployment identities differed.
    ShadowSelectionMismatch,
    /// Runtime AccountProfile width, alias, or privileges differed.
    RuntimeProfileMismatch,
    /// Clock schedule or exact Series request construction refused.
    ActionMismatch,
    /// Checked arithmetic or instruction encoding failed.
    Arithmetic,
}

/// Build a dust-tolerant pre-founding Ticket Prepare instruction.
pub fn build_series_prepare_hot_v3(
    state: &SeriesOccurrenceHotStateV3,
) -> Result<SeriesOccurrenceHotReportV3, SeriesHotOperatorErrorV3> {
    build_series_occurrence_hot_v3(state, SeriesActionV3::Prepare)
}

/// Build the atomic prepared-Ticket to Found-Market Consume instruction.
pub fn build_series_consume_hot_v3(
    state: &SeriesOccurrenceHotStateV3,
) -> Result<SeriesOccurrenceHotReportV3, SeriesHotOperatorErrorV3> {
    build_series_occurrence_hot_v3(state, SeriesActionV3::Consume)
}

/// Build the exact post-retry Ticket refund and cleanup instruction.
pub fn build_series_expire_hot_v3(
    state: &SeriesOccurrenceHotStateV3,
) -> Result<SeriesOccurrenceHotReportV3, SeriesHotOperatorErrorV3> {
    build_series_occurrence_hot_v3(state, SeriesActionV3::Expire)
}

fn build_series_occurrence_hot_v3(
    state: &SeriesOccurrenceHotStateV3,
    action: SeriesActionV3,
) -> Result<SeriesOccurrenceHotReportV3, SeriesHotOperatorErrorV3> {
    let outer = state
        .hot_outer
        .ok_or(SeriesHotOperatorErrorV3::CheckedReleaseUnavailable)?;
    let accelerator = state
        .shadow_accelerator
        .ok_or(SeriesHotOperatorErrorV3::CheckedReleaseUnavailable)?;
    require_checked_identities(outer, accelerator)?;
    let frame = authenticate_frame(state, outer)?;
    authenticate_occurrence_records(state, frame.registry_program, &frame.rent)?;
    let request = build_family_request(state, action, frame.series, frame.clock_slot, outer)?;
    let artifacts = artifacts_from_frame(state)?;
    let selection = SeriesArtifactSelectionV3 {
        program_set: frame.header.selection().capability_release().to_bytes(),
        template: frame.header.selection().config(),
    };
    let bundle = authenticate_series_artifacts_v3(selection, artifacts, request.as_bytes())
        .map_err(|_| SeriesHotOperatorErrorV3::ArtifactMismatch)?;
    if bundle.request.action() != action {
        return Err(SeriesHotOperatorErrorV3::ActionMismatch);
    }
    validate_runtime_profile(state, bundle)?;
    validate_shadow_selection(
        state,
        frame.registry_program,
        &frame.rent,
        bundle,
        accelerator,
    )?;

    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.as_bytes().len())
            .map_err(|_| SeriesHotOperatorErrorV3::Arithmetic)?,
        frame.header.release_set().to_bytes(),
        frame.header.market(),
        frame.header.generation(),
        hash(&frame.root.data).to_bytes(),
    )
    .map_err(|_| SeriesHotOperatorErrorV3::FixedFrameMismatch)?;
    let mut instruction_data = Vec::with_capacity(
        dclutch_capability_program_contract::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(request.as_bytes().len())
            .ok_or(SeriesHotOperatorErrorV3::Arithmetic)?,
    );
    instruction_data.extend_from_slice(&envelope.to_bytes());
    instruction_data.extend_from_slice(request.as_bytes());

    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| {
                count.checked_add(
                    state
                        .runtime_accounts
                        .len()
                        .saturating_sub(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3),
                )
            })
            .ok_or(SeriesHotOperatorErrorV3::Arithmetic)?,
    );
    accounts.extend(state.fixed_accounts.iter().map(account_meta));
    accounts.extend(state.strategy_accounts.iter().map(account_meta));
    accounts.extend(
        state
            .runtime_accounts
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(account_meta),
    );
    Ok(SeriesOccurrenceHotReportV3 {
        instruction: Instruction {
            program_id: outer.trading_program,
            accounts,
            data: instruction_data.clone(),
        },
        instruction_data,
        observation: frame.observation,
        action,
        selected_program: hash(artifacts.descriptor).to_bytes(),
        accelerator_artifact_release: accelerator.artifact_release,
    })
}

struct AuthenticatedFrameV3<'a> {
    observation: Observation,
    header: CapabilityRootHeaderV1,
    root: &'a ObservedAccount,
    registry_program: Pubkey,
    rent: solana_program::rent::Rent,
    series: SeriesStateV3,
    clock_slot: u64,
}

fn authenticate_frame<'a>(
    state: &'a SeriesOccurrenceHotStateV3,
    checked: CheckedHotOuterReleaseV3,
) -> Result<AuthenticatedFrameV3<'a>, SeriesHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.strategy_accounts.len() != SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    let market = fixed(state, HOT_MARKET_ACCOUNT_V3)?;
    let root_meta = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let trading = fixed(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let registry = fixed(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?;
    let rent_account = fixed(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?;
    let instructions = fixed(state, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?;
    if root_meta.account.data.len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || root_meta.account.owner != checked.trading_program
        || root_meta.is_signer
        || !root_meta.is_writable
        || root_meta.account.executable
        || trading.account.key != checked.trading_program
        || !trading.account.executable
        || trading.is_signer
        || trading.is_writable
        || !registry.account.executable
        || registry.is_signer
        || registry.is_writable
        || rent_account.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    let header = CapabilityRootHeaderV1::decode(
        root_meta
            .account
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesHotOperatorErrorV3::FixedFrameMismatch)?,
    )
    .map_err(|_| SeriesHotOperatorErrorV3::FixedFrameMismatch)?;
    let root_seeds = header.seeds();
    if header.market() != market.account.key.to_bytes()
        || header.selection().executor_role() != ExecutionRoleV1::Trading
        || Pubkey::find_program_address(&root_seeds.as_slices(), &checked.trading_program).0
            != root_meta.account.key
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    validate_injected_runtime(state)?;
    let observation = market.account.observation;
    validate_observation_set(state, observation)?;
    let rent = decode_rent(&rent_account.account)
        .map_err(|_| SeriesHotOperatorErrorV3::FixedFrameMismatch)?;
    let template = TemplateV3::decode(&fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?.account.data)
        .map_err(|_| SeriesHotOperatorErrorV3::RecordMismatch)?;
    let series = SeriesStateV3::decode(
        root_meta
            .account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesHotOperatorErrorV3::ReplayMismatch)?,
        template.occurrence_count(),
    )
    .map_err(|_| SeriesHotOperatorErrorV3::ReplayMismatch)?;
    let clock = decode_clock(&state.clock).map_err(|_| SeriesHotOperatorErrorV3::ActionMismatch)?;
    require_runtime_account_once(state, &state.clock)?;
    Ok(AuthenticatedFrameV3 {
        observation,
        header,
        root: &root_meta.account,
        registry_program: registry.account.key,
        rent,
        series,
        clock_slot: clock.slot,
    })
}

fn authenticate_occurrence_records(
    state: &SeriesOccurrenceHotStateV3,
    registry_program: Pubkey,
    rent: &solana_program::rent::Rent,
) -> Result<(), SeriesHotOperatorErrorV3> {
    let template = fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?;
    let template_staging = fixed(state, HOT_CONFIG_STAGING_ACCOUNT_V3)?;
    let template_finalization = FinalizedRecordProof {
        schema_release_id: SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        staging_cursor: template_staging.account.clone(),
    };
    authenticate_finalized_record(
        registry_program,
        rent,
        &template.account,
        &template_finalization,
    )
    .map_err(|_| SeriesHotOperatorErrorV3::RecordMismatch)?;
    for (record, schema) in [
        (&state.occurrence, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3),
        (&state.ticket, SERIES_TICKET_SCHEMA_RELEASE_ID_V3),
    ] {
        if record.finalization.schema_release_id != schema {
            return Err(SeriesHotOperatorErrorV3::RecordMismatch);
        }
        authenticate_finalized_record(registry_program, rent, &record.raw, &record.finalization)
            .map_err(|_| SeriesHotOperatorErrorV3::RecordMismatch)?;
        require_runtime_account_once(state, &record.raw)?;
        require_runtime_account_once(state, &record.finalization.staging_cursor)?;
    }
    Ok(())
}

fn build_family_request(
    state: &SeriesOccurrenceHotStateV3,
    action: SeriesActionV3,
    series: SeriesStateV3,
    now_slot: u64,
    outer: CheckedHotOuterReleaseV3,
) -> Result<UnsignedSeriesActionV3, SeriesHotOperatorErrorV3> {
    let ticket = admit_ticket(&state.ticket.raw.data)
        .map_err(|_| SeriesHotOperatorErrorV3::RecordMismatch)?;
    let seeds = TicketStateSeedsV3::new(
        fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.key.to_bytes(),
        ticket.content_id(),
    );
    let expected_ticket =
        Pubkey::find_program_address(&seeds.as_slices(), &outer.trading_program).0;
    if state.ticket_replay.key != expected_ticket
        || state.ticket_replay.executable
        || require_runtime_account_once(state, &state.ticket_replay)?.is_signer
        || !require_runtime_account_once(state, &state.ticket_replay)?.is_writable
    {
        return Err(SeriesHotOperatorErrorV3::ReplayMismatch);
    }
    let ticket_state = match action {
        SeriesActionV3::Prepare => {
            if state.ticket_replay.owner != system_program::ID
                || !state.ticket_replay.data.is_empty()
            {
                return Err(SeriesHotOperatorErrorV3::ReplayMismatch);
            }
            None
        }
        SeriesActionV3::Consume | SeriesActionV3::Expire => {
            if state.ticket_replay.owner != outer.trading_program {
                return Err(SeriesHotOperatorErrorV3::ReplayMismatch);
            }
            Some(
                TicketStateV3::decode(&state.ticket_replay.data)
                    .map_err(|_| SeriesHotOperatorErrorV3::ReplayMismatch)?,
            )
        }
        SeriesActionV3::Retire | SeriesActionV3::Close => {
            return Err(SeriesHotOperatorErrorV3::ActionMismatch);
        }
    };
    let snapshot = SeriesOccurrenceSnapshotV3 {
        template_bytes: &fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?.account.data,
        occurrence_bytes: &state.occurrence.raw.data,
        ticket_bytes: &state.ticket.raw.data,
        siblings: &state.occurrence_proof,
        series,
        ticket_state,
        now_slot,
    };
    match action {
        SeriesActionV3::Prepare => build_prepare_v3(snapshot),
        SeriesActionV3::Consume => build_consume_v3(snapshot),
        SeriesActionV3::Expire => build_expire_v3(snapshot),
        SeriesActionV3::Retire | SeriesActionV3::Close => {
            return Err(SeriesHotOperatorErrorV3::ActionMismatch);
        }
    }
    .map_err(|_| SeriesHotOperatorErrorV3::ActionMismatch)
}

fn artifacts_from_frame(
    state: &SeriesOccurrenceHotStateV3,
) -> Result<SeriesArtifactBytesV3<'_>, SeriesHotOperatorErrorV3> {
    Ok(SeriesArtifactBytesV3 {
        program_set: &fixed(state, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?.account.data,
        descriptor: &fixed(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?.account.data,
        account_profile: &fixed(state, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?
            .account
            .data,
        request_profile: &fixed(state, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3)?
            .account
            .data,
        strategy: &fixed(state, HOT_STRATEGY_RAW_ACCOUNT_V3)?.account.data,
        transition: &fixed(state, HOT_TRANSITION_RAW_ACCOUNT_V3)?.account.data,
        effect: &fixed(state, HOT_EFFECT_RAW_ACCOUNT_V3)?.account.data,
    })
}

fn validate_runtime_profile(
    state: &SeriesOccurrenceHotStateV3,
    bundle: SeriesArtifactBundleV3<'_>,
) -> Result<(), SeriesHotOperatorErrorV3> {
    let profile = bundle.account_profile;
    let expected = usize::from(profile.fixed_account_count());
    if profile.item_account_stride() != 0 || state.runtime_accounts.len() != expected {
        return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
    }
    for (coordinate, account) in state.runtime_accounts.iter().enumerate() {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| SeriesHotOperatorErrorV3::Arithmetic)?,
            )
            .map_err(|_| SeriesHotOperatorErrorV3::RuntimeProfileMismatch)?;
        let privileges = rule.privileges();
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
        {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

fn validate_shadow_selection(
    state: &SeriesOccurrenceHotStateV3,
    registry_program: Pubkey,
    rent: &solana_program::rent::Rent,
    bundle: SeriesArtifactBundleV3<'_>,
    checked: CheckedSeriesShadowAcceleratorV3,
) -> Result<(), SeriesHotOperatorErrorV3> {
    if bundle.strategy.disposition() != StrategyDispositionV2::ShadowAot
        || bundle.strategy.transport_profile()
            != Ok(AcceleratorTransportProfileV2::ShadowTranscriptV3)
    {
        return Err(SeriesHotOperatorErrorV3::ShadowSelectionMismatch);
    }
    let certificate_raw = strategy(state, SHADOW_CERTIFICATE_RAW_V3)?;
    let certificate_staging = strategy(state, SHADOW_CERTIFICATE_STAGING_V3)?;
    let artifact_raw = strategy(state, SHADOW_ARTIFACT_RAW_V3)?;
    let artifact_staging = strategy(state, SHADOW_ARTIFACT_STAGING_V3)?;
    for (account, schema, staging) in [
        (
            &certificate_raw.account,
            EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
            &certificate_staging.account,
        ),
        (
            &artifact_raw.account,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &artifact_staging.account,
        ),
    ] {
        authenticate_finalized_record(
            registry_program,
            rent,
            account,
            &FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    }
    let certificate_id = content_id(&certificate_raw.account.data)?;
    let strategy_id = content_id(&fixed(state, HOT_STRATEGY_RAW_ACCOUNT_V3)?.account.data)?;
    let certificate = ExecutionStrategyCertificateV2::decode(&certificate_raw.account.data)
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    certificate
        .validate_v3(
            certificate_id,
            strategy_id,
            bundle.strategy,
            bundle.descriptor,
            AuthenticatedInterpreterArtifactsV2 {
                account_profile_program: bundle.descriptor.account_profile(),
                request_profile_schema: bundle.descriptor.request_profile_schema(),
                request_profile_program: bundle.descriptor.request_profile_program(),
                transition_schema: bundle.strategy.transition_schema(),
                transition_program: bundle.strategy.transition_program(),
                effect_program: bundle.descriptor.effect_program(),
            },
        )
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    let artifact_id = ArtifactReleaseIdV1::new(checked.artifact_release)
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    certificate
        .validate_artifact(artifact_id)
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    if hash(&artifact_raw.account.data).to_bytes() != checked.artifact_release {
        return Err(SeriesHotOperatorErrorV3::ShadowSelectionMismatch);
    }
    let artifact = ArtifactReleaseV1::decode(&artifact_raw.account.data)
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)?;
    let program = strategy(state, SHADOW_ACCELERATOR_PROGRAM_V3)?;
    let programdata = strategy(state, SHADOW_ACCELERATOR_PROGRAMDATA_V3)?;
    let caller = strategy(state, SHADOW_CALLER_AUTHORITY_V3)?;
    if program.account.key != checked.accelerator_program
        || programdata.account.key != checked.accelerator_programdata
        || artifact.program().to_bytes() != program.account.key.to_bytes()
        || artifact.programdata() != programdata.account.key.to_bytes()
        || artifact.loader_program().to_bytes() != program.account.owner.to_bytes()
        || programdata.account.owner != program.account.owner
        || program.account.owner != bpf_loader_upgradeable::ID
        || !program.account.executable
        || programdata.account.executable
        || program.is_signer
        || program.is_writable
        || programdata.is_signer
        || programdata.is_writable
        || caller.is_signer
        || caller.is_writable
        || caller.account.executable
    {
        return Err(SeriesHotOperatorErrorV3::ShadowSelectionMismatch);
    }
    Ok(())
}

fn require_checked_identities(
    outer: CheckedHotOuterReleaseV3,
    accelerator: CheckedSeriesShadowAcceleratorV3,
) -> Result<(), SeriesHotOperatorErrorV3> {
    if outer.trading_program == Pubkey::default()
        || outer.artifact_release == [0; 32]
        || outer.checked_manifest_digest == [0; 32]
        || accelerator.artifact_release == [0; 32]
        || accelerator.accelerator_program == Pubkey::default()
        || accelerator.accelerator_programdata == Pubkey::default()
        || accelerator.accelerator_program == accelerator.accelerator_programdata
        || accelerator.checked_manifest_digest == [0; 32]
        || accelerator.checked_manifest_digest != outer.checked_manifest_digest
    {
        return Err(SeriesHotOperatorErrorV3::ZeroIdentity);
    }
    Ok(())
}

fn validate_injected_runtime(
    state: &SeriesOccurrenceHotStateV3,
) -> Result<(), SeriesHotOperatorErrorV3> {
    for (runtime, physical) in [
        (HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_ROOT_ACCOUNT_V3),
        (HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_CONFIG_RAW_ACCOUNT_V3),
        (
            HOT_RUNTIME_PRODUCT_COORDINATE_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
            dclutch_capability_program_contract::hot_v3::HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ),
    ] {
        if state.runtime_accounts.get(runtime) != state.fixed_accounts.get(physical) {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

fn validate_observation_set(
    state: &SeriesOccurrenceHotStateV3,
    observation: Observation,
) -> Result<(), SeriesHotOperatorErrorV3> {
    for value in state
        .fixed_accounts
        .iter()
        .chain(&state.strategy_accounts)
        .chain(&state.runtime_accounts)
    {
        if value.account.observation != observation
            || value.account.observation.finality != Finality::Finalized
        {
            return Err(SeriesHotOperatorErrorV3::ObservationMismatch);
        }
    }
    for value in [
        &state.occurrence.raw,
        &state.occurrence.finalization.staging_cursor,
        &state.ticket.raw,
        &state.ticket.finalization.staging_cursor,
        &state.ticket_replay,
        &state.clock,
    ] {
        if value.observation != observation || value.observation.finality != Finality::Finalized {
            return Err(SeriesHotOperatorErrorV3::ObservationMismatch);
        }
    }
    Ok(())
}

fn require_runtime_account_once<'a>(
    state: &'a SeriesOccurrenceHotStateV3,
    observed: &ObservedAccount,
) -> Result<&'a ObservedAccountMetaV3, SeriesHotOperatorErrorV3> {
    let mut matches = state
        .runtime_accounts
        .iter()
        .filter(|candidate| candidate.account == *observed);
    let value = matches
        .next()
        .ok_or(SeriesHotOperatorErrorV3::RuntimeProfileMismatch)?;
    if matches.next().is_some() {
        return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
    }
    Ok(value)
}

fn fixed(
    state: &SeriesOccurrenceHotStateV3,
    index: usize,
) -> Result<&ObservedAccountMetaV3, SeriesHotOperatorErrorV3> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(SeriesHotOperatorErrorV3::FixedFrameMismatch)
}

fn strategy(
    state: &SeriesOccurrenceHotStateV3,
    index: usize,
) -> Result<&ObservedAccountMetaV3, SeriesHotOperatorErrorV3> {
    state
        .strategy_accounts
        .get(index)
        .ok_or(SeriesHotOperatorErrorV3::ShadowSelectionMismatch)
}

fn content_id(bytes: &[u8]) -> Result<ContentId, SeriesHotOperatorErrorV3> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| SeriesHotOperatorErrorV3::ShadowSelectionMismatch)
}

fn account_meta(value: &ObservedAccountMetaV3) -> AccountMeta {
    AccountMeta {
        pubkey: value.account.key,
        is_signer: value.is_signer,
        is_writable: value.is_writable,
    }
}

const _: () = assert!(
    SERIES_ROOT_ACCOUNT_BYTES_V3 == CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
);

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> Observation {
        Observation {
            slot: 9,
            unix_timestamp: 10,
            finality: Finality::Finalized,
        }
    }

    fn observed(key: u8) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key: Pubkey::new_from_array([key; 32]),
            owner: Pubkey::new_from_array([key.wrapping_add(1); 32]),
            lamports: u64::from(key),
            executable: false,
            data: vec![key],
        }
    }

    fn meta(key: u8) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: observed(key),
            is_signer: false,
            is_writable: false,
        }
    }

    fn minimal_state(runtime: Vec<ObservedAccountMetaV3>) -> SeriesOccurrenceHotStateV3 {
        let record = SeriesFinalizedRecordV3 {
            raw: observed(90),
            finalization: FinalizedRecordProof {
                schema_release_id: [91; 32],
                staging_cursor: observed(92),
            },
        };
        SeriesOccurrenceHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_accounts: runtime,
            occurrence: record.clone(),
            ticket: record,
            ticket_replay: observed(93),
            clock: observed(94),
            occurrence_proof: Vec::new(),
            hot_outer: None,
            shadow_accelerator: None,
        }
    }

    #[test]
    fn runtime_record_must_appear_exactly_once() {
        let account = observed(7);
        let state = minimal_state(vec![ObservedAccountMetaV3 {
            account: account.clone(),
            is_signer: false,
            is_writable: true,
        }]);
        assert!(require_runtime_account_once(&state, &account).is_ok());
        let duplicated = minimal_state(vec![meta(7), meta(7)]);
        assert_eq!(
            require_runtime_account_once(&duplicated, &observed(7)),
            Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch)
        );
    }

    #[test]
    fn checked_outer_and_accelerator_must_share_manifest() {
        let outer = CheckedHotOuterReleaseV3 {
            trading_program: Pubkey::new_from_array([1; 32]),
            artifact_release: [2; 32],
            checked_manifest_digest: [3; 32],
        };
        let mut accelerator = CheckedSeriesShadowAcceleratorV3 {
            artifact_release: [4; 32],
            accelerator_program: Pubkey::new_from_array([5; 32]),
            accelerator_programdata: Pubkey::new_from_array([6; 32]),
            checked_manifest_digest: [3; 32],
        };
        assert_eq!(require_checked_identities(outer, accelerator), Ok(()));
        accelerator.checked_manifest_digest = [7; 32];
        assert_eq!(
            require_checked_identities(outer, accelerator),
            Err(SeriesHotOperatorErrorV3::ZeroIdentity)
        );
    }

    #[test]
    fn shadow_transaction_geometry_is_six_extras_then_caller() {
        assert_eq!(SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3, 7);
        assert_eq!(SHADOW_CERTIFICATE_RAW_V3, 0);
        assert_eq!(SHADOW_ARTIFACT_RAW_V3, 2);
        assert_eq!(SHADOW_ACCELERATOR_PROGRAM_V3, 4);
        assert_eq!(SHADOW_ACCELERATOR_PROGRAMDATA_V3, 5);
        assert_eq!(SHADOW_CALLER_AUTHORITY_V3, 6);
    }
}
