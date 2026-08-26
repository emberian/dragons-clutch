//! Claims-owned physical lifecycle for Rational Representation V2 resources.
//!
//! This adapter persists no parallel lifecycle or economic ledger. The
//! finalized descriptor owns immutable resource coordinates, Token-2022 owns
//! Mint supply and token-account balances, Claims owns the canonical LBV2
//! aggregate/Position state, and RentCredit owns reclaimed lamports.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::convert::TryFrom;

use dclutch_claims_svm::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2,
    PROTOCOL_POSITION_REQUEST_BYTES_V2, ProtocolPositionActionV2, ProtocolPositionAdmissionSeedsV2,
    ProtocolPositionAdmissionV2, ProtocolPositionClaimsCapabilitySeedsV2,
    ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase as CorePhase};
use dclutch_rational_representation_v2_contract::{
    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2,
    RATIONAL_STRUCTURED_CUSTODY_SEED_V2, RationalReceiptMintSeedsV2,
};
use dclutch_rational_representation_v2_kernel::{
    DescriptorAdmissionV2, REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    RepresentationDescriptorV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2,
    LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LifecycleActionV2, LifecycleCompletionEvidenceV2,
    LifecycleCoordinateV2, LifecycleRequestV2, finalize, prepare,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{
    ACCOUNT_BYTES, AccountState, TOKEN_2022_CLOSEABLE_MINT_BYTES_V2, TOKEN_2022_PROGRAM_ID,
    Token2022CloseableMintProfileV2, TokenAccount,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::Instruction,
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};
use spl_token_2022_interface::instruction as token_instruction;

use super::reauthenticate;
use crate::{
    liability_basis_v2::{LIABILITY_BASIS_MARKET_SEED_V2, MarketViewV2},
    protocol_position_v2,
    rational_representation_v2::authenticate_finalized_rational_record,
};

/// Exact account count shared by receipt-wide lifecycle actions.
pub const RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2: usize = LIFECYCLE_COMMON_ACCOUNT_COUNT_V2;
/// Exact account count for one coordinate activation or retirement.
pub const RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2: usize =
    LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2;
/// Exact account count added per proven-vacant coordinate on receipt retirement.
pub const RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2: usize = LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2;

const CALLER_AUTHORITY: usize = 0;
const TRADING_PROGRAM: usize = 1;
const TRADING_PROGRAMDATA: usize = 2;
const CLAIMS_PROGRAM: usize = 3;
const CLAIMS_PROGRAMDATA: usize = 4;
const REGISTRY_PROGRAM: usize = 5;
const ACTIVATION_CACHE: usize = 6;
const RENT_SYSVAR: usize = 7;
const SYSTEM_PROGRAM: usize = 8;
const DESCRIPTOR_RAW: usize = 9;
const DESCRIPTOR_STAGING: usize = 10;
const REPRESENTATION_AUTHORITY: usize = 11;
const RECEIPT_MINT: usize = 12;
const TOKEN_PROGRAM: usize = 13;
const RENT_CREDIT: usize = 14;
const RENT_PROGRAM: usize = 15;
const CLAIMS_AGGREGATE: usize = 16;
const CORE_MARKET: usize = 17;
const CORE_PROGRAM: usize = 18;
const CORE_PROGRAMDATA: usize = 19;

const CHILD_AUTHORITY: usize = 20;
const COORDINATE_POSITION: usize = 21;
const COORDINATE_ADMISSION: usize = 22;
const COORDINATE_SHARD_MINT: usize = 23;
const COORDINATE_STRUCTURED_CUSTODY: usize = 24;
const COORDINATE_OWNER: usize = 25;
const BASIS_RECORD: usize = 26;
const BASIS_STAGING: usize = 27;
const PRODUCT_RECORD: usize = 28;
const PRODUCT_STAGING: usize = 29;
const RESULT_RECORD: usize = 30;
const RESULT_STAGING: usize = 31;
const PORTFOLIO_RECORD: usize = 32;
const PORTFOLIO_STAGING: usize = 33;

const VACANCY_SHARD_MINT: usize = 0;
const VACANCY_STRUCTURED_CUSTODY: usize = 1;
const VACANCY_POSITION: usize = 2;
const VACANCY_ADMISSION: usize = 3;

const POST_RESOURCE_DOMAIN_V2: &[u8] = b"dclutch/rational-lifecycle/post/v2";

/// Stable physical lifecycle refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RationalLifecycleSbfErrorV2 {
    /// Instruction bytes or runtime width refused.
    Instruction = 210,
    /// Account frame, privilege, or alias refused.
    Accounts = 211,
    /// Current release selection or Trading caller refused.
    Release = 212,
    /// Finalized descriptor or a derived resource identity refused.
    Descriptor = 213,
    /// Core Market or canonical Claims aggregate refused.
    Market = 214,
    /// Prepaid or reclaimed native rent accounting refused.
    Rent = 215,
    /// Token-2022 resource state or effect refused.
    Token = 216,
    /// Canonical protocol Position lifecycle refused.
    Position = 217,
    /// Final resource observation or typed receipt refused.
    Receipt = 218,
}

impl From<RationalLifecycleSbfErrorV2> for ProgramError {
    fn from(value: RationalLifecycleSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct CommonAccounts<'accounts, 'info> {
    caller_authority: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    descriptor_raw: &'accounts AccountInfo<'info>,
    descriptor_staging: &'accounts AccountInfo<'info>,
    representation_authority: &'accounts AccountInfo<'info>,
    receipt_mint: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
}

#[derive(Clone, Copy)]
struct CoordinateAccounts<'accounts, 'info> {
    common: CommonAccounts<'accounts, 'info>,
    child_authority: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    shard_mint: &'accounts AccountInfo<'info>,
    structured_custody: &'accounts AccountInfo<'info>,
    owner: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_record: &'accounts AccountInfo<'info>,
    result_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
}

/// Execute one exact granular Rational resource lifecycle action.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = LifecycleRequestV2::decode(instruction_data)
        .map_err(|_| RationalLifecycleSbfErrorV2::Instruction)?;
    let common = CommonAccounts::parse(account_infos)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_common(program_id, account_infos, common, request, request_digest)?;

    let descriptor_data = common
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let rent =
        Rent::from_account_info(common.rent).map_err(|_| RationalLifecycleSbfErrorV2::Rent)?;
    authenticate_finalized_rational_record(
        common.registry.key,
        &rent,
        common.descriptor_raw,
        common.descriptor_staging,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        request.header().descriptor_id,
        &descriptor_data,
    )?;
    let descriptor = RepresentationDescriptorV2::decode(
        &descriptor_data,
        DescriptorAdmissionV2 {
            selected_descriptor_id: request.header().descriptor_id,
            finalized_descriptor_id: request.header().descriptor_id,
            recomputed_descriptor_digest: hash(&descriptor_data).to_bytes(),
            finalized_descriptor_digest: request.header().descriptor_id,
            record_authenticated: true,
            derived_representation_authority: request.header().representation_authority,
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
    authenticate_descriptor_resources(program_id, common, request, descriptor)?;
    let graph_digest = descriptor.graph_digest();
    let prepared =
        prepare(request, descriptor).map_err(|_| RationalLifecycleSbfErrorV2::Instruction)?;
    drop(descriptor_data);

    let position_receipt_digest = match prepared.action() {
        LifecycleActionV2::ActivateReceipt => {
            activate_receipt(program_id, common, request, graph_digest, &rent)?;
            [0; 32]
        }
        LifecycleActionV2::ActivateCoordinate => {
            activate_coordinate(program_id, account_infos, request, request_digest, &rent)?
        }
        LifecycleActionV2::RetireCoordinate => {
            retire_coordinate(program_id, account_infos, request, request_digest)?
        }
        LifecycleActionV2::RetireReceipt => {
            authenticate_complete_vacancy(program_id, account_infos, common, request)?;
            retire_receipt(program_id, common, request)?;
            [0; 32]
        }
    };

    let post_resource_digest = post_resource_digest(account_infos, common, request)?;
    let receipt = finalize(
        prepared,
        LifecycleCompletionEvidenceV2 {
            request_digest,
            descriptor_digest: request.header().descriptor_id,
            post_resource_digest,
            position_lifecycle_receipt_digest: position_receipt_digest,
            rent_credit_before: request.header().rent_credit_before,
            rent_credit_after: common.rent_credit.lamports(),
            caller_authenticated: true,
            descriptor_and_resources_authenticated: true,
            physical_effects_committed: true,
        },
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Receipt)?;
    set_return_data(
        &receipt
            .to_bytes()
            .map_err(|_| RationalLifecycleSbfErrorV2::Receipt)?,
    );
    Ok(())
}

impl<'accounts, 'info> CommonAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() < RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2 {
            return Err(RationalLifecycleSbfErrorV2::Accounts.into());
        }
        Ok(Self {
            caller_authority: account(accounts, CALLER_AUTHORITY)?,
            trading_program: account(accounts, TRADING_PROGRAM)?,
            trading_programdata: account(accounts, TRADING_PROGRAMDATA)?,
            claims_program: account(accounts, CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA)?,
            registry: account(accounts, REGISTRY_PROGRAM)?,
            cache: account(accounts, ACTIVATION_CACHE)?,
            rent: account(accounts, RENT_SYSVAR)?,
            system: account(accounts, SYSTEM_PROGRAM)?,
            descriptor_raw: account(accounts, DESCRIPTOR_RAW)?,
            descriptor_staging: account(accounts, DESCRIPTOR_STAGING)?,
            representation_authority: account(accounts, REPRESENTATION_AUTHORITY)?,
            receipt_mint: account(accounts, RECEIPT_MINT)?,
            token_program: account(accounts, TOKEN_PROGRAM)?,
            rent_credit: account(accounts, RENT_CREDIT)?,
            rent_program: account(accounts, RENT_PROGRAM)?,
            aggregate: account(accounts, CLAIMS_AGGREGATE)?,
            core_market: account(accounts, CORE_MARKET)?,
            core_program: account(accounts, CORE_PROGRAM)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA)?,
        })
    }
}

impl<'accounts, 'info> CoordinateAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2 {
            return Err(RationalLifecycleSbfErrorV2::Accounts.into());
        }
        Ok(Self {
            common: CommonAccounts::parse(accounts)?,
            child_authority: account(accounts, CHILD_AUTHORITY)?,
            position: account(accounts, COORDINATE_POSITION)?,
            admission: account(accounts, COORDINATE_ADMISSION)?,
            shard_mint: account(accounts, COORDINATE_SHARD_MINT)?,
            structured_custody: account(accounts, COORDINATE_STRUCTURED_CUSTODY)?,
            owner: account(accounts, COORDINATE_OWNER)?,
            basis_record: account(accounts, BASIS_RECORD)?,
            basis_staging: account(accounts, BASIS_STAGING)?,
            product_record: account(accounts, PRODUCT_RECORD)?,
            product_staging: account(accounts, PRODUCT_STAGING)?,
            result_record: account(accounts, RESULT_RECORD)?,
            result_staging: account(accounts, RESULT_STAGING)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING)?,
        })
    }
}

#[inline(never)]
fn authenticate_common(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let header = request.header();
    let expected_len = match header.action {
        LifecycleActionV2::ActivateReceipt => RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2
        }
        LifecycleActionV2::RetireReceipt => usize::try_from(header.coordinate_count)
            .ok()
            .and_then(|count| count.checked_mul(RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2))
            .and_then(|tail| tail.checked_add(RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2))
            .ok_or(RationalLifecycleSbfErrorV2::Accounts)?,
    };
    if accounts.len() != expected_len {
        return Err(RationalLifecycleSbfErrorV2::Accounts.into());
    }
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        request_digest,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Release)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), common.trading_program.key).0;
    let expected_authority = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            header.descriptor_id.as_slice(),
        ],
        program_id,
    )
    .0;
    if !common.caller_authority.is_signer
        || common.caller_authority.is_writable
        || common.caller_authority.key != &expected_caller
        || common.caller_authority.executable
        || !common.trading_program.executable
        || common.trading_program.is_writable
        || common.trading_program.is_signer
        || common.trading_programdata.is_writable
        || common.trading_programdata.is_signer
        || common.claims_program.key != program_id
        || !common.claims_program.executable
        || common.claims_program.is_writable
        || common.claims_program.is_signer
        || common.claims_programdata.is_writable
        || common.claims_programdata.is_signer
        || !common.registry.executable
        || common.registry.is_writable
        || common.registry.is_signer
        || common.cache.is_writable
        || common.cache.is_signer
        || common.rent.key != &sysvar::rent::ID
        || common.rent.is_writable
        || common.rent.is_signer
        || common.system.key != &system_program::ID
        || !common.system.executable
        || common.system.is_writable
        || common.system.is_signer
        || common.descriptor_raw.is_writable
        || common.descriptor_raw.is_signer
        || common.descriptor_raw.executable
        || common.descriptor_staging.is_writable
        || common.descriptor_staging.is_signer
        || common.descriptor_staging.executable
        || common.representation_authority.key != &expected_authority
        || common.representation_authority.is_writable
        || common.representation_authority.is_signer
        || common.representation_authority.executable
        || common.receipt_mint.key.to_bytes() != header.receipt_mint
        || common.receipt_mint.is_writable
            != matches!(
                header.action,
                LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
            )
        || common.receipt_mint.is_signer
        || common.receipt_mint.executable
        || common.token_program.key.to_bytes() != TOKEN_2022_PROGRAM_ID
        || !common.token_program.executable
        || common.token_program.is_writable
        || common.token_program.is_signer
        || common.rent_credit.key.to_bytes() != header.rent_credit
        || common.rent_credit.is_writable != header.action.retires()
        || common.rent_credit.is_signer
        || common.rent_credit.executable
        || common.rent_program.key.to_bytes() != header.rent_program
        || !common.rent_program.executable
        || common.rent_program.is_writable
        || common.rent_program.is_signer
        || common.aggregate.is_writable
        || common.aggregate.is_signer
        || common.aggregate.executable
        || common.core_market.is_writable
        || common.core_market.is_signer
        || common.core_market.executable
        || !common.core_program.executable
        || common.core_program.is_writable
        || common.core_program.is_signer
        || common.core_programdata.is_writable
        || common.core_programdata.is_signer
    {
        return Err(RationalLifecycleSbfErrorV2::Accounts.into());
    }
    // Coordinate actions execute the canonical ProtocolPosition lifecycle in
    // this same Claims invocation.  That sole state writer independently
    // reauthenticates Trading and Claims; Admit also reauthenticates Core.
    // Repeating those Registry CPIs here would add no authority fact and can
    // make the runtime-width positive route exceed the SVM transaction budget.
    // Close does not consume Core, so retirement retains the one missing Core
    // selection check here.  Receipt-wide actions have no ProtocolPosition
    // child and therefore authenticate all three roles here.
    match header.action {
        LifecycleActionV2::ActivateCoordinate => {}
        LifecycleActionV2::RetireCoordinate => authenticate_release(
            common,
            header.release_set,
            ExecutionRoleV1::Core,
            common.core_program,
            common.core_programdata,
        )?,
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt => {
            authenticate_releases(common, header.release_set)?;
        }
    }
    authenticate_market(program_id, common, request)?;
    authenticate_rent_credit(common, request)?;
    if common.rent_credit.lamports() != header.rent_credit_before {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    Ok(())
}

fn authenticate_descriptor_resources(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
) -> Result<(), ProgramError> {
    let header = request.header();
    let receipt_seeds = RationalReceiptMintSeedsV2::new(
        descriptor.graph_digest(),
        header.market,
        header.release_set,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
    let expected_receipt = Pubkey::find_program_address(&receipt_seeds.as_slices(), program_id).0;
    receipt_seeds
        .authenticate_address(expected_receipt.to_bytes(), descriptor.receipt_mint())
        .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
    if common.receipt_mint.key != &expected_receipt
        || header.receipt_mint != expected_receipt.to_bytes()
        || descriptor.graph_id() != header.graph_id
    {
        return Err(RationalLifecycleSbfErrorV2::Descriptor.into());
    }
    Ok(())
}

fn authenticate_releases(
    common: CommonAccounts<'_, '_>,
    release_set: [u8; 32],
) -> Result<(), ProgramError> {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            common.trading_program,
            common.trading_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            common.claims_program,
            common.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            common.core_program,
            common.core_programdata,
        ),
    ] {
        authenticate_release(common, release_set, role, program, programdata)?;
    }
    Ok(())
}

fn authenticate_release<'accounts, 'info>(
    common: CommonAccounts<'accounts, 'info>,
    release_set: [u8; 32],
    role: ExecutionRoleV1,
    program: &'accounts AccountInfo<'info>,
    programdata: &'accounts AccountInfo<'info>,
) -> Result<(), ProgramError> {
    let receipt = reauthenticate(common.registry, common.cache, role, program, programdata)
        .map_err(|_| RationalLifecycleSbfErrorV2::Release)?;
    if receipt.execution_release_set_id().as_bytes() != &release_set {
        return Err(RationalLifecycleSbfErrorV2::Release.into());
    }
    Ok(())
}

fn authenticate_market(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
) -> Result<(), ProgramError> {
    let header = request.header();
    let aggregate_data = common
        .aggregate
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let aggregate =
        MarketViewV2::decode(&aggregate_data).map_err(|_| RationalLifecycleSbfErrorV2::Market)?;
    let expected_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, header.market.as_slice()],
        program_id,
    )
    .0;
    if common.aggregate.owner != program_id
        || common.aggregate.key != &expected_aggregate
        || aggregate.logical_market != header.market
        || aggregate.release_set != header.release_set
        || aggregate.registry_program != common.registry.key.to_bytes()
        || aggregate.claim_count != header.outcome_count
        || aggregate.revision != header.expected_claims_market_revision
        || aggregate.generation != header.generation
    {
        return Err(RationalLifecycleSbfErrorV2::Market.into());
    }
    drop(aggregate_data);

    let core_data = common
        .core_market
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| RationalLifecycleSbfErrorV2::Market)?;
    let core_seeds = MarketCoreStateSeedsV2::new(core.identity);
    let expected_core =
        Pubkey::find_program_address(&core_seeds.as_slices(), common.core_program.key).0;
    let expected_phase = if header.action.activates() {
        CorePhase::Open
    } else {
        CorePhase::Retiring
    };
    if common.core_market.owner != common.core_program.key
        || common.core_market.key != &expected_core
        || core.identity.market_id.to_bytes() != header.market
        || core.identity.selected_release_set.to_bytes() != header.release_set
        || core.identity.registry_program.to_bytes() != common.registry.key.to_bytes()
        || core.identity.generation != header.generation
        || core.rent_beneficiary.to_bytes() != header.rent_credit
        || core.phase != expected_phase
    {
        return Err(RationalLifecycleSbfErrorV2::Market.into());
    }
    Ok(())
}

fn authenticate_rent_credit(
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
) -> Result<(), ProgramError> {
    let data = common
        .rent_credit
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let credit =
        LifecycleRentCreditV2::decode(&data).map_err(|_| RationalLifecycleSbfErrorV2::Rent)?;
    let header = request.header();
    if credit.market().to_bytes() != header.market
        || credit.release_set().to_bytes() != header.release_set
        || credit.generation() != header.generation
    {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        common.rent_program.key,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Rent)?;
    if common.rent_credit.owner != common.rent_program.key
        || common.rent_credit.key != &expected
        || header.rent_credit != expected.to_bytes()
    {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    Ok(())
}

fn activate_receipt(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
    graph_digest: [u8; 32],
    rent: &Rent,
) -> Result<(), ProgramError> {
    let header = request.header();
    if rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2) != header.receipt_rent_principal {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    authenticate_vacant_prepaid(
        common.receipt_mint,
        header.observed_receipt_lamports,
        header.receipt_rent_principal,
    )?;
    let seeds = RationalReceiptMintSeedsV2::new(graph_digest, header.market, header.release_set)
        .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
    let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
    let [domain, graph, market, release] = seeds.as_slices();
    allocate_and_assign(
        common.receipt_mint,
        common.system,
        common.token_program.key,
        TOKEN_2022_CLOSEABLE_MINT_BYTES_V2,
        &[domain, graph, market, release, &bump],
    )?;
    initialize_closeable_mint(common, common.receipt_mint)?;
    authenticate_closeable_mint(common, common.receipt_mint, 0)
}

#[inline(never)]
fn activate_coordinate(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    request: LifecycleRequestV2<'_>,
    request_digest: [u8; 32],
    rent: &Rent,
) -> Result<[u8; 32], ProgramError> {
    let accounts = CoordinateAccounts::parse(account_infos)?;
    let row = single_coordinate(request)?;
    authenticate_coordinate_accounts(program_id, accounts, request, row)?;
    if rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2) != row.shard_rent_principal
        || rent.minimum_balance(ACCOUNT_BYTES) != row.structured_rent_principal
    {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    authenticate_vacant_prepaid(
        accounts.shard_mint,
        row.observed_shard_lamports,
        row.shard_rent_principal,
    )?;
    authenticate_vacant_prepaid(
        accounts.structured_custody,
        row.observed_structured_lamports,
        row.structured_rent_principal,
    )?;
    let descriptor = request.header().descriptor_id;
    let outcome = row.outcome.to_le_bytes();
    let shard_bump = [Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor, &outcome],
        program_id,
    )
    .1];
    allocate_and_assign(
        accounts.shard_mint,
        accounts.common.system,
        accounts.common.token_program.key,
        TOKEN_2022_CLOSEABLE_MINT_BYTES_V2,
        &[
            RATIONAL_SHARD_MINT_SEED_V2,
            &descriptor,
            &outcome,
            &shard_bump,
        ],
    )?;
    initialize_closeable_mint(accounts.common, accounts.shard_mint)?;
    let custody_bump = [Pubkey::find_program_address(
        &[RATIONAL_STRUCTURED_CUSTODY_SEED_V2, &descriptor, &outcome],
        program_id,
    )
    .1];
    allocate_and_assign(
        accounts.structured_custody,
        accounts.common.system,
        accounts.common.token_program.key,
        ACCOUNT_BYTES,
        &[
            RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
            &descriptor,
            &outcome,
            &custody_bump,
        ],
    )?;
    initialize_structured_custody(accounts)?;
    let digest = execute_protocol_position(
        program_id,
        accounts,
        request,
        request_digest,
        row,
        ProtocolPositionActionV2::Admit,
    )?;
    authenticate_closeable_mint(accounts.common, accounts.shard_mint, 0)?;
    authenticate_structured_custody(accounts, 0)?;
    Ok(digest)
}

#[inline(never)]
fn retire_coordinate(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    request: LifecycleRequestV2<'_>,
    request_digest: [u8; 32],
) -> Result<[u8; 32], ProgramError> {
    let accounts = CoordinateAccounts::parse(account_infos)?;
    let row = single_coordinate(request)?;
    authenticate_coordinate_accounts(program_id, accounts, request, row)?;
    authenticate_closeable_mint(
        accounts.common,
        accounts.shard_mint,
        row.expected_shard_supply,
    )?;
    authenticate_structured_custody(accounts, row.expected_structured_amount)?;
    close_token_resource(program_id, accounts.common, accounts.shard_mint)?;
    close_token_resource(program_id, accounts.common, accounts.structured_custody)?;
    let digest = execute_protocol_position(
        program_id,
        accounts,
        request,
        request_digest,
        row,
        ProtocolPositionActionV2::Close,
    )?;
    require_closed(accounts.shard_mint)?;
    require_closed(accounts.structured_custody)?;
    require_closed(accounts.position)?;
    require_closed(accounts.admission)?;
    Ok(digest)
}

fn retire_receipt(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
) -> Result<(), ProgramError> {
    authenticate_closeable_mint(
        common,
        common.receipt_mint,
        request.header().expected_receipt_supply,
    )?;
    close_token_resource(program_id, common, common.receipt_mint)?;
    require_closed(common.receipt_mint)
}

fn authenticate_coordinate_accounts(
    program_id: &Pubkey,
    accounts: CoordinateAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
    row: LifecycleCoordinateV2,
) -> Result<(), ProgramError> {
    let header = request.header();
    let outcome = row.outcome.to_le_bytes();
    let expected_shard = Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &header.descriptor_id, &outcome],
        program_id,
    )
    .0;
    let expected_structured = Pubkey::find_program_address(
        &[
            RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
            &header.descriptor_id,
            &outcome,
        ],
        program_id,
    )
    .0;
    let owner_seeds =
        ProtocolPositionClaimsCapabilitySeedsV2::new(header.descriptor_id, row.outcome)
            .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
    let expected_owner = Pubkey::find_program_address(&owner_seeds.as_slices(), program_id).0;
    let position_seeds = ProtocolPositionSeedsV2::new(
        accounts.common.aggregate.key.to_bytes(),
        expected_owner.to_bytes(),
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        accounts.common.aggregate.key.to_bytes(),
        expected_owner.to_bytes(),
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0;
    if accounts.shard_mint.key != &expected_shard
        || row.shard_mint != expected_shard.to_bytes()
        || accounts.structured_custody.key != &expected_structured
        || row.structured_custody_account != expected_structured.to_bytes()
        || accounts.owner.key != &expected_owner
        || row.claims_custody_owner != expected_owner.to_bytes()
        || accounts.position.key != &expected_position
        || row.claims_custody_position != expected_position.to_bytes()
        || accounts.admission.key != &expected_admission
        || row.position_admission != expected_admission.to_bytes()
        || !accounts.child_authority.is_signer
        || accounts.child_authority.is_writable
        || accounts.child_authority.executable
        || !accounts.position.is_writable
        || accounts.position.is_signer
        || accounts.position.executable
        || !accounts.admission.is_writable
        || accounts.admission.is_signer
        || accounts.admission.executable
        || !accounts.shard_mint.is_writable
        || accounts.shard_mint.is_signer
        || accounts.shard_mint.executable
        || !accounts.structured_custody.is_writable
        || accounts.structured_custody.is_signer
        || accounts.structured_custody.executable
        || accounts.owner.is_writable
        || accounts.owner.is_signer
        || accounts.owner.executable
    {
        return Err(RationalLifecycleSbfErrorV2::Accounts.into());
    }
    for record in [
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_record,
        accounts.result_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
    ] {
        if record.is_writable || record.is_signer || record.executable {
            return Err(RationalLifecycleSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

fn execute_protocol_position(
    program_id: &Pubkey,
    accounts: CoordinateAccounts<'_, '_>,
    lifecycle: LifecycleRequestV2<'_>,
    lifecycle_digest: [u8; 32],
    row: LifecycleCoordinateV2,
    action: ProtocolPositionActionV2,
) -> Result<[u8; 32], ProgramError> {
    let header = lifecycle.header();
    let owner = accounts.owner.key.to_bytes();
    let request = ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: header.release_set,
        market: header.market,
        position_owner: owner,
        parent_request_digest: lifecycle_digest,
        rent_credit: header.rent_credit,
        rent_program: header.rent_program,
        generation: header.generation,
        expected_market_revision: header.expected_claims_market_revision,
        expected_position_revision: row.expected_position_revision,
        observed_position_lamports: row.observed_position_lamports,
        observed_admission_lamports: row.observed_admission_lamports,
        position_rent_principal: row.position_rent_principal,
        admission_rent_principal: row.admission_rent_principal,
        capability_descriptor: header.descriptor_id,
        capability_outcome: row.outcome,
    }
    .new()
    .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
    let request_bytes = request
        .to_bytes()
        .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
    if request_bytes.len() != PROTOCOL_POSITION_REQUEST_BYTES_V2 {
        return Err(RationalLifecycleSbfErrorV2::Position.into());
    }
    let request_digest = hash(&request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        owner,
        request_digest,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Release)?;
    let expected_authority = Pubkey::find_program_address(
        &authority_seeds.as_slices(),
        accounts.common.trading_program.key,
    )
    .0;
    if accounts.child_authority.key != &expected_authority {
        return Err(RationalLifecycleSbfErrorV2::Release.into());
    }
    let child_accounts = protocol_position_accounts(accounts, action);
    protocol_position_v2::process(program_id, &child_accounts, &request_bytes)
        .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
    let (producer, bytes) = get_return_data().ok_or(RationalLifecycleSbfErrorV2::Receipt)?;
    if producer != *program_id {
        return Err(RationalLifecycleSbfErrorV2::Receipt.into());
    }
    match action {
        ProtocolPositionActionV2::Admit => {
            if bytes.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2 {
                return Err(RationalLifecycleSbfErrorV2::Receipt.into());
            }
            ProtocolPositionAdmissionV2::decode_receipt(&bytes)
                .and_then(|receipt| {
                    receipt.validate_request(
                        request,
                        request_digest,
                        program_id.to_bytes(),
                        accounts.common.trading_program.key.to_bytes(),
                    )
                })
                .map_err(|_| RationalLifecycleSbfErrorV2::Receipt)?;
        }
        ProtocolPositionActionV2::Close => {
            if bytes.len() != PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2 {
                return Err(RationalLifecycleSbfErrorV2::Receipt.into());
            }
            ProtocolPositionCloseReceiptV2::decode(&bytes)
                .and_then(|receipt| {
                    receipt.validate_request(request, request_digest, program_id.to_bytes())
                })
                .map_err(|_| RationalLifecycleSbfErrorV2::Receipt)?;
        }
    }
    Ok(hash(&bytes).to_bytes())
}

fn protocol_position_accounts<'info>(
    accounts: CoordinateAccounts<'_, 'info>,
    action: ProtocolPositionActionV2,
) -> Vec<AccountInfo<'info>> {
    let mut output = vec![
        accounts.child_authority.clone(),
        accounts.common.aggregate.clone(),
        accounts.position.clone(),
        accounts.admission.clone(),
    ];
    if action == ProtocolPositionActionV2::Admit {
        output.extend([
            accounts.basis_record.clone(),
            accounts.basis_staging.clone(),
            accounts.product_record.clone(),
            accounts.product_staging.clone(),
            accounts.result_record.clone(),
            accounts.result_staging.clone(),
            accounts.portfolio_record.clone(),
            accounts.portfolio_staging.clone(),
            accounts.common.rent.clone(),
            accounts.common.system.clone(),
            accounts.common.core_market.clone(),
            accounts.common.cache.clone(),
            accounts.common.registry.clone(),
            accounts.common.trading_program.clone(),
            accounts.common.trading_programdata.clone(),
            accounts.common.claims_program.clone(),
            accounts.common.claims_programdata.clone(),
            accounts.common.core_program.clone(),
            accounts.common.core_programdata.clone(),
            accounts.owner.clone(),
            accounts.common.rent_credit.clone(),
            accounts.common.rent_program.clone(),
        ]);
    } else {
        output.extend([
            accounts.common.rent.clone(),
            accounts.common.system.clone(),
            accounts.common.cache.clone(),
            accounts.common.registry.clone(),
            accounts.common.trading_program.clone(),
            accounts.common.trading_programdata.clone(),
            accounts.common.claims_program.clone(),
            accounts.common.claims_programdata.clone(),
            accounts.owner.clone(),
            accounts.common.rent_credit.clone(),
            accounts.common.rent_program.clone(),
        ]);
    }
    output
}

fn initialize_closeable_mint<'info>(
    common: CommonAccounts<'_, 'info>,
    mint: &AccountInfo<'info>,
) -> Result<(), ProgramError> {
    for instruction in [
        token_instruction::initialize_mint_close_authority(
            common.token_program.key,
            mint.key,
            Some(common.representation_authority.key),
        ),
        token_instruction::initialize_mint2(
            common.token_program.key,
            mint.key,
            common.representation_authority.key,
            None,
            0,
        ),
    ] {
        let instruction = instruction.map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
        invoke(&instruction, &[mint.clone(), common.token_program.clone()])
            .map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    }
    Ok(())
}

fn initialize_structured_custody(accounts: CoordinateAccounts<'_, '_>) -> Result<(), ProgramError> {
    let instruction = token_instruction::initialize_account3(
        accounts.common.token_program.key,
        accounts.structured_custody.key,
        accounts.shard_mint.key,
        accounts.common.representation_authority.key,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    invoke(
        &instruction,
        &[
            accounts.structured_custody.clone(),
            accounts.shard_mint.clone(),
            accounts.common.token_program.clone(),
        ],
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Token.into())
}

fn authenticate_closeable_mint(
    common: CommonAccounts<'_, '_>,
    mint: &AccountInfo<'_>,
    expected_supply: u64,
) -> Result<(), ProgramError> {
    let data = mint
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    Token2022CloseableMintProfileV2::check_mint(
        common.token_program.key.to_bytes(),
        &data,
        common.representation_authority.key.to_bytes(),
        common.representation_authority.key.to_bytes(),
        expected_supply,
        0,
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    Ok(())
}

fn authenticate_structured_custody(
    accounts: CoordinateAccounts<'_, '_>,
    expected_amount: u64,
) -> Result<(), ProgramError> {
    let data = accounts
        .structured_custody
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let token = TokenAccount::parse(&data).map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    if accounts.structured_custody.owner != accounts.common.token_program.key
        || token.mint != accounts.shard_mint.key.to_bytes()
        || token.owner != accounts.common.representation_authority.key.to_bytes()
        || token.amount != expected_amount
        || token.state != AccountState::Initialized
        || !token.delegate.is_none()
        || token.delegated_amount != 0
        || !token.native_reserve.is_none()
        || !token.close_authority.is_none()
    {
        return Err(RationalLifecycleSbfErrorV2::Token.into());
    }
    Ok(())
}

fn close_token_resource<'info>(
    program_id: &Pubkey,
    common: CommonAccounts<'_, 'info>,
    resource: &AccountInfo<'info>,
) -> Result<(), ProgramError> {
    let descriptor = common
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    let descriptor_id = hash(&descriptor).to_bytes();
    drop(descriptor);
    let (_, bump) = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        program_id,
    );
    let bump = [bump];
    let instruction = token_instruction::close_account(
        common.token_program.key,
        resource.key,
        common.rent_credit.key,
        common.representation_authority.key,
        &[],
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    invoke_signed(
        &instruction,
        &[
            resource.clone(),
            common.rent_credit.clone(),
            common.representation_authority.clone(),
            common.token_program.clone(),
        ],
        &[&[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            descriptor_id.as_slice(),
            &bump,
        ]],
    )
    .map_err(|_| RationalLifecycleSbfErrorV2::Token.into())
}

fn allocate_and_assign<'info>(
    resource: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    owner: &Pubkey,
    width: usize,
    signer: &[&[u8]],
) -> Result<(), ProgramError> {
    let width_u64 = u64::try_from(width).map_err(|_| RationalLifecycleSbfErrorV2::Rent)?;
    for instruction in [
        allocate(resource.key, width_u64),
        assign(resource.key, owner),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[resource.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| RationalLifecycleSbfErrorV2::Token)?;
    }
    if resource.owner != owner || resource.data_len() != width {
        return Err(RationalLifecycleSbfErrorV2::Token.into());
    }
    Ok(())
}

fn authenticate_vacant_prepaid(
    resource: &AccountInfo<'_>,
    observed_lamports: u64,
    rent_principal: u64,
) -> Result<(), ProgramError> {
    if resource.owner != &system_program::ID
        || !resource.data_is_empty()
        || resource.lamports() != observed_lamports
        || observed_lamports < rent_principal
    {
        return Err(RationalLifecycleSbfErrorV2::Rent.into());
    }
    Ok(())
}

fn authenticate_complete_vacancy(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
) -> Result<(), ProgramError> {
    for (row_index, coordinate) in request.coordinates().enumerate() {
        let coordinate = coordinate.map_err(|_| RationalLifecycleSbfErrorV2::Instruction)?;
        let base = row_index
            .checked_mul(RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
            .and_then(|offset| offset.checked_add(RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2))
            .ok_or(RationalLifecycleSbfErrorV2::Accounts)?;
        let shard = account(accounts, base + VACANCY_SHARD_MINT)?;
        let structured = account(accounts, base + VACANCY_STRUCTURED_CUSTODY)?;
        let position = account(accounts, base + VACANCY_POSITION)?;
        let admission = account(accounts, base + VACANCY_ADMISSION)?;
        let outcome = coordinate.outcome.to_le_bytes();
        let owner_seeds = ProtocolPositionClaimsCapabilitySeedsV2::new(
            request.header().descriptor_id,
            coordinate.outcome,
        )
        .map_err(|_| RationalLifecycleSbfErrorV2::Descriptor)?;
        let owner = Pubkey::find_program_address(&owner_seeds.as_slices(), program_id).0;
        let position_seeds =
            ProtocolPositionSeedsV2::new(common.aggregate.key.to_bytes(), owner.to_bytes())
                .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
        let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
            common.aggregate.key.to_bytes(),
            owner.to_bytes(),
        )
        .map_err(|_| RationalLifecycleSbfErrorV2::Position)?;
        let expected = [
            Pubkey::find_program_address(
                &[
                    RATIONAL_SHARD_MINT_SEED_V2,
                    &request.header().descriptor_id,
                    &outcome,
                ],
                program_id,
            )
            .0,
            Pubkey::find_program_address(
                &[
                    RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                    &request.header().descriptor_id,
                    &outcome,
                ],
                program_id,
            )
            .0,
            Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0,
            Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0,
        ];
        for (resource, expected) in [shard, structured, position, admission]
            .into_iter()
            .zip(expected)
        {
            if resource.key != &expected
                || resource.owner != &system_program::ID
                || !resource.data_is_empty()
                || resource.lamports() != 0
                || resource.is_writable
                || resource.is_signer
                || resource.executable
            {
                return Err(RationalLifecycleSbfErrorV2::Accounts.into());
            }
        }
    }
    Ok(())
}

fn post_resource_digest(
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    request: LifecycleRequestV2<'_>,
) -> Result<[u8; 32], ProgramError> {
    let mut transcript = Vec::new();
    transcript.extend_from_slice(&request.header().descriptor_id);
    transcript.push(action_byte(request.header().action));
    transcript.extend_from_slice(common.receipt_mint.key.as_ref());
    append_account_observation(&mut transcript, common.receipt_mint)?;
    transcript.extend_from_slice(common.rent_credit.key.as_ref());
    transcript.extend_from_slice(&common.rent_credit.lamports().to_le_bytes());
    match request.header().action {
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            let coordinate = CoordinateAccounts::parse(accounts)?;
            for resource in [
                coordinate.shard_mint,
                coordinate.structured_custody,
                coordinate.position,
                coordinate.admission,
            ] {
                transcript.extend_from_slice(resource.key.as_ref());
                append_account_observation(&mut transcript, resource)?;
            }
        }
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt => {}
    }
    Ok(hashv(&[POST_RESOURCE_DOMAIN_V2, &transcript]).to_bytes())
}

fn append_account_observation(
    output: &mut Vec<u8>,
    account: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    output.extend_from_slice(account.owner.as_ref());
    output.extend_from_slice(&account.lamports().to_le_bytes());
    let data = account
        .try_borrow_data()
        .map_err(|_| RationalLifecycleSbfErrorV2::Accounts)?;
    output.extend_from_slice(&hash(&data).to_bytes());
    Ok(())
}

fn single_coordinate(
    request: LifecycleRequestV2<'_>,
) -> Result<LifecycleCoordinateV2, ProgramError> {
    request
        .coordinates()
        .next()
        .ok_or(RationalLifecycleSbfErrorV2::Instruction)?
        .map_err(|_| RationalLifecycleSbfErrorV2::Instruction.into())
}

fn require_closed(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.lamports() != 0 {
        return Err(RationalLifecycleSbfErrorV2::Receipt.into());
    }
    Ok(())
}

const fn action_byte(action: LifecycleActionV2) -> u8 {
    match action {
        LifecycleActionV2::ActivateReceipt => 0,
        LifecycleActionV2::ActivateCoordinate => 1,
        LifecycleActionV2::RetireCoordinate => 2,
        LifecycleActionV2::RetireReceipt => 3,
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| RationalLifecycleSbfErrorV2::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_geometry_is_sparse_and_runtime_width() {
        assert_eq!(RATIONAL_LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, 20);
        assert_eq!(RATIONAL_LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2, 34);
        assert_eq!(RATIONAL_LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, 4);
        assert_eq!(
            dclutch_rational_representation_v2_lifecycle_contract::LIFECYCLE_COORDINATE_BYTES_V2,
            272
        );
    }

    #[test]
    fn error_codes_are_stable_and_disjoint() {
        assert_eq!(
            ProgramError::from(RationalLifecycleSbfErrorV2::Instruction),
            ProgramError::Custom(210)
        );
        assert_eq!(
            ProgramError::from(RationalLifecycleSbfErrorV2::Receipt),
            ProgramError::Custom(218)
        );
    }

    #[test]
    fn receipt_coordinate_uses_graph_digest_not_semantic_graph_id() {
        let program = Pubkey::new_from_array([0xa1; 32]);
        let market = [0xa2; 32];
        let release = [0xa3; 32];
        let semantic_graph = [0xa4; 32];
        let canonical_digest = [0xa5; 32];
        let substituted_digest = [0xa6; 32];
        let semantic_seeds =
            RationalReceiptMintSeedsV2::new(semantic_graph, market, release).expect("semantic");
        let canonical_seeds =
            RationalReceiptMintSeedsV2::new(canonical_digest, market, release).expect("canonical");
        let substituted_seeds =
            RationalReceiptMintSeedsV2::new(substituted_digest, market, release)
                .expect("substituted");
        let semantic = Pubkey::find_program_address(&semantic_seeds.as_slices(), &program).0;
        let canonical = Pubkey::find_program_address(&canonical_seeds.as_slices(), &program).0;
        let substituted = Pubkey::find_program_address(&substituted_seeds.as_slices(), &program).0;
        assert_ne!(canonical, semantic);
        assert_ne!(canonical, substituted);
        assert!(
            canonical_seeds
                .authenticate_address(canonical.to_bytes(), canonical.to_bytes())
                .is_ok()
        );
        assert!(
            canonical_seeds
                .authenticate_address(canonical.to_bytes(), substituted.to_bytes())
                .is_err()
        );
    }
}
