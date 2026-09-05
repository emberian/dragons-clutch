//! Retirement-only Trading-to-Core replay authority handoff.

use alloc::boxed::Box;
use core::convert::TryFrom;

use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2,
};
use dclutch_custody::token_svm::{AccountState, COption};
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyVaultSeedsV1, RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1,
    RetirementReplayHandoffObservationV1, RetirementReplayHandoffPlanV1,
    RetirementReplayHandoffRequestV1, retirement_replay_handoff_accounts_v1::*,
};
use dclutch_market::realm::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_market::{CoreState, MarketAdmissionV1, MarketCoreStateSeedsV2, Phase};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::authenticate_activated_role_v1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::create_account;

use super::{CustodySbfError, collateral_profile, read_custody_account, registry_role};

/// Execute the one retirement-specific cross-role replay handoff.
/// Market phases in which Custody accepts a retirement replay handoff.
///
/// The Core half of this same handoff declares the same phase in its own
/// program (`core::retirement_replay_handoff_v1`). Two programs authenticate
/// one Market between them, so each names the prestate it enforces rather than
/// one deferring to the other's; a constant that drifted from Core's would show
/// as two different admissions on one act in `docs/reference/routes.md`, which
/// is where a reader would see it. The written guard names no readiness,
/// because readiness carries no authority by the time a Market is Retiring.
///
/// The program prefix is load-bearing, not decoration: the census keys these
/// constants by bare name and refuses a colliding one, so the first draft of
/// this file -- which shared Core's name exactly -- un-gated Core's route as
/// well as leaving this one ungated.
pub const CUSTODY_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::phases(&[Phase::Retiring]);

#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let accounts = require_frame(program_id, accounts)?;
    let market = authenticate_market(accounts, request)?;
    authenticate_current_roles(accounts, market)?;
    let aggregate = authenticate_claims_aggregate(accounts, market, request)?;
    authenticate_realm_and_rent_credit(accounts, market, aggregate)?;
    authenticate_caller(program_id, accounts, market, request, request_bytes)?;
    execute_handoff(
        program_id,
        accounts,
        market,
        aggregate,
        request,
        request_bytes,
    )
}

#[inline(never)]
fn execute_handoff(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    market: CoreState,
    aggregate: LiabilityBasisMarketViewV2,
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let plan = prepare_plan(
        program_id,
        accounts,
        market,
        aggregate,
        request,
        request_bytes,
    )?;
    let projected_digest = plan.receipt().core_replay_post_digest;
    create_core_replay(program_id, accounts, market, request)?;
    commit_core_replay(accounts, plan.core_replay())?;
    close_trading_replay(accounts, request)?;
    verify_poststate(accounts, request, plan.receipt(), projected_digest)?;
    set_return_data(&plan.receipt().to_bytes());
    Ok(())
}

#[inline(never)]
fn prepare_plan(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    market: CoreState,
    aggregate: LiabilityBasisMarketViewV2,
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> Result<Box<RetirementReplayHandoffPlanV1>, ProgramError> {
    let (source, source_digest) = authenticate_replays(program_id, accounts, market, request)?;
    let hoard_digest = authenticate_hoard(program_id, accounts, market, aggregate, request)?;

    let request_digest = hash(request_bytes).to_bytes();
    let projected = CustodyReplayV1 {
        caller_role: ExecutionRoleV1::Core,
        caller_program: accounts[CORE_PROGRAM].key.to_bytes(),
        ..source
    };
    let projected_bytes = projected.to_bytes().map_err(|_| CustodySbfError::Replay)?;
    let projected_digest = hash(&projected_bytes).to_bytes();
    let plan = RetirementReplayHandoffPlanV1::new(
        request,
        request_digest,
        RetirementReplayHandoffObservationV1 {
            core_program: accounts[CORE_PROGRAM].key.to_bytes(),
            trading_program: accounts[TRADING_PROGRAM].key.to_bytes(),
            trading_replay: accounts[TRADING_REPLAY].key.to_bytes(),
            core_replay: accounts[CORE_REPLAY].key.to_bytes(),
            hoard_vault: accounts[HOARD].key.to_bytes(),
            rent_credit: accounts[RENT_CREDIT].key.to_bytes(),
            replay: source,
            trading_replay_digest: source_digest,
            hoard_data_digest: hoard_digest,
            trading_replay_lamports: accounts[TRADING_REPLAY].lamports(),
            core_replay_lamports: accounts[CORE_REPLAY].lamports(),
            hoard_lamports: accounts[HOARD].lamports(),
            rent_credit_lamports: accounts[RENT_CREDIT].lamports(),
            payer_lamports: accounts[PAYER].lamports(),
        },
        projected_digest,
    )
    .map_err(|_| CustodySbfError::Replay)?;
    Ok(Box::new(plan))
}

/// Check the frame once and hand back the arity as a type, so that every
/// downstream `accounts[ORDINAL]` is a proof rather than a hope.
#[inline(never)]
fn require_frame<'a, 'b>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'b>],
) -> Result<&'a [AccountInfo<'b>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1], ProgramError> {
    let accounts: &'a [AccountInfo<'b>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1] = accounts
        .try_into()
        .map_err(|_| CustodySbfError::AccountFrame)?;
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key {
                return Err(CustodySbfError::AccountFrame.into());
            }
        }
    }
    for (index, account) in accounts.iter().enumerate() {
        let writable = matches!(index, PAYER | RENT_CREDIT | TRADING_REPLAY | CORE_REPLAY);
        let signer = matches!(index, PAYER | CALLER_AUTHORITY);
        let executable = matches!(
            index,
            REGISTRY | CORE_PROGRAM | TRADING_PROGRAM | CUSTODY_PROGRAM | SYSTEM | TOKEN_PROGRAM
        );
        if account.is_writable != writable
            || account.is_signer != signer
            || account.executable != executable
        {
            return Err(CustodySbfError::AccountFrame.into());
        }
    }
    if accounts[CUSTODY_PROGRAM].key != program_id
        || accounts[SYSTEM].key != &system_program::ID
        || accounts[RENT].key != &sysvar::rent::ID
        || accounts[CALLER_AUTHORITY].owner != &system_program::ID
        || !accounts[CALLER_AUTHORITY].data_is_empty()
        || accounts[CALLER_AUTHORITY].lamports() != 0
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(accounts)
}

#[inline(never)]
fn authenticate_market(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    request: RetirementReplayHandoffRequestV1,
) -> Result<CoreState, ProgramError> {
    let data = accounts[MARKET]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let state = CoreState::decode(&data).map_err(|_| CustodySbfError::Release)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        accounts[CORE_PROGRAM].key,
    )
    .0;
    if accounts[MARKET].key != &expected
        || accounts[MARKET].owner != accounts[CORE_PROGRAM].key
        || accounts[MARKET].key.to_bytes() != request.market()
        || state.identity.market_id.to_bytes() != request.market()
        || state.identity.registry_program.to_bytes() != accounts[REGISTRY].key.to_bytes()
        || state.identity.generation != request.generation()
        || !CUSTODY_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(state)
}

#[inline(never)]
fn authenticate_current_roles(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
) -> ProgramResult {
    let release = state.identity.selected_release_set.to_bytes();
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &accounts[CORE_PROGRAM],
            &accounts[CORE_PROGRAMDATA],
        ),
        (
            ExecutionRoleV1::Trading,
            &accounts[TRADING_PROGRAM],
            &accounts[TRADING_PROGRAMDATA],
        ),
        (
            ExecutionRoleV1::Custody,
            &accounts[CUSTODY_PROGRAM],
            &accounts[CUSTODY_PROGRAMDATA],
        ),
    ] {
        authenticate_activated_role_v1(
            &accounts[REGISTRY],
            &accounts[CACHE],
            &release,
            registry_role(role),
            program,
            programdata,
        )
        .map_err(CustodySbfError::from)?;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_claims_aggregate(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
) -> Result<LiabilityBasisMarketViewV2, ProgramError> {
    let cache_data = accounts[CACHE]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let cache = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| CustodySbfError::Release)?;
    let claims_program = cache
        .role(registry_role(ExecutionRoleV1::Claims))
        .map_err(|_| CustodySbfError::Release)?
        .release()
        .program()
        .to_bytes();
    drop(cache_data);
    let expected = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            accounts[MARKET].key.as_ref(),
        ],
        &Pubkey::new_from_array(claims_program),
    )
    .0;
    let data = accounts[CLAIMS_AGGREGATE]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let aggregate =
        LiabilityBasisMarketViewV2::decode(&data).map_err(|_| CustodySbfError::Release)?;
    if accounts[CLAIMS_AGGREGATE].key != &expected
        || accounts[CLAIMS_AGGREGATE].owner != &Pubkey::new_from_array(claims_program)
        || aggregate.logical_market != accounts[MARKET].key.to_bytes()
        || aggregate.release_set != state.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != accounts[REGISTRY].key.to_bytes()
        || aggregate.product_instance_id != state.identity.product_id.to_bytes()
        || aggregate.realm_id != state.identity.realm_id.to_bytes()
        || aggregate.custody_context != request.context()
        || aggregate.generation != state.identity.generation
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(aggregate)
}

#[inline(never)]
fn authenticate_realm_and_rent_credit(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    aggregate: LiabilityBasisMarketViewV2,
) -> ProgramResult {
    let realm_data = accounts[REALM]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Realm)?;
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
    let realm = RealmV1::decode(&realm_data).map_err(|_| CustodySbfError::Realm)?;
    drop(realm_data);
    if accounts[REALM].key != &expected_realm
        || accounts[REALM].owner != accounts[REGISTRY].key
        || accounts[REALM].data_len() != REALM_BYTES
        || realm_digest != state.identity.realm_id.to_bytes()
        || aggregate.realm_id != state.identity.realm_id.to_bytes()
        || accounts[REALM_STAGING].key != &expected_staging
        || accounts[REALM_STAGING].owner != &system_program::ID
        || !accounts[REALM_STAGING].data_is_empty()
        || accounts[MINT].key.to_bytes() != *realm.collateral_mint()
        || accounts[TOKEN_PROGRAM].key.to_bytes() != *realm.token_program()
        || accounts[MINT].owner != accounts[TOKEN_PROGRAM].key
    {
        return Err(CustodySbfError::Realm.into());
    }
    collateral_profile(realm)?;

    let credit_data = accounts[RENT_CREDIT]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Create)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| CustodySbfError::Create)?;
    drop(credit_data);
    let seeds = credit.pda_seeds();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected_credit = Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.market().to_bytes().as_slice(),
            generation.as_slice(),
            &bump,
        ],
        accounts[RENT_CREDIT].owner,
    )
    .map_err(|_| CustodySbfError::Create)?;
    if accounts[RENT_CREDIT].key != &expected_credit
        || accounts[RENT_CREDIT].owner == &system_program::ID
        || accounts[RENT_CREDIT].key.to_bytes() != state.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != state.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || credit.generation() != state.identity.generation
    {
        return Err(CustodySbfError::Create.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_caller(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
    request_bytes: &[u8],
) -> ProgramResult {
    let digest = hash(request_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        state.identity.selected_release_set.to_bytes(),
        request.market(),
        ExecutionRoleV1::Core,
        request.context(),
        digest,
    )
    .map_err(|_| CustodySbfError::CallerAuthority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts[CORE_PROGRAM].key).0;
    if accounts[CALLER_AUTHORITY].key != &expected || program_id != accounts[CUSTODY_PROGRAM].key {
        return Err(CustodySbfError::CallerAuthority.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_replays(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
) -> Result<(CustodyReplayV1, [u8; 32]), ProgramError> {
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
    let source_key = Pubkey::find_program_address(&source_seeds.as_slices(), program_id).0;
    let core_key = Pubkey::find_program_address(&core_seeds.as_slices(), program_id).0;
    if accounts[TRADING_REPLAY].key != &source_key
        || accounts[TRADING_REPLAY].owner != program_id
        || accounts[TRADING_REPLAY].data_len() != CUSTODY_REPLAY_BYTES_V1
        || accounts[CORE_REPLAY].key != &core_key
        || accounts[CORE_REPLAY].owner != &system_program::ID
        || accounts[CORE_REPLAY].lamports() != 0
        || !accounts[CORE_REPLAY].data_is_empty()
    {
        return Err(CustodySbfError::Replay.into());
    }
    let source_data = accounts[TRADING_REPLAY]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let digest = hash(&source_data).to_bytes();
    let replay = CustodyReplayV1::decode(&source_data).map_err(|_| CustodySbfError::Replay)?;
    drop(source_data);
    if replay.release_set != release
        || replay.realm != state.identity.realm_id.to_bytes()
        || replay.rent_refund != accounts[RENT_CREDIT].key.to_bytes()
    {
        return Err(CustodySbfError::Replay.into());
    }
    Ok((replay, digest))
}

#[inline(never)]
fn authenticate_hoard(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    aggregate: LiabilityBasisMarketViewV2,
    request: RetirementReplayHandoffRequestV1,
) -> Result<[u8; 32], ProgramError> {
    let vault_seeds = CustodyVaultSeedsV1::new(
        request.market(),
        state.identity.selected_release_set.to_bytes(),
        request.context(),
        CompartmentV1::HoardPrincipal,
    );
    let expected_vault = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0;
    let authority_seeds = CustodyAuthoritySeedsV1::new(
        request.market(),
        state.identity.selected_release_set.to_bytes(),
    );
    let expected_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).0;
    let realm_data = accounts[REALM]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Realm)?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| CustodySbfError::Realm)?;
    drop(realm_data);
    let profile = collateral_profile(realm)?;
    let token = read_custody_account(
        &accounts[HOARD],
        &accounts[TOKEN_PROGRAM],
        &accounts[MINT],
        &accounts[CUSTODY_AUTHORITY],
        profile,
    )?;
    let data = accounts[HOARD]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let digest = hash(&data).to_bytes();
    if accounts[HOARD].key != &expected_vault
        || accounts[CUSTODY_AUTHORITY].key != &expected_authority
        || token.state != AccountState::Initialized
        || token.delegate != COption::None
        || token.native_reserve != COption::None
        || token.close_authority != COption::None
        || token.delegated_amount != 0
        || aggregate.custody_context != request.context()
        || digest != request.hoard_data_digest()
    {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(digest)
}

#[inline(never)]
fn create_core_replay(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    state: CoreState,
    request: RetirementReplayHandoffRequestV1,
) -> ProgramResult {
    let rent = Rent::from_account_info(&accounts[RENT]).map_err(|_| CustodySbfError::Create)?;
    let exact_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    if exact_rent != request.core_replay_rent_lamports() {
        return Err(CustodySbfError::Create.into());
    }
    let instruction = create_account(
        accounts[PAYER].key,
        accounts[CORE_REPLAY].key,
        exact_rent,
        u64::try_from(CUSTODY_REPLAY_BYTES_V1).map_err(|_| CustodySbfError::Create)?,
        program_id,
    );
    let seeds = CustodyReplaySeedsV1::new(
        request.market(),
        state.identity.selected_release_set.to_bytes(),
        ExecutionRoleV1::Core,
        request.context(),
    );
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, role, context] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            accounts[PAYER].clone(),
            accounts[CORE_REPLAY].clone(),
            accounts[SYSTEM].clone(),
        ],
        &[&[domain, market, release, role, context, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::Create.into())
}

#[inline(never)]
fn commit_core_replay(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    replay: CustodyReplayV1,
) -> ProgramResult {
    let bytes = replay.to_bytes().map_err(|_| CustodySbfError::Commit)?;
    let mut data = accounts[CORE_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

#[inline(never)]
fn close_trading_replay(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    request: RetirementReplayHandoffRequestV1,
) -> ProgramResult {
    let refund_after = accounts[RENT_CREDIT]
        .lamports()
        .checked_add(accounts[TRADING_REPLAY].lamports())
        .ok_or(CustodySbfError::Postcondition)?;
    {
        let mut data = accounts[TRADING_REPLAY]
            .try_borrow_mut_data()
            .map_err(|_| CustodySbfError::Commit)?;
        data.fill(0);
    }
    {
        let mut source = accounts[TRADING_REPLAY]
            .try_borrow_mut_lamports()
            .map_err(|_| CustodySbfError::Commit)?;
        let mut refund = accounts[RENT_CREDIT]
            .try_borrow_mut_lamports()
            .map_err(|_| CustodySbfError::Commit)?;
        **source = 0;
        **refund = refund_after;
    }
    accounts[TRADING_REPLAY]
        .resize(0)
        .map_err(|_| CustodySbfError::Commit)?;
    accounts[TRADING_REPLAY].assign(&system_program::ID);
    if request.trading_replay_lamports() == 0 {
        return Err(CustodySbfError::Postcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn verify_poststate(
    accounts: &[AccountInfo<'_>; RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
    request: RetirementReplayHandoffRequestV1,
    receipt: dclutch_custody::RetirementReplayHandoffReceiptV1,
    core_digest: [u8; 32],
) -> ProgramResult {
    let core_data = accounts[CORE_REPLAY]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Postcondition)?;
    let observed_core_digest = hash(&core_data).to_bytes();
    drop(core_data);
    let hoard_data = accounts[HOARD]
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Postcondition)?;
    let observed_hoard_digest = hash(&hoard_data).to_bytes();
    drop(hoard_data);
    if accounts[TRADING_REPLAY].owner != &system_program::ID
        || accounts[TRADING_REPLAY].lamports() != 0
        || !accounts[TRADING_REPLAY].data_is_empty()
        || accounts[CORE_REPLAY].owner != accounts[CUSTODY_PROGRAM].key
        || accounts[CORE_REPLAY].lamports() != request.core_replay_rent_lamports()
        || observed_core_digest != core_digest
        || observed_core_digest != receipt.core_replay_post_digest
        || accounts[HOARD].lamports() != request.hoard_lamports()
        || observed_hoard_digest != request.hoard_data_digest()
        || observed_hoard_digest != receipt.hoard_post_data_digest
        || accounts[RENT_CREDIT].lamports() != receipt.rent_credit_post_lamports
        || accounts[PAYER].lamports() != receipt.payer_post_lamports
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    Ok(())
}

#[cfg(test)]
mod admissible_prestates {
    use dclutch_market::{Phase, Readiness};

    use super::CUSTODY_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1;

    /// The constant against the exact inline condition it replaced.
    ///
    /// `state.phase != Phase::Retiring` named no readiness, so every readiness
    /// in `Retiring` was admitted and the declaration says so. A rename that
    /// narrowed it to one readiness would compile, pass the program test whose
    /// fixture only ever reaches that readiness, and fail here.
    #[test]
    fn the_constant_admits_what_the_inline_guard_admitted() {
        for phase in [
            Phase::Founding,
            Phase::Open,
            Phase::Terminal,
            Phase::Retiring,
            Phase::Retired,
        ] {
            assert_eq!(
                CUSTODY_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1.admits_phase(phase),
                !(phase != Phase::Retiring),
                "custody handoff admission disagrees at {phase:?}"
            );
            for readiness in [Readiness::Prepaid, Readiness::Ready, Readiness::Consumed] {
                assert_eq!(
                    CUSTODY_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1
                        .admits(phase, readiness),
                    phase == Phase::Retiring,
                    "custody handoff admission disagrees at {phase:?}/{readiness:?}"
                );
            }
        }
    }
}
