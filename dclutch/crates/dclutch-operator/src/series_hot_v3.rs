//! Chain-derived unsigned recurring-Series V3 hot execution construction.
//!
//! The builder derives the immutable Hot envelope, Series replay coordinates,
//! action-selected artifact bundle, and Shadow accelerator selection from one
//! finalized observation. It never performs RPC, signs, submits, or treats a
//! client projection as onchain authority. The canonical Trading interpreter
//! and its child receipt chain remain authoritative at execution time.
//!
//! # The three occurrence builders have no caller, and the blocker is upstream
//!
//! [`build_series_prepare_hot_v3`], [`build_series_consume_hot_v3`] and
//! [`build_series_expire_hot_v3`] are not reached from anywhere in the tree.
//! They are also the ONLY public door to the private
//! `build_series_occurrence_hot_v3` behind them, so they are not three
//! wrappers that could be dropped cheaply: everything from the frame
//! authentication to the strategy selection is reachable through them and
//! through nothing else.
//!
//! `tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs`
//! compiles all three exact family requests and then refuses, and states why in
//! its own words: Series is a ShadowAot family, the common authenticated Shadow
//! callback is not committed (see
//! `programs/dclutch-series-shadow-sbf/program-test/README.md`), so no Series
//! action has a dispatched Hot route to enter Trading through. The absent
//! caller is downstream of that; writing one now would be a caller that ran
//! against nothing.
//!
//! Series expiry carries a second, independent blocker. `6f258cf5e` convicted
//! the artifact set rather than the fixture: route 4 declares a borrowed range
//! while `proof_height(1) = 0` makes the canonical single-occurrence proof
//! empty, and both spellings of "a borrowed thing is here" refuse a zero
//! length. The repair moves shipped artifact digests and has a second author at
//! `hot_v3.rs:12251`, so it was not taken.
//!
//! Whether Series is built or cut is D7's ruling and it is pending. A cut takes
//! this module whole; a build enters through exactly these three symbols. Until
//! it lands, neither the builders nor the occurrence path behind them should be
//! deleted for want of a caller.
//!
//! None of this applies to the selected-V5 path further down. That one is live:
//! [`inspect_current_series_hot_v5`] is consumed by
//! [`crate::series_current_acquisition_v5`], the Trading program-test's
//! `series_premarket_expiry_chain_v1` support, and the successor's
//! `series_terminal_campaign`.

use crate::series_lifecycle_v3::{
    SeriesLifecycleSnapshotV3, SeriesNextActV3, inspect_series_lifecycle_v3,
};
use crate::{
    Finality, Observation, ObservedAccount,
    direct_inline_v3::{CheckedHotOuterReleaseV3, ObservedAccountMetaV3},
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_clock, decode_rent},
};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_CONFIG_COORDINATE_V3,
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, HOT_RUNTIME_PRODUCT_COORDINATE_V3,
        HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotBumpHintsV1,
        HotExecutionEnvelopeV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_market::execution_strategy::v2::{
    AcceleratorTransportProfileV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2, ExecutionStrategyCertificateV2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_hot_bump_miner_v1::{
    HotBumpCorpusV1, activated_custody_program_v1, mine_hot_bump_hints_v1,
};
use dclutch_market::{Identity as CoreIdentity, SeriesFoundingPermitSeedsV1};
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1};
use dclutch_registry::release_set::{ArtifactReleaseIdV1, ExecutionRoleV1};
use dclutch_trading::series::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, TemplateV3, admit_ticket,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3, TicketStateSeedsV3, TicketStateV3},
    request::SeriesActionRequestV3,
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
    release_v5::{
        SeriesCurrentReleaseInputV5, SeriesLogicalPhysicalBindingV5, SeriesOccurrenceAuthorityV5,
        SeriesReleaseV5, SeriesSelectedActionV5, authenticate_series_selected_action_v5,
        compile_series_release_v5, emit_current_series_release_source_v5,
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
    /// Exact family-neutral Hot prefix.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Strategy-selected physical extras; empty for interpreted execution.
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
    /// Checked current Shadow accelerator release, absent for interpreted execution.
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
    /// Checked Shadow ArtifactRelease content identity, absent when interpreted.
    pub accelerator_artifact_release: Option<[u8; 32]>,
}

/// Stable refusal from finalized state, replay, artifact, or account projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesHotOperatorErrorV3 {
    /// The common outer or a selected accelerator was not checked for this snapshot.
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
    /// Strategy disposition, extras, Certificate, or deployment identities differed.
    StrategySelectionMismatch,
    /// Runtime AccountProfile width, alias, or privileges differed.
    RuntimeProfileMismatch,
    /// Clock schedule or exact Series request construction refused.
    ActionMismatch,
    /// Checked arithmetic or instruction encoding failed.
    Arithmetic,
    /// `dclutch_trading_sbf` refused; the cause is its own.
    SeriesArtifact(dclutch_trading_sbf::series::artifacts_v3::SeriesArtifactErrorV3),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    HotExecution(dclutch_market::capability_program::hot_v3::HotExecutionErrorV3),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_trading::series` refused; the cause is its own.
    Series(dclutch_trading::series::SeriesV3Error),
    /// `dclutch_trading::series` refused; the cause is its own.
    SeriesState(dclutch_trading::series::replay::SeriesStateError),
    /// `dclutch_operator` refused; the cause is its own.
    Observation(crate::observation::ObservationError),
    /// `dclutch_trading_sbf` refused; the cause is its own.
    SeriesOperator(dclutch_trading_sbf::series::operator::SeriesOperatorErrorV3),
    /// `dclutch_vm::account_profile` refused; the cause is its own.
    AccountProfile(dclutch_vm::account_profile::v2::Error),
    /// `dclutch_market::execution_strategy` refused; the cause is its own.
    ExecutionStrategy(dclutch_market::execution_strategy::v2::Error),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_trading_sbf` refused; the cause is its own.
    SeriesRelease(dclutch_trading_sbf::series::release_v5::SeriesReleaseErrorV5),
    /// `dclutch_trading::series` refused; the cause is its own.
    SeriesInstruction(dclutch_trading::series::request::SeriesInstructionErrorV3),
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
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
    require_checked_outer(outer)?;
    let frame = authenticate_frame(state, outer)?;
    authenticate_occurrence_records(state, frame.registry_program)?;
    let request = build_family_request(state, action, frame.series, frame.clock_slot, outer)?;
    let artifacts = artifacts_from_frame(state)?;
    // ONE AUTHOR. `authenticate_frame` has already required the root's
    // `selection().config()` to be `hash(config_record)`, so the Template's
    // content identity this join needs is DERIVED from those same bytes rather
    // than read off the root. The two values are `sha256(t)` and
    // `sha256("dclutch/series-template-v3" || 0x00 || t)`; handing one where
    // the other was meant is what `SeriesArtifactSelectionV3::from_config_record`
    // now makes unspellable.
    let selection = SeriesArtifactSelectionV3::from_config_record(
        frame.header.selection().capability_release().to_bytes(),
        frame.config_record,
    )
    .map_err(SeriesHotOperatorErrorV3::SeriesArtifact)?;
    let bundle = authenticate_series_artifacts_v3(selection, artifacts, request.as_bytes())
        .map_err(SeriesHotOperatorErrorV3::SeriesArtifact)?;
    if bundle.request.action() != action {
        return Err(SeriesHotOperatorErrorV3::ActionMismatch);
    }
    validate_runtime_profile(state, bundle)?;
    let accelerator_artifact_release =
        validate_strategy_selection(state, outer, frame.registry_program, bundle)?;

    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.as_bytes().len())
            .map_err(|_| SeriesHotOperatorErrorV3::Arithmetic)?,
        frame.header.release_set().to_bytes(),
        frame.header.market(),
        frame.header.generation(),
        hash(&frame.root.data).to_bytes(),
    )
    .map_err(SeriesHotOperatorErrorV3::HotExecution)?
    .with_bump_hints(series_occurrence_hot_bump_hints_v3(
        state,
        &outer.trading_program,
        frame.header.release_set().to_bytes(),
    )?);
    let mut instruction_data = Vec::with_capacity(
        dclutch_market::capability_program::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3
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
        accelerator_artifact_release,
    })
}

struct AuthenticatedFrameV3<'a> {
    observation: Observation,
    header: CapabilityRootHeaderV1,
    root: &'a ObservedAccount,
    registry_program: Pubkey,
    series: SeriesStateV3,
    clock_slot: u64,
    /// Exact bytes of the root's selected config record, which for Series IS
    /// the Template record. Carried out so the Template's content identity is
    /// derived from the bytes the root names rather than from the root's own
    /// config field, which is those bytes' Registry RECORD DIGEST.
    config_record: &'a [u8],
}

fn authenticate_frame<'a>(
    state: &'a SeriesOccurrenceHotStateV3,
    checked: CheckedHotOuterReleaseV3,
) -> Result<AuthenticatedFrameV3<'a>, SeriesHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
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
    .map_err(SeriesHotOperatorErrorV3::CapabilityProgram)?;
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
    // THE ROOT NAMES ITS CONFIG RECORD BY THAT RECORD'S OWN DIGEST. This is
    // the family-neutral rule `borrow_record_against` enforces on chain and
    // that `series_current_acquisition_v5` already spelled here; the Series Hot
    // builder did not state it, and instead read the config field as if it were
    // the Template's domain-separated content identity. Stating it makes the
    // config record's bytes -- the Template record -- the single author of both.
    let config_record = fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?
        .account
        .data
        .as_slice();
    if header.selection().config().to_bytes() != hash(config_record).to_bytes() {
        return Err(SeriesHotOperatorErrorV3::RecordMismatch);
    }
    let template = TemplateV3::decode(config_record).map_err(SeriesHotOperatorErrorV3::Series)?;
    let series = SeriesStateV3::decode(
        root_meta
            .account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(SeriesHotOperatorErrorV3::ReplayMismatch)?,
        template.occurrence_count(),
    )
    .map_err(SeriesHotOperatorErrorV3::SeriesState)?;
    let clock = decode_clock(&state.clock).map_err(SeriesHotOperatorErrorV3::Observation)?;
    require_runtime_account_once(state, &state.clock)?;
    Ok(AuthenticatedFrameV3 {
        observation,
        header,
        root: &root_meta.account,
        registry_program: registry.account.key,
        series,
        clock_slot: clock.slot,
        config_record,
    })
}

fn authenticate_occurrence_records(
    state: &SeriesOccurrenceHotStateV3,
    registry_program: Pubkey,
) -> Result<(), SeriesHotOperatorErrorV3> {
    let template = fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?;
    let template_staging = fixed(state, HOT_CONFIG_STAGING_ACCOUNT_V3)?;
    let template_finalization = FinalizedRecordProof {
        schema_release_id: SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        staging_cursor: template_staging.account.clone(),
    };
    authenticate_finalized_record(registry_program, &template.account, &template_finalization)
        .map_err(SeriesHotOperatorErrorV3::Observation)?;
    for (record, schema) in [
        (&state.occurrence, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3),
        (&state.ticket, SERIES_TICKET_SCHEMA_RELEASE_ID_V3),
    ] {
        if record.finalization.schema_release_id != schema {
            return Err(SeriesHotOperatorErrorV3::RecordMismatch);
        }
        authenticate_finalized_record(registry_program, &record.raw, &record.finalization)
            .map_err(SeriesHotOperatorErrorV3::Observation)?;
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
    let ticket = admit_ticket(&state.ticket.raw.data).map_err(SeriesHotOperatorErrorV3::Series)?;
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
                    .map_err(SeriesHotOperatorErrorV3::SeriesState)?,
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
    .map_err(SeriesHotOperatorErrorV3::SeriesOperator)
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
            .map_err(SeriesHotOperatorErrorV3::AccountProfile)?;
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

fn validate_strategy_selection(
    state: &SeriesOccurrenceHotStateV3,
    outer: CheckedHotOuterReleaseV3,
    registry_program: Pubkey,
    bundle: SeriesArtifactBundleV3<'_>,
) -> Result<Option<[u8; 32]>, SeriesHotOperatorErrorV3> {
    match bundle.strategy.disposition() {
        StrategyDispositionV2::Interpreted => {
            if bundle.strategy.transport_profile()
                != Ok(AcceleratorTransportProfileV2::ChunkedBankV2)
                || !state.strategy_accounts.is_empty()
                || state.shadow_accelerator.is_some()
            {
                return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
            }
            return Ok(None);
        }
        StrategyDispositionV2::ShadowAot => {}
        StrategyDispositionV2::AdmittedAot => {
            return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
        }
    }
    if bundle.strategy.transport_profile() != Ok(AcceleratorTransportProfileV2::ShadowTranscriptV3)
        || state.strategy_accounts.len() != SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3
    {
        return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
    }
    let checked = state
        .shadow_accelerator
        .ok_or(SeriesHotOperatorErrorV3::CheckedReleaseUnavailable)?;
    require_checked_shadow_identities(outer, checked)?;
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
            account,
            &FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(SeriesHotOperatorErrorV3::Observation)?;
    }
    let certificate_id = content_id(&certificate_raw.account.data)?;
    let strategy_id = content_id(&fixed(state, HOT_STRATEGY_RAW_ACCOUNT_V3)?.account.data)?;
    let certificate = ExecutionStrategyCertificateV2::decode(&certificate_raw.account.data)
        .map_err(SeriesHotOperatorErrorV3::ExecutionStrategy)?;
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
        .map_err(SeriesHotOperatorErrorV3::ExecutionStrategy)?;
    let artifact_id = ArtifactReleaseIdV1::new(checked.artifact_release)
        .map_err(SeriesHotOperatorErrorV3::ReleaseSet)?;
    certificate
        .validate_artifact(artifact_id)
        .map_err(SeriesHotOperatorErrorV3::ExecutionStrategy)?;
    if hash(&artifact_raw.account.data).to_bytes() != checked.artifact_release {
        return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
    }
    let artifact = ArtifactReleaseV1::decode(&artifact_raw.account.data)
        .map_err(SeriesHotOperatorErrorV3::Registry)?;
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
        return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
    }
    Ok(Some(checked.artifact_release))
}

fn require_checked_outer(outer: CheckedHotOuterReleaseV3) -> Result<(), SeriesHotOperatorErrorV3> {
    if outer.trading_program == Pubkey::default()
        || outer.artifact_release == [0; 32]
        || outer.checked_manifest_digest == [0; 32]
    {
        return Err(SeriesHotOperatorErrorV3::ZeroIdentity);
    }
    Ok(())
}

fn require_checked_shadow_identities(
    outer: CheckedHotOuterReleaseV3,
    accelerator: CheckedSeriesShadowAcceleratorV3,
) -> Result<(), SeriesHotOperatorErrorV3> {
    require_checked_outer(outer)?;
    if accelerator.artifact_release == [0; 32]
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
            dclutch_market::capability_program::hot_v3::HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
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

/// Mine the bumps this family's readers would otherwise search for on chain.
///
/// The DERIVATION is `dclutch_hot_bump_miner_v1`'s, shared with the Direct
/// builder, the Dealer LP builder, the Rational public outer builders and the
/// campaign's bundle builder. This function owns only the CORPUS -- which
/// coordinate of the Series occurrence Hot frame is the Market, which is the root, and which account
/// names the Custody deployment.
///
/// Every hint is reproduced by the reader with `create_program_address` against
/// the account the frame supplied, so a wrong byte names a different address
/// and refuses at an equality that was already there. No conjunct moves.
///
/// # Which slots this corpus reaches, and which it deliberately leaves
///
/// `market`, `root` and Custody's transfer authority are derivable from the
/// frame this builder already authenticated. `child_relay[0]` is Custody's
/// replay cursor, whose seeds end in the projected child request's replay
/// context; `child_caller`'s seeds end in a digest over a request projected ON
/// chain; `lifecycle` is this family's created accounts in materialization
/// order. None of the three is projected here, so all three stay zero and
/// search, which is correct and merely slower.
fn series_occurrence_hot_bump_hints_v3(
    state: &SeriesOccurrenceHotStateV3,
    trading_program: &Pubkey,
    release_set: [u8; 32],
) -> Result<HotBumpHintsV1, SeriesHotOperatorErrorV3> {
    let market = &fixed(state, HOT_MARKET_ACCOUNT_V3)?.account;
    // Custody is not in the hot fixed frame; the Market's activation cache is,
    // and it names the release set's Custody deployment.
    let activation = &fixed(state, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    Ok(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: market.key,
        market_data: &market.data,
        root_data: &fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.data,
        core_program: fixed(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?.account.key,
        trading_program: *trading_program,
        custody_program: activated_custody_program_v1(&activation.data),
        release_set,
    }))
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
        .ok_or(SeriesHotOperatorErrorV3::StrategySelectionMismatch)
}

fn content_id(bytes: &[u8]) -> Result<ContentId, SeriesHotOperatorErrorV3> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| SeriesHotOperatorErrorV3::StrategySelectionMismatch)
}

fn account_meta(value: &ObservedAccountMetaV3) -> AccountMeta {
    AccountMeta {
        pubkey: value.account.key,
        is_signer: value.is_signer,
        is_writable: value.is_writable,
    }
}

/// Same-finalized physical evidence for the current five-action Series release.
///
/// `runtime_physical_accounts` is the packed AccountProfile vector, including
/// the five injected entries.  It is deliberately not a client-side expansion:
/// the selected release supplies the only logical-to-physical mapping.
pub struct SeriesCurrentHotStateV5<'a> {
    /// Exact generic Hot fixed account prefix.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Consume-only Shadow physical extras in canonical certificate/artifact/
    /// program/programdata/caller order; empty for interpreted actions.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact packed selected runtime accounts, including injected prefix entries.
    pub runtime_physical_accounts: Vec<ObservedAccountMetaV3>,
    /// One same-slot lifecycle snapshot; it is the only action selector.
    pub lifecycle: SeriesLifecycleSnapshotV3<'a>,
    /// Optional still-vacant, prefunded founding-permit PDA. Only Expire supplies one.
    pub permit: Option<ObservedAccount>,
}

/// Actual selected physical keys, resolved through the authenticated mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesSelectedRoleKeysV5 {
    /// Composite Series root.
    pub root: Pubkey,
    /// Selected Ticket or Ticket replay account.
    pub ticket: Option<Pubkey>,
    /// Selected lifecycle RentCredit.
    pub rent_credit: Option<Pubkey>,
    /// Prepare payer.
    pub payer: Option<Pubkey>,
    /// Prepare surplus refund destination.
    pub refund: Option<Pubkey>,
    /// Prepare System program.
    pub system_program: Option<Pubkey>,
    /// Distinct future occurrence Market, absent for terminal actions.
    pub occurrence_market: Option<Pubkey>,
    /// Future occurrence generation, absent for terminal actions.
    pub occurrence_generation: Option<u64>,
    /// Expire's authenticated permit account, absent for every other action.
    pub permit: Option<Pubkey>,
}

/// Ready generic-Hot instruction authenticated from the current five-action source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesSelectedHotReportV5 {
    /// Exact selected release artifacts, geometry, authority, and bindings.
    pub selected: SeriesSelectedActionV5,
    /// Sole unsigned generic Trading Hot instruction.
    pub instruction: Instruction,
    /// Same finalized observation for every physical account and root envelope.
    pub observation: Observation,
    /// Selected Trading program.
    pub trading_program: Pubkey,
    /// Root-authenticated live predecessor/controller Market.
    pub parent_market: Pubkey,
    /// Root-authenticated nonzero predecessor/controller generation.
    pub parent_generation: u64,
    /// Root-authenticated release set.
    pub release_set: [u8; 32],
    /// SHA-256 identity of the exact selected five-entry ProgramSet.
    pub program_set_id: [u8; 32],
    /// Resolved physical role keys.
    pub roles: SeriesSelectedRoleKeysV5,
    /// Lifecycle's stable economic consequence; no second policy lives here.
    pub consequence: crate::series_lifecycle_v3::SeriesConsequenceV3,
}

/// Gricean current-source result: evidence acquisition, a wait boundary, or a
/// ready unsigned instruction.  This adapter deliberately has no action input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SeriesCurrentHotPlanV5 {
    /// Lifecycle needs another authenticated observation.
    Acquire(crate::series_lifecycle_v3::SeriesAcquisitionV3),
    /// A selected prepared Ticket is valid but not yet schedulable.
    WaitUntil {
        /// First slot at which the selected Consume may be constructed.
        scheduled_slot: u64,
    },
    /// The sole authenticated selected action and generic Hot instruction.
    Ready(SeriesSelectedHotReportV5),
}

/// Derive the current act from lifecycle state, emit the current five-action
/// source, authenticate that exact selected release, and construct generic Hot.
pub fn inspect_current_series_hot_v5(
    state: &SeriesCurrentHotStateV5<'_>,
    current_source: SeriesCurrentReleaseInputV5<'_>,
) -> Result<SeriesCurrentHotPlanV5, SeriesHotOperatorErrorV3> {
    let plan = select_current_series_plan_v5(state.lifecycle)?;
    let plan = match plan {
        SeriesNextActV3::Acquire(value) => return Ok(SeriesCurrentHotPlanV5::Acquire(value)),
        SeriesNextActV3::WaitUntil { scheduled_slot } => {
            return Ok(SeriesCurrentHotPlanV5::WaitUntil { scheduled_slot });
        }
        SeriesNextActV3::Ready(value) => value,
    };
    build_current_series_hot_v5_ready(state, current_source, plan)
}

fn select_current_series_plan_v5(
    lifecycle: SeriesLifecycleSnapshotV3<'_>,
) -> Result<SeriesNextActV3, SeriesHotOperatorErrorV3> {
    Ok(inspect_series_lifecycle_v3(lifecycle)
        .map_err(SeriesHotOperatorErrorV3::SeriesOperator)?
        .next())
}

fn build_current_series_hot_v5_ready(
    state: &SeriesCurrentHotStateV5<'_>,
    current_source: SeriesCurrentReleaseInputV5<'_>,
    plan: crate::series_lifecycle_v3::PlannedSeriesActV3,
) -> Result<SeriesCurrentHotPlanV5, SeriesHotOperatorErrorV3> {
    let source = emit_current_series_release_source_v5(current_source)
        .map_err(SeriesHotOperatorErrorV3::SeriesRelease)?;
    let release = compile_series_release_v5(source.as_source())
        .map_err(SeriesHotOperatorErrorV3::SeriesRelease)?;
    let selected = authenticate_series_selected_action_v5(
        &release,
        source.as_source(),
        plan.request().as_bytes(),
    )
    .map_err(SeriesHotOperatorErrorV3::SeriesRelease)?;
    if selected.action != plan.action() {
        return Err(SeriesHotOperatorErrorV3::ActionMismatch);
    }
    let strategy = ExecutionStrategyProgramV2::decode(&selected.artifacts.strategy)
        .map_err(SeriesHotOperatorErrorV3::ExecutionStrategy)?;
    let expected = if selected.action == SeriesActionV3::Consume {
        StrategyDispositionV2::ShadowAot
    } else {
        StrategyDispositionV2::Interpreted
    };
    if strategy.disposition() != expected
        || (expected == StrategyDispositionV2::ShadowAot
            && strategy.transport_profile()
                != Ok(AcceleratorTransportProfileV2::ShadowTranscriptV3))
    {
        return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
    }
    build_selected_series_hot_v5(state, release, selected, plan.consequence())
        .map(SeriesCurrentHotPlanV5::Ready)
}

fn build_selected_series_hot_v5(
    state: &SeriesCurrentHotStateV5<'_>,
    release: SeriesReleaseV5,
    selected: SeriesSelectedActionV5,
    consequence: crate::series_lifecycle_v3::SeriesConsequenceV3,
) -> Result<SeriesSelectedHotReportV5, SeriesHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.runtime_physical_accounts.len() != selected.geometry.physical_accounts
        || selected.geometry.physical_accounts < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
    {
        return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
    }
    validate_strategy_accounts_v5(selected.action, &state.strategy_accounts)?;
    let is_consume = selected.action == SeriesActionV3::Consume;
    let market = fixed_v5(state, HOT_MARKET_ACCOUNT_V3)?;
    let root = fixed_v5(state, HOT_ROOT_ACCOUNT_V3)?;
    let trading = fixed_v5(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let program_set = fixed_v5(state, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?;
    let descriptor = fixed_v5(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?;
    let lifecycle = fixed_v5(state, HOT_LIFECYCLE_RAW_ACCOUNT_V3)?;
    let config = fixed_v5(state, HOT_CONFIG_RAW_ACCOUNT_V3)?;
    let artifact_hash = |index, expected| {
        fixed_v5(state, index).map(|account| hash(&account.account.data).to_bytes() == expected)
    };
    if !trading.account.executable
        || trading.is_signer
        || trading.is_writable
        || root.account.owner != trading.account.key
        || !root.is_writable
        || root.is_signer
        || hash(&program_set.account.data).to_bytes() != release.program_set_id
        || descriptor.account.data.as_slice() != selected.descriptor
        || hash(&lifecycle.account.data).to_bytes() != selected.artifact_ids.lifecycle
        || hash(&config.account.data).to_bytes() != hash(state.lifecycle.template_bytes).to_bytes()
        || !artifact_hash(
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            selected.artifact_ids.account_profile,
        )?
        || !artifact_hash(
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            selected.artifact_ids.request_profile,
        )?
        || !artifact_hash(HOT_STRATEGY_RAW_ACCOUNT_V3, selected.artifact_ids.strategy)?
        || !artifact_hash(
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            selected.artifact_ids.transition,
        )?
        || !artifact_hash(HOT_EFFECT_RAW_ACCOUNT_V3, selected.artifact_ids.effect)?
        || market.account.observation.finality != Finality::Finalized
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    let header = CapabilityRootHeaderV1::decode(
        root.account
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(SeriesHotOperatorErrorV3::FixedFrameMismatch)?,
    )
    .map_err(SeriesHotOperatorErrorV3::CapabilityProgram)?;
    if header.generation() == 0 || header.selection().executor_role() != ExecutionRoleV1::Trading {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    require_fixed_market_v5(header.market(), market.account.key)?;
    match selected.authority {
        SeriesOccurrenceAuthorityV5::Expire {
            market: occurrence_market,
            generation: occurrence_generation,
            ..
        } => {
            require_expire_controller_and_future_market_v5(
                header.market(),
                market.account.key,
                occurrence_market,
                occurrence_generation,
            )?;
        }
        _ => {}
    }
    if !matches!(selected.authority, SeriesOccurrenceAuthorityV5::Terminal)
        && (header.release_set().to_bytes() != selected_release_set(selected.authority)
            || selected_parent_root(selected.authority) != Some(root.account.key))
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    let observation = market.account.observation;
    for account in state
        .fixed_accounts
        .iter()
        .chain(&state.strategy_accounts)
        .chain(&state.runtime_physical_accounts)
    {
        if account.account.observation != observation || observation.finality != Finality::Finalized
        {
            return Err(SeriesHotOperatorErrorV3::ObservationMismatch);
        }
    }
    let injected = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        dclutch_market::capability_program::hot_v3::HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ];
    for (ordinal, physical) in injected.into_iter().enumerate() {
        if state.runtime_physical_accounts[ordinal] != state.fixed_accounts[physical] {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    validate_selected_bindings_v5(&selected, &state.runtime_physical_accounts)?;
    let roles = resolve_roles_v5(state, &selected)?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(selected.request_bytes.len())
            .map_err(|_| SeriesHotOperatorErrorV3::Arithmetic)?,
        header.release_set().to_bytes(),
        header.market(),
        header.generation(),
        hash(&root.account.data).to_bytes(),
    )
    .map_err(SeriesHotOperatorErrorV3::HotExecution)?
    .with_bump_hints(series_selected_hot_bump_hints_v5(
        state,
        &trading.account.key,
        header.release_set().to_bytes(),
    )?);
    let mut data = envelope.to_bytes().to_vec();
    data.extend_from_slice(&selected.request_bytes);
    let mut accounts: Vec<AccountMeta> = state.fixed_accounts.iter().map(account_meta).collect();
    accounts.extend(state.strategy_accounts.iter().map(account_meta));
    accounts.extend(
        state
            .runtime_physical_accounts
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(account_meta),
    );
    for key in [
        Some(roles.root),
        roles.ticket,
        roles.rent_credit,
        roles.payer,
        roles.refund,
        roles.system_program,
    ]
    .into_iter()
    .flatten()
    {
        if !accounts.iter().any(|account| account.pubkey == key) {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    for key in [roles.occurrence_market, roles.permit]
        .into_iter()
        .flatten()
    {
        if !accounts.iter().any(|account| account.pubkey == key) {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    if is_consume {
        let caller = strategy_v5(state, SHADOW_CALLER_AUTHORITY_V3)?;
        if caller.is_signer
            || caller.is_writable
            || caller.account.executable
            || !accounts
                .iter()
                .any(|account| account.pubkey == caller.account.key)
        {
            return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
        }
    }
    Ok(SeriesSelectedHotReportV5 {
        selected,
        instruction: Instruction {
            program_id: trading.account.key,
            accounts,
            data,
        },
        observation,
        trading_program: trading.account.key,
        parent_market: Pubkey::new_from_array(header.market()),
        parent_generation: header.generation(),
        release_set: header.release_set().to_bytes(),
        program_set_id: release.program_set_id,
        roles,
        consequence,
    })
}

/// Mine the bumps this family's readers would otherwise search for on chain.
///
/// The DERIVATION is `dclutch_hot_bump_miner_v1`'s, shared with the Direct
/// builder, the Dealer LP builder, the Rational public outer builders and the
/// campaign's bundle builder. This function owns only the CORPUS -- which
/// coordinate of the selected Series Hot frame is the Market, which is the root, and which account
/// names the Custody deployment.
///
/// Every hint is reproduced by the reader with `create_program_address` against
/// the account the frame supplied, so a wrong byte names a different address
/// and refuses at an equality that was already there. No conjunct moves.
///
/// # Which slots this corpus reaches, and which it deliberately leaves
///
/// `market`, `root` and Custody's transfer authority are derivable from the
/// frame this builder already authenticated. `child_relay[0]` is Custody's
/// replay cursor, whose seeds end in the projected child request's replay
/// context; `child_caller`'s seeds end in a digest over a request projected ON
/// chain; `lifecycle` is this family's created accounts in materialization
/// order. None of the three is projected here, so all three stay zero and
/// search, which is correct and merely slower.
fn series_selected_hot_bump_hints_v5(
    state: &SeriesCurrentHotStateV5<'_>,
    trading_program: &Pubkey,
    release_set: [u8; 32],
) -> Result<HotBumpHintsV1, SeriesHotOperatorErrorV3> {
    let market = &fixed_v5(state, HOT_MARKET_ACCOUNT_V3)?.account;
    // Custody is not in the hot fixed frame; the Market's activation cache is,
    // and it names the release set's Custody deployment.
    let activation = &fixed_v5(state, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    Ok(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: market.key,
        market_data: &market.data,
        root_data: &fixed_v5(state, HOT_ROOT_ACCOUNT_V3)?.account.data,
        core_program: fixed_v5(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?.account.key,
        trading_program: *trading_program,
        custody_program: activated_custody_program_v1(&activation.data),
        release_set,
    }))
}

fn fixed_v5<'a>(
    state: &'a SeriesCurrentHotStateV5<'_>,
    index: usize,
) -> Result<&'a ObservedAccountMetaV3, SeriesHotOperatorErrorV3> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(SeriesHotOperatorErrorV3::FixedFrameMismatch)
}

fn require_fixed_market_v5(
    header_market: [u8; 32],
    fixed_market: Pubkey,
) -> Result<(), SeriesHotOperatorErrorV3> {
    if header_market != fixed_market.to_bytes() {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    Ok(())
}

fn require_expire_controller_and_future_market_v5(
    controller_market: [u8; 32],
    fixed_market: Pubkey,
    future_market: [u8; 32],
    future_generation: u64,
) -> Result<(), SeriesHotOperatorErrorV3> {
    if controller_market != fixed_market.to_bytes()
        || future_market == controller_market
        || future_generation == 0
    {
        return Err(SeriesHotOperatorErrorV3::FixedFrameMismatch);
    }
    Ok(())
}

fn strategy_v5<'a>(
    state: &'a SeriesCurrentHotStateV5<'_>,
    index: usize,
) -> Result<&'a ObservedAccountMetaV3, SeriesHotOperatorErrorV3> {
    state
        .strategy_accounts
        .get(index)
        .ok_or(SeriesHotOperatorErrorV3::StrategySelectionMismatch)
}

fn validate_strategy_accounts_v5(
    action: SeriesActionV3,
    accounts: &[ObservedAccountMetaV3],
) -> Result<(), SeriesHotOperatorErrorV3> {
    let expected = if action == SeriesActionV3::Consume {
        SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3
    } else {
        0
    };
    if accounts.len() != expected {
        return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
    }
    if action == SeriesActionV3::Consume {
        let caller = accounts
            .get(SHADOW_CALLER_AUTHORITY_V3)
            .ok_or(SeriesHotOperatorErrorV3::StrategySelectionMismatch)?;
        if caller.is_signer || caller.is_writable || caller.account.executable {
            return Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch);
        }
    }
    Ok(())
}

fn validate_selected_bindings_v5(
    selected: &SeriesSelectedActionV5,
    physical: &[ObservedAccountMetaV3],
) -> Result<(), SeriesHotOperatorErrorV3> {
    if selected.account_bindings.len() != selected.geometry.logical_accounts {
        return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
    }
    for (logical, binding) in selected.account_bindings.iter().enumerate() {
        if binding.logical != logical
            || binding.representative > logical
            || binding.physical_ordinal >= physical.len()
            || selected
                .account_bindings
                .get(binding.representative)
                .map(|representative| representative.physical_ordinal)
                != Some(binding.physical_ordinal)
        {
            return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

fn role_key_v5(
    bindings: &[SeriesLogicalPhysicalBindingV5],
    physical: &[ObservedAccountMetaV3],
    coordinate: u16,
) -> Result<Pubkey, SeriesHotOperatorErrorV3> {
    let binding = bindings
        .get(usize::from(coordinate))
        .ok_or(SeriesHotOperatorErrorV3::RuntimeProfileMismatch)?;
    physical
        .get(binding.physical_ordinal)
        .map(|value| value.account.key)
        .ok_or(SeriesHotOperatorErrorV3::RuntimeProfileMismatch)
}

pub(crate) fn authenticate_expire_permit_v5(
    selected: &SeriesSelectedActionV5,
    core: &ObservedAccountMetaV3,
    rent_account: &ObservedAccount,
    controller_observation: Observation,
    observed: &ObservedAccount,
    physical: &[ObservedAccountMetaV3],
) -> Result<Pubkey, SeriesHotOperatorErrorV3> {
    let SeriesOccurrenceAuthorityV5::Expire {
        market,
        release_set,
        ..
    } = selected.authority
    else {
        return Err(SeriesHotOperatorErrorV3::ActionMismatch);
    };
    let request = SeriesActionRequestV3::decode(&selected.request_bytes)
        .map_err(SeriesHotOperatorErrorV3::SeriesInstruction)?;
    let ticket = request
        .ticket()
        .ok_or(SeriesHotOperatorErrorV3::ActionMismatch)?;
    let seeds = SeriesFoundingPermitSeedsV1::new(
        CoreIdentity::new(release_set).map_err(SeriesHotOperatorErrorV3::MarketCore)?,
        CoreIdentity::new(market).map_err(SeriesHotOperatorErrorV3::MarketCore)?,
        CoreIdentity::new(ticket.to_bytes()).map_err(SeriesHotOperatorErrorV3::MarketCore)?,
    );
    // The Rent sysvar is still AUTHENTICATED here -- key, owner, executable bit,
    // exact width, canonical body -- even though nothing prices a floor against
    // it any more. Dropping the decode with the floor would silently stop
    // checking the coordinate, which is the debt `a4b2cbb17` named at
    // `authenticate_execution_strategy_v2` and this does not repeat.
    decode_rent(rent_account).map_err(SeriesHotOperatorErrorV3::Observation)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), &core.account.key).0;
    let mut matches = physical.iter().filter(|value| value.account == *observed);
    let permit_meta = matches
        .next()
        .ok_or(SeriesHotOperatorErrorV3::ActionMismatch)?;
    if matches.next().is_some()
        || request.action() != SeriesActionV3::Expire
        || observed.observation != controller_observation
        || observed.observation.finality != Finality::Finalized
        || observed.key != expected
        || observed.owner != system_program::ID
        || observed.executable
        || !observed.data.is_empty()
        || !funded_rent_persists_v1(observed.lamports)
        || !permit_meta.is_writable
        || permit_meta.is_signer
        || !core.account.executable
        || core.account.owner != bpf_loader_upgradeable::ID
        || core.is_writable
        || core.is_signer
    {
        return Err(SeriesHotOperatorErrorV3::ActionMismatch);
    }
    Ok(expected)
}

fn resolve_roles_v5(
    state: &SeriesCurrentHotStateV5<'_>,
    selected: &SeriesSelectedActionV5,
) -> Result<SeriesSelectedRoleKeysV5, SeriesHotOperatorErrorV3> {
    let key = |coordinate| {
        role_key_v5(
            &selected.account_bindings,
            &state.runtime_physical_accounts,
            coordinate,
        )
    };
    let root = key(selected.roles.root)?;
    if root != fixed_v5(state, HOT_ROOT_ACCOUNT_V3)?.account.key {
        return Err(SeriesHotOperatorErrorV3::RuntimeProfileMismatch);
    }
    let optional = |coordinate: Option<u16>| coordinate.map(key).transpose();
    let (occurrence_market, occurrence_generation, permit) = match selected.authority {
        SeriesOccurrenceAuthorityV5::Prepare {
            market, generation, ..
        }
        | SeriesOccurrenceAuthorityV5::Consume {
            market, generation, ..
        } => {
            if state.permit.is_some() {
                return Err(SeriesHotOperatorErrorV3::ActionMismatch);
            }
            (Some(Pubkey::new_from_array(market)), Some(generation), None)
        }
        SeriesOccurrenceAuthorityV5::Expire {
            market, generation, ..
        } => {
            let observed = state
                .permit
                .as_ref()
                .ok_or(SeriesHotOperatorErrorV3::ActionMismatch)?;
            let core = fixed_v5(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?;
            let expected = authenticate_expire_permit_v5(
                selected,
                core,
                &fixed_v5(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?.account,
                fixed_v5(state, HOT_MARKET_ACCOUNT_V3)?.account.observation,
                observed,
                &state.runtime_physical_accounts,
            )?;
            (
                Some(Pubkey::new_from_array(market)),
                Some(generation),
                Some(expected),
            )
        }
        SeriesOccurrenceAuthorityV5::Terminal => {
            if state.permit.is_some() {
                return Err(SeriesHotOperatorErrorV3::ActionMismatch);
            }
            (None, None, None)
        }
    };
    Ok(SeriesSelectedRoleKeysV5 {
        root,
        ticket: optional(selected.roles.ticket)?,
        rent_credit: optional(selected.roles.rent_credit)?,
        payer: optional(selected.roles.payer)?,
        refund: optional(selected.roles.refund)?,
        system_program: optional(selected.roles.system_program)?,
        occurrence_market,
        occurrence_generation,
        permit,
    })
}

fn selected_release_set(authority: SeriesOccurrenceAuthorityV5) -> [u8; 32] {
    match authority {
        SeriesOccurrenceAuthorityV5::Prepare { release_set, .. }
        | SeriesOccurrenceAuthorityV5::Consume { release_set, .. }
        | SeriesOccurrenceAuthorityV5::Expire { release_set, .. } => release_set,
        SeriesOccurrenceAuthorityV5::Terminal => [0; 32],
    }
}

fn selected_parent_root(authority: SeriesOccurrenceAuthorityV5) -> Option<Pubkey> {
    match authority {
        SeriesOccurrenceAuthorityV5::Prepare { parent_root, .. }
        | SeriesOccurrenceAuthorityV5::Consume { parent_root, .. }
        | SeriesOccurrenceAuthorityV5::Expire { parent_root, .. } => {
            Some(Pubkey::new_from_array(parent_root))
        }
        SeriesOccurrenceAuthorityV5::Terminal => None,
    }
}

const _: () = assert!(
    SERIES_ROOT_ACCOUNT_BYTES_V3 == CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
);

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market::SERIES_FOUNDING_PERMIT_BYTES_V1;
    use dclutch_trading::series::{generated, request::encode_series_action_header_v3};
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};

    use dclutch_trading_sbf::series::release_v5::{
        SeriesActionArtifactIdsV5, SeriesSelectedArtifactBodiesV5, SeriesSelectedGeometryV5,
        SeriesSelectedRolesV5,
    };

    fn observation() -> Observation {
        Observation {
            slot: 9,
            unix_timestamp: 10,
            finality: Finality::Finalized,
        }
    }

    fn test_id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("test identity")
    }

    fn expire_selected(
        future_market: [u8; 32],
        release_set: [u8; 32],
        parent_root: [u8; 32],
        ticket: ContentId,
    ) -> SeriesSelectedActionV5 {
        SeriesSelectedActionV5 {
            action: SeriesActionV3::Expire,
            request_bytes: encode_series_action_header_v3(
                SeriesActionV3::Expire,
                test_id(1),
                Some(test_id(2)),
                Some(ticket),
                3,
                4,
                0,
            )
            .expect("Expire request")
            .to_vec(),
            descriptor: [0; dclutch_market::capability_program::v4::CAPABILITY_PROGRAM_V4_BYTES],
            artifact_ids: SeriesActionArtifactIdsV5 {
                account_profile: [1; 32],
                request_profile: [2; 32],
                lifecycle: [3; 32],
                strategy: [4; 32],
                transition: [5; 32],
                effect: [6; 32],
            },
            geometry: SeriesSelectedGeometryV5 {
                logical_fixed_accounts: 1,
                logical_accounts: 1,
                physical_accounts: 1,
                common_scalars: 0,
                common_identities: 0,
                route_count: 0,
            },
            roles: SeriesSelectedRolesV5 {
                root: 0,
                ticket: None,
                rent_credit: None,
                payer: None,
                refund: None,
                system_program: None,
            },
            authority: SeriesOccurrenceAuthorityV5::Expire {
                market: future_market,
                generation: 7,
                release_set,
                parent_root,
            },
            account_bindings: Vec::new(),
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
        }
    }

    fn rent_observation(rent: &Rent) -> ObservedAccount {
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

    fn exact_expire_permit(
        selected: &SeriesSelectedActionV5,
        core: Pubkey,
        rent: &Rent,
    ) -> ObservedAccount {
        let SeriesOccurrenceAuthorityV5::Expire {
            market,
            release_set,
            ..
        } = selected.authority
        else {
            panic!("Expire authority")
        };
        let ticket = SeriesActionRequestV3::decode(&selected.request_bytes)
            .expect("request")
            .ticket()
            .expect("ticket");
        let seeds = SeriesFoundingPermitSeedsV1::new(
            CoreIdentity::new(release_set).expect("release"),
            CoreIdentity::new(market).expect("Market"),
            CoreIdentity::new(ticket.to_bytes()).expect("ticket"),
        );
        ObservedAccount {
            observation: observation(),
            key: Pubkey::find_program_address(&seeds.as_slices(), &core).0,
            owner: system_program::ID,
            lamports: rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1),
            executable: false,
            data: Vec::new(),
        }
    }

    fn core_meta(key: Pubkey) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1,
                executable: true,
                data: vec![1],
            },
            is_signer: false,
            is_writable: false,
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
    fn checked_outer_is_sufficient_for_interpreted_execution() {
        let outer = CheckedHotOuterReleaseV3 {
            trading_program: Pubkey::new_from_array([1; 32]),
            artifact_release: [2; 32],
            checked_manifest_digest: [3; 32],
        };
        assert_eq!(require_checked_outer(outer), Ok(()));
        let mut zero = outer;
        zero.checked_manifest_digest = [0; 32];
        assert_eq!(
            require_checked_outer(zero),
            Err(SeriesHotOperatorErrorV3::ZeroIdentity)
        );
    }

    #[test]
    fn checked_outer_and_shadow_accelerator_must_share_manifest() {
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
        assert_eq!(
            require_checked_shadow_identities(outer, accelerator),
            Ok(())
        );
        accelerator.checked_manifest_digest = [7; 32];
        assert_eq!(
            require_checked_shadow_identities(outer, accelerator),
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

    #[test]
    fn v5_interpreted_has_no_extras_and_consume_requires_exact_shadow_caller() {
        assert_eq!(
            validate_strategy_accounts_v5(SeriesActionV3::Prepare, &[]),
            Ok(())
        );
        assert_eq!(
            validate_strategy_accounts_v5(SeriesActionV3::Prepare, &[meta(1)]),
            Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch)
        );
        let shadow = (0..SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3)
            .map(|index| meta(u8::try_from(index + 1).expect("small fixture index")))
            .collect::<Vec<_>>();
        assert_eq!(
            validate_strategy_accounts_v5(SeriesActionV3::Consume, &shadow),
            Ok(())
        );
        assert_eq!(
            validate_strategy_accounts_v5(SeriesActionV3::Consume, &shadow[..6]),
            Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch)
        );
        let mut substituted_caller = shadow;
        substituted_caller[SHADOW_CALLER_AUTHORITY_V3].is_signer = true;
        assert_eq!(
            validate_strategy_accounts_v5(SeriesActionV3::Consume, &substituted_caller),
            Err(SeriesHotOperatorErrorV3::StrategySelectionMismatch)
        );
    }

    #[test]
    fn v5_planner_acquire_needs_no_current_source_or_physical_frame() {
        let template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let selected = select_current_series_plan_v5(SeriesLifecycleSnapshotV3 {
            template_bytes: &template,
            series: SeriesStateV3::new(7),
            now_slot: 0,
            current: None,
            terminal_ticket: None,
            observed_root_lamports: 0,
            exact_root_rent: 0,
            rent_sink: None,
        })
        .expect("minimal lifecycle selection");
        assert_eq!(
            selected,
            SeriesNextActV3::Acquire(
                crate::series_lifecycle_v3::SeriesAcquisitionV3::CurrentOccurrence {
                    occurrence: 0
                }
            )
        );
    }

    #[test]
    fn v5_planner_wait_until_needs_no_current_source_or_physical_frame() {
        use dclutch_trading::series::{
            admit_occurrence, admit_ticket, occurrence_content_id, template_content_id,
        };
        use solana_program::hash::hashv;

        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3;
        let siblings = [[90; 32], [91; 32]];
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let mut node = occurrence_id.to_bytes();
        let mut index = 1_u32;
        for sibling in siblings {
            node = if index & 1 == 0 {
                hashv(&[
                    &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                    &[0],
                    &node,
                    &sibling,
                ])
            } else {
                hashv(&[
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
        let scheduled = admit_occurrence(&template, &occurrence, &siblings)
            .expect("occurrence")
            .occurrence()
            .scheduled_slot();
        let ticket_id = admit_ticket(&ticket).expect("ticket").content_id();
        let at_one = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare zero")
            .settle_current(1, 3)
            .expect("settle zero")
            .retire_ticket(2)
            .expect("retire zero");
        let prepared = at_one.prepare_ticket(at_one.revision()).expect("prepared");
        let selected = select_current_series_plan_v5(SeriesLifecycleSnapshotV3 {
            template_bytes: &template,
            series: prepared,
            now_slot: scheduled - 1,
            current: Some(crate::series_lifecycle_v3::SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &occurrence,
                ticket_bytes: &ticket,
                siblings: &siblings,
                ticket_state: Some(TicketStateV3::prepared(ticket_id)),
            }),
            terminal_ticket: None,
            observed_root_lamports: 0,
            exact_root_rent: 0,
            rent_sink: None,
        })
        .expect("wait lifecycle selection");
        assert_eq!(
            selected,
            SeriesNextActV3::WaitUntil {
                scheduled_slot: scheduled
            }
        );
    }

    #[test]
    fn v5_expire_keeps_one_controller_root_across_distinct_future_markets() {
        let controller = Pubkey::new_from_array([41; 32]);
        let first_future = [42; 32];
        let second_future = [43; 32];
        assert_eq!(
            require_expire_controller_and_future_market_v5(
                controller.to_bytes(),
                controller,
                first_future,
                7,
            ),
            Ok(())
        );
        assert_eq!(
            require_expire_controller_and_future_market_v5(
                controller.to_bytes(),
                controller,
                second_future,
                8,
            ),
            Ok(())
        );

        let selection = dclutch_registry::release_set::CapabilityExecutionSelectionV1::from_bytes(
            0, [51; 32], [52; 32], [53; 32], [54; 32],
        )
        .expect("controller selection");
        let header = CapabilityRootHeaderV1::new(
            ContentId::new([55; 32]).expect("release set"),
            controller.to_bytes(),
            9,
            selection,
            dclutch_market::capability_program::SelectedRecordBumpsV1::default(),
        )
        .expect("persistent controller header");
        let trading = Pubkey::new_from_array([56; 32]);
        let first_root = Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0;
        let second_root = Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0;
        assert_eq!(first_root, second_root);

        assert_eq!(
            require_expire_controller_and_future_market_v5([44; 32], controller, first_future, 7,),
            Err(SeriesHotOperatorErrorV3::FixedFrameMismatch)
        );
        assert_eq!(
            require_expire_controller_and_future_market_v5(
                controller.to_bytes(),
                controller,
                controller.to_bytes(),
                7,
            ),
            Err(SeriesHotOperatorErrorV3::FixedFrameMismatch)
        );
        assert_eq!(
            require_expire_controller_and_future_market_v5(
                controller.to_bytes(),
                controller,
                second_future,
                0,
            ),
            Err(SeriesHotOperatorErrorV3::FixedFrameMismatch)
        );
    }

    #[test]
    fn expire_permit_is_the_exact_vacant_request_derived_runtime_member() {
        let rent = Rent::default();
        let rent_account = rent_observation(&rent);
        let core_key = Pubkey::new_from_array([70; 32]);
        let core = core_meta(core_key);
        let release_set = [71; 32];
        let parent_root = [72; 32];
        let first = expire_selected([73; 32], release_set, parent_root, test_id(74));
        let second = expire_selected([75; 32], release_set, parent_root, test_id(76));
        let first_permit = exact_expire_permit(&first, core_key, &rent);
        let second_permit = exact_expire_permit(&second, core_key, &rent);
        assert_ne!(first_permit.key, second_permit.key);
        for (selected, permit) in [(&first, &first_permit), (&second, &second_permit)] {
            let physical = [ObservedAccountMetaV3 {
                account: permit.clone(),
                is_signer: false,
                is_writable: true,
            }];
            assert_eq!(
                authenticate_expire_permit_v5(
                    selected,
                    &core,
                    &rent_account,
                    observation(),
                    permit,
                    &physical,
                ),
                Ok(permit.key)
            );
        }

        let refuse = |selected: &SeriesSelectedActionV5,
                      core: &ObservedAccountMetaV3,
                      permit: &ObservedAccount,
                      physical: &[ObservedAccountMetaV3]| {
            assert_eq!(
                authenticate_expire_permit_v5(
                    selected,
                    core,
                    &rent_account,
                    observation(),
                    permit,
                    physical,
                ),
                Err(SeriesHotOperatorErrorV3::ActionMismatch)
            );
        };
        let hostile_meta = |account: ObservedAccount| ObservedAccountMetaV3 {
            account,
            is_signer: false,
            is_writable: true,
        };

        let mut wrong_key = first_permit.clone();
        wrong_key.key = Pubkey::new_unique();
        refuse(
            &first,
            &core,
            &wrong_key,
            &[hostile_meta(wrong_key.clone())],
        );
        let mut body = first_permit.clone();
        body.data.push(1);
        refuse(&first, &core, &body, &[hostile_meta(body.clone())]);
        let mut owner = first_permit.clone();
        owner.owner = Pubkey::new_unique();
        refuse(&first, &core, &owner, &[hostile_meta(owner.clone())]);
        // A PERMIT PREPAID AT A CHEAPER RATE IS NOT A HOSTILE. This expiry
        // REFUNDS a slot Core never allocated; the prepayment was made at the
        // rate of an earlier transaction, and one lamport below what the
        // cluster charges today is a slot the founding really did prepay. The
        // old floor refused exactly that and stranded the refund forever, which
        // is `a4b2cbb17`'s ruling with the sign flipped.
        let mut stranded = first_permit.clone();
        stranded.lamports = rent
            .minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
            .saturating_sub(1);
        assert_eq!(
            authenticate_expire_permit_v5(
                &first,
                &core,
                &rent_account,
                observation(),
                &stranded,
                &[hostile_meta(stranded.clone())],
            ),
            Ok(stranded.key),
            "a prepaid slot a risen rate stranded is still the slot the founding prepaid"
        );
        // THE HOSTILE is a DRAINED slot: nothing to refund, and its vacancy is
        // residue rather than a prepayment.
        let mut drained = first_permit.clone();
        drained.lamports = 0;
        refuse(&first, &core, &drained, &[hostile_meta(drained.clone())]);
        let readonly = [ObservedAccountMetaV3 {
            account: first_permit.clone(),
            is_signer: false,
            is_writable: false,
        }];
        refuse(&first, &core, &first_permit, &readonly);
        let signer = [ObservedAccountMetaV3 {
            account: first_permit.clone(),
            is_signer: true,
            is_writable: true,
        }];
        refuse(&first, &core, &first_permit, &signer);
        let duplicate = [
            hostile_meta(first_permit.clone()),
            hostile_meta(first_permit.clone()),
        ];
        refuse(&first, &core, &first_permit, &duplicate);

        let mut wrong_core = core.clone();
        wrong_core.account.key = Pubkey::new_unique();
        refuse(
            &first,
            &wrong_core,
            &first_permit,
            &[hostile_meta(first_permit.clone())],
        );
        let mut wrong_release = first.clone();
        let SeriesOccurrenceAuthorityV5::Expire { release_set, .. } = &mut wrong_release.authority
        else {
            panic!("Expire authority")
        };
        release_set[0] ^= 1;
        refuse(
            &wrong_release,
            &core,
            &first_permit,
            &[hostile_meta(first_permit.clone())],
        );
        let mut wrong_market = first.clone();
        let SeriesOccurrenceAuthorityV5::Expire { market, .. } = &mut wrong_market.authority else {
            panic!("Expire authority")
        };
        market[0] ^= 1;
        refuse(
            &wrong_market,
            &core,
            &first_permit,
            &[hostile_meta(first_permit.clone())],
        );
        let mut wrong_ticket = first.clone();
        wrong_ticket.request_bytes = encode_series_action_header_v3(
            SeriesActionV3::Expire,
            test_id(1),
            Some(test_id(2)),
            Some(test_id(77)),
            3,
            4,
            0,
        )
        .expect("hostile request")
        .to_vec();
        refuse(
            &wrong_ticket,
            &core,
            &first_permit,
            &[hostile_meta(first_permit.clone())],
        );
    }

    /// The corpus this builder mines from reaches this frame's Market, root and
    /// Custody deployment, and not some other coordinate.
    ///
    /// The DERIVATION is `dclutch-hot-bump-miner-v1`'s and has its own tests;
    /// what is per-family, and what nothing tested before 2026-09-03, is which
    /// coordinate of THIS frame each fact is read from. Every other fixture in
    /// this file fills its Market and root accounts with constant bytes, so
    /// both decodes fail, every slot degrades to zero, and a corpus reading the
    /// wrong coordinate would emit exactly the same all-zero block as one
    /// reading the right one -- a disconnected instrument logging as silence.
    ///
    /// `hot_bump_corpus_fixture_v1` stages bodies that DO decode, and derives
    /// the three bumps from the seeds it built them from. Two authors: this
    /// side decodes those bodies and re-derives.
    #[test]
    fn the_mined_corpus_reads_this_frames_market_root_and_custody_deployment() {
        use crate::hot_bump_corpus_fixture_v1 as corpus;
        let occurrence = SeriesOccurrenceHotStateV3 {
            fixed_accounts: corpus::fixed_frame(),
            ..minimal_state(Vec::new())
        };
        assert_eq!(
            series_occurrence_hot_bump_hints_v3(
                &occurrence,
                &corpus::trading_program(),
                corpus::release_set_id()
            )
            .expect("staged corpus mines"),
            corpus::expected_hints()
        );
        // The two Series builders own SEPARATE corpora over the same frame, so
        // the selected route is asserted here too rather than assumed from its
        // sibling.
        let template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let selected = SeriesCurrentHotStateV5 {
            fixed_accounts: corpus::fixed_frame(),
            strategy_accounts: Vec::new(),
            runtime_physical_accounts: Vec::new(),
            lifecycle: SeriesLifecycleSnapshotV3 {
                template_bytes: &template,
                series: SeriesStateV3::new(7),
                now_slot: 0,
                current: None,
                terminal_ticket: None,
                observed_root_lamports: 0,
                exact_root_rent: 0,
                rent_sink: None,
            },
            permit: None,
        };
        assert_eq!(
            series_selected_hot_bump_hints_v5(
                &selected,
                &corpus::trading_program(),
                corpus::release_set_id()
            )
            .expect("staged corpus mines"),
            corpus::expected_hints()
        );
    }
}
