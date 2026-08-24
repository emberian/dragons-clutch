//! Hostile-chain-derived prerequisites for Product-owned Source custody retirement.
//!
//! This is deliberately not an instruction builder. The current program owns
//! the exact retirement kernel, but no dispatched SBF action yet constructs the
//! private Product/Failure terminal authorities consumed by that kernel. This
//! module authenticates every already-persisted founder fact without accepting
//! semantic IDs, amounts, or destinations from a browser. Once the program
//! publishes the final account contract, the remaining operator change is a
//! narrow composition over this opaque value rather than a caller-shaped DTO.

use crate::rpc_index::{
    CanonicalFamily, IndexedProgramRelease, ObservedRpcAccount, RpcCommitment,
};
use clutch_product_series::{
    CompiledSourceOccurrenceV3, ContentId, FixedCodec, MarketLifecyclePhaseV2,
    SeriesFundingTermsV2, SeriesMarketLinkPhaseV2,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v2, MarketLifecycleRootAccountV2,
    SeriesMarketLinkAccountV2, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V2,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_source_release_account, authenticate_source_route,
    AuthenticatedSourceRouteV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourceFundingCustodyLedgerV1, SourceWorkScheduleBindingV1,
    SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES, SOURCE_WORK_SCHEDULE_BYTES,
};
use sha2::{Digest, Sha256};
use solana_address::Address;

const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
const SEED_PRODUCT_MARKET_LIFECYCLE_ROOT_V1: &[u8] = b"dc:market-lifecycle-root:v1";
const SEED_PRODUCT_SERIES_MARKET_LINK_V1: &[u8] = b"dc:series-market-link:v1";
const SEED_SOURCE_FUNDING_CUSTODY_V1: &[u8] = b"dc:source-funding:v1";
const SEED_SOURCE_OCCURRENCE_V1: &[u8] = b"dc:source-occurrence:v1";
const SOURCE_OCCURRENCE_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/authenticated-source-occurrence-account/v1";
const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v2\0";
const SOURCE_CUSTODY_RETIREMENT_FOUNDER_MATERIAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/operator/source-custody-retirement-founder/v1";
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Result for the current, deliberately non-callable retirement preflight.
pub type SourceCustodyRetirementMaterialResult<T> =
    core::result::Result<T, SourceCustodyRetirementMaterialError>;

type Result<T> = SourceCustodyRetirementMaterialResult<T>;

/// Fail-closed construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCustodyRetirementMaterialError {
    /// Release identity does not cover both current Source and Product Series.
    CheckedRelease,
    /// Accounts were not observed together at one finalized snapshot.
    ChainSnapshot,
    /// An owner, PDA, codec, phase, or cross-account equality failed.
    ChainAuthority,
    /// Principal/donation accounting or an immutable destination failed.
    Funding,
    /// Exact integer arithmetic overflowed.
    Arithmetic,
    /// No dispatched SBF tuple owns the final Product/Failure authority yet.
    ExactSbfTupleUnavailable,
}

impl core::fmt::Display for SourceCustodyRetirementMaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not bind current Source and Product",
            Self::ChainSnapshot => "Source retirement accounts are not one finalized snapshot",
            Self::ChainAuthority => "hostile Source retirement authority failed authentication",
            Self::Funding => "Source retirement principal or donation partition refused",
            Self::Arithmetic => "Source retirement arithmetic overflowed",
            Self::ExactSbfTupleUnavailable => {
                "exact Product-owned Source custody retirement tuple is not dispatched"
            }
        })
    }
}

impl std::error::Error for SourceCustodyRetirementMaterialError {}

/// Complete hostile snapshot of persisted facts shared with the future
/// Product-owned retirement action. No semantic identity or amount is a field.
#[derive(Clone, Copy, Debug)]
pub struct SourceCustodyRetirementChainSnapshotV1<'a> {
    pub source_release: &'a ObservedRpcAccount,
    pub adapter_program: &'a ObservedRpcAccount,
    pub adapter_program_data: &'a ObservedRpcAccount,
    pub parser_program: &'a ObservedRpcAccount,
    pub parser_program_data: &'a ObservedRpcAccount,
    pub parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub source_work_schedule: &'a ObservedRpcAccount,
    pub funding_terms: &'a ObservedRpcAccount,
    pub market_lifecycle_root: &'a ObservedRpcAccount,
    pub series_market_link: &'a ObservedRpcAccount,
    pub source_occurrence: &'a ObservedRpcAccount,
    pub source_funding_custody: &'a ObservedRpcAccount,
    pub lamport_principal_refund: &'a ObservedRpcAccount,
    pub neutral_lamport_sink: &'a ObservedRpcAccount,
}

/// Opaque persisted founder authority. This value cannot be converted into an
/// instruction until a checked program release publishes the missing private
/// Product/Failure terminal composition and exact account contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainDerivedSourceCustodyRetirementFounderV1 {
    id: [u8; 32],
    market_lifecycle_root: Address,
    market_root_semantic_id: [u8; 32],
    market_root_authentication_id: [u8; 32],
    series_market_link: Address,
    link_semantic_id: [u8; 32],
    link_authentication_id: [u8; 32],
    market_instance_id: [u8; 32],
    series_plan_id: [u8; 32],
    ordinal: u32,
    source_generation: u64,
    source_repair_generation: u64,
    funding_terms_id: [u8; 32],
    source_release_manifest_id: [u8; 32],
    source_release_authentication_id: [u8; 32],
    source_route_id: [u8; 32],
    source_work_schedule_id: [u8; 32],
    source_lifecycle_id: [u8; 32],
    source_occurrence_id: [u8; 32],
    source_occurrence_account: Address,
    source_occurrence_authentication_id: [u8; 32],
    pre_root_source_occurrence_receipt_id: [u8; 32],
    source_funding_custody: Address,
    custody_data_id: [u8; 32],
    capitalization_authority_id: [u8; 32],
    capitalization_receipt_id: [u8; 32],
    allocated_principal_lamports: u64,
    remaining_principal_lamports: u64,
    completed_principal_lamports: u64,
    donation_lamports: u64,
    lamport_principal_refund: Address,
    neutral_lamport_sink: Address,
}

impl ChainDerivedSourceCustodyRetirementFounderV1 {
    /// Operator projection identity. It is not the unavailable Product
    /// retirement authorization or a substitute for the onchain check.
    pub const fn id(&self) -> [u8; 32] { self.id }
    pub const fn market_lifecycle_root(&self) -> Address { self.market_lifecycle_root }
    pub const fn market_root_semantic_id(&self) -> [u8; 32] { self.market_root_semantic_id }
    pub const fn market_root_authentication_id(&self) -> [u8; 32] {
        self.market_root_authentication_id
    }
    pub const fn series_market_link(&self) -> Address { self.series_market_link }
    pub const fn link_semantic_id(&self) -> [u8; 32] { self.link_semantic_id }
    pub const fn link_authentication_id(&self) -> [u8; 32] { self.link_authentication_id }
    pub const fn market_instance_id(&self) -> [u8; 32] { self.market_instance_id }
    pub const fn series_plan_id(&self) -> [u8; 32] { self.series_plan_id }
    pub const fn ordinal(&self) -> u32 { self.ordinal }
    pub const fn source_generation(&self) -> u64 { self.source_generation }
    pub const fn source_repair_generation(&self) -> u64 { self.source_repair_generation }
    pub const fn funding_terms_id(&self) -> [u8; 32] { self.funding_terms_id }
    pub const fn source_release_manifest_id(&self) -> [u8; 32] {
        self.source_release_manifest_id
    }
    pub const fn source_release_authentication_id(&self) -> [u8; 32] {
        self.source_release_authentication_id
    }
    pub const fn source_route_id(&self) -> [u8; 32] { self.source_route_id }
    pub const fn source_work_schedule_id(&self) -> [u8; 32] {
        self.source_work_schedule_id
    }
    pub const fn source_lifecycle_id(&self) -> [u8; 32] { self.source_lifecycle_id }
    pub const fn source_occurrence_id(&self) -> [u8; 32] { self.source_occurrence_id }
    pub const fn source_occurrence_account(&self) -> Address { self.source_occurrence_account }
    pub const fn source_occurrence_authentication_id(&self) -> [u8; 32] {
        self.source_occurrence_authentication_id
    }
    pub const fn pre_root_source_occurrence_receipt_id(&self) -> [u8; 32] {
        self.pre_root_source_occurrence_receipt_id
    }
    pub const fn source_funding_custody(&self) -> Address { self.source_funding_custody }
    pub const fn custody_data_id(&self) -> [u8; 32] { self.custody_data_id }
    pub const fn capitalization_authority_id(&self) -> [u8; 32] {
        self.capitalization_authority_id
    }
    pub const fn capitalization_receipt_id(&self) -> [u8; 32] {
        self.capitalization_receipt_id
    }
    pub const fn allocated_principal_lamports(&self) -> u64 {
        self.allocated_principal_lamports
    }
    pub const fn completed_principal_lamports(&self) -> u64 {
        self.completed_principal_lamports
    }
    pub const fn lamport_principal_refund(&self) -> Address { self.lamport_principal_refund }
    pub const fn neutral_lamport_sink(&self) -> Address { self.neutral_lamport_sink }
    pub const fn principal_refund_lamports(&self) -> u64 { self.remaining_principal_lamports }
    pub const fn neutral_donation_lamports(&self) -> u64 { self.donation_lamports }

    /// Fail closed until the exact SBF action owns the Product counted receipt
    /// and exhaustive Failure terminal capability. Release gating is not a
    /// replacement for those missing semantic owners.
    pub const fn require_exact_sbf_tuple(&self) -> Result<()> {
        Err(SourceCustodyRetirementMaterialError::ExactSbfTupleUnavailable)
    }
}

/// Authenticate all already-persisted founder facts for whole-lifecycle Source
/// custody retirement. This performs no state mutation and creates no generic
/// payload or account vector.
pub fn derive_source_custody_retirement_founder_v1(
    release: &IndexedProgramRelease,
    snapshot: SourceCustodyRetirementChainSnapshotV1<'_>,
) -> Result<ChainDerivedSourceCustodyRetirementFounderV1> {
    authenticate_release(release)?;
    authenticate_provenance(release, snapshot)?;
    let program_id = release.program_id;
    let manifest = clutch_source_plane_v3_runtime::SourceReleaseManifestV2::decode(
        &snapshot.source_release.data,
    )
    .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let manifest_id = manifest
        .id()
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let release_recipe = PdaRecipeV3::source_release(manifest_id)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let authenticated_release = authenticate_source_release_account(
        runtime_key(program_id),
        account_view(snapshot.source_release, false),
        derive_recipe(program_id, release_recipe)?,
    )
    .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    if snapshot.adapter_program.address != program_id
        || snapshot.adapter_program_data.address != release.program_data
    {
        return Err(SourceCustodyRetirementMaterialError::CheckedRelease);
    }
    let route = authenticate_source_route(
        authenticated_release,
        account_view(snapshot.adapter_program, false),
        account_view(snapshot.adapter_program_data, false),
        account_view(snapshot.parser_program, false),
        account_view(snapshot.parser_program_data, false),
        account_view(snapshot.parser_config, false),
        account_view(snapshot.source_spec, false),
    )
    .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let schedule = authenticate_schedule(program_id, route, snapshot.source_work_schedule)?;
    let custody = authenticate_custody(program_id, route, schedule, snapshot.source_funding_custody)?;
    let funding_terms = authenticate_funding_terms(program_id, snapshot.funding_terms)?;
    let root = authenticate_market_root(program_id, snapshot.market_lifecycle_root)?;
    let link = authenticate_retiring_link(program_id, snapshot.series_market_link, &root)?;
    let binding = link.account.state.binding();
    let occurrence = authenticate_occurrence(program_id, snapshot.source_occurrence)?;

    if funding_terms.id().map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?
        != binding.funding_terms_id
        || funding_terms.series_plan_id != binding.series_plan_id
        || funding_terms.lamport_principal_refund.bytes()
            != snapshot.lamport_principal_refund.address.to_bytes()
        || funding_terms.neutral_lamport_sink.bytes()
            != snapshot.neutral_lamport_sink.address.to_bytes()
        || occurrence.body.series_plan_id != binding.series_plan_id
        || occurrence.body.ordinal != binding.ordinal
        || occurrence.body.market_instance_id != binding.market_instance_id
        || occurrence.body.attachment_plan_id.content_id()
            != binding.attachment_plan_id.content_id()
        || occurrence.id.bytes() != binding.source_occurrence_id.bytes()
        || occurrence.account != snapshot.source_occurrence.address
        || occurrence.account.to_bytes() != binding.source_occurrence_account_id.bytes()
        || occurrence.authentication_id
            != binding.source_occurrence_account_authentication_id.bytes()
        || binding.source_release_id != route.release_manifest_id()
        || binding.source_route_id != route.route_id()
        || binding.clock_policy_id != route.clock_policy_id()
        || binding.source_plane_contract_id != route.source_plane_contract_id()
        || binding.source_spec_id != route.source_spec_id()
        || binding.generation != schedule.generation()
        || binding.rent_refund_owner != funding_terms.lamport_principal_refund
        || binding.neutral_lamport_sink != funding_terms.neutral_lamport_sink
        || custody.release_manifest_id != route.release_manifest_id()
        || custody.route_id != route.route_id()
        || custody.source_work_schedule_id != schedule.source_work_schedule_id()
        || custody.lifecycle_id != schedule.lifecycle_id()
        || custody.principal_refund != runtime_key(snapshot.lamport_principal_refund.address)
        || custody.neutral_sink != runtime_key(snapshot.neutral_lamport_sink.address)
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    authenticate_system_destination(snapshot.lamport_principal_refund)?;
    authenticate_system_destination(snapshot.neutral_lamport_sink)?;
    require_distinct([
        snapshot.market_lifecycle_root.address,
        snapshot.series_market_link.address,
        snapshot.source_occurrence.address,
        snapshot.source_funding_custody.address,
        snapshot.lamport_principal_refund.address,
        snapshot.neutral_lamport_sink.address,
    ])?;

    let explained = custody
        .remaining_principal_lamports
        .checked_add(custody.donation_lamports)
        .ok_or(SourceCustodyRetirementMaterialError::Arithmetic)?;
    if explained != snapshot.source_funding_custody.lamports {
        return Err(SourceCustodyRetirementMaterialError::Funding);
    }
    let completed_principal_lamports = custody
        .allocated_principal_lamports
        .checked_sub(custody.remaining_principal_lamports)
        .ok_or(SourceCustodyRetirementMaterialError::Funding)?;
    let custody_data_id = account_data_id(
        runtime_key(snapshot.source_funding_custody.address),
        &snapshot.source_funding_custody.data,
    )
    .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let ordinal_bytes = binding.ordinal.to_le_bytes();
    let source_generation_bytes = binding.generation.to_le_bytes();
    let repair_generation_bytes = binding.source_repair_generation.to_le_bytes();
    let allocated_bytes = custody.allocated_principal_lamports.to_le_bytes();
    let remaining_bytes = custody.remaining_principal_lamports.to_le_bytes();
    let completed_bytes = completed_principal_lamports.to_le_bytes();
    let donation_bytes = custody.donation_lamports.to_le_bytes();
    let id = founder_material_id(&[
        root.semantic_id,
        root.authentication_id,
        link.semantic_id,
        link.authentication_id,
        binding.market_instance_id.bytes(),
        binding.series_plan_id.bytes(),
        binding.funding_terms_id.bytes(),
        route.release_manifest_id().bytes(),
        route.release_authentication_id().bytes(),
        route.route_id().bytes(),
        schedule.source_work_schedule_id().bytes(),
        schedule.lifecycle_id().bytes(),
        occurrence.id.bytes(),
        occurrence.authentication_id,
        binding.source_occurrence_receipt_id.bytes(),
        custody_data_id.bytes(),
        custody.capitalization_authority_id.bytes(),
        custody.capitalization_receipt_id.bytes(),
    ], &[
        &ordinal_bytes,
        &source_generation_bytes,
        &repair_generation_bytes,
        &allocated_bytes,
        &remaining_bytes,
        &completed_bytes,
        &donation_bytes,
        snapshot.lamport_principal_refund.address.as_ref(),
        snapshot.neutral_lamport_sink.address.as_ref(),
    ]);
    if id == [0; 32] {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }

    Ok(ChainDerivedSourceCustodyRetirementFounderV1 {
        id,
        market_lifecycle_root: snapshot.market_lifecycle_root.address,
        market_root_semantic_id: root.semantic_id,
        market_root_authentication_id: root.authentication_id,
        series_market_link: snapshot.series_market_link.address,
        link_semantic_id: link.semantic_id,
        link_authentication_id: link.authentication_id,
        market_instance_id: binding.market_instance_id.bytes(),
        series_plan_id: binding.series_plan_id.bytes(),
        ordinal: binding.ordinal,
        source_generation: binding.generation,
        source_repair_generation: binding.source_repair_generation,
        funding_terms_id: binding.funding_terms_id.bytes(),
        source_release_manifest_id: route.release_manifest_id().bytes(),
        source_release_authentication_id: route.release_authentication_id().bytes(),
        source_route_id: route.route_id().bytes(),
        source_work_schedule_id: schedule.source_work_schedule_id().bytes(),
        source_lifecycle_id: schedule.lifecycle_id().bytes(),
        source_occurrence_id: occurrence.id.bytes(),
        source_occurrence_account: occurrence.account,
        source_occurrence_authentication_id: occurrence.authentication_id,
        pre_root_source_occurrence_receipt_id: binding.source_occurrence_receipt_id.bytes(),
        source_funding_custody: snapshot.source_funding_custody.address,
        custody_data_id: custody_data_id.bytes(),
        capitalization_authority_id: custody.capitalization_authority_id.bytes(),
        capitalization_receipt_id: custody.capitalization_receipt_id.bytes(),
        allocated_principal_lamports: custody.allocated_principal_lamports,
        remaining_principal_lamports: custody.remaining_principal_lamports,
        completed_principal_lamports,
        donation_lamports: custody.donation_lamports,
        lamport_principal_refund: snapshot.lamport_principal_refund.address,
        neutral_lamport_sink: snapshot.neutral_lamport_sink.address,
    })
}

struct AuthenticatedMarketRootV2 {
    address: Address,
    account: MarketLifecycleRootAccountV2,
    semantic_id: [u8; 32],
    authentication_id: [u8; 32],
}

fn authenticate_market_root(
    program_id: Address,
    observed: &ObservedRpcAccount,
) -> Result<AuthenticatedMarketRootV2> {
    if observed.owner != program_id
        || observed.executable
        || observed.data.len() != MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let account = MarketLifecycleRootAccountV2::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let binding = account.state.binding();
    let (expected, bump) = Address::find_program_address(
        &[
            SEED_PRODUCT_MARKET_LIFECYCLE_ROOT_V1,
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ],
        &program_id,
    );
    if observed.address != expected
        || account.stored_bump != bump
        || account.state.phase() != MarketLifecyclePhaseV2::Active
        || account.state.live_series_links() == 0
        || observed.lamports < account.rent_principal_lamports
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let semantic_id = account
        .state
        .semantic_id()
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?
        .bytes();
    let data_id = account_data_id(runtime_key(observed.address), &observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let rent_bytes = account.rent_principal_lamports.to_le_bytes();
    let lamport_bytes = observed.lamports.to_le_bytes();
    let bump_bytes = [account.stored_bump];
    let authentication_id = hash_parts(&[
        MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V2,
        observed.address.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &semantic_id,
        &rent_bytes,
        &lamport_bytes,
        &bump_bytes,
    ]);
    if authentication_id == [0; 32] {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    Ok(AuthenticatedMarketRootV2 {
        address: observed.address,
        account,
        semantic_id,
        authentication_id,
    })
}

struct AuthenticatedRetiringLinkV2 {
    account: SeriesMarketLinkAccountV2,
    semantic_id: [u8; 32],
    authentication_id: [u8; 32],
}

fn authenticate_retiring_link(
    program_id: Address,
    observed: &ObservedRpcAccount,
    root: &AuthenticatedMarketRootV2,
) -> Result<AuthenticatedRetiringLinkV2> {
    if observed.owner != program_id
        || observed.executable
        || observed.data.len() != SERIES_MARKET_LINK_ACCOUNT_BYTES_V2
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let account = SeriesMarketLinkAccountV2::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let binding = account.state.binding();
    let root_binding = root.account.state.binding();
    let (expected, bump) = Address::find_program_address(
        &[
            SEED_PRODUCT_SERIES_MARKET_LINK_V1,
            &binding.series_plan_id.bytes(),
            &binding.ordinal.to_le_bytes(),
        ],
        &program_id,
    );
    let accounted = account
        .state
        .rent_principal_lamports()
        .checked_add(account.state.current_donation_lamports())
        .ok_or(SourceCustodyRetirementMaterialError::Arithmetic)?;
    if observed.address != expected
        || account.stored_bump != bump
        || account.state.phase() != SeriesMarketLinkPhaseV2::Retiring
        || observed.lamports < accounted
        || binding.market_root_account_id.bytes() != root.address.to_bytes()
        || binding.market_instance_id != root_binding.market_instance_id
        || binding.generation != root_binding.generation
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let data_id = account_data_id(runtime_key(observed.address), &observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let semantic_id = account
        .state
        .semantic_id()
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?
        .bytes();
    let authentication_id = series_market_link_authentication_id_v2(
        observed.address.to_bytes(),
        program_id.to_bytes(),
        data_id.bytes(),
        semantic_id,
        binding.market_root_account_id.bytes(),
        observed.lamports,
    )
    .0;
    if authentication_id == [0; 32] {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    Ok(AuthenticatedRetiringLinkV2 { account, semantic_id, authentication_id })
}

struct AuthenticatedOccurrenceV1 {
    body: CompiledSourceOccurrenceV3,
    id: ContentId,
    account: Address,
    authentication_id: [u8; 32],
}

fn authenticate_occurrence(
    program_id: Address,
    observed: &ObservedRpcAccount,
) -> Result<AuthenticatedOccurrenceV1> {
    if observed.owner != program_id || observed.executable {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let body = CompiledSourceOccurrenceV3::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let id = ContentId::from_bytes(
        body.id()
            .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?
            .bytes(),
    );
    let (expected, bump) = Address::find_program_address(
        &[SEED_SOURCE_OCCURRENCE_V1, &id.bytes()],
        &program_id,
    );
    if observed.address != expected {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let data_id = account_data_id(runtime_key(observed.address), &observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let mut preimage = [0_u8; 104];
    preimage[..32].copy_from_slice(&observed.address.to_bytes());
    preimage[32..64].copy_from_slice(&data_id.bytes());
    preimage[64..96].copy_from_slice(&id.bytes());
    preimage[96] = bump;
    let authentication_id = domain_id(
        SOURCE_OCCURRENCE_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
        &preimage,
    );
    Ok(AuthenticatedOccurrenceV1 { body, id, account: observed.address, authentication_id })
}

fn authenticate_funding_terms(
    program_id: Address,
    observed: &ObservedRpcAccount,
) -> Result<SeriesFundingTermsV2> {
    if observed.owner != program_id || observed.executable {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let terms = SeriesFundingTermsV2::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let id = terms.id().map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let (expected, _) = Address::find_program_address(
        &[
            SEED_PRODUCT_ARTIFACT_V1,
            &[ArtifactKind::SeriesFundingTermsV2.byte()],
            &id.bytes(),
        ],
        &program_id,
    );
    if observed.address != expected {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    Ok(terms)
}

fn authenticate_schedule(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    observed: &ObservedRpcAccount,
) -> Result<SourceWorkScheduleBindingV1> {
    if observed.owner != program_id
        || observed.executable
        || observed.data.len() != SOURCE_WORK_SCHEDULE_BYTES
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let schedule = SourceWorkScheduleBindingV1::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    schedule
        .validate_against(route)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let id = schedule
        .id()
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let (expected, _) = Address::find_program_address(
        &[
            SEED_PRODUCT_ARTIFACT_V1,
            &[ArtifactKind::SourceWorkScheduleV1.byte()],
            &id.bytes(),
        ],
        &program_id,
    );
    if observed.address != expected {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    Ok(schedule)
}

fn authenticate_custody(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    observed: &ObservedRpcAccount,
) -> Result<SourceFundingCustodyLedgerV1> {
    let (expected, _) = Address::find_program_address(
        &[SEED_SOURCE_FUNDING_CUSTODY_V1, &schedule.lifecycle_id().bytes()],
        &program_id,
    );
    if observed.address != expected
        || observed.owner != program_id
        || observed.executable
        || observed.data.len() != SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    let ledger = SourceFundingCustodyLedgerV1::decode(&observed.data)
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    if !ledger.is_live()
        || schedule.payer() != runtime_key(observed.address)
        || ledger.adapter_program != runtime_key(program_id)
        || ledger.release_manifest_id != route.release_manifest_id()
        || ledger.route_id != route.route_id()
        || ledger.source_work_schedule_id != schedule.source_work_schedule_id()
        || ledger.lifecycle_id != schedule.lifecycle_id()
        || ledger.custody_account != runtime_key(observed.address)
        || ledger.neutral_sink != route.neutral_sink()
    {
        return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
    }
    Ok(ledger)
}

fn authenticate_system_destination(observed: &ObservedRpcAccount) -> Result<()> {
    if observed.address == Address::default()
        || observed.owner != SYSTEM_PROGRAM_ID
        || observed.executable
        || !observed.data.is_empty()
    {
        return Err(SourceCustodyRetirementMaterialError::Funding);
    }
    Ok(())
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    release
        .validate()
        .map_err(|_| SourceCustodyRetirementMaterialError::CheckedRelease)?;
    if !release.families.contains(&CanonicalFamily::Source)
        || !release.families.contains(&CanonicalFamily::Series)
    {
        return Err(SourceCustodyRetirementMaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_provenance(
    release: &IndexedProgramRelease,
    snapshot: SourceCustodyRetirementChainSnapshotV1<'_>,
) -> Result<()> {
    let first = &snapshot.source_release.provenance;
    let release_key = release.key();
    let accounts = [
        snapshot.source_release,
        snapshot.adapter_program,
        snapshot.adapter_program_data,
        snapshot.parser_program,
        snapshot.parser_program_data,
        snapshot.parser_config,
        snapshot.source_spec,
        snapshot.source_work_schedule,
        snapshot.funding_terms,
        snapshot.market_lifecycle_root,
        snapshot.series_market_link,
        snapshot.source_occurrence,
        snapshot.source_funding_custody,
        snapshot.lamport_principal_refund,
        snapshot.neutral_lamport_sink,
    ];
    if first.slot == 0
        || first.commitment != RpcCommitment::Finalized
        || first.release_key != release_key
        || accounts.iter().any(|account| {
            account.provenance.cluster_key != first.cluster_key
                || account.provenance.slot != first.slot
                || account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.release_key != release_key
        })
    {
        return Err(SourceCustodyRetirementMaterialError::ChainSnapshot);
    }
    Ok(())
}

fn require_distinct<const N: usize>(accounts: [Address; N]) -> Result<()> {
    let mut right = 0_usize;
    while right < N {
        let mut left = 0_usize;
        while left < right {
            if accounts[left] == accounts[right] {
                return Err(SourceCustodyRetirementMaterialError::ChainAuthority);
            }
            left += 1;
        }
        right += 1;
    }
    Ok(())
}

fn account_view(account: &ObservedRpcAccount, writable: bool) -> RuntimeAccountViewV1<'_> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.address),
        owner: runtime_key(account.owner),
        lamports: account.lamports,
        executable: account.executable,
        writable,
        signer: false,
        data: &account.data,
    }
}

fn derive_recipe(program_id: Address, recipe: PdaRecipeV3) -> Result<RuntimeDerivedPdaV1> {
    recipe
        .validate()
        .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    let mut index = 0_usize;
    while index < usize::from(recipe.seed_count()) {
        seeds.push(
            recipe
                .seed(index)
                .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?,
        );
        index += 1;
    }
    let (address, bump) = Address::find_program_address(&seeds, &program_id);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe
            .id()
            .map_err(|_| SourceCustodyRetirementMaterialError::ChainAuthority)?,
        address: runtime_key(address),
        bump,
    })
}

fn runtime_key(address: Address) -> RuntimeKey {
    RuntimeKey::from_bytes(address.to_bytes())
}

fn domain_id(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hash_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn founder_material_id(ids: &[[u8; 32]], tails: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CUSTODY_RETIREMENT_FOUNDER_MATERIAL_DOMAIN_V1);
    for id in ids {
        hasher.update(id);
    }
    for tail in tails {
        hasher.update(tail);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    #[test]
    fn material_has_no_callable_escape_hatch_before_exact_tuple() {
        let source = include_str!("source_custody_retirement_material.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("bounded production source");
        assert!(!production.contains("pub fn unsigned_instruction"));
        assert!(production.contains("ExactSbfTupleUnavailable"));
        assert!(!production.contains("product_retirement_authority_id:"));
        assert!(!production.contains("counted_retirement_receipt_id:"));
    }

    #[test]
    fn product_link_and_occurrence_are_hostile_chain_inputs() {
        let source = include_str!("source_custody_retirement_material.rs");
        assert!(source.contains("SeriesMarketLinkAccountV2::decode"));
        assert!(source.contains("CompiledSourceOccurrenceV3::decode"));
        assert!(source.contains("SeriesMarketLinkPhaseV2::Retiring"));
        assert!(source.contains("source_occurrence_account_authentication_id"));
    }

    #[test]
    fn funding_owners_are_not_browser_amounts() {
        let source = include_str!("source_custody_retirement_material.rs");
        let snapshot = source
            .split("pub struct SourceCustodyRetirementChainSnapshotV1")
            .nth(1)
            .and_then(|value| value.split("/// Opaque persisted founder authority").next())
            .expect("bounded snapshot source");
        assert!(!snapshot.contains("principal_lamports"));
        assert!(!snapshot.contains("donation_lamports"));
        assert!(source.contains("explained != snapshot.source_funding_custody.lamports"));
    }

    #[test]
    fn identity_aliases_refuse_before_composition() {
        let one = Address::new_from_array([1; 32]);
        let two = Address::new_from_array([2; 32]);
        assert_eq!(require_distinct([one, two]), Ok(()));
        assert_eq!(
            require_distinct([one, one]),
            Err(SourceCustodyRetirementMaterialError::ChainAuthority)
        );
    }

    #[test]
    fn every_founder_identity_mutation_changes_material_id() {
        let mut ids = [[1_u8; 32], [2_u8; 32]];
        let tails: [&[u8]; 2] = [&[3_u8; 8], &[4_u8; 8]];
        let first = founder_material_id(&ids, &tails);
        ids[1][31] ^= 1;
        assert_ne!(founder_material_id(&ids, &tails), first);
        let changed_tail: [&[u8]; 2] = [&[3_u8; 8], &[5_u8; 8]];
        assert_ne!(founder_material_id(&[[1; 32], [2; 32]], &changed_tail), first);
    }
}
