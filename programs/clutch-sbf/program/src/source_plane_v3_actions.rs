//! Unreachable-until-enabled SourcePlane V3 mutation handlers.
//!
//! The central SourceSeries 77/v2 coordinates are allocated, but the product
//! capability table remains empty for actions 1 through 12. This module owns
//! their complete inner execution: all semantic outputs are planned before a
//! write, predictable accounts are prefund-safe, mutable postimages advance
//! their durable lineage in the same instruction, immutable accounts retain
//! an explicit payer/donation rent partition, and every paid transition emits
//! the exact Source receipt plus liveness intent. The dispatcher cannot enter
//! these handlers until the separate capability gate is changed.

use std::vec;
use std::vec::Vec;

use clutch_liveness::runtime_adapter_v1::{RuntimeReceiptObservationV1, RuntimeTransitionIntentV1};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, SourcePlaneProgramV3, StatisticKeyV3,
    StatisticResultV3, WindowSpecV3, WindowWorkV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, advance_lineage_state, authenticate_source_work_receipt_account,
    authorize_reopen, close_lineage_generation, decode_runtime_account, encode_runtime_account,
    initialize_source_head, open_lineage_generation, plan_runtime_account_close_from_header,
    plan_source_account_creation, AccountCloseFundingV1, AccountCreationFundingV1,
    AuthenticatedEvaluationV1, AuthenticatedOpenRawPageV1, AuthenticatedRawPageV1,
    AuthenticatedReopenLineageV1, AuthenticatedSourceGenerationV1, AuthenticatedSourceHeadV1,
    AuthenticatedSourceRouteV1, AuthenticatedSourceWorkReceiptV1,
    AuthenticatedStatisticResultAbsenceV1, AuthenticatedStatisticResultAccountV1,
    AuthenticatedWindowEvidenceV1, AuthenticatedWindowWorkV1, BoundaryBatchV1, ClockPolicyV1,
    ClockSnapshotV1, FailurePolicySourceHandoffV1, IngestBatchOutputV1, LineageFamilyV1,
    RentExemptionQuoteV1, ReopenLineageV1, RuntimeAccountBodyV1, RuntimeAccountHeaderV1,
    RuntimeAccountViewV1, RuntimeKey, SealBatchModeV1, SourceReceiptDispositionV1,
    SourceReleaseManifestV1, SourceTerminalAuthorizationV1, SourceTerminalOutcomeV1,
    SourceWorkAuthorizationV1, SourceWorkKindV1, SourceWorkReceiptAccessV1,
    SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1, SuccessfulEvaluationHandoffV1,
    RUNTIME_ACCOUNT_HEADER_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program, RentParameters,
    SYSTEM_PROGRAM_ID,
};
use crate::source_plane_v3::{
    derive_runtime_pda, project_liveness_receipt, project_liveness_terminal_intent,
    project_liveness_work_intent, runtime_key, SourceV3SbfError,
};

/// Complete open-account postimage committed with one lineage postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRuntimeAccountResultV1 {
    /// Exact prefund-safe payer/donation partition.
    pub funding: AccountCreationFundingV1,
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

/// Atomic raw-page seal outputs across the immutable page, head CAS, and
/// consumed open-page generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealRawPageExecutionV1 {
    /// Semantic page/head transition.
    pub semantic: clutch_source_plane_v3_runtime::SealOpenPageOutputV1,
    /// Exact immutable RawPage rent postimage.
    pub page_funding: ImmutableAccountFundingV1,
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

/// Physical-account join delivered with one persisted policy handoff receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePolicyHandoffJoinV1 {
    /// Exact source-only handoff identity used as receipt semantics.
    handoff_id: ContentId,
    /// Release owner/PDA/body/deployment authentication identity.
    release_authentication_id: ContentId,
    /// Exact authenticated route owning every joined account.
    route_id: ContentId,
    /// Physical Product/Series occurrence account.
    occurrence_account: RuntimeKey,
    /// Physical StatisticResult or absent-result slot.
    result_or_absence_account: RuntimeKey,
    /// Exact result-account or absence authentication identity.
    source_fact_authentication_id: ContentId,
    /// Physical persisted 0x92 work receipt.
    work_receipt_account: RuntimeKey,
    /// Exact 0x92 owner/PDA/body/schedule authentication identity.
    work_receipt_authentication_id: ContentId,
    /// Frozen Clock policy identity.
    clock_policy_id: ContentId,
    /// Adapter-authenticated maturity Clock snapshot.
    clock: ClockSnapshotV1,
    /// Exact Source liveness generation.
    generation: u64,
    /// Failure binding selected by the Product occurrence.
    failure_policy_binding_id: ContentId,
    /// Existing SourceSpec fixed by the occurrence and release.
    source_spec_id: ContentId,
    /// Primary or repair Window fixed by the occurrence.
    window_id: ContentId,
    /// Exact StatisticKey fixed before evaluation.
    statistic_key_id: ContentId,
}

impl SourcePolicyHandoffJoinV1 {
    /// Exact source-only handoff identity persisted by the 0x92 receipt.
    pub const fn handoff_id(self) -> ContentId {
        self.handoff_id
    }

    /// Release owner/PDA/body/deployment authentication identity.
    pub const fn release_authentication_id(self) -> ContentId {
        self.release_authentication_id
    }

    /// Exact authenticated Source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical Product/Series occurrence account.
    pub const fn occurrence_account(self) -> RuntimeKey {
        self.occurrence_account
    }

    /// Physical StatisticResult or absent-result slot.
    pub const fn result_or_absence_account(self) -> RuntimeKey {
        self.result_or_absence_account
    }

    /// Exact result-account or absence authentication identity.
    pub const fn source_fact_authentication_id(self) -> ContentId {
        self.source_fact_authentication_id
    }

    /// Physical immutable 0x92 receipt account.
    pub const fn work_receipt_account(self) -> RuntimeKey {
        self.work_receipt_account
    }

    /// Exact 0x92 owner/PDA/body/schedule authentication identity.
    pub const fn work_receipt_authentication_id(self) -> ContentId {
        self.work_receipt_authentication_id
    }

    /// Frozen Clock policy identity.
    pub const fn clock_policy_id(self) -> ContentId {
        self.clock_policy_id
    }

    /// Adapter-authenticated maturity Clock snapshot.
    pub const fn clock(self) -> ClockSnapshotV1 {
        self.clock
    }

    /// Exact Source liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Frozen downstream FailurePolicy binding.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }

    /// Existing SourceSpec identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Exact primary or repair Window identity.
    pub const fn window_id(self) -> ContentId {
        self.window_id
    }

    /// Exact StatisticKey fixed before evaluation.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }
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
    manifest: &SourceReleaseManifestV1,
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

/// Initialize action 2: construct and persist the exact first/reopened SourceHead.
#[allow(clippy::too_many_arguments)]
pub fn initialize_head(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    authorization: AuthenticatedSourceGenerationV1,
    lineage: AuthenticatedReopenLineageV1,
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
    open_runtime_account(
        program_id,
        route,
        lineage,
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
    lineage: AuthenticatedReopenLineageV1,
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
    open_runtime_account(
        program_id,
        route,
        lineage,
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
) -> Outcome<IngestBatchOutputV1> {
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
    mutate_runtime_account(
        route,
        open_lineage,
        open_account,
        open_lineage_account,
        &open_after,
    )?;
    Ok(output)
}

/// Initialize action 6: create one predictable WindowWork generation.
#[allow(clippy::too_many_arguments)]
pub fn initialize_window_work(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    lineage: AuthenticatedReopenLineageV1,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<OpenRuntimeAccountResultV1> {
    let body = WindowWorkV3::new(window).map_err(source_core)?;
    let window_id = window.id().map_err(source_core)?;
    let recipe = PdaRecipeV3::window_work(window_id).map_err(source_pda)?;
    open_runtime_account(
        program_id,
        route,
        lineage,
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
    let (page_funding, _, _) = create_immutable_runtime_account(
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
    lineage: AuthenticatedReopenLineageV1,
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
    open_runtime_account(
        program_id,
        route,
        lineage,
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
    require(
        handoff.source_fact_receipt_id() == absence.id(),
        ClutchError::MismatchedState,
    )?;
    join_policy_handoff(
        route,
        handoff.id(),
        handoff.failure_policy_binding_id(),
        handoff.occurrence(),
        absence.result_account(),
        absence.id(),
        handoff.clock(),
        work_receipt,
    )
}

/// Emit action 10 refusal path: bind the durable refused result account and
/// exact failure handoff to one persisted paid-work receipt.
pub fn join_failure_result_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: FailurePolicySourceHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    require(
        handoff.source_fact_receipt_id() == result.id(),
        ClutchError::MismatchedState,
    )?;
    join_policy_handoff(
        route,
        handoff.id(),
        handoff.failure_policy_binding_id(),
        handoff.occurrence(),
        result.account(),
        result.id(),
        handoff.clock(),
        work_receipt,
    )
}

/// Emit action 10 success path: bind successful source evidence for downstream
/// relation review without allowing Source to classify the relation outcome.
pub fn join_successful_evaluation_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff: SuccessfulEvaluationHandoffV1,
    result: AuthenticatedStatisticResultAccountV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    require(
        handoff.result_account_authentication_id() == result.id(),
        ClutchError::MismatchedState,
    )?;
    join_policy_handoff(
        route,
        handoff.id(),
        handoff.failure_policy_binding_id(),
        handoff.occurrence(),
        result.account(),
        result.id(),
        handoff.clock(),
        work_receipt,
    )
}

#[allow(clippy::too_many_arguments)]
fn join_policy_handoff(
    route: AuthenticatedSourceRouteV1,
    handoff_id: ContentId,
    failure_policy_binding_id: ContentId,
    occurrence: clutch_source_plane_v3_runtime::OccurrenceSourceReceiptV1,
    result_or_absence_account: RuntimeKey,
    source_fact_authentication_id: ContentId,
    clock: ClockSnapshotV1,
    work_receipt: AuthenticatedSourceWorkReceiptV1,
) -> Outcome<SourcePolicyHandoffJoinV1> {
    let receipt = work_receipt.receipt();
    require(
        occurrence.route_id() == route.route_id()
            && occurrence.source_spec_id() == route.source_spec_id()
            && occurrence.source_plane_contract_id() == route.source_plane_contract_id()
            && occurrence.clock_policy_id() == route.clock_policy_id()
            && receipt.route_id() == route.route_id()
            && receipt.disposition() == SourceReceiptDispositionV1::Work
            && receipt.work_kind() == Some(SourceWorkKindV1::FailureHandoff)
            && receipt.semantic_receipt_id() == handoff_id,
        ClutchError::MismatchedState,
    )?;
    Ok(SourcePolicyHandoffJoinV1 {
        handoff_id,
        release_authentication_id: route.release_authentication_id(),
        route_id: route.route_id(),
        occurrence_account: occurrence.occurrence_account(),
        result_or_absence_account,
        source_fact_authentication_id,
        work_receipt_account: work_receipt.account(),
        work_receipt_authentication_id: work_receipt.id(),
        clock_policy_id: occurrence.clock_policy_id(),
        clock,
        generation: receipt.generation(),
        failure_policy_binding_id,
        source_spec_id: occurrence.source_spec_id(),
        window_id: occurrence.window_id(),
        statistic_key_id: occurrence.statistic_key_id(),
    })
}

/// Generic action 11 engine: open the exact next generation and persist body
/// plus lineage as one rollback-safe postimage pair.
#[allow(clippy::too_many_arguments)]
fn open_runtime_account<T: RuntimeAccountBodyV1>(
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
        header,
        account_data_id: data_id,
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
