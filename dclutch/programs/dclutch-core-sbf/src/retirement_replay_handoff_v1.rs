//! Core-authorized retirement-only Custody replay handoff.

use alloc::vec::Vec;

use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2,
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyVaultSeedsV1, RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1,
    RetirementReplayHandoffReceiptV1, RetirementReplayHandoffRequestV1,
    retirement_replay_handoff_accounts_v1::*,
};
use dclutch_market_core_codec::{
    CoreState, MarketAdmissionV1, MarketCoreStateSeedsV2, Phase, Role,
};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{AccountState, COption, TokenAccount};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    release::{RoleDeploymentAccounts, authenticate_roles},
};

/// Market phases in which a retirement replay handoff is admissible.
///
/// The written guard names no readiness, and that is accurate rather than
/// lax: readiness is spent long before a Market reaches `Retiring`, so it
/// carries no further authority here.
pub const RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Retiring]);

/// Execute and verify one atomic retirement replay handoff.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let accounts = require_frame(program_id, accounts)?;
    let rent = Rent::from_account_info(&accounts[RENT]).map_err(|_| CoreSbfError::Creation)?;
    if rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1) != request.core_replay_rent_lamports() {
        return Err(CoreSbfError::Creation.into());
    }
    let state = authenticate_market(program_id, accounts, request)?;
    let claims_program = authenticate_releases(accounts, state)?;
    authenticate_claims_and_realm(accounts, state, claims_program, request)?;
    authenticate_rent_credit(accounts, state)?;
    authenticate_prestate(program_id, accounts, state, request)?;
    invoke_custody(program_id, accounts, state, request, request_bytes)?;
    authenticate_poststate(accounts, request, request_bytes)
}

/// Check the frame once and hand back the arity as a type, so that every
/// downstream `accounts[ORDINAL]` is a proof rather than a hope.
fn require_frame<'a, 'b>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'b>],
) -> Result<&'a [AccountInfo<'b>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1], ProgramError> {
    let accounts: &'a [AccountInfo<'b>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1] = accounts
        .try_into()
        .map_err(|_| CoreSbfError::AccountFrame)?;
    crate::frame::require_distinct(accounts.as_slice())?;
    for (index, account) in accounts.iter().enumerate() {
        let writable = matches!(index, PAYER | RENT_CREDIT | TRADING_REPLAY | CORE_REPLAY);
        let signer = index == PAYER;
        let executable = matches!(
            index,
            REGISTRY | CORE_PROGRAM | TRADING_PROGRAM | CUSTODY_PROGRAM | SYSTEM | TOKEN_PROGRAM
        );
        if account.is_writable != writable
            || account.is_signer != signer
            || account.executable != executable
        {
            return Err(CoreSbfError::AccountFrame.into());
        }
    }
    if accounts[CORE_PROGRAM].key != program_id
        || accounts[SYSTEM].key != &system_program::ID
        || accounts[RENT].key != &sysvar::rent::ID
        || accounts[CALLER_AUTHORITY].owner != &system_program::ID
        || !accounts[CALLER_AUTHORITY].data_is_empty()
        || accounts[CALLER_AUTHORITY].lamports() != 0
    {
        return Err(CoreSbfError::AccountFrame.into());
    }
    Ok(accounts)
}

fn authenticate_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    request: RetirementReplayHandoffRequestV1,
) -> Result<CoreState, ProgramError> {
    let data = accounts[MARKET]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    let state = CoreState::decode(&data).map_err(|_| CoreSbfError::Market)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        program_id,
    )
    .0;
    if accounts[MARKET].key != &expected
        || accounts[MARKET].owner != program_id
        || accounts[MARKET].key.to_bytes() != request.market()
        || state.identity.market_id.to_bytes() != request.market()
        || state.identity.registry_program.to_bytes() != accounts[REGISTRY].key.to_bytes()
        || state.identity.generation != request.generation()
        || !RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
    {
        return Err(CoreSbfError::Market.into());
    }
    Ok(state)
}

fn authenticate_releases(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
) -> Result<[u8; 32], ProgramError> {
    let admissions = authenticate_roles(
        &accounts[CACHE],
        &accounts[REGISTRY],
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        &[
            RoleDeploymentAccounts::new(
                Role::Core,
                &accounts[CORE_PROGRAM],
                &accounts[CORE_PROGRAMDATA],
            ),
            RoleDeploymentAccounts::new(
                Role::Trading,
                &accounts[TRADING_PROGRAM],
                &accounts[TRADING_PROGRAMDATA],
            ),
            RoleDeploymentAccounts::new(
                Role::Custody,
                &accounts[CUSTODY_PROGRAM],
                &accounts[CUSTODY_PROGRAMDATA],
            ),
        ],
    )?;
    Ok(admissions
        .projected_binding(Role::Claims)
        .program
        .to_bytes())
}

fn authenticate_claims_and_realm(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    claims_program: [u8; 32],
    request: RetirementReplayHandoffRequestV1,
) -> ProgramResult {
    let claims_program = Pubkey::new_from_array(claims_program);
    let expected_aggregate = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            accounts[MARKET].key.as_ref(),
        ],
        &claims_program,
    )
    .0;
    let aggregate_data = accounts[CLAIMS_AGGREGATE]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let aggregate =
        LiabilityBasisMarketViewV2::decode(&aggregate_data).map_err(|_| CoreSbfError::Reference)?;
    if accounts[CLAIMS_AGGREGATE].key != &expected_aggregate
        || accounts[CLAIMS_AGGREGATE].owner != &claims_program
        || aggregate.logical_market != accounts[MARKET].key.to_bytes()
        || aggregate.release_set != state.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != accounts[REGISTRY].key.to_bytes()
        || aggregate.product_instance_id != state.identity.product_id.to_bytes()
        || aggregate.realm_id != state.identity.realm_id.to_bytes()
        || aggregate.custody_context != request.context()
        || aggregate.generation != state.identity.generation
    {
        return Err(CoreSbfError::Reference.into());
    }
    drop(aggregate_data);

    let realm_data = accounts[REALM]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let realm_digest = hash(&realm_data).to_bytes();
    let expected_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        accounts[REGISTRY].key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        accounts[REGISTRY].key,
    )
    .0;
    let realm = RealmV1::decode(&realm_data).map_err(|_| CoreSbfError::FinalizedRecord)?;
    if accounts[REALM].key != &expected_realm
        || accounts[REALM].owner != accounts[REGISTRY].key
        || accounts[REALM].data_len() != REALM_BYTES
        || realm_digest != state.identity.realm_id.to_bytes()
        || accounts[REALM_STAGING].key != &expected_staging
        || accounts[REALM_STAGING].owner != &system_program::ID
        || !accounts[REALM_STAGING].data_is_empty()
        || accounts[MINT].key.to_bytes() != *realm.collateral_mint()
        || accounts[MINT].owner != accounts[TOKEN_PROGRAM].key
        || accounts[TOKEN_PROGRAM].key.to_bytes() != *realm.token_program()
    {
        return Err(CoreSbfError::FinalizedRecord.into());
    }
    Ok(())
}

fn authenticate_rent_credit(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
) -> ProgramResult {
    let data = accounts[RENT_CREDIT]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| CoreSbfError::RentCredit)?;
    let seeds = credit.pda_seeds();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.market().to_bytes().as_slice(),
            generation.as_slice(),
            &bump,
        ],
        accounts[RENT_CREDIT].owner,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if accounts[RENT_CREDIT].key != &expected
        || accounts[RENT_CREDIT].owner == &system_program::ID
        || accounts[RENT_CREDIT].key.to_bytes() != state.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != state.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || credit.generation() != state.identity.generation
    {
        return Err(CoreSbfError::RentCredit.into());
    }
    Ok(())
}

fn authenticate_prestate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
) -> ProgramResult {
    let release = state.identity.selected_release_set.to_bytes();
    let source_seeds = CustodyReplaySeedsV1::new(
        request.market(),
        release,
        ExecutionRoleV1::Trading,
        request.context(),
    );
    let core_seeds = CustodyReplaySeedsV1::new(
        request.market(),
        release,
        ExecutionRoleV1::Core,
        request.context(),
    );
    let vault_seeds = CustodyVaultSeedsV1::new(
        request.market(),
        release,
        request.context(),
        CompartmentV1::HoardPrincipal,
    );
    let authority_seeds = CustodyAuthoritySeedsV1::new(request.market(), release);
    if accounts[TRADING_REPLAY].key
        != &Pubkey::find_program_address(&source_seeds.as_slices(), accounts[CUSTODY_PROGRAM].key).0
        || accounts[TRADING_REPLAY].owner != accounts[CUSTODY_PROGRAM].key
        || accounts[TRADING_REPLAY].data_len() != CUSTODY_REPLAY_BYTES_V1
        || accounts[CORE_REPLAY].key
            != &Pubkey::find_program_address(&core_seeds.as_slices(), accounts[CUSTODY_PROGRAM].key)
                .0
        || accounts[CORE_REPLAY].owner != &system_program::ID
        || accounts[CORE_REPLAY].lamports() != 0
        || !accounts[CORE_REPLAY].data_is_empty()
        || accounts[HOARD].key
            != &Pubkey::find_program_address(
                &vault_seeds.as_slices(),
                accounts[CUSTODY_PROGRAM].key,
            )
            .0
        || accounts[CUSTODY_AUTHORITY].key
            != &Pubkey::find_program_address(
                &authority_seeds.as_slices(),
                accounts[CUSTODY_PROGRAM].key,
            )
            .0
        || accounts[TRADING_REPLAY].lamports() != request.trading_replay_lamports()
        || accounts[HOARD].lamports() != request.hoard_lamports()
        || accounts[RENT_CREDIT].lamports() != request.rent_credit_lamports()
        || accounts[PAYER].lamports() != request.payer_lamports()
    {
        return Err(CoreSbfError::Reference.into());
    }
    let replay_data = accounts[TRADING_REPLAY]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| CoreSbfError::Reference)?;
    if hash(&replay_data).to_bytes() != request.trading_replay_digest()
        || replay.caller_role != ExecutionRoleV1::Trading
        || replay.release_set != release
        || replay.market != request.market()
        || replay.realm != state.identity.realm_id.to_bytes()
        || replay.context != request.context()
        || replay.caller_program != accounts[TRADING_PROGRAM].key.to_bytes()
        || replay.rent_refund != accounts[RENT_CREDIT].key.to_bytes()
        || replay.open_vault_count != 1
        || replay.next_revision != request.revision()
        || replay.generation != request.generation()
    {
        return Err(CoreSbfError::Reference.into());
    }
    drop(replay_data);
    let hoard_data = accounts[HOARD]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let token = TokenAccount::parse(&hoard_data).map_err(|_| CoreSbfError::Reference)?;
    if hash(&hoard_data).to_bytes() != request.hoard_data_digest()
        || accounts[HOARD].owner != accounts[TOKEN_PROGRAM].key
        || token.mint != accounts[MINT].key.to_bytes()
        || token.owner != accounts[CUSTODY_AUTHORITY].key.to_bytes()
        || token.state != AccountState::Initialized
        || token.delegate != COption::None
        || token.native_reserve != COption::None
        || token.close_authority != COption::None
        || token.delegated_amount != 0
    {
        return Err(CoreSbfError::Reference.into());
    }
    let digest = hash(request.to_bytes().as_slice()).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release,
        request.market(),
        ExecutionRoleV1::Core,
        request.context(),
        digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected_caller = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0;
    if accounts[CALLER_AUTHORITY].key != &expected_caller {
        return Err(CoreSbfError::CallerAuthority.into());
    }
    Ok(())
}

fn invoke_custody(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let mut metas = Vec::with_capacity(accounts.len());
    let mut infos = Vec::with_capacity(accounts.len().saturating_add(1));
    for (index, account) in accounts.iter().enumerate() {
        let writable = matches!(index, PAYER | RENT_CREDIT | TRADING_REPLAY | CORE_REPLAY);
        let signer = matches!(index, PAYER | CALLER_AUTHORITY);
        metas.push(if writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
        infos.push(account.clone());
    }
    infos.push(accounts[CUSTODY_PROGRAM].clone());
    let instruction = Instruction {
        program_id: *accounts[CUSTODY_PROGRAM].key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let digest = hash(request_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        state.identity.selected_release_set.to_bytes(),
        request.market(),
        ExecutionRoleV1::Core,
        request.context(),
        digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| CoreSbfError::ChildCpi.into())
}

fn authenticate_poststate(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *accounts[CUSTODY_PROGRAM].key {
        return Err(CoreSbfError::ChildAck.into());
    }
    let receipt = RetirementReplayHandoffReceiptV1::decode(&receipt_bytes)
        .map_err(|_| CoreSbfError::ChildAck)?;
    let core_data = accounts[CORE_REPLAY]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let core_digest = hash(&core_data).to_bytes();
    drop(core_data);
    let hoard_data = accounts[HOARD]
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let hoard_digest = hash(&hoard_data).to_bytes();
    if receipt.request_digest != hash(request_bytes).to_bytes()
        || receipt.market != request.market()
        || receipt.context != request.context()
        || receipt.trading_replay != accounts[TRADING_REPLAY].key.to_bytes()
        || receipt.core_replay != accounts[CORE_REPLAY].key.to_bytes()
        || receipt.hoard_vault != accounts[HOARD].key.to_bytes()
        || receipt.trading_replay_pre_digest != request.trading_replay_digest()
        || receipt.core_replay_post_digest != core_digest
        || receipt.hoard_pre_data_digest != request.hoard_data_digest()
        || receipt.hoard_post_data_digest != hoard_digest
        || receipt.generation != request.generation()
        || receipt.revision != request.revision()
        || receipt.trading_replay_pre_lamports != request.trading_replay_lamports()
        || receipt.core_replay_post_lamports != request.core_replay_rent_lamports()
        || receipt.hoard_pre_lamports != request.hoard_lamports()
        || receipt.hoard_post_lamports != request.hoard_lamports()
        || receipt.rent_credit_pre_lamports != request.rent_credit_lamports()
        || receipt.payer_pre_lamports != request.payer_lamports()
        || accounts[TRADING_REPLAY].owner != &system_program::ID
        || accounts[TRADING_REPLAY].lamports() != 0
        || !accounts[TRADING_REPLAY].data_is_empty()
        || accounts[CORE_REPLAY].owner != accounts[CUSTODY_PROGRAM].key
        || accounts[CORE_REPLAY].lamports() != request.core_replay_rent_lamports()
        || accounts[HOARD].lamports() != request.hoard_lamports()
        || accounts[RENT_CREDIT].lamports() != receipt.rent_credit_post_lamports
        || accounts[PAYER].lamports() != receipt.payer_post_lamports
    {
        return Err(CoreSbfError::ChildAck.into());
    }
    Ok(())
}
