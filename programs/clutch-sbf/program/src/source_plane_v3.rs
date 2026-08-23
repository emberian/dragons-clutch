//! Solana runtime boundary for the reserved SourcePlane V3 family.
//!
//! This module owns only facts that the portable SourcePlane runtime cannot
//! obtain itself: real `AccountInfo` metadata, canonical PDA derivation under
//! the executing program, and canonical Clock-sysvar decoding. Semantic
//! bodies, account identities, release authentication, and policy joins are
//! consumed directly from `clutch-source-plane-v3-runtime`; there is no SBF-
//! local Source release, Clock policy, page, result, or handoff DTO.
//!
//! SourceSeries 77/v2 actions 1 through 12 remain capability-disabled. These
//! functions are the typed trust-boundary seam a later activation must call;
//! their existence does not make any instruction executable.

use clutch_liveness::{
    runtime_adapter_v1::{
        RuntimeAdapterErrorV1, RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
        RuntimeTransitionActionV1, RuntimeTransitionIntentV1,
    },
    runtime_v1::RuntimeCompartmentKindV1,
    Id as LivenessId,
};
use clutch_product_series::{CompiledSourceOccurrenceV3, FixedCodec as ProductFixedCodec};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, RawPageV3, SourceHeadV3, StatisticKeyV3,
    StatisticResultV3, SummaryProgramV3, WindowSpecV3,
};
use clutch_source_plane_v3_adapter::{PdaRecipeV3, MAX_PDA_SEEDS};
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_boundary, authenticate_evaluation_authority,
    authenticate_open_raw_page_account, authenticate_raw_page_account,
    authenticate_reopen_lineage_account, authenticate_source_head_account,
    authenticate_source_release_account, authenticate_source_route,
    authenticate_source_work_receipt_account, authenticate_statistic_result,
    authenticate_statistic_result_absence, authenticate_statistic_result_account,
    authenticate_window_seal_account, authenticate_window_work_account, decode_runtime_account,
    join_source_occurrence, AdapterInvocationV1, AuthenticatedBoundaryV1,
    AuthenticatedClockBucketV1, AuthenticatedEvaluationV1, AuthenticatedOpenRawPageV1,
    AuthenticatedRawPageV1, AuthenticatedReopenLineageV1, AuthenticatedSourceHeadV1,
    AuthenticatedSourceReleaseV1, AuthenticatedSourceRouteV1, AuthenticatedSourceWorkReceiptV1,
    AuthenticatedStatisticResultAbsenceV1, AuthenticatedStatisticResultAccountV1,
    AuthenticatedWindowEvidenceV1, AuthenticatedWindowSealAccountV1, AuthenticatedWindowWorkV1,
    ClockSnapshotV1, EvaluationReleaseBindingV1, FailurePolicySourceHandoffV1, LineageAccessV1,
    OccurrenceDispositionV1, OccurrenceSourceReceiptV1, ParserOutputV1, ReopenLineageV1,
    RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey, SourceReceiptDispositionV1,
    SourceReleaseManifestV1, SourceWorkReceiptAccessV1, SourceWorkReceiptAccountV1,
    SourceWorkScheduleBindingV1, SuccessfulEvaluationHandoffV1, OPEN_RAW_PAGE_ACCOUNT_TAG,
    RAW_PAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_VERSION,
    RUNTIME_ACCOUNT_GLOBAL_VERSION, SOURCE_HEAD_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_TAG,
    SOURCE_RELEASE_ACCOUNT_VERSION, SOURCE_WORK_RECEIPT_ACCOUNT_TAG,
    SOURCE_WORK_RECEIPT_ACCOUNT_VERSION, STATISTIC_RESULT_ACCOUNT_TAG, WINDOW_SEAL_ACCOUNT_TAG,
    WINDOW_WORK_ACCOUNT_TAG,
};
use solana_account_info::AccountInfo;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::accounts::Outcome;
use crate::error::{ClutchError, Refusal};
use crate::source_identity::CLOCK_SYSVAR_ID;
use crate::source_v2::auth::{decode_clock_view, AccountViewV2, AuthV2Error};

const INSTRUCTION_DATA_DOMAIN: &[u8] = b"dragons-clutch/sbf-instruction-data/v1";
const ACCOUNT_VECTOR_DOMAIN: &[u8] = b"dragons-clutch/sbf-account-vector/v1";
const ACCOUNT_VECTOR_ENTRY_BYTES: usize = 105;
/// Maximum ordered accounts admitted to one reviewed Source parser invocation.
pub const MAX_SOURCE_PARSER_ACCOUNTS: usize = 16;

/// Route one centrally allocated SourcePlane action to its reserved-disabled handler.
///
/// This boundary is deliberately account-free. The dispatcher calls it before
/// account inspection, so merely allocating actions 1 through 12 cannot make a
/// partially implemented transition executable. The exhaustive match also
/// prevents a newly allocated Source action from inheriting this refusal
/// without an explicit review here.
#[inline(never)]
pub fn process_reserved_disabled(
    action: clutch_solana_layout::registry::SourceSeriesAction,
) -> Outcome<()> {
    use clutch_solana_layout::registry::SourceSeriesAction;

    match action {
        SourceSeriesAction::RegisterRelease
        | SourceSeriesAction::InitializeHead
        | SourceSeriesAction::OpenRawPage
        | SourceSeriesAction::IngestBoundaryBatch
        | SourceSeriesAction::SealRawPage
        | SourceSeriesAction::InitializeWindowWork
        | SourceSeriesAction::FoldWindowPages
        | SourceSeriesAction::SealWindow
        | SourceSeriesAction::EvaluateStatistic
        | SourceSeriesAction::EmitFailureHandoff
        | SourceSeriesAction::ReopenGeneration
        | SourceSeriesAction::CloseGeneration => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

const _: () = assert!(
    SOURCE_WORK_RECEIPT_ACCOUNT_TAG
        == clutch_solana_layout::registry::SOURCE_V3_WORK_RECEIPT_ACCOUNT_TAG
);
const _: () = assert!(
    SOURCE_RELEASE_ACCOUNT_TAG == clutch_solana_layout::registry::SOURCE_V3_RELEASE_ACCOUNT_TAG
);
const _: () = assert!(
    SOURCE_RELEASE_ACCOUNT_VERSION
        == clutch_solana_layout::registry::SOURCE_V3_RELEASE_ACCOUNT_VERSION
);
const _: () =
    assert!(SOURCE_HEAD_ACCOUNT_TAG == clutch_solana_layout::registry::SOURCE_V3_HEAD_ACCOUNT_TAG);
const _: () = assert!(
    REOPEN_LINEAGE_ACCOUNT_TAG
        == clutch_solana_layout::registry::SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_TAG
);
const _: () = assert!(
    REOPEN_LINEAGE_ACCOUNT_VERSION
        == clutch_solana_layout::registry::SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION
);
const _: () = assert!(
    OPEN_RAW_PAGE_ACCOUNT_TAG
        == clutch_solana_layout::registry::SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_TAG
);
const _: () =
    assert!(RAW_PAGE_ACCOUNT_TAG == clutch_solana_layout::registry::SOURCE_V3_RAW_PAGE_ACCOUNT_TAG);
const _: () = assert!(
    WINDOW_WORK_ACCOUNT_TAG == clutch_solana_layout::registry::SOURCE_V3_WINDOW_WORK_ACCOUNT_TAG
);
const _: () = assert!(
    WINDOW_SEAL_ACCOUNT_TAG == clutch_solana_layout::registry::SOURCE_V3_WINDOW_SEAL_ACCOUNT_TAG
);
const _: () = assert!(
    STATISTIC_RESULT_ACCOUNT_TAG
        == clutch_solana_layout::registry::SOURCE_V3_STATISTIC_RESULT_ACCOUNT_TAG
);
const _: () = assert!(
    RUNTIME_ACCOUNT_GLOBAL_VERSION
        == clutch_solana_layout::registry::SOURCE_V3_HEAD_ACCOUNT_VERSION
        && RUNTIME_ACCOUNT_GLOBAL_VERSION
            == clutch_solana_layout::registry::SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_VERSION
        && RUNTIME_ACCOUNT_GLOBAL_VERSION
            == clutch_solana_layout::registry::SOURCE_V3_RAW_PAGE_ACCOUNT_VERSION
        && RUNTIME_ACCOUNT_GLOBAL_VERSION
            == clutch_solana_layout::registry::SOURCE_V3_WINDOW_WORK_ACCOUNT_VERSION
        && RUNTIME_ACCOUNT_GLOBAL_VERSION
            == clutch_solana_layout::registry::SOURCE_V3_WINDOW_SEAL_ACCOUNT_VERSION
        && RUNTIME_ACCOUNT_GLOBAL_VERSION
            == clutch_solana_layout::registry::SOURCE_V3_STATISTIC_RESULT_ACCOUNT_VERSION
);
const _: () = assert!(
    SOURCE_WORK_RECEIPT_ACCOUNT_VERSION
        == clutch_solana_layout::registry::SOURCE_V3_WORK_RECEIPT_ACCOUNT_VERSION
);

/// Fail-closed SourcePlane V3 refusal at the real SBF boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceV3SbfError {
    /// An account data borrow conflicted with another live borrow.
    AccountBorrow,
    /// The canonical Clock account was not supplied read-only.
    WrongClockAccount,
    /// The canonical Clock decoder or signed-to-unsigned projection refused.
    Clock(AuthV2Error),
    /// The portable SourcePlane account/runtime contract refused.
    Runtime(clutch_source_plane_v3_runtime::Error),
    /// The canonical SourcePlane PDA recipe refused.
    Pda(clutch_source_plane_v3_adapter::Error),
    /// A reviewed parser invocation failed in CPI.
    ParserCpi,
    /// The parser did not return exact data under its own program identity.
    ParserReturn,
    /// The invoked parser program differed from the authenticated release.
    WrongParserProgram,
    /// The parser invocation account vector exceeded its reviewed fixed bound.
    ParserAccountCount,
    /// The ordered parser accounts omitted, aliased, or widened feed/config roles.
    ParserAccountVector,
    /// A reviewed statistic evaluator invocation failed in CPI.
    EvaluatorCpi,
    /// The evaluator did not return one exact StatisticResult under its own identity.
    EvaluatorReturn,
    /// The CPI program differed from the authenticated evaluator release.
    WrongEvaluatorProgram,
    /// An authenticated receipt disposition did not match the requested liveness action.
    WrongReceiptDisposition,
    /// The single-custody liveness runtime refused the projected intent.
    Liveness(RuntimeAdapterErrorV1),
}

impl From<clutch_source_plane_v3_runtime::Error> for SourceV3SbfError {
    fn from(value: clutch_source_plane_v3_runtime::Error) -> Self {
        Self::Runtime(value)
    }
}

impl From<clutch_source_plane_v3::Error> for SourceV3SbfError {
    fn from(value: clutch_source_plane_v3::Error) -> Self {
        Self::Runtime(clutch_source_plane_v3_runtime::Error::Core(value))
    }
}

impl From<clutch_source_plane_v3_adapter::Error> for SourceV3SbfError {
    fn from(value: clutch_source_plane_v3_adapter::Error) -> Self {
        Self::Pda(value)
    }
}

impl From<RuntimeAdapterErrorV1> for SourceV3SbfError {
    fn from(value: RuntimeAdapterErrorV1) -> Self {
        Self::Liveness(value)
    }
}

impl From<SourceV3SbfError> for Refusal {
    fn from(value: SourceV3SbfError) -> Self {
        let error = match value {
            SourceV3SbfError::AccountBorrow => ClutchError::AccountBorrowFailed,
            SourceV3SbfError::WrongClockAccount => ClutchError::MismatchedState,
            SourceV3SbfError::Clock(_) => ClutchError::SourceAdmissionFailed,
            SourceV3SbfError::Runtime(
                clutch_source_plane_v3_runtime::Error::ArithmeticOverflow,
            ) => ClutchError::Arithmetic,
            SourceV3SbfError::Runtime(clutch_source_plane_v3_runtime::Error::InvalidCodec) => {
                ClutchError::NonCanonical
            }
            SourceV3SbfError::Runtime(_) => ClutchError::SourceAdmissionFailed,
            SourceV3SbfError::Pda(_) => ClutchError::WrongPda,
            SourceV3SbfError::ParserCpi
            | SourceV3SbfError::ParserReturn
            | SourceV3SbfError::WrongParserProgram
            | SourceV3SbfError::ParserAccountCount
            | SourceV3SbfError::ParserAccountVector
            | SourceV3SbfError::EvaluatorCpi
            | SourceV3SbfError::EvaluatorReturn
            | SourceV3SbfError::WrongEvaluatorProgram => ClutchError::SourceAdmissionFailed,
            SourceV3SbfError::WrongReceiptDisposition | SourceV3SbfError::Liveness(_) => {
                ClutchError::MismatchedState
            }
        };
        Self::Adapter(error)
    }
}

/// SourcePlane V3 result at the SBF trust boundary.
pub type SourceV3SbfResult<T> = core::result::Result<T, SourceV3SbfError>;

/// Convert one runtime address without reinterpreting it as a content digest.
pub fn runtime_key(key: &Pubkey) -> RuntimeKey {
    RuntimeKey::from_bytes(key.to_bytes())
}

/// Invoke Solana's canonical PDA derivation for one portable Source recipe.
pub fn derive_runtime_pda(
    program_id: &Pubkey,
    recipe: &PdaRecipeV3,
) -> SourceV3SbfResult<RuntimeDerivedPdaV1> {
    recipe.validate()?;
    let mut seeds: [&[u8]; MAX_PDA_SEEDS] = [&[]; MAX_PDA_SEEDS];
    let count = usize::from(recipe.seed_count());
    let mut index = 0_usize;
    while index < count {
        seeds[index] = recipe.seed(index)?;
        index += 1;
    }
    let (address, bump) = crate::seeds::find(program_id, &seeds[..count]);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe.id()?,
        address: runtime_key(&address),
        bump,
    })
}

/// Authenticate the exact immutable Source release account under this program.
pub fn authenticate_release(
    program_id: &Pubkey,
    release_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSourceReleaseV1> {
    let data = release_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let manifest = SourceReleaseManifestV1::decode(&data)?;
    let recipe = PdaRecipeV3::source_release(manifest.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_source_release_account(
        runtime_key(program_id),
        runtime_account_view(release_account, &data),
        derived,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged persistent reopen-lineage PDA.
pub fn authenticate_lineage(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    lineage_account: &AccountInfo<'_>,
    access: LineageAccessV1,
) -> SourceV3SbfResult<AuthenticatedReopenLineageV1> {
    let data = lineage_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let lineage = ReopenLineageV1::decode(&data)?;
    let recipe = PdaRecipeV3::reopen_lineage(lineage.recipe_id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_reopen_lineage_account(
        route,
        runtime_account_view(lineage_account, &data),
        derived,
        access,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged mutable SourceHead PDA and lineage.
pub fn authenticate_head(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head_account: &AccountInfo<'_>,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedSourceHeadV1> {
    let data = head_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let (_, head) = decode_runtime_account::<SourceHeadV3>(&data, route.neutral_sink())?;
    let recipe = PdaRecipeV3::source_head(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        head.repair_generation,
    )?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_source_head_account(
        route,
        runtime_account_view(head_account, &data),
        derived,
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged mutable OpenRawPage PDA and lineage.
pub fn authenticate_open_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    open_account: &AccountInfo<'_>,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedOpenRawPageV1> {
    let data = open_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let (_, open) = decode_runtime_account::<OpenRawPageV3>(&data, route.neutral_sink())?;
    let recipe = PdaRecipeV3::open_raw_page(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        open.repair_generation,
        open.page_index,
    )?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_open_raw_page_account(
        route,
        head,
        runtime_account_view(open_account, &data),
        derived,
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged immutable RawPage content PDA.
pub fn authenticate_raw_page(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    page_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedRawPageV1> {
    let data = page_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let (_, page) = decode_runtime_account::<RawPageV3>(&data, route.neutral_sink())?;
    let recipe = PdaRecipeV3::raw_page(route.source_plane_contract_id(), page.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_raw_page_account(route, runtime_account_view(page_account, &data), derived)
        .map_err(Into::into)
}

/// Authenticate one globally tagged mutable WindowWork PDA and lineage.
pub fn authenticate_window_work(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    work_account: &AccountInfo<'_>,
    window: &WindowSpecV3,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedWindowWorkV1> {
    let data = work_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let recipe = PdaRecipeV3::window_work(window.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_window_work_account(
        route,
        runtime_account_view(work_account, &data),
        derived,
        window,
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged immutable WindowSeal PDA.
pub fn authenticate_window_seal(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    seal_account: &AccountInfo<'_>,
    window: &WindowSpecV3,
) -> SourceV3SbfResult<AuthenticatedWindowSealAccountV1> {
    let data = seal_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let recipe = PdaRecipeV3::window_seal(window.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_window_seal_account(
        route,
        runtime_account_view(seal_account, &data),
        derived,
        window,
    )
    .map_err(Into::into)
}

/// Authenticate one globally tagged immutable StatisticResult PDA.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_result_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    result_account: &AccountInfo<'_>,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    evidence: AuthenticatedWindowEvidenceV1,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedStatisticResultAccountV1> {
    let data = result_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let recipe = PdaRecipeV3::statistic_result(key.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_statistic_result_account(
        route,
        runtime_account_view(result_account, &data),
        derived,
        window,
        key,
        summary,
        evidence,
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate the exact unallocated StatisticResult PDA and its never-opened lineage.
pub fn authenticate_result_absence(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    result_account: &AccountInfo<'_>,
    key: &StatisticKeyV3,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedStatisticResultAbsenceV1> {
    let data = result_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let recipe = PdaRecipeV3::statistic_result(key.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_statistic_result_absence(
        route,
        key,
        runtime_account_view(result_account, &data),
        derived,
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate release account, both deployments, config, and SourceSpec bytes.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_route(
    program_id: &Pubkey,
    release_account: &AccountInfo<'_>,
    adapter_program: &AccountInfo<'_>,
    adapter_programdata: &AccountInfo<'_>,
    parser_program: &AccountInfo<'_>,
    parser_programdata: &AccountInfo<'_>,
    parser_config: &AccountInfo<'_>,
    source_spec_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSourceRouteV1> {
    let release = authenticate_release(program_id, release_account)?;
    let adapter_program_data = adapter_program
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let adapter_programdata_data = adapter_programdata
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let parser_program_data = parser_program
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let parser_programdata_data = parser_programdata
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let parser_config_data = parser_config
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let source_spec_data = source_spec_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    authenticate_source_route(
        release,
        runtime_account_view(adapter_program, &adapter_program_data),
        runtime_account_view(adapter_programdata, &adapter_programdata_data),
        runtime_account_view(parser_program, &parser_program_data),
        runtime_account_view(parser_programdata, &parser_programdata_data),
        runtime_account_view(parser_config, &parser_config_data),
        runtime_account_view(source_spec_account, &source_spec_data),
    )
    .map_err(Into::into)
}

/// Derive a policy-bound current bucket from the canonical Solana Clock account.
pub fn authenticate_clock_bucket(
    release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedClockBucketV1> {
    if clock_account.key.to_bytes() != CLOCK_SYSVAR_ID
        || clock_account.is_signer
        || clock_account.is_writable
    {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    let data = clock_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let clock = decode_clock_view(AccountViewV2::new(
        clock_account.key.to_bytes(),
        clock_account.owner.to_bytes(),
        clock_account.executable,
        &data,
    ))
    .map_err(SourceV3SbfError::Clock)?;
    let unix_timestamp =
        u64::try_from(clock.unix_timestamp).map_err(|_| SourceV3SbfError::WrongClockAccount)?;
    AuthenticatedClockBucketV1::from_snapshot(
        &release.clock_policy(),
        ClockSnapshotV1 {
            slot: clock.slot,
            unix_timestamp,
        },
    )
    .map_err(Into::into)
}

/// Authenticate Product's exact occurrence PDA/body and join it to Source semantics.
pub fn authenticate_occurrence(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    occurrence_account: &AccountInfo<'_>,
    disposition: OccurrenceDispositionV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
) -> SourceV3SbfResult<OccurrenceSourceReceiptV1> {
    let data = occurrence_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let occurrence = CompiledSourceOccurrenceV3::decode(&data).map_err(|_| {
        SourceV3SbfError::Runtime(clutch_source_plane_v3_runtime::Error::InvalidCodec)
    })?;
    let occurrence_record_id = ContentId::from_bytes(
        occurrence
            .id()
            .map_err(|_| {
                SourceV3SbfError::Runtime(clutch_source_plane_v3_runtime::Error::InvalidCodec)
            })?
            .bytes(),
    );
    let (address, bump) =
        crate::seeds::source_occurrence_pda(program_id, &occurrence_record_id.bytes());
    let derived = RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: occurrence_record_id,
        address: runtime_key(&address),
        bump,
    };
    join_source_occurrence(
        route,
        runtime_account_view(occurrence_account, &data),
        derived,
        disposition,
        window,
        key,
    )
    .map_err(Into::into)
}

/// Authenticate the exact globally tagged Source work/terminal receipt PDA.
pub fn authenticate_work_receipt(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    receipt_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSourceWorkReceiptV1> {
    let data = receipt_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let receipt = SourceWorkReceiptAccountV1::decode(&data)?;
    let recipe = PdaRecipeV3::source_work_receipt(receipt.receipt_slot_id(route, schedule)?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_source_work_receipt_account(
        route,
        schedule,
        runtime_account_view(receipt_account, &data),
        derived,
        SourceWorkReceiptAccessV1::ExistingReadOnly,
    )
    .map_err(Into::into)
}

/// Project only an authenticated persisted Source receipt into liveness.
///
/// The liveness runtime remains the sole lamport custodian and performs the
/// keeper debit/refund. This projection contains no transfer instruction and
/// therefore cannot create a second payment path.
pub fn project_liveness_receipt(
    authenticated: AuthenticatedSourceWorkReceiptV1,
) -> RuntimeReceiptObservationV1 {
    let receipt = authenticated.receipt();
    RuntimeReceiptObservationV1 {
        receipt_account_id: LivenessId::from_bytes(authenticated.account().bytes()),
        receipt_account_owner_program_id: LivenessId::from_bytes(
            receipt.receipt_account_owner_program_id().bytes(),
        ),
        receipt_id: LivenessId::from_bytes(receipt.receipt_id().bytes()),
        receipt_kind: match receipt.disposition() {
            SourceReceiptDispositionV1::Work => RuntimeReceiptKindV1::WorkCompleted,
            SourceReceiptDispositionV1::TerminalSuccess => RuntimeReceiptKindV1::TerminalSuccess,
            SourceReceiptDispositionV1::TerminalFailure => RuntimeReceiptKindV1::TerminalFailure,
        },
        compartment_kind: RuntimeCompartmentKindV1::Source,
        semantic_owner: LivenessId::from_bytes(receipt.source_compartment_owner().bytes()),
        lifecycle_id: LivenessId::from_bytes(receipt.lifecycle_id().bytes()),
        quote_schedule_id: LivenessId::from_bytes(receipt.source_work_schedule_id().bytes()),
        generation: receipt.generation(),
        call_ordinal: receipt.call_ordinal(),
        call_ceiling_lamports: receipt.call_ceiling_lamports(),
    }
}

/// Construct the sole liveness spend intent for authenticated Source work.
pub fn project_liveness_work_intent(
    authenticated: AuthenticatedSourceWorkReceiptV1,
    keeper: &Pubkey,
    keeper_payment_lamports: u64,
) -> SourceV3SbfResult<RuntimeTransitionIntentV1> {
    let receipt = authenticated.receipt();
    if receipt.disposition() != SourceReceiptDispositionV1::Work {
        return Err(SourceV3SbfError::WrongReceiptDisposition);
    }
    let schedule = authenticated.schedule();
    let intent = RuntimeTransitionIntentV1 {
        action: RuntimeTransitionActionV1::SpendWork,
        kind: RuntimeCompartmentKindV1::Source,
        policy_id: LivenessId::from_bytes(schedule.liveness_policy_id().bytes()),
        lifecycle_id: LivenessId::from_bytes(receipt.lifecycle_id().bytes()),
        account_id: LivenessId::from_bytes(receipt.source_compartment_account().bytes()),
        semantic_owner: LivenessId::from_bytes(receipt.source_compartment_owner().bytes()),
        quote_schedule_id: LivenessId::from_bytes(receipt.source_work_schedule_id().bytes()),
        receipt_id: LivenessId::from_bytes(receipt.receipt_id().bytes()),
        keeper: LivenessId::from_bytes(keeper.to_bytes()),
        generation: receipt.generation(),
        call_ordinal: receipt.call_ordinal(),
        call_ceiling_lamports: receipt.call_ceiling_lamports(),
        keeper_payment_lamports,
        flags: 0,
    };
    intent.validate()?;
    Ok(intent)
}

/// Construct the sole liveness terminal intent for an authenticated receipt.
pub fn project_liveness_terminal_intent(
    authenticated: AuthenticatedSourceWorkReceiptV1,
) -> SourceV3SbfResult<RuntimeTransitionIntentV1> {
    let receipt = authenticated.receipt();
    let action = match receipt.disposition() {
        SourceReceiptDispositionV1::TerminalSuccess => RuntimeTransitionActionV1::CloseSuccess,
        SourceReceiptDispositionV1::TerminalFailure => RuntimeTransitionActionV1::CloseFailure,
        SourceReceiptDispositionV1::Work => return Err(SourceV3SbfError::WrongReceiptDisposition),
    };
    let schedule = authenticated.schedule();
    let intent = RuntimeTransitionIntentV1 {
        action,
        kind: RuntimeCompartmentKindV1::Source,
        policy_id: LivenessId::from_bytes(schedule.liveness_policy_id().bytes()),
        lifecycle_id: LivenessId::from_bytes(receipt.lifecycle_id().bytes()),
        account_id: LivenessId::from_bytes(receipt.source_compartment_account().bytes()),
        semantic_owner: LivenessId::from_bytes(receipt.source_compartment_owner().bytes()),
        quote_schedule_id: LivenessId::from_bytes(receipt.source_work_schedule_id().bytes()),
        receipt_id: LivenessId::from_bytes(receipt.receipt_id().bytes()),
        keeper: LivenessId::ZERO,
        generation: receipt.generation(),
        call_ordinal: 0,
        call_ceiling_lamports: 0,
        keeper_payment_lamports: 0,
        flags: 0,
    };
    intent.validate()?;
    Ok(intent)
}

/// Project exact mature result absence into the failure/recovery boundary.
pub fn primary_maturity_handoff(
    route: AuthenticatedSourceRouteV1,
    failure_policy_binding_id: ContentId,
    occurrence: OccurrenceSourceReceiptV1,
    clock: AuthenticatedClockBucketV1,
    window: &WindowSpecV3,
    absence: AuthenticatedStatisticResultAbsenceV1,
) -> SourceV3SbfResult<FailurePolicySourceHandoffV1> {
    if clock.policy_id() != route.clock_policy_id() {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    FailurePolicySourceHandoffV1::primary_maturity_without_resolution(
        failure_policy_binding_id,
        occurrence,
        &route.clock_policy(),
        clock.snapshot(),
        window,
        absence,
    )
    .map_err(Into::into)
}

/// Project an exact stable evaluator refusal into the failure/recovery boundary.
#[allow(clippy::too_many_arguments)]
pub fn source_refusal_handoff(
    route: AuthenticatedSourceRouteV1,
    failure_policy_binding_id: ContentId,
    occurrence: OccurrenceSourceReceiptV1,
    clock: AuthenticatedClockBucketV1,
    window: &WindowSpecV3,
    evidence: AuthenticatedWindowEvidenceV1,
    result_account: AuthenticatedStatisticResultAccountV1,
) -> SourceV3SbfResult<FailurePolicySourceHandoffV1> {
    if clock.policy_id() != route.clock_policy_id() {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    FailurePolicySourceHandoffV1::source_evaluation_refused(
        failure_policy_binding_id,
        occurrence,
        &route.clock_policy(),
        clock.snapshot(),
        window,
        evidence,
        result_account,
    )
    .map_err(Into::into)
}

/// Project successful source evaluation for downstream relation-policy review.
#[allow(clippy::too_many_arguments)]
pub fn successful_evaluation_handoff(
    route: AuthenticatedSourceRouteV1,
    failure_policy_binding_id: ContentId,
    occurrence: OccurrenceSourceReceiptV1,
    clock: AuthenticatedClockBucketV1,
    window: &WindowSpecV3,
    evidence: AuthenticatedWindowEvidenceV1,
    result_account: AuthenticatedStatisticResultAccountV1,
) -> SourceV3SbfResult<SuccessfulEvaluationHandoffV1> {
    if clock.policy_id() != route.clock_policy_id() {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    SuccessfulEvaluationHandoffV1::at_maturity(
        failure_policy_binding_id,
        occurrence,
        &route.clock_policy(),
        clock.snapshot(),
        window,
        evidence,
        result_account,
    )
    .map_err(Into::into)
}

/// Invoke one exact reviewed evaluator release and authenticate its canonical
/// StatisticResult against the already-authenticated Window evidence.
#[allow(clippy::too_many_arguments)]
pub fn invoke_statistic_evaluator(
    binding: EvaluationReleaseBindingV1,
    summary: SummaryProgramV3,
    evaluator_program: &AccountInfo<'_>,
    evaluator_programdata: &AccountInfo<'_>,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    instruction: &Instruction,
    invocation_accounts: &[AccountInfo<'_>],
) -> SourceV3SbfResult<AuthenticatedEvaluationV1> {
    let program_data = evaluator_program
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let programdata_data = evaluator_programdata
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let authority = authenticate_evaluation_authority(
        binding,
        summary,
        runtime_account_view(evaluator_program, &program_data),
        runtime_account_view(evaluator_programdata, &programdata_data),
    )?;
    drop(programdata_data);
    drop(program_data);
    if runtime_key(&instruction.program_id) != authority.evaluator_program() {
        return Err(SourceV3SbfError::WrongEvaluatorProgram);
    }
    if invocation_accounts.len() > MAX_SOURCE_PARSER_ACCOUNTS {
        return Err(SourceV3SbfError::ParserAccountCount);
    }
    solana_cpi::invoke(instruction, invocation_accounts)
        .map_err(|_| SourceV3SbfError::EvaluatorCpi)?;
    let (return_program, return_bytes) =
        solana_cpi::get_return_data().ok_or(SourceV3SbfError::EvaluatorReturn)?;
    if runtime_key(&return_program) != authority.evaluator_program() {
        return Err(SourceV3SbfError::EvaluatorReturn);
    }
    let result = StatisticResultV3::decode(&return_bytes)?;
    let invocation = AdapterInvocationV1 {
        invoked_program: authority.evaluator_program(),
        return_data_program: runtime_key(&return_program),
        return_data_id: result.id()?,
        instruction_data_id: hash_parts(INSTRUCTION_DATA_DOMAIN, &instruction.data),
        account_vector_id: account_vector_id(invocation_accounts)?,
    };
    authenticate_statistic_result(
        authority,
        clock,
        window,
        key,
        evidence,
        &return_bytes,
        invocation,
    )
    .map_err(Into::into)
}

/// Invoke the exact reviewed parser and authenticate its immediate return data.
///
/// `expected_bucket` remains state-owned: a caller cannot append this receipt
/// unless the authenticated open-page cursor independently requires the same
/// bucket. This function authenticates the CPI, feed bytes, Clock window, and
/// parser semantics without inventing a second parser-output representation.
#[allow(clippy::too_many_arguments)]
pub fn invoke_parser_boundary(
    route: AuthenticatedSourceRouteV1,
    clock: AuthenticatedClockBucketV1,
    feed: &AccountInfo<'_>,
    expected_bucket: u64,
    repair_generation: u64,
    instruction: &Instruction,
    invocation_accounts: &[AccountInfo<'_>],
) -> SourceV3SbfResult<AuthenticatedBoundaryV1> {
    if runtime_key(&instruction.program_id) != route.parser_program() {
        return Err(SourceV3SbfError::WrongParserProgram);
    }
    if clock.policy_id() != route.clock_policy_id() {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    if invocation_accounts.len() > MAX_SOURCE_PARSER_ACCOUNTS {
        return Err(SourceV3SbfError::ParserAccountCount);
    }
    validate_parser_account_vector(route, feed, invocation_accounts)?;
    solana_cpi::invoke(instruction, invocation_accounts)
        .map_err(|_| SourceV3SbfError::ParserCpi)?;
    let (return_program, return_bytes) =
        solana_cpi::get_return_data().ok_or(SourceV3SbfError::ParserReturn)?;
    if runtime_key(&return_program) != route.parser_program() {
        return Err(SourceV3SbfError::ParserReturn);
    }
    let parser_output = ParserOutputV1::decode(&return_bytes)?;
    let invocation = AdapterInvocationV1 {
        invoked_program: route.parser_program(),
        return_data_program: runtime_key(&return_program),
        return_data_id: parser_output.id()?,
        instruction_data_id: hash_parts(INSTRUCTION_DATA_DOMAIN, &instruction.data),
        account_vector_id: account_vector_id(invocation_accounts)?,
    };
    let feed_data = feed
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    authenticate_boundary(
        route,
        &route.clock_policy(),
        clock.snapshot(),
        runtime_account_view(feed, &feed_data),
        expected_bucket,
        repair_generation,
        parser_output,
        invocation,
    )
    .map_err(Into::into)
}

#[inline(never)]
fn account_vector_id(accounts: &[AccountInfo<'_>]) -> SourceV3SbfResult<ContentId> {
    if accounts.len() > MAX_SOURCE_PARSER_ACCOUNTS {
        return Err(SourceV3SbfError::ParserAccountCount);
    }
    let mut body =
        std::boxed::Box::new([0_u8; 8 + MAX_SOURCE_PARSER_ACCOUNTS * ACCOUNT_VECTOR_ENTRY_BYTES]);
    body[..8].copy_from_slice(
        &u64::try_from(accounts.len())
            .map_err(|_| SourceV3SbfError::ParserAccountCount)?
            .to_le_bytes(),
    );
    let mut index = 0_usize;
    while index < accounts.len() {
        let account = &accounts[index];
        let data = account
            .try_borrow_data()
            .map_err(|_| SourceV3SbfError::AccountBorrow)?;
        let at = 8 + index * ACCOUNT_VECTOR_ENTRY_BYTES;
        body[at..at + 32].copy_from_slice(&account.key.to_bytes());
        body[at + 32..at + 64].copy_from_slice(&account.owner.to_bytes());
        body[at + 64..at + 72].copy_from_slice(&account.lamports().to_le_bytes());
        body[at + 72] = u8::from(account.is_signer)
            | (u8::from(account.is_writable) << 1)
            | (u8::from(account.executable) << 2);
        body[at + 73..at + 105]
            .copy_from_slice(&account_data_id(runtime_key(account.key), &data)?.bytes());
        index += 1;
    }
    Ok(hash_parts(ACCOUNT_VECTOR_DOMAIN, &body[..]))
}

fn validate_parser_account_vector(
    route: AuthenticatedSourceRouteV1,
    feed: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> SourceV3SbfResult<()> {
    if runtime_key(feed.key) != route.feed()
        || feed.is_signer
        || feed.is_writable
        || feed.executable
    {
        return Err(SourceV3SbfError::ParserAccountVector);
    }
    let mut feed_count = 0_u8;
    let mut config_count = 0_u8;
    let mut left = 0_usize;
    while left < accounts.len() {
        let account = &accounts[left];
        if account.key == feed.key {
            feed_count = feed_count
                .checked_add(1)
                .ok_or(SourceV3SbfError::ParserAccountVector)?;
        }
        if runtime_key(account.key) == route.parser_config() {
            if account.is_signer || account.is_writable || account.executable {
                return Err(SourceV3SbfError::ParserAccountVector);
            }
            config_count = config_count
                .checked_add(1)
                .ok_or(SourceV3SbfError::ParserAccountVector)?;
        }
        let mut right = left + 1;
        while right < accounts.len() {
            if account.key == accounts[right].key {
                return Err(SourceV3SbfError::ParserAccountVector);
            }
            right += 1;
        }
        left += 1;
    }
    if feed_count != 1 || config_count != 1 {
        return Err(SourceV3SbfError::ParserAccountVector);
    }
    Ok(())
}

fn hash_parts(domain: &[u8], body: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[domain, body]).to_bytes())
}

fn runtime_account_view<'a>(
    account: &'a AccountInfo<'_>,
    data: &'a [u8],
) -> RuntimeAccountViewV1<'a> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.key),
        owner: runtime_key(account.owner),
        lamports: account.lamports(),
        executable: account.executable,
        writable: account.is_writable,
        signer: account.is_signer,
        data,
    }
}
