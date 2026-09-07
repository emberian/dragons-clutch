//! Chain-derived unsigned recurring-Series V5 hot execution construction.
//!
//! ONE PATH. [`inspect_current_series_hot_v5`] takes one same-finalized
//! observation of the Series root, its Template and the current occurrence,
//! lets the lifecycle planner (`crate::series_lifecycle_v3`) select the act,
//! re-emits the exact current five-entry V5 release, authenticates the
//! selected action's Profile13 packing and runtime roles, and constructs the
//! sole unsigned generic Trading Hot instruction. It never performs RPC,
//! signs, submits, or treats a client projection as onchain authority; the
//! canonical Trading interpreter and its child receipt chain remain
//! authoritative at execution time. Its callers are
//! [`crate::series_current_acquisition_v5`], the Trading program-test's
//! `series_premarket_expiry_chain_v1` support and the successor's
//! `series_terminal_campaign` -- through which the three runbook verbs
//! `devnet-series-{open,consume,expire}-v1` drive the family.
//!
//! The three action-named V3 builders (`build_series_{prepare,consume,expire}_hot_v3`)
//! and the `SeriesOccurrenceHotStateV3` they consumed were a second,
//! caller-less path over the same frame: an artifacts-V3 join, a
//! `CheckedSeriesShadowAcceleratorV3` bound by ELF digest, and a bump corpus
//! of their own. They were deleted on 2026-09-06 under the parsimony
//! attractor (no parallel legacy/current paths): everything they proved, the
//! V5 path proves against the current release, and a caller for them would
//! have been a caller that ran beside the one that runs.

use crate::hot_bump_miner::{
    HotBumpCorpusV1, activated_custody_program_v1, mine_hot_bump_hints_v1,
};
use crate::series_lifecycle_v3::{
    SeriesLifecycleSnapshotV3, SeriesNextActV3, inspect_series_lifecycle_v3,
};
use crate::{
    Finality, Observation, ObservedAccount, direct_inline_v3::ObservedAccountMetaV3,
    observation::decode_rent,
};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotBumpHintsV1,
        HotExecutionEnvelopeV3,
    },
};
use dclutch_market::execution_strategy::v2::{
    AcceleratorTransportProfileV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market::{Identity as CoreIdentity, SeriesFoundingPermitSeedsV1};
use dclutch_registry::release_set::ExecutionRoleV1;
use dclutch_trading::series::{replay::SERIES_STATE_BYTES_V3, request::SeriesActionRequestV3};
use dclutch_trading_sbf::series::{
    accounts::SERIES_ROOT_ACCOUNT_BYTES_V3,
    instruction::SeriesActionV3,
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
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

/// The seven Consume-only Shadow strategy extras, in canonical order:
/// certificate raw and staging, ArtifactRelease raw and staging, accelerator
/// Program and ProgramData, then the caller authority.
/// `series_current_acquisition_v5` assembles and authenticates them in this
/// order; this module pins the count and the caller's slot.
const SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3: usize = 7;
const SHADOW_CALLER_AUTHORITY_V3: usize = 6;

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
/// The DERIVATION is `crate::hot_bump_miner`'s, shared with the Direct
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
    use dclutch_core_contract::ContentId;
    use dclutch_market::SERIES_FOUNDING_PERMIT_BYTES_V1;
    use dclutch_trading::series::replay::{SeriesStateV3, TicketStateV3};
    use dclutch_trading::series::{generated, request::encode_series_action_header_v3};
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::sysvar;

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

    #[test]
    fn shadow_transaction_geometry_is_six_extras_then_caller() {
        assert_eq!(SHADOW_STRATEGY_PHYSICAL_ACCOUNT_COUNT_V3, 7);
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

    /// The corpus the V5 builder mines from reaches this frame's Market, root
    /// and Custody deployment, and not some other coordinate.
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
