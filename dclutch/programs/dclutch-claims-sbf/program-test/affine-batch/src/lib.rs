#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived unsigned construction for canonical affine Claims batches.
//!
//! This host projection authenticates a same-snapshot activation cache, open
//! Core Market, Product Runtime V2 graph, exact linked LBV2 raw record, Claims
//! aggregate, and ordered Position accounts before it emits an instruction.
//! It is not authority: the Claims program independently repeats every check.

/// Shared canonical Product Runtime V2/LBV2 real-SBF fixture compiler.
pub mod fixture;

use dclutch_claims_svm::{
    CallerRole,
    affine_batch_v2::{
        AffineBatchPlanInputV2, AffineBatchPlanV2, AffineBatchPositionV2, AffineBatchRowInputV2,
        AffineBatchRowV2, SignedMagnitudeV2, plan_bytes,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_liability_basis_v2_kernel::product_claims::LinkedBasisRecordV2;
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{system_program, sysvar};

const MARKET_MAGIC_V2: [u8; 8] = *b"DCLLBM02";
const POSITION_MAGIC_V2: [u8; 8] = *b"DCLLBP02";
const ABI_VERSION_V2: u16 = 2;
const LIABILITY_BASIS_MARKET_HEADER_BYTES_V2: usize = 256;
const LIABILITY_BASIS_POSITION_HEADER_BYTES_V2: usize = 128;
const LIABILITY_BASIS_MARKET_SEED_V2: &[u8] = b"dclutch:lbv2:market";
const LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0x5c, 0x84, 0x2a, 0xe9, 0xe9, 0x15, 0x51, 0xd1, 0xaf, 0x99, 0xcf, 0x99, 0xfd, 0x53, 0x7f, 0x64,
    0xfb, 0x8d, 0xbf, 0x6a, 0x4e, 0x88, 0x3f, 0x22, 0xd9, 0x0b, 0xd5, 0xf3, 0x24, 0x5f, 0x6e, 0x2e,
];
const MARKET_CLAIM_COUNT_OFFSET: usize = 12;
const MARKET_REVISION_OFFSET: usize = 16;
const MARKET_LOGICAL_ID_OFFSET: usize = 24;
const MARKET_RELEASE_SET_OFFSET: usize = 56;
const MARKET_REGISTRY_OFFSET: usize = 88;
const MARKET_PRODUCT_OFFSET: usize = 120;
const MARKET_BASIS_OFFSET: usize = 152;
const MARKET_GENERATION_OFFSET: usize = 248;
const POSITION_CLAIM_COUNT_OFFSET: usize = 12;
const POSITION_REVISION_OFFSET: usize = 16;
const POSITION_MARKET_OFFSET: usize = 24;
const POSITION_OWNER_OFFSET: usize = 56;
const POSITION_BASIS_OFFSET: usize = 88;
const BASIS_SEMANTIC_ID_DOMAIN_V2: &[u8] = b"dclutch/lbv2/semantic-id/v2";
const BASIS_PRODUCT_LINK_OFFSET_V2: usize = 32;
const BASIS_PRODUCT_LINK_END_V2: usize = 64;

/// Stable unsigned-builder refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Observations came from different finalized slots.
    ObservationMismatch,
    /// Registry activation, role, or current release refused.
    InvalidActivation,
    /// Core owner, PDA, phase, Product, release, or Registry join refused.
    InvalidCore,
    /// Product raw/staging finality or Product graph composition refused.
    InvalidProduct,
    /// Linked LBV2 finality, raw digest, semantic ID, or Product join refused.
    InvalidBasis,
    /// Claims aggregate identity, width, revision, or immutable join refused.
    InvalidMarket,
    /// Position table identity, width, owner, order, or revision refused.
    InvalidPosition,
    /// Requested affine row was noncanonical or nonconserving.
    InvalidRow,
    /// Exact packet width or encoding refused.
    InvalidPlan,
}

/// Result alias for unsigned affine construction.
pub type Result<T> = core::result::Result<T, Error>;

/// One account observed at a finalized slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedAccountV2<'a> {
    /// Finalized observation slot.
    pub slot: u64,
    /// Account key.
    pub key: Pubkey,
    /// Account owner.
    pub owner: Pubkey,
    /// Account lamports.
    pub lamports: u64,
    /// Executable flag.
    pub executable: bool,
    /// Complete account bytes.
    pub data: &'a [u8],
}

/// Exact finalized raw record and vacant staging cursor observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedRecordObservationV2<'a> {
    /// Registry- or Core-owned raw record.
    pub raw: ObservedAccountV2<'a>,
    /// System-owned, zero-data staging PDA.
    pub staging: ObservedAccountV2<'a>,
}

/// Exact Product Runtime V2 graph observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductGraphObservationV2<'a> {
    /// Product graph-root raw/staging pair.
    pub product: FinalizedRecordObservationV2<'a>,
    /// Product-selected result-domain raw/staging pair.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Product-selected portfolio raw/staging pair.
    pub portfolio: FinalizedRecordObservationV2<'a>,
}

/// Same-slot chain observations required for one affine batch.
#[derive(Clone, Copy, Debug)]
pub struct AffineBatchObservationV2<'a> {
    /// Registry role of the upstream caller program.
    pub caller_role: CallerRole,
    /// Nonzero request identity owned by the upstream caller.
    pub request_id: [u8; 32],
    /// Current Registry program.
    pub registry_program: Pubkey,
    /// Current Registry-owned activation cache.
    pub activation_cache: ObservedAccountV2<'a>,
    /// Current open Core Market.
    pub core_market: ObservedAccountV2<'a>,
    /// Current LBV2 aggregate.
    pub claims_market: ObservedAccountV2<'a>,
    /// Exact finalized Product-linked LBV2 raw/staging pair.
    pub linked_basis: FinalizedRecordObservationV2<'a>,
    /// Exact Product Runtime V2 graph.
    pub product: ProductGraphObservationV2<'a>,
    /// Ordered, unique LBV2 Position observations.
    pub positions: &'a [ObservedAccountV2<'a>],
    /// Same-slot Rent value used for raw-record exemption checks.
    pub rent: &'a Rent,
}

/// Irreducible requested mutation over chain-derived Position indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineMutationInputV2 {
    /// Whether the source Position-table index is active.
    pub source_present: bool,
    /// Whether the destination Position-table index is active.
    pub destination_present: bool,
    /// Runtime outcome coordinate.
    pub outcome: u32,
    /// Source observation index, zero when inactive.
    pub source_position_index: u32,
    /// Destination observation index, zero when inactive.
    pub destination_position_index: u32,
    /// Exact aggregate delta.
    pub aggregate_delta: SignedMagnitudeV2,
    /// Exact source delta.
    pub source_delta: SignedMagnitudeV2,
    /// Exact destination delta.
    pub destination_delta: SignedMagnitudeV2,
}

/// Chain-derived exact unsigned Claims instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedAffineBatchV2 {
    /// Exact unsigned child instruction; its first meta is the caller PDA signer.
    pub instruction: Instruction,
    /// SHA-256 of the complete canonical plan bytes.
    pub request_digest: [u8; 32],
    /// Exact upstream caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Current Claims program selected by Registry.
    pub claims_program: Pubkey,
    /// Ordered Position account keys committed by the packet table.
    pub positions: Vec<Pubkey>,
}

#[derive(Clone, Copy)]
struct RoleProgramsV2 {
    caller: Pubkey,
    caller_programdata: Pubkey,
    claims: Pubkey,
    claims_programdata: Pubkey,
    core: Pubkey,
    core_programdata: Pubkey,
}

#[derive(Clone, Copy)]
struct MarketProjectionV2 {
    outcome_count: u32,
    revision: u64,
    logical_market: [u8; 32],
    release_set: [u8; 32],
    registry_program: [u8; 32],
    product_id: [u8; 32],
    basis_id: [u8; 32],
    generation: u64,
}

#[derive(Clone, Copy)]
struct PositionProjectionV2 {
    outcome_count: u32,
    revision: u64,
    market: [u8; 32],
    owner: [u8; 32],
    basis_id: [u8; 32],
}

/// Authenticate one same-slot observation set and build its exact unsigned
/// affine child instruction. Requested deltas remain caller intent; all
/// identities, widths, revisions, release programs, and account metas come
/// from the observed chain state.
pub fn construct_affine_batch_v2(
    observation: AffineBatchObservationV2<'_>,
    mutations: &[AffineMutationInputV2],
) -> Result<ConstructedAffineBatchV2> {
    let slot = observation.activation_cache.slot;
    if observation.request_id == [0; 32]
        || observations(observation)
            .iter()
            .any(|account| account.slot != slot)
    {
        return Err(Error::ObservationMismatch);
    }
    let (roles, release_set) = authenticate_activation(observation)?;
    let core = authenticate_core(observation, roles.core, release_set)?;
    let market = authenticate_market(observation, roles.claims, release_set, core)?;
    let product_digest = core.identity.product_record.to_bytes();
    let product = authenticate_product(observation, product_digest)?;
    if product.join.product_id.to_bytes() != market.product_id
        || product.join.liability_basis_id.to_bytes() != market.basis_id
        || product.join.outcome_count != market.outcome_count
    {
        return Err(Error::InvalidProduct);
    }
    let linked_basis_digest = authenticate_basis(
        observation,
        roles.core,
        market,
        product.join.product_id.to_bytes(),
    )?;
    let positions = authenticate_positions(observation, roles.claims, market)?;
    let position_count = u32::try_from(positions.len()).map_err(|_| Error::InvalidPosition)?;
    let mut rows = Vec::with_capacity(mutations.len());
    for mutation in mutations {
        rows.push(
            AffineBatchRowV2::new(
                AffineBatchRowInputV2 {
                    source_present: mutation.source_present,
                    destination_present: mutation.destination_present,
                    outcome: mutation.outcome,
                    source_position_index: mutation.source_position_index,
                    destination_position_index: mutation.destination_position_index,
                    aggregate_delta: mutation.aggregate_delta,
                    source_delta: mutation.source_delta,
                    destination_delta: mutation.destination_delta,
                },
                market.outcome_count,
                position_count,
            )
            .map_err(|_| Error::InvalidRow)?,
        );
    }
    let row_count = u32::try_from(rows.len()).map_err(|_| Error::InvalidPlan)?;
    let mut data = vec![0; plan_bytes(position_count, row_count).map_err(|_| Error::InvalidPlan)?];
    AffineBatchPlanV2::encode_into(
        AffineBatchPlanInputV2 {
            caller_role: observation.caller_role,
            release_set,
            market: market.logical_market,
            request_id: observation.request_id,
            product_record_digest: product_digest,
            semantic_basis_id: market.basis_id,
            linked_basis_record_digest: linked_basis_digest,
            expected_market_revision: market.revision,
            outcome_count: market.outcome_count,
        },
        &positions,
        &rows,
        &mut data,
    )
    .map_err(|_| Error::InvalidPlan)?;
    let request_digest = hash(&data).to_bytes();
    let role = execution_role(observation.caller_role);
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.logical_market,
        role,
        observation.request_id,
        request_digest,
    )
    .map_err(|_| Error::InvalidPlan)?;
    let caller_authority = Pubkey::find_program_address(&caller_seeds.as_slices(), &roles.caller).0;
    let mut metas = Vec::with_capacity(20_usize.saturating_add(positions.len()));
    metas.extend([
        AccountMeta::new_readonly(caller_authority, true),
        AccountMeta::new(observation.claims_market.key, false),
        AccountMeta::new_readonly(observation.linked_basis.raw.key, false),
        AccountMeta::new_readonly(observation.linked_basis.staging.key, false),
        AccountMeta::new_readonly(observation.product.product.raw.key, false),
        AccountMeta::new_readonly(observation.product.product.staging.key, false),
        AccountMeta::new_readonly(observation.product.result_domain.raw.key, false),
        AccountMeta::new_readonly(observation.product.result_domain.staging.key, false),
        AccountMeta::new_readonly(observation.product.portfolio.raw.key, false),
        AccountMeta::new_readonly(observation.product.portfolio.staging.key, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(observation.core_market.key, false),
        AccountMeta::new_readonly(observation.activation_cache.key, false),
        AccountMeta::new_readonly(observation.registry_program, false),
        AccountMeta::new_readonly(roles.caller, false),
        AccountMeta::new_readonly(roles.caller_programdata, false),
        AccountMeta::new_readonly(roles.claims, false),
        AccountMeta::new_readonly(roles.claims_programdata, false),
        AccountMeta::new_readonly(roles.core, false),
        AccountMeta::new_readonly(roles.core_programdata, false),
    ]);
    let mut position_keys = Vec::with_capacity(observation.positions.len());
    for position in observation.positions {
        metas.push(AccountMeta::new(position.key, false));
        position_keys.push(position.key);
    }
    Ok(ConstructedAffineBatchV2 {
        instruction: Instruction {
            program_id: roles.claims,
            accounts: metas,
            data,
        },
        request_digest,
        caller_authority,
        claims_program: roles.claims,
        positions: position_keys,
    })
}

fn observations<'a>(observation: AffineBatchObservationV2<'a>) -> Vec<ObservedAccountV2<'a>> {
    let mut values = vec![
        observation.activation_cache,
        observation.core_market,
        observation.claims_market,
        observation.linked_basis.raw,
        observation.linked_basis.staging,
        observation.product.product.raw,
        observation.product.product.staging,
        observation.product.result_domain.raw,
        observation.product.result_domain.staging,
        observation.product.portfolio.raw,
        observation.product.portfolio.staging,
    ];
    values.extend_from_slice(observation.positions);
    values
}

fn authenticate_activation(
    observation: AffineBatchObservationV2<'_>,
) -> Result<(RoleProgramsV2, [u8; 32])> {
    if observation.activation_cache.owner != observation.registry_program
        || observation.activation_cache.executable
    {
        return Err(Error::InvalidActivation);
    }
    let cache = ActivatedExecutionReleaseSetViewV1::decode(observation.activation_cache.data)
        .map_err(|_| Error::InvalidActivation)?;
    let release = cache
        .execution_release_set_id()
        .map_err(|_| Error::InvalidActivation)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release.as_bytes()],
        &observation.registry_program,
    )
    .0;
    if observation.activation_cache.key != expected {
        return Err(Error::InvalidActivation);
    }
    let caller = cache
        .role(execution_role(observation.caller_role))
        .map_err(|_| Error::InvalidActivation)?
        .release();
    let claims = cache
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    let core = cache
        .role(ExecutionRoleV1::Core)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    Ok((
        RoleProgramsV2 {
            caller: Pubkey::new_from_array(caller.program().to_bytes()),
            caller_programdata: Pubkey::new_from_array(caller.programdata()),
            claims: Pubkey::new_from_array(claims.program().to_bytes()),
            claims_programdata: Pubkey::new_from_array(claims.programdata()),
            core: Pubkey::new_from_array(core.program().to_bytes()),
            core_programdata: Pubkey::new_from_array(core.programdata()),
        },
        release.to_bytes(),
    ))
}

fn authenticate_core(
    observation: AffineBatchObservationV2<'_>,
    core_program: Pubkey,
    release_set: [u8; 32],
) -> Result<CoreState> {
    if observation.core_market.owner != core_program
        || observation.core_market.executable
        || observation.core_market.data.len() != STATE_BYTES
    {
        return Err(Error::InvalidCore);
    }
    let core = CoreState::decode(observation.core_market.data).map_err(|_| Error::InvalidCore)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &core_program,
    )
    .0;
    if observation.core_market.key != expected
        || core.phase != CorePhase::Open
        || core.identity.market_id.to_bytes() != observation.core_market.key.to_bytes()
        || core.identity.selected_release_set.to_bytes() != release_set
        || core.identity.registry_program.to_bytes() != observation.registry_program.to_bytes()
    {
        return Err(Error::InvalidCore);
    }
    Ok(core)
}

fn authenticate_market(
    observation: AffineBatchObservationV2<'_>,
    claims_program: Pubkey,
    release_set: [u8; 32],
    core: CoreState,
) -> Result<MarketProjectionV2> {
    let data = observation.claims_market.data;
    let value = MarketProjectionV2 {
        outcome_count: u32_at(data, MARKET_CLAIM_COUNT_OFFSET).ok_or(Error::InvalidMarket)?,
        revision: u64_at(data, MARKET_REVISION_OFFSET).ok_or(Error::InvalidMarket)?,
        logical_market: array_at(data, MARKET_LOGICAL_ID_OFFSET).ok_or(Error::InvalidMarket)?,
        release_set: array_at(data, MARKET_RELEASE_SET_OFFSET).ok_or(Error::InvalidMarket)?,
        registry_program: array_at(data, MARKET_REGISTRY_OFFSET).ok_or(Error::InvalidMarket)?,
        product_id: array_at(data, MARKET_PRODUCT_OFFSET).ok_or(Error::InvalidMarket)?,
        basis_id: array_at(data, MARKET_BASIS_OFFSET).ok_or(Error::InvalidMarket)?,
        generation: u64_at(data, MARKET_GENERATION_OFFSET).ok_or(Error::InvalidMarket)?,
    };
    let width = usize::try_from(value.outcome_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .and_then(|tail| LIABILITY_BASIS_MARKET_HEADER_BYTES_V2.checked_add(tail))
        .ok_or(Error::InvalidMarket)?;
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, &value.logical_market],
        &claims_program,
    )
    .0;
    if array_at::<8>(data, 0) != Some(MARKET_MAGIC_V2)
        || u16_at(data, 8) != Some(ABI_VERSION_V2)
        || value.outcome_count == 0
        || data.len() != width
        || observation.claims_market.owner != claims_program
        || observation.claims_market.executable
        || observation.claims_market.key != expected
        || value.logical_market != observation.core_market.key.to_bytes()
        || value.release_set != release_set
        || value.registry_program != observation.registry_program.to_bytes()
        || value.product_id != core.identity.product_id.to_bytes()
        || value.generation != core.identity.generation
    {
        return Err(Error::InvalidMarket);
    }
    Ok(value)
}

fn authenticate_product(
    observation: AffineBatchObservationV2<'_>,
    expected_digest: [u8; 32],
) -> Result<dclutch_product_runtime_v2_admission::AdmissionProjectionV2> {
    let product = authenticate_record(
        observation.product.product,
        observation.registry_program,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        expected_digest,
        observation.rent,
        Error::InvalidProduct,
    )?;
    let product_record = dclutch_product_runtime_v2_admission::ProductRecordV2::decode(
        observation.product.product.raw.data,
    )
    .map_err(|_| Error::InvalidProduct)?;
    let result_domain = authenticate_record(
        observation.product.result_domain,
        observation.registry_program,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product_record.result_domain_digest().to_bytes(),
        observation.rent,
        Error::InvalidProduct,
    )?;
    let portfolio = authenticate_record(
        observation.product.portfolio,
        observation.registry_program,
        PORTFOLIO_SCHEMA_ID_V2,
        product_record.portfolio_digest().to_bytes(),
        observation.rent,
        Error::InvalidProduct,
    )?;
    admit_authenticated_records_v2(
        AdmissionReceiptV2 {
            product,
            result_domain,
            portfolio,
        },
        observation.product.product.raw.data,
        observation.product.result_domain.raw.data,
        observation.product.portfolio.raw.data,
    )
    .map_err(|_| Error::InvalidProduct)
}

fn authenticate_basis(
    observation: AffineBatchObservationV2<'_>,
    core_program: Pubkey,
    market: MarketProjectionV2,
    product_id: [u8; 32],
) -> Result<[u8; 32]> {
    let digest = hash(observation.linked_basis.raw.data).to_bytes();
    authenticate_record(
        observation.linked_basis,
        core_program,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        digest,
        observation.rent,
        Error::InvalidBasis,
    )?;
    let linked = LinkedBasisRecordV2::decode(observation.linked_basis.raw.data)
        .map_err(|_| Error::InvalidBasis)?;
    let embedded = linked.basis_record();
    let prefix = embedded
        .get(..BASIS_PRODUCT_LINK_OFFSET_V2)
        .ok_or(Error::InvalidBasis)?;
    let suffix = embedded
        .get(BASIS_PRODUCT_LINK_END_V2..)
        .ok_or(Error::InvalidBasis)?;
    let semantic = hashv(&[BASIS_SEMANTIC_ID_DOMAIN_V2, prefix, suffix]).to_bytes();
    if linked.product_instance_id().to_bytes() != product_id
        || linked.semantic_basis_id().to_bytes() != market.basis_id
        || semantic != market.basis_id
    {
        return Err(Error::InvalidBasis);
    }
    Ok(digest)
}

fn authenticate_positions(
    observation: AffineBatchObservationV2<'_>,
    claims_program: Pubkey,
    market: MarketProjectionV2,
) -> Result<Vec<AffineBatchPositionV2>> {
    let mut output = Vec::with_capacity(observation.positions.len());
    for (left, observed) in observation.positions.iter().enumerate() {
        let value = PositionProjectionV2 {
            outcome_count: u32_at(observed.data, POSITION_CLAIM_COUNT_OFFSET)
                .ok_or(Error::InvalidPosition)?,
            revision: u64_at(observed.data, POSITION_REVISION_OFFSET)
                .ok_or(Error::InvalidPosition)?,
            market: array_at(observed.data, POSITION_MARKET_OFFSET)
                .ok_or(Error::InvalidPosition)?,
            owner: array_at(observed.data, POSITION_OWNER_OFFSET).ok_or(Error::InvalidPosition)?,
            basis_id: array_at(observed.data, POSITION_BASIS_OFFSET)
                .ok_or(Error::InvalidPosition)?,
        };
        let width = usize::try_from(value.outcome_count)
            .ok()
            .and_then(|count| count.checked_mul(8))
            .and_then(|tail| LIABILITY_BASIS_POSITION_HEADER_BYTES_V2.checked_add(tail))
            .ok_or(Error::InvalidPosition)?;
        let position_seeds =
            ProtocolPositionSeedsV2::new(observation.claims_market.key.to_bytes(), value.owner)
                .map_err(|_| Error::InvalidPosition)?;
        let expected = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
        if array_at::<8>(observed.data, 0) != Some(POSITION_MAGIC_V2)
            || u16_at(observed.data, 8) != Some(ABI_VERSION_V2)
            || observed.owner != claims_program
            || observed.executable
            || observed.data.len() != width
            || observed.key != expected
            || value.outcome_count != market.outcome_count
            || value.market != observation.claims_market.key.to_bytes()
            || value.basis_id != market.basis_id
            || observation
                .positions
                .iter()
                .skip(left.saturating_add(1))
                .any(|right| right.key == observed.key)
        {
            return Err(Error::InvalidPosition);
        }
        output.push(
            AffineBatchPositionV2::new(value.owner, value.revision)
                .map_err(|_| Error::InvalidPosition)?,
        );
    }
    Ok(output)
}

fn authenticate_record(
    observation: FinalizedRecordObservationV2<'_>,
    owner: Pubkey,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    rent: &Rent,
    refusal: Error,
) -> Result<FinalizedRecordCoordinateV2> {
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &expected_digest], &owner)
            .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &expected_digest],
        &owner,
    )
    .0;
    if observation.raw.key != raw
        || observation.raw.owner != owner
        || observation.raw.executable
        || hash(observation.raw.data).to_bytes() != expected_digest
        || !rent.is_exempt(observation.raw.lamports, observation.raw.data.len())
        || observation.staging.key != staging
        || observation.staging.owner != system_program::ID
        || observation.staging.executable
        || !observation.staging.data.is_empty()
    {
        return Err(refusal);
    }
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).map_err(|_| refusal)?,
        content_digest: ContentId::new(expected_digest).map_err(|_| refusal)?,
        raw_account: ContentId::new(raw.to_bytes()).map_err(|_| refusal)?,
        staging_account: ContentId::new(staging.to_bytes()).map_err(|_| refusal)?,
    })
}

const fn execution_role(role: CallerRole) -> ExecutionRoleV1 {
    match role {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    }
}

fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset.checked_add(N)?)?.try_into().ok()
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    array_at(bytes, offset).map(u16::from_le_bytes)
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    array_at(bytes, offset).map(u32::from_le_bytes)
}

fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    array_at(bytes, offset).map(u64::from_le_bytes)
}
