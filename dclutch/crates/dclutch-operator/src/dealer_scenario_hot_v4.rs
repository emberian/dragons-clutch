//! Chain-derived physical-account projection for Dealer scenario exact-fill.
//!
//! Selector 9 has one runtime-width semantic path.  This module re-executes
//! that path from the exact SignedDelta-bearing family request, derives the
//! complete candidate register bank, and lets the canonical Profile13 artifact
//! select all nine protected account spans. Callers cannot supply a Position
//! count, readonly evidence width, Custody-route bitmap, or packed
//! account width separately.
//!
//! Common Hot finalized-record and accelerator authentication remains owned by
//! the family-neutral Trading outer.  This module deliberately returns only
//! the derived account metas and semantic report; the one-instruction wrapper
//! consumes the common authenticated context rather than creating a parallel
//! Dealer authority.

use crate::{Finality, Observation, direct_inline_v3::ObservedAccountMetaV3};
use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HotExecutionEnvelopeV3,
};
use dclutch_dealer_codec::config_v4::DEALER_CONFIG_BYTES_V4;
use dclutch_trading_sbf::{
    admitted_composition_v3::admitted_caller_authority_count_v3,
    dealer::{
        v3_composer::{
            MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3,
            ScenarioCollateralFrameV3, ScenarioComposerContextV3, ScenarioCustodyEffectV3,
        },
        v3_obligation::obligation_account_bytes_v3,
        v3_trade::{
            DealerScenarioTradeRequestV3, ScenarioTradeChainProjectionV3, prepare_scenario_trade_v3,
        },
        v3_trade_artifacts::{
            DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            project_dealer_scenario_hot_registers_v4,
        },
        v3_trade_profile::{
            DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DEALER_SCENARIO_PROFILE_SPANS_V4,
            DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
        },
    },
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use dclutch_execution_strategy_contract::admitted_v3::{
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3, ADMITTED_STRATEGY_EVIDENCE_COUNT_V3,
    ADMITTED_STRATEGY_EVIDENCE_START_V3,
};

// The admitted CPI frame's evidence suffix is owned by
// `dclutch_execution_strategy_contract::admitted_v3`, which derives every slot
// from `ADMITTED_STRATEGY_EVIDENCE_START_V3` and pins the span's length to its
// last named account. The coordinates below are that table read relative to the
// start of the suffix, because `strategy_accounts` is the suffix, not the whole
// frame -- so they are subtracted from the contract's absolute coordinates
// rather than restated as the numbers they currently evaluate to.
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = ADMITTED_STRATEGY_EVIDENCE_COUNT_V3;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize =
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3 - ADMITTED_STRATEGY_EVIDENCE_START_V3;
/// Number of fixed Hot coordinates a Dealer scenario injects ahead of its
/// packed runtime suffix.
pub const DEALER_HOT_INJECTED_ACCOUNTS_V4: usize = 5;

const DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4: [usize; DEALER_HOT_INJECTED_ACCOUNTS_V4] = [
    HOT_ROOT_ACCOUNT_V3,
    HOT_CONFIG_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
];

/// Exact fixed geometry of one Dealer scenario Hot frame.
///
/// This is the supported way to consume the frame's shape. A campaign, a
/// producer, or a durable caller that needs to enumerate the physical frame
/// reads it from here rather than restating coordinates of its own, which is
/// the same doctrine the derived-not-supplied frames elsewhere in the tree
/// already follow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerHotFrameProjectionV4 {
    /// Exact common Hot frame width every Dealer scenario restates.
    pub fixed_account_count: usize,
    /// Number of injected physical accounts ahead of the packed runtime suffix.
    pub injected_account_count: usize,
    /// The exact fixed Hot coordinates those injected accounts occupy, in the
    /// canonical order the account profile packs them.
    pub injected_physical_indices: [usize; DEALER_HOT_INJECTED_ACCOUNTS_V4],
    /// Exact admitted-AOT evidence width between the fixed frame and the
    /// caller authorities.
    pub admitted_evidence_count: usize,
}

/// Borrow the one canonical Dealer scenario Hot frame projection.
#[must_use]
pub const fn dealer_hot_frame_projection_v4() -> DealerHotFrameProjectionV4 {
    DealerHotFrameProjectionV4 {
        fixed_account_count: HOT_FIXED_ACCOUNT_COUNT_V3,
        injected_account_count: DEALER_HOT_INJECTED_ACCOUNTS_V4,
        injected_physical_indices: DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4,
        admitted_evidence_count: ADMITTED_AOT_FIXED_EXTRAS_V3,
    }
}

/// Account-lock ceiling currently active on Solana devnet.
///
/// Address lookup tables compress message addresses but do not change this
/// runtime lock ceiling.
pub const SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1: usize = 64;

/// Exact resolved transaction lock census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioTransactionLockCensusV1 {
    /// Pairwise-distinct payer, instruction-meta, and invoked-program keys.
    pub unique_account_lock_count: usize,
}

/// Stable refusal from the devnet transaction lock gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioLockLimitErrorV1 {
    /// The resolved transaction names more than 64 distinct account locks.
    LockLimit,
}

/// Count exact resolved transaction locks before signing or serialization.
///
/// Pass every instruction in the transaction, including compute-budget or
/// memo instructions. Account metas carry their resolved pubkeys before v0
/// compilation, so moving them into an address lookup table cannot alter this
/// census.
pub fn census_dealer_scenario_transaction_locks_v1(
    payer: Pubkey,
    instructions: &[Instruction],
) -> DealerScenarioTransactionLockCensusV1 {
    let mut unique = vec![payer];
    for instruction in instructions {
        if !unique.contains(&instruction.program_id) {
            unique.push(instruction.program_id);
        }
        for meta in &instruction.accounts {
            if !unique.contains(&meta.pubkey) {
                unique.push(meta.pubkey);
            }
        }
    }
    DealerScenarioTransactionLockCensusV1 {
        unique_account_lock_count: unique.len(),
    }
}

/// Admit one transaction only when its resolved lock census fits devnet.
pub fn require_dealer_scenario_devnet_lock_limit_v1(
    payer: Pubkey,
    instructions: &[Instruction],
) -> Result<DealerScenarioTransactionLockCensusV1, DealerScenarioLockLimitErrorV1> {
    let census = census_dealer_scenario_transaction_locks_v1(payer, instructions);
    if census.unique_account_lock_count <= SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1 {
        Ok(census)
    } else {
        Err(DealerScenarioLockLimitErrorV1::LockLimit)
    }
}

/// Same-finalized physical inputs after common Hot authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioHotMetaStateV4<'a> {
    /// Exact common Hot39 prefix in canonical ABI order.
    pub fixed_accounts: &'a [ObservedAccountMetaV3],
    /// Eight admitted-AOT extras followed by exact caller-authority pages.
    pub strategy_accounts: &'a [ObservedAccountMetaV3],
    /// Packed Profile13 suffix after the five common injected coordinates.
    pub runtime_suffix_accounts: &'a [ObservedAccountMetaV3],
}

/// Semantic inputs authenticated from the same chain observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioSemanticStateV4<'a> {
    /// Current obligation, Claims Positions, and Market joins.
    pub chain: ScenarioTradeChainProjectionV3<'a>,
    /// Current Registry/Realm/Custody composition coordinates.
    pub context: ScenarioComposerContextV3,
    /// Current canonical collateral accounts and balances.
    pub collateral: ScenarioCollateralFrameV3,
}

/// Exact selector-9 semantic and packed-account projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioHotMetaReportV4 {
    /// Same finalized observation shared by every physical account.
    pub observation: Observation,
    /// Canonical scenario-solvent plan re-derived from the request.
    pub semantic_plan: ScenarioAtomicPlanV3,
    /// Six `{0,14}` Custody spans, the Claims `{1,2}` span, and the trailing
    /// Readonly missing-collateral/Dealer evidence `{0..3}` span in canonical
    /// Profile13 table order.
    pub dynamic_span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    /// Exact packed physical AccountProfile account count, including the five
    /// common injected accounts.
    pub runtime_physical_account_count: usize,
    /// Exact admitted-AOT caller-authority page count for `102 + N` scalars and
    /// 117 identities.
    pub caller_authority_count: usize,
    /// Canonical transaction metas in `Hot39 || strategy || packed suffix`
    /// order. No signer or submission is performed.
    pub instruction_accounts: Vec<AccountMeta>,
}

/// Chain-derived selector-9 semantics before physical account validation.
///
/// This is the missing seam between the Dealer semantic owner and a bundle
/// builder.  Span widths and caller-authority pages are outputs here, never
/// fixture inputs, so a caller cannot choose a smaller admitted frame than the
/// transition it asks the accelerator to evaluate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioHotSemanticReportV4 {
    /// Canonical scenario-solvent plan re-derived from the request.
    pub semantic_plan: ScenarioAtomicPlanV3,
    /// Exact nine Profile13 protected-span widths.
    pub dynamic_span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    /// Exact admitted-AOT caller-authority page count.
    pub caller_authority_count: usize,
    /// Candidate scalar register bank the accelerator must return.
    pub candidate_scalars: Vec<u64>,
    /// Candidate identity register bank the accelerator must return.
    pub candidate_identities: Vec<[u8; 32]>,
    /// Exact projected Custody requests in canonical semantic order.
    pub custody_effects: [Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    /// Exact candidate obligation account body.
    pub candidate_obligation_state: Vec<u8>,
}

/// Exact unsplit Trading topology and its lock census.
///
/// This shape is intentionally not an executable capability. Its canonical
/// 121 instruction locks exceed devnet's 64-lock runtime limit, and an address
/// lookup table cannot change that limit. It exists to derive the split
/// checkpoint topology without losing semantic or physical facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioUnsplitTopologyV4 {
    /// Canonical unsplit Hot instruction used only for topology analysis.
    pub instruction: Instruction,
    /// Semantic and physical projection which authorized the instruction.
    pub report: DealerScenarioHotMetaReportV4,
    /// Total account-meta entries in the instruction.
    pub account_meta_count: usize,
    /// Pairwise-distinct instruction locks: metas plus the invoked program.
    ///
    /// A transaction payer is deliberately outside this unsigned-instruction
    /// constructor and therefore outside this count.
    pub unique_account_lock_count: usize,
}

/// Stable refusal from selector-9 semantic/meta projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioHotMetaErrorV4 {
    /// The family request or chain-derived semantic composition refused.
    Semantics,
    /// The canonical Profile13 artifact differed.
    AccountProfile,
    /// Common, strategy, or packed runtime account geometry differed.
    AccountGeometry,
    /// Accounts did not share one finalized observation.
    Observation,
    /// A checked count or width overflowed.
    Arithmetic,
}

/// Re-execute selector 9 before constructing any physical frame.
///
/// The returned registers are the authoritative candidate bank; the returned
/// span counts are projected from those same registers.  This ordering is
/// important for Dealer: six Custody spans are transition-owned, not written
/// by the family request, while the trailing input-bank span is Hot-owned.
pub fn project_dealer_scenario_hot_semantics_v4(
    semantic: DealerScenarioSemanticStateV4<'_>,
    family_request: &[u8],
) -> Result<DealerScenarioHotSemanticReportV4, DealerScenarioHotMetaErrorV4> {
    let request = DealerScenarioTradeRequestV3::decode(family_request)
        .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let width =
        usize::try_from(request.width).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let scalar_count = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        .checked_add(width)
        .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let identity_count = usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4);

    let mut acquired = vec![0_u64; width];
    let mut delivered = vec![0_u64; width];
    let mut obligations_before = vec![0_u64; width];
    let mut obligations_after = vec![0_u64; width];
    let mut candidate_obligation_state = vec![
        0_u8;
        obligation_account_bytes_v3(request.width).map_err(
            |_| DealerScenarioHotMetaErrorV4::Arithmetic
        )?
    ];
    let mut post_inventory = vec![0_u64; width];
    let mut post_counterparty_inventory = vec![0_u64; width];
    let mut post_equity = vec![0_i128; width];
    let mut custody_effects =
        [None::<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3];
    let prepared = prepare_scenario_trade_v3(
        request,
        semantic.chain,
        semantic.context,
        semantic.collateral,
        &mut acquired,
        &mut delivered,
        &mut candidate_obligation_state,
        &mut obligations_before,
        &mut obligations_after,
        &mut post_inventory,
        &mut post_counterparty_inventory,
        &mut post_equity,
        &mut custody_effects,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let mut candidate_scalars = vec![0_u64; scalar_count];
    let mut candidate_identities = vec![[0_u8; 32]; identity_count];
    project_dealer_scenario_hot_registers_v4(
        request,
        &prepared.plan,
        prepared.candidate_obligation,
        &custody_effects,
        semantic.chain.trading_program,
        semantic.chain.now,
        &mut candidate_scalars,
        &mut candidate_identities,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let mut dynamic_span_counts = [0_u32; DEALER_SCENARIO_PROFILE_SPANS_V4];
    // Profile13's span selectors have one semantic owner: the candidate bank
    // emitted above.  The physical profile independently authenticates this
    // same projection once its account bytes are available.
    let mut profile_scratch = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    let mut profile_bytes = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    encode_dealer_scenario_account_profile_v4_atomic(
        DealerScenarioAccountProfileInputV4 {
            common_data_lengths: [
                1,
                u32::try_from(DEALER_CONFIG_BYTES_V4)
                    .map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
                1,
                1,
                1,
            ],
        },
        &mut profile_scratch,
        &mut profile_bytes,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    let profile = AccountProfileV2::decode(&profile_bytes)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    profile
        .dynamic_span_widths_from_scalars(&candidate_scalars, &mut dynamic_span_counts)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if usize::from(profile.dynamic_fixed_span_count()) != DEALER_SCENARIO_PROFILE_SPANS_V4 {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    let caller_authority_count = admitted_caller_authority_count_v3(
        u32::try_from(scalar_count).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
        u32::try_from(identity_count).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::AccountGeometry)?;
    Ok(DealerScenarioHotSemanticReportV4 {
        semantic_plan: prepared.plan,
        dynamic_span_counts,
        caller_authority_count,
        candidate_scalars,
        candidate_identities,
        custody_effects,
        candidate_obligation_state,
    })
}

/// Re-execute selector 9 and derive its complete unsigned account-meta list.
///
/// The returned dynamic spans are derived only from the canonical semantic
/// composer and candidate register projection.  In particular, a caller
/// cannot smuggle a second custody bitmap or Claims Position count into the
/// physical account frame.
pub fn project_dealer_scenario_hot_metas_v4(
    state: DealerScenarioHotMetaStateV4<'_>,
    semantic: DealerScenarioSemanticStateV4<'_>,
    family_request: &[u8],
) -> Result<DealerScenarioHotMetaReportV4, DealerScenarioHotMetaErrorV4> {
    let observation = validate_common_observation(state, semantic)?;
    let request = DealerScenarioTradeRequestV3::decode(family_request)
        .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let projected = project_dealer_scenario_hot_semantics_v4(semantic, family_request)?;

    let profile = authenticate_account_profile(state)?;
    let expected_scalars = usize::from(profile.common_scalar_count())
        .checked_add(
            usize::try_from(request.width).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
        )
        .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?;
    if expected_scalars != projected.candidate_scalars.len()
        || usize::from(profile.common_scalar_count())
            != usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        || usize::from(profile.common_identity_count()) != projected.candidate_identities.len()
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    let mut authenticated_span_counts = [0_u32; DEALER_SCENARIO_PROFILE_SPANS_V4];
    profile
        .dynamic_span_widths_from_scalars(
            &projected.candidate_scalars,
            &mut authenticated_span_counts,
        )
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if authenticated_span_counts != projected.dynamic_span_counts {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    let runtime_physical_account_count = validate_runtime_accounts(
        state,
        profile,
        request.width,
        &projected.dynamic_span_counts,
        observation,
    )?;
    if state.strategy_accounts.len()
        != ADMITTED_AOT_FIXED_EXTRAS_V3
            .checked_add(projected.caller_authority_count)
            .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || !state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .is_some_and(|account| account.account.executable)
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }

    let capacity = state
        .fixed_accounts
        .len()
        .checked_add(state.strategy_accounts.len())
        .and_then(|value| value.checked_add(state.runtime_suffix_accounts.len()))
        .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let mut instruction_accounts = Vec::with_capacity(capacity);
    instruction_accounts.extend(state.fixed_accounts.iter().map(account_meta));
    instruction_accounts.extend(state.strategy_accounts.iter().map(account_meta));
    instruction_accounts.extend(state.runtime_suffix_accounts.iter().map(account_meta));
    Ok(DealerScenarioHotMetaReportV4 {
        observation,
        semantic_plan: projected.semantic_plan,
        dynamic_span_counts: projected.dynamic_span_counts,
        runtime_physical_account_count,
        caller_authority_count: projected.caller_authority_count,
        instruction_accounts,
    })
}

/// Project the exact unsplit Trading topology from one same-finalized view.
///
/// The root prestate digest is derived from the observed root bytes. Release,
/// Market, generation, program identity, account order, and all dynamic widths
/// are taken from authenticated semantic or physical inputs; none are accepted
/// as parallel caller fields. The returned instruction must not be submitted:
/// its lock census is the evidence that a durable split is required.
pub fn project_dealer_scenario_unsplit_topology_v4(
    state: DealerScenarioHotMetaStateV4<'_>,
    semantic: DealerScenarioSemanticStateV4<'_>,
    family_request: &[u8],
) -> Result<DealerScenarioUnsplitTopologyV4, DealerScenarioHotMetaErrorV4> {
    let report = project_dealer_scenario_hot_metas_v4(state, semantic, family_request)?;
    let root = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let trading = fixed(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let market = fixed(state, HOT_MARKET_ACCOUNT_V3)?;
    if trading.account.key.to_bytes() != semantic.chain.trading_program
        || market.account.key.to_bytes() != semantic.chain.market
        || root.account.key.to_bytes() != semantic.chain.child_root
    {
        return Err(DealerScenarioHotMetaErrorV4::Observation);
    }
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len())
            .map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
        semantic.chain.release_set,
        semantic.chain.market,
        semantic.chain.generation,
        hash(&root.account.data).to_bytes(),
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let mut data = Vec::with_capacity(
        envelope
            .to_bytes()
            .len()
            .checked_add(family_request.len())
            .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(family_request);
    let account_meta_count = report.instruction_accounts.len();
    let mut unique = Vec::with_capacity(account_meta_count);
    for meta in &report.instruction_accounts {
        if !unique.contains(&meta.pubkey) {
            unique.push(meta.pubkey);
        }
    }
    // The Trading program is already a fixed Hot-frame meta today.  Count it
    // only when a future profile omits it; Solana message compilation de-dupes
    // the invoked program against instruction metas.
    if !unique.contains(&trading.account.key) {
        unique.push(trading.account.key);
    }
    let unique_account_lock_count = unique.len();
    Ok(DealerScenarioUnsplitTopologyV4 {
        instruction: Instruction {
            program_id: trading.account.key,
            accounts: report.instruction_accounts.clone(),
            data,
        },
        report,
        account_meta_count,
        unique_account_lock_count,
    })
}

fn authenticate_account_profile<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
) -> Result<AccountProfileV2<'a>, DealerScenarioHotMetaErrorV4> {
    let profile_bytes = fixed(state, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?
        .account
        .data
        .as_slice();
    let mut common_data_lengths = [0_u32; DEALER_HOT_INJECTED_ACCOUNTS_V4];
    for (destination, index) in common_data_lengths
        .iter_mut()
        .zip(DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4)
    {
        *destination = u32::try_from(fixed(state, index)?.account.data.len())
            .map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?;
    }
    let mut scratch = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    let mut expected = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    encode_dealer_scenario_account_profile_v4_atomic(
        DealerScenarioAccountProfileInputV4 {
            common_data_lengths,
        },
        &mut scratch,
        &mut expected,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if profile_bytes != expected {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    AccountProfileV2::decode(profile_bytes)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)
}

fn validate_common_observation(
    state: DealerScenarioHotMetaStateV4<'_>,
    semantic: DealerScenarioSemanticStateV4<'_>,
) -> Result<Observation, DealerScenarioHotMetaErrorV4> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }
    let observation = fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.observation;
    if observation.finality != Finality::Finalized
        || semantic.chain.now != observation.slot
        || fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.key.to_bytes() != semantic.chain.market
        || fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.key.to_bytes() != semantic.chain.child_root
        || fixed(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?
            .account
            .key
            .to_bytes()
            != semantic.chain.trading_program
    {
        return Err(DealerScenarioHotMetaErrorV4::Observation);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.account.observation != observation
            || account.is_signer
            || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
        }
    }
    for account in state
        .strategy_accounts
        .iter()
        .chain(state.runtime_suffix_accounts)
    {
        if account.account.observation != observation
            || account.account.observation.finality != Finality::Finalized
        {
            return Err(DealerScenarioHotMetaErrorV4::Observation);
        }
    }
    Ok(observation)
}

fn validate_runtime_accounts(
    state: DealerScenarioHotMetaStateV4<'_>,
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    observation: Observation,
) -> Result<usize, DealerScenarioHotMetaErrorV4> {
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(tail_count, span_counts)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if physical_count < DEALER_HOT_INJECTED_ACCOUNTS_V4
        || state.runtime_suffix_accounts.len()
            != physical_count
                .checked_sub(DEALER_HOT_INJECTED_ACCOUNTS_V4)
                .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }
    for physical_ordinal in 0..physical_count {
        let account = runtime_account(state, physical_ordinal)?;
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(tail_count, span_counts, physical_ordinal)
            .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
        let privileges = geometry.privileges();
        if account.account.observation != observation
            || account.is_signer != privileges.signer()
            || account.is_writable != privileges.writable()
            || account.account.executable != privileges.executable()
            || !data_geometry_matches(geometry.data(), account.account.data.len())
        {
            return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
        }
    }
    Ok(physical_count)
}

fn data_geometry_matches(geometry: PhysicalAccountDataGeometryV2, actual: usize) -> bool {
    match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => actual == bytes,
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
            actual == 0 || actual == live_bytes
        }
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            actual >= minimum_bytes
        }
        PhysicalAccountDataGeometryV2::Opaque => true,
    }
}

fn runtime_account<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
    physical_ordinal: usize,
) -> Result<&'a ObservedAccountMetaV3, DealerScenarioHotMetaErrorV4> {
    if physical_ordinal < DEALER_HOT_INJECTED_ACCOUNTS_V4 {
        let index = *DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4
            .get(physical_ordinal)
            .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)?;
        fixed(state, index)
    } else {
        state
            .runtime_suffix_accounts
            .get(
                physical_ordinal
                    .checked_sub(DEALER_HOT_INJECTED_ACCOUNTS_V4)
                    .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?,
            )
            .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)
    }
}

fn fixed<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
    index: usize,
) -> Result<&'a ObservedAccountMetaV3, DealerScenarioHotMetaErrorV4> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)
}

fn account_meta(value: &ObservedAccountMetaV3) -> AccountMeta {
    AccountMeta {
        pubkey: value.account.key,
        is_signer: value.is_signer,
        is_writable: value.is_writable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ObservedAccount,
        dealer_scenario_checkpoint_v1::{
            DealerScenarioFinalCommitFixedAccountsV1,
            project_dealer_scenario_final_commit_topology_v1,
        },
    };
    use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
    use dclutch_custody_contract::{
        CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CompartmentV1, CustodyVaultSeedsV1,
    };
    use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
    use dclutch_trading_sbf::dealer::v3_trade_profile::dealer_scenario_logical_frame_v4;
    use dclutch_trading_sbf::dealer::v3_trade_profile::{
        DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
    };
    use dclutch_trading_sbf::dealer::{
        v3_obligation::{
            DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
            DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
            DealerObligationProjectionV3,
        },
        v3_trade::{
            DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
            ScenarioTradeDirectionV3, ScenarioTradeIntentV3, build_scenario_trade_request_v3,
            scenario_trade_max_request_bytes_v3,
        },
    };
    use solana_program::pubkey::Pubkey;

    fn observation() -> Observation {
        Observation {
            slot: 20,
            unix_timestamp: 12,
            finality: Finality::Finalized,
        }
    }

    fn meta(
        index: usize,
        bytes: usize,
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: Pubkey::new_from_array([u8::try_from(index + 1).expect("small index"); 32]),
                owner: Pubkey::new_from_array([200; 32]),
                lamports: 1,
                executable,
                data: vec![0; bytes],
            },
            is_signer: signer,
            is_writable: writable,
        }
    }

    fn canonical_profile(common_data_lengths: [u32; 5]) -> Vec<u8> {
        let mut scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        encode_dealer_scenario_account_profile_v4_atomic(
            DealerScenarioAccountProfileInputV4 {
                common_data_lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    fn runtime_fixture(
        tail_count: u32,
        span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    ) -> (Vec<ObservedAccountMetaV3>, Vec<ObservedAccountMetaV3>) {
        let common_lengths = [32_u32, 128, 48, 56, 64];
        let profile_bytes = canonical_profile(common_lengths);
        let profile = AccountProfileV2::decode(&profile_bytes).expect("decode");
        let physical_count = profile
            .physical_account_count_with_dynamic_spans(tail_count, &span_counts)
            .expect("count");
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| meta(index, 0, false, index == HOT_ROOT_ACCOUNT_V3, false))
            .collect::<Vec<_>>();
        fixed_accounts
            .get_mut(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
            .expect("profile account")
            .account
            .data = profile_bytes.clone();
        for (length, index) in common_lengths
            .into_iter()
            .zip(DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4)
        {
            fixed_accounts
                .get_mut(index)
                .expect("injected account")
                .account
                .data = vec![0; usize::try_from(length).expect("length")];
        }
        let mut suffix = Vec::new();
        for ordinal in DEALER_HOT_INJECTED_ACCOUNTS_V4..physical_count {
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(tail_count, &span_counts, ordinal)
                .expect("geometry");
            let privileges = geometry.privileges();
            let bytes = match geometry.data() {
                PhysicalAccountDataGeometryV2::Exact { bytes }
                | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
                PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                    minimum_bytes
                }
                PhysicalAccountDataGeometryV2::Opaque => 7,
            };
            suffix.push(meta(
                HOT_FIXED_ACCOUNT_COUNT_V3 + ordinal,
                bytes,
                privileges.signer(),
                privileges.writable(),
                privileges.executable(),
            ));
        }
        (fixed_accounts, suffix)
    }

    #[test]
    fn devnet_lock_gate_admits_exactly_64_and_refuses_65() {
        let payer = Pubkey::new_from_array([250; 32]);
        let program_id = Pubkey::new_from_array([249; 32]);
        let instruction = |meta_count: u8| Instruction {
            program_id,
            accounts: (0..meta_count)
                .map(|index| {
                    AccountMeta::new_readonly(Pubkey::new_from_array([index + 1; 32]), false)
                })
                .collect(),
            data: Vec::new(),
        };
        let admitted = instruction(62);
        assert_eq!(
            census_dealer_scenario_transaction_locks_v1(payer, core::slice::from_ref(&admitted)),
            DealerScenarioTransactionLockCensusV1 {
                unique_account_lock_count: 64,
            }
        );
        assert_eq!(
            require_dealer_scenario_devnet_lock_limit_v1(payer, core::slice::from_ref(&admitted),),
            Ok(DealerScenarioTransactionLockCensusV1 {
                unique_account_lock_count: 64,
            })
        );

        let refused = instruction(63);
        assert_eq!(
            census_dealer_scenario_transaction_locks_v1(payer, core::slice::from_ref(&refused)),
            DealerScenarioTransactionLockCensusV1 {
                unique_account_lock_count: 65,
            }
        );
        assert_eq!(
            require_dealer_scenario_devnet_lock_limit_v1(payer, core::slice::from_ref(&refused),),
            Err(DealerScenarioLockLimitErrorV1::LockLimit)
        );
    }

    #[test]
    fn profile13_packs_sparse_and_dense_selector_nine_frames() {
        for spans in [
            [0, 0, 0, 0, 1, 0, 0, 3, 6],
            [14, 14, 14, 14, 2, 14, 14, 0, 6],
        ] {
            let (fixed_accounts, suffix) = runtime_fixture(4, spans);
            let state = DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &[],
                runtime_suffix_accounts: &suffix,
            };
            let profile = authenticate_account_profile(state).expect("canonical profile");
            let observed = validate_runtime_accounts(state, profile, 4, &spans, observation())
                .expect("packed geometry");
            assert_eq!(observed, suffix.len() + DEALER_HOT_INJECTED_ACCOUNTS_V4);
        }
    }

    #[test]
    fn final_commit_topology_reports_dense_selector_nine_lock_wall() {
        let spans = [14, 14, 14, 14, 2, 14, 14, 0, 6];
        let (fixed_accounts, suffix) = runtime_fixture(4, spans);
        let topology = project_dealer_scenario_final_commit_topology_v1(
            DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &[],
                runtime_suffix_accounts: &suffix,
            },
            4,
            spans,
            DealerScenarioFinalCommitFixedAccountsV1 {
                payer: Pubkey::new_from_array([255; 32]),
                trading_program: Pubkey::new_from_array([254; 32]),
                checkpoint: Pubkey::new_from_array([253; 32]),
                clock: Pubkey::new_from_array([252; 32]),
                request: Pubkey::new_from_array([251; 32]),
                evaluation_receipt: Pubkey::new_from_array([250; 32]),
                candidate_bank: Pubkey::new_from_array([249; 32]),
                candidate_obligation: Pubkey::new_from_array([248; 32]),
                claims_delta: Pubkey::new_from_array([247; 32]),
                effects: Pubkey::new_from_array([246; 32]),
            },
        )
        .expect("topology");
        assert_eq!(topology.effect_accounts.len(), 117);
        assert_eq!(topology.unique_account_lock_count, 119);
        assert!(!topology.fits_devnet_lock_limit);
    }

    #[test]
    fn packed_frame_refuses_privilege_and_exact_data_substitution() {
        let spans = [0, 0, 0, 0, 1, 0, 0, 3, 6];
        let (fixed_accounts, mut suffix) = runtime_fixture(4, spans);
        let profile_bytes = canonical_profile([32, 128, 48, 56, 64]);
        let profile = AccountProfileV2::decode(&profile_bytes).expect("canonical profile");
        let state = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert!(authenticate_account_profile(state).is_ok());
        assert!(validate_runtime_accounts(state, profile, 4, &spans, observation()).is_ok());

        let logical = dealer_scenario_logical_frame_v4(spans).expect("logical frame");
        let obligation_ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(
                4,
                &spans,
                usize::try_from(logical.obligation).expect("obligation coordinate"),
            )
            .expect("obligation ordinal");
        let obligation_suffix = obligation_ordinal
            .checked_sub(DEALER_HOT_INJECTED_ACCOUNTS_V4)
            .expect("obligation after injected prefix");
        suffix
            .get_mut(obligation_suffix)
            .expect("obligation")
            .is_writable = false;
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            validate_runtime_accounts(substituted, profile, 4, &spans, observation()),
            Err(DealerScenarioHotMetaErrorV4::AccountGeometry)
        );
        let obligation = suffix.get_mut(obligation_suffix).expect("obligation");
        obligation.is_writable = true;
        obligation.account.data.pop();
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            validate_runtime_accounts(substituted, profile, 4, &spans, observation()),
            Err(DealerScenarioHotMetaErrorV4::AccountGeometry)
        );
    }

    #[test]
    fn profile_bytes_are_bound_to_observed_common_widths() {
        let spans = [0, 0, 0, 0, 1, 0, 0, 1, 6];
        let (mut fixed_accounts, suffix) = runtime_fixture(4, spans);
        let state = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert!(authenticate_account_profile(state).is_ok());
        fixed_accounts
            .get_mut(HOT_CONFIG_RAW_ACCOUNT_V3)
            .expect("config")
            .account
            .data
            .push(0);
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            authenticate_account_profile(substituted),
            Err(DealerScenarioHotMetaErrorV4::AccountProfile)
        );
    }

    fn obligation_bytes(
        market: [u8; 32],
        product: [u8; 32],
        basis: [u8; 32],
        owner: [u8; 32],
        child: [u8; 32],
        revision: u64,
        values: &[u64],
    ) -> Vec<u8> {
        let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(
            &u32::try_from(values.len())
                .expect("small obligation width")
                .to_le_bytes(),
        );
        bytes[16..24].copy_from_slice(&revision.to_le_bytes());
        for (offset, value) in [
            (24, market),
            (56, product),
            (88, basis),
            (120, owner),
            (152, child),
        ] {
            bytes[offset..offset + 32].copy_from_slice(&value);
        }
        for (index, value) in values.iter().enumerate() {
            let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn program_set() -> Vec<u8> {
        let mut bytes = vec![0; 72];
        bytes[..8].copy_from_slice(b"DCLTCPS1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3.to_le_bytes());
        bytes[16] = 2;
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
        bytes[32..36].copy_from_slice(&u32::from(DEALER_SCENARIO_TRADE_ACTION_V3).to_le_bytes());
        bytes[36..68].copy_from_slice(&[42; 32]);
        bytes
    }

    #[test]
    fn unsplit_topology_derives_spans_and_proves_devnet_refusal() {
        let trading = [1; 32];
        let custody = [12; 32];
        let release = [6; 32];
        let market = [2; 32];
        let product = [3; 32];
        let basis = [4; 32];
        let dealer = [5; 32];
        let child = [7; 32];
        let counterparty = [8; 32];
        let counterparty_account = [9; 32];
        let realm = [13; 32];
        let mint = [14; 32];
        let token_program = [15; 32];
        let obligation_state =
            obligation_bytes(market, product, basis, dealer, child, 7, &[12, 20, 10]);
        let current_obligation =
            DealerObligationProjectionV3::decode(&obligation_state).expect("obligation");
        let obligation = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child],
            &Pubkey::new_from_array(trading),
        )
        .0
        .to_bytes();
        let dealer_inventory = [2, 10, 0];
        let counterparty_inventory = [20, 5, 9];
        let chain = ScenarioTradeChainProjectionV3 {
            trading_program: trading,
            release_set: release,
            market,
            child_root: child,
            obligation_address: obligation,
            current_obligation,
            dealer_position: ClaimsInventoryObservation {
                market_id: market,
                product_id: product,
                liability_basis_id: basis,
                position_owner: dealer,
                revision: 9,
                inventory: &dealer_inventory,
            },
            counterparty_position: ClaimsInventoryObservation {
                market_id: market,
                product_id: product,
                liability_basis_id: basis,
                position_owner: counterparty,
                revision: 11,
                inventory: &counterparty_inventory,
            },
            product_record_digest: [10; 32],
            linked_basis_record_digest: [11; 32],
            counterparty_account,
            principal_balance: 100,
            locked_capital_floor: 0,
            claims_revision: 8,
            generation: 17,
            now: 20,
            expires_at: 25,
            terminal: false,
            basis_scale: 1,
        };
        let intent = ScenarioTradeIntentV3 {
            direction: ScenarioTradeDirectionV3::CounterpartyPaysDealer,
            principal: 10,
            realized_fee: 1,
            acquired: &[3, 0, 4],
            delivered: &[0, 1, 0],
            candidate_obligations: &[10, 19, 13],
        };
        let set_bytes = program_set();
        let set = CapabilityProgramSetV1::decode(&set_bytes).expect("program set");
        let mut request = vec![0; scenario_trade_max_request_bytes_v3(3).expect("request bound")];
        let built_request =
            build_scenario_trade_request_v3(chain, intent, set, &mut request).expect("request");
        request.truncate(built_request.request_bytes);
        let vault = |context, compartment| {
            Pubkey::find_program_address(
                &CustodyVaultSeedsV1::new(market, release, context, compartment).as_slices(),
                &Pubkey::new_from_array(custody),
            )
            .0
            .to_bytes()
        };
        let semantic = DealerScenarioSemanticStateV4 {
            chain,
            context: ScenarioComposerContextV3 {
                trading_program: trading,
                custody_program: custody,
                release_set: release,
                market,
                realm,
                child_root: child,
                obligation_account: obligation,
                mint,
                token_program,
                parent_request_digest: hash(&request).to_bytes(),
                generation: 17,
                custody_replay_revision: 7,
                locked_capital_floor: 0,
                basis_scale: 1,
            },
            collateral: ScenarioCollateralFrameV3 {
                principal_vault: vault(child, CompartmentV1::TradingPrincipal),
                principal_balance: 100,
                fee_vault: vault(child, CompartmentV1::FeeVault),
                fee_balance: 9,
                hoard_vault: vault(market, CompartmentV1::HoardPrincipal),
                hoard_balance: 100,
                counterparty_account,
                counterparty_owner: counterparty,
                counterparty_external_delegate: Pubkey::find_program_address(
                    &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market, &release],
                    &Pubkey::new_from_array(custody),
                )
                .0
                .to_bytes(),
                counterparty_external_delegated_amount: 11,
                counterparty_balance: 100,
            },
        };
        let projection = project_dealer_scenario_hot_semantics_v4(semantic, &request)
            .expect("semantic projection");
        assert_eq!(projection.dynamic_span_counts[4], 2);
        assert_eq!(projection.dynamic_span_counts[8], 6);
        assert_eq!(projection.caller_authority_count, 6);
        assert_eq!(
            projection.candidate_scalars.len(),
            usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4) + 3
        );
        // 118 since `322de4b2` moved `DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4`
        // from 117 for selector 9's obligation guard. NOT a General number and
        // not this lane's: it is moved here only because the literal is a
        // restatement of a constant with one owner in
        // `programs/dclutch-trading-sbf/src/dealer/v3_trade_artifacts.rs`, and
        // it had been red in the shared operator suite since 2026-09-01 19:32.
        assert_eq!(projection.candidate_identities.len(), 118);

        let spans = projection.dynamic_span_counts;
        let (mut fixed_accounts, suffix) = runtime_fixture(3, spans);
        fixed_accounts[HOT_MARKET_ACCOUNT_V3].account.key = Pubkey::new_from_array(market);
        fixed_accounts[HOT_ROOT_ACCOUNT_V3].account.key = Pubkey::new_from_array(child);
        fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3].account.key =
            Pubkey::new_from_array(trading);
        let mut strategy_accounts = (0..8 + projection.caller_authority_count)
            .map(|index| meta(200 + index, 0, false, false, index == 6))
            .collect::<Vec<_>>();
        strategy_accounts[6].account.executable = true;
        let built = project_dealer_scenario_unsplit_topology_v4(
            DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &strategy_accounts,
                runtime_suffix_accounts: &suffix,
            },
            semantic,
            &request,
        )
        .expect("unsplit topology");
        assert_eq!(built.report.dynamic_span_counts, spans);
        assert_eq!(built.report.caller_authority_count, 6);
        assert_eq!(built.account_meta_count, 122);
        assert_eq!(built.instruction.accounts.len(), 122);
        assert_eq!(built.unique_account_lock_count, 121);
        let final_topology =
            crate::dealer_scenario_checkpoint_v1::project_dealer_scenario_final_commit_topology_v1(
                DealerScenarioHotMetaStateV4 {
                    fixed_accounts: &fixed_accounts,
                    strategy_accounts: &strategy_accounts,
                    runtime_suffix_accounts: &suffix,
                },
                3,
                spans,
                crate::dealer_scenario_checkpoint_v1::DealerScenarioFinalCommitFixedAccountsV1 {
                    payer: Pubkey::new_from_array([250; 32]),
                    trading_program: Pubkey::new_from_array(trading),
                    checkpoint: Pubkey::new_from_array([249; 32]),
                    clock: Pubkey::new_from_array([248; 32]),
                    request: Pubkey::new_from_array([247; 32]),
                    evaluation_receipt: Pubkey::new_from_array([246; 32]),
                    candidate_bank: Pubkey::new_from_array([245; 32]),
                    candidate_obligation: Pubkey::new_from_array([244; 32]),
                    claims_delta: Pubkey::new_from_array([243; 32]),
                    effects: Pubkey::new_from_array([242; 32]),
                },
            )
            .expect("final topology");
        assert_eq!(final_topology.effect_accounts.len(), 75);
        assert_eq!(final_topology.unique_account_lock_count, 77);
        assert!(!final_topology.fits_devnet_lock_limit);
        let checkpoint = Pubkey::new_from_array([249; 32]);
        let request_digest = hash(&request).to_bytes();
        let canonical = crate::dealer_scenario_checkpoint_v1::project_dealer_scenario_canonical_membership_pages_v1(
            DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &strategy_accounts,
                runtime_suffix_accounts: &suffix,
            },
            Pubkey::new_from_array([241; 32]),
            checkpoint,
            request_digest,
        ).expect("canonical pages");
        let flattened = canonical
            .pages
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(canonical.manifest.total_account_count, 121);
        assert_eq!(
            canonical.manifest.page_account_counts,
            [21, 20, 20, 20, 20, 20]
        );
        assert_eq!(
            usize::from(canonical.manifest.total_account_count),
            flattened.len()
        );
        assert!(
            canonical
                .pages
                .iter()
                .all(|page| !page.is_empty() && page.len() <= 48)
        );
        assert!(
            flattened
                .windows(2)
                .all(|pair| pair[0].to_bytes() < pair[1].to_bytes())
        );
        let reservation_receipts =
            [230_u8, 231, 232, 233].map(|byte| Pubkey::new_from_array([byte; 32]));
        let reservation_states =
            [234_u8, 235, 236, 237].map(|byte| Pubkey::new_from_array([byte; 32]));
        let reserved = crate::dealer_scenario_checkpoint_v1::project_dealer_scenario_reserved_final_topology_v1(
            DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &strategy_accounts,
                runtime_suffix_accounts: &suffix,
            },
            3,
            spans,
            crate::dealer_scenario_checkpoint_v1::DealerScenarioReservedFinalAccountsV1 {
                fixed: crate::dealer_scenario_checkpoint_v1::DealerScenarioFinalCommitFixedAccountsV1 {
                    payer: Pubkey::new_from_array([250; 32]),
                    trading_program: Pubkey::new_from_array(trading),
                    checkpoint,
                    clock: Pubkey::new_from_array([248; 32]),
                    request: Pubkey::new_from_array([247; 32]),
                    evaluation_receipt: Pubkey::new_from_array([246; 32]),
                    candidate_bank: Pubkey::new_from_array([245; 32]),
                    candidate_obligation: Pubkey::new_from_array([244; 32]),
                    claims_delta: Pubkey::new_from_array([243; 32]),
                    effects: Pubkey::new_from_array([242; 32]),
                },
                custody_program: Pubkey::new_from_array(custody),
                reservation_receipts,
                reservation_states,
                effect_count: projection.semantic_plan.custody_count,
            },
        ).expect("reserved final topology");
        assert_eq!(projection.semantic_plan.custody_count, 3);
        assert_eq!(reserved.effect_accounts.len(), 39);
        assert_eq!(reserved.unique_account_lock_count, 41);
        assert!(reserved.fits_devnet_lock_limit);
        let transaction_census = census_dealer_scenario_transaction_locks_v1(
            Pubkey::new_from_array([250; 32]),
            core::slice::from_ref(&built.instruction),
        );
        assert_eq!(transaction_census.unique_account_lock_count, 122);
        assert_eq!(
            require_dealer_scenario_devnet_lock_limit_v1(
                Pubkey::new_from_array([250; 32]),
                core::slice::from_ref(&built.instruction),
            ),
            Err(DealerScenarioLockLimitErrorV1::LockLimit)
        );
        let (envelope, family) = HotExecutionEnvelopeV3::split_instruction(&built.instruction.data)
            .expect("Hot instruction");
        assert_eq!(family, request);
        assert_eq!(envelope.release_set(), release);
        assert_eq!(envelope.market(), market);
        assert_eq!(envelope.generation(), 17);

        strategy_accounts.pop();
        assert_eq!(
            project_dealer_scenario_unsplit_topology_v4(
                DealerScenarioHotMetaStateV4 {
                    fixed_accounts: &fixed_accounts,
                    strategy_accounts: &strategy_accounts,
                    runtime_suffix_accounts: &suffix,
                },
                semantic,
                &request,
            ),
            Err(DealerScenarioHotMetaErrorV4::AccountGeometry)
        );
    }
}
