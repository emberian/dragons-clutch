//! Claims-owned admission and reclamation for canonical LBV2 Positions.
//!
//! This adapter creates exactly the Position representation consumed by the
//! affine Claims batch. It never mutates claim balances: registration, fill,
//! cancellation, and expiry compose this lifecycle with the affine batch in
//! the outer Trading transaction. Closing is admitted only for an exact zero
//! vector and credits both prepaid accounts to one authenticated RentCredit.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::convert::TryFrom;

use dclutch_claims_svm::frame_spec_v1::ClaimsFrameSpecV1;
pub use dclutch_claims_svm::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_ADMISSION_MAGIC_V2,
    PROTOCOL_POSITION_ADMISSION_SEED_V2, PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2,
    PROTOCOL_POSITION_CLOSE_RECEIPT_MAGIC_V2, PROTOCOL_POSITION_RECEIPT_MAGIC_V2,
    PROTOCOL_POSITION_REQUEST_BYTES_V2, PROTOCOL_POSITION_REQUEST_MAGIC_V2,
    PROTOCOL_POSITION_STATE_SEED_V2, ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2,
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2,
    ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionCloseEvidenceV2,
    ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
};
use dclutch_market_core_codec::Phase as CorePhase;
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::RentCreditV1;
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
    LIABILITY_BASIS_MARKET_SEED_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    LiabilityBasisPositionInputV2, MarketViewV2, PositionViewV2,
    encode_liability_basis_position_v2, read_vector, vector_width,
};

/// Exact admission account count.
pub const PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2: usize =
    dclutch_claims_svm::frame_spec_v1::PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1 as usize;
/// Exact close account count.
pub const PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2: usize =
    dclutch_claims_svm::frame_spec_v1::PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1 as usize;

const AUTHORITY: usize = 0;
const MARKET: usize = 1;
const POSITION: usize = 2;
const ADMISSION: usize = 3;

const ADMIT_BASIS_RECORD: usize = 4;
const ADMIT_BASIS_STAGING: usize = 5;
const ADMIT_PRODUCT_RECORD: usize = 6;
const ADMIT_PRODUCT_STAGING: usize = 7;
const ADMIT_RESULT_RECORD: usize = 8;
const ADMIT_RESULT_STAGING: usize = 9;
const ADMIT_PORTFOLIO_RECORD: usize = 10;
const ADMIT_PORTFOLIO_STAGING: usize = 11;
const ADMIT_RENT: usize = 12;
const ADMIT_SYSTEM: usize = 13;
const ADMIT_CORE_MARKET: usize = 14;
const ADMIT_CACHE: usize = 15;
const ADMIT_REGISTRY: usize = 16;
const ADMIT_TRADING_PROGRAM: usize = 17;
const ADMIT_TRADING_PROGRAMDATA: usize = 18;
const ADMIT_CLAIMS_PROGRAM: usize = 19;
const ADMIT_CLAIMS_PROGRAMDATA: usize = 20;
const ADMIT_CORE_PROGRAM: usize = 21;
const ADMIT_CORE_PROGRAMDATA: usize = 22;
const ADMIT_OWNER_IDENTITY: usize = 23;
const ADMIT_RENT_CREDIT: usize = 24;
const ADMIT_RENT_PROGRAM: usize = 25;

const CLOSE_RENT: usize = 4;
const CLOSE_SYSTEM: usize = 5;
const CLOSE_CACHE: usize = 6;
const CLOSE_REGISTRY: usize = 7;
const CLOSE_TRADING_PROGRAM: usize = 8;
const CLOSE_TRADING_PROGRAMDATA: usize = 9;
const CLOSE_CLAIMS_PROGRAM: usize = 10;
const CLOSE_CLAIMS_PROGRAMDATA: usize = 11;
const CLOSE_OWNER_IDENTITY: usize = 12;
const CLOSE_RENT_CREDIT: usize = 13;
const CLOSE_RENT_PROGRAM: usize = 14;

const CLOSE_RESOURCE_DOMAIN_V2: &[u8] = b"dclutch/claims/protocol-position-close/v2";

/// Stable protocol Position adapter refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ProtocolPositionSbfErrorV2 {
    /// Instruction bytes did not decode as the canonical lifecycle ABI.
    Instruction = 140,
    /// Account count, privilege, executable, or alias facts refused.
    Accounts = 141,
    /// Current release selection or caller authority refused.
    Release = 142,
    /// Claims aggregate identity, revision, release, or generation refused.
    Market = 143,
    /// Product graph, linked basis, Core, or runtime width refused.
    ProductBasis = 144,
    /// Position/admission PDA vacancy, shape, owner, or balance refused.
    Position = 145,
    /// Prepaid rent or authenticated RentCredit facts refused.
    Rent = 146,
    /// System allocation or assignment refused.
    Allocation = 147,
    /// Persisted admission did not join the requested terminal close.
    Admission = 148,
    /// Complete candidate state or rent-credit reclamation did not commit.
    Commit = 149,
    /// Immediate receipt construction or poststate commitment refused.
    Receipt = 150,
}

impl From<ProtocolPositionSbfErrorV2> for ProgramError {
    fn from(value: ProtocolPositionSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct CommonAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
}

#[derive(Clone, Copy)]
struct AdmitAccounts<'accounts, 'info> {
    common: CommonAccounts<'accounts, 'info>,
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
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    owner_identity: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
}

#[derive(Clone, Copy)]
struct CloseAccounts<'accounts, 'info> {
    common: CommonAccounts<'accounts, 'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    owner_identity: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
}

/// Execute one exact admission or close.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = ProtocolPositionRequestV2::decode(instruction_data)
        .map_err(|_| ProtocolPositionSbfErrorV2::Instruction)?;
    authenticate_frame_spec(
        ClaimsFrameSpecV1::protocol_position(request.action),
        account_infos,
    )?;
    match request.action {
        ProtocolPositionActionV2::Admit => process_admit(
            program_id,
            AdmitAccounts::parse(account_infos)?,
            instruction_data,
            request,
        ),
        ProtocolPositionActionV2::Close => process_close(
            program_id,
            CloseAccounts::parse(account_infos)?,
            instruction_data,
            request,
        ),
    }
}

impl<'accounts, 'info> AdmitAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2 {
            return Err(ProtocolPositionSbfErrorV2::Accounts.into());
        }
        Ok(Self {
            common: common(accounts)?,
            basis_record: account(accounts, ADMIT_BASIS_RECORD)?,
            basis_staging: account(accounts, ADMIT_BASIS_STAGING)?,
            product_record: account(accounts, ADMIT_PRODUCT_RECORD)?,
            product_staging: account(accounts, ADMIT_PRODUCT_STAGING)?,
            result_record: account(accounts, ADMIT_RESULT_RECORD)?,
            result_staging: account(accounts, ADMIT_RESULT_STAGING)?,
            portfolio_record: account(accounts, ADMIT_PORTFOLIO_RECORD)?,
            portfolio_staging: account(accounts, ADMIT_PORTFOLIO_STAGING)?,
            rent: account(accounts, ADMIT_RENT)?,
            system: account(accounts, ADMIT_SYSTEM)?,
            core_market: account(accounts, ADMIT_CORE_MARKET)?,
            cache: account(accounts, ADMIT_CACHE)?,
            registry: account(accounts, ADMIT_REGISTRY)?,
            trading_program: account(accounts, ADMIT_TRADING_PROGRAM)?,
            trading_programdata: account(accounts, ADMIT_TRADING_PROGRAMDATA)?,
            claims_program: account(accounts, ADMIT_CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, ADMIT_CLAIMS_PROGRAMDATA)?,
            core_program: account(accounts, ADMIT_CORE_PROGRAM)?,
            core_programdata: account(accounts, ADMIT_CORE_PROGRAMDATA)?,
            owner_identity: account(accounts, ADMIT_OWNER_IDENTITY)?,
            rent_credit: account(accounts, ADMIT_RENT_CREDIT)?,
            rent_program: account(accounts, ADMIT_RENT_PROGRAM)?,
        })
    }
}

impl<'accounts, 'info> CloseAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2 {
            return Err(ProtocolPositionSbfErrorV2::Accounts.into());
        }
        Ok(Self {
            common: common(accounts)?,
            rent: account(accounts, CLOSE_RENT)?,
            system: account(accounts, CLOSE_SYSTEM)?,
            cache: account(accounts, CLOSE_CACHE)?,
            registry: account(accounts, CLOSE_REGISTRY)?,
            trading_program: account(accounts, CLOSE_TRADING_PROGRAM)?,
            trading_programdata: account(accounts, CLOSE_TRADING_PROGRAMDATA)?,
            claims_program: account(accounts, CLOSE_CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLOSE_CLAIMS_PROGRAMDATA)?,
            owner_identity: account(accounts, CLOSE_OWNER_IDENTITY)?,
            rent_credit: account(accounts, CLOSE_RENT_CREDIT)?,
            rent_program: account(accounts, CLOSE_RENT_PROGRAM)?,
        })
    }
}

fn common<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
) -> Result<CommonAccounts<'accounts, 'info>, ProgramError> {
    Ok(CommonAccounts {
        authority: account(accounts, AUTHORITY)?,
        market: account(accounts, MARKET)?,
        position: account(accounts, POSITION)?,
        admission: account(accounts, ADMISSION)?,
    })
}

#[inline(never)]
fn process_admit(
    program_id: &Pubkey,
    accounts: AdmitAccounts<'_, '_>,
    instruction_data: &[u8],
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    authenticate_admit_privileges(program_id, accounts)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_authority(
        accounts.common.authority,
        accounts.trading_program,
        request,
        request_digest,
    )?;
    authenticate_releases_admit(accounts, request)?;
    let (market, market_digest) = authenticate_market(
        program_id,
        accounts.common.market,
        accounts.registry,
        request,
    )?;
    authenticate_owner(
        accounts.owner_identity,
        accounts.trading_program,
        accounts.claims_program,
        request,
    )?;
    authenticate_rent_credit(accounts.rent_credit, accounts.rent_program, request)?;

    let product_digest = account_digest(accounts.product_record)?;
    let linked_digest = account_digest(accounts.basis_record)?;
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
        product_digest,
        linked_digest,
        CorePhase::Open,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;

    let rent =
        Rent::from_account_info(accounts.rent).map_err(|_| ProtocolPositionSbfErrorV2::Rent)?;
    let position_width = vector_width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, market.claim_count)
        .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    if rent.minimum_balance(position_width) != request.position_rent_principal
        || rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            != request.admission_rent_principal
    {
        return Err(ProtocolPositionSbfErrorV2::Rent.into());
    }
    authenticate_vacancy(program_id, accounts.common, request, position_width)?;
    let zero_balances = vec![
        0_u64;
        usize::try_from(market.claim_count)
            .map_err(|_| ProtocolPositionSbfErrorV2::Position)?
    ];
    let position_candidate = encode_liability_basis_position_v2(
        LiabilityBasisPositionInputV2 {
            revision: 0,
            market_account: accounts.common.market.key.to_bytes(),
            owner: request.position_owner,
            basis_id: market.basis_id,
        },
        &zero_balances,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let admission = ProtocolPositionAdmissionV2::new(
        request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: product_digest,
            semantic_basis_id: market.basis_id,
            linked_basis_record_digest: linked_digest,
            request_digest,
            claims_program: program_id.to_bytes(),
            trading_program: accounts.trading_program.key.to_bytes(),
            capability_descriptor: request.capability_descriptor,
            capability_outcome: request.capability_outcome,
            outcome_count: market.claim_count,
        },
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Receipt)?;
    let admission_candidate = admission
        .to_state_bytes()
        .map_err(|_| ProtocolPositionSbfErrorV2::Receipt)?;
    let receipt = admission
        .to_receipt_bytes()
        .map_err(|_| ProtocolPositionSbfErrorV2::Receipt)?;

    allocate_pair(program_id, accounts, request, position_width)?;
    commit_admission(accounts.common, &position_candidate, &admission_candidate)?;
    if account_digest(accounts.common.market)? != market_digest {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    set_return_data(&receipt);
    Ok(())
}

#[inline(never)]
fn process_close(
    program_id: &Pubkey,
    accounts: CloseAccounts<'_, '_>,
    instruction_data: &[u8],
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    authenticate_close_privileges(program_id, accounts)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_authority(
        accounts.common.authority,
        accounts.trading_program,
        request,
        request_digest,
    )?;
    authenticate_releases_close(accounts, request)?;
    let (market, market_digest) = authenticate_market(
        program_id,
        accounts.common.market,
        accounts.registry,
        request,
    )?;
    authenticate_owner(
        accounts.owner_identity,
        accounts.trading_program,
        accounts.claims_program,
        request,
    )?;
    let rent_credit_data =
        authenticate_rent_credit(accounts.rent_credit, accounts.rent_program, request)?;

    let admission_data = accounts
        .common
        .admission
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let admission_digest = hash(&admission_data).to_bytes();
    let admission = ProtocolPositionAdmissionV2::decode(&admission_data)
        .map_err(|_| ProtocolPositionSbfErrorV2::Admission)?;
    authenticate_admission(program_id, accounts, request, market, admission)?;
    drop(admission_data);

    let position_data = accounts
        .common
        .position
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let position =
        PositionViewV2::decode(&position_data).map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let balances = read_vector(
        &position_data,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        position.claim_count,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    if position.market_account != accounts.common.market.key.to_bytes()
        || position.owner != request.position_owner
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
        || position.revision != request.expected_position_revision
        || balances.iter().any(|value| *value != 0)
    {
        return Err(ProtocolPositionSbfErrorV2::Position.into());
    }
    drop(position_data);

    let rent =
        Rent::from_account_info(accounts.rent).map_err(|_| ProtocolPositionSbfErrorV2::Rent)?;
    if rent.minimum_balance(accounts.common.position.data_len()) != request.position_rent_principal
        || rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            != request.admission_rent_principal
        || accounts.common.position.lamports() != request.observed_position_lamports
        || accounts.common.admission.lamports() != request.observed_admission_lamports
    {
        return Err(ProtocolPositionSbfErrorV2::Rent.into());
    }
    let rent_before = accounts.rent_credit.lamports();
    let total = request
        .observed_position_lamports
        .checked_add(request.observed_admission_lamports)
        .ok_or(ProtocolPositionSbfErrorV2::Rent)?;
    let rent_after = rent_before
        .checked_add(total)
        .ok_or(ProtocolPositionSbfErrorV2::Rent)?;
    let rent_after_bytes = rent_after.to_le_bytes();
    let rent_data_digest = hash(&rent_credit_data).to_bytes();
    let post_resource_digest = hashv(&[
        CLOSE_RESOURCE_DOMAIN_V2,
        accounts.common.position.key.as_ref(),
        accounts.common.admission.key.as_ref(),
        accounts.rent_credit.key.as_ref(),
        &rent_after_bytes,
        &rent_data_digest,
    ])
    .to_bytes();
    let receipt = ProtocolPositionCloseReceiptV2::new(
        request,
        ProtocolPositionCloseEvidenceV2 {
            request_digest,
            admission_digest,
            claims_program: program_id.to_bytes(),
            post_resource_digest,
            rent_credit_before: rent_before,
            rent_credit_after: rent_after,
        },
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Receipt)?
    .to_bytes()
    .map_err(|_| ProtocolPositionSbfErrorV2::Receipt)?;

    close_pair(accounts, rent_after)?;
    if account_digest(accounts.common.market)? != market_digest {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    set_return_data(&receipt);
    Ok(())
}

fn authenticate_admit_privileges(
    program_id: &Pubkey,
    accounts: AdmitAccounts<'_, '_>,
) -> Result<(), ProgramError> {
    authenticate_common(program_id, accounts.common)?;
    if accounts.rent.key != &sysvar::rent::ID
        || accounts.system.key != &system_program::ID
        || accounts.claims_program.key != program_id
    {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    require_distinct(&[
        accounts.common.authority,
        accounts.common.market,
        accounts.common.position,
        accounts.common.admission,
        accounts.owner_identity,
        accounts.rent_credit,
        accounts.rent_program,
        accounts.trading_program,
        accounts.claims_program,
        accounts.core_program,
    ])
}

fn authenticate_close_privileges(
    program_id: &Pubkey,
    accounts: CloseAccounts<'_, '_>,
) -> Result<(), ProgramError> {
    authenticate_common(program_id, accounts.common)?;
    if accounts.rent.key != &sysvar::rent::ID
        || accounts.system.key != &system_program::ID
        || accounts.claims_program.key != program_id
    {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    require_distinct(&[
        accounts.common.authority,
        accounts.common.market,
        accounts.common.position,
        accounts.common.admission,
        accounts.owner_identity,
        accounts.rent_credit,
        accounts.rent_program,
        accounts.trading_program,
        accounts.claims_program,
    ])
}

fn authenticate_common(
    program_id: &Pubkey,
    accounts: CommonAccounts<'_, '_>,
) -> Result<(), ProgramError> {
    if accounts.position.key == accounts.admission.key
        || accounts.market.key == accounts.position.key
        || accounts.market.key == accounts.admission.key
        || accounts.market.owner != program_id
    {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    Ok(())
}

fn authenticate_frame_spec(
    spec: ClaimsFrameSpecV1,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let count = usize::from(
        spec.account_count()
            .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?,
    );
    if accounts.len() != count {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    for (index, observed) in accounts.iter().enumerate() {
        let coordinate = u16::try_from(index).map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
        let expected = spec
            .account(coordinate)
            .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?
            .privileges();
        if observed.is_signer != expected.signer()
            || observed.is_writable != expected.writable()
            || observed.executable != expected.executable()
        {
            return Err(ProtocolPositionSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

fn authenticate_authority(
    authority: &AccountInfo<'_>,
    trading_program: &AccountInfo<'_>,
    request: ProtocolPositionRequestV2,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.position_owner,
        request_digest,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), trading_program.key).0;
    if authority.key != &expected {
        return Err(ProtocolPositionSbfErrorV2::Release.into());
    }
    Ok(())
}

fn authenticate_releases_admit(
    accounts: AdmitAccounts<'_, '_>,
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            accounts.trading_program,
            accounts.trading_programdata,
        ),
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
    ] {
        let receipt = reauthenticate(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
        )
        .map_err(|_| ProtocolPositionSbfErrorV2::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &request.release_set {
            return Err(ProtocolPositionSbfErrorV2::Release.into());
        }
    }
    Ok(())
}

fn authenticate_releases_close(
    accounts: CloseAccounts<'_, '_>,
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            accounts.trading_program,
            accounts.trading_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
    ] {
        let receipt = reauthenticate(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
        )
        .map_err(|_| ProtocolPositionSbfErrorV2::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &request.release_set {
            return Err(ProtocolPositionSbfErrorV2::Release.into());
        }
    }
    Ok(())
}

fn authenticate_market(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    registry: &AccountInfo<'_>,
    request: ProtocolPositionRequestV2,
) -> Result<(MarketViewV2, [u8; 32]), ProgramError> {
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, request.market.as_slice()],
        program_id,
    )
    .0;
    let data = market_account
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let digest = hash(&data).to_bytes();
    let market = MarketViewV2::decode(&data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?;
    if market_account.key != &expected
        || market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != registry.key.to_bytes()
        || market.generation != request.generation
        || market.revision != request.expected_market_revision
    {
        return Err(ProtocolPositionSbfErrorV2::Market.into());
    }
    Ok((market, digest))
}

fn authenticate_owner(
    owner: &AccountInfo<'_>,
    trading_program: &AccountInfo<'_>,
    claims_program: &AccountInfo<'_>,
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    if owner.key.to_bytes() != request.position_owner
        || owner.is_signer
        || owner.is_writable
        || owner.executable
    {
        return Err(ProtocolPositionSbfErrorV2::Position.into());
    }
    match request.owner_kind {
        ProtocolPositionOwnerKindV2::TradingRecord => {
            if owner.owner != trading_program.key || owner.data_is_empty() {
                return Err(ProtocolPositionSbfErrorV2::Position.into());
            }
        }
        ProtocolPositionOwnerKindV2::User => {}
        ProtocolPositionOwnerKindV2::ClaimsCapability => {
            let seeds = ProtocolPositionClaimsCapabilitySeedsV2::new(
                request.capability_descriptor,
                request.capability_outcome,
            )
            .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
            let expected = Pubkey::find_program_address(&seeds.as_slices(), claims_program.key).0;
            if expected != *owner.key {
                return Err(ProtocolPositionSbfErrorV2::Position.into());
            }
        }
    }
    Ok(())
}

fn authenticate_rent_credit(
    rent_credit: &AccountInfo<'_>,
    rent_program: &AccountInfo<'_>,
    request: ProtocolPositionRequestV2,
) -> Result<Vec<u8>, ProgramError> {
    if rent_credit.key.to_bytes() != request.rent_credit
        || rent_program.key.to_bytes() != request.rent_program
        || rent_credit.owner != rent_program.key
        || rent_credit.executable
        || !rent_program.executable
    {
        return Err(ProtocolPositionSbfErrorV2::Rent.into());
    }
    let data = rent_credit
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| ProtocolPositionSbfErrorV2::Rent)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let authority = seeds.refund_authority().to_bytes();
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), authority.as_slice(), &bump],
        rent_program.key,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Rent)?;
    if expected != *rent_credit.key {
        return Err(ProtocolPositionSbfErrorV2::Rent.into());
    }
    Ok(data.to_vec())
}

fn authenticate_vacancy(
    program_id: &Pubkey,
    accounts: CommonAccounts<'_, '_>,
    request: ProtocolPositionRequestV2,
    position_width: usize,
) -> Result<(), ProgramError> {
    let position_seeds =
        ProtocolPositionSeedsV2::new(accounts.market.key.to_bytes(), request.position_owner)
            .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        accounts.market.key.to_bytes(),
        request.position_owner,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0;
    if accounts.position.key != &expected_position
        || accounts.admission.key != &expected_admission
        || accounts.position.owner != &system_program::ID
        || accounts.admission.owner != &system_program::ID
        || !accounts.position.data_is_empty()
        || !accounts.admission.data_is_empty()
        || accounts.position.lamports() != request.observed_position_lamports
        || accounts.admission.lamports() != request.observed_admission_lamports
        || position_width == 0
    {
        return Err(ProtocolPositionSbfErrorV2::Position.into());
    }
    Ok(())
}

fn authenticate_admission(
    program_id: &Pubkey,
    accounts: CloseAccounts<'_, '_>,
    request: ProtocolPositionRequestV2,
    market: MarketViewV2,
    admission: ProtocolPositionAdmissionV2,
) -> Result<(), ProgramError> {
    let position_seeds = ProtocolPositionSeedsV2::new(
        accounts.common.market.key.to_bytes(),
        request.position_owner,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        accounts.common.market.key.to_bytes(),
        request.position_owner,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Position)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0;
    if accounts.common.position.key != &expected_position
        || accounts.common.admission.key != &expected_admission
        || accounts.common.position.owner != program_id
        || accounts.common.admission.owner != program_id
        || admission.owner_kind() != request.owner_kind
        || admission.release_set() != request.release_set
        || admission.market() != request.market
        || admission.position_owner() != request.position_owner
        || admission.rent_credit() != request.rent_credit
        || admission.rent_program() != request.rent_program
        || admission.claims_program() != program_id.to_bytes()
        || admission.trading_program() != accounts.trading_program.key.to_bytes()
        || admission.semantic_basis_id() != market.basis_id
        || admission.outcome_count() != market.claim_count
        || admission.generation() != request.generation
        || admission.position_rent_principal() != request.position_rent_principal
        || admission.admission_rent_principal() != request.admission_rent_principal
        || admission.capability_descriptor() != request.capability_descriptor
        || admission.capability_outcome() != request.capability_outcome
    {
        return Err(ProtocolPositionSbfErrorV2::Admission.into());
    }
    Ok(())
}

fn allocate_pair(
    program_id: &Pubkey,
    accounts: AdmitAccounts<'_, '_>,
    request: ProtocolPositionRequestV2,
    position_width: usize,
) -> Result<(), ProgramError> {
    let position_seeds = ProtocolPositionSeedsV2::new(
        accounts.common.market.key.to_bytes(),
        request.position_owner,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    let position_bump = [Pubkey::find_program_address(&position_seeds.as_slices(), program_id).1];
    let [position_domain, position_market, position_owner] = position_seeds.as_slices();
    allocate_and_assign(
        program_id,
        accounts.common.position,
        accounts.system,
        position_width,
        &[
            position_domain,
            position_market,
            position_owner,
            &position_bump,
        ],
    )?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        accounts.common.market.key.to_bytes(),
        request.position_owner,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    let admission_bump = [Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).1];
    let [admission_domain, admission_market, admission_owner] = admission_seeds.as_slices();
    allocate_and_assign(
        program_id,
        accounts.common.admission,
        accounts.system,
        PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        &[
            admission_domain,
            admission_market,
            admission_owner,
            &admission_bump,
        ],
    )
}

fn allocate_and_assign<'info>(
    program_id: &Pubkey,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let space = u64::try_from(width).map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
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
            &[seeds],
        )
        .map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    }
    if destination.owner != program_id || destination.data_len() != width {
        return Err(ProtocolPositionSbfErrorV2::Allocation.into());
    }
    Ok(())
}

fn commit_admission(
    accounts: CommonAccounts<'_, '_>,
    position_candidate: &[u8],
    admission_candidate: &[u8],
) -> Result<(), ProgramError> {
    let mut position = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
    let mut admission = accounts
        .admission
        .try_borrow_mut_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
    if position.len() != position_candidate.len()
        || admission.len() != admission_candidate.len()
        || position.iter().any(|byte| *byte != 0)
        || admission.iter().any(|byte| *byte != 0)
    {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    position.copy_from_slice(position_candidate);
    admission.copy_from_slice(admission_candidate);
    Ok(())
}

fn close_pair(accounts: CloseAccounts<'_, '_>, rent_after: u64) -> Result<(), ProgramError> {
    {
        let mut position = accounts
            .common
            .position
            .try_borrow_mut_data()
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        let mut admission = accounts
            .common
            .admission
            .try_borrow_mut_data()
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        position.fill(0);
        admission.fill(0);
    }
    {
        let mut position_lamports = accounts
            .common
            .position
            .try_borrow_mut_lamports()
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        let mut admission_lamports = accounts
            .common
            .admission
            .try_borrow_mut_lamports()
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        let mut rent_lamports = accounts
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        **position_lamports = 0;
        **admission_lamports = 0;
        **rent_lamports = rent_after;
    }
    for closed in [accounts.common.position, accounts.common.admission] {
        closed
            .resize(0)
            .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
        closed.assign(&system_program::ID);
    }
    if accounts.common.position.lamports() != 0
        || accounts.common.admission.lamports() != 0
        || !accounts.common.position.data_is_empty()
        || !accounts.common.admission.data_is_empty()
        || accounts.common.position.owner != &system_program::ID
        || accounts.common.admission.owner != &system_program::ID
        || accounts.rent_credit.lamports() != rent_after
    {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .ok_or(ProtocolPositionSbfErrorV2::Accounts)?
            .iter()
            .any(|other| other.key == account.key)
        {
            return Err(ProtocolPositionSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

fn account_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    Ok(hash(&data).to_bytes())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ProtocolPositionSbfErrorV2::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_frames_are_action_specific_and_minimal() {
        assert_eq!(PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2, 26);
        assert_eq!(PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2, 15);
        assert_ne!(PROTOCOL_POSITION_REQUEST_MAGIC_V2, *b"DCLPPR01");
    }

    #[test]
    fn position_and_admission_use_one_market_owner_coordinate() {
        let program = Pubkey::new_from_array([7; 32]);
        let market = Pubkey::new_from_array([8; 32]);
        let owner = [9; 32];
        let position_seeds =
            ProtocolPositionSeedsV2::new(market.to_bytes(), owner).expect("position seeds");
        let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(market.to_bytes(), owner)
            .expect("admission seeds");
        let position = Pubkey::find_program_address(&position_seeds.as_slices(), &program).0;
        let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &program).0;
        assert_ne!(position, admission);
    }
}
