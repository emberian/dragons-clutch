use clutch_product_series::{CompiledSourceOccurrenceV3, FixedCodec as ProductFixedCodec};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, RawPageV3, SourcePlaneProgramV3, StatisticKeyV3,
    StatisticResultStatusV3, StatisticResultV3, SummaryProgramV3, WindowClosureReceiptV3,
    WindowSealV3, WindowSpecV3, WindowWorkV3, RAW_PAGE_BYTES, STATISTIC_RESULT_BYTES,
    WINDOW_SEAL_BYTES, WINDOW_WORK_BYTES,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;

use crate::account::{decode_runtime_account, RuntimeAccountHeaderV1};
use crate::auth::{
    account_data_id, domain_id, live_id, AdapterInvocationV1, AuthenticatedSourceRouteV1,
    ClockPolicyV1, ClockSnapshotV1, DeploymentBindingV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1,
    RuntimeKey,
};
use crate::funding::{
    AuthenticatedSourceWorkReceiptV1, SourceReceiptDispositionV1, SourceWorkKindV1,
};
use crate::lineage::{AuthenticatedReopenLineageV1, LineageAccessV1, LineageFamilyV1};
use crate::{Error, Result};

const PAGE_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-raw-page/v1";
const WORK_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-window-work/v1";
const WORK_STATE_DOMAIN: &[u8] = b"dragons-clutch/window-work-state/v1";
const SEAL_ACCOUNT_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-window-seal-account/v1";
const FOLD_DOMAIN: &[u8] = b"dragons-clutch/window-page-fold-batch/v1";
const WINDOW_EVIDENCE_DOMAIN: &[u8] = b"dragons-clutch/authenticated-window-evidence/v1";
const PERSISTED_WINDOW_EVIDENCE_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-persisted-window-evidence/v1";
const EVALUATION_AUTHORITY_DOMAIN: &[u8] = b"dragons-clutch/evaluation-authority/v1";
const EVALUATION_RELEASE_DOMAIN: &[u8] =
    b"dragons-clutch/source-evaluation-release-binding/v1";
const EVALUATION_DOMAIN: &[u8] = b"dragons-clutch/authenticated-evaluation/v1";
const RESULT_ABSENCE_DOMAIN: &[u8] = b"dragons-clutch/authenticated-statistic-result-absence/v1";
const RESULT_ACCOUNT_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-statistic-result-account/v1";
const OCCURRENCE_JOIN_DOMAIN: &[u8] = b"dragons-clutch/source-occurrence-join/v1";
const FAILURE_HANDOFF_DOMAIN: &[u8] = b"dragons-clutch/failure-policy-source-handoff/v1";
const SUCCESS_HANDOFF_DOMAIN: &[u8] = b"dragons-clutch/successful-evaluation-source-handoff/v1";
const POLICY_HANDOFF_JOIN_DOMAIN: &[u8] = b"dragons-clutch/source-policy-handoff-account-join/v1";
const PERSISTED_POLICY_HANDOFF_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-persisted-source-policy-handoff/v1";
const SOURCE_POLICY_HANDOFF_MAGIC: [u8; 8] = *b"DCSPHF01";

/// Exact raw account width of one persisted Source policy handoff.
pub const SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES: usize = 488;

/// Exact Product/Series-owned occurrence record width.
pub const SOURCE_OCCURRENCE_RECORD_BYTES: usize =
    clutch_product_series::SOURCE_OCCURRENCE_RECORD_BYTES;
/// Maximum immutable pages folded by one bounded runtime call.
pub const MAX_PAGES_PER_FOLD: usize = 4;

/// Runtime-authenticated immutable raw-page account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRawPageV1 {
    route_id: ContentId,
    account: RuntimeKey,
    account_data_id: ContentId,
    header: RuntimeAccountHeaderV1,
    page: RawPageV3,
    authentication_id: ContentId,
}

/// Runtime-authenticated mutable WindowWork account and open lineage generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedWindowWorkV1 {
    route_id: ContentId,
    account: RuntimeKey,
    terminal_generation: u64,
    header: RuntimeAccountHeaderV1,
    account_data_id: ContentId,
    work: WindowWorkV3,
    authentication_id: ContentId,
}

impl AuthenticatedWindowWorkV1 {
    /// Exact authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical WindowWork account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Durable close/reopen generation.
    pub const fn terminal_generation(self) -> u64 {
        self.terminal_generation
    }

    /// Exact decoded runtime account header.
    pub const fn header(self) -> RuntimeAccountHeaderV1 {
        self.header
    }

    /// Digest of complete before-account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Complete canonical WindowWork body.
    pub const fn work(self) -> WindowWorkV3 {
        self.work
    }

    /// Complete account/PDA/body/lineage authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

impl AuthenticatedRawPageV1 {
    /// Exact source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical immutable page account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete canonical account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact decoded runtime account header.
    pub const fn header(self) -> RuntimeAccountHeaderV1 {
        self.header
    }

    /// Complete canonical raw page.
    pub const fn page(self) -> RawPageV3 {
        self.page
    }

    /// Exact account/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate owner, address, PDA recipe, bump, envelope, and complete page body.
pub fn authenticate_raw_page_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
) -> Result<AuthenticatedRawPageV1> {
    require_immutable_adapter_account(route, account)?;
    let (header, page) = decode_runtime_account::<RawPageV3>(account.data, route.neutral_sink())?;
    page.validate()?;
    let recipe = PdaRecipeV3::raw_page(route.source_plane_contract_id(), page.id()?)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        header.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0; 136];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&page.id()?.bytes());
    bytes[128] = header.bump;
    Ok(AuthenticatedRawPageV1 {
        route_id: route.route_id(),
        account: account.key,
        account_data_id,
        header,
        page,
        authentication_id: domain_id(PAGE_AUTH_DOMAIN, &bytes),
    })
}

/// Authenticate writable WindowWork owner/PDA/body and its exact open lineage.
pub fn authenticate_window_work_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    window: &WindowSpecV3,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedWindowWorkV1> {
    require_mutable_adapter_account(route, account)?;
    if authenticated_lineage.access() != LineageAccessV1::Mutable {
        return Err(Error::WrongPrivilege);
    }
    validate_window_route(route, window)?;
    let lineage = authenticated_lineage.lineage();
    lineage.validate()?;
    let (header, work) =
        decode_runtime_account::<WindowWorkV3>(account.data, route.neutral_sink())?;
    work.validate_against(window)?;
    let recipe = PdaRecipeV3::window_work(window.id()?)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        header.bump,
    )?;
    if lineage.adapter_program != route.adapter_program()
        || lineage.family != LineageFamilyV1::WindowWork
        || lineage.semantic_binding_id != window.id()?
        || !lineage.is_open
        || lineage.active_account != account.key
        || lineage.latest_generation != header.generation
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    let account_data_id = account_data_id(account.key, account.data)?;
    let state_id = window_work_state_id(&work)?;
    let mut bytes = [0; 208];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&window.id()?.bytes());
    bytes[64..96].copy_from_slice(&account.key.bytes());
    bytes[96..128].copy_from_slice(&account_data_id.bytes());
    bytes[128..160].copy_from_slice(&state_id.bytes());
    bytes[160..192].copy_from_slice(&lineage.lineage_account.bytes());
    bytes[192..200].copy_from_slice(&header.generation.to_le_bytes());
    bytes[200] = header.bump;
    let authentication_id = domain_id(WORK_AUTH_DOMAIN, &bytes);
    if lineage.last_opened_state_id != account_data_id {
        return Err(Error::InvalidLineage);
    }
    Ok(AuthenticatedWindowWorkV1 {
        route_id: route.route_id(),
        account: account.key,
        terminal_generation: header.generation,
        header,
        account_data_id,
        work,
        authentication_id,
    })
}

/// Result of a bounded multi-page WindowWork fold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldPagesOutputV1 {
    /// Exact post-fold work.
    pub work_after: WindowWorkV3,
    /// Number of pages folded.
    pub page_count: u8,
    /// Ordered page-authentication receipt.
    pub fold_receipt_id: ContentId,
}

/// Fold several runtime-authenticated immutable pages in canonical order.
pub fn fold_authenticated_pages(
    route: AuthenticatedSourceRouteV1,
    window: &WindowSpecV3,
    authenticated_work: AuthenticatedWindowWorkV1,
    pages: &[AuthenticatedRawPageV1],
) -> Result<FoldPagesOutputV1> {
    window.validate()?;
    if authenticated_work.route_id() != route.route_id() {
        return Err(Error::MismatchedBinding);
    }
    let work = authenticated_work.work();
    work.validate_against(window)?;
    if pages.is_empty() || pages.len() > MAX_PAGES_PER_FOLD {
        return Err(Error::InvalidCount);
    }
    validate_window_route(route, window)?;
    let mut next = work;
    let mut receipt_bytes = [0; 72 + MAX_PAGES_PER_FOLD * 32];
    receipt_bytes[..32].copy_from_slice(&window.id()?.bytes());
    receipt_bytes[32..64].copy_from_slice(&route.route_id().bytes());
    receipt_bytes[64] = u8::try_from(pages.len()).map_err(|_| Error::InvalidCount)?;
    receipt_bytes[65..72].fill(0);
    let mut index = 0_usize;
    while index < pages.len() {
        let authenticated = pages[index];
        let page = authenticated.page();
        if authenticated.route_id() != route.route_id()
            || page.source_spec_id != window.source_spec_id
            || page.repair_generation != window.repair_generation
        {
            return Err(Error::MismatchedBinding);
        }
        next = next.push_page(window, &page)?;
        let at = 72 + index * 32;
        receipt_bytes[at..at + 32].copy_from_slice(&authenticated.id().bytes());
        index += 1;
    }
    let mut transition_bytes = [0; 96];
    transition_bytes[..32].copy_from_slice(&authenticated_work.id().bytes());
    transition_bytes[32..64].copy_from_slice(&window_work_state_id(&next)?.bytes());
    transition_bytes[64..96].copy_from_slice(&domain_id(FOLD_DOMAIN, &receipt_bytes).bytes());
    Ok(FoldPagesOutputV1 {
        work_after: next,
        page_count: u8::try_from(pages.len()).map_err(|_| Error::InvalidCount)?,
        fold_receipt_id: domain_id(FOLD_DOMAIN, &transition_bytes),
    })
}

/// Runtime-authenticated final WindowSeal and closure evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedWindowEvidenceV1 {
    route_id: ContentId,
    source_spec_id: ContentId,
    source_plane_contract_id: ContentId,
    window_id: ContentId,
    repair_generation: u64,
    closure: WindowClosureReceiptV3,
    seal: WindowSealV3,
    evidence_id: ContentId,
}

/// Runtime-authenticated exact-existing immutable WindowSeal account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedWindowSealAccountV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    header: RuntimeAccountHeaderV1,
    seal: WindowSealV3,
    authentication_id: ContentId,
}

impl AuthenticatedWindowSealAccountV1 {
    /// Physical WindowSeal account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of complete account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact decoded runtime account header.
    pub const fn header(self) -> RuntimeAccountHeaderV1 {
        self.header
    }

    /// Canonical WindowSeal body.
    pub const fn seal(self) -> WindowSealV3 {
        self.seal
    }

    /// Complete owner/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

impl AuthenticatedWindowEvidenceV1 {
    /// Exact source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Existing SourceSpec identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Exact reviewed SourcePlane contract.
    pub const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    /// Predictable WindowKey.
    pub const fn window_id(self) -> ContentId {
        self.window_id
    }

    /// Exact repair generation.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// Canonical closure receipt.
    pub const fn closure(self) -> WindowClosureReceiptV3 {
        self.closure
    }

    /// Canonical final WindowSeal.
    pub const fn seal(self) -> WindowSealV3 {
        self.seal
    }

    /// Authentication identity for the complete page/work/Clock join.
    pub const fn id(self) -> ContentId {
        self.evidence_id
    }
}

/// Finish a mature Window from exact authenticated page and Clock facts.
pub fn seal_authenticated_window(
    route: AuthenticatedSourceRouteV1,
    source_plane: &SourcePlaneProgramV3,
    clock_policy: &ClockPolicyV1,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    authenticated_work: AuthenticatedWindowWorkV1,
    maturity_page: AuthenticatedRawPageV1,
) -> Result<AuthenticatedWindowEvidenceV1> {
    source_plane.validate()?;
    if authenticated_work.route_id() != route.route_id() {
        return Err(Error::MismatchedBinding);
    }
    let work = authenticated_work.work();
    validate_window_route(route, window)?;
    if source_plane.id()? != route.source_plane_contract_id()
        || clock_policy.id()? != route.clock_policy_id()
        || clock.unix_timestamp < clock_policy.bucket_timestamp(window.maturity_bucket_exclusive)?
        || maturity_page.route_id() != route.route_id()
        || maturity_page.page().source_spec_id != window.source_spec_id
        || maturity_page.page().repair_generation != window.repair_generation
    {
        return Err(Error::MismatchedBinding);
    }
    let closure = WindowClosureReceiptV3::from_page(source_plane, window, &maturity_page.page())?;
    let seal = work.finish(window, &closure)?;
    let mut bytes = [0; 232];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&window.id()?.bytes());
    bytes[64..72].copy_from_slice(&window.repair_generation.to_le_bytes());
    bytes[72..80].copy_from_slice(&clock.slot.to_le_bytes());
    bytes[80..88].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    bytes[88..120].copy_from_slice(&maturity_page.id().bytes());
    bytes[120..152].copy_from_slice(&closure.id()?.bytes());
    bytes[152..184].copy_from_slice(&seal.id()?.bytes());
    bytes[184..216].copy_from_slice(&authenticated_work.id().bytes());
    let evidence_id = domain_id(WINDOW_EVIDENCE_DOMAIN, &bytes);
    Ok(AuthenticatedWindowEvidenceV1 {
        route_id: route.route_id(),
        source_spec_id: window.source_spec_id,
        source_plane_contract_id: window.source_plane_program_id,
        window_id: window.id()?,
        repair_generation: window.repair_generation,
        closure,
        seal,
        evidence_id,
    })
}

/// Re-authenticate the durable action-8 WindowSeal as evaluation evidence.
///
/// The ephemeral page/work receipt used to create the seal intentionally does
/// not cross transactions. The immutable seal carries every field needed to
/// reconstruct its canonical closure receipt; matching that reconstructed
/// receipt to `closure_receipt_id`, the content-addressed seal account, the
/// exact release, and a mature Clock snapshot is the durable action-9
/// authority.
pub fn authenticate_persisted_window_evidence(
    route: AuthenticatedSourceRouteV1,
    source_plane: &SourcePlaneProgramV3,
    clock_policy: &ClockPolicyV1,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    authenticated_seal: AuthenticatedWindowSealAccountV1,
) -> Result<AuthenticatedWindowEvidenceV1> {
    source_plane.validate()?;
    validate_window_route(route, window)?;
    if source_plane.id()? != route.source_plane_contract_id()
        || clock_policy.id()? != route.clock_policy_id()
        || clock.unix_timestamp < clock_policy.bucket_timestamp(window.maturity_bucket_exclusive)?
    {
        return Err(Error::MismatchedBinding);
    }
    let seal = authenticated_seal.seal();
    let closure = reconstruct_persisted_window_closure(window, &seal)?;
    seal.validate_against(window)?;
    let mut bytes = [0_u8; 208];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&route.release_authentication_id().bytes());
    bytes[64..96].copy_from_slice(&authenticated_seal.id().bytes());
    bytes[96..128].copy_from_slice(&closure.id()?.bytes());
    bytes[128..160].copy_from_slice(&seal.id()?.bytes());
    bytes[160..192].copy_from_slice(&window.id()?.bytes());
    bytes[192..200].copy_from_slice(&clock.slot.to_le_bytes());
    bytes[200..208].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    Ok(AuthenticatedWindowEvidenceV1 {
        route_id: route.route_id(),
        source_spec_id: window.source_spec_id,
        source_plane_contract_id: window.source_plane_program_id,
        window_id: window.id()?,
        repair_generation: window.repair_generation,
        closure,
        seal,
        evidence_id: domain_id(PERSISTED_WINDOW_EVIDENCE_DOMAIN, &bytes),
    })
}

fn reconstruct_persisted_window_closure(
    window: &WindowSpecV3,
    seal: &WindowSealV3,
) -> Result<WindowClosureReceiptV3> {
    let closure = WindowClosureReceiptV3 {
        source_plane_program_id: window.source_plane_program_id,
        source_spec_id: window.source_spec_id,
        maturity_page_id: seal.last_page_id,
        sealed_boundary_bucket: seal.sealed_boundary_bucket,
        repair_generation: window.repair_generation,
    };
    closure.validate_against(window, seal.last_page_id, seal.sealed_boundary_bucket)?;
    if closure.id()? != seal.closure_receipt_id {
        return Err(Error::MismatchedBinding);
    }
    Ok(closure)
}

/// Reviewed evaluator deployment plus its exact summary-program semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationReleaseBindingV1 {
    /// Reviewed evaluator executable and ProgramData.
    pub deployment: DeploymentBindingV1,
    /// Exact SummaryProgram V3 identity implemented by that release.
    pub summary_program_id: ContentId,
}

impl EvaluationReleaseBindingV1 {
    /// Validate the complete reviewed evaluator release selection.
    pub fn validate(&self) -> Result<()> {
        self.deployment.validate()?;
        live_id(self.summary_program_id)
    }

    /// Content identity selected by the immutable Source release.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut bytes = [0; 64];
        bytes[..32].copy_from_slice(&self.deployment.id()?.bytes());
        bytes[32..].copy_from_slice(&self.summary_program_id.bytes());
        Ok(domain_id(EVALUATION_RELEASE_DOMAIN, &bytes))
    }
}

/// Runtime-authenticated evaluator authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationAuthorityV1 {
    deployment: DeploymentBindingV1,
    deployment_id: ContentId,
    summary_program: SummaryProgramV3,
    authority_id: ContentId,
}

impl EvaluationAuthorityV1 {
    /// Exact evaluator program.
    pub const fn evaluator_program(self) -> RuntimeKey {
        self.deployment.program
    }

    /// Reviewed executable/ProgramData release identity.
    pub const fn deployment_id(self) -> ContentId {
        self.deployment_id
    }

    /// Exact source-neutral SummaryProgram.
    pub const fn summary_program(self) -> SummaryProgramV3 {
        self.summary_program
    }

    /// Authentication identity of release plus SummaryProgram.
    pub const fn id(self) -> ContentId {
        self.authority_id
    }
}

/// Authenticate evaluator executable/ProgramData bytes and SummaryProgram identity.
pub fn authenticate_evaluation_authority(
    route: AuthenticatedSourceRouteV1,
    binding: EvaluationReleaseBindingV1,
    summary_program: SummaryProgramV3,
    program: RuntimeAccountViewV1<'_>,
    programdata: RuntimeAccountViewV1<'_>,
) -> Result<EvaluationAuthorityV1> {
    if binding.id()? != route.evaluation_release_id() {
        return Err(Error::MismatchedBinding);
    }
    summary_program.validate()?;
    if summary_program.id()? != binding.summary_program_id {
        return Err(Error::MismatchedBinding);
    }
    let deployment_id = binding.deployment.authenticate(program, programdata)?;
    let mut bytes = [0; 64];
    bytes[..32].copy_from_slice(&deployment_id.bytes());
    bytes[32..].copy_from_slice(&binding.summary_program_id.bytes());
    Ok(EvaluationAuthorityV1 {
        deployment: binding.deployment,
        deployment_id,
        summary_program,
        authority_id: domain_id(EVALUATION_AUTHORITY_DOMAIN, &bytes),
    })
}

/// Exact reviewed evaluator output bound to raw evidence and Clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEvaluationV1 {
    authority_id: ContentId,
    window_evidence_id: ContentId,
    statistic_key_id: ContentId,
    result: StatisticResultV3,
    evaluation_id: ContentId,
}

/// Runtime- and lineage-authenticated never-created StatisticResult slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedStatisticResultAbsenceV1 {
    statistic_key_id: ContentId,
    result_account: RuntimeKey,
    lineage_id: ContentId,
    absence_id: ContentId,
}

/// Runtime-authenticated persisted immutable StatisticResult generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedStatisticResultAccountV1 {
    route_id: ContentId,
    account: RuntimeKey,
    account_data_id: ContentId,
    header: RuntimeAccountHeaderV1,
    statistic_key_id: ContentId,
    window_evidence_id: ContentId,
    result: StatisticResultV3,
    summary_program_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedStatisticResultAccountV1 {
    /// Exact authenticated Source route owning this result account.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical predictable result account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete globally tagged account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact persisted account header and reopen generation.
    pub const fn header(self) -> RuntimeAccountHeaderV1 {
        self.header
    }

    /// Predictable StatisticKey bound to the physical result slot.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Exact authenticated Window evidence evaluated into the result.
    pub const fn window_evidence_id(self) -> ContentId {
        self.window_evidence_id
    }

    /// Canonical successful or refused StatisticResult.
    pub const fn result(self) -> StatisticResultV3 {
        self.result
    }

    /// Exact SummaryProgram whose semantics the persisted result satisfies.
    pub const fn summary_program_id(self) -> ContentId {
        self.summary_program_id
    }

    /// Complete owner/PDA/body/evaluation/lineage authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

impl AuthenticatedStatisticResultAbsenceV1 {
    /// Predictable StatisticKey whose result is absent.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Predictable unallocated result account.
    pub const fn result_account(self) -> RuntimeKey {
        self.result_account
    }

    /// Exact never-created lineage state.
    pub const fn lineage_id(self) -> ContentId {
        self.lineage_id
    }

    /// Complete runtime absence receipt.
    pub const fn id(self) -> ContentId {
        self.absence_id
    }
}

/// Authenticate an unallocated predictable result PDA plus never-created lineage.
pub fn authenticate_statistic_result_absence(
    route: AuthenticatedSourceRouteV1,
    key: &StatisticKeyV3,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedStatisticResultAbsenceV1> {
    key.validate()?;
    if authenticated_lineage.access() != LineageAccessV1::ReadOnly {
        return Err(Error::WrongPrivilege);
    }
    let lineage = authenticated_lineage.lineage();
    lineage.validate()?;
    let key_id = key.id()?;
    let recipe = PdaRecipeV3::statistic_result(key_id)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    if account.owner != route.system_program()
        || account.lamports != 0
        || !account.data.is_empty()
        || account.executable
        || account.signer
        || account.writable
    {
        return Err(Error::MismatchedBinding);
    }
    if lineage.adapter_program != route.adapter_program()
        || lineage.family != LineageFamilyV1::StatisticResult
        || lineage.semantic_binding_id != key_id
        || lineage.latest_generation != 0
        || lineage.is_open
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    let lineage_id = lineage.id()?;
    let mut bytes = [0; 168];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&key_id.bytes());
    bytes[64..96].copy_from_slice(&account.key.bytes());
    bytes[96..128].copy_from_slice(&lineage_id.bytes());
    bytes[128..160].copy_from_slice(&authenticated_lineage.id().bytes());
    bytes[160] = derived_pda.bump;
    Ok(AuthenticatedStatisticResultAbsenceV1 {
        statistic_key_id: key_id,
        result_account: account.key,
        lineage_id,
        absence_id: domain_id(RESULT_ABSENCE_DOMAIN, &bytes),
    })
}

impl AuthenticatedEvaluationV1 {
    /// Exact evaluator authority identity.
    pub const fn authority_id(self) -> ContentId {
        self.authority_id
    }

    /// Exact authenticated Window evidence.
    pub const fn window_evidence_id(self) -> ContentId {
        self.window_evidence_id
    }

    /// Predictable StatisticKey.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Canonical evaluator result.
    pub const fn result(self) -> StatisticResultV3 {
        self.result
    }

    /// Complete authentication identity.
    pub const fn id(self) -> ContentId {
        self.evaluation_id
    }
}

/// Authenticate exact returned result bytes against release, WindowSeal, and key.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_statistic_result(
    authority: EvaluationAuthorityV1,
    clock: ClockSnapshotV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    evidence: AuthenticatedWindowEvidenceV1,
    returned_result_bytes: &[u8],
    invocation: AdapterInvocationV1,
) -> Result<AuthenticatedEvaluationV1> {
    if returned_result_bytes.len() != STATISTIC_RESULT_BYTES {
        return Err(Error::InvalidCodec);
    }
    let result = StatisticResultV3::decode(returned_result_bytes)?;
    result.validate_against(key, &authority.summary_program, &evidence.seal(), window)?;
    invocation.validate()?;
    if invocation.invoked_program != authority.evaluator_program()
        || invocation.return_data_id != result.id()?
        || key.summary_program_id != authority.summary_program.id()?
        || evidence.window_id() != window.id()?
        || evidence.source_spec_id() != window.source_spec_id
        || evidence.repair_generation() != window.repair_generation
    {
        return Err(Error::WrongInvocation);
    }
    let mut bytes = [0; 184];
    bytes[..32].copy_from_slice(&authority.id().bytes());
    bytes[32..64].copy_from_slice(&evidence.id().bytes());
    bytes[64..96].copy_from_slice(&key.id()?.bytes());
    bytes[96..128].copy_from_slice(&result.id()?.bytes());
    bytes[128..160].copy_from_slice(&invocation.id()?.bytes());
    bytes[160..168].copy_from_slice(&clock.slot.to_le_bytes());
    bytes[168..176].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    Ok(AuthenticatedEvaluationV1 {
        authority_id: authority.id(),
        window_evidence_id: evidence.id(),
        statistic_key_id: key.id()?,
        result,
        evaluation_id: domain_id(EVALUATION_DOMAIN, &bytes),
    })
}

/// Authenticate one persisted result account from program ownership, exact
/// PDA/body/lineage, and its complete Window/Summary semantics.
///
/// The evaluator invocation is required when the account is first written.
/// Later instructions consume this durable account receipt rather than
/// pretending an ephemeral CPI return receipt survived across transactions.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_statistic_result_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    evidence: AuthenticatedWindowEvidenceV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedStatisticResultAccountV1> {
    authenticate_statistic_result_account_inner(
        route,
        account,
        derived_pda,
        window,
        key,
        summary.id()?,
        Some(summary),
        evidence,
        authenticated_lineage,
    )
}

/// Authenticate a durable result for action 10 using the SummaryProgram
/// identity already authenticated by the immutable Failure/Product policy.
/// The full SummaryProgram body and evaluator release were consumed by the
/// action-9 writer and are not caller inputs at this later boundary.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_persisted_statistic_result_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    authenticated_summary_program_id: ContentId,
    evidence: AuthenticatedWindowEvidenceV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedStatisticResultAccountV1> {
    authenticate_statistic_result_account_inner(
        route,
        account,
        derived_pda,
        window,
        key,
        authenticated_summary_program_id,
        None,
        evidence,
        authenticated_lineage,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_statistic_result_account_inner(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    summary_program_id: ContentId,
    summary: Option<&SummaryProgramV3>,
    evidence: AuthenticatedWindowEvidenceV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedStatisticResultAccountV1> {
    require_immutable_adapter_account(route, account)?;
    if authenticated_lineage.access() != LineageAccessV1::ReadOnly {
        return Err(Error::WrongPrivilege);
    }
    validate_window_route(route, window)?;
    key.validate()?;
    let key_id = key.id()?;
    let (header, result) =
        decode_runtime_account::<StatisticResultV3>(account.data, route.neutral_sink())?;
    match summary {
        Some(summary) => result.validate_against(key, summary, &evidence.seal(), window)?,
        None => result.validate_persisted_against(
            key,
            summary_program_id,
            &evidence.seal(),
            window,
        )?,
    }
    if evidence.route_id() != route.route_id()
        || evidence.window_id() != window.id()?
        || evidence.source_spec_id() != window.source_spec_id
        || evidence.source_plane_contract_id() != window.source_plane_program_id
        || evidence.repair_generation() != window.repair_generation
        || key.window_id != window.id()?
        || key.summary_program_id != summary_program_id
        || result.statistic_key_id() != key_id
        || result.window_seal_id() != evidence.seal().id()?
    {
        return Err(Error::MismatchedBinding);
    }
    let recipe = PdaRecipeV3::statistic_result(key_id)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        header.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let lineage = authenticated_lineage.lineage();
    lineage.validate()?;
    if lineage.adapter_program != route.adapter_program()
        || lineage.family != LineageFamilyV1::StatisticResult
        || lineage.semantic_binding_id != key_id
        || !lineage.is_open
        || lineage.active_account != account.key
        || lineage.latest_generation != header.generation
        || lineage.last_opened_state_id != account_data_id
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    let mut bytes = [0_u8; 240];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&result.id()?.bytes());
    bytes[128..160].copy_from_slice(&summary_program_id.bytes());
    bytes[160..192].copy_from_slice(&evidence.id().bytes());
    bytes[192..224].copy_from_slice(&authenticated_lineage.id().bytes());
    bytes[224..232].copy_from_slice(&header.generation.to_le_bytes());
    bytes[232] = header.bump;
    Ok(AuthenticatedStatisticResultAccountV1 {
        route_id: route.route_id(),
        account: account.key,
        account_data_id,
        header,
        statistic_key_id: key_id,
        window_evidence_id: evidence.id(),
        result,
        summary_program_id,
        authentication_id: domain_id(RESULT_ACCOUNT_AUTH_DOMAIN, &bytes),
    })
}

/// Whether the Product/Series component was atomically created or read exact-existing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OccurrenceDispositionV1 {
    /// Canonical occurrence account was created in this transaction.
    Created = 1,
    /// Exact independently existing canonical occurrence was authenticated.
    ExactExisting = 2,
}

/// Private-field runtime receipt joining Product/Series provenance to one
/// exact Source Window before a StatisticKey body is needed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceWindowReceiptV1 {
    route_id: ContentId,
    clock_policy_id: ContentId,
    occurrence_record_id: ContentId,
    series_plan_id: ContentId,
    ordinal: u32,
    market_instance_id: ContentId,
    attachment_plan_id: ContentId,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
    repair_generation: u64,
    disposition: OccurrenceDispositionV1,
    occurrence_account: RuntimeKey,
    occurrence_account_authentication_id: ContentId,
    join_id: ContentId,
}

impl OccurrenceWindowReceiptV1 {
    /// Complete authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Product/Series-owned occurrence record identity.
    pub const fn occurrence_record_id(self) -> ContentId {
        self.occurrence_record_id
    }

    /// Predictable WindowKey authenticated against the supplied body.
    pub const fn window_id(self) -> ContentId {
        self.window_id
    }

    /// Predictable StatisticKey identity committed by Product compilation.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Exact selected repair generation.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// Created versus exact-existing disposition.
    pub const fn disposition(self) -> OccurrenceDispositionV1 {
        self.disposition
    }

    /// Physical Product/Series occurrence account authenticated by this join.
    pub const fn occurrence_account(self) -> RuntimeKey {
        self.occurrence_account
    }

    /// Exact occurrence-account owner/PDA/body authentication.
    pub const fn occurrence_account_authentication_id(self) -> ContentId {
        self.occurrence_account_authentication_id
    }

    /// Complete Product/Source Window join identity.
    pub const fn id(self) -> ContentId {
        self.join_id
    }
}

/// Private-field runtime receipt joining Product/Series provenance to SourcePlane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceSourceReceiptV1 {
    route_id: ContentId,
    clock_policy_id: ContentId,
    occurrence_record_id: ContentId,
    series_plan_id: ContentId,
    ordinal: u32,
    market_instance_id: ContentId,
    attachment_plan_id: ContentId,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
    repair_generation: u64,
    disposition: OccurrenceDispositionV1,
    occurrence_account: RuntimeKey,
    occurrence_account_authentication_id: ContentId,
    join_id: ContentId,
}

impl OccurrenceSourceReceiptV1 {
    /// Complete authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Exact Clock/bucket policy selected by the source route.
    pub const fn clock_policy_id(self) -> ContentId {
        self.clock_policy_id
    }

    /// Product/Series-owned occurrence record identity.
    pub const fn occurrence_record_id(self) -> ContentId {
        self.occurrence_record_id
    }

    /// Exact SeriesPlanV5 identity.
    pub const fn series_plan_id(self) -> ContentId {
        self.series_plan_id
    }

    /// Exact finite Series ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Economic MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> ContentId {
        self.market_instance_id
    }

    /// Exact operational attachment plan.
    pub const fn attachment_plan_id(self) -> ContentId {
        self.attachment_plan_id
    }

    /// Exact SourcePlane contract.
    pub const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    /// Existing SourceSpec identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Predictable WindowKey.
    pub const fn window_id(self) -> ContentId {
        self.window_id
    }

    /// Predictable StatisticKey.
    pub const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }

    /// Exact selected repair generation.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// Created versus exact-existing disposition.
    pub const fn disposition(self) -> OccurrenceDispositionV1 {
        self.disposition
    }

    /// Physical Product/Series occurrence account authenticated by this join.
    pub const fn occurrence_account(self) -> RuntimeKey {
        self.occurrence_account
    }

    /// Exact created/existing occurrence-account authentication receipt.
    pub const fn occurrence_account_authentication_id(self) -> ContentId {
        self.occurrence_account_authentication_id
    }

    /// Complete Product/Source runtime join identity.
    pub const fn id(self) -> ContentId {
        self.join_id
    }
}

/// Validate and identify the exact Product/Series-owned 184-byte codec.
pub fn source_occurrence_record_id(input: &[u8]) -> Result<ContentId> {
    let occurrence = CompiledSourceOccurrenceV3::decode(input).map_err(|_| Error::InvalidCodec)?;
    let id = occurrence.id().map_err(|_| Error::InvalidCodec)?;
    Ok(ContentId::from_bytes(id.bytes()))
}

fn occurrence_join_preimage(receipt: OccurrenceWindowReceiptV1) -> [u8; 376] {
    let mut bytes = [0; 376];
    bytes[..32].copy_from_slice(&receipt.occurrence_record_id.bytes());
    bytes[32..64].copy_from_slice(&receipt.series_plan_id.bytes());
    bytes[64..68].copy_from_slice(&receipt.ordinal.to_le_bytes());
    bytes[72..104].copy_from_slice(&receipt.market_instance_id.bytes());
    bytes[104..136].copy_from_slice(&receipt.attachment_plan_id.bytes());
    bytes[136..168].copy_from_slice(&receipt.source_plane_contract_id.bytes());
    bytes[168..200].copy_from_slice(&receipt.source_spec_id.bytes());
    bytes[200..232].copy_from_slice(&receipt.window_id.bytes());
    bytes[232..264].copy_from_slice(&receipt.statistic_key_id.bytes());
    bytes[264..272].copy_from_slice(&receipt.repair_generation.to_le_bytes());
    bytes[272] = receipt.disposition as u8;
    bytes[280..312].copy_from_slice(&receipt.occurrence_account_authentication_id.bytes());
    bytes[312..344].copy_from_slice(&receipt.route_id.bytes());
    bytes[344..376].copy_from_slice(&receipt.clock_policy_id.bytes());
    bytes
}

/// Join the exact Product/Series-owned 184-byte codec to one authenticated
/// WindowSpec without accepting an unneeded caller StatisticKey body.
pub fn join_source_occurrence_window(
    route: AuthenticatedSourceRouteV1,
    occurrence_account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    disposition: OccurrenceDispositionV1,
    window: &WindowSpecV3,
) -> Result<OccurrenceWindowReceiptV1> {
    let occurrence = CompiledSourceOccurrenceV3::decode(occurrence_account.data)
        .map_err(|_| Error::InvalidCodec)?;
    let occurrence_record_id =
        ContentId::from_bytes(occurrence.id().map_err(|_| Error::InvalidCodec)?.bytes());
    if occurrence_account.owner != route.generation_authority_program() {
        return Err(Error::WrongOwner);
    }
    if occurrence_account.executable
        || occurrence_account.signer
        || (disposition == OccurrenceDispositionV1::Created) != occurrence_account.writable
    {
        return Err(Error::WrongPrivilege);
    }
    validate_window_route(route, window)?;
    let series_plan_id = ContentId::from_bytes(occurrence.series_plan_id.bytes());
    let ordinal = occurrence.ordinal;
    let market_instance_id = ContentId::from_bytes(occurrence.market_instance_id.bytes());
    let attachment_plan_id = ContentId::from_bytes(occurrence.attachment_plan_id.bytes());
    let source_window_id = ContentId::from_bytes(occurrence.source_window_id.bytes());
    let statistic_key_id = ContentId::from_bytes(occurrence.statistic_key_id.bytes());
    if source_window_id != window.id()? {
        return Err(Error::MismatchedBinding);
    }
    derived_pda.validate_for(
        route.generation_authority_program(),
        occurrence_record_id,
        occurrence_account.key,
        derived_pda.bump,
    )?;
    let occurrence_account_data_id =
        account_data_id(occurrence_account.key, occurrence_account.data)?;
    let mut account_auth_bytes = [0; 104];
    account_auth_bytes[..32].copy_from_slice(&occurrence_account.key.bytes());
    account_auth_bytes[32..64].copy_from_slice(&occurrence_account_data_id.bytes());
    account_auth_bytes[64..96].copy_from_slice(&occurrence_record_id.bytes());
    account_auth_bytes[96] = derived_pda.bump;
    let occurrence_account_authentication_id = domain_id(
        b"dragons-clutch/authenticated-source-occurrence-account/v1",
        &account_auth_bytes,
    );
    let mut receipt = OccurrenceWindowReceiptV1 {
        route_id: route.route_id(),
        clock_policy_id: route.clock_policy_id(),
        occurrence_record_id,
        series_plan_id,
        ordinal,
        market_instance_id,
        attachment_plan_id,
        source_plane_contract_id: route.source_plane_contract_id(),
        source_spec_id: route.source_spec_id(),
        window_id: source_window_id,
        statistic_key_id,
        repair_generation: window.repair_generation,
        disposition,
        occurrence_account: occurrence_account.key,
        occurrence_account_authentication_id,
        join_id: ContentId::ZERO,
    };
    let bytes = occurrence_join_preimage(receipt);
    receipt.join_id = domain_id(
        b"dragons-clutch/source-occurrence-window-join/v1",
        &bytes,
    );
    Ok(receipt)
}

/// Join the exact Product/Series-owned occurrence and Window receipt to the
/// complete predictable StatisticKey body without persisting a parallel DTO.
pub fn join_source_occurrence(
    route: AuthenticatedSourceRouteV1,
    occurrence_account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    disposition: OccurrenceDispositionV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
) -> Result<OccurrenceSourceReceiptV1> {
    let receipt = join_source_occurrence_window(
        route,
        occurrence_account,
        derived_pda,
        disposition,
        window,
    )?;
    key.validate()?;
    if receipt.statistic_key_id != key.id()? || key.window_id != receipt.window_id {
        return Err(Error::MismatchedBinding);
    }
    let bytes = occurrence_join_preimage(receipt);
    Ok(OccurrenceSourceReceiptV1 {
        route_id: receipt.route_id,
        clock_policy_id: receipt.clock_policy_id,
        occurrence_record_id: receipt.occurrence_record_id,
        series_plan_id: receipt.series_plan_id,
        ordinal: receipt.ordinal,
        market_instance_id: receipt.market_instance_id,
        attachment_plan_id: receipt.attachment_plan_id,
        source_plane_contract_id: receipt.source_plane_contract_id,
        source_spec_id: receipt.source_spec_id,
        window_id: receipt.window_id,
        statistic_key_id: receipt.statistic_key_id,
        repair_generation: receipt.repair_generation,
        disposition: receipt.disposition,
        occurrence_account: receipt.occurrence_account,
        occurrence_account_authentication_id: receipt.occurrence_account_authentication_id,
        join_id: domain_id(OCCURRENCE_JOIN_DOMAIN, &bytes),
    })
}

/// Source-owned failure facts accepted by the failure-policy runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceFailureKindV1 {
    /// Immutable primary maturity passed with no accepted result account.
    PrimaryMaturityWithoutAcceptedResolution = 1,
    /// Reviewed evaluator returned exact stable refusal content.
    SourceEvaluationRefused = 2,
}

/// Exact successful source evaluation offered to a downstream relation policy.
///
/// This receipt proves only source-owned facts. It deliberately contains no
/// relation-refusal class, relation-policy identity, payout choice, or recovery
/// transition. Those remain owned by the failure/relation runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuccessfulEvaluationHandoffV1 {
    failure_policy_binding_id: ContentId,
    occurrence: OccurrenceSourceReceiptV1,
    window_evidence_id: ContentId,
    result_account_authentication_id: ContentId,
    result: StatisticResultV3,
    clock_policy_id: ContentId,
    clock: ClockSnapshotV1,
    handoff_id: ContentId,
}

impl SuccessfulEvaluationHandoffV1 {
    /// Bind one successful authenticated evaluation at or after immutable maturity.
    #[allow(clippy::too_many_arguments)]
    pub fn at_maturity(
        failure_policy_binding_id: ContentId,
        occurrence: OccurrenceSourceReceiptV1,
        clock_policy: &ClockPolicyV1,
        clock: ClockSnapshotV1,
        window: &WindowSpecV3,
        evidence: AuthenticatedWindowEvidenceV1,
        result_account: AuthenticatedStatisticResultAccountV1,
    ) -> Result<Self> {
        live_id(failure_policy_binding_id)?;
        let clock_policy_id = clock_policy.id()?;
        let window_id = window.id()?;
        let result = result_account.result();
        let result_id = result.id()?;
        if result.status() != StatisticResultStatusV3::Success
            || result.refusal_code() != 0
            || occurrence.window_id() != window_id
            || occurrence.window_id() != evidence.window_id()
            || occurrence.route_id() != evidence.route_id()
            || occurrence.route_id() != result_account.route_id()
            || occurrence.statistic_key_id() != result_account.statistic_key_id()
            || occurrence.source_spec_id() != window.source_spec_id
            || occurrence.source_spec_id() != evidence.source_spec_id()
            || occurrence.source_plane_contract_id() != window.source_plane_program_id
            || occurrence.source_plane_contract_id() != evidence.source_plane_contract_id()
            || occurrence.repair_generation() != window.repair_generation
            || occurrence.repair_generation() != evidence.repair_generation()
            || occurrence.clock_policy_id() != clock_policy_id
            || result_account.window_evidence_id() != evidence.id()
            || result.statistic_key_id() != occurrence.statistic_key_id()
            || result.window_seal_id() != evidence.seal().id()?
            || clock.unix_timestamp
                < clock_policy.bucket_timestamp(window.maturity_bucket_exclusive)?
        {
            return Err(Error::InvalidFailureHandoff);
        }
        let mut bytes = [0; 272];
        bytes[..32].copy_from_slice(&failure_policy_binding_id.bytes());
        bytes[32..64].copy_from_slice(&occurrence.id().bytes());
        bytes[64..96].copy_from_slice(&evidence.id().bytes());
        bytes[96..128].copy_from_slice(&result_account.id().bytes());
        bytes[128..160].copy_from_slice(&result_account.summary_program_id().bytes());
        bytes[160..192].copy_from_slice(&result_id.bytes());
        bytes[192..224].copy_from_slice(&window_id.bytes());
        bytes[224..256].copy_from_slice(&clock_policy_id.bytes());
        bytes[256..264].copy_from_slice(&clock.slot.to_le_bytes());
        bytes[264..272].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
        Ok(Self {
            failure_policy_binding_id,
            occurrence,
            window_evidence_id: evidence.id(),
            result_account_authentication_id: result_account.id(),
            result,
            clock_policy_id,
            clock,
            handoff_id: domain_id(SUCCESS_HANDOFF_DOMAIN, &bytes),
        })
    }

    /// Exact FailurePolicyBindingV1 identity this source fact may be offered to.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }

    /// Product/Series occurrence provenance fixed before evaluation.
    pub const fn occurrence(self) -> OccurrenceSourceReceiptV1 {
        self.occurrence
    }

    /// Exact authenticated final Window evidence.
    pub const fn window_evidence_id(self) -> ContentId {
        self.window_evidence_id
    }

    /// Exact persisted result-account owner/PDA/body/Summary/lineage receipt.
    pub const fn result_account_authentication_id(self) -> ContentId {
        self.result_account_authentication_id
    }

    /// Canonical successful StatisticResult; its constructor provenance is this receipt.
    pub const fn result(self) -> StatisticResultV3 {
        self.result
    }

    /// Content identity of the successful StatisticResult.
    pub fn statistic_result_id(self) -> Result<ContentId> {
        self.result.id().map_err(Into::into)
    }

    /// Exact primary or repair Window identity evaluated.
    pub const fn window_id(self) -> ContentId {
        self.occurrence.window_id()
    }

    /// Frozen Clock policy used to interpret maturity.
    pub const fn clock_policy_id(self) -> ContentId {
        self.clock_policy_id
    }

    /// Adapter-supplied maturity Clock snapshot committed by this receipt.
    pub const fn clock(self) -> ClockSnapshotV1 {
        self.clock
    }

    /// Complete source-only handoff identity.
    pub const fn id(self) -> ContentId {
        self.handoff_id
    }
}

/// Source-owned physical-account authentication for one policy handoff.
///
/// This joins the semantic handoff to the exact release, occurrence, source
/// fact, and paid-work receipt accounts authenticated by the Source adapter.
/// It deliberately does not classify a downstream relation or select a
/// payout. Fields are private and construction consumes the private Source
/// authentication receipts, so downstream semantic owners cannot mint an
/// account-authenticated handoff from caller-supplied identity bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePolicyHandoffJoinV1 {
    handoff_id: ContentId,
    release_authentication_id: ContentId,
    route_id: ContentId,
    occurrence_account: RuntimeKey,
    result_or_absence_account: RuntimeKey,
    source_fact_authentication_id: ContentId,
    work_receipt_account: RuntimeKey,
    work_receipt_authentication_id: ContentId,
    clock_policy_id: ContentId,
    clock: ClockSnapshotV1,
    generation: u64,
    failure_policy_binding_id: ContentId,
    source_spec_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
    authentication_id: ContentId,
}

#[allow(clippy::too_many_arguments)]
fn source_policy_handoff_join_id(
    handoff_id: ContentId,
    release_authentication_id: ContentId,
    route_id: ContentId,
    occurrence_account: RuntimeKey,
    result_account: RuntimeKey,
    source_fact_authentication_id: ContentId,
    work_receipt_account: RuntimeKey,
    work_receipt_authentication_id: ContentId,
    clock_policy_id: ContentId,
    clock: ClockSnapshotV1,
    generation: u64,
    failure_policy_binding_id: ContentId,
    source_spec_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
) -> ContentId {
    let mut bytes = [0_u8; 440];
    bytes[..32].copy_from_slice(&handoff_id.bytes());
    bytes[32..64].copy_from_slice(&release_authentication_id.bytes());
    bytes[64..96].copy_from_slice(&route_id.bytes());
    bytes[96..128].copy_from_slice(&occurrence_account.bytes());
    bytes[128..160].copy_from_slice(&result_account.bytes());
    bytes[160..192].copy_from_slice(&source_fact_authentication_id.bytes());
    bytes[192..224].copy_from_slice(&work_receipt_account.bytes());
    bytes[224..256].copy_from_slice(&work_receipt_authentication_id.bytes());
    bytes[256..288].copy_from_slice(&clock_policy_id.bytes());
    bytes[288..296].copy_from_slice(&clock.slot.to_le_bytes());
    bytes[296..304].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    bytes[304..312].copy_from_slice(&generation.to_le_bytes());
    bytes[312..344].copy_from_slice(&failure_policy_binding_id.bytes());
    bytes[344..376].copy_from_slice(&source_spec_id.bytes());
    bytes[376..408].copy_from_slice(&window_id.bytes());
    bytes[408..440].copy_from_slice(&statistic_key_id.bytes());
    domain_id(POLICY_HANDOFF_JOIN_DOMAIN, &bytes)
}

impl SourcePolicyHandoffJoinV1 {
    /// Authenticate the complete physical join for a successful evaluation.
    pub fn successful_evaluation(
        route: AuthenticatedSourceRouteV1,
        handoff: SuccessfulEvaluationHandoffV1,
        result: AuthenticatedStatisticResultAccountV1,
        work_receipt: AuthenticatedSourceWorkReceiptV1,
    ) -> Result<Self> {
        if handoff.result_account_authentication_id() != result.id() {
            return Err(Error::MismatchedBinding);
        }
        Self::new(
            route,
            handoff.id(),
            handoff.failure_policy_binding_id(),
            handoff.occurrence(),
            result.account(),
            result.id(),
            handoff.clock(),
            work_receipt,
            SourceWorkKindV1::EvaluateStatistic,
            result.account_data_id(),
        )
    }

    /// Authenticate the complete physical join for a mature result absence.
    pub fn failure_absence(
        route: AuthenticatedSourceRouteV1,
        handoff: FailurePolicySourceHandoffV1,
        absence: AuthenticatedStatisticResultAbsenceV1,
        work_receipt: AuthenticatedSourceWorkReceiptV1,
    ) -> Result<Self> {
        if handoff.source_fact_receipt_id() != absence.id() {
            return Err(Error::MismatchedBinding);
        }
        Self::new(
            route,
            handoff.id(),
            handoff.failure_policy_binding_id(),
            handoff.occurrence(),
            absence.result_account(),
            absence.id(),
            handoff.clock(),
            work_receipt,
            SourceWorkKindV1::FailureHandoff,
            handoff.id(),
        )
    }

    /// Authenticate the complete physical join for a stable source refusal.
    pub fn failure_result(
        route: AuthenticatedSourceRouteV1,
        handoff: FailurePolicySourceHandoffV1,
        result: AuthenticatedStatisticResultAccountV1,
        work_receipt: AuthenticatedSourceWorkReceiptV1,
    ) -> Result<Self> {
        if handoff.source_fact_receipt_id() != result.id() {
            return Err(Error::MismatchedBinding);
        }
        Self::new(
            route,
            handoff.id(),
            handoff.failure_policy_binding_id(),
            handoff.occurrence(),
            result.account(),
            result.id(),
            handoff.clock(),
            work_receipt,
            SourceWorkKindV1::EvaluateStatistic,
            result.account_data_id(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        route: AuthenticatedSourceRouteV1,
        handoff_id: ContentId,
        failure_policy_binding_id: ContentId,
        occurrence: OccurrenceSourceReceiptV1,
        result_or_absence_account: RuntimeKey,
        source_fact_authentication_id: ContentId,
        clock: ClockSnapshotV1,
        work_receipt: AuthenticatedSourceWorkReceiptV1,
        expected_work_kind: SourceWorkKindV1,
        expected_semantic_receipt_id: ContentId,
    ) -> Result<Self> {
        let receipt = work_receipt.receipt();
        if occurrence.route_id() != route.route_id()
            || occurrence.source_spec_id() != route.source_spec_id()
            || occurrence.source_plane_contract_id() != route.source_plane_contract_id()
            || occurrence.clock_policy_id() != route.clock_policy_id()
            || receipt.route_id() != route.route_id()
            || receipt.disposition() != SourceReceiptDispositionV1::Work
            || receipt.work_kind() != Some(expected_work_kind)
            || receipt.semantic_receipt_id() != expected_semantic_receipt_id
        {
            return Err(Error::MismatchedBinding);
        }

        let release_authentication_id = route.release_authentication_id();
        let route_id = route.route_id();
        let occurrence_account = occurrence.occurrence_account();
        let work_receipt_account = work_receipt.account();
        let work_receipt_authentication_id = work_receipt.id();
        let clock_policy_id = occurrence.clock_policy_id();
        let generation = receipt.generation();
        let source_spec_id = occurrence.source_spec_id();
        let window_id = occurrence.window_id();
        let statistic_key_id = occurrence.statistic_key_id();
        let authentication_id = source_policy_handoff_join_id(
            handoff_id,
            release_authentication_id,
            route_id,
            occurrence_account,
            result_or_absence_account,
            source_fact_authentication_id,
            work_receipt_account,
            work_receipt_authentication_id,
            clock_policy_id,
            clock,
            generation,
            failure_policy_binding_id,
            source_spec_id,
            window_id,
            statistic_key_id,
        );
        Ok(Self {
            handoff_id,
            release_authentication_id,
            route_id,
            occurrence_account,
            result_or_absence_account,
            source_fact_authentication_id,
            work_receipt_account,
            work_receipt_authentication_id,
            clock_policy_id,
            clock,
            generation,
            failure_policy_binding_id,
            source_spec_id,
            window_id,
            statistic_key_id,
            authentication_id,
        })
    }

    /// Exact source-only semantic handoff identity persisted by the work receipt.
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

    /// Physical immutable work receipt account.
    pub const fn work_receipt_account(self) -> RuntimeKey {
        self.work_receipt_account
    }

    /// Exact owner/PDA/body/schedule authentication identity of the work receipt.
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

    /// Complete release/account/result/work authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Durable Source-owned record of one exact action-10 policy handoff.
///
/// Every field is copied only from the private [`SourcePolicyHandoffJoinV1`].
/// This account is content-addressed by that join and carries no payout or
/// downstream relation classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePolicyHandoffAccountV1 {
    handoff_id: ContentId,
    release_authentication_id: ContentId,
    route_id: ContentId,
    occurrence_account: RuntimeKey,
    result_account: RuntimeKey,
    source_fact_authentication_id: ContentId,
    work_receipt_account: RuntimeKey,
    work_receipt_authentication_id: ContentId,
    clock_policy_id: ContentId,
    clock: ClockSnapshotV1,
    generation: u64,
    failure_policy_binding_id: ContentId,
    source_spec_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
    source_policy_handoff_join_id: ContentId,
}

impl SourcePolicyHandoffAccountV1 {
    /// Project the sole canonical durable body from a private Source join.
    pub fn from_join(join: SourcePolicyHandoffJoinV1) -> Result<Self> {
        let value = Self {
            handoff_id: join.handoff_id(),
            release_authentication_id: join.release_authentication_id(),
            route_id: join.route_id(),
            occurrence_account: join.occurrence_account(),
            result_account: join.result_or_absence_account(),
            source_fact_authentication_id: join.source_fact_authentication_id(),
            work_receipt_account: join.work_receipt_account(),
            work_receipt_authentication_id: join.work_receipt_authentication_id(),
            clock_policy_id: join.clock_policy_id(),
            clock: join.clock(),
            generation: join.generation(),
            failure_policy_binding_id: join.failure_policy_binding_id(),
            source_spec_id: join.source_spec_id(),
            window_id: join.window_id(),
            statistic_key_id: join.statistic_key_id(),
            source_policy_handoff_join_id: join.id(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        for id in [
            self.handoff_id,
            self.release_authentication_id,
            self.route_id,
            self.source_fact_authentication_id,
            self.work_receipt_authentication_id,
            self.clock_policy_id,
            self.failure_policy_binding_id,
            self.source_spec_id,
            self.window_id,
            self.statistic_key_id,
            self.source_policy_handoff_join_id,
        ] {
            live_id(id)?;
        }
        self.occurrence_account.validate()?;
        self.result_account.validate()?;
        self.work_receipt_account.validate()?;
        if self.generation == 0
            || self.occurrence_account == self.result_account
            || self.occurrence_account == self.work_receipt_account
            || self.result_account == self.work_receipt_account
            || self.source_policy_handoff_join_id != self.expected_join_id()
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    fn expected_join_id(&self) -> ContentId {
        source_policy_handoff_join_id(
            self.handoff_id,
            self.release_authentication_id,
            self.route_id,
            self.occurrence_account,
            self.result_account,
            self.source_fact_authentication_id,
            self.work_receipt_account,
            self.work_receipt_authentication_id,
            self.clock_policy_id,
            self.clock,
            self.generation,
            self.failure_policy_binding_id,
            self.source_spec_id,
            self.window_id,
            self.statistic_key_id,
        )
    }

    /// Exact private Source join persisted by this body.
    pub const fn source_policy_handoff_join_id(self) -> ContentId {
        self.source_policy_handoff_join_id
    }

    /// Exact semantic successful/failure handoff identity.
    pub const fn handoff_id(self) -> ContentId {
        self.handoff_id
    }
}

impl FixedCodec for SourcePolicyHandoffAccountV1 {
    const ENCODED_LEN: usize = SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES;

    fn encode_into(
        &self,
        output: &mut [u8],
    ) -> core::result::Result<(), clutch_source_plane_v3::Error> {
        if output.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if output.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        self.validate()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        output.fill(0);
        output[..8].copy_from_slice(&SOURCE_POLICY_HANDOFF_MAGIC);
        output[8..10].copy_from_slice(&1_u16.to_le_bytes());
        let ids_and_keys = [
            self.handoff_id.bytes(),
            self.release_authentication_id.bytes(),
            self.route_id.bytes(),
            self.occurrence_account.bytes(),
            self.result_account.bytes(),
            self.source_fact_authentication_id.bytes(),
            self.work_receipt_account.bytes(),
            self.work_receipt_authentication_id.bytes(),
            self.clock_policy_id.bytes(),
            self.failure_policy_binding_id.bytes(),
            self.source_spec_id.bytes(),
            self.window_id.bytes(),
            self.statistic_key_id.bytes(),
            self.source_policy_handoff_join_id.bytes(),
        ];
        let mut at = 16_usize;
        for value in ids_and_keys {
            output[at..at + 32].copy_from_slice(&value);
            at += 32;
        }
        output[464..472].copy_from_slice(&self.clock.slot.to_le_bytes());
        output[472..480].copy_from_slice(&self.clock.unix_timestamp.to_le_bytes());
        output[480..488].copy_from_slice(&self.generation.to_le_bytes());
        Ok(())
    }

    fn decode(
        input: &[u8],
    ) -> core::result::Result<Self, clutch_source_plane_v3::Error> {
        if input.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if input.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        if input[..8] != SOURCE_POLICY_HANDOFF_MAGIC {
            return Err(clutch_source_plane_v3::Error::BadMagic);
        }
        if input[8..10] != 1_u16.to_le_bytes() {
            return Err(clutch_source_plane_v3::Error::BadVersion);
        }
        if input[10..16].iter().any(|byte| *byte != 0) {
            return Err(clutch_source_plane_v3::Error::NonCanonicalReserved);
        }
        let read_32 = |at: usize| {
            let mut value = [0_u8; 32];
            value.copy_from_slice(&input[at..at + 32]);
            value
        };
        let read_u64 = |at: usize| {
            let mut value = [0_u8; 8];
            value.copy_from_slice(&input[at..at + 8]);
            u64::from_le_bytes(value)
        };
        let value = Self {
            handoff_id: ContentId::from_bytes(read_32(16)),
            release_authentication_id: ContentId::from_bytes(read_32(48)),
            route_id: ContentId::from_bytes(read_32(80)),
            occurrence_account: RuntimeKey::from_bytes(read_32(112)),
            result_account: RuntimeKey::from_bytes(read_32(144)),
            source_fact_authentication_id: ContentId::from_bytes(read_32(176)),
            work_receipt_account: RuntimeKey::from_bytes(read_32(208)),
            work_receipt_authentication_id: ContentId::from_bytes(read_32(240)),
            clock_policy_id: ContentId::from_bytes(read_32(272)),
            failure_policy_binding_id: ContentId::from_bytes(read_32(304)),
            source_spec_id: ContentId::from_bytes(read_32(336)),
            window_id: ContentId::from_bytes(read_32(368)),
            statistic_key_id: ContentId::from_bytes(read_32(400)),
            source_policy_handoff_join_id: ContentId::from_bytes(read_32(432)),
            clock: ClockSnapshotV1 {
                slot: read_u64(464),
                unix_timestamp: read_u64(472),
            },
            generation: read_u64(480),
        };
        value
            .validate()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        Ok(value)
    }
}

/// Privilege mode for durable handoff authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePolicyHandoffAccessV1 {
    /// Same-instruction postwrite authentication.
    CreatedMutable,
    /// Later downstream consumption.
    ExistingReadOnly,
}

/// Private receipt authenticating the durable action-10 handoff postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPersistedSourcePolicyHandoffV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    body: SourcePolicyHandoffAccountV1,
    authentication_id: ContentId,
}

impl AuthenticatedPersistedSourcePolicyHandoffV1 {
    /// Physical content-addressed handoff account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of complete account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact persisted Source join identity.
    pub const fn source_policy_handoff_join_id(self) -> ContentId {
        self.body.source_policy_handoff_join_id()
    }

    /// Complete owner/PDA/body/postwrite authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one exact persisted handoff against the private live join.
pub fn authenticate_persisted_source_policy_handoff(
    route: AuthenticatedSourceRouteV1,
    join: SourcePolicyHandoffJoinV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    access: SourcePolicyHandoffAccessV1,
) -> Result<AuthenticatedPersistedSourcePolicyHandoffV1> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable
        || account.signer
        || account.writable != (access == SourcePolicyHandoffAccessV1::CreatedMutable)
    {
        return Err(Error::WrongPrivilege);
    }
    let body = SourcePolicyHandoffAccountV1::decode(account.data).map_err(Error::Core)?;
    let expected = SourcePolicyHandoffAccountV1::from_join(join)?;
    if body != expected || body.route_id != route.route_id() {
        return Err(Error::MismatchedBinding);
    }
    let recipe = PdaRecipeV3::source_policy_handoff(join.id())?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0_u8; 128];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&join.id().bytes());
    Ok(AuthenticatedPersistedSourcePolicyHandoffV1 {
        account: account.key,
        account_data_id,
        body,
        authentication_id: domain_id(PERSISTED_POLICY_HANDOFF_AUTH_DOMAIN, &bytes),
    })
}

/// Exact source half of a failure-policy binding; it never selects a payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailurePolicySourceHandoffV1 {
    kind: SourceFailureKindV1,
    failure_policy_binding_id: ContentId,
    occurrence: OccurrenceSourceReceiptV1,
    clock: ClockSnapshotV1,
    window_evidence_id: ContentId,
    statistic_result_id: ContentId,
    refusal_code: u32,
    absence_or_evaluation_receipt_id: ContentId,
    handoff_id: ContentId,
}

impl FailurePolicySourceHandoffV1 {
    /// Build an exact no-accepted-resolution maturity fact from adapter-authenticated absence.
    pub fn primary_maturity_without_resolution(
        failure_policy_binding_id: ContentId,
        occurrence: OccurrenceSourceReceiptV1,
        clock_policy: &ClockPolicyV1,
        clock: ClockSnapshotV1,
        window: &WindowSpecV3,
        absence: AuthenticatedStatisticResultAbsenceV1,
    ) -> Result<Self> {
        live_id(failure_policy_binding_id)?;
        if occurrence.window_id() != window.id()?
            || occurrence.source_spec_id() != window.source_spec_id
            || occurrence.repair_generation() != window.repair_generation
            || occurrence.statistic_key_id() != absence.statistic_key_id()
            || occurrence.clock_policy_id() != clock_policy.id()?
            || clock.unix_timestamp
                < clock_policy.bucket_timestamp(window.maturity_bucket_exclusive)?
        {
            return Err(Error::InvalidFailureHandoff);
        }
        Self::new(
            SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
            failure_policy_binding_id,
            occurrence,
            clock,
            ContentId::ZERO,
            ContentId::ZERO,
            0,
            absence.id(),
        )
    }

    /// Build an exact stable evaluator-refusal fact.
    pub fn source_evaluation_refused(
        failure_policy_binding_id: ContentId,
        occurrence: OccurrenceSourceReceiptV1,
        clock_policy: &ClockPolicyV1,
        clock: ClockSnapshotV1,
        window: &WindowSpecV3,
        evidence: AuthenticatedWindowEvidenceV1,
        result_account: AuthenticatedStatisticResultAccountV1,
    ) -> Result<Self> {
        live_id(failure_policy_binding_id)?;
        let result = result_account.result();
        if result.status() != StatisticResultStatusV3::Refused
            || result.refusal_code() == 0
            || occurrence.window_id() != evidence.window_id()
            || occurrence.route_id() != evidence.route_id()
            || occurrence.route_id() != result_account.route_id()
            || occurrence.statistic_key_id() != result_account.statistic_key_id()
            || occurrence.source_spec_id() != evidence.source_spec_id()
            || occurrence.source_plane_contract_id() != evidence.source_plane_contract_id()
            || occurrence.repair_generation() != evidence.repair_generation()
            || occurrence.window_id() != window.id()?
            || occurrence.clock_policy_id() != clock_policy.id()?
            || result_account.window_evidence_id() != evidence.id()
            || result.statistic_key_id() != occurrence.statistic_key_id()
            || result.window_seal_id() != evidence.seal().id()?
            || clock.unix_timestamp
                < clock_policy.bucket_timestamp(window.maturity_bucket_exclusive)?
        {
            return Err(Error::InvalidFailureHandoff);
        }
        Self::new(
            SourceFailureKindV1::SourceEvaluationRefused,
            failure_policy_binding_id,
            occurrence,
            clock,
            evidence.id(),
            result.id()?,
            result.refusal_code(),
            result_account.id(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        kind: SourceFailureKindV1,
        failure_policy_binding_id: ContentId,
        occurrence: OccurrenceSourceReceiptV1,
        clock: ClockSnapshotV1,
        window_evidence_id: ContentId,
        statistic_result_id: ContentId,
        refusal_code: u32,
        absence_or_evaluation_receipt_id: ContentId,
    ) -> Result<Self> {
        let mut bytes = [0; 224];
        bytes[0] = kind as u8;
        bytes[8..40].copy_from_slice(&failure_policy_binding_id.bytes());
        bytes[40..72].copy_from_slice(&occurrence.id().bytes());
        bytes[72..80].copy_from_slice(&clock.slot.to_le_bytes());
        bytes[80..88].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
        bytes[88..120].copy_from_slice(&window_evidence_id.bytes());
        bytes[120..152].copy_from_slice(&statistic_result_id.bytes());
        bytes[152..156].copy_from_slice(&refusal_code.to_le_bytes());
        bytes[160..192].copy_from_slice(&absence_or_evaluation_receipt_id.bytes());
        bytes[192..224].copy_from_slice(&occurrence.market_instance_id().bytes());
        Ok(Self {
            kind,
            failure_policy_binding_id,
            occurrence,
            clock,
            window_evidence_id,
            statistic_result_id,
            refusal_code,
            absence_or_evaluation_receipt_id,
            handoff_id: domain_id(FAILURE_HANDOFF_DOMAIN, &bytes),
        })
    }

    /// Exact source failure kind.
    pub const fn kind(self) -> SourceFailureKindV1 {
        self.kind
    }

    /// FailurePolicyBindingV1 identity.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }

    /// Exact occurrence provenance.
    pub const fn occurrence(self) -> OccurrenceSourceReceiptV1 {
        self.occurrence
    }

    /// Adapter-authenticated trigger Clock.
    pub const fn clock(self) -> ClockSnapshotV1 {
        self.clock
    }

    /// Window evidence identity, zero only for the no-resolution absence path.
    pub const fn window_evidence_id(self) -> ContentId {
        self.window_evidence_id
    }

    /// StatisticResult identity, zero only for the no-resolution absence path.
    pub const fn statistic_result_id(self) -> ContentId {
        self.statistic_result_id
    }

    /// Stable nonzero refusal code only for `SourceEvaluationRefused`.
    pub const fn refusal_code(self) -> u32 {
        self.refusal_code
    }

    /// Authenticated absence or evaluation receipt.
    pub const fn source_fact_receipt_id(self) -> ContentId {
        self.absence_or_evaluation_receipt_id
    }

    /// Complete handoff identity.
    pub const fn id(self) -> ContentId {
        self.handoff_id
    }
}

/// Authenticate an existing immutable WindowSeal account for client/failure joins.
pub fn authenticate_window_seal_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    window: &WindowSpecV3,
) -> Result<AuthenticatedWindowSealAccountV1> {
    require_immutable_adapter_account(route, account)?;
    validate_window_route(route, window)?;
    let (header, seal) =
        decode_runtime_account::<WindowSealV3>(account.data, route.neutral_sink())?;
    seal.validate_against(window)?;
    let recipe = PdaRecipeV3::window_seal(window.id()?)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        header.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0; 136];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&seal.id()?.bytes());
    bytes[128] = header.bump;
    Ok(AuthenticatedWindowSealAccountV1 {
        account: account.key,
        account_data_id,
        header,
        seal,
        authentication_id: domain_id(SEAL_ACCOUNT_AUTH_DOMAIN, &bytes),
    })
}

fn require_immutable_adapter_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
) -> Result<()> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable {
        return Err(Error::WrongExecutableState);
    }
    if account.signer || account.writable {
        return Err(Error::WrongPrivilege);
    }
    Ok(())
}

fn require_mutable_adapter_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
) -> Result<()> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable {
        return Err(Error::WrongExecutableState);
    }
    if account.signer || !account.writable {
        return Err(Error::WrongPrivilege);
    }
    Ok(())
}

fn validate_window_route(route: AuthenticatedSourceRouteV1, window: &WindowSpecV3) -> Result<()> {
    window.validate()?;
    if window.source_plane_program_id != route.source_plane_contract_id()
        || window.source_spec_id != route.source_spec_id()
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn window_work_state_id(work: &WindowWorkV3) -> Result<ContentId> {
    let mut bytes = [0; WINDOW_WORK_BYTES];
    work.encode_into(&mut bytes)?;
    Ok(domain_id(WORK_STATE_DOMAIN, &bytes))
}

const _: () = assert!(RAW_PAGE_BYTES == 2_152);
const _: () = assert!(WINDOW_SEAL_BYTES == 192);

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_source_plane_v3::COVERAGE_COMPLETE_REQUIRED;

    fn id(seed: u8) -> ContentId {
        ContentId::from_bytes([seed; 32])
    }

    fn window() -> WindowSpecV3 {
        WindowSpecV3 {
            source_spec_id: id(1),
            source_plane_program_id: id(2),
            start_bucket: 10,
            end_bucket_exclusive: 12,
            maturity_bucket_exclusive: 12,
            repair_generation: 3,
            coverage_policy_id: COVERAGE_COMPLETE_REQUIRED,
            coverage_policy_parameter: 0,
        }
    }

    fn seal(window: &WindowSpecV3) -> WindowSealV3 {
        let closure = WindowClosureReceiptV3 {
            source_plane_program_id: window.source_plane_program_id,
            source_spec_id: window.source_spec_id,
            maturity_page_id: id(4),
            sealed_boundary_bucket: 12,
            repair_generation: window.repair_generation,
        };
        WindowSealV3 {
            window_id: window.id().unwrap(),
            first_page_id: id(4),
            last_page_id: id(4),
            record_stream_root: id(5),
            closure_receipt_id: closure.id().unwrap(),
            sealed_boundary_bucket: 12,
            accepted_count: 2,
            gap_count: 0,
            evidence_page_count: 1,
        }
    }

    #[test]
    fn persisted_seal_reconstructs_exact_closure_and_rejects_forged_id() {
        let window = window();
        let mut seal = seal(&window);
        let closure = reconstruct_persisted_window_closure(&window, &seal).unwrap();
        assert_eq!(closure.maturity_page_id, seal.last_page_id);
        assert_eq!(closure.sealed_boundary_bucket, seal.sealed_boundary_bucket);

        seal.closure_receipt_id = id(9);
        assert_eq!(
            reconstruct_persisted_window_closure(&window, &seal),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn persisted_policy_handoff_codec_is_exact_and_hostile() {
        let body = SourcePolicyHandoffAccountV1 {
            handoff_id: id(1),
            release_authentication_id: id(2),
            route_id: id(3),
            occurrence_account: RuntimeKey::from_bytes([4; 32]),
            result_account: RuntimeKey::from_bytes([5; 32]),
            source_fact_authentication_id: id(6),
            work_receipt_account: RuntimeKey::from_bytes([7; 32]),
            work_receipt_authentication_id: id(8),
            clock_policy_id: id(9),
            clock: ClockSnapshotV1 {
                slot: 10,
                unix_timestamp: 11,
            },
            generation: 12,
            failure_policy_binding_id: id(13),
            source_spec_id: id(14),
            window_id: id(15),
            statistic_key_id: id(16),
            source_policy_handoff_join_id: ContentId::ZERO,
        };
        let body = SourcePolicyHandoffAccountV1 {
            source_policy_handoff_join_id: body.expected_join_id(),
            ..body
        };
        let mut bytes = [0_u8; SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES];
        body.encode_into(&mut bytes).unwrap();
        assert_eq!(SourcePolicyHandoffAccountV1::decode(&bytes), Ok(body));

        let mut hostile = bytes;
        hostile[10] = 1;
        assert_eq!(
            SourcePolicyHandoffAccountV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::NonCanonicalReserved)
        );
        assert_eq!(
            SourcePolicyHandoffAccountV1::decode(&bytes[..bytes.len() - 1]),
            Err(clutch_source_plane_v3::Error::Truncated)
        );
        let mut hostile = bytes;
        hostile[432] ^= 1;
        assert_eq!(
            SourcePolicyHandoffAccountV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::MismatchedArtifact)
        );
    }
}
