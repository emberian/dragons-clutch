#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Thin SVM refinement of the canonical multiprogram Custody contract.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryFrom;

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1, ReceiptEvidenceV1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1,
};
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{
    AuthorityRole, COption, ExactTransferInput, ExactTransferProfileV1,
    PRODUCTION_ADAPTER_RELEASES, close_account, initialize_account3, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::create_account;

/// Exact common prefix length.
pub const COMMON_ACCOUNT_COUNT_V1: usize = 7;
/// Exact `InitializeReplay` account count.
pub const INITIALIZE_REPLAY_ACCOUNT_COUNT_V1: usize = 10;
/// Exact `OpenVault` account count.
pub const OPEN_VAULT_ACCOUNT_COUNT_V1: usize = 14;
/// Exact `Transfer` account count.
pub const TRANSFER_ACCOUNT_COUNT_V1: usize = 12;
/// Exact `CloseVault` account count.
pub const CLOSE_VAULT_ACCOUNT_COUNT_V1: usize = 12;

const CALLER_AUTHORITY: usize = 0;
const ACTIVATION_CACHE: usize = 1;
const REGISTRY_PROGRAM: usize = 2;
const CALLER_PROGRAM: usize = 3;
const CALLER_PROGRAMDATA: usize = 4;
const REALM: usize = 5;
const REPLAY: usize = 6;

/// Stable refusal from the thin Custody SBF adapter.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodySbfError {
    /// Instruction bytes did not decode as the one generated request.
    Instruction = 0,
    /// Account count, order, privileges, or aliases were not exact.
    AccountFrame = 1,
    /// Registry CPI, producer, receipt, release, role, or caller refused.
    Release = 2,
    /// Caller authority was not the release-pinned role PDA signer.
    CallerAuthority = 3,
    /// Realm content, PDA, owner, Mint, token program, or adapter release refused.
    Realm = 4,
    /// Replay PDA, owner, bytes, or revision refused.
    Replay = 5,
    /// Vault PDA, token state, or authority policy refused.
    TokenState = 6,
    /// Rent, payer, System program, or account creation refused.
    Create = 7,
    /// Exact token or close-account CPI refused.
    TokenCpi = 8,
    /// Exact CPI postcondition or checked balance arithmetic refused.
    Postcondition = 9,
    /// Replay state could not be committed after all effects succeeded.
    Commit = 10,
}

impl From<CustodySbfError> for ProgramError {
    fn from(value: CustodySbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Execute one exact replay, vault, transfer, or close effect.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request =
        CustodyRequestV1::decode(instruction_data).map_err(|_| CustodySbfError::Instruction)?;
    require_account_count(accounts, request.operation)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_common_frame(program_id, accounts, request, request_digest)?;
    let realm = authenticate_realm(program_id, accounts, request)?;
    match request.operation {
        OperationV1::InitializeReplay => {
            initialize_replay(program_id, accounts, request, request_digest)
        }
        OperationV1::OpenVault => open_vault(program_id, accounts, request, request_digest, realm),
        OperationV1::Transfer => {
            execute_transfer(program_id, accounts, request, request_digest, realm)
        }
        OperationV1::CloseVault => {
            close_vault(program_id, accounts, request, request_digest, realm)
        }
    }
}

#[inline(never)]
fn authenticate_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let caller_authority = account(accounts, CALLER_AUTHORITY)?;
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let caller_programdata = account(accounts, CALLER_PROGRAMDATA)?;
    let realm = account(accounts, REALM)?;
    let replay = account(accounts, REPLAY)?;

    if !caller_authority.is_signer
        || caller_authority.is_writable
        || caller_authority.executable
        || cache.is_signer
        || cache.is_writable
        || cache.executable
        || registry.is_signer
        || registry.is_writable
        || !registry.executable
        || caller_program.is_signer
        || caller_program.is_writable
        || !caller_program.executable
        || caller_programdata.is_signer
        || caller_programdata.is_writable
        || caller_programdata.executable
        || realm.is_signer
        || realm.is_writable
        || realm.executable
        || replay.is_signer
        || !replay.is_writable
        || replay.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if caller_program.key.to_bytes() != request.caller_program {
        return Err(CustodySbfError::Release.into());
    }
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| CustodySbfError::Release)?,
        request.market,
        registry_role(request.caller_role),
        request.context,
        request_digest,
    )
    .map_err(|_| CustodySbfError::CallerAuthority)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), caller_program.key).0;
    if caller_authority.key != &expected_caller {
        return Err(CustodySbfError::CallerAuthority.into());
    }
    authenticate_calling_release(accounts, request)?;
    authenticate_replay_identity(program_id, replay, request)?;
    Ok(())
}

#[inline(never)]
fn authenticate_calling_release(
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
) -> ProgramResult {
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let caller_programdata = account(accounts, CALLER_PROGRAMDATA)?;
    let role = registry_role(request.caller_role);
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*caller_program.key, false),
            AccountMeta::new_readonly(*caller_programdata.key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(role)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            caller_program.clone(),
            caller_programdata.clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| CustodySbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(CustodySbfError::Release)?;
    if producer != *registry.key {
        return Err(CustodySbfError::Release.into());
    }
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| CustodySbfError::Release)?;
    if receipt.role() != role
        || receipt.execution_release_set_id().as_bytes() != &request.release_set
        || receipt.program().as_bytes() != &request.caller_program
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    profile: ExactTransferProfileV1,
}

#[inline(never)]
fn authenticate_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
) -> Result<RealmFacts, ProgramError> {
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let realm_account = account(accounts, REALM)?;
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| CustodySbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| CustodySbfError::Release)?
        .as_bytes()
        != &request.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
    {
        return Err(CustodySbfError::Release.into());
    }
    let core_program = Pubkey::new_from_array(
        *activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes(),
    );
    drop(cache_data);

    if realm_account.owner != &core_program {
        return Err(CustodySbfError::Realm.into());
    }
    let realm_data = realm_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Realm)?;
    let realm_digest = hash(&realm_data).to_bytes();
    let expected_realm =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &core_program).0;
    if realm_account.key != &expected_realm || realm_digest != request.realm {
        return Err(CustodySbfError::Realm.into());
    }
    let realm = RealmV1::decode(&realm_data).map_err(|_| CustodySbfError::Realm)?;
    let profile = collateral_profile(realm)?;
    if matches!(
        request.operation,
        OperationV1::OpenVault | OperationV1::Transfer | OperationV1::CloseVault
    ) && (request.mint != *realm.collateral_mint()
        || request.token_program != *realm.token_program()
        || request.token_program != profile.program_id())
    {
        return Err(CustodySbfError::Realm.into());
    }
    Ok(RealmFacts { realm, profile })
}

fn collateral_profile(realm: RealmV1) -> Result<ExactTransferProfileV1, ProgramError> {
    for release in PRODUCTION_ADAPTER_RELEASES {
        if hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id() {
            return Ok(release.profile());
        }
    }
    Err(CustodySbfError::Realm.into())
}

fn authenticate_replay_identity(
    program_id: &Pubkey,
    replay: &AccountInfo<'_>,
    request: CustodyRequestV1,
) -> ProgramResult {
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    let expected = Pubkey::find_program_address(&replay_seeds.as_slices(), program_id).0;
    if replay.key != &expected {
        return Err(CustodySbfError::Replay.into());
    }
    match request.operation {
        OperationV1::InitializeReplay => {
            if replay.owner != &system_program::ID
                || replay.lamports() != 0
                || replay.data_len() != 0
            {
                return Err(CustodySbfError::Replay.into());
            }
        }
        OperationV1::OpenVault | OperationV1::Transfer | OperationV1::CloseVault => {
            if replay.owner != program_id || replay.data_len() != CUSTODY_REPLAY_BYTES_V1 {
                return Err(CustodySbfError::Replay.into());
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn initialize_replay(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let replay = account(accounts, REPLAY)?;
    let payer = account(accounts, 7)?;
    let system = account(accounts, 8)?;
    let rent_account = account(accounts, 9)?;
    if !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || system.key != &system_program::ID
        || !system.executable
        || system.is_signer
        || system.is_writable
        || rent_account.key != &sysvar::rent::ID
        || rent_account.is_signer
        || rent_account.is_writable
        || rent_account.executable
        || payer.key.to_bytes() != request.payer
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let exact_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    if exact_rent != request.rent_lamports {
        return Err(CustodySbfError::Create.into());
    }
    let instruction = create_account(
        payer.key,
        replay.key,
        exact_rent,
        u64::try_from(CUSTODY_REPLAY_BYTES_V1).map_err(|_| CustodySbfError::Create)?,
        program_id,
    );
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    let bump = Pubkey::find_program_address(&replay_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, context] = replay_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[payer.clone(), replay.clone(), system.clone()],
        &[&[domain, market, release, context, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::Create)?;
    if replay.owner != program_id
        || replay.data_len() != CUSTODY_REPLAY_BYTES_V1
        || replay.lamports() != exact_rent
    {
        return Err(CustodySbfError::Create.into());
    }
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: replay.key.to_bytes(),
        destination: replay.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: exact_rent,
    });
    let replay_state = CustodyReplayV1::initialize(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        replay,
        request,
        request_digest,
        replay_state,
        zero_evidence(poststate),
    )
}

#[inline(never)]
fn open_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let mint = account(accounts, 7)?;
    let vault = account(accounts, 8)?;
    let authority = account(accounts, 9)?;
    let token_program = account(accounts, 10)?;
    let payer = account(accounts, 11)?;
    let system = account(accounts, 12)?;
    let rent_account = account(accounts, 13)?;
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    validate_custody_authority(program_id, authority, request)?;
    validate_vault_key(program_id, vault, request, false)?;
    if vault.owner != &system_program::ID
        || vault.lamports() != 0
        || vault.data_len() != 0
        || !vault.is_writable
        || vault.is_signer
        || !payer.is_signer
        || !payer.is_writable
        || payer.key.to_bytes() != request.payer
        || system.key != &system_program::ID
        || !system.executable
        || rent_account.key != &sysvar::rent::ID
        || rent_account.is_signer
        || rent_account.is_writable
        || rent_account.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let exact_rent = rent.minimum_balance(dclutch_token_svm::ACCOUNT_BYTES);
    if request.rent_lamports != exact_rent {
        return Err(CustodySbfError::Create.into());
    }
    create_vault(
        program_id,
        payer,
        vault,
        system,
        token_program,
        request,
        exact_rent,
    )?;
    initialize_vault(vault, mint, authority, token_program, request)?;
    let token = read_custody_account(vault, token_program, mint, authority, realm.profile)?;
    if token.amount != 0 || vault.lamports() != exact_rent {
        return Err(CustodySbfError::Postcondition.into());
    }
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: vault.key.to_bytes(),
        destination: vault.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: exact_rent,
    });
    let next = replay
        .advance(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        zero_evidence(poststate),
    )
}

#[inline(never)]
fn execute_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let mint = account(accounts, 7)?;
    let source = account(accounts, 8)?;
    let destination = account(accounts, 9)?;
    let authority = account(accounts, 10)?;
    let token_program = account(accounts, 11)?;
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    validate_custody_authority(program_id, authority, request)?;
    if !source.is_writable
        || source.is_signer
        || source.executable
        || !destination.is_writable
        || destination.is_signer
        || destination.executable
        || source.key.to_bytes() != request.source
        || destination.key.to_bytes() != request.destination
        || source.owner != token_program.key
        || destination.owner != token_program.key
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if request.source_compartment != CompartmentV1::External {
        validate_vault_key(program_id, source, request, true)?;
    }
    if request.destination_compartment != CompartmentV1::External {
        validate_vault_key(program_id, destination, request, false)?;
    }
    let transfer_accounts = TransferAccounts {
        source,
        destination,
        mint,
        authority,
        token_program,
    };
    let before = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, true)?;
    invoke_exact_transfer(transfer_accounts, request, before.decimals, program_id)?;
    let after = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, false)?;
    if before
        .source
        .checked_sub(request.amount)
        .ok_or(CustodySbfError::Postcondition)?
        != after.source
        || before
            .destination
            .checked_add(request.amount)
            .ok_or(CustodySbfError::Postcondition)?
            != after.destination
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let evidence = ReceiptEvidenceV1 {
        source_before: before.source,
        source_after: after.source,
        destination_before: before.destination,
        destination_after: after.destination,
        poststate_commitment: poststate_commitment(PoststateProjection {
            request_digest,
            source: source.key.to_bytes(),
            destination: destination.key.to_bytes(),
            source_before: before.source,
            source_after: after.source,
            destination_before: before.destination,
            destination_after: after.destination,
            rent_lamports: 0,
        }),
        replay_state_digest: [0; 32],
    };
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let next = replay
        .advance(request, request_digest, evidence.poststate_commitment)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        evidence,
    )
}

#[inline(never)]
fn close_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let mint = account(accounts, 7)?;
    let vault = account(accounts, 8)?;
    let authority = account(accounts, 9)?;
    let token_program = account(accounts, 10)?;
    let rent_refund = account(accounts, 11)?;
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    validate_custody_authority(program_id, authority, request)?;
    validate_vault_key(program_id, vault, request, true)?;
    if !vault.is_writable
        || vault.is_signer
        || vault.executable
        || !rent_refund.is_writable
        || rent_refund.executable
        || rent_refund.key.to_bytes() != request.rent_refund
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let token = read_custody_account(vault, token_program, mint, authority, realm.profile)?;
    let vault_lamports = vault.lamports();
    let refund_before = rent_refund.lamports();
    if token.amount != 0 || vault_lamports != request.rent_lamports {
        return Err(CustodySbfError::TokenState.into());
    }
    refund_before
        .checked_add(vault_lamports)
        .ok_or(CustodySbfError::Postcondition)?;
    invoke_close(
        vault,
        rent_refund,
        authority,
        token_program,
        request,
        program_id,
    )?;
    if vault.lamports() != 0
        || rent_refund.lamports()
            != refund_before
                .checked_add(vault_lamports)
                .ok_or(CustodySbfError::Postcondition)?
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: vault.key.to_bytes(),
        destination: rent_refund.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: vault_lamports,
    });
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let next = replay
        .advance(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        zero_evidence(poststate),
    )
}

fn require_account_count(accounts: &[AccountInfo<'_>], operation: OperationV1) -> ProgramResult {
    let expected = match operation {
        OperationV1::InitializeReplay => INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
        OperationV1::OpenVault => OPEN_VAULT_ACCOUNT_COUNT_V1,
        OperationV1::Transfer => TRANSFER_ACCOUNT_COUNT_V1,
        OperationV1::CloseVault => CLOSE_VAULT_ACCOUNT_COUNT_V1,
    };
    if accounts.len() != expected {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn registry_role(role: CallerRoleV1) -> ExecutionRoleV1 {
    role
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| CustodySbfError::AccountFrame.into())
}

fn validate_custody_authority(
    program_id: &Pubkey,
    authority: &AccountInfo<'_>,
    request: CustodyRequestV1,
) -> ProgramResult {
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let expected = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).0;
    if authority.key != &expected
        || authority.is_signer
        || authority.is_writable
        || authority.executable
    {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

fn validate_vault_key(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    request: CustodyRequestV1,
    source: bool,
) -> ProgramResult {
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, source);
    let expected = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0;
    if vault.key != &expected {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

fn validate_token_program_and_mint(
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    request: CustodyRequestV1,
    facts: RealmFacts,
) -> ProgramResult {
    if token_program.key.to_bytes() != request.token_program
        || !token_program.executable
        || token_program.is_signer
        || token_program.is_writable
        || mint.key.to_bytes() != request.mint
        || mint.owner != token_program.key
        || mint.is_signer
        || mint.is_writable
        || mint.executable
    {
        return Err(CustodySbfError::TokenState.into());
    }
    let data = mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_state = facts
        .profile
        .check_mint(request.token_program, &data)
        .map_err(|_| CustodySbfError::TokenState)?;
    if (facts.realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && !matches!(mint_state.mint_authority, COption::None))
        || (facts.realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && !matches!(mint_state.freeze_authority, COption::None))
    {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TransferBalances {
    source: u64,
    destination: u64,
    decimals: u8,
}

#[derive(Clone, Copy)]
struct TransferAccounts<'a, 'info> {
    source: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    authority: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
}

fn authenticate_transfer_accounts(
    accounts: TransferAccounts<'_, '_>,
    request: CustodyRequestV1,
    profile: ExactTransferProfileV1,
    require_authority: bool,
) -> Result<TransferBalances, ProgramError> {
    let mint_data = accounts
        .mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_data = accounts
        .source
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let destination_data = accounts
        .destination
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_state = profile
        .check_mint(request.token_program, &mint_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_state = profile
        .check_transfer_account(request.token_program, &source_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let destination_state = profile
        .check_transfer_account(request.token_program, &destination_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    if source_state.mint != request.mint || destination_state.mint != request.mint {
        return Err(CustodySbfError::TokenState.into());
    }
    let authority_role = if require_authority {
        Some(
            profile
                .check_transfer(ExactTransferInput {
                    program_id: request.token_program,
                    mint_address: request.mint,
                    mint_data: &mint_data,
                    source_data: &source_data,
                    destination_data: &destination_data,
                    authority: accounts.authority.key.to_bytes(),
                    amount: request.amount,
                    decimals: mint_state.decimals,
                })
                .map_err(|_| CustodySbfError::TokenState)?
                .authority_role(),
        )
    } else {
        None
    };
    if request.source_compartment == CompartmentV1::External {
        if (require_authority && authority_role != Some(AuthorityRole::Delegate))
            || source_state.owner == accounts.authority.key.to_bytes()
            || source_state.owner != request.semantic.actor
        {
            return Err(CustodySbfError::TokenState.into());
        }
    } else {
        profile
            .check_custody_account(
                request.token_program,
                &source_data,
                request.mint,
                accounts.authority.key.to_bytes(),
            )
            .map_err(|_| CustodySbfError::TokenState)?;
    }
    if request.destination_compartment == CompartmentV1::External {
        if destination_state.owner == accounts.authority.key.to_bytes()
            || destination_state.owner != request.semantic.actor
        {
            return Err(CustodySbfError::TokenState.into());
        }
    } else {
        profile
            .check_custody_account(
                request.token_program,
                &destination_data,
                request.mint,
                accounts.authority.key.to_bytes(),
            )
            .map_err(|_| CustodySbfError::TokenState)?;
    }
    Ok(TransferBalances {
        source: source_state.amount,
        destination: destination_state.amount,
        decimals: mint_state.decimals,
    })
}

fn read_custody_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    profile: ExactTransferProfileV1,
) -> Result<dclutch_token_svm::TokenAccount, ProgramError> {
    if account.owner != token_program.key {
        return Err(CustodySbfError::TokenState.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    profile
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            authority.key.to_bytes(),
        )
        .map_err(|_| CustodySbfError::TokenState.into())
}

fn create_vault<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
    rent_lamports: u64,
) -> ProgramResult {
    let instruction = create_account(
        payer.key,
        vault.key,
        rent_lamports,
        u64::try_from(dclutch_token_svm::ACCOUNT_BYTES).map_err(|_| CustodySbfError::Create)?,
        token_program.key,
    );
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, false);
    let bump = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, context, compartment] = vault_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[payer.clone(), vault.clone(), system.clone()],
        &[&[domain, market, release, context, compartment, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::Create.into())
}

fn initialize_vault<'a>(
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
) -> ProgramResult {
    let specification = initialize_account3(
        request.token_program,
        request.destination,
        request.mint,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    invoke(
        &instruction,
        &[vault.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn invoke_exact_transfer(
    accounts: TransferAccounts<'_, '_>,
    request: CustodyRequestV1,
    decimals: u8,
    program_id: &Pubkey,
) -> ProgramResult {
    let specification = transfer_checked(
        request.token_program,
        request.source,
        request.mint,
        request.destination,
        accounts.authority.key.to_bytes(),
        request.amount,
        decimals,
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let bump = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            accounts.source.clone(),
            accounts.mint.clone(),
            accounts.destination.clone(),
            accounts.authority.clone(),
            accounts.token_program.clone(),
        ],
        &[&[domain, market, release, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn invoke_close<'a>(
    vault: &AccountInfo<'a>,
    rent_refund: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
    program_id: &Pubkey,
) -> ProgramResult {
    let specification = close_account(
        request.token_program,
        request.source,
        request.rent_refund,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let bump = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            vault.clone(),
            rent_refund.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[domain, market, release, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn token_instruction<const ACCOUNTS: usize, const DATA: usize>(
    specification: &dclutch_token_svm::InstructionSpec<ACCOUNTS, DATA>,
) -> Instruction {
    let mut accounts = Vec::with_capacity(ACCOUNTS);
    for role in specification.accounts() {
        let address = Pubkey::new_from_array(*role.address());
        accounts.push(if role.is_writable() {
            AccountMeta::new(address, role.is_signer())
        } else {
            AccountMeta::new_readonly(address, role.is_signer())
        });
    }
    Instruction {
        program_id: Pubkey::new_from_array(*specification.program_id()),
        accounts,
        data: specification.data().to_vec(),
    }
}

fn read_replay(replay: &AccountInfo<'_>) -> Result<CustodyReplayV1, ProgramError> {
    let data = replay
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    CustodyReplayV1::decode(&data).map_err(|_| CustodySbfError::Replay.into())
}

fn zero_evidence(poststate_commitment: [u8; 32]) -> ReceiptEvidenceV1 {
    ReceiptEvidenceV1 {
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        poststate_commitment,
        replay_state_digest: [0; 32],
    }
}

#[inline(never)]
fn commit_replay_and_receipt(
    replay: &AccountInfo<'_>,
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    replay_state: CustodyReplayV1,
    mut evidence: ReceiptEvidenceV1,
) -> ProgramResult {
    let replay_bytes = replay_state
        .to_bytes()
        .map_err(|_| CustodySbfError::Replay)?;
    evidence.replay_state_digest = hash(&replay_bytes).to_bytes();
    let receipt = CustodyReceiptV1::new(request, request_digest, evidence)
        .map_err(|_| CustodySbfError::Postcondition)?;
    let receipt_bytes = receipt
        .to_bytes()
        .map_err(|_| CustodySbfError::Postcondition)?;
    if receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(CustodySbfError::Postcondition.into());
    }
    let mut data = replay
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(&replay_bytes);
    drop(data);
    set_return_data(&receipt_bytes);
    Ok(())
}

#[derive(Clone, Copy)]
struct PoststateProjection {
    request_digest: [u8; 32],
    source: [u8; 32],
    destination: [u8; 32],
    source_before: u64,
    source_after: u64,
    destination_before: u64,
    destination_after: u64,
    rent_lamports: u64,
}

fn poststate_commitment(projection: PoststateProjection) -> [u8; 32] {
    hashv(&[
        dclutch_custody_contract::CUSTODY_POSTSTATE_DOMAIN_V1,
        &projection.request_digest,
        &projection.source,
        &projection.destination,
        &projection.source_before.to_le_bytes(),
        &projection.source_after.to_le_bytes(),
        &projection.destination_before.to_le_bytes(),
        &projection.destination_after.to_le_bytes(),
        &projection.rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_counts_are_operation_specific() {
        assert_eq!(INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, 10);
        assert_eq!(OPEN_VAULT_ACCOUNT_COUNT_V1, 14);
        assert_eq!(TRANSFER_ACCOUNT_COUNT_V1, 12);
        assert_eq!(CLOSE_VAULT_ACCOUNT_COUNT_V1, 12);
    }

    #[test]
    fn role_mapping_never_lends_custody_to_itself() {
        assert_eq!(registry_role(CallerRoleV1::Core), ExecutionRoleV1::Core);
        assert_eq!(registry_role(CallerRoleV1::Claims), ExecutionRoleV1::Claims);
        assert_eq!(
            registry_role(CallerRoleV1::Trading),
            ExecutionRoleV1::Trading
        );
        assert_eq!(
            registry_role(CallerRoleV1::Resolution),
            ExecutionRoleV1::Resolution
        );
    }

    #[test]
    fn both_realm_profiles_are_pinned_by_release_preimage_digest() {
        for release in PRODUCTION_ADAPTER_RELEASES {
            let encoded = release.to_bytes();
            assert_ne!(hash(&encoded).to_bytes(), [0; 32]);
            assert_eq!(
                dclutch_token_svm::CollateralAdapterReleaseV1::decode(&encoded),
                Ok(release)
            );
        }
    }
}
