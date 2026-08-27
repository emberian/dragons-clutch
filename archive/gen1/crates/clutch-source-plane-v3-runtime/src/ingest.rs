use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, RawPageV3, SourceHeadV3, MAX_RAW_PAGE_RECORDS,
    OPEN_RAW_PAGE_BYTES,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;

use crate::account::{decode_runtime_account, RuntimeAccountHeaderV1};
use crate::auth::{
    account_data_id, domain_id, AuthenticatedBoundaryV1, AuthenticatedSourceRouteV1,
    RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::lineage::{AuthenticatedReopenLineageV1, LineageAccessV1, LineageFamilyV1};
use crate::{Error, Result};

const BATCH_DOMAIN: &[u8] = b"dragons-clutch/source-ingest-batch/v1";
const HEAD_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-head/v1";
const OPEN_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-open-raw-page/v1";
const OPEN_STATE_DOMAIN: &[u8] = b"dragons-clutch/open-raw-page-state/v1";
const INGEST_TRANSITION_DOMAIN: &[u8] = b"dragons-clutch/source-ingest-transition/v1";
const SEAL_OPEN_TRANSITION_DOMAIN: &[u8] = b"dragons-clutch/source-seal-open-page/v1";
const GENERATION_REQUEST_MAGIC: [u8; 8] = *b"DCSGEN01";
const GENERATION_REQUEST_DOMAIN: &[u8] = b"dragons-clutch/source-generation-request/v1";
const GENERATION_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-generation/v1";

/// Maximum authenticated boundaries admitted by one runtime call.
pub const MAX_BOUNDARIES_PER_INGEST: usize = 8;
/// Exact immutable generation-request bytes.
pub const SOURCE_GENERATION_REQUEST_BYTES: usize = 168;

/// Product/failure-owned immutable request for one source repair generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceGenerationRequestV1 {
    /// Exact reviewed SourcePlane contract.
    pub source_plane_contract_id: ContentId,
    /// Existing SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Exact repair generation; zero is the original generation.
    pub repair_generation: u64,
    /// First canonical boundary to ingest.
    pub first_bucket: u64,
    /// Exclusive last boundary required by the requesting lifecycle.
    pub required_end_bucket_exclusive: u64,
    /// Immutable Product/failure policy request identity.
    pub generation_policy_id: ContentId,
    /// Exact heterogeneous Source work schedule.
    pub source_work_schedule_id: ContentId,
}

impl SourceGenerationRequestV1 {
    /// Validate complete immutable generation coordinates.
    pub fn validate(&self) -> Result<()> {
        if self.source_plane_contract_id.is_zero()
            || self.source_spec_id.is_zero()
            || self.generation_policy_id.is_zero()
            || self.source_work_schedule_id.is_zero()
            || self.first_bucket >= self.required_end_bucket_exclusive
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exact canonical request bytes.
    pub fn encode(&self) -> Result<[u8; SOURCE_GENERATION_REQUEST_BYTES]> {
        self.validate()?;
        let mut out = [0; SOURCE_GENERATION_REQUEST_BYTES];
        out[..8].copy_from_slice(&GENERATION_REQUEST_MAGIC);
        out[8..10].copy_from_slice(&1_u16.to_le_bytes());
        out[16..48].copy_from_slice(&self.source_plane_contract_id.bytes());
        out[48..80].copy_from_slice(&self.source_spec_id.bytes());
        out[80..88].copy_from_slice(&self.repair_generation.to_le_bytes());
        out[88..96].copy_from_slice(&self.first_bucket.to_le_bytes());
        out[96..104].copy_from_slice(&self.required_end_bucket_exclusive.to_le_bytes());
        out[104..136].copy_from_slice(&self.generation_policy_id.bytes());
        out[136..168].copy_from_slice(&self.source_work_schedule_id.bytes());
        Ok(out)
    }

    /// Hostile-decode exact canonical request bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_GENERATION_REQUEST_BYTES
            || input[..8] != GENERATION_REQUEST_MAGIC
            || le_u16(&input[8..10]) != 1
            || input[10..16].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let value = Self {
            source_plane_contract_id: id_at(input, 16),
            source_spec_id: id_at(input, 48),
            repair_generation: le_u64(&input[80..88]),
            first_bucket: le_u64(&input[88..96]),
            required_end_bucket_exclusive: le_u64(&input[96..104]),
            generation_policy_id: id_at(input, 104),
            source_work_schedule_id: id_at(input, 136),
        };
        value.validate()?;
        Ok(value)
    }

    /// Content identity of the exact Product/failure request.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(GENERATION_REQUEST_DOMAIN, &self.encode()?))
    }
}

/// Runtime-authenticated initial/repair generation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceGenerationV1 {
    route_id: ContentId,
    request_account: RuntimeKey,
    request: SourceGenerationRequestV1,
    authorization_id: ContentId,
}

impl AuthenticatedSourceGenerationV1 {
    /// Exact authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Immutable Product/failure request account.
    pub const fn request_account(self) -> RuntimeKey {
        self.request_account
    }

    /// Complete canonical request body.
    pub const fn request(self) -> SourceGenerationRequestV1 {
        self.request
    }

    /// Complete account/owner/body authorization identity.
    pub const fn id(self) -> ContentId {
        self.authorization_id
    }
}

/// Authenticate an immutable Product/failure request account under the frozen authority program.
pub fn authenticate_source_generation_request(
    route: AuthenticatedSourceRouteV1,
    request_account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
) -> Result<AuthenticatedSourceGenerationV1> {
    if request_account.owner != route.generation_authority_program() {
        return Err(Error::WrongOwner);
    }
    if request_account.executable || request_account.signer || request_account.writable {
        return Err(Error::WrongPrivilege);
    }
    let request = SourceGenerationRequestV1::decode(request_account.data)?;
    if request.source_plane_contract_id != route.source_plane_contract_id()
        || request.source_spec_id != route.source_spec_id()
        || request.source_work_schedule_id != route.source_work_schedule_id()
    {
        return Err(Error::MismatchedBinding);
    }
    derived_pda.validate_for(
        route.generation_authority_program(),
        request.id()?,
        request_account.key,
        derived_pda.bump,
    )?;
    let data_id = account_data_id(request_account.key, request_account.data)?;
    let mut bytes = [0; 128];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&request_account.key.bytes());
    bytes[64..96].copy_from_slice(&data_id.bytes());
    bytes[96..128].copy_from_slice(&request.id()?.bytes());
    Ok(AuthenticatedSourceGenerationV1 {
        route_id: route.route_id(),
        request_account: request_account.key,
        request,
        authorization_id: domain_id(GENERATION_AUTH_DOMAIN, &bytes),
    })
}

/// Construct the exact initial SourceHead body from authenticated generation authority.
pub fn initialize_source_head(
    route: AuthenticatedSourceRouteV1,
    authorization: AuthenticatedSourceGenerationV1,
) -> Result<SourceHeadV3> {
    if authorization.route_id() != route.route_id() {
        return Err(Error::MismatchedBinding);
    }
    let request = authorization.request();
    SourceHeadV3::new(
        request.source_spec_id,
        request.first_bucket,
        request.repair_generation,
    )
    .map_err(Error::from)
}

/// Runtime-authenticated mutable SourceHead account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceHeadV1 {
    route_id: ContentId,
    account: RuntimeKey,
    terminal_generation: u64,
    header: RuntimeAccountHeaderV1,
    account_data_id: ContentId,
    head: SourceHeadV3,
    authentication_id: ContentId,
}

impl AuthenticatedSourceHeadV1 {
    /// Exact authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical SourceHead account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Durable terminal/reopen generation.
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

    /// Complete canonical SourceHead body.
    pub const fn head(self) -> SourceHeadV3 {
        self.head
    }

    /// Complete account/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Runtime-authenticated mutable OpenRawPage account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedOpenRawPageV1 {
    route_id: ContentId,
    account: RuntimeKey,
    terminal_generation: u64,
    header: RuntimeAccountHeaderV1,
    account_data_id: ContentId,
    open: OpenRawPageV3,
    authentication_id: ContentId,
}

impl AuthenticatedOpenRawPageV1 {
    /// Exact authenticated source route.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical OpenRawPage account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Durable terminal/reopen generation.
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

    /// Complete canonical OpenRawPage body.
    pub const fn open(self) -> OpenRawPageV3 {
        self.open
    }

    /// Complete account/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one SourceHead owner/PDA/envelope/body at the lineage-selected
/// read-only or mutable privilege.
pub fn authenticate_source_head_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedSourceHeadV1> {
    require_adapter_account(route, account, authenticated_lineage)?;
    let (header, head) =
        decode_runtime_account::<SourceHeadV3>(account.data, route.neutral_sink())?;
    head.validate()?;
    if head.source_spec_id != route.source_spec_id() {
        return Err(Error::MismatchedBinding);
    }
    let recipe = PdaRecipeV3::source_head(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        head.repair_generation,
    )?;
    let recipe_id = recipe.id()?;
    derived_pda.validate_for(route.adapter_program(), recipe_id, account.key, header.bump)?;
    let account_data_id = account_data_id(account.key, account.data)?;
    validate_open_lineage(
        route,
        authenticated_lineage,
        LineageFamilyV1::SourceHead,
        recipe_id,
        account.key,
        header.generation,
        account_data_id,
    )?;
    let mut bytes = [0; 144];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&head.snapshot_id()?.bytes());
    bytes[128..136].copy_from_slice(&header.generation.to_le_bytes());
    bytes[136] = header.bump;
    Ok(AuthenticatedSourceHeadV1 {
        route_id: route.route_id(),
        account: account.key,
        terminal_generation: header.generation,
        header,
        account_data_id,
        head,
        authentication_id: domain_id(HEAD_AUTH_DOMAIN, &bytes),
    })
}

/// Authenticate one writable OpenRawPage against the exact current SourceHead.
pub fn authenticate_open_raw_page_account(
    route: AuthenticatedSourceRouteV1,
    head: AuthenticatedSourceHeadV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedOpenRawPageV1> {
    require_mutable_adapter_account(route, account, authenticated_lineage)?;
    if head.route_id() != route.route_id() {
        return Err(Error::MismatchedBinding);
    }
    let (header, open) =
        decode_runtime_account::<OpenRawPageV3>(account.data, route.neutral_sink())?;
    open.validate_against_head(&head.head())?;
    let recipe = PdaRecipeV3::open_raw_page(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        open.repair_generation,
        open.page_index,
    )?;
    let recipe_id = recipe.id()?;
    derived_pda.validate_for(route.adapter_program(), recipe_id, account.key, header.bump)?;
    let account_data_id = account_data_id(account.key, account.data)?;
    validate_open_lineage(
        route,
        authenticated_lineage,
        LineageFamilyV1::OpenRawPage,
        recipe_id,
        account.key,
        header.generation,
        account_data_id,
    )?;
    let open_state_id = open_state_id(&open)?;
    let mut bytes = [0; 176];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&head.id().bytes());
    bytes[64..96].copy_from_slice(&account.key.bytes());
    bytes[96..128].copy_from_slice(&account_data_id.bytes());
    bytes[128..160].copy_from_slice(&open_state_id.bytes());
    bytes[160..168].copy_from_slice(&header.generation.to_le_bytes());
    bytes[168] = header.bump;
    Ok(AuthenticatedOpenRawPageV1 {
        route_id: route.route_id(),
        account: account.key,
        terminal_generation: header.generation,
        header,
        account_data_id,
        open,
        authentication_id: domain_id(OPEN_AUTH_DOMAIN, &bytes),
    })
}

/// Fixed-capacity consecutive authenticated boundary batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundaryBatchV1 {
    count: u8,
    receipts: [AuthenticatedBoundaryV1; MAX_BOUNDARIES_PER_INGEST],
}

impl BoundaryBatchV1 {
    /// Construct one nonempty, consecutive batch from already authenticated receipts.
    pub fn new(receipts: &[AuthenticatedBoundaryV1]) -> Result<Self> {
        if receipts.is_empty() || receipts.len() > MAX_BOUNDARIES_PER_INGEST {
            return Err(Error::InvalidCount);
        }
        let mut value = Self {
            count: u8::try_from(receipts.len()).map_err(|_| Error::InvalidCount)?,
            receipts: [AuthenticatedBoundaryV1::ZERO; MAX_BOUNDARIES_PER_INGEST],
        };
        let first = receipts[0];
        let mut index = 0_usize;
        while index < receipts.len() {
            let receipt = receipts[index];
            let offset = u64::try_from(index).map_err(|_| Error::ArithmeticOverflow)?;
            if receipt.id().is_zero()
                || receipt.route_id() != first.route_id()
                || receipt.source_spec_id() != first.source_spec_id()
                || receipt.repair_generation() != first.repair_generation()
                || receipt.bucket()
                    != first
                        .bucket()
                        .checked_add(offset)
                        .ok_or(Error::ArithmeticOverflow)?
            {
                return Err(Error::MismatchedBinding);
            }
            value.receipts[index] = receipt;
            index += 1;
        }
        Ok(value)
    }

    /// Active receipt count.
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Active receipt by index.
    pub fn receipt(&self, index: usize) -> Result<AuthenticatedBoundaryV1> {
        if index >= usize::from(self.count) {
            return Err(Error::InvalidCount);
        }
        Ok(self.receipts[index])
    }

    /// Content identity of the exact ordered authentication receipts.
    pub fn id(&self) -> Result<ContentId> {
        if self.count == 0 || usize::from(self.count) > MAX_BOUNDARIES_PER_INGEST {
            return Err(Error::InvalidCount);
        }
        let first = self.receipts[0];
        let mut bytes = [0; 16 + MAX_BOUNDARIES_PER_INGEST * 32];
        bytes[0] = self.count;
        bytes[8..16].copy_from_slice(&first.bucket().to_le_bytes());
        let mut index = 0_usize;
        while index < MAX_BOUNDARIES_PER_INGEST {
            let at = 16 + index * 32;
            if index < usize::from(self.count) {
                let receipt = self.receipts[index];
                if receipt.id().is_zero() {
                    return Err(Error::MismatchedBinding);
                }
                bytes[at..at + 32].copy_from_slice(&receipt.id().bytes());
            } else if self.receipts[index] != AuthenticatedBoundaryV1::ZERO {
                return Err(Error::InvalidCodec);
            }
            index += 1;
        }
        Ok(domain_id(BATCH_DOMAIN, &bytes))
    }
}

/// Page disposition requested for one multi-boundary call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SealBatchModeV1 {
    /// Retain the updated open page. Refuses if the page becomes full.
    KeepOpen,
    /// Seal the exact post-batch prefix even when the page is not full.
    SealAfterBatch,
    /// Seal only when the batch fills the page.
    SealIfFull,
}

/// Atomic pure result of appending several authenticated boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestBatchOutputV1 {
    /// Original head when work remains open; advanced head when sealed.
    pub head_after: SourceHeadV3,
    /// Updated open-page value. `None` means it was terminally consumed.
    pub open_after: Option<OpenRawPageV3>,
    /// Immutable page created by the same transition, if any.
    pub sealed_page: Option<RawPageV3>,
    /// Ordered boundary-batch authentication receipt.
    pub batch_receipt_id: ContentId,
    /// Exact last Clock slot across the ordered receipt batch.
    pub last_clock_slot: u64,
    /// Atomic compare-and-swap receipt binding both before accounts and all outputs.
    pub transition_receipt_id: ContentId,
}

/// Pure output of sealing the exact authenticated open-page prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealOpenPageOutputV1 {
    /// Source head after committing the immutable page.
    pub head_after: SourceHeadV3,
    /// Exact immutable page created from the open prefix.
    pub sealed_page: RawPageV3,
    /// Atomic receipt binding both mutable preimages and both semantic outputs.
    pub transition_receipt_id: ContentId,
}

/// Seal one nonempty authenticated open page without smuggling another append.
pub fn seal_authenticated_open_page(
    route: AuthenticatedSourceRouteV1,
    authenticated_head: AuthenticatedSourceHeadV1,
    authenticated_open: AuthenticatedOpenRawPageV1,
) -> Result<SealOpenPageOutputV1> {
    if authenticated_head.route_id() != route.route_id()
        || authenticated_open.route_id() != route.route_id()
    {
        return Err(Error::MismatchedBinding);
    }
    let head = authenticated_head.head();
    let open = authenticated_open.open();
    head.validate()?;
    open.validate_against_head(&head)?;
    let sealed_page = open.seal()?;
    let head_after = head.commit_page(&sealed_page)?;
    let mut bytes = [0_u8; 160];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&authenticated_head.id().bytes());
    bytes[64..96].copy_from_slice(&authenticated_open.id().bytes());
    bytes[96..128].copy_from_slice(&head_after.snapshot_id()?.bytes());
    bytes[128..160].copy_from_slice(&sealed_page.id()?.bytes());
    Ok(SealOpenPageOutputV1 {
        head_after,
        sealed_page,
        transition_receipt_id: domain_id(SEAL_OPEN_TRANSITION_DOMAIN, &bytes),
    })
}

/// Append a bounded sequence and optionally seal/advance atomically.
pub fn ingest_boundary_batch(
    route: AuthenticatedSourceRouteV1,
    authenticated_head: AuthenticatedSourceHeadV1,
    authenticated_open: AuthenticatedOpenRawPageV1,
    batch: &BoundaryBatchV1,
    mode: SealBatchModeV1,
) -> Result<IngestBatchOutputV1> {
    if authenticated_head.route_id() != route.route_id()
        || authenticated_open.route_id() != route.route_id()
    {
        return Err(Error::MismatchedBinding);
    }
    let head = authenticated_head.head();
    let open = authenticated_open.open();
    head.validate()?;
    open.validate_against_head(&head)?;
    if route.source_spec_id() != head.source_spec_id
        || route.source_spec_id() != open.source_spec_id
        || batch.receipt(0)?.route_id() != route.route_id()
        || batch.receipt(0)?.source_spec_id() != head.source_spec_id
        || batch.receipt(0)?.repair_generation() != head.repair_generation
    {
        return Err(Error::MismatchedBinding);
    }
    let expected_bucket = open
        .start_bucket
        .checked_add(u64::from(open.record_count))
        .ok_or(Error::ArithmeticOverflow)?;
    if batch.receipt(0)?.bucket() != expected_bucket {
        return Err(Error::MismatchedBinding);
    }
    let remaining = MAX_RAW_PAGE_RECORDS
        .checked_sub(usize::from(open.record_count))
        .ok_or(Error::InvalidCount)?;
    if usize::from(batch.count()) > remaining {
        return Err(Error::InvalidCount);
    }

    let mut next_open = open;
    let mut index = 0_usize;
    let mut last_clock_slot = 0_u64;
    while index < usize::from(batch.count()) {
        let receipt = batch.receipt(index)?;
        if receipt.route_id() != route.route_id()
            || receipt.source_spec_id() != head.source_spec_id
            || receipt.repair_generation() != head.repair_generation
            || receipt.bucket()
                != expected_bucket
                    .checked_add(u64::try_from(index).map_err(|_| Error::ArithmeticOverflow)?)
                    .ok_or(Error::ArithmeticOverflow)?
            || (index > 0 && receipt.clock().slot < last_clock_slot)
        {
            return Err(Error::MismatchedBinding);
        }
        next_open = next_open.append_observation(receipt.record())?;
        last_clock_slot = receipt.clock().slot;
        index += 1;
    }
    let full = usize::from(next_open.record_count) == MAX_RAW_PAGE_RECORDS;
    let seal = match mode {
        SealBatchModeV1::KeepOpen if full => return Err(Error::InvalidCount),
        SealBatchModeV1::KeepOpen => false,
        SealBatchModeV1::SealAfterBatch => true,
        SealBatchModeV1::SealIfFull => full,
    };
    let batch_receipt_id = batch.id()?;
    if seal {
        let page = next_open.seal()?;
        let head_after = head.commit_page(&page)?;
        let transition_receipt_id = ingest_transition_id(
            authenticated_head,
            authenticated_open,
            batch_receipt_id,
            head_after.snapshot_id()?,
            page.id()?,
            mode,
        );
        Ok(IngestBatchOutputV1 {
            head_after,
            open_after: None,
            sealed_page: Some(page),
            batch_receipt_id,
            last_clock_slot,
            transition_receipt_id,
        })
    } else {
        let transition_receipt_id = ingest_transition_id(
            authenticated_head,
            authenticated_open,
            batch_receipt_id,
            head.snapshot_id()?,
            open_state_id(&next_open)?,
            mode,
        );
        Ok(IngestBatchOutputV1 {
            head_after: head,
            open_after: Some(next_open),
            sealed_page: None,
            batch_receipt_id,
            last_clock_slot,
            transition_receipt_id,
        })
    }
}

fn require_mutable_adapter_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<()> {
    if authenticated_lineage.access() != LineageAccessV1::Mutable {
        return Err(Error::WrongPrivilege);
    }
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

fn require_adapter_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    authenticated_lineage: AuthenticatedReopenLineageV1,
) -> Result<()> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable {
        return Err(Error::WrongExecutableState);
    }
    let expected_writable = authenticated_lineage.access() == LineageAccessV1::Mutable;
    if account.signer || account.writable != expected_writable {
        return Err(Error::WrongPrivilege);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_open_lineage(
    route: AuthenticatedSourceRouteV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    account: RuntimeKey,
    generation: u64,
    account_data_id: ContentId,
) -> Result<()> {
    let lineage = authenticated_lineage.lineage();
    lineage.validate()?;
    if lineage.adapter_program != route.adapter_program()
        || lineage.family != family
        || lineage.semantic_binding_id != semantic_binding_id
        || !lineage.is_open
        || lineage.active_account != account
        || lineage.latest_generation != generation
        || lineage.last_opened_state_id != account_data_id
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    Ok(())
}

fn open_state_id(open: &OpenRawPageV3) -> Result<ContentId> {
    let mut bytes = [0; OPEN_RAW_PAGE_BYTES];
    open.encode_into(&mut bytes)?;
    Ok(domain_id(OPEN_STATE_DOMAIN, &bytes))
}

fn ingest_transition_id(
    head: AuthenticatedSourceHeadV1,
    open: AuthenticatedOpenRawPageV1,
    batch_receipt_id: ContentId,
    head_after_id: ContentId,
    page_or_open_after_id: ContentId,
    mode: SealBatchModeV1,
) -> ContentId {
    let mut bytes = [0; 168];
    bytes[0] = match mode {
        SealBatchModeV1::KeepOpen => 1,
        SealBatchModeV1::SealAfterBatch => 2,
        SealBatchModeV1::SealIfFull => 3,
    };
    bytes[8..40].copy_from_slice(&head.id().bytes());
    bytes[40..72].copy_from_slice(&open.id().bytes());
    bytes[72..104].copy_from_slice(&batch_receipt_id.bytes());
    bytes[104..136].copy_from_slice(&head_after_id.bytes());
    bytes[136..168].copy_from_slice(&page_or_open_after_id.bytes());
    domain_id(INGEST_TRANSITION_DOMAIN, &bytes)
}

fn id_at(input: &[u8], at: usize) -> ContentId {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&input[at..at + 32]);
    ContentId::from_bytes(bytes)
}

fn le_u16(input: &[u8]) -> u16 {
    let mut bytes = [0; 2];
    bytes.copy_from_slice(input);
    u16::from_le_bytes(bytes)
}

fn le_u64(input: &[u8]) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(input);
    u64::from_le_bytes(bytes)
}
