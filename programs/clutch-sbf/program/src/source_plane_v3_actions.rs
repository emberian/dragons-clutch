//! SourcePlane V3 mutation handlers entered only by exact capability tuples.
//!
//! The central SourceSeries 77/v2 coordinates are allocated; action 1 release
//! registration and actions 2/3 SourceHead/OpenRawPage creation are live in
//! full profiles. This module owns their complete inner execution: semantic outputs are
//! checked before instruction success, predictable accounts are prefund-safe,
//! mutable postimages advance their durable lineage in the same rollback
//! domain, immutable accounts retain an explicit payer/donation rent partition,
//! and every paid transition emits the exact Source receipt plus liveness
//! intent. The dispatcher cannot enter any remaining handler until its
//! separate capability tuple is admitted.

use std::vec;
use std::vec::Vec;

use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, SourcePlaneProgramV3, StatisticKeyV3,
    StatisticResultV3, SummaryProgramV3, WindowSpecV3, WindowWorkV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
pub use clutch_source_plane_v3_runtime::SourcePolicyHandoffJoinV1;
use clutch_source_plane_v3_runtime::{
    account_data_id, advance_lineage_state, authenticate_source_work_receipt_account,
    authorize_reopen, close_lineage_generation, decode_runtime_account, encode_runtime_account,
    initialize_source_head, open_lineage_generation, plan_runtime_account_close_from_header,
    plan_source_account_creation, AccountCloseFundingV1, AccountCreationFundingV1,
    AuthenticatedBoundaryV1, AuthenticatedClockBucketV1, AuthenticatedEvaluationV1,
    AuthenticatedOpenRawPageV1, AuthenticatedRawPageV1, AuthenticatedReceiverRouteV2,
    AuthenticatedReopenLineageV1, AuthenticatedSourceGenerationV1, AuthenticatedSourceHeadV1,
    AuthenticatedSourceRouteV1, AuthenticatedSourceWorkReceiptV1,
    AuthenticatedStatisticResultAbsenceV1, AuthenticatedStatisticResultAccountV1,
    AuthenticatedWindowEvidenceV1, AuthenticatedWindowWorkV1, BoundaryBatchV1, ClockPolicyV1,
    ClockSnapshotV1, EvaluationReleaseBindingV1, FailurePolicySourceHandoffV1, IngestBatchOutputV1,
    LineageFamilyV1, RentExemptionQuoteV1, ReopenLineageV1, RuntimeAccountBodyV1,
    RuntimeAccountHeaderV1, RuntimeAccountViewV1, RuntimeKey, SealBatchModeV1,
    SourceReleaseManifestV2, SourceTerminalAuthorizationV1, SourceTerminalOutcomeV1,
    SourceWorkAuthorizationV1, SourceWorkKindV1, SourceWorkReceiptAccessV1,
    SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1, SuccessfulEvaluationHandoffV1,
    REOPEN_LINEAGE_BYTES, RUNTIME_ACCOUNT_HEADER_BYTES,
};
use solana_account_info::AccountInfo;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program, RentParameters,
    SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use crate::source_plane_v3::{
    derive_runtime_pda, invoke_parser_boundary, invoke_statistic_evaluator,
    project_liveness_receipt, project_liveness_terminal_intent, project_liveness_work_intent,
    runtime_key, SourceV3SbfError,
};
use clutch_solana_layout::artifact::ArtifactKind;

/// Complete open-account postimage committed with one lineage postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRuntimeAccountResultV1 {
    /// Exact prefund-safe payer/donation partition.
    pub funding: AccountCreationFundingV1,
    /// Permanent lineage-account rent funding when this action bootstrapped
    /// the lineage; absent for an already-authenticated reopen.
    pub lineage_funding: Option<ImmutableAccountFundingV1>,
    /// Globally tagged account header written onchain.
    pub header: RuntimeAccountHeaderV1,
    /// Digest of the complete account postimage.
    pub account_data_id: ContentId,
    /// Durable lineage postimage written atomically.
    pub lineage_after: ReopenLineageV1,
}

/// Complete in-place state/lineage compare-and-swap result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutateRuntimeAccountResultV1 {
    /// Digest of the complete account preimage.
    pub account_data_before_id: ContentId,
    /// Digest of the complete account postimage.
    pub account_data_after_id: ContentId,
    /// Durable lineage postimage written atomically.
    pub lineage_after: ReopenLineageV1,
}

/// Exact action-4 semantic result plus the committed account CAS receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedIngestBoundaryBatchV1 {
    /// Pure Source V3 ingest result.
    pub semantic: IngestBatchOutputV1,
    /// Exact OpenRawPage preimage/postimage and lineage mutation receipt.
    pub mutation: MutateRuntimeAccountResultV1,
}

/// Complete close split and lineage tombstone postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseRuntimeAccountResultV1 {
    /// Exact payer-principal versus neutral-surplus split.
    pub funding: AccountCloseFundingV1,
    /// Durable closed lineage postimage.
    pub lineage_after: ReopenLineageV1,
}

/// Explicit rent ownership for a permanent immutable Source evidence account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableAccountFundingV1 {
    /// Physical immutable account.
    pub account: RuntimeKey,
    /// Payer of the exact rent shortfall, or zero for a fully prefunded PDA.
    pub payer: RuntimeKey,
    /// Exact shortfall supplied by `payer`.
    pub payer_debit_lamports: u64,
    /// Pre-existing lamports classified as neutral donation.
    pub donation_lamports: u64,
    /// Runtime Rent-sysvar identity.
    pub rent_sysvar_id: ContentId,
    /// Exact rent-exempt floor for the allocated bytes.
    pub rent_exempt_minimum_lamports: u64,
    /// Post-create balance.
    pub account_balance_after: u64,
}

/// One persisted paid-work receipt and its sole liveness transition intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceWorkExecutionV1 {
    /// Canonical immutable 0x92 receipt body.
    pub receipt: SourceWorkReceiptAccountV1,
    /// Exact prefund/rent partition for the immutable receipt account.
    pub receipt_funding: ImmutableAccountFundingV1,
    /// Exact authenticated receipt observation consumed by liveness.
    pub observation: RuntimeReceiptObservationV1,
    /// Sole Source-compartment spend intent; it performs no second debit here.
    pub intent: RuntimeTransitionIntentV1,
}

/// One persisted terminal receipt and the sole liveness close intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTerminalExecutionV1 {
    /// Canonical immutable 0x92 terminal receipt body.
    pub receipt: SourceWorkReceiptAccountV1,
    /// Exact prefund/rent partition for the immutable receipt account.
    pub receipt_funding: ImmutableAccountFundingV1,
    /// Exact authenticated terminal observation consumed by liveness.
    pub observation: RuntimeReceiptObservationV1,
    /// Sole Source-compartment terminal intent.
    pub intent: RuntimeTransitionIntentV1,
}

/// Apply the Source work receipt's sole liveness debit in the same SBF
/// instruction as its parser/evaluator CPI and Source state mutation.
/// Any later refusal rolls every preceding CPI-visible write back.
#[allow(clippy::too_many_arguments)]
pub fn apply_source_work_liveness(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    execution: SourceWorkExecutionV1,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
) -> Outcome<RuntimeAtomicTransitionV1> {
    require(
        policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && compartment_account.owner == program_id
            && compartment_account.is_writable
            && !compartment_account.is_signer
            && !compartment_account.executable
            && keeper.is_writable
            && payer_refund.is_writable
            && !keeper.executable
            && !payer_refund.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        runtime_key(compartment_account.key) == route.source_compartment_account()
            && policy_account.key != compartment_account.key
            && policy_account.key != keeper.key
            && policy_account.key != payer_refund.key
            && compartment_account.key != keeper.key
            && compartment_account.key != payer_refund.key,
        ClutchError::AccountAlias,
    )?;
    expect_pda(
        policy_account.key,
        seeds::source_liveness_policy_pda(program_id, &route.liveness_policy_id().bytes()),
        None,
    )?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let compartment_data = compartment_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let balance_after = compartment_account
        .lamports()
        .checked_sub(execution.intent.call_ceiling_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transition = plan_runtime_transition_v1(
        LivenessId::from_bytes(program_id.to_bytes()),
        LivenessId::from_bytes(policy_account.key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(policy_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
            lamports: policy_account.lamports(),
            data: &policy_data,
            writable: policy_account.is_writable,
        },
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(compartment_account.key.to_bytes()),
            owner_program_id: LivenessId::from_bytes(compartment_account.owner.to_bytes()),
            lamports: compartment_account.lamports(),
            data: &compartment_data,
            writable: compartment_account.is_writable,
        },
        execution.intent,
        Some(execution.observation),
        balance_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(compartment_data);
    drop(policy_data);
    require(
        transition.write_account_data
            && !transition.close_account
            && transition.account_balance_before == compartment_account.lamports()
            && transition.account_balance_after == balance_after,
        ClutchError::MismatchedState,
    )?;
    let mut keeper_lamports = 0_u64;
    let mut payer_lamports = 0_u64;
    for movement in transition.transfers() {
        match movement.role {
            RuntimeTransferRoleV1::KeeperPayment => {
                require(
                    movement.destination == LivenessId::from_bytes(keeper.key.to_bytes())
                        && keeper_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                keeper_lamports = movement.lamports;
            }
            RuntimeTransferRoleV1::PayerWorkRefund => {
                require(
                    movement.destination == LivenessId::from_bytes(payer_refund.key.to_bytes())
                        && payer_lamports == 0,
                    ClutchError::MismatchedState,
                )?;
                payer_lamports = movement.lamports;
            }
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
    }
    require(
        keeper_lamports == execution.intent.keeper_payment_lamports
            && payer_lamports
                == execution
                    .intent
                    .call_ceiling_lamports
                    .checked_sub(execution.intent.keeper_payment_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let coalesced_recipient = keeper.key == payer_refund.key;
    let keeper_after = keeper
        .lamports()
        .checked_add(keeper_lamports)
        .and_then(|balance| {
            if coalesced_recipient {
                balance.checked_add(payer_lamports)
            } else {
                Some(balance)
            }
        })
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_after = if coalesced_recipient {
        keeper_after
    } else {
        payer_refund
            .lamports()
            .checked_add(payer_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
    };
    {
        let mut data = compartment_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.len() == transition.post_account_data.len(),
            ClutchError::WrongDataLength,
        )?;
        data.copy_from_slice(&transition.post_account_data);
    }
    {
        let mut compartment_lamports = compartment_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **compartment_lamports = balance_after;
        if coalesced_recipient {
            let mut recipient_balance = keeper
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **recipient_balance = keeper_after;
        } else {
            let mut keeper_balance = keeper
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let mut payer_balance = payer_refund
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            **keeper_balance = keeper_after;
            **payer_balance = payer_after;
        }
    }
    Ok(transition)
}

/// Complete action-4 parser, Source mutation, receipt, and liveness result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicBoundaryIngestExecutionV1 {
    /// Exact parser-authenticated boundary consumed by the mutation.
    pub boundary: AuthenticatedBoundaryV1,
    /// Exact open-page mutation receipt.
    pub ingest: PersistedIngestBoundaryBatchV1,
    /// Persisted paid-work receipt and intent.
    pub work: SourceWorkExecutionV1,
    /// Applied Source-compartment debit/refund transition.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Execute one real parser CPI and atomically append its authenticated output,
/// persist the paid-work receipt, and debit Source liveness custody.
#[allow(clippy::too_many_arguments)]
pub fn ingest_parser_boundary_atomic(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    receiver: AuthenticatedReceiverRouteV2,
    clock: AuthenticatedClockBucketV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    open_lineage: AuthenticatedReopenLineageV1,
    feed: &AccountInfo<'_>,
    parser_instruction: &Instruction,
    parser_accounts: &[AccountInfo<'_>],
    open_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    schedule: SourceWorkScheduleBindingV1,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &AccountInfo<'_>,
    keeper_payment_lamports: u64,
    payer: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AtomicBoundaryIngestExecutionV1> {
    let open_state = open.open();
    let expected_bucket = open_state
        .start_bucket
        .checked_add(u64::from(open_state.record_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let boundary = invoke_parser_boundary(
        route,
        receiver,
        clock,
        feed,
        expected_bucket,
        open_state.repair_generation,
        parser_instruction,
        parser_accounts,
    )
    .map_err(Refusal::from)?;
    let batch = BoundaryBatchV1::new(&[boundary]).map_err(source_runtime)?;
    let ingest = ingest_boundary_batch(
        route,
        head,
        open,
        &batch,
        open_account,
        open_lineage_account,
        open_lineage,
    )?;
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        SourceWorkKindV1::AppendBoundaryBatch,
        ingest.semantic.transition_receipt_id,
        receipt_account,
        call_ordinal,
        call_ceiling_lamports,
        keeper.key,
        keeper_payment_lamports,
        payer,
        system_program,
        rent_sysvar,
    )?;
    let liveness = apply_source_work_liveness(
        program_id,
        route,
        work,
        policy_account,
        compartment_account,
        keeper,
        payer_refund,
    )?;
    Ok(AtomicBoundaryIngestExecutionV1 {
        boundary,
        ingest,
        work,
        liveness,
    })
}

/// Complete action-9 evaluator, persistence, receipt, and liveness result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicEvaluationExecutionV1 {
    /// Release-selected evaluator output authenticated after CPI.
    pub evaluation: AuthenticatedEvaluationV1,
    /// Persisted StatisticResult and bootstrapped lineage.
    pub result: OpenRuntimeAccountResultV1,
    /// Persisted paid-work receipt and intent.
    pub work: SourceWorkExecutionV1,
    /// Applied Source-compartment debit/refund transition.
    pub liveness: RuntimeAtomicTransitionV1,
}

/// Execute the release-selected evaluator CPI and atomically persist its
/// result, work receipt, and Source liveness debit.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_statistic_atomic(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    binding: EvaluationReleaseBindingV1,
    summary: SummaryProgramV3,
    evaluator_program: &AccountInfo<'_>,
    evaluator_programdata: &AccountInfo<'_>,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    evaluator_instruction: &Instruction,
    evaluator_accounts: &[AccountInfo<'_>],
    result_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    schedule: SourceWorkScheduleBindingV1,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &AccountInfo<'_>,
    keeper_payment_lamports: u64,
    payer: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AtomicEvaluationExecutionV1> {
    let evaluation = invoke_statistic_evaluator(
        route,
        binding,
        summary,
        evaluator_program,
        evaluator_programdata,
        clock,
        window,
        key,
        evidence,
        evaluator_instruction,
        evaluator_accounts,
    )
    .map_err(Refusal::from)?;
    let result = persist_evaluation_result(
        program_id,
        route,
        key,
        evidence,
        evaluation,
        payer,
        result_account,
        result_lineage_account,
        system_program,
        rent_sysvar,
    )?;
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        SourceWorkKindV1::EvaluateStatistic,
        result.account_data_id,
        receipt_account,
        call_ordinal,
        call_ceiling_lamports,
        keeper.key,
        keeper_payment_lamports,
        payer,
        system_program,
        rent_sysvar,
    )?;
    let liveness = apply_source_work_liveness(
        program_id,
        route,
        work,
        policy_account,
        compartment_account,
        keeper,
        payer_refund,
    )?;
    Ok(AtomicEvaluationExecutionV1 {
        evaluation,
        result,
        work,
        liveness,
    })
}

/// Atomic raw-page seal outputs across the immutable page, head CAS, and
/// consumed open-page generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealRawPageExecutionV1 {
    /// Semantic page/head transition.
    pub semantic: clutch_source_plane_v3_runtime::SealOpenPageOutputV1,
    /// Exact immutable RawPage rent postimage.
    pub page_funding: ImmutableAccountFundingV1,
    /// Exact immutable RawPage envelope persisted by this instruction.
    pub page_header: RuntimeAccountHeaderV1,
    /// Digest of the complete immutable RawPage account postimage.
    pub page_account_data_id: ContentId,
    /// SourceHead postimage and lineage CAS.
    pub head: MutateRuntimeAccountResultV1,
    /// OpenRawPage close/refund/sink postimage.
    pub open_close: CloseRuntimeAccountResultV1,
}

/// Atomic Window seal outputs across immutable evidence and consumed work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealWindowExecutionV1 {
    /// Exact authenticated semantic Window evidence.
    pub evidence: AuthenticatedWindowEvidenceV1,
    /// Immutable WindowSeal rent postimage.
    pub seal_funding: ImmutableAccountFundingV1,
    /// WindowWork close/refund/sink postimage.
    pub work_close: CloseRuntimeAccountResultV1,
}

/// Bind one semantic transition to a predictable persisted work receipt and
/// the sole liveness Source-compartment spend.
#[allow(clippy::too_many_arguments)]
pub fn bind_work_execution(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    kind: SourceWorkKindV1,
    semantic_receipt_id: ContentId,
    receipt_account: &AccountInfo<'_>,
    call_ordinal: u32,
    call_ceiling_lamports: u64,
    keeper: &Pubkey,
    keeper_payment_lamports: u64,
    payer: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SourceWorkExecutionV1> {
    let slot_id = SourceWorkAuthorizationV1::receipt_slot_id(
        route,
        schedule,
        kind,
        call_ordinal,
        semantic_receipt_id,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot_id).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        runtime_key(receipt_account.key) == derived.address,
        ClutchError::WrongPda,
    )?;
    let authorization = SourceWorkAuthorizationV1::new(
        route,
        schedule,
        kind,
        runtime_key(receipt_account.key),
        call_ordinal,
        call_ceiling_lamports,
        semantic_receipt_id,
    )
    .map_err(source_runtime)?;
    let receipt =
        SourceWorkReceiptAccountV1::from_work(route, authorization).map_err(source_runtime)?;

    // The account is authenticated from the exact postimage below. It is
    // deliberately writable only during this atomic creation instruction.
    let (receipt_funding, authenticated) = create_work_receipt_account(
        program_id,
        route,
        schedule,
        receipt,
        payer,
        receipt_account,
        system_program,
        rent_sysvar,
    )?;
    let observation = project_liveness_receipt(authenticated);
    let intent = project_liveness_work_intent(authenticated, keeper, keeper_payment_lamports)
        .map_err(Refusal::from)?;
    Ok(SourceWorkExecutionV1 {
        receipt,
        receipt_funding,
        observation,
        intent,
    })
}

/// Bind one checked terminal Source fact to its predictable persisted receipt
/// and the sole liveness close-success/close-failure intent.
#[allow(clippy::too_many_arguments)]
pub fn bind_terminal_execution(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    outcome: SourceTerminalOutcomeV1,
    semantic_terminal_receipt_id: ContentId,
    receipt_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SourceTerminalExecutionV1> {
    let slot_id = SourceTerminalAuthorizationV1::receipt_slot_id(
        route,
        schedule,
        outcome,
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot_id).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        runtime_key(receipt_account.key) == derived.address,
        ClutchError::WrongPda,
    )?;
    let authorization = SourceTerminalAuthorizationV1::new(
        route,
        schedule,
        outcome,
        runtime_key(receipt_account.key),
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let receipt =
        SourceWorkReceiptAccountV1::from_terminal(route, authorization).map_err(source_runtime)?;
    let (receipt_funding, authenticated) = create_work_receipt_account(
        program_id,
        route,
        schedule,
        receipt,
        payer,
        receipt_account,
        system_program,
        rent_sysvar,
    )?;
    Ok(SourceTerminalExecutionV1 {
        receipt,
        receipt_funding,
        observation: project_liveness_receipt(authenticated),
        intent: project_liveness_terminal_intent(authenticated).map_err(Refusal::from)?,
    })
}

/// Register action 1: persist the exact content-addressed reviewed release.
///
/// Release evidence is permanent registry state. Its creator sponsors only the
/// observed rent shortfall; a prefund never becomes creator principal or
/// authority, and this account deliberately has no close/refund action.
#[allow(clippy::too_many_arguments)]
pub fn register_release(
    program_id: &Pubkey,
    manifest: &SourceReleaseManifestV2,
    payer: &AccountInfo<'_>,
    release_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<ImmutableAccountFundingV1> {
    require_creation_roles(program_id, payer, release_account, system_program)?;
    let recipe =
        PdaRecipeV3::source_release(manifest.id().map_err(source_runtime)?).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(release_account.key),
        ClutchError::WrongPda,
    )?;
    let bytes = manifest.encode().map_err(source_runtime)?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = release_account.lamports();
    let debit = minimum.saturating_sub(before);
    create_with_recipe(
        program_id,
        payer,
        release_account,
        system_program,
        &rent,
        bytes.len(),
        &recipe,
        derived.bump,
    )?;
    write_exact_account_data(release_account, &bytes)?;
    Ok(ImmutableAccountFundingV1 {
        account: runtime_key(release_account.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            runtime_key(payer.key)
        },
        payer_debit_lamports: debit,
        donation_lamports: before,
        rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
        rent_exempt_minimum_lamports: minimum,
        account_balance_after: before
            .checked_add(debit)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    })
}

/// Register action 1 from the sealed content-addressed artifact transport.
/// The 1,296-byte successor manifest never appears in instruction data. Repeating the
/// same registration converges on the exact existing 0x8a account; any byte,
/// owner, or PDA mismatch refuses.
#[allow(clippy::too_many_arguments)]
pub fn register_release_from_artifact(
    program_id: &Pubkey,
    expected_manifest_id: ContentId,
    artifact_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    release_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<ImmutableAccountFundingV1> {
    expected_manifest_id.validate().map_err(source_core)?;
    require(
        artifact_account.owner == program_id
            && !artifact_account.is_signer
            && !artifact_account.is_writable
            && !artifact_account.executable
            && artifact_account.data_len()
                == clutch_source_plane_v3_runtime::SOURCE_RELEASE_MANIFEST_BYTES,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        artifact_account.key,
        seeds::product_artifact_pda(
            program_id,
            ArtifactKind::SourceReleaseManifestV2.byte(),
            &expected_manifest_id.bytes(),
        ),
        None,
    )?;
    let artifact_data = artifact_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let manifest = SourceReleaseManifestV2::decode(&artifact_data).map_err(source_runtime)?;
    require(
        manifest.id().map_err(source_runtime)? == expected_manifest_id,
        ClutchError::MismatchedState,
    )?;
    let expected_bytes = manifest.encode().map_err(source_runtime)?;
    drop(artifact_data);

    if release_account.owner == program_id && release_account.data_len() == expected_bytes.len() {
        require(
            release_account.is_writable
                && !release_account.is_signer
                && !release_account.executable,
            ClutchError::MismatchedState,
        )?;
        let recipe = PdaRecipeV3::source_release(expected_manifest_id).map_err(source_pda)?;
        let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
        require(
            derived.address == runtime_key(release_account.key),
            ClutchError::WrongPda,
        )?;
        let release_data = release_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            &*release_data == expected_bytes.as_slice(),
            ClutchError::MismatchedState,
        )?;
        let rent = read_rent(rent_sysvar)?;
        let minimum = rent.minimum_balance(expected_bytes.len())?;
        require(
            release_account.lamports() >= minimum,
            ClutchError::MismatchedState,
        )?;
        return Ok(ImmutableAccountFundingV1 {
            account: runtime_key(release_account.key),
            payer: RuntimeKey::ZERO,
            payer_debit_lamports: 0,
            donation_lamports: release_account.lamports(),
            rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
            rent_exempt_minimum_lamports: minimum,
            account_balance_after: release_account.lamports(),
        });
    }
    register_release(
        program_id,
        &manifest,
        payer,
        release_account,
        system_program,
        rent_sysvar,
    )
}

/// Authenticate the sole content-addressed paid-work schedule selected by the
/// immutable Source release. Callers supply only the physical account; they
/// cannot substitute a different schedule body with the same instruction.
pub fn authenticate_source_work_schedule_artifact(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule_account: &AccountInfo<'_>,
) -> Outcome<SourceWorkScheduleBindingV1> {
    require(
        schedule_account.owner == program_id
            && !schedule_account.is_signer
            && !schedule_account.is_writable
            && !schedule_account.executable
            && schedule_account.data_len()
                == clutch_source_plane_v3_runtime::SOURCE_WORK_SCHEDULE_BYTES,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        schedule_account.key,
        seeds::product_artifact_pda(
            program_id,
            ArtifactKind::SourceWorkScheduleV1.byte(),
            &route.source_work_schedule_id().bytes(),
        ),
        None,
    )?;
    let data = schedule_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let schedule = SourceWorkScheduleBindingV1::decode(&data).map_err(source_runtime)?;
    require(
        schedule.id().map_err(source_runtime)? == route.source_work_schedule_id()
            && schedule.liveness_policy_id() == route.liveness_policy_id()
            && schedule.source_compartment_account() == route.source_compartment_account()
            && schedule.source_compartment_owner() == route.source_compartment_owner()
            && schedule.receipt_account_owner_program() == route.adapter_program(),
        ClutchError::MismatchedState,
    )?;
    Ok(schedule)
}

/// Initialize action 2: atomically bootstrap the never-created release-bound
/// lineage and persist the exact first SourceHead generation.
#[allow(clippy::too_many_arguments)]
pub fn initialize_head(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    authorization: AuthenticatedSourceGenerationV1,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = initialize_source_head(route, authorization).map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_head(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        body.repair_generation,
    )
    .map_err(source_pda)?;
    let semantic_binding_id = recipe.id().map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::SourceHead,
        semantic_binding_id,
        &recipe,
        &body,
        payer,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Open action 3: create the state-assigned page derived only from SourceHead.
#[allow(clippy::too_many_arguments)]
pub fn open_raw_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = head.head().open_page().map_err(source_core)?;
    let recipe = PdaRecipeV3::open_raw_page(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        body.repair_generation,
        body.page_index,
    )
    .map_err(source_pda)?;
    let semantic_binding_id = recipe.id().map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::OpenRawPage,
        semantic_binding_id,
        &recipe,
        &body,
        payer,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Ingest action 4: append an already-authenticated bounded batch and persist
/// the open-page compare-and-swap. Sealing is action 5 and is refused here.
pub fn ingest_boundary_batch(
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    batch: &BoundaryBatchV1,
    open_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    open_lineage: AuthenticatedReopenLineageV1,
) -> Outcome<PersistedIngestBoundaryBatchV1> {
    let output = clutch_source_plane_v3_runtime::ingest_boundary_batch(
        route,
        head,
        open,
        batch,
        SealBatchModeV1::KeepOpen,
    )
    .map_err(source_runtime)?;
    let open_after = output
        .open_after
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let mutation = mutate_runtime_account(
        route,
        open_lineage,
        open_account,
        open_lineage_account,
        &open_after,
    )?;
    Ok(PersistedIngestBoundaryBatchV1 {
        semantic: output,
        mutation,
    })
}

/// Initialize action 6: create one predictable WindowWork generation.
#[allow(clippy::too_many_arguments)]
pub fn initialize_window_work(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = WindowWorkV3::new(window).map_err(source_core)?;
    let window_id = window.id().map_err(source_core)?;
    let recipe = PdaRecipeV3::window_work(window_id).map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::WindowWork,
        window_id,
        &recipe,
        &body,
        payer,
        target,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Fold action 7: persist the exact bounded WindowWork postimage and lineage CAS.
pub fn fold_window_pages(
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    work: AuthenticatedWindowWorkV1,
    pages: &[clutch_source_plane_v3_runtime::AuthenticatedRawPageV1],
    work_account: &AccountInfo<'_>,
    work_lineage_account: &AccountInfo<'_>,
    work_lineage: AuthenticatedReopenLineageV1,
) -> Outcome<clutch_source_plane_v3_runtime::FoldPagesOutputV1> {
    let output =
        clutch_source_plane_v3_runtime::fold_authenticated_pages(route, window, work, pages)
            .map_err(source_runtime)?;
    mutate_runtime_account(
        route,
        work_lineage,
        work_account,
        work_lineage_account,
        &output.work_after,
    )?;
    Ok(output)
}

/// Seal action 5: create the immutable RawPage, advance SourceHead, then close
/// the consumed open generation. Every semantic check and funding split is
/// computed before its corresponding write; any later refusal rolls the whole
/// Solana instruction back.
#[allow(clippy::too_many_arguments)]
pub fn seal_raw_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    head_lineage: AuthenticatedReopenLineageV1,
    open_lineage: AuthenticatedReopenLineageV1,
    head_account: &AccountInfo<'_>,
    open_account: &AccountInfo<'_>,
    head_lineage_account: &AccountInfo<'_>,
    open_lineage_account: &AccountInfo<'_>,
    page_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    open_principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SealRawPageExecutionV1> {
    let semantic = clutch_source_plane_v3_runtime::seal_authenticated_open_page(route, head, open)
        .map_err(source_runtime)?;
    let page_recipe = PdaRecipeV3::raw_page(
        route.source_plane_contract_id(),
        semantic.sealed_page.id().map_err(source_core)?,
    )
    .map_err(source_pda)?;
    let (page_funding, page_header, page_account_data_id) = create_immutable_runtime_account(
        program_id,
        route,
        &page_recipe,
        &semantic.sealed_page,
        payer,
        page_account,
        system_program,
        rent_sysvar,
    )?;
    let head_postimage = mutate_runtime_account(
        route,
        head_lineage,
        head_account,
        head_lineage_account,
        &semantic.head_after,
    )?;
    let open_close = close_runtime_account::<OpenRawPageV3>(
        program_id,
        route,
        open_lineage,
        open_account,
        open_lineage_account,
        open_principal_refund,
        neutral_sink,
        semantic.transition_receipt_id,
    )?;
    Ok(SealRawPageExecutionV1 {
        semantic,
        page_funding,
        page_header,
        page_account_data_id,
        head: head_postimage,
        open_close,
    })
}

/// Seal action 8: freeze exact Window evidence and close its consumed work
/// generation with the persisted payer/donation split.
#[allow(clippy::too_many_arguments)]
pub fn seal_window(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    source_plane: &SourcePlaneProgramV3,
    clock_policy: &ClockPolicyV1,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    work: AuthenticatedWindowWorkV1,
    maturity_page: AuthenticatedRawPageV1,
    work_lineage: AuthenticatedReopenLineageV1,
    work_account: &AccountInfo<'_>,
    work_lineage_account: &AccountInfo<'_>,
    seal_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    work_principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<SealWindowExecutionV1> {
    let evidence = clutch_source_plane_v3_runtime::seal_authenticated_window(
        route,
        source_plane,
        clock_policy,
        clock,
        window,
        work,
        maturity_page,
    )
    .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::window_seal(window.id().map_err(source_core)?).map_err(source_pda)?;
    let (seal_funding, _, _) = create_immutable_runtime_account(
        program_id,
        route,
        &recipe,
        &evidence.seal(),
        payer,
        seal_account,
        system_program,
        rent_sysvar,
    )?;
    let work_close = close_runtime_account::<WindowWorkV3>(
        program_id,
        route,
        work_lineage,
        work_account,
        work_lineage_account,
        work_principal_refund,
        neutral_sink,
        evidence.id(),
    )?;
    Ok(SealWindowExecutionV1 {
        evidence,
        seal_funding,
        work_close,
    })
}

/// Evaluate action 9 persistence half: consume only a release-authenticated
/// evaluator result and open its exact StatisticKey generation. The CPI-facing
/// adapter constructs `evaluation`; this function owns the durable postimage.
#[allow(clippy::too_many_arguments)]
pub fn persist_evaluation_result(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    evaluation: AuthenticatedEvaluationV1,
    payer: &AccountInfo<'_>,
    result_account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let key_id = key.id().map_err(source_core)?;
    require(
        evaluation.statistic_key_id() == key_id && evaluation.window_evidence_id() == evidence.id(),
        ClutchError::MismatchedState,
    )?;
    let recipe = PdaRecipeV3::statistic_result(key_id).map_err(source_pda)?;
    bootstrap_runtime_account(
        program_id,
        route,
        LineageFamilyV1::StatisticResult,
        key_id,
        &recipe,
        &evaluation.result(),
        payer,
        result_account,
        lineage_account,
        system_program,
        rent_sysvar,
    )
}

/// Persist one immutable RawPage or WindowSeal account with its exact header
/// and explicit payer/donation rent partition.
#[allow(clippy::too_many_arguments)]
fn create_immutable_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    recipe: &PdaRecipeV3,
    body: &T,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<(ImmutableAccountFundingV1, RuntimeAccountHeaderV1, ContentId)> {
    require_creation_roles(program_id, payer, target, system_program)?;
    let derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_key = if debit == 0 {
        RuntimeKey::ZERO
    } else {
        runtime_key(payer.key)
    };
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: derived.bump,
        principal_recipient: payer_key,
        payer_principal_lamports: debit,
        donation_floor_lamports: before,
        generation: 1,
    };
    let mut postimage = vec![0_u8; space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let data_id = account_data_id(runtime_key(target.key), &postimage).map_err(source_runtime)?;
    create_with_recipe(
        program_id,
        payer,
        target,
        system_program,
        &rent,
        space,
        recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &postimage)?;
    Ok((
        ImmutableAccountFundingV1 {
            account: runtime_key(target.key),
            payer: payer_key,
            payer_debit_lamports: debit,
            donation_lamports: before,
            rent_sysvar_id: rent_id,
            rent_exempt_minimum_lamports: minimum,
            account_balance_after: after,
        },
        header,
        data_id,
    ))
}

/// Persist one immutable 0x92 work receipt. Its rent ownership is returned
/// explicitly; liveness work principal remains in the separate Source account.
#[allow(clippy::too_many_arguments)]
pub fn create_work_receipt_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    receipt: SourceWorkReceiptAccountV1,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<(ImmutableAccountFundingV1, AuthenticatedSourceWorkReceiptV1)> {
    require_creation_roles(program_id, payer, target, system_program)?;
    let slot = receipt
        .receipt_slot_id(route, schedule)
        .map_err(source_runtime)?;
    let recipe = PdaRecipeV3::source_work_receipt(slot).map_err(source_pda)?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    require(
        derived.address == runtime_key(target.key)
            && receipt.receipt_account_id() == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let bytes = receipt.encode().map_err(source_runtime)?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let minimum = rent.minimum_balance(bytes.len())?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    create_with_recipe(
        program_id,
        payer,
        target,
        system_program,
        &rent,
        bytes.len(),
        &recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &bytes)?;
    let funding = ImmutableAccountFundingV1 {
        account: runtime_key(target.key),
        payer: if debit == 0 {
            RuntimeKey::ZERO
        } else {
            runtime_key(payer.key)
        },
        payer_debit_lamports: debit,
        donation_lamports: before,
        rent_sysvar_id: rent_id,
        rent_exempt_minimum_lamports: minimum,
        account_balance_after: after,
    };
    let data = target
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let authenticated = authenticate_source_work_receipt_account(
        route,
        schedule,
        RuntimeAccountViewV1 {
            key: runtime_key(target.key),
            owner: runtime_key(target.owner),
            lamports: target.lamports(),
            executable: target.executable,
            writable: target.is_writable,
            signer: target.is_signer,
            data: &data,
        },
        derived,
        SourceWorkReceiptAccessV1::CreatedMutable,
    )
    .map_err(source_runtime)?;
    Ok((funding, authenticated))
}

/// Emit action 10 absence path: bind the mature absence handoff to its three
/// physical evidence accounts and the persisted paid-work receipt.
pub fn join_failure_absence_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: FailurePolicySourceHandoffV1,
    absence: AuthenticatedStatisticResultAbsenceV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::failure_absence(route, handoff, absence, work_receipt)
        .map_err(source_runtime)
}

/// Emit action 10 refusal path: bind the durable refused result account and
/// exact failure handoff to one persisted paid-work receipt.
pub fn join_failure_result_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: FailurePolicySourceHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::failure_result(route, handoff, result, work_receipt)
        .map_err(source_runtime)
}

/// Emit action 10 success path: bind successful source evidence for downstream
/// relation review without allowing Source to classify the relation outcome.
pub fn join_successful_evaluation_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: SuccessfulEvaluationHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    SourcePolicyHandoffJoinV1::successful_evaluation(route, handoff, result, work_receipt)
        .map_err(source_runtime)
}

/// Generic action 11 engine: open the exact next generation and persist body
/// plus lineage as one rollback-safe postimage pair.
#[allow(clippy::too_many_arguments)]
pub fn reopen_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    recipe: &PdaRecipeV3,
    body: &T,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    require_creation_roles(program_id, payer, target, system_program)?;
    require_lineage_account(route, lineage, lineage_account)?;
    require(
        lineage.lineage().latest_generation != 0 && !lineage.lineage().is_open,
        ClutchError::MismatchedState,
    )?;
    let derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    let reopen = authorize_reopen(
        route,
        lineage,
        family,
        semantic_binding_id,
        recipe.id().map_err(source_pda)?,
        runtime_key(target.key),
        derived,
    )
    .map_err(source_runtime)?;
    let space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    let debit = minimum.saturating_sub(before);
    let after = before
        .checked_add(debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let funding = plan_source_account_creation(
        route,
        reopen,
        RentExemptionQuoteV1 {
            rent_sysvar_id: rent_sysvar_id(rent_sysvar)?,
            account: runtime_key(target.key),
            data_len: u32::try_from(space)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            minimum_balance_lamports: minimum,
        },
        runtime_key(payer.key),
        before,
        debit,
        after,
    )
    .map_err(source_runtime)?;
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: derived.bump,
        principal_recipient: funding.ledger.principal_recipient,
        payer_principal_lamports: funding.ledger.payer_principal_lamports,
        donation_floor_lamports: funding.ledger.donation_lamports,
        generation: funding.ledger.generation,
    };
    let mut postimage = vec![0_u8; space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let data_id = account_data_id(runtime_key(target.key), &postimage).map_err(source_runtime)?;
    let lineage_after =
        open_lineage_generation(lineage.lineage(), reopen, data_id).map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;

    create_with_recipe(
        program_id,
        payer,
        target,
        system_program,
        &rent,
        space,
        recipe,
        derived.bump,
    )?;
    write_exact_account_data(target, &postimage)?;
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(OpenRuntimeAccountResultV1 {
        funding,
        lineage_funding: None,
        header,
        account_data_id: data_id,
        lineage_after,
    })
}

/// Create a permanent release/route-bound lineage and its first runtime
/// generation as one rollback-safe instruction. This is intentionally used
/// only for action 2; action 11 requires an authenticated closed lineage.
#[allow(clippy::too_many_arguments)]
fn bootstrap_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    recipe: &PdaRecipeV3,
    body: &T,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    require_creation_roles(program_id, payer, target, system_program)?;
    require_creation_roles(program_id, payer, lineage_account, system_program)?;
    require(target.key != lineage_account.key, ClutchError::AccountAlias)?;
    let target_derived = derive_runtime_pda(program_id, recipe).map_err(Refusal::from)?;
    require(
        target_derived.address == runtime_key(target.key),
        ClutchError::WrongPda,
    )?;
    let lineage_recipe_id = ReopenLineageV1::recipe_id_for(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        family,
        semantic_binding_id,
        route.source_work_schedule_id(),
    )
    .map_err(source_runtime)?;
    let lineage_recipe = PdaRecipeV3::reopen_lineage(lineage_recipe_id).map_err(source_pda)?;
    let lineage_derived = derive_runtime_pda(program_id, &lineage_recipe).map_err(Refusal::from)?;
    require(
        lineage_derived.address == runtime_key(lineage_account.key),
        ClutchError::WrongPda,
    )?;
    let lineage = ReopenLineageV1::new(
        route.adapter_program(),
        route.release_manifest_id(),
        route.route_id(),
        semantic_binding_id,
        runtime_key(lineage_account.key),
        family,
        route.source_work_schedule_id(),
        route.neutral_sink(),
    )
    .map_err(source_runtime)?;
    let reopen = authorize_reopen(
        route,
        lineage,
        family,
        semantic_binding_id,
        recipe.id().map_err(source_pda)?,
        runtime_key(target.key),
        target_derived,
    )
    .map_err(source_runtime)?;
    let target_space = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let rent_id = rent_sysvar_id(rent_sysvar)?;
    let target_minimum = rent.minimum_balance(target_space)?;
    let target_before = target.lamports();
    let target_debit = target_minimum.saturating_sub(target_before);
    let target_after = target_before
        .checked_add(target_debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let funding = plan_source_account_creation(
        route,
        reopen,
        RentExemptionQuoteV1 {
            rent_sysvar_id: rent_id,
            account: runtime_key(target.key),
            data_len: u32::try_from(target_space)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            minimum_balance_lamports: target_minimum,
        },
        runtime_key(payer.key),
        target_before,
        target_debit,
        target_after,
    )
    .map_err(source_runtime)?;
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: target_derived.bump,
        principal_recipient: funding.ledger.principal_recipient,
        payer_principal_lamports: funding.ledger.payer_principal_lamports,
        donation_floor_lamports: funding.ledger.donation_lamports,
        generation: funding.ledger.generation,
    };
    let mut target_postimage = vec![0_u8; target_space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut target_postimage)
        .map_err(source_runtime)?;
    let target_data_id =
        account_data_id(runtime_key(target.key), &target_postimage).map_err(source_runtime)?;
    let lineage_after =
        open_lineage_generation(lineage, reopen, target_data_id).map_err(source_runtime)?;
    let lineage_postimage = lineage_after.encode().map_err(source_runtime)?;
    let lineage_minimum = rent.minimum_balance(REOPEN_LINEAGE_BYTES)?;
    let lineage_before = lineage_account.lamports();
    let lineage_debit = lineage_minimum.saturating_sub(lineage_before);
    let lineage_after_balance = lineage_before
        .checked_add(lineage_debit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let lineage_funding = ImmutableAccountFundingV1 {
        account: runtime_key(lineage_account.key),
        payer: if lineage_debit == 0 {
            RuntimeKey::ZERO
        } else {
            runtime_key(payer.key)
        },
        payer_debit_lamports: lineage_debit,
        donation_lamports: lineage_before,
        rent_sysvar_id: rent_id,
        rent_exempt_minimum_lamports: lineage_minimum,
        account_balance_after: lineage_after_balance,
    };

    create_with_recipe(
        program_id,
        payer,
        lineage_account,
        system_program,
        &rent,
        REOPEN_LINEAGE_BYTES,
        &lineage_recipe,
        lineage_derived.bump,
    )?;
    create_with_recipe(
        program_id,
        payer,
        target,
        system_program,
        &rent,
        target_space,
        recipe,
        target_derived.bump,
    )?;
    write_exact_account_data(lineage_account, &lineage_postimage)?;
    write_exact_account_data(target, &target_postimage)?;
    Ok(OpenRuntimeAccountResultV1 {
        funding,
        lineage_funding: Some(lineage_funding),
        header,
        account_data_id: target_data_id,
        lineage_after,
    })
}

/// Persist one typed runtime body and advance the exact open lineage CAS.
fn mutate_runtime_account<T: RuntimeAccountBodyV1>(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    body_after: &T,
) -> Outcome<MutateRuntimeAccountResultV1> {
    require_lineage_account(route, lineage, lineage_account)?;
    require(
        account.owner == &Pubkey::new_from_array(route.adapter_program().bytes())
            && account.is_writable
            && !account.executable
            && !account.is_signer,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (header, _) =
        decode_runtime_account::<T>(&data, route.neutral_sink()).map_err(source_runtime)?;
    let before_id = account_data_id(runtime_key(account.key), &data).map_err(source_runtime)?;
    drop(data);
    let state = lineage.lineage();
    require(
        state.is_open
            && state.active_account == runtime_key(account.key)
            && state.latest_generation == header.generation
            && state.last_opened_state_id == before_id,
        ClutchError::MismatchedState,
    )?;
    let mut postimage = vec![0_u8; RUNTIME_ACCOUNT_HEADER_BYTES + T::ENCODED_LEN];
    encode_runtime_account(header, body_after, route.neutral_sink(), &mut postimage)
        .map_err(source_runtime)?;
    let after_id = account_data_id(runtime_key(account.key), &postimage).map_err(source_runtime)?;
    let lineage_after = advance_lineage_state(
        state,
        runtime_key(account.key),
        header.generation,
        before_id,
        after_id,
    )
    .map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;
    write_exact_account_data(account, &postimage)?;
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(MutateRuntimeAccountResultV1 {
        account_data_before_id: before_id,
        account_data_after_id: after_id,
        lineage_after,
    })
}

/// Close action 12 for a SourceHead generation.
#[allow(clippy::too_many_arguments)]
pub fn close_head_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_runtime_account::<SourceHeadV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        semantic_terminal_receipt_id,
    )
}

/// Close action 12 for an OpenRawPage generation.
#[allow(clippy::too_many_arguments)]
pub fn close_open_page_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_runtime_account::<OpenRawPageV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        semantic_terminal_receipt_id,
    )
}

/// Close action 12 for a WindowWork generation.
#[allow(clippy::too_many_arguments)]
pub fn close_window_work_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_runtime_account::<WindowWorkV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        semantic_terminal_receipt_id,
    )
}

/// Close action 12 for a persisted StatisticResult repair generation.
#[allow(clippy::too_many_arguments)]
pub fn close_statistic_result_generation(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    close_runtime_account::<StatisticResultV3>(
        program_id,
        route,
        lineage,
        account,
        lineage_account,
        principal_refund,
        neutral_sink,
        semantic_terminal_receipt_id,
    )
}

/// Generic action 12 engine: close one mutable generation, refund only stored
/// payer principal, sink every donation/surplus lamport, and retain lineage.
fn close_runtime_account<T: RuntimeAccountBodyV1>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    semantic_terminal_receipt_id: ContentId,
) -> Outcome<CloseRuntimeAccountResultV1> {
    require_lineage_account(route, lineage, lineage_account)?;
    require(
        account.owner == program_id
            && account.is_writable
            && principal_refund.is_writable
            && neutral_sink.is_writable
            && !account.executable
            && !principal_refund.executable
            && !neutral_sink.executable,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (header, _) =
        decode_runtime_account::<T>(&data, route.neutral_sink()).map_err(source_runtime)?;
    let final_state_id =
        account_data_id(runtime_key(account.key), &data).map_err(source_runtime)?;
    drop(data);
    require(
        runtime_key(principal_refund.key) == header.principal_recipient
            || (header.payer_principal_lamports == 0
                && runtime_key(principal_refund.key) != route.neutral_sink()),
        ClutchError::MismatchedState,
    )?;
    require(
        runtime_key(neutral_sink.key) == route.neutral_sink()
            && account.key != principal_refund.key
            && account.key != neutral_sink.key
            && principal_refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    let funding = plan_runtime_account_close_from_header(
        runtime_key(account.key),
        header,
        route.neutral_sink(),
        account.lamports(),
        semantic_terminal_receipt_id,
    )
    .map_err(source_runtime)?;
    let lineage_after = close_lineage_generation(
        lineage.lineage(),
        runtime_key(account.key),
        header.generation,
        final_state_id,
        funding.close_receipt_id,
    )
    .map_err(source_runtime)?;
    let lineage_bytes = lineage_after.encode().map_err(source_runtime)?;
    let refund_after = principal_refund
        .lamports()
        .checked_add(funding.payer_refund_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(funding.neutral_surplus_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    debit_all(account)?;
    credit_lamports(principal_refund, funding.payer_refund_lamports)?;
    credit_lamports(neutral_sink, funding.neutral_surplus_lamports)?;
    require(
        account.lamports() == 0
            && principal_refund.lamports() == refund_after
            && neutral_sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )?;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    write_exact_account_data(lineage_account, &lineage_bytes)?;
    Ok(CloseRuntimeAccountResultV1 {
        funding,
        lineage_after,
    })
}

fn require_creation_roles(
    program_id: &Pubkey,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require_creatable(target)?;
    require(
        payer.is_signer
            && payer.is_writable
            && !payer.executable
            && payer.key != target.key
            && program_id != &SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

fn require_lineage_account(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    require(
        lineage.access() == clutch_source_plane_v3_runtime::LineageAccessV1::Mutable
            && runtime_key(account.key) == lineage.lineage().lineage_account
            && account.owner == &Pubkey::new_from_array(route.adapter_program().bytes())
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_with_recipe<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    recipe: &PdaRecipeV3,
    bump: u8,
) -> Outcome<()> {
    let mut seeds: [&[u8]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1] =
        [&[]; clutch_source_plane_v3_adapter::MAX_PDA_SEEDS + 1];
    let count = usize::from(recipe.seed_count());
    let mut index = 0_usize;
    while index < count {
        seeds[index] = recipe.seed(index).map_err(source_pda)?;
        index += 1;
    }
    let bump_seed = [bump];
    seeds[count] = &bump_seed;
    create_pda_account(
        program_id,
        payer,
        target,
        system_program,
        rent,
        space,
        &seeds[..=count],
    )
}

fn write_exact_account_data(account: &AccountInfo<'_>, bytes: &[u8]) -> Outcome<()> {
    require(
        account.is_writable && account.data_len() == bytes.len(),
        ClutchError::WrongDataLength,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(bytes);
    Ok(())
}

fn rent_sysvar_id(rent_sysvar: &AccountInfo<'_>) -> Outcome<ContentId> {
    let data = rent_sysvar
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    account_data_id(runtime_key(rent_sysvar.key), &data).map_err(source_runtime)
}

fn debit_all(account: &AccountInfo<'_>) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = 0;
    Ok(())
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = (**lamports)
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn source_runtime(error: clutch_source_plane_v3_runtime::Error) -> Refusal {
    Refusal::from(SourceV3SbfError::Runtime(error))
}

fn source_core(error: clutch_source_plane_v3::Error) -> Refusal {
    source_runtime(clutch_source_plane_v3_runtime::Error::Core(error))
}

fn source_pda(error: clutch_source_plane_v3_adapter::Error) -> Refusal {
    Refusal::from(SourceV3SbfError::Pda(error))
}
