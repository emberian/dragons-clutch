//! Solana runtime boundary for the reserved SourcePlane V3 family.
//!
//! This module owns only facts that the portable SourcePlane runtime cannot
//! obtain itself: real `AccountInfo` metadata, canonical PDA derivation under
//! the executing program, and canonical Clock-sysvar decoding. Semantic
//! bodies, account identities, release authentication, and policy joins are
//! consumed directly from `clutch-source-plane-v3-runtime`; there is no SBF-
//! local Source release, Clock policy, page, result, or handoff DTO.
//!
//! SourceSeries 77/v2 reserves an artifact-authenticated release registry and
//! the complete Source lifecycle. Its current implementation includes the
//! Product-owned founding request/policy producer, fully prepaid PDA custody,
//! hostile ingest/evaluate/handoff reconstruction, Failure ResolutionV5
//! terminal join, deterministic reopen, result close, and private Product
//! retirement drain. Central dispatch remains the sole all-or-none capability
//! owner; code presence by itself is not capability.

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
    StatisticResultV3, SummaryProgramV3, WindowSealV3, WindowSpecV3,
};
use clutch_source_plane_v3_adapter::{PdaRecipeV3, MAX_PDA_SEEDS};
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_boundary, authenticate_evaluation_authority,
    authenticate_persisted_source_policy_handoff,
    authenticate_persisted_statistic_result_account,
    authenticate_persisted_statistic_result_account_for_resolution,
    authenticate_persisted_window_evidence,
    authenticate_open_raw_page_account, authenticate_raw_page_account,
    authenticate_receiver_route_v2, authenticate_reopen_lineage_account,
    authenticate_source_generation_request, authenticate_source_head_account,
    authenticate_source_reopen_generation_request,
    authenticate_source_reopen_generation_request_before_close as authenticate_source_reopen_generation_request_before_close_runtime,
    authenticate_source_release_account, authenticate_source_route,
    authenticate_source_policy_handoff_record,
    authenticate_source_work_receipt_account, authenticate_statistic_result,
    authenticate_statistic_result_absence, authenticate_statistic_result_account,
    authenticate_window_seal_account, authenticate_window_work_account, decode_runtime_account,
    join_source_occurrence, join_source_occurrence_window, AdapterInvocationV1,
    AuthenticatedBoundaryV1,
    AuthenticatedClockBucketV1, AuthenticatedEvaluationV1, AuthenticatedOpenRawPageV1,
    AuthenticatedRawPageV1, AuthenticatedReceiverRouteV2, AuthenticatedReopenLineageV1,
    AuthenticatedSourceGenerationV1, AuthenticatedSourceHeadV1,
    AuthenticatedSourcePolicyHandoffRecordV1,
    AuthenticatedSourceReleaseV1, AuthenticatedSourceReopenGenerationV1,
    AuthenticatedSourceReopenPrecloseV1,
    AuthenticatedSourceRouteV1, AuthenticatedSourceWorkReceiptV1,
    AuthenticatedStatisticResultAbsenceV1, AuthenticatedStatisticResultAccountV1,
    AuthenticatedWindowEvidenceV1, AuthenticatedWindowSealAccountV1, AuthenticatedWindowWorkV1,
    ClockSnapshotV1, DeploymentBindingV1, EvaluationReleaseBindingV1,
    FailurePolicySourceHandoffV1, LineageAccessV1, OccurrenceDispositionV1,
    OccurrenceSourceReceiptV1, OccurrenceWindowReceiptV1,
    ParserOutputV1, ReopenLineageV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourceGenerationRequestV1, SourcePolicyHandoffAccessV1, SourcePolicyHandoffAccountV1,
    SourcePolicyHandoffJoinV1, SourceReopenGenerationRequestV1,
    SourceReceiptDispositionV1, SourceReleaseManifestV2, SourceWorkReceiptAccessV1,
    SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1, SuccessfulEvaluationHandoffV1,
    OPEN_RAW_PAGE_ACCOUNT_TAG, RAW_PAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_TAG,
    REOPEN_LINEAGE_ACCOUNT_VERSION, RUNTIME_ACCOUNT_GLOBAL_VERSION, SOURCE_HEAD_ACCOUNT_TAG,
    SOURCE_RELEASE_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_VERSION, SOURCE_WORK_RECEIPT_ACCOUNT_TAG,
    SOURCE_WORK_RECEIPT_ACCOUNT_VERSION, STATISTIC_RESULT_ACCOUNT_TAG, WINDOW_SEAL_ACCOUNT_TAG,
    WINDOW_WORK_ACCOUNT_TAG,
};
use solana_account_info::AccountInfo;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::accounts::Outcome;
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{
    decode_loader_pair_v1, LoaderAccountViewV1, PROGRAMDATA_SLOT_OFFSET, PROGRAM_LINK_OFFSET,
    UPGRADEABLE_LOADER_ID,
};
use crate::instructions::artifact::CLOCK_SYSVAR_ID;
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;

const CLOCK_SYSVAR_BYTES_V1: usize = 40;
const CLOCK_UNIX_TIMESTAMP_OFFSET_V1: usize = 32;

const INSTRUCTION_DATA_DOMAIN: &[u8] = b"dragons-clutch/sbf-instruction-data/v1";
const ACCOUNT_VECTOR_DOMAIN: &[u8] = b"dragons-clutch/sbf-account-vector/v1";
const ACCOUNT_VECTOR_ENTRY_BYTES: usize = 105;
/// Maximum ordered accounts admitted to one reviewed Source parser invocation.
pub const MAX_SOURCE_PARSER_ACCOUNTS: usize = 16;

/// Route one centrally allocated but profile-disabled SourcePlane action to refusal.
///
/// This boundary is deliberately account-free. The dispatcher calls it before
/// account inspection, so merely allocating actions 1 through 12 cannot make a
/// partially implemented action 2 through 12 executable. The exhaustive match also
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
    let manifest = SourceReleaseManifestV2::decode(&data)?;
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

/// Runtime-authenticated immutable WindowSpec input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedWindowSpecInputV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    window: WindowSpecV3,
    authentication_id: ContentId,
}

impl AuthenticatedWindowSpecInputV1 {
    /// Physical content-addressed WindowSpec account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete canonical account body.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact canonical WindowSpec body.
    pub const fn window(self) -> WindowSpecV3 {
        self.window
    }

    /// Owner/PDA/body/route authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Runtime-authenticated immutable SummaryProgram input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSummaryProgramInputV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    summary: SummaryProgramV3,
    authentication_id: ContentId,
}

impl AuthenticatedSummaryProgramInputV1 {
    /// Physical content-addressed SummaryProgram account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete canonical account body.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact canonical SummaryProgram body.
    pub const fn summary(self) -> SummaryProgramV3 {
        self.summary
    }

    /// Owner/PDA/body/route authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Runtime-authenticated immutable StatisticKey input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedStatisticKeyInputV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    key: StatisticKeyV3,
    authentication_id: ContentId,
}

impl AuthenticatedStatisticKeyInputV1 {
    /// Physical content-addressed StatisticKey account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete canonical account body.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact canonical StatisticKey body.
    pub const fn key(self) -> StatisticKeyV3 {
        self.key
    }

    /// Owner/PDA/body/route authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

fn require_immutable_source_input(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> SourceV3SbfResult<()> {
    if account.owner != program_id {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongOwner,
        ));
    }
    if account.is_writable || account.is_signer {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongPrivilege,
        ));
    }
    if account.executable {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongExecutableState,
        ));
    }
    Ok(())
}

fn immutable_source_input_id(
    domain: &[u8],
    route: AuthenticatedSourceRouteV1,
    account: RuntimeKey,
    account_data_id: ContentId,
    semantic_id: ContentId,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            &route.route_id().bytes(),
            &account.bytes(),
            &account_data_id.bytes(),
            &semantic_id.bytes(),
        ])
        .to_bytes(),
    )
}

/// Authenticate one canonical program-owned WindowSpec content account.
pub fn authenticate_window_spec_input(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedWindowSpecInputV1> {
    require_immutable_source_input(program_id, account)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let window = WindowSpecV3::decode(&data)?;
    let window_id = window.id()?;
    if window.source_spec_id != route.source_spec_id()
        || window.source_plane_program_id != route.source_plane_contract_id()
    {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::MismatchedBinding,
        ));
    }
    let recipe = PdaRecipeV3::window_spec(window_id)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    if derived.address != runtime_key(account.key) {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongPda,
        ));
    }
    let account_key = runtime_key(account.key);
    let data_id = account_data_id(account_key, &data)?;
    Ok(AuthenticatedWindowSpecInputV1 {
        account: account_key,
        account_data_id: data_id,
        window,
        authentication_id: immutable_source_input_id(
            b"dragons-clutch/authenticated-window-spec-input/v1",
            route,
            account_key,
            data_id,
            window_id,
        ),
    })
}

/// Authenticate one canonical program-owned SummaryProgram content account.
pub fn authenticate_summary_program_input(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSummaryProgramInputV1> {
    require_immutable_source_input(program_id, account)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let summary = SummaryProgramV3::decode(&data)?;
    let summary_id = summary.id()?;
    let recipe = PdaRecipeV3::summary_program(summary_id)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    if derived.address != runtime_key(account.key) {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongPda,
        ));
    }
    let account_key = runtime_key(account.key);
    let data_id = account_data_id(account_key, &data)?;
    Ok(AuthenticatedSummaryProgramInputV1 {
        account: account_key,
        account_data_id: data_id,
        summary,
        authentication_id: immutable_source_input_id(
            b"dragons-clutch/authenticated-summary-program-input/v1",
            route,
            account_key,
            data_id,
            summary_id,
        ),
    })
}

/// Authenticate one canonical program-owned StatisticKey content account and
/// bind it to the already-authenticated WindowSpec and SummaryProgram inputs.
pub fn authenticate_statistic_key_input(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    account: &AccountInfo<'_>,
    window: AuthenticatedWindowSpecInputV1,
    summary: AuthenticatedSummaryProgramInputV1,
) -> SourceV3SbfResult<AuthenticatedStatisticKeyInputV1> {
    require_immutable_source_input(program_id, account)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let key = StatisticKeyV3::decode(&data)?;
    let key_id = key.id()?;
    if key.window_id != window.window().id()?
        || key.summary_program_id != summary.summary().id()?
        || !summary.summary().supports(key.statistic)
    {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::MismatchedBinding,
        ));
    }
    let recipe = PdaRecipeV3::statistic_key(key_id)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    if derived.address != runtime_key(account.key) {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::WrongPda,
        ));
    }
    let account_key = runtime_key(account.key);
    let data_id = account_data_id(account_key, &data)?;
    Ok(AuthenticatedStatisticKeyInputV1 {
        account: account_key,
        account_data_id: data_id,
        key,
        authentication_id: immutable_source_input_id(
            b"dragons-clutch/authenticated-statistic-key-input/v1",
            route,
            account_key,
            data_id,
            key_id,
        ),
    })
}

/// Authenticate action 10's StatisticKey against the WindowSpec and the exact
/// SummaryProgram identity supplied by the already-authenticated immutable
/// Failure/Product policy. No caller SummaryProgram body is accepted here.
pub fn authenticate_statistic_key_policy_input(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    account: &AccountInfo<'_>,
    window: AuthenticatedWindowSpecInputV1,
    authenticated_summary_program_id: ContentId,
) -> SourceV3SbfResult<AuthenticatedStatisticKeyInputV1> {
    require_immutable_source_input(program_id, account)?;
    if authenticated_summary_program_id.is_zero() {
        return Err(clutch_source_plane_v3_runtime::Error::ZeroIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let key = StatisticKeyV3::decode(&data)?;
    let key_id = key.id()?;
    if key.window_id != window.window().id()?
        || key.summary_program_id != authenticated_summary_program_id
    {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    let recipe = PdaRecipeV3::statistic_key(key_id)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    if derived.address != runtime_key(account.key) {
        return Err(clutch_source_plane_v3_runtime::Error::WrongPda.into());
    }
    let account_key = runtime_key(account.key);
    let data_id = account_data_id(account_key, &data)?;
    Ok(AuthenticatedStatisticKeyInputV1 {
        account: account_key,
        account_data_id: data_id,
        key,
        authentication_id: immutable_source_input_id(
            b"dragons-clutch/authenticated-statistic-key-policy-input/v1",
            route,
            account_key,
            data_id,
            key_id,
        ),
    })
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

/// Authenticate a durable WindowSeal account as the exact evidence admitted
/// by action 9 under the current release and mature Clock.
pub fn authenticate_persisted_window_evidence_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    seal_account: &AccountInfo<'_>,
    clock: AuthenticatedClockBucketV1,
    window: &WindowSpecV3,
) -> SourceV3SbfResult<AuthenticatedWindowEvidenceV1> {
    if clock.policy_id() != route.clock_policy_id() {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    let seal = authenticate_window_seal(program_id, route, seal_account, window)?;
    authenticate_persisted_window_evidence(
        route,
        &route.source_plane(),
        &route.clock_policy(),
        clock.snapshot(),
        window,
        seal,
    )
    .map_err(Into::into)
}

/// Derive action 9's evaluator release binding only from the presented
/// Upgradeable Loader Program/ProgramData accounts, then require both the
/// SummaryProgram deployment digest and the Source release's complete
/// deployment-plus-semantics digest to match.
pub fn authenticate_evaluation_release_binding(
    route: AuthenticatedSourceRouteV1,
    summary: SummaryProgramV3,
    evaluator_program: &AccountInfo<'_>,
    evaluator_programdata: &AccountInfo<'_>,
) -> SourceV3SbfResult<EvaluationReleaseBindingV1> {
    if evaluator_program.is_signer
        || evaluator_program.is_writable
        || evaluator_programdata.is_signer
        || evaluator_programdata.is_writable
    {
        return Err(clutch_source_plane_v3_runtime::Error::WrongPrivilege.into());
    }
    let program_data = evaluator_program
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let programdata_data = evaluator_programdata
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let decoded = decode_loader_pair_v1(
        LoaderAccountViewV1::new(
            evaluator_program.key.to_bytes(),
            evaluator_program.owner.to_bytes(),
            evaluator_program.executable,
            &program_data,
        ),
        LoaderAccountViewV1::new(
            evaluator_programdata.key.to_bytes(),
            evaluator_programdata.owner.to_bytes(),
            evaluator_programdata.executable,
            &programdata_data,
        ),
    )
    .map_err(|_| SourceV3SbfError::WrongEvaluatorProgram)?;
    let deployment = DeploymentBindingV1 {
        program: runtime_key(evaluator_program.key),
        program_account_data_id: account_data_id(runtime_key(evaluator_program.key), &program_data)?,
        programdata: runtime_key(evaluator_programdata.key),
        programdata_account_data_id: account_data_id(
            runtime_key(evaluator_programdata.key),
            &programdata_data,
        )?,
        loader: RuntimeKey::from_bytes(UPGRADEABLE_LOADER_ID),
        programdata_link_offset: u16::try_from(PROGRAM_LINK_OFFSET)
            .map_err(|_| clutch_source_plane_v3_runtime::Error::ArithmeticOverflow)?,
        deployment_slot_offset: u16::try_from(PROGRAMDATA_SLOT_OFFSET)
            .map_err(|_| clutch_source_plane_v3_runtime::Error::ArithmeticOverflow)?,
        deployment_slot: decoded.state.deployment_slot,
    };
    let binding = EvaluationReleaseBindingV1 {
        deployment,
        summary_program_id: summary.id()?,
    };
    if deployment.id()? != summary.evaluator_release_id
        || binding.id()? != route.evaluation_release_id()
    {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    Ok(binding)
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

/// Authenticate action 10's persisted result using the immutable
/// Failure/Product policy's SummaryProgram identity rather than accepting a
/// caller-provided SummaryProgram body.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_persisted_result_account(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    result_account: &AccountInfo<'_>,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    authenticated_summary_program_id: ContentId,
    evidence: AuthenticatedWindowEvidenceV1,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedStatisticResultAccountV1> {
    let data = result_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let recipe = PdaRecipeV3::statistic_result(key.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    authenticate_persisted_statistic_result_account(
        route,
        runtime_account_view(result_account, &data),
        derived,
        window,
        key,
        authenticated_summary_program_id,
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

/// Authenticate the exact predictable WindowSeal slot as still unallocated.
///
/// The mature result-absence branch cannot accept a caller-selected evidence
/// account and cannot silently ignore the frozen action-10 WindowSeal role.
pub fn authenticate_window_seal_absence(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    account: &AccountInfo<'_>,
    window: &WindowSpecV3,
) -> SourceV3SbfResult<ContentId> {
    let recipe = PdaRecipeV3::window_seal(window.id()?)?;
    let derived = derive_runtime_pda(program_id, &recipe)?;
    if runtime_key(account.key) != derived.address
        || *account.owner != SYSTEM_PROGRAM_ID
        || !account.data_is_empty()
        || account.executable
        || account.is_signer
        || account.is_writable
    {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::MismatchedBinding,
        ));
    }
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/authenticated-window-seal-absence/v1",
            &route.route_id().bytes(),
            &window.id()?.bytes(),
            account.key.as_ref(),
        ])
        .to_bytes(),
    );
    if value.is_zero() {
        return Err(SourceV3SbfError::Runtime(
            clutch_source_plane_v3_runtime::Error::ZeroIdentity,
        ));
    }
    Ok(value)
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

/// Authenticate action 4's release-selected receiver Program, ProgramData,
/// and exact Config bytes.
pub fn authenticate_receiver_route(
    route: AuthenticatedSourceRouteV1,
    receiver_program: &AccountInfo<'_>,
    receiver_programdata: &AccountInfo<'_>,
    receiver_config: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedReceiverRouteV2> {
    let program_data = receiver_program
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let programdata_data = receiver_programdata
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let config_data = receiver_config
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    authenticate_receiver_route_v2(
        route,
        runtime_account_view(receiver_program, &program_data),
        runtime_account_view(receiver_programdata, &programdata_data),
        runtime_account_view(receiver_config, &config_data),
    )
    .map_err(Into::into)
}

/// Authenticate one immutable Product/failure generation request under the
/// exact authority program selected by the Source release. Current V2 releases
/// require that authority to be the already-authenticated adapter deployment;
/// historical V1 external-authority artifacts remain non-executable.
pub fn authenticate_generation_request(
    route: AuthenticatedSourceRouteV1,
    request_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSourceGenerationV1> {
    let data = request_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let request = SourceGenerationRequestV1::decode(&data)?;
    let request_id = request.id()?;
    let authority = Pubkey::new_from_array(route.generation_authority_program().bytes());
    let (address, bump) = crate::seeds::find(
        &authority,
        &[crate::seeds::SEED_SOURCE_GENERATION_REQUEST_V1, &request_id.bytes()],
    );
    authenticate_source_generation_request(
        route,
        runtime_account_view(request_account, &data),
        RuntimeDerivedPdaV1 {
            program_id: route.generation_authority_program(),
            recipe_id: request_id,
            address: runtime_key(&address),
            bump,
        },
    )
    .map_err(Into::into)
}

/// Authenticate action 11's immutable typed target under the exact
/// generation-authority program selected by the current Source release, then
/// join it to the complete closed-lineage preimage.
pub fn authenticate_reopen_generation_request(
    route: AuthenticatedSourceRouteV1,
    request_account: &AccountInfo<'_>,
    lineage: AuthenticatedReopenLineageV1,
) -> SourceV3SbfResult<AuthenticatedSourceReopenGenerationV1> {
    let data = request_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let request = SourceReopenGenerationRequestV1::decode(&data)?;
    let request_id = request.id()?;
    let authority = Pubkey::new_from_array(route.generation_authority_program().bytes());
    let (address, bump) = crate::seeds::find(
        &authority,
        &[crate::seeds::SEED_SOURCE_REOPEN_REQUEST_V1, &request_id.bytes()],
    );
    authenticate_source_reopen_generation_request(
        route,
        runtime_account_view(request_account, &data),
        RuntimeDerivedPdaV1 {
            program_id: route.generation_authority_program(),
            recipe_id: request_id,
            address: runtime_key(&address),
            bump,
        },
        lineage,
    )
    .map_err(Into::into)
}

/// Authenticate action 12's persisted reopen policy against the exact
/// currently-open lineage and deterministic closed-lineage postimage. This is
/// only a preclose proof; action 11 still requires the later closed-lineage
/// authentication.
pub fn authenticate_reopen_generation_request_before_close(
    route: AuthenticatedSourceRouteV1,
    request_account: &AccountInfo<'_>,
    lineage: AuthenticatedReopenLineageV1,
    terminal_semantic_id: ContentId,
) -> SourceV3SbfResult<AuthenticatedSourceReopenPrecloseV1> {
    let data = request_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let request = SourceReopenGenerationRequestV1::decode(&data)?;
    let request_id = request.id()?;
    let authority = Pubkey::new_from_array(route.generation_authority_program().bytes());
    let (address, bump) = crate::seeds::find(
        &authority,
        &[crate::seeds::SEED_SOURCE_REOPEN_REQUEST_V1, &request_id.bytes()],
    );
    authenticate_source_reopen_generation_request_before_close_runtime(
        route,
        runtime_account_view(request_account, &data),
        RuntimeDerivedPdaV1 {
            program_id: route.generation_authority_program(),
            recipe_id: request_id,
            address: runtime_key(&address),
            bump,
        },
        lineage,
        terminal_semantic_id,
    )
    .map_err(Into::into)
}

fn decode_current_clock_snapshot(
    clock_account: &AccountInfo<'_>,
    data: &[u8],
) -> SourceV3SbfResult<ClockSnapshotV1> {
    if clock_account.owner.to_bytes() != crate::instructions_sysvar::SYSVAR_OWNER_ID
        || clock_account.executable
        || data.len() != CLOCK_SYSVAR_BYTES_V1
    {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(&data[..8]);
    let mut unix_timestamp = [0_u8; 8];
    unix_timestamp.copy_from_slice(
        &data[CLOCK_UNIX_TIMESTAMP_OFFSET_V1..CLOCK_UNIX_TIMESTAMP_OFFSET_V1 + 8],
    );
    Ok(ClockSnapshotV1 {
        slot: u64::from_le_bytes(slot),
        unix_timestamp: u64::try_from(i64::from_le_bytes(unix_timestamp))
            .map_err(|_| SourceV3SbfError::WrongClockAccount)?,
    })
}

/// Derive a policy-bound current bucket from the canonical Solana Clock account.
pub fn authenticate_clock_bucket(
    release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedClockBucketV1> {
    if *clock_account.key != CLOCK_SYSVAR_ID
        || clock_account.is_signer
        || clock_account.is_writable
    {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    let data = clock_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let clock = decode_current_clock_snapshot(clock_account, &data)?;
    AuthenticatedClockBucketV1::from_snapshot(
        &release.clock_policy(),
        clock,
    )
    .map_err(Into::into)
}

/// Derive the current bucket from canonical Clock under the exact clock policy
/// already selected by the fully authenticated Source route.
pub fn authenticate_route_clock_bucket(
    route: AuthenticatedSourceRouteV1,
    clock_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedClockBucketV1> {
    if *clock_account.key != CLOCK_SYSVAR_ID
        || clock_account.is_signer
        || clock_account.is_writable
    {
        return Err(SourceV3SbfError::WrongClockAccount);
    }
    let data = clock_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let clock = decode_current_clock_snapshot(clock_account, &data)?;
    AuthenticatedClockBucketV1::from_snapshot(&route.clock_policy(), clock).map_err(Into::into)
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

/// Authenticate Product's exact occurrence PDA/body and its canonical
/// WindowSpec before a StatisticKey body is needed by evaluation.
pub fn authenticate_occurrence_window(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    occurrence_account: &AccountInfo<'_>,
    disposition: OccurrenceDispositionV1,
    window: &WindowSpecV3,
) -> SourceV3SbfResult<OccurrenceWindowReceiptV1> {
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
    join_source_occurrence_window(
        route,
        runtime_account_view(occurrence_account, &data),
        derived,
        disposition,
        window,
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

/// Fully reconstructed successful action-10 handoff from persisted Source
/// accounts.
///
/// Private fields prevent a downstream Failure caller from substituting a
/// handoff, Clock snapshot, occurrence, result, or work receipt. Construction
/// starts from the self-authenticating durable record, reopens every exact
/// account it names, reconstructs the semantic handoff under the release Clock
/// policy, and finally mints the stronger persisted-record receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSuccessfulSourceHandoffV1 {
    record: AuthenticatedSourcePolicyHandoffRecordV1,
    handoff: SuccessfulEvaluationHandoffV1,
    join: SourcePolicyHandoffJoinV1,
    persisted: clutch_source_plane_v3_runtime::AuthenticatedPersistedSourcePolicyHandoffV1,
    lineage: AuthenticatedReopenLineageV1,
    interval: AuthenticatedSourceIntervalContextV1,
}

/// Private exact interval bodies reconstructed from the durable action-10
/// graph. Failure/Product may borrow these values only through this receipt;
/// they never re-decode caller bodies or manufacture parallel semantic IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceIntervalContextV1 {
    occurrence: CompiledSourceOccurrenceV3,
    window: WindowSpecV3,
    statistic_key: StatisticKeyV3,
    summary_program: SummaryProgramV3,
    window_seal: WindowSealV3,
    statistic_result: StatisticResultV3,
    authentication_id: ContentId,
}

impl AuthenticatedSourceIntervalContextV1 {
    pub(crate) const fn occurrence(self) -> CompiledSourceOccurrenceV3 {
        self.occurrence
    }

    pub(crate) const fn window(self) -> WindowSpecV3 {
        self.window
    }

    pub(crate) const fn statistic_key(self) -> StatisticKeyV3 {
        self.statistic_key
    }

    pub(crate) const fn summary_program(self) -> SummaryProgramV3 {
        self.summary_program
    }

    pub(crate) const fn window_seal(self) -> WindowSealV3 {
        self.window_seal
    }

    pub(crate) const fn statistic_result(self) -> StatisticResultV3 {
        self.statistic_result
    }

    pub(crate) const fn id(self) -> ContentId {
        self.authentication_id
    }
}

impl AuthenticatedSuccessfulSourceHandoffV1 {
    /// Reconstructed semantic successful-evaluation handoff.
    pub(crate) const fn handoff(self) -> SuccessfulEvaluationHandoffV1 {
        self.handoff
    }

    /// Exact Source physical-account join.
    pub(crate) const fn join(self) -> SourcePolicyHandoffJoinV1 {
        self.join
    }

    /// Strong persisted action-10 account authentication.
    pub(crate) const fn persisted(
        self,
    ) -> clutch_source_plane_v3_runtime::AuthenticatedPersistedSourcePolicyHandoffV1 {
        self.persisted
    }

    /// Exact open StatisticResult lineage for the final terminal composer.
    pub(crate) const fn lineage(self) -> AuthenticatedReopenLineageV1 {
        self.lineage
    }

    /// Exact hostile-authenticated Source interval bodies required by Product
    /// consensus during the composed Failure resolution.
    pub(crate) const fn interval(self) -> AuthenticatedSourceIntervalContextV1 {
        self.interval
    }

    /// Complete no-caller-projection reconstruction identity.
    pub(crate) fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                b"dragons-clutch/sbf/reconstructed-successful-source-handoff/v1",
                &self.record.id().bytes(),
                &self.handoff.id().bytes(),
                &self.join.id().bytes(),
                &self.persisted.id().bytes(),
                &self.lineage.id().bytes(),
                &self.lineage.account_data_id().bytes(),
                &self.interval.id().bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Reconstruct a successful Source handoff directly from the exact durable
/// action-10 account and all accounts named by it. The Clock snapshot is
/// already committed by action 10 and is revalidated under the current release
/// policy; a later Failure instruction neither needs nor may substitute a live
/// Clock account.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_successful_source_handoff_from_accounts_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    occurrence_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    summary_program_account: &AccountInfo<'_>,
    window_seal_account: &AccountInfo<'_>,
    result_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    handoff_account: &AccountInfo<'_>,
    work_receipt_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSuccessfulSourceHandoffV1> {
    authenticate_successful_source_handoff_from_accounts_inner_v1(
        program_id,
        route,
        schedule,
        occurrence_account,
        window_account,
        statistic_key_account,
        summary_program_account,
        window_seal_account,
        result_account,
        result_lineage_account,
        handoff_account,
        work_receipt_account,
        SuccessfulSourceHandoffAccessV1::ExistingReadOnly,
    )
}

/// Sole reconstruction admitted inside the atomic ResolutionV5 terminal path.
/// Result and lineage must both be writable for the later physical close;
/// every other persisted Source fact remains exact-existing read-only.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_successful_source_handoff_for_resolution_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    occurrence_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    summary_program_account: &AccountInfo<'_>,
    window_seal_account: &AccountInfo<'_>,
    result_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    handoff_account: &AccountInfo<'_>,
    work_receipt_account: &AccountInfo<'_>,
) -> SourceV3SbfResult<AuthenticatedSuccessfulSourceHandoffV1> {
    authenticate_successful_source_handoff_from_accounts_inner_v1(
        program_id,
        route,
        schedule,
        occurrence_account,
        window_account,
        statistic_key_account,
        summary_program_account,
        window_seal_account,
        result_account,
        result_lineage_account,
        handoff_account,
        work_receipt_account,
        SuccessfulSourceHandoffAccessV1::ResolutionMutable,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuccessfulSourceHandoffAccessV1 {
    ExistingReadOnly,
    ResolutionMutable,
}

impl SuccessfulSourceHandoffAccessV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::ExistingReadOnly => 1,
            Self::ResolutionMutable => 2,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_successful_source_handoff_from_accounts_inner_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    occurrence_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    summary_program_account: &AccountInfo<'_>,
    window_seal_account: &AccountInfo<'_>,
    result_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    handoff_account: &AccountInfo<'_>,
    work_receipt_account: &AccountInfo<'_>,
    access: SuccessfulSourceHandoffAccessV1,
) -> SourceV3SbfResult<AuthenticatedSuccessfulSourceHandoffV1> {
    let handoff_data = handoff_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let handoff_body = SourcePolicyHandoffAccountV1::decode(&handoff_data)?;
    let handoff_recipe =
        PdaRecipeV3::source_policy_handoff(handoff_body.source_policy_handoff_join_id())?;
    let handoff_derived = derive_runtime_pda(program_id, &handoff_recipe)?;
    let record = authenticate_source_policy_handoff_record(
        route,
        runtime_account_view(handoff_account, &handoff_data),
        handoff_derived,
    )?;
    if record.occurrence_account() != runtime_key(occurrence_account.key)
        || record.result_account() != runtime_key(result_account.key)
        || record.work_receipt_account() != runtime_key(work_receipt_account.key)
    {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    let window = authenticate_window_spec_input(program_id, route, window_account)?;
    let summary = authenticate_summary_program_input(program_id, route, summary_program_account)?;
    let key = authenticate_statistic_key_input(
        program_id,
        route,
        statistic_key_account,
        window,
        summary,
    )?;
    if record.window_id() != window.window().id()?
        || record.statistic_key_id() != key.key().id()?
    {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    let occurrence = authenticate_occurrence(
        program_id,
        route,
        occurrence_account,
        OccurrenceDispositionV1::ExactExisting,
        &window.window(),
        &key.key(),
    )?;
    let occurrence_data = occurrence_account
        .try_borrow_data()
        .map_err(|_| SourceV3SbfError::AccountBorrow)?;
    let occurrence_body = CompiledSourceOccurrenceV3::decode(&occurrence_data).map_err(|_| {
        SourceV3SbfError::Runtime(clutch_source_plane_v3_runtime::Error::InvalidCodec)
    })?;
    drop(occurrence_data);
    let clock = AuthenticatedClockBucketV1::from_snapshot(&route.clock_policy(), record.clock())?;
    let evidence = authenticate_persisted_window_evidence_account(
        program_id,
        route,
        window_seal_account,
        clock,
        &window.window(),
    )?;
    let lineage_access = match access {
        SuccessfulSourceHandoffAccessV1::ExistingReadOnly => LineageAccessV1::ReadOnly,
        SuccessfulSourceHandoffAccessV1::ResolutionMutable => LineageAccessV1::Mutable,
    };
    let lineage = authenticate_lineage(
        program_id,
        route,
        result_lineage_account,
        lineage_access,
    )?;
    let result = match access {
        SuccessfulSourceHandoffAccessV1::ExistingReadOnly => authenticate_result_account(
            program_id,
            route,
            result_account,
            &window.window(),
            &key.key(),
            &summary.summary(),
            evidence,
            lineage,
        )?,
        SuccessfulSourceHandoffAccessV1::ResolutionMutable => {
            let result_data = result_account
                .try_borrow_data()
                .map_err(|_| SourceV3SbfError::AccountBorrow)?;
            let recipe = PdaRecipeV3::statistic_result(key.key().id()?)?;
            let derived = derive_runtime_pda(program_id, &recipe)?;
            let authenticated = authenticate_persisted_statistic_result_account_for_resolution(
                route,
                runtime_account_view(result_account, &result_data),
                derived,
                &window.window(),
                &key.key(),
                summary.summary().id()?,
                evidence,
                lineage,
            )?;
            drop(result_data);
            authenticated
        }
    };
    let handoff = successful_evaluation_handoff(
        route,
        record.failure_policy_binding_id(),
        occurrence,
        clock,
        &window.window(),
        evidence,
        result,
    )?;
    let work = authenticate_work_receipt(program_id, route, schedule, work_receipt_account)?;
    let join = SourcePolicyHandoffJoinV1::successful_evaluation(route, handoff, result, work)?;
    if join.id() != record.source_policy_handoff_join_id()
        || handoff.id() != record.handoff_id()
        || join.generation() != record.generation()
    {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    let persisted = authenticate_persisted_source_policy_handoff(
        route,
        join,
        runtime_account_view(handoff_account, &handoff_data),
        handoff_derived,
        SourcePolicyHandoffAccessV1::ExistingReadOnly,
    )?;
    drop(handoff_data);
    let interval_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/authenticated-source-interval-context/v1",
            &occurrence.occurrence_record_id().bytes(),
            &occurrence.id().bytes(),
            &window.id().bytes(),
            &key.id().bytes(),
            &summary.id().bytes(),
            &evidence.id().bytes(),
            &result.id().bytes(),
            &lineage.id().bytes(),
            &[access.byte()],
        ])
        .to_bytes(),
    );
    if interval_authentication_id.is_zero() {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    let interval = AuthenticatedSourceIntervalContextV1 {
        occurrence: occurrence_body,
        window: window.window(),
        statistic_key: key.key(),
        summary_program: summary.summary(),
        window_seal: evidence.seal(),
        statistic_result: result.result(),
        authentication_id: interval_authentication_id,
    };
    let value = AuthenticatedSuccessfulSourceHandoffV1 {
        record,
        handoff,
        join,
        persisted,
        lineage,
        interval,
    };
    if value.id().is_zero() {
        return Err(clutch_source_plane_v3_runtime::Error::MismatchedBinding.into());
    }
    Ok(value)
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
    route: AuthenticatedSourceRouteV1,
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
        route,
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
    receiver: AuthenticatedReceiverRouteV2,
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
        receiver,
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
    let mut receiver_program_count = 0_u8;
    let mut receiver_programdata_count = 0_u8;
    let mut receiver_config_count = 0_u8;
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
        if runtime_key(account.key) == route.receiver_program() {
            if account.is_signer || account.is_writable || !account.executable {
                return Err(SourceV3SbfError::ParserAccountVector);
            }
            receiver_program_count = receiver_program_count
                .checked_add(1)
                .ok_or(SourceV3SbfError::ParserAccountVector)?;
        }
        if runtime_key(account.key) == route.receiver_programdata() {
            if account.is_signer || account.is_writable || account.executable {
                return Err(SourceV3SbfError::ParserAccountVector);
            }
            receiver_programdata_count = receiver_programdata_count
                .checked_add(1)
                .ok_or(SourceV3SbfError::ParserAccountVector)?;
        }
        if runtime_key(account.key) == route.receiver_config() {
            if account.is_signer || account.is_writable || account.executable {
                return Err(SourceV3SbfError::ParserAccountVector);
            }
            receiver_config_count = receiver_config_count
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
    if feed_count != 1
        || config_count != 1
        || receiver_program_count != 1
        || receiver_programdata_count != 1
        || receiver_config_count != 1
    {
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
