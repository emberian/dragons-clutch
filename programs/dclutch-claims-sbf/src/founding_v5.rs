//! Core-authorized atomic Claims founding over the sole LBV2 ledger.
//!
//! The parent Core route has already executed and producer-authenticated the
//! exact Custody source-to-Hoard transfer. This adapter independently joins
//! the current release-set authority, Custody post-observations, finalized
//! Product/basis graph, Founding Core Market, canonical Claims PDAs, and exact
//! prepaid rent before allocating or committing any Claims state.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};
use core::convert::TryFrom;

use dclutch_claims_svm::{
    founding_v5::{
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, ClaimsFoundingAggregateSeedsV5,
        ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_custody_contract::{CallerRoleV1, CustodyReplayV1};
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyLockReceiptV1, ProjectedCustodyReceiptV1,
};
use dclutch_market_core_codec::{
    CoreState, FoundingIntentV5, Identity, Phase as CorePhase, SERIES_FOUNDING_PERMIT_BYTES_V1,
    STATE_BYTES, SeriesFoundingPermitV1,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV3};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::Instruction,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};

use super::{
    affine_batch_v2::authenticate_runtime_product_basis_core_v3, authenticate_activated_role,
};
use crate::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2, MarketViewV2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2, vector_width,
};

pub use dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_ACCOUNT_COUNT_V5;
/// Exact request plus typed projected-Custody receipt instruction width.
pub const CLAIMS_FOUNDING_INSTRUCTION_BYTES_V5: usize =
    dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
        + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1
        + PROJECTED_CUSTODY_RECEIPT_BYTES_V1;

const AUTHORITY: usize = 0;
const PERMIT: usize = 1;
const AGGREGATE: usize = 2;
const POSITION: usize = 3;
const ADMISSION: usize = 4;
const FUNDING_SOURCE: usize = 5;
const HOARD: usize = 6;
const CUSTODY_REPLAY: usize = 7;
const BASIS_RECORD: usize = 8;
const BASIS_STAGING: usize = 9;
const PRODUCT_RECORD: usize = 10;
const PRODUCT_STAGING: usize = 11;
const RESULT_RECORD: usize = 12;
const RESULT_STAGING: usize = 13;
const PORTFOLIO_RECORD: usize = 14;
const PORTFOLIO_STAGING: usize = 15;
const RENT: usize = 16;
const SYSTEM: usize = 17;
const CORE_MARKET: usize = 18;
const CACHE: usize = 19;
const REGISTRY: usize = 20;
const CLAIMS_PROGRAM: usize = 21;
const CLAIMS_PROGRAMDATA: usize = 22;
const CORE_PROGRAM: usize = 23;
const CORE_PROGRAMDATA: usize = 24;
const TRADING_PROGRAM: usize = 25;
const TRADING_PROGRAMDATA: usize = 26;
const CUSTODY_PROGRAM: usize = 27;
const CUSTODY_PROGRAMDATA: usize = 28;
const FOUNDER: usize = 29;
const RENT_CREDIT: usize = 30;
const RENT_PROGRAM: usize = 31;

/// Stable FoundingV5 adapter refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsFoundingSbfErrorV5 {
    /// Instruction bytes did not decode as the sole FoundingV5 ABI.
    Instruction = 180,
    /// Account count, privileges, executable flags, or aliases refused.
    Accounts = 181,
    /// Core caller authority or current release selection refused.
    Release = 182,
    /// Custody source, Hoard, or replay post-observations refused.
    Custody = 183,
    /// Product graph, linked basis, or Founding Core Market refused.
    ProductBasis = 184,
    /// Claims aggregate, Position, or admission PDA/vacancy refused.
    ClaimsState = 185,
    /// Rent sysvar, exact principals, target lamports, or RentCredit refused.
    Rent = 186,
    /// System allocation or assignment refused.
    Allocation = 187,
    /// Candidate receipt or post-resource digest refused.
    Receipt = 188,
    /// State-last copy or immutable postcondition refused.
    Commit = 189,
}

impl From<ClaimsFoundingSbfErrorV5> for ProgramError {
    fn from(value: ClaimsFoundingSbfErrorV5) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct FoundingAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    permit: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    funding_source: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_record: &'accounts AccountInfo<'info>,
    result_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    founder: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FoundingAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CLAIMS_FOUNDING_ACCOUNT_COUNT_V5 {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY)?,
            permit: account(accounts, PERMIT)?,
            aggregate: account(accounts, AGGREGATE)?,
            position: account(accounts, POSITION)?,
            admission: account(accounts, ADMISSION)?,
            funding_source: account(accounts, FUNDING_SOURCE)?,
            hoard: account(accounts, HOARD)?,
            custody_replay: account(accounts, CUSTODY_REPLAY)?,
            basis_record: account(accounts, BASIS_RECORD)?,
            basis_staging: account(accounts, BASIS_STAGING)?,
            product_record: account(accounts, PRODUCT_RECORD)?,
            product_staging: account(accounts, PRODUCT_STAGING)?,
            result_record: account(accounts, RESULT_RECORD)?,
            result_staging: account(accounts, RESULT_STAGING)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING)?,
            rent: account(accounts, RENT)?,
            system: account(accounts, SYSTEM)?,
            core_market: account(accounts, CORE_MARKET)?,
            cache: account(accounts, CACHE)?,
            registry: account(accounts, REGISTRY)?,
            claims_program: account(accounts, CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA)?,
            core_program: account(accounts, CORE_PROGRAM)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA)?,
            trading_program: account(accounts, TRADING_PROGRAM)?,
            trading_programdata: account(accounts, TRADING_PROGRAMDATA)?,
            custody_program: account(accounts, CUSTODY_PROGRAM)?,
            custody_programdata: account(accounts, CUSTODY_PROGRAMDATA)?,
            founder: account(accounts, FOUNDER)?,
            rent_credit: account(accounts, RENT_CREDIT)?,
            rent_program: account(accounts, RENT_PROGRAM)?,
        })
    }
}

/// Execute one exact atomic Claims founding request.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let decoded = decode_instruction(instruction_data)?;
    let request = decoded.request;
    let lock_receipt = decoded.lock_receipt;
    let projected_receipt = decoded.projected_receipt;
    let request_digest = decoded.request_digest;
    let lock_receipt_digest = decoded.lock_receipt_digest;
    let projected_receipt_digest = decoded.projected_receipt_digest;
    let accounts = FoundingAccounts::parse(account_infos)?;
    authenticate_privileges(program_id, accounts, &request)?;
    authenticate_authority(accounts, &request, request_digest)?;
    authenticate_releases(accounts, &request)?;
    authenticate_permit_and_projection(
        accounts,
        &request,
        request_digest,
        &lock_receipt,
        lock_receipt_digest,
        &projected_receipt,
        projected_receipt_digest,
    )?;
    authenticate_custody_poststate(
        accounts,
        &request,
        &projected_receipt,
        projected_receipt_digest,
    )?;
    let market = authenticate_product_core(program_id, accounts, &request)?;
    authenticate_rent_and_vacancy(program_id, accounts, &request, market)?;

    let candidates =
        build_candidates_boxed(program_id, accounts, &request, market, request_digest)?;
    let receipt = build_receipt(&request, request_digest, &candidates)?;

    allocate_all(program_id, accounts, &request, &candidates)?;
    commit_candidates(accounts, &candidates)?;
    set_return_data(receipt.as_slice());
    Ok(())
}

#[inline(never)]
fn decode_instruction(instruction_data: &[u8]) -> Result<DecodedFounding, ProgramError> {
    if instruction_data.len() != CLAIMS_FOUNDING_INSTRUCTION_BYTES_V5 {
        return Err(ClaimsFoundingSbfErrorV5::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(..dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5)
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let projected_receipt_bytes = instruction_data
        .get(
            dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                ..dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                    + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
        )
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    let realized_receipt_bytes = instruction_data
        .get(
            dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_REQUEST_BYTES_V5
                + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1..,
        )
        .ok_or(ClaimsFoundingSbfErrorV5::Instruction)?;
    Ok(DecodedFounding {
        request: Box::new(
            ClaimsFoundingRequestV5::decode(request_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Instruction)?,
        ),
        lock_receipt: Box::new(
            ProjectedCustodyLockReceiptV1::decode(projected_receipt_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        projected_receipt: Box::new(
            ProjectedCustodyReceiptV1::decode(realized_receipt_bytes)
                .map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?,
        ),
        request_digest: hash(request_bytes).to_bytes(),
        lock_receipt_digest: hash(projected_receipt_bytes).to_bytes(),
        projected_receipt_digest: hash(realized_receipt_bytes).to_bytes(),
    })
}

#[inline(never)]
fn build_receipt(
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    candidates: &FoundingCandidates,
) -> Result<
    Box<[u8; dclutch_claims_svm::founding_v5::CLAIMS_FOUNDING_RECEIPT_BYTES_V5]>,
    ProgramError,
> {
    let receipt = ClaimsFoundingReceiptV5::new(
        *request,
        request_digest,
        hash(&candidates.aggregate).to_bytes(),
        hash(&candidates.position).to_bytes(),
        hash(&candidates.admission).to_bytes(),
        hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
            &candidates.aggregate,
            &candidates.position,
            &candidates.admission,
        ])
        .to_bytes(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::Receipt)?;
    Ok(Box::new(receipt.to_bytes()))
}

struct FoundingCandidates {
    aggregate: Vec<u8>,
    position: Vec<u8>,
    admission: [u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2],
}

struct DecodedFounding {
    request: Box<ClaimsFoundingRequestV5>,
    lock_receipt: Box<ProjectedCustodyLockReceiptV1>,
    projected_receipt: Box<ProjectedCustodyReceiptV1>,
    request_digest: [u8; 32],
    lock_receipt_digest: [u8; 32],
    projected_receipt_digest: [u8; 32],
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || accounts.permit.is_signer
        || accounts.permit.is_writable
        || accounts.permit.executable
        || !accounts.aggregate.is_writable
        || !accounts.position.is_writable
        || !accounts.admission.is_writable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.key.to_bytes() != request.claims_program()
        || accounts.trading_program.key.to_bytes() != request.trading_program()
        || !accounts.claims_program.executable
        || !accounts.core_program.executable
        || !accounts.trading_program.executable
        || !accounts.custody_program.executable
        || !accounts.registry.executable
        || !accounts.rent_program.executable
        || accounts.system.key != &system_program::ID
        || !accounts.system.executable
        || accounts.rent.key != &sysvar::rent::ID
    {
        return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
    }
    for readonly in [
        accounts.permit,
        accounts.funding_source,
        accounts.hoard,
        accounts.custody_replay,
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_record,
        accounts.result_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
        accounts.rent,
        accounts.system,
        accounts.core_market,
        accounts.cache,
        accounts.registry,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.core_programdata,
        accounts.trading_program,
        accounts.trading_programdata,
        accounts.custody_program,
        accounts.custody_programdata,
        accounts.founder,
        accounts.rent_credit,
        accounts.rent_program,
    ] {
        if readonly.is_signer || readonly.is_writable {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
    }
    require_distinct(&[
        accounts.authority,
        accounts.permit,
        accounts.aggregate,
        accounts.position,
        accounts.admission,
        accounts.funding_source,
        accounts.hoard,
        accounts.custody_replay,
        accounts.core_market,
        accounts.registry,
        accounts.claims_program,
        accounts.core_program,
        accounts.trading_program,
        accounts.custody_program,
        accounts.founder,
        accounts.rent_credit,
        accounts.rent_program,
    ])
}

#[inline(never)]
fn authenticate_authority(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set(),
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        request_digest,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.trading_program.key).0;
    if accounts.authority.key != &expected {
        return Err(ClaimsFoundingSbfErrorV5::Release.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_permit_and_projection(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_digest: [u8; 32],
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if accounts.permit.owner != accounts.core_program.key
        || accounts.permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
    {
        return Err(ClaimsFoundingSbfErrorV5::Release.into());
    }
    let permit_data = accounts
        .permit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let permit = SeriesFoundingPermitV1::decode(&permit_data)
        .map_err(|_| ClaimsFoundingSbfErrorV5::Release)?;
    drop(permit_data);
    let intent = authenticate_permit_body(permit, request, request_digest)?;
    let permit_seeds = permit.seeds();
    let seed_slices = permit_seeds.as_slices();
    let (expected_permit, expected_bump) =
        Pubkey::find_program_address(&seed_slices, accounts.core_program.key);
    if expected_permit != *accounts.permit.key || expected_bump != intent.bump() {
        return Err(ClaimsFoundingSbfErrorV5::Release.into());
    }
    let projected_context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        intent.ticket_context().to_bytes().as_slice(),
    ])
    .to_bytes();
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core_digest = hash(&core_data).to_bytes();
    drop(core_data);
    authenticate_lock_receipt(
        intent,
        request,
        lock_receipt,
        projected_context,
        lock_receipt_digest,
        projected_receipt,
    )?;
    authenticate_projected_receipt(
        intent,
        request,
        projected_receipt,
        projected_context,
        projected_receipt_digest,
        core_digest,
    )
}

#[inline(never)]
fn authenticate_lock_receipt(
    intent: FoundingIntentV5,
    request: &ClaimsFoundingRequestV5,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    projected_context: [u8; 32],
    lock_receipt_digest: [u8; 32],
    projected_receipt: &ProjectedCustodyReceiptV1,
) -> Result<(), ProgramError> {
    if lock_receipt.market != request.market()
        || lock_receipt.release_set != request.release_set()
        || lock_receipt.context_digest != projected_context
        || lock_receipt.source_vault != request.funding_source()
        || lock_receipt.hoard_vault != request.hoard()
        || lock_receipt.rent_credit != request.rent_credit()
        || lock_receipt.request_digest != request.custody_request_digest()
        || lock_receipt_digest != request.custody_receipt_digest()
        || lock_receipt.amount != request.collateral_transferred()
        || lock_receipt.resulting_revision.checked_add(1)
            != Some(intent.projected_resulting_revision())
        || projected_receipt.resulting_revision != intent.projected_resulting_revision()
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_permit_body(
    permit: SeriesFoundingPermitV1,
    request: &ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
) -> Result<FoundingIntentV5, ProgramError> {
    let intent = permit.intent();
    let intent_bytes = intent
        .encode()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Release)?;
    let intent_digest = hash(&intent_bytes).to_bytes();
    permit
        .verify_for_intent_and_request(
            intent,
            Identity::new(intent_digest).map_err(|_| ClaimsFoundingSbfErrorV5::Release)?,
            Identity::new(request_digest).map_err(|_| ClaimsFoundingSbfErrorV5::Release)?,
        )
        .map_err(|_| ClaimsFoundingSbfErrorV5::Release)?;
    if intent_digest != request.founding_intent_digest()
        || intent.release_set().to_bytes() != request.release_set()
        || intent.market().to_bytes() != request.market()
        || intent.product_record().to_bytes() != request.product_record_digest()
        || intent.founder().to_bytes() != request.founder()
        || intent.projected_replay().to_bytes() != request.custody_replay()
        || intent.funding_source().to_bytes() != request.funding_source()
        || intent.hoard().to_bytes() != request.hoard()
        || intent.projected_request_digest().to_bytes() == request.custody_request_digest()
        || intent.projected_receipt_digest().to_bytes() == request.custody_receipt_digest()
        || intent.trading_program().to_bytes() != request.trading_program()
        || intent.claims_program().to_bytes() != request.claims_program()
        || intent.rent_credit().to_bytes() != request.rent_credit()
        || intent.generation() != request.generation()
        || intent.quantity() != request.quantity()
        || intent.basis_scale() != request.basis_scale()
        || intent.normal_replay_revision() != request.post_custody_revision()
        || request.pre_custody_revision().checked_add(1) != Some(intent.normal_replay_revision())
    {
        return Err(ClaimsFoundingSbfErrorV5::Release.into());
    }
    Ok(intent)
}

#[inline(never)]
fn authenticate_projected_receipt(
    intent: FoundingIntentV5,
    request: &ClaimsFoundingRequestV5,
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_context: [u8; 32],
    projected_receipt_digest: [u8; 32],
    core_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if !projected_receipt.realized
        || projected_receipt.aborted_open
        || projected_receipt.market != request.market()
        || projected_receipt.release_set != request.release_set()
        || projected_receipt.parent_capability_root != intent.parent_root().to_bytes()
        || projected_receipt.context_digest != projected_context
        || projected_receipt.hoard_vault != request.hoard()
        || projected_receipt.amount != request.collateral_transferred()
        || projected_receipt.request_digest != intent.projected_request_digest().to_bytes()
        || projected_receipt.market_state_digest != core_digest
        || projected_receipt.rent_credit != request.rent_credit()
        || projected_receipt.resulting_revision != intent.projected_resulting_revision()
        || projected_receipt_digest != intent.projected_receipt_digest().to_bytes()
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        ),
        (
            ExecutionRoleV1::Trading,
            accounts.trading_program,
            accounts.trading_programdata,
        ),
        (
            ExecutionRoleV1::Custody,
            accounts.custody_program,
            accounts.custody_programdata,
        ),
    ] {
        let receipt = authenticate_activated_role(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
            &request.release_set(),
        )
        .map_err(|_| ClaimsFoundingSbfErrorV5::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &request.release_set() {
            return Err(ClaimsFoundingSbfErrorV5::Release.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_custody_poststate(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    projected_receipt: &ProjectedCustodyReceiptV1,
    projected_receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if accounts.funding_source.key.to_bytes() != request.funding_source()
        || accounts.hoard.key.to_bytes() != request.hoard()
        || accounts.custody_replay.key.to_bytes() != request.custody_replay()
        || accounts.funding_source.owner != &system_program::ID
        || accounts.funding_source.lamports() != 0
        || !accounts.funding_source.data_is_empty()
        || accounts.funding_source.executable
        || TokenProgram::parse(accounts.hoard.owner.to_bytes()).is_err()
        || accounts.custody_replay.owner != accounts.custody_program.key
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    let hoard_data = accounts
        .hoard
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    if request.post_source_amount() != 0
        || request.pre_source_amount() != request.collateral_transferred()
        || hoard.amount != request.post_hoard_amount()
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    let replay_data = accounts
        .custody_replay
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let replay =
        CustodyReplayV1::decode(&replay_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::Custody)?;
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set()
        || replay.market != request.market()
        || replay.realm != core.identity.realm_id.to_bytes()
        || replay.context != projected_receipt.context_digest
        || replay.generation != request.generation()
        || replay.caller_program != request.trading_program()
        || replay.rent_refund != request.rent_credit()
        || replay.open_vault_count != 1
        || replay.next_revision != request.post_custody_revision()
        || replay.last_request_digest != projected_receipt.request_digest
        || replay.last_poststate_commitment != projected_receipt_digest
        || replay.last_request_digest != projected_receipt.request_digest
    {
        return Err(ClaimsFoundingSbfErrorV5::Custody.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_product_core(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
) -> Result<MarketViewV2, ProgramError> {
    if accounts.core_market.key.to_bytes() != request.market()
        || accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsFoundingSbfErrorV5::ProductBasis.into());
    }
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::ProductBasis)?;
    drop(core_data);
    let market = MarketViewV2 {
        claim_count: request.claim_count(),
        revision: request.post_aggregate_revision(),
        logical_market: request.market(),
        release_set: request.release_set(),
        registry_program: accounts.registry.key.to_bytes(),
        product_instance_id: request.product_instance_id(),
        basis_id: request.semantic_basis_id(),
        realm_id: core.identity.realm_id.to_bytes(),
        custody_context: request.market(),
        generation: request.generation(),
    };
    authenticate_runtime_product_basis_core_v3(
        accounts.registry,
        accounts.rent,
        accounts.core_market,
        accounts.core_program,
        ProductRuntimeFrameV3 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_record,
                staging: accounts.result_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
            linked_basis: FinalizedRecordFrameV2 {
                raw: accounts.basis_record,
                staging: accounts.basis_staging,
            },
        },
        market,
        request.product_record_digest(),
        request.linked_basis_record_digest(),
        CorePhase::Founding,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ProductBasis)?;
    let aggregate_seeds = ClaimsFoundingAggregateSeedsV5::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if Pubkey::find_program_address(&aggregate_seeds.as_slices(), program_id).0
        != *accounts.aggregate.key
        || accounts.aggregate.key.to_bytes() != request.aggregate()
    {
        return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
    }
    Ok(market)
}

#[inline(never)]
fn authenticate_rent_and_vacancy(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let rent =
        Rent::from_account_info(accounts.rent).map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    let aggregate_width = vector_width(
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let position_width = vector_width(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if rent.minimum_balance(aggregate_width) != request.aggregate_rent_principal()
        || rent.minimum_balance(position_width) != request.position_rent_principal()
        || rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            != request.admission_rent_principal()
        || accounts.aggregate.lamports() != request.observed_aggregate_lamports()
        || accounts.position.lamports() != request.observed_position_lamports()
        || accounts.admission.lamports() != request.observed_admission_lamports()
    {
        return Err(ClaimsFoundingSbfErrorV5::Rent.into());
    }
    for vacant in [accounts.aggregate, accounts.position, accounts.admission] {
        if vacant.owner != &system_program::ID
            || !vacant.data_is_empty()
            || vacant.is_signer
            || !vacant.is_writable
            || vacant.executable
        {
            return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
        }
    }
    let position_seeds =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0
        != *accounts.position.key
        || Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0
            != *accounts.admission.key
        || accounts.position.key.to_bytes() != request.position()
        || accounts.admission.key.to_bytes() != request.admission()
        || accounts.founder.key.to_bytes() != request.founder()
        || accounts.founder.executable
        || accounts.rent_credit.key.to_bytes() != request.rent_credit()
        || accounts.rent_program.key.to_bytes() != request.rent_program()
        || accounts.rent_credit.owner != accounts.rent_program.key
    {
        return Err(ClaimsFoundingSbfErrorV5::ClaimsState.into());
    }
    authenticate_rent_credit(accounts, market)?;
    Ok(())
}

#[inline(never)]
fn authenticate_rent_credit(
    accounts: FoundingAccounts<'_, '_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    let credit_data = accounts
        .rent_credit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Accounts)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    let seeds = credit.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation_seed = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market_seed.as_slice(),
            generation_seed.as_slice(),
            &bump,
        ],
        accounts.rent_program.key,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::Rent)?;
    if expected != *accounts.rent_credit.key
        || accounts.rent_credit.key.to_bytes() != core.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != market.logical_market
        || credit.release_set().to_bytes() != market.release_set
        || credit.generation() != core.identity.generation
        || core.identity.market_id.to_bytes() != market.logical_market
    {
        return Err(ClaimsFoundingSbfErrorV5::Rent.into());
    }
    Ok(())
}

#[inline(never)]
fn build_candidates_boxed(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    market: MarketViewV2,
    request_digest: [u8; 32],
) -> Result<Box<FoundingCandidates>, ProgramError> {
    let count = usize::try_from(request.claim_count())
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let quantities = vec![request.quantity(); count];
    let aggregate = encode_liability_basis_market_v2(
        LiabilityBasisMarketInputV2 {
            revision: request.post_aggregate_revision(),
            logical_market: market.logical_market,
            release_set: market.release_set,
            registry_program: market.registry_program,
            product_instance_id: market.product_instance_id,
            basis_id: market.basis_id,
            realm_id: market.realm_id,
            custody_context: market.custody_context,
            generation: market.generation,
        },
        &quantities,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let position = encode_liability_basis_position_v2(
        LiabilityBasisPositionInputV2 {
            revision: request.post_position_revision(),
            market_account: accounts.aggregate.key.to_bytes(),
            owner: request.founder(),
            basis_id: market.basis_id,
        },
        &quantities,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: request.release_set(),
        market: request.market(),
        position_owner: request.founder(),
        parent_request_digest: request.founding_intent_digest(),
        rent_credit: request.rent_credit(),
        rent_program: request.rent_program(),
        generation: request.generation(),
        expected_market_revision: request.post_aggregate_revision(),
        expected_position_revision: request.pre_position_revision(),
        observed_position_lamports: request.observed_position_lamports(),
        observed_admission_lamports: request.observed_admission_lamports(),
        position_rent_principal: request.position_rent_principal(),
        admission_rent_principal: request.admission_rent_principal(),
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission_request_bytes = admission_request
        .to_bytes()
        .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    let admission = ProtocolPositionAdmissionV2::new(
        admission_request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: request.product_record_digest(),
            semantic_basis_id: request.semantic_basis_id(),
            linked_basis_record_digest: request.linked_basis_record_digest(),
            request_digest: hash(&admission_request_bytes).to_bytes(),
            claims_program: program_id.to_bytes(),
            trading_program: accounts.trading_program.key.to_bytes(),
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: request.claim_count(),
        },
    )
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?
    .to_state_bytes()
    .map_err(|_| ClaimsFoundingSbfErrorV5::ClaimsState)?;
    if request_digest == [0; 32] {
        return Err(ClaimsFoundingSbfErrorV5::Receipt.into());
    }
    Ok(Box::new(FoundingCandidates {
        aggregate,
        position,
        admission,
    }))
}

#[inline(never)]
fn allocate_all(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV5,
    candidates: &FoundingCandidates,
) -> Result<(), ProgramError> {
    let aggregate = ClaimsFoundingAggregateSeedsV5::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.aggregate,
        accounts.system,
        candidates.aggregate.len(),
        &aggregate.as_slices(),
    )?;
    let position =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.position,
        accounts.system,
        candidates.position.len(),
        &position.as_slices(),
    )?;
    let admission =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    allocate_one(
        program_id,
        accounts.admission,
        accounts.system,
        candidates.admission.len(),
        &admission.as_slices(),
    )
}

#[inline(never)]
fn allocate_one<'info>(
    program_id: &Pubkey,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let bump = [Pubkey::find_program_address(seeds, program_id).1];
    let mut signer = Vec::with_capacity(seeds.len() + 1);
    signer.extend_from_slice(seeds);
    signer.push(&bump);
    let space = u64::try_from(width).map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    for instruction in [
        allocate(destination.key, space),
        assign(destination.key, program_id),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[destination.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| ClaimsFoundingSbfErrorV5::Allocation)?;
    }
    if destination.owner != program_id || destination.data_len() != width {
        return Err(ClaimsFoundingSbfErrorV5::Allocation.into());
    }
    Ok(())
}

#[inline(never)]
fn commit_candidates(
    accounts: FoundingAccounts<'_, '_>,
    candidates: &FoundingCandidates,
) -> Result<(), ProgramError> {
    let mut aggregate = accounts
        .aggregate
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    let mut position = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    let mut admission = accounts
        .admission
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV5::Commit)?;
    if aggregate.len() != candidates.aggregate.len()
        || position.len() != candidates.position.len()
        || admission.len() != candidates.admission.len()
        || aggregate.iter().any(|byte| *byte != 0)
        || position.iter().any(|byte| *byte != 0)
        || admission.iter().any(|byte| *byte != 0)
    {
        return Err(ClaimsFoundingSbfErrorV5::Commit.into());
    }
    aggregate.copy_from_slice(&candidates.aggregate);
    position.copy_from_slice(&candidates.position);
    admission.copy_from_slice(&candidates.admission);
    Ok(())
}

fn require_distinct(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, candidate) in accounts.iter().enumerate() {
        if accounts
            .get(..index)
            .ok_or(ClaimsFoundingSbfErrorV5::Accounts)?
            .iter()
            .any(|prior| prior.key == candidate.key)
        {
            return Err(ClaimsFoundingSbfErrorV5::Accounts.into());
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsFoundingSbfErrorV5::Accounts.into())
}

#[cfg(test)]
mod tests {
    use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestInputV5;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn identity(value: [u8; 32]) -> Identity {
        Identity::new(value).expect("nonzero identity")
    }

    fn fixture() -> (
        ClaimsFoundingRequestV5,
        SeriesFoundingPermitV1,
        ProjectedCustodyLockReceiptV1,
        ProjectedCustodyReceiptV1,
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ) {
        let lock_request_digest = id(19);
        let projected_request_digest = id(20);
        let core_digest = id(24);
        let ticket_context = id(22);
        let projected_context =
            hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ticket_context.as_slice()]).to_bytes();
        let lock_receipt = ProjectedCustodyLockReceiptV1 {
            market: id(2),
            release_set: id(1),
            context_digest: projected_context,
            source_vault: id(12),
            source_replay: id(25),
            hoard_vault: id(13),
            rent_credit: id(15),
            request_digest: lock_request_digest,
            amount: 77,
            source_vault_rent_lamports: 30,
            source_replay_rent_lamports: 31,
            resulting_revision: 4,
        };
        let lock_receipt_digest =
            hash(&lock_receipt.encode().expect("canonical lock receipt")).to_bytes();
        let projected_receipt = ProjectedCustodyReceiptV1 {
            realized: true,
            aborted_open: false,
            market: id(2),
            release_set: id(1),
            parent_capability_root: id(23),
            context_digest: projected_context,
            hoard_vault: id(13),
            amount: 77,
            request_digest: projected_request_digest,
            market_state_digest: core_digest,
            rent_credit: id(15),
            resulting_revision: 5,
        };
        let projected_receipt_digest = hash(
            &projected_receipt
                .encode()
                .expect("canonical projected receipt"),
        )
        .to_bytes();
        let intent = FoundingIntentV5::new(
            255,
            identity(id(1)),
            identity(id(2)),
            identity(id(3)),
            identity(id(21)),
            identity(id(7)),
            identity(ticket_context),
            identity(id(23)),
            identity(id(14)),
            identity(id(12)),
            identity(id(13)),
            identity(projected_request_digest),
            identity(projected_receipt_digest),
            identity(id(18)),
            identity(id(17)),
            identity(id(15)),
            21,
            7,
            11,
            500,
            5,
            1,
        )
        .expect("canonical intent");
        let intent_digest = hash(&intent.encode().expect("intent bytes")).to_bytes();
        let request = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(3),
            product_instance_id: id(4),
            linked_basis_record_digest: id(5),
            semantic_basis_id: id(6),
            founder: id(7),
            founding_intent_digest: intent_digest,
            aggregate: id(9),
            position: id(10),
            admission: id(11),
            funding_source: id(12),
            hoard: id(13),
            custody_replay: id(14),
            rent_credit: id(15),
            rent_program: id(16),
            claims_program: id(17),
            trading_program: id(18),
            custody_request_digest: lock_request_digest,
            custody_receipt_digest: lock_receipt_digest,
            generation: 21,
            claim_count: 5,
            quantity: 7,
            basis_scale: 11,
            pre_source_amount: 77,
            post_source_amount: 0,
            pre_hoard_amount: 23,
            post_hoard_amount: 100,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("canonical request");
        let request_digest = hash(&request.to_bytes()).to_bytes();
        let permit =
            SeriesFoundingPermitV1::new(intent, identity(intent_digest), identity(request_digest))
                .expect("canonical permit");
        (
            request,
            permit,
            lock_receipt,
            projected_receipt,
            lock_receipt_digest,
            projected_receipt_digest,
            core_digest,
        )
    }

    #[test]
    fn exact_instruction_and_permit_projection_join() {
        let (request, permit, lock, projected, lock_digest, projected_digest, core_digest) =
            fixture();
        let request_bytes = request.to_bytes();
        let request_digest = hash(&request_bytes).to_bytes();
        let intent = authenticate_permit_body(permit, &request, request_digest)
            .expect("permit binds request");
        let context = hashv(&[
            PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
            intent.ticket_context().to_bytes().as_slice(),
        ])
        .to_bytes();
        authenticate_projected_receipt(
            intent,
            &request,
            &projected,
            context,
            projected_digest,
            core_digest,
        )
        .expect("projected receipt binds intent");
        authenticate_lock_receipt(intent, &request, &lock, context, lock_digest, &projected)
            .expect("lock receipt binds intent");
        let mut instruction = Vec::from(request_bytes);
        instruction.extend_from_slice(&lock.encode().expect("lock bytes"));
        instruction.extend_from_slice(&projected.encode().expect("projected bytes"));
        assert!(decode_instruction(&instruction).is_ok());
        let short = instruction
            .get(..instruction.len().saturating_sub(1))
            .expect("short instruction slice");
        assert!(decode_instruction(short).is_err());
    }

    #[test]
    fn substituted_request_permit_and_projected_receipt_refuse() {
        let (request, permit, lock, projected, lock_digest, projected_digest, core_digest) =
            fixture();
        let request_digest = hash(&request.to_bytes()).to_bytes();
        let mut hostile_input = request.input();
        hostile_input.trading_program = id(99);
        let hostile_request =
            ClaimsFoundingRequestV5::new(hostile_input).expect("same-shape hostile request");
        assert!(authenticate_permit_body(permit, &hostile_request, request_digest).is_err());

        let intent = authenticate_permit_body(permit, &request, request_digest)
            .expect("permit binds request");
        let context = hashv(&[
            PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
            intent.ticket_context().to_bytes().as_slice(),
        ])
        .to_bytes();
        let mut hostile_projected = projected;
        hostile_projected.parent_capability_root = id(98);
        assert!(
            authenticate_projected_receipt(
                intent,
                &request,
                &hostile_projected,
                context,
                projected_digest,
                core_digest,
            )
            .is_err()
        );
        let mut hostile_lock = lock;
        hostile_lock.source_vault = id(97);
        assert!(
            authenticate_lock_receipt(
                intent,
                &request,
                &hostile_lock,
                context,
                lock_digest,
                &projected,
            )
            .is_err()
        );
        let mut obsolete = request.to_bytes();
        obsolete[..8]
            .copy_from_slice(&dclutch_claims_svm::founding_v4::CLAIMS_FOUNDING_REQUEST_MAGIC_V4);
        let mut instruction = Vec::from(obsolete);
        instruction.extend_from_slice(&lock.encode().expect("lock bytes"));
        instruction.extend_from_slice(&projected.encode().expect("projected bytes"));
        assert!(decode_instruction(&instruction).is_err());
    }
}
