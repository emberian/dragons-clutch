//! Fixed-width sparse native transfer over canonical LiabilityBasisV2 state.
//!
//! The adapter reauthenticates the complete Product Runtime V3 graph, current
//! release roles, open Core Market, aggregate, and two exact Position PDAs. It
//! changes one selected source/destination coordinate, advances all three
//! optimistic revisions once, borrows every writable account, and commits last.

extern crate alloc;

use dclutch_claims_svm::{
    CallerRole,
    composition_v3::validate_sparse_admission_receipt_v3,
    frame_spec_v1::SparseNativeTransferFrameSpecV1,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionV2, ProtocolPositionSeedsV2,
    },
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SparseNativeTransferReceiptV1, SparseNativeTransferV1,
    },
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3, authenticate_product_runtime_v3,
};
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

use super::authenticate_activated_role;

/// Exact fixed account frame for one sparse transfer.
pub const SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1: usize =
    dclutch_claims_svm::frame_spec_v1::SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1 as usize;

const AUTHORITY: usize = 0;
const MARKET: usize = 1;
const BASIS_RAW: usize = 2;
const BASIS_STAGING: usize = 3;
const PRODUCT_RAW: usize = 4;
const PRODUCT_STAGING: usize = 5;
const DOMAIN_RAW: usize = 6;
const DOMAIN_STAGING: usize = 7;
const PORTFOLIO_RAW: usize = 8;
const PORTFOLIO_STAGING: usize = 9;
const RENT: usize = 10;
const CORE_MARKET: usize = 11;
const ACTIVATION_CACHE: usize = 12;
const REGISTRY_PROGRAM: usize = 13;
const CALLER_PROGRAM: usize = 14;
const CALLER_PROGRAMDATA: usize = 15;
const CLAIMS_PROGRAM: usize = 16;
const CLAIMS_PROGRAMDATA: usize = 17;
const CORE_PROGRAM: usize = 18;
const CORE_PROGRAMDATA: usize = 19;
const SOURCE_POSITION: usize = 20;
const DESTINATION_POSITION: usize = 21;

const SCALAR_BYTES: usize = 8;
const RESOURCE_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/claims/sparse-native-post/v1";

/// Stable sparse-transfer SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SparseNativeTransferSbfErrorV1 {
    /// Request bytes refused the canonical fixed codec.
    Instruction = 0x5260,
    /// Account count, privilege, owner, or alias refused.
    Accounts = 0x5261,
    /// Registry current-role or caller authority refused.
    Release = 0x5262,
    /// Product Runtime V3, linked basis, or Core join refused.
    Product = 0x5263,
    /// Aggregate or Position identity, width, PDA, or revision refused.
    ClaimsState = 0x5264,
    /// Debit, credit, revision, or conservation arithmetic refused.
    Candidate = 0x5265,
    /// Candidate accounts could not all be borrowed and committed last.
    Commit = 0x5266,
    /// Exact success receipt construction refused.
    Receipt = 0x5267,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    SparseNativeTransferSbfErrorV1::Instruction as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x260,
    "SparseNativeTransferSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (SparseNativeTransferSbfErrorV1::Receipt as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "SparseNativeTransferSbfErrorV1 must not run past its registered refusal band"
);

impl From<SparseNativeTransferSbfErrorV1> for ProgramError {
    fn from(value: SparseNativeTransferSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct Accounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    basis_raw: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_raw: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    domain_raw: &'accounts AccountInfo<'info>,
    domain_staging: &'accounts AccountInfo<'info>,
    portfolio_raw: &'accounts AccountInfo<'info>,
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
    source: &'accounts AccountInfo<'info>,
    destination: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> Accounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1 {
            return Err(SparseNativeTransferSbfErrorV1::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY)?,
            market: account(accounts, MARKET)?,
            basis_raw: account(accounts, BASIS_RAW)?,
            basis_staging: account(accounts, BASIS_STAGING)?,
            product_raw: account(accounts, PRODUCT_RAW)?,
            product_staging: account(accounts, PRODUCT_STAGING)?,
            domain_raw: account(accounts, DOMAIN_RAW)?,
            domain_staging: account(accounts, DOMAIN_STAGING)?,
            portfolio_raw: account(accounts, PORTFOLIO_RAW)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING)?,
            rent: account(accounts, RENT)?,
            core_market: account(accounts, CORE_MARKET)?,
            cache: account(accounts, ACTIVATION_CACHE)?,
            registry: account(accounts, REGISTRY_PROGRAM)?,
            caller_program: account(accounts, CALLER_PROGRAM)?,
            caller_programdata: account(accounts, CALLER_PROGRAMDATA)?,
            claims_program: account(accounts, CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA)?,
            core_program: account(accounts, CORE_PROGRAM)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA)?,
            source: account(accounts, SOURCE_POSITION)?,
            destination: account(accounts, DESTINATION_POSITION)?,
        })
    }
}

/// Execute one authenticated sparse native transfer.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let (request_bytes, admission) = split_instruction(instruction_data)?;
    let request = SparseNativeTransferV1::decode(request_bytes)
        .map_err(|_| SparseNativeTransferSbfErrorV1::Instruction)?;
    let accounts = Accounts::parse(account_infos)?;
    authenticate_privileges(program_id, account_infos, accounts)?;
    let packet_digest = hash(request_bytes).to_bytes();
    authenticate_authority(accounts, request, packet_digest)?;
    authenticate_releases(accounts, request)?;
    if let Some(admission) = admission {
        validate_sparse_admission_receipt_v3(
            admission,
            request,
            program_id.to_bytes(),
            accounts.caller_program.key.to_bytes(),
        )
        .map_err(|_| SparseNativeTransferSbfErrorV1::ClaimsState)?;
    }

    let market_before = accounts
        .market
        .try_borrow_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?;
    let market = LiabilityBasisMarketViewV2::decode(&market_before)
        .map_err(|_| SparseNativeTransferSbfErrorV1::ClaimsState)?;
    authenticate_market(program_id, accounts, request, market)?;
    authenticate_product_and_core(accounts, request, market)?;
    let source_before = accounts
        .source
        .try_borrow_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?;
    let destination_before = accounts
        .destination
        .try_borrow_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?;
    authenticate_position(
        program_id,
        accounts.market,
        accounts.source,
        &source_before,
        request.input().source_owner,
        request.input().expected_source_revision,
        market,
    )?;
    authenticate_position(
        program_id,
        accounts.market,
        accounts.destination,
        &destination_before,
        request.input().destination_owner,
        request.input().expected_destination_revision,
        market,
    )?;

    let mut market_candidate = market_before.to_vec();
    let mut source_candidate = source_before.to_vec();
    let mut destination_candidate = destination_before.to_vec();
    drop(destination_before);
    drop(source_before);
    drop(market_before);
    apply_transfer(
        request,
        &mut market_candidate,
        &mut source_candidate,
        &mut destination_candidate,
    )?;

    let input = request.input();
    let post_market_revision = input
        .expected_market_revision
        .checked_add(1)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    let post_source_revision = input
        .expected_source_revision
        .checked_add(1)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    let post_destination_revision = input
        .expected_destination_revision
        .checked_add(1)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    put_u64(&mut market_candidate, 16, post_market_revision)?;
    put_u64(&mut source_candidate, 16, post_source_revision)?;
    put_u64(&mut destination_candidate, 16, post_destination_revision)?;
    let resource_digest = hashv(&[
        RESOURCE_DIGEST_DOMAIN_V1,
        &market_candidate,
        &source_candidate,
        &destination_candidate,
    ])
    .to_bytes();
    let receipt = SparseNativeTransferReceiptV1::new(
        request,
        packet_digest,
        program_id.to_bytes(),
        resource_digest,
        post_market_revision,
        post_source_revision,
        post_destination_revision,
    )
    .map_err(|_| SparseNativeTransferSbfErrorV1::Receipt)?;
    commit_candidates(
        accounts,
        &market_candidate,
        &source_candidate,
        &destination_candidate,
    )?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

fn split_instruction(
    instruction_data: &[u8],
) -> Result<(&[u8], Option<ProtocolPositionAdmissionV2>), ProgramError> {
    let request = instruction_data
        .get(..SPARSE_NATIVE_TRANSFER_BYTES_V1)
        .ok_or(SparseNativeTransferSbfErrorV1::Instruction)?;
    let suffix = instruction_data
        .get(SPARSE_NATIVE_TRANSFER_BYTES_V1..)
        .ok_or(SparseNativeTransferSbfErrorV1::Instruction)?;
    let admission = if suffix.is_empty() {
        None
    } else if suffix.len() == PROTOCOL_POSITION_ADMISSION_BYTES_V2 {
        Some(
            ProtocolPositionAdmissionV2::decode_receipt(suffix)
                .map_err(|_| SparseNativeTransferSbfErrorV1::Instruction)?,
        )
    } else {
        return Err(SparseNativeTransferSbfErrorV1::Instruction.into());
    };
    Ok((request, admission))
}

fn authenticate_privileges(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    accounts: Accounts<'_, '_>,
) -> Result<(), ProgramError> {
    let spec = SparseNativeTransferFrameSpecV1;
    if account_infos.len() != usize::from(spec.account_count()) {
        return Err(SparseNativeTransferSbfErrorV1::Accounts.into());
    }
    for (index, observed) in account_infos.iter().enumerate() {
        let expected = spec
            .account(u16::try_from(index).map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?)
            .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?
            .privileges();
        if observed.is_signer != expected.signer()
            || observed.is_writable != expected.writable()
            || observed.executable != expected.executable()
        {
            return Err(SparseNativeTransferSbfErrorV1::Accounts.into());
        }
    }
    if accounts.market.key == accounts.source.key
        || accounts.market.key == accounts.destination.key
        || accounts.source.key == accounts.destination.key
        || accounts.claims_program.key != program_id
        || accounts.rent.key != &sysvar::rent::ID
    {
        return Err(SparseNativeTransferSbfErrorV1::Accounts.into());
    }
    Ok(())
}

fn authenticate_authority(
    accounts: Accounts<'_, '_>,
    request: SparseNativeTransferV1,
    packet_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let input = request.input();
    let seeds = CallerAuthoritySeedsV1::new(
        CoreContentId::new(input.release_set)
            .map_err(|_| SparseNativeTransferSbfErrorV1::Release)?,
        input.market,
        execution_role(input.caller_role),
        input.request_id,
        packet_digest,
    )
    .map_err(|_| SparseNativeTransferSbfErrorV1::Release)?;
    if accounts.authority.key
        != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
    {
        return Err(SparseNativeTransferSbfErrorV1::Release.into());
    }
    Ok(())
}

fn authenticate_releases(
    accounts: Accounts<'_, '_>,
    request: SparseNativeTransferV1,
) -> Result<(), ProgramError> {
    let input = request.input();
    let receipts = [
        authenticate_activated_role(
            accounts.registry,
            accounts.cache,
            execution_role(input.caller_role),
            accounts.caller_program,
            accounts.caller_programdata,
            &input.release_set,
        ),
        authenticate_activated_role(
            accounts.registry,
            accounts.cache,
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
            &input.release_set,
        ),
        authenticate_activated_role(
            accounts.registry,
            accounts.cache,
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
            &input.release_set,
        ),
    ];
    for receipt in receipts {
        let receipt = receipt.map_err(|_| SparseNativeTransferSbfErrorV1::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &input.release_set {
            return Err(SparseNativeTransferSbfErrorV1::Release.into());
        }
    }
    Ok(())
}

fn authenticate_market(
    program_id: &Pubkey,
    accounts: Accounts<'_, '_>,
    request: SparseNativeTransferV1,
    market: LiabilityBasisMarketViewV2,
) -> Result<(), ProgramError> {
    let input = request.input();
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_slice()],
        program_id,
    )
    .0;
    if accounts.market.owner != program_id
        || accounts.market.key != &expected
        || market.logical_market != input.market
        || market.release_set != input.release_set
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.product_instance_id == [0; 32]
        || market.basis_id != input.semantic_basis_id
        || market.claim_count != input.claim_count
        || market.generation != input.generation
        || market.revision != input.expected_market_revision
    {
        return Err(SparseNativeTransferSbfErrorV1::ClaimsState.into());
    }
    Ok(())
}

fn authenticate_product_and_core(
    accounts: Accounts<'_, '_>,
    request: SparseNativeTransferV1,
    market: LiabilityBasisMarketViewV2,
) -> Result<(), ProgramError> {
    let input = request.input();
    let rent = Rent::from_account_info(accounts.rent)
        .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?;
    let product = authenticate_product_runtime_v3(
        accounts.registry.key,
        &rent,
        ProductContentId::new(input.product_record_digest)
            .map_err(|_| SparseNativeTransferSbfErrorV1::Product)?,
        ProductRuntimeFrameV3 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_raw,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.domain_raw,
                staging: accounts.domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_raw,
                staging: accounts.portfolio_staging,
            },
            linked_basis: FinalizedRecordFrameV2 {
                raw: accounts.basis_raw,
                staging: accounts.basis_staging,
            },
        },
    )
    .map_err(|_| SparseNativeTransferSbfErrorV1::Product)?;
    if product.runtime.product_record.content_digest.to_bytes() != input.product_record_digest
        || product.runtime.product_id.to_bytes() != market.product_instance_id
        || product.runtime.liability_basis_id.to_bytes() != input.semantic_basis_id
        || product.semantic_basis_id.to_bytes() != input.semantic_basis_id
        || product.linked_basis_record.content_digest.to_bytes() != input.linked_basis_record_digest
        || product.runtime.outcome_count != input.claim_count
        || product.basis_width != input.claim_count
    {
        return Err(SparseNativeTransferSbfErrorV1::Product.into());
    }
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Accounts)?;
    if accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.key.to_bytes() != input.market
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(SparseNativeTransferSbfErrorV1::Product.into());
    }
    let core =
        CoreState::decode(&core_data).map_err(|_| SparseNativeTransferSbfErrorV1::Product)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        accounts.core_program.key,
    )
    .0;
    if expected != *accounts.core_market.key
        || core.phase != CorePhase::Open
        || core.identity.market_id.to_bytes() != input.market
        || core.identity.product_record.to_bytes() != input.product_record_digest
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.realm_id.to_bytes() != market.realm_id
        || core.identity.selected_release_set.to_bytes() != input.release_set
        || core.identity.registry_program.to_bytes() != accounts.registry.key.to_bytes()
        || core.identity.generation != input.generation
    {
        return Err(SparseNativeTransferSbfErrorV1::Product.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_position(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    data: &[u8],
    owner: [u8; 32],
    revision: u64,
    market: LiabilityBasisMarketViewV2,
) -> Result<(), ProgramError> {
    let seeds = ProtocolPositionSeedsV2::new(market_account.key.to_bytes(), owner)
        .map_err(|_| SparseNativeTransferSbfErrorV1::ClaimsState)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    let position = LiabilityBasisPositionViewV2::decode(data)
        .map_err(|_| SparseNativeTransferSbfErrorV1::ClaimsState)?;
    if account.owner != program_id
        || account.key != &expected
        || position.market_account != market_account.key.to_bytes()
        || position.owner != owner
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
        || position.revision != revision
    {
        return Err(SparseNativeTransferSbfErrorV1::ClaimsState.into());
    }
    Ok(())
}

fn apply_transfer(
    request: SparseNativeTransferV1,
    market: &mut [u8],
    source: &mut [u8],
    destination: &mut [u8],
) -> Result<(), ProgramError> {
    let input = request.input();
    let market_before = read_claim(
        market,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        input.outcome,
    )?;
    let source_before = read_claim(
        source,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        input.outcome,
    )?;
    let destination_before = read_claim(
        destination,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        input.outcome,
    )?;
    let source_after = source_before
        .checked_sub(input.quantity)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    let destination_after = destination_before
        .checked_add(input.quantity)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    if u128::from(source_before) + u128::from(destination_before)
        != u128::from(source_after) + u128::from(destination_after)
    {
        return Err(SparseNativeTransferSbfErrorV1::Candidate.into());
    }
    put_claim(
        source,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        input.outcome,
        source_after,
    )?;
    put_claim(
        destination,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        input.outcome,
        destination_after,
    )?;
    if read_claim(
        market,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        input.outcome,
    )? != market_before
    {
        return Err(SparseNativeTransferSbfErrorV1::Candidate.into());
    }
    Ok(())
}

fn commit_candidates(
    accounts: Accounts<'_, '_>,
    market_candidate: &[u8],
    source_candidate: &[u8],
    destination_candidate: &[u8],
) -> Result<(), ProgramError> {
    let mut market = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Commit)?;
    let mut source = accounts
        .source
        .try_borrow_mut_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Commit)?;
    let mut destination = accounts
        .destination
        .try_borrow_mut_data()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Commit)?;
    if market.len() != market_candidate.len()
        || source.len() != source_candidate.len()
        || destination.len() != destination_candidate.len()
    {
        return Err(SparseNativeTransferSbfErrorV1::Commit.into());
    }
    market.copy_from_slice(market_candidate);
    source.copy_from_slice(source_candidate);
    destination.copy_from_slice(destination_candidate);
    Ok(())
}

fn read_claim(bytes: &[u8], header: usize, outcome: u32) -> Result<u64, ProgramError> {
    let offset = claim_offset(header, outcome)?;
    read_u64(bytes, offset)
}

fn put_claim(
    bytes: &mut [u8],
    header: usize,
    outcome: u32,
    value: u64,
) -> Result<(), ProgramError> {
    let offset = claim_offset(header, outcome)?;
    put_u64(bytes, offset, value)
}

fn claim_offset(header: usize, outcome: u32) -> Result<usize, ProgramError> {
    usize::try_from(outcome)
        .ok()
        .and_then(|value| value.checked_mul(SCALAR_BYTES))
        .and_then(|value| header.checked_add(value))
        .ok_or_else(|| SparseNativeTransferSbfErrorV1::Candidate.into())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    let value: [u8; SCALAR_BYTES] = bytes
        .get(offset..end)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?
        .try_into()
        .map_err(|_| SparseNativeTransferSbfErrorV1::Candidate)?;
    Ok(u64::from_le_bytes(value))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?;
    bytes
        .get_mut(offset..end)
        .ok_or(SparseNativeTransferSbfErrorV1::Candidate)?
        .copy_from_slice(&value.to_le_bytes());
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
        .ok_or_else(|| SparseNativeTransferSbfErrorV1::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims_svm::{
        CallerRole,
        liability_basis_state_v2::{
            LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
            encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
        },
        sparse_native_transfer_v1::{SparseNativeTransferInputV1, SparseNativeTransferV1},
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request(quantity: u64) -> SparseNativeTransferV1 {
        SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: CallerRole::Trading,
            release_set: id(1),
            market: id(2),
            request_id: id(3),
            product_record_digest: id(4),
            semantic_basis_id: id(5),
            linked_basis_record_digest: id(6),
            source_owner: id(7),
            destination_owner: id(8),
            expected_market_revision: 9,
            expected_source_revision: 10,
            expected_destination_revision: 11,
            generation: 12,
            outcome: 1,
            claim_count: 3,
            quantity,
        })
        .expect("request")
    }

    fn states() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 24];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 9,
                logical_market: id(2),
                release_set: id(1),
                registry_program: id(20),
                product_instance_id: id(21),
                basis_id: id(5),
                realm_id: id(22),
                custody_context: id(23),
                generation: 12,
            },
            &[100, 100, 100],
            &mut market,
        )
        .expect("market");
        let mut source = vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 24];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 10,
                market_account: id(24),
                owner: id(7),
                basis_id: id(5),
            },
            &[10, 20, 30],
            &mut source,
        )
        .expect("source");
        let mut destination = vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 24];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 11,
                market_account: id(24),
                owner: id(8),
                basis_id: id(5),
            },
            &[1, 2, 3],
            &mut destination,
        )
        .expect("destination");
        (market, source, destination)
    }

    #[test]
    fn sparse_transfer_is_exact_and_conserved() {
        let (mut market, mut source, mut destination) = states();
        let market_before = market.clone();
        apply_transfer(request(7), &mut market, &mut source, &mut destination).expect("transfer");
        assert_eq!(market, market_before);
        assert_eq!(
            read_claim(&source, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 1),
            Ok(13)
        );
        assert_eq!(
            read_claim(&destination, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 1),
            Ok(9)
        );
    }

    #[test]
    fn underflow_refuses_without_candidate_mutation() {
        let (mut market, mut source, mut destination) = states();
        let before = (market.clone(), source.clone(), destination.clone());
        assert_eq!(
            apply_transfer(request(21), &mut market, &mut source, &mut destination),
            Err(SparseNativeTransferSbfErrorV1::Candidate.into())
        );
        assert_eq!((market, source, destination), before);
    }
}
