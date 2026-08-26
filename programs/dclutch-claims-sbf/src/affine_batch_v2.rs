//! Authenticated runtime-width affine batches over LiabilityBasisV2 Claims.
//!
//! The adapter independently authenticates the current Product Runtime V2
//! graph, the exact linked-basis raw record, the Core Market, current release
//! programs, aggregate state, and every unique Position-table entry. It builds
//! all candidates in memory, verifies the complete receipt commitment, borrows
//! every writable account, and only then copies any candidate bytes.

extern crate alloc;

use alloc::vec::Vec;
use core::{
    cell::RefMut,
    convert::{TryFrom, TryInto},
};

use dclutch_claims_svm::{
    CallerRole,
    affine_batch_v2::{
        AffineBatchPlanV2, AffineBatchReceiptV2, DeltaDirectionV2, SignedMagnitudeV2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_liability_basis_v2_kernel::product_claims::LinkedBasisRecordV2;
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::sysvar;

use super::{product_runtime_v2::authenticate_product_runtime_v2, reauthenticate};
use crate::liability_basis_v2::{
    BASIS_PRODUCT_LINK_END_V2, BASIS_PRODUCT_LINK_OFFSET_V2, BASIS_SEMANTIC_ID_DOMAIN_V2,
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_SEED_V2,
    LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, MarketViewV2, PositionViewV2,
    authenticate_self_finalized_record,
};

/// Exact fixed affine-batch account count before the runtime Position tail.
pub const AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2: usize = 20;

const AUTHORITY_ACCOUNT: usize = 0;
const MARKET_ACCOUNT: usize = 1;
const BASIS_RECORD_ACCOUNT: usize = 2;
const BASIS_STAGING_ACCOUNT: usize = 3;
const PRODUCT_RECORD_ACCOUNT: usize = 4;
const PRODUCT_STAGING_ACCOUNT: usize = 5;
const RESULT_DOMAIN_RECORD_ACCOUNT: usize = 6;
const RESULT_DOMAIN_STAGING_ACCOUNT: usize = 7;
const PORTFOLIO_RECORD_ACCOUNT: usize = 8;
const PORTFOLIO_STAGING_ACCOUNT: usize = 9;
const RENT_ACCOUNT: usize = 10;
const CORE_MARKET_ACCOUNT: usize = 11;
const ACTIVATION_CACHE_ACCOUNT: usize = 12;
const REGISTRY_PROGRAM_ACCOUNT: usize = 13;
const CALLER_PROGRAM_ACCOUNT: usize = 14;
const CALLER_PROGRAMDATA_ACCOUNT: usize = 15;
const CLAIMS_PROGRAM_ACCOUNT: usize = 16;
const CLAIMS_PROGRAMDATA_ACCOUNT: usize = 17;
const CORE_PROGRAM_ACCOUNT: usize = 18;
const CORE_PROGRAMDATA_ACCOUNT: usize = 19;

const MARKET_REVISION_OFFSET: usize = 16;
const POSITION_REVISION_OFFSET: usize = 16;
const SCALAR_BYTES: usize = 8;
const TABLE_DIGEST_DOMAIN_V2: &[u8] = b"dclutch/claims/affine-table/v2";
const RESOURCE_DIGEST_DOMAIN_V2: &[u8] = b"dclutch/claims/affine-post-resources/v2";

/// Stable affine-batch SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AffineBatchSbfErrorV2 {
    /// Instruction bytes did not decode as the canonical public ABI.
    Instruction = 160,
    /// Account count, order, privileges, owners, or aliases refused.
    Accounts = 161,
    /// Registry current-release authentication or caller authority refused.
    Release = 162,
    /// Product graph, linked basis, semantic identity, or Core join refused.
    ProductBasis = 163,
    /// Aggregate or Position PDA, width, identity, or revision refused.
    ClaimsState = 164,
    /// An exact delta overflowed, underflowed, or selected an invalid coordinate.
    Candidate = 165,
    /// Complete candidate buffers could not all be borrowed and committed last.
    Commit = 166,
    /// The canonical success receipt could not be constructed.
    Receipt = 167,
}

impl From<AffineBatchSbfErrorV2> for ProgramError {
    fn from(value: AffineBatchSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct AffineBatchAccountsV2<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_domain_record: &'accounts AccountInfo<'info>,
    result_domain_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    caller_program: &'accounts AccountInfo<'info>,
    caller_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    positions: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> AffineBatchAccountsV2<'accounts, 'info> {
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        position_count: u32,
    ) -> Result<Self, ProgramError> {
        let count = usize::try_from(position_count)
            .ok()
            .and_then(|count| AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2.checked_add(count))
            .ok_or(AffineBatchSbfErrorV2::Accounts)?;
        if accounts.len() != count {
            return Err(AffineBatchSbfErrorV2::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY_ACCOUNT)?,
            market: account(accounts, MARKET_ACCOUNT)?,
            basis_record: account(accounts, BASIS_RECORD_ACCOUNT)?,
            basis_staging: account(accounts, BASIS_STAGING_ACCOUNT)?,
            product_record: account(accounts, PRODUCT_RECORD_ACCOUNT)?,
            product_staging: account(accounts, PRODUCT_STAGING_ACCOUNT)?,
            result_domain_record: account(accounts, RESULT_DOMAIN_RECORD_ACCOUNT)?,
            result_domain_staging: account(accounts, RESULT_DOMAIN_STAGING_ACCOUNT)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD_ACCOUNT)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING_ACCOUNT)?,
            rent: account(accounts, RENT_ACCOUNT)?,
            core_market: account(accounts, CORE_MARKET_ACCOUNT)?,
            cache: account(accounts, ACTIVATION_CACHE_ACCOUNT)?,
            registry: account(accounts, REGISTRY_PROGRAM_ACCOUNT)?,
            caller_program: account(accounts, CALLER_PROGRAM_ACCOUNT)?,
            caller_programdata: account(accounts, CALLER_PROGRAMDATA_ACCOUNT)?,
            claims_program: account(accounts, CLAIMS_PROGRAM_ACCOUNT)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA_ACCOUNT)?,
            core_program: account(accounts, CORE_PROGRAM_ACCOUNT)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA_ACCOUNT)?,
            positions: accounts
                .get(AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2..)
                .ok_or(AffineBatchSbfErrorV2::Accounts)?,
        })
    }
}

/// Execute one authenticated, runtime-width affine Claims batch.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let plan = AffineBatchPlanV2::decode(instruction_data)
        .map_err(|_| AffineBatchSbfErrorV2::Instruction)?;
    let accounts = AffineBatchAccountsV2::parse(account_infos, plan.position_count())?;
    authenticate_privileges(program_id, &accounts)?;
    let packet_digest = hash(instruction_data).to_bytes();
    authenticate_authority(&accounts, plan, packet_digest)?;
    authenticate_releases(&accounts, plan)?;

    let market_before = accounts
        .market
        .try_borrow_data()
        .map_err(|_| AffineBatchSbfErrorV2::Accounts)?;
    let market =
        MarketViewV2::decode(&market_before).map_err(|_| AffineBatchSbfErrorV2::ClaimsState)?;
    authenticate_market(program_id, &accounts, plan, market)?;
    authenticate_product_and_basis(&accounts, plan, market)?;
    let (mut market_candidate, mut position_candidates) =
        build_candidates(program_id, &accounts, plan, market, &market_before)?;
    drop(market_before);

    apply_rows(plan, &mut market_candidate, &mut position_candidates)?;
    let post_market_revision = plan
        .expected_market_revision()
        .checked_add(1)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    put_u64(
        &mut market_candidate,
        MARKET_REVISION_OFFSET,
        post_market_revision,
    )?;
    for candidate in &mut position_candidates {
        let revision = read_u64(candidate, POSITION_REVISION_OFFSET)?
            .checked_add(1)
            .ok_or(AffineBatchSbfErrorV2::Candidate)?;
        put_u64(candidate, POSITION_REVISION_OFFSET, revision)?;
    }

    let (position_table, rows) = plan.table_bytes();
    let table_digest = hashv(&[TABLE_DIGEST_DOMAIN_V2, position_table, rows]).to_bytes();
    let post_resource_digest = resource_digest(&market_candidate, &position_candidates);
    let receipt = AffineBatchReceiptV2::new(
        plan,
        packet_digest,
        table_digest,
        program_id.to_bytes(),
        post_resource_digest,
        post_market_revision,
    )
    .map_err(|_| AffineBatchSbfErrorV2::Receipt)?;
    let receipt_bytes = receipt.to_bytes();
    commit_candidates(&accounts, &market_candidate, &position_candidates)?;
    set_return_data(&receipt_bytes);
    Ok(())
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &AffineBatchAccountsV2<'_, '_>,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || !accounts.market.is_writable
        || accounts.market.is_signer
        || accounts.market.executable
        || accounts.claims_program.key != program_id
        || !accounts.claims_program.executable
        || accounts.claims_program.is_signer
        || accounts.claims_program.is_writable
        || !accounts.registry.executable
        || accounts.registry.is_signer
        || accounts.registry.is_writable
        || !accounts.caller_program.executable
        || accounts.caller_program.is_signer
        || accounts.caller_program.is_writable
        || !accounts.core_program.executable
        || accounts.core_program.is_signer
        || accounts.core_program.is_writable
        || accounts.rent.key != &sysvar::rent::ID
    {
        return Err(AffineBatchSbfErrorV2::Accounts.into());
    }
    for account in [
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_domain_record,
        accounts.result_domain_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
        accounts.rent,
        accounts.core_market,
        accounts.cache,
        accounts.caller_programdata,
        accounts.claims_programdata,
        accounts.core_programdata,
    ] {
        if account.is_signer || account.is_writable || account.executable {
            return Err(AffineBatchSbfErrorV2::Accounts.into());
        }
    }
    for (left, position) in accounts.positions.iter().enumerate() {
        if !position.is_writable
            || position.is_signer
            || position.executable
            || position.key == accounts.market.key
            || accounts
                .positions
                .iter()
                .skip(left.saturating_add(1))
                .any(|right| right.key == position.key)
        {
            return Err(AffineBatchSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

fn authenticate_authority(
    accounts: &AffineBatchAccountsV2<'_, '_>,
    plan: AffineBatchPlanV2<'_>,
    packet_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let role = execution_role(plan.caller_role());
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).map_err(|_| AffineBatchSbfErrorV2::Release)?,
        plan.market(),
        role,
        plan.request_id(),
        packet_digest,
    )
    .map_err(|_| AffineBatchSbfErrorV2::Release)?;
    if accounts.authority.key
        != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
    {
        return Err(AffineBatchSbfErrorV2::Release.into());
    }
    Ok(())
}

fn authenticate_releases(
    accounts: &AffineBatchAccountsV2<'_, '_>,
    plan: AffineBatchPlanV2<'_>,
) -> Result<(), ProgramError> {
    let caller = reauthenticate(
        accounts.registry,
        accounts.cache,
        execution_role(plan.caller_role()),
        accounts.caller_program,
        accounts.caller_programdata,
    )
    .map_err(|_| AffineBatchSbfErrorV2::Release)?;
    let claims = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Claims,
        accounts.claims_program,
        accounts.claims_programdata,
    )
    .map_err(|_| AffineBatchSbfErrorV2::Release)?;
    let core = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )
    .map_err(|_| AffineBatchSbfErrorV2::Release)?;
    for receipt in [caller, claims, core] {
        if receipt.execution_release_set_id().as_bytes() != &plan.release_set() {
            return Err(AffineBatchSbfErrorV2::Release.into());
        }
    }
    Ok(())
}

fn authenticate_market(
    program_id: &Pubkey,
    accounts: &AffineBatchAccountsV2<'_, '_>,
    plan: AffineBatchPlanV2<'_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let expected_market = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        program_id,
    )
    .0;
    if accounts.market.owner != program_id
        || accounts.market.key != &expected_market
        || market.logical_market != plan.market()
        || market.release_set != plan.release_set()
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.product_instance_id == [0; 32]
        || market.basis_id != plan.semantic_basis_id()
        || market.claim_count != plan.outcome_count()
        || market.revision != plan.expected_market_revision()
    {
        return Err(AffineBatchSbfErrorV2::ClaimsState.into());
    }
    Ok(())
}

fn authenticate_product_and_basis(
    accounts: &AffineBatchAccountsV2<'_, '_>,
    plan: AffineBatchPlanV2<'_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    authenticate_runtime_product_basis_core_v2(
        accounts.registry,
        accounts.rent,
        accounts.core_market,
        accounts.core_program,
        accounts.basis_record,
        accounts.basis_staging,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_domain_record,
                staging: accounts.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
        },
        market,
        plan.product_record_digest(),
        plan.linked_basis_record_digest(),
    )
}

/// Independently authenticate the Product Runtime V2 graph, linked liability
/// basis raw record, and exact open Core Market selected by an LBV2 Market.
///
/// This is the reusable, read-only Claims admission boundary. The Product
/// graph-root digest, Product semantic identity, semantic basis identity, and
/// linked-record digest remain distinct joins; no receipt or decoded DTO is a
/// substitute for any of them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_runtime_product_basis_core_v2(
    registry: &AccountInfo<'_>,
    rent_account: &AccountInfo<'_>,
    core_market: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    basis_record: &AccountInfo<'_>,
    basis_staging: &AccountInfo<'_>,
    product_frame: ProductRuntimeFrameV2<'_, '_>,
    market: MarketViewV2,
    expected_product_record_digest: [u8; 32],
    expected_linked_basis_record_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let rent =
        Rent::from_account_info(rent_account).map_err(|_| AffineBatchSbfErrorV2::Accounts)?;
    let product = authenticate_product_runtime_v2(
        registry.key,
        &rent,
        expected_product_record_digest,
        None,
        product_frame,
    )
    .map_err(|_| AffineBatchSbfErrorV2::ProductBasis)?;
    authenticate_self_finalized_record(
        core_program,
        rent_account,
        basis_record,
        basis_staging,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
    )
    .map_err(|_| AffineBatchSbfErrorV2::ProductBasis)?;
    let basis_data = basis_record
        .try_borrow_data()
        .map_err(|_| AffineBatchSbfErrorV2::Accounts)?;
    if hash(&basis_data).to_bytes() != expected_linked_basis_record_digest {
        return Err(AffineBatchSbfErrorV2::ProductBasis.into());
    }
    let linked = LinkedBasisRecordV2::decode(&basis_data)
        .map_err(|_| AffineBatchSbfErrorV2::ProductBasis)?;
    let embedded = linked.basis_record();
    let prefix = embedded
        .get(..BASIS_PRODUCT_LINK_OFFSET_V2)
        .ok_or(AffineBatchSbfErrorV2::ProductBasis)?;
    let suffix = embedded
        .get(BASIS_PRODUCT_LINK_END_V2..)
        .ok_or(AffineBatchSbfErrorV2::ProductBasis)?;
    let semantic_basis_id = hashv(&[BASIS_SEMANTIC_ID_DOMAIN_V2, prefix, suffix]).to_bytes();
    if product.product_record.content_digest.to_bytes() != expected_product_record_digest
        || product.product_id.to_bytes() != market.product_instance_id
        || product.liability_basis_id.to_bytes() != market.basis_id
        || product.outcome_count != market.claim_count
        || linked.product_instance_id().to_bytes() != product.product_id.to_bytes()
        || linked.semantic_basis_id().to_bytes() != market.basis_id
        || semantic_basis_id != market.basis_id
    {
        return Err(AffineBatchSbfErrorV2::ProductBasis.into());
    }
    let core_data = core_market
        .try_borrow_data()
        .map_err(|_| AffineBatchSbfErrorV2::Accounts)?;
    if core_market.owner != core_program.key
        || core_market.key.to_bytes() != market.logical_market
        || core_market.data_len() != STATE_BYTES
    {
        return Err(AffineBatchSbfErrorV2::ProductBasis.into());
    }
    let core = CoreState::decode(&core_data).map_err(|_| AffineBatchSbfErrorV2::ProductBasis)?;
    let expected_core = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        core_program.key,
    )
    .0;
    if expected_core != *core_market.key
        || core.phase != CorePhase::Open
        || core.identity.market_id.to_bytes() != market.logical_market
        || core.identity.product_record.to_bytes() != expected_product_record_digest
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.registry_program.to_bytes() != registry.key.to_bytes()
        || core.identity.generation != market.generation
    {
        return Err(AffineBatchSbfErrorV2::ProductBasis.into());
    }
    Ok(())
}

fn build_candidates(
    program_id: &Pubkey,
    accounts: &AffineBatchAccountsV2<'_, '_>,
    plan: AffineBatchPlanV2<'_>,
    market: MarketViewV2,
    market_before: &[u8],
) -> Result<(Vec<u8>, Vec<Vec<u8>>), ProgramError> {
    let mut candidates = Vec::with_capacity(accounts.positions.len());
    for (index, account) in accounts.positions.iter().enumerate() {
        let table_index = u32::try_from(index).map_err(|_| AffineBatchSbfErrorV2::ClaimsState)?;
        let expected = plan
            .position(table_index)
            .map_err(|_| AffineBatchSbfErrorV2::Instruction)?;
        let expected_key = Pubkey::find_program_address(
            &[
                LIABILITY_BASIS_POSITION_SEED_V2,
                accounts.market.key.as_ref(),
                expected.owner().as_slice(),
            ],
            program_id,
        )
        .0;
        let data = account
            .try_borrow_data()
            .map_err(|_| AffineBatchSbfErrorV2::Accounts)?;
        let position =
            PositionViewV2::decode(&data).map_err(|_| AffineBatchSbfErrorV2::ClaimsState)?;
        if account.owner != program_id
            || account.key != &expected_key
            || position.market_account != accounts.market.key.to_bytes()
            || position.owner != expected.owner()
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
            || position.revision != expected.expected_revision()
        {
            return Err(AffineBatchSbfErrorV2::ClaimsState.into());
        }
        candidates.push(data.to_vec());
    }
    Ok((market_before.to_vec(), candidates))
}

fn apply_rows(
    plan: AffineBatchPlanV2<'_>,
    market: &mut [u8],
    positions: &mut [Vec<u8>],
) -> Result<(), ProgramError> {
    for row_index in 0..plan.row_count() {
        let row = plan
            .row(row_index)
            .map_err(|_| AffineBatchSbfErrorV2::Instruction)?;
        apply_coordinate(
            market,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            row.outcome(),
            row.aggregate_delta(),
        )?;
        if row.source_present() {
            apply_position(
                positions,
                row.source_position_index(),
                row.outcome(),
                row.source_delta(),
            )?;
        }
        if row.destination_present() {
            apply_position(
                positions,
                row.destination_position_index(),
                row.outcome(),
                row.destination_delta(),
            )?;
        }
    }
    Ok(())
}

fn apply_position(
    positions: &mut [Vec<u8>],
    index: u32,
    outcome: u32,
    delta: SignedMagnitudeV2,
) -> Result<(), ProgramError> {
    let index = usize::try_from(index).map_err(|_| AffineBatchSbfErrorV2::Candidate)?;
    let candidate = positions
        .get_mut(index)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    apply_coordinate(
        candidate,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        outcome,
        delta,
    )
}

fn apply_coordinate(
    bytes: &mut [u8],
    header: usize,
    outcome: u32,
    delta: SignedMagnitudeV2,
) -> Result<(), ProgramError> {
    let offset = usize::try_from(outcome)
        .ok()
        .and_then(|outcome| outcome.checked_mul(SCALAR_BYTES))
        .and_then(|relative| header.checked_add(relative))
        .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    let before = read_u64(bytes, offset)?;
    let after = match delta.direction() {
        DeltaDirectionV2::Neutral => Some(before),
        DeltaDirectionV2::Credit => before.checked_add(delta.magnitude()),
        DeltaDirectionV2::Debit => before.checked_sub(delta.magnitude()),
    }
    .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    put_u64(bytes, offset, after)
}

fn resource_digest(market: &[u8], positions: &[Vec<u8>]) -> [u8; 32] {
    let mut resources: Vec<&[u8]> = Vec::with_capacity(positions.len().saturating_add(2));
    resources.push(RESOURCE_DIGEST_DOMAIN_V2);
    resources.push(market);
    for position in positions {
        resources.push(position);
    }
    hashv(&resources).to_bytes()
}

fn commit_candidates(
    accounts: &AffineBatchAccountsV2<'_, '_>,
    market_candidate: &[u8],
    position_candidates: &[Vec<u8>],
) -> Result<(), ProgramError> {
    let mut market = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| AffineBatchSbfErrorV2::Commit)?;
    if market.len() != market_candidate.len()
        || position_candidates.len() != accounts.positions.len()
    {
        return Err(AffineBatchSbfErrorV2::Commit.into());
    }
    let mut positions: Vec<RefMut<'_, &mut [u8]>> = Vec::with_capacity(accounts.positions.len());
    for (account, candidate) in accounts.positions.iter().zip(position_candidates) {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| AffineBatchSbfErrorV2::Commit)?;
        if data.len() != candidate.len() {
            return Err(AffineBatchSbfErrorV2::Commit.into());
        }
        positions.push(data);
    }
    market.copy_from_slice(market_candidate);
    for (mut position, candidate) in positions.into_iter().zip(position_candidates) {
        position.copy_from_slice(candidate);
    }
    Ok(())
}

const fn execution_role(role: CallerRole) -> ExecutionRoleV1 {
    match role {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| AffineBatchSbfErrorV2::Accounts.into())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    let field: [u8; SCALAR_BYTES] = bytes
        .get(offset..end)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?
        .try_into()
        .map_err(|_| AffineBatchSbfErrorV2::Candidate)?;
    Ok(u64::from_le_bytes(field))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?;
    bytes
        .get_mut(offset..end)
        .ok_or(AffineBatchSbfErrorV2::Candidate)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use dclutch_claims_svm::affine_batch_v2::{
        AFFINE_BATCH_PLAN_HEADER_BYTES_V2, AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanInputV2,
        AffineBatchPositionV2, AffineBatchRowInputV2, AffineBatchRowV2, DeltaDirectionV2,
        SignedMagnitudeV2, plan_bytes,
    };

    use super::*;

    fn delta(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(direction, magnitude).expect("delta")
    }

    fn plan_bytes_fixture() -> Vec<u8> {
        let positions = [
            AffineBatchPositionV2::new([7; 32], 4).expect("source"),
            AffineBatchPositionV2::new([8; 32], 9).expect("destination"),
        ];
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 1,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
                source_delta: delta(DeltaDirectionV2::Debit, u64::MAX),
                destination_delta: delta(DeltaDirectionV2::Credit, u64::MAX),
            },
            2,
            2,
        )
        .expect("row")];
        let mut bytes = vec![0; plan_bytes(2, 1).expect("width")];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 3,
                outcome_count: 2,
            },
            &positions,
            &rows,
            &mut bytes,
        )
        .expect("encode");
        bytes
    }

    #[test]
    fn candidate_application_is_atomic_and_full_range() {
        let bytes = plan_bytes_fixture();
        let plan = AffineBatchPlanV2::decode(&bytes).expect("plan");
        let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16];
        put_u64(
            &mut market,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8,
            u64::MAX,
        )
        .expect("market supply");
        let mut positions = vec![
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
        ];
        put_u64(
            positions.get_mut(0).expect("source"),
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8,
            u64::MAX,
        )
        .expect("source balance");
        apply_rows(plan, &mut market, &mut positions).expect("full-range transfer");
        assert_eq!(
            read_u64(
                positions.first().expect("source"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8,
            ),
            Ok(0)
        );
        assert_eq!(
            read_u64(
                positions.get(1).expect("destination"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8,
            ),
            Ok(u64::MAX)
        );

        let before = positions.clone();
        assert_eq!(
            apply_rows(plan, &mut market, &mut positions),
            Err(AffineBatchSbfErrorV2::Candidate.into())
        );
        assert_eq!(positions, before, "first-coordinate refusal is nonmutating");
    }

    #[test]
    fn dispatch_magic_is_exact_and_public() {
        let bytes = plan_bytes_fixture();
        assert_eq!(
            bytes.get(..AFFINE_BATCH_PLAN_MAGIC_V2.len()),
            Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice())
        );
        assert!(bytes.len() > AFFINE_BATCH_PLAN_HEADER_BYTES_V2);
    }
}
