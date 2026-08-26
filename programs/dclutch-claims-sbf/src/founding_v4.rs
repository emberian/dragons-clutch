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
    founding_v4::{
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V4, ClaimsFoundingAggregateSeedsV4,
        ClaimsFoundingReceiptV4, ClaimsFoundingRequestV4,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_custody_contract::{CallerRoleV1, CustodyReplayV1};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, STATE_BYTES};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::RentCreditV1;
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

use super::{affine_batch_v2::authenticate_runtime_product_basis_core_v2, reauthenticate};
use crate::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2, MarketViewV2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2, vector_width,
};

/// Exact FoundingV4 account count.
pub const CLAIMS_FOUNDING_ACCOUNT_COUNT_V4: usize = 31;

const AUTHORITY: usize = 0;
const AGGREGATE: usize = 1;
const POSITION: usize = 2;
const ADMISSION: usize = 3;
const FUNDING_SOURCE: usize = 4;
const HOARD: usize = 5;
const CUSTODY_REPLAY: usize = 6;
const BASIS_RECORD: usize = 7;
const BASIS_STAGING: usize = 8;
const PRODUCT_RECORD: usize = 9;
const PRODUCT_STAGING: usize = 10;
const RESULT_RECORD: usize = 11;
const RESULT_STAGING: usize = 12;
const PORTFOLIO_RECORD: usize = 13;
const PORTFOLIO_STAGING: usize = 14;
const RENT: usize = 15;
const SYSTEM: usize = 16;
const CORE_MARKET: usize = 17;
const CACHE: usize = 18;
const REGISTRY: usize = 19;
const CLAIMS_PROGRAM: usize = 20;
const CLAIMS_PROGRAMDATA: usize = 21;
const CORE_PROGRAM: usize = 22;
const CORE_PROGRAMDATA: usize = 23;
const TRADING_PROGRAM: usize = 24;
const TRADING_PROGRAMDATA: usize = 25;
const CUSTODY_PROGRAM: usize = 26;
const CUSTODY_PROGRAMDATA: usize = 27;
const FOUNDER: usize = 28;
const RENT_CREDIT: usize = 29;
const RENT_PROGRAM: usize = 30;

/// Stable FoundingV4 adapter refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsFoundingSbfErrorV4 {
    /// Instruction bytes did not decode as the sole FoundingV4 ABI.
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

impl From<ClaimsFoundingSbfErrorV4> for ProgramError {
    fn from(value: ClaimsFoundingSbfErrorV4) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct FoundingAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
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
        if accounts.len() != CLAIMS_FOUNDING_ACCOUNT_COUNT_V4 {
            return Err(ClaimsFoundingSbfErrorV4::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY)?,
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
    let request = decode_request(instruction_data)?;
    let accounts = FoundingAccounts::parse(account_infos)?;
    authenticate_privileges(program_id, accounts, &request)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_authority(accounts, &request, request_digest)?;
    authenticate_releases(accounts, &request)?;
    authenticate_custody_poststate(accounts, &request)?;
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
fn decode_request(instruction_data: &[u8]) -> Result<Box<ClaimsFoundingRequestV4>, ProgramError> {
    Ok(Box::new(
        ClaimsFoundingRequestV4::decode(instruction_data)
            .map_err(|_| ClaimsFoundingSbfErrorV4::Instruction)?,
    ))
}

#[inline(never)]
fn build_receipt(
    request: &ClaimsFoundingRequestV4,
    request_digest: [u8; 32],
    candidates: &FoundingCandidates,
) -> Result<
    Box<[u8; dclutch_claims_svm::founding_v4::CLAIMS_FOUNDING_RECEIPT_BYTES_V4]>,
    ProgramError,
> {
    let receipt = ClaimsFoundingReceiptV4::new(
        *request,
        request_digest,
        hash(&candidates.aggregate).to_bytes(),
        hash(&candidates.position).to_bytes(),
        hash(&candidates.admission).to_bytes(),
        hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V4,
            &candidates.aggregate,
            &candidates.position,
            &candidates.admission,
        ])
        .to_bytes(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::Receipt)?;
    Ok(Box::new(receipt.to_bytes()))
}

struct FoundingCandidates {
    aggregate: Vec<u8>,
    position: Vec<u8>,
    admission: [u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2],
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || !accounts.aggregate.is_writable
        || !accounts.position.is_writable
        || !accounts.admission.is_writable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.key.to_bytes() != request.claims_program()
        || accounts.core_program.key.to_bytes() != request.core_program()
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
        return Err(ClaimsFoundingSbfErrorV4::Accounts.into());
    }
    for readonly in [
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
            return Err(ClaimsFoundingSbfErrorV4::Accounts.into());
        }
    }
    require_distinct(&[
        accounts.authority,
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
    request: &ClaimsFoundingRequestV4,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set(),
        request.market(),
        ExecutionRoleV1::Core,
        request.parent_request_digest(),
        request_digest,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.core_program.key).0;
    if accounts.authority.key != &expected {
        return Err(ClaimsFoundingSbfErrorV4::Release.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
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
        let receipt = reauthenticate(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
        )
        .map_err(|_| ClaimsFoundingSbfErrorV4::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &request.release_set() {
            return Err(ClaimsFoundingSbfErrorV4::Release.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_custody_poststate(
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
) -> Result<(), ProgramError> {
    if accounts.funding_source.key.to_bytes() != request.funding_source()
        || accounts.hoard.key.to_bytes() != request.hoard()
        || accounts.custody_replay.key.to_bytes() != request.custody_replay()
        || accounts.funding_source.owner != accounts.hoard.owner
        || TokenProgram::parse(accounts.funding_source.owner.to_bytes()).is_err()
        || accounts.custody_replay.owner != accounts.custody_program.key
    {
        return Err(ClaimsFoundingSbfErrorV4::Custody.into());
    }
    let source_data = accounts
        .funding_source
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let hoard_data = accounts
        .hoard
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let source =
        TokenAccount::parse(&source_data).map_err(|_| ClaimsFoundingSbfErrorV4::Custody)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| ClaimsFoundingSbfErrorV4::Custody)?;
    if source.mint != hoard.mint
        || source.amount != request.post_source_amount()
        || hoard.amount != request.post_hoard_amount()
        || source.state != AccountState::Initialized
        || hoard.state != AccountState::Initialized
        || !source.delegate.is_none()
        || source.delegated_amount != 0
        || !source.native_reserve.is_none()
        || !source.close_authority.is_none()
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(ClaimsFoundingSbfErrorV4::Custody.into());
    }
    let replay_data = accounts
        .custody_replay
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let replay =
        CustodyReplayV1::decode(&replay_data).map_err(|_| ClaimsFoundingSbfErrorV4::Custody)?;
    if !matches!(
        replay.caller_role,
        CallerRoleV1::Core | CallerRoleV1::Trading
    ) || replay.release_set != request.release_set()
        || replay.market != request.market()
        || replay.generation != request.generation()
        || replay.next_revision != request.post_custody_revision()
        || replay.last_request_digest != request.custody_request_digest()
    {
        return Err(ClaimsFoundingSbfErrorV4::Custody.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_product_core(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
) -> Result<MarketViewV2, ProgramError> {
    if accounts.core_market.key.to_bytes() != request.market()
        || accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsFoundingSbfErrorV4::ProductBasis.into());
    }
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV4::ProductBasis)?;
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
                raw: accounts.result_record,
                staging: accounts.result_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
        },
        market,
        request.product_record_digest(),
        request.linked_basis_record_digest(),
        CorePhase::Founding,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::ProductBasis)?;
    let aggregate_seeds = ClaimsFoundingAggregateSeedsV4::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    if Pubkey::find_program_address(&aggregate_seeds.as_slices(), program_id).0
        != *accounts.aggregate.key
        || accounts.aggregate.key.to_bytes() != request.aggregate()
    {
        return Err(ClaimsFoundingSbfErrorV4::ClaimsState.into());
    }
    Ok(market)
}

#[inline(never)]
fn authenticate_rent_and_vacancy(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let rent =
        Rent::from_account_info(accounts.rent).map_err(|_| ClaimsFoundingSbfErrorV4::Rent)?;
    let aggregate_width = vector_width(
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    let position_width = vector_width(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        request.claim_count(),
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    if rent.minimum_balance(aggregate_width) != request.aggregate_rent_principal()
        || rent.minimum_balance(position_width) != request.position_rent_principal()
        || rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            != request.admission_rent_principal()
        || accounts.aggregate.lamports() != request.observed_aggregate_lamports()
        || accounts.position.lamports() != request.observed_position_lamports()
        || accounts.admission.lamports() != request.observed_admission_lamports()
    {
        return Err(ClaimsFoundingSbfErrorV4::Rent.into());
    }
    for vacant in [accounts.aggregate, accounts.position, accounts.admission] {
        if vacant.owner != &system_program::ID
            || !vacant.data_is_empty()
            || vacant.is_signer
            || !vacant.is_writable
            || vacant.executable
        {
            return Err(ClaimsFoundingSbfErrorV4::ClaimsState.into());
        }
    }
    let position_seeds =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
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
        return Err(ClaimsFoundingSbfErrorV4::ClaimsState.into());
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
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsFoundingSbfErrorV4::Rent)?;
    let credit_data = accounts
        .rent_credit
        .try_borrow_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Accounts)?;
    let credit = RentCreditV1::decode(&credit_data).map_err(|_| ClaimsFoundingSbfErrorV4::Rent)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.refund_authority().to_bytes().as_slice(),
            &bump,
        ],
        accounts.rent_program.key,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::Rent)?;
    if expected != *accounts.rent_credit.key
        || credit.refund_authority().to_bytes() != core.rent_beneficiary.to_bytes()
        || core.identity.market_id.to_bytes() != market.logical_market
    {
        return Err(ClaimsFoundingSbfErrorV4::Rent.into());
    }
    Ok(())
}

#[inline(never)]
fn build_candidates_boxed(
    program_id: &Pubkey,
    accounts: FoundingAccounts<'_, '_>,
    request: &ClaimsFoundingRequestV4,
    market: MarketViewV2,
    request_digest: [u8; 32],
) -> Result<Box<FoundingCandidates>, ProgramError> {
    let count = usize::try_from(request.claim_count())
        .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
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
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    let position = encode_liability_basis_position_v2(
        LiabilityBasisPositionInputV2 {
            revision: request.post_position_revision(),
            market_account: accounts.aggregate.key.to_bytes(),
            owner: request.founder(),
            basis_id: market.basis_id,
        },
        &quantities,
    )
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    let admission_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: request.release_set(),
        market: request.market(),
        position_owner: request.founder(),
        parent_request_digest: request.parent_request_digest(),
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
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    let admission_request_bytes = admission_request
        .to_bytes()
        .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
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
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?
    .to_state_bytes()
    .map_err(|_| ClaimsFoundingSbfErrorV4::ClaimsState)?;
    if request_digest == [0; 32] {
        return Err(ClaimsFoundingSbfErrorV4::Receipt.into());
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
    request: &ClaimsFoundingRequestV4,
    candidates: &FoundingCandidates,
) -> Result<(), ProgramError> {
    let aggregate = ClaimsFoundingAggregateSeedsV4::new(request.market())
        .map_err(|_| ClaimsFoundingSbfErrorV4::Allocation)?;
    allocate_one(
        program_id,
        accounts.aggregate,
        accounts.system,
        candidates.aggregate.len(),
        &aggregate.as_slices(),
    )?;
    let position =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV4::Allocation)?;
    allocate_one(
        program_id,
        accounts.position,
        accounts.system,
        candidates.position.len(),
        &position.as_slices(),
    )?;
    let admission =
        ProtocolPositionAdmissionSeedsV2::new(accounts.aggregate.key.to_bytes(), request.founder())
            .map_err(|_| ClaimsFoundingSbfErrorV4::Allocation)?;
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
    let space = u64::try_from(width).map_err(|_| ClaimsFoundingSbfErrorV4::Allocation)?;
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
        .map_err(|_| ClaimsFoundingSbfErrorV4::Allocation)?;
    }
    if destination.owner != program_id || destination.data_len() != width {
        return Err(ClaimsFoundingSbfErrorV4::Allocation.into());
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
        .map_err(|_| ClaimsFoundingSbfErrorV4::Commit)?;
    let mut position = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Commit)?;
    let mut admission = accounts
        .admission
        .try_borrow_mut_data()
        .map_err(|_| ClaimsFoundingSbfErrorV4::Commit)?;
    if aggregate.len() != candidates.aggregate.len()
        || position.len() != candidates.position.len()
        || admission.len() != candidates.admission.len()
        || aggregate.iter().any(|byte| *byte != 0)
        || position.iter().any(|byte| *byte != 0)
        || admission.iter().any(|byte| *byte != 0)
    {
        return Err(ClaimsFoundingSbfErrorV4::Commit.into());
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
            .ok_or(ClaimsFoundingSbfErrorV4::Accounts)?
            .iter()
            .any(|prior| prior.key == candidate.key)
        {
            return Err(ClaimsFoundingSbfErrorV4::Accounts.into());
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
        .ok_or_else(|| ClaimsFoundingSbfErrorV4::Accounts.into())
}
