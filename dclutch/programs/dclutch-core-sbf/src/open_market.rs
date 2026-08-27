//! Canonical Custody replay/vault creation and commit-last Market opening.

use alloc::{boxed::Box, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CUSTODY_REQUEST_BYTES_V1, CallerRoleV1,
    CompartmentV1, CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{
    Action, ChildEffectObservation, CollateralObservation, CoreState, Realm, Request, Role,
    VacantAccount, open_market,
};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationRequestV1,
};
use dclutch_release_set_contract::CallerAuthoritySeedsV1;
use dclutch_token_svm::PRODUCTION_ADAPTER_RELEASES;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
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
    fixed_role::{FixedRoleAccountsV1, authenticate_market, persist_state, read_market_bytes},
    frame::require_distinct,
    records::authenticate_finalized_record,
    release::{RoleDeploymentAccounts, authenticate_continuation_roles, identity},
};

/// Exact top-level instruction width for one canonical Custody creation effect.
pub const OPEN_MARKET_INSTRUCTION_BYTES_V1: usize =
    dclutch_market_core_codec::REQUEST_BYTES + CUSTODY_REQUEST_BYTES_V1;
/// Exact outer count for prerequisite replay initialization.
pub const INITIALIZE_REPLAY_OUTER_ACCOUNT_COUNT_V1: usize = 15;
/// Exact outer count for vault creation and Market opening.
pub const OPEN_MARKET_OUTER_ACCOUNT_COUNT_V1: usize = 19;

const REALM_RAW: usize = 8;
const REALM_STAGING: usize = 9;
const REPLAY: usize = 10;
const INITIALIZE_PAYER: usize = 11;
const INITIALIZE_SYSTEM: usize = 12;
const INITIALIZE_RENT: usize = 13;
const OPEN_MINT: usize = 11;
const OPEN_VAULT: usize = 12;
const OPEN_AUTHORITY: usize = 13;
const OPEN_TOKEN_PROGRAM: usize = 14;
const OPEN_PAYER: usize = 15;
const OPEN_SYSTEM: usize = 16;
const OPEN_RENT: usize = 17;

struct AuthenticatedOpenV1 {
    state: Box<CoreState>,
    state_bytes: Box<[u8; dclutch_market_core_codec::STATE_BYTES]>,
    custody_admission: Box<dclutch_market_core_codec::Admission>,
    realm: RealmV1,
    rent: Rent,
    continuation: RegistryContinuationRequestV1,
}

/// Execute replay initialization or the exact Custody effect that opens a Market.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    request_bytes: &[u8],
    custody_bytes: &[u8],
) -> Result<(), ProgramError> {
    if request.action != Action::OpenMarket
        || custody_bytes.len() != CUSTODY_REQUEST_BYTES_V1
        || request_bytes.len() != dclutch_market_core_codec::REQUEST_BYTES
    {
        return Err(CoreSbfError::Instruction.into());
    }
    let custody = CustodyRequestV1::decode(custody_bytes).map_err(|_| CoreSbfError::Instruction)?;
    if !matches!(
        custody.operation,
        OperationV1::InitializeReplay | OperationV1::OpenVault
    ) {
        return Err(CoreSbfError::Instruction.into());
    }
    validate_outer_frame(program_id, accounts, custody.operation)?;
    let frame = FixedRoleAccountsV1::parse(program_id, accounts)?;
    let authenticated = authenticate_open(
        program_id,
        accounts,
        &frame,
        request,
        request_bytes,
        custody,
        custody_bytes,
    )?;
    let pre = authenticate_prestate(accounts, &frame, authenticated.state.as_ref(), custody)?;
    invoke_custody(
        program_id,
        accounts,
        &frame,
        custody,
        custody_bytes,
        authenticated.continuation,
    )?;
    let receipt = authenticate_receipt_and_poststate(
        accounts,
        &frame,
        authenticated.state.as_ref(),
        authenticated.realm,
        authenticated.rent,
        custody,
        custody_bytes,
        pre,
    )?;
    require_market_unchanged(&frame, authenticated.state_bytes.as_ref())?;

    if custody.operation == OperationV1::OpenVault {
        let mut candidate = *authenticated.state;
        let creation = open_market(
            request,
            &mut candidate,
            *authenticated.custody_admission,
            semantic_realm(authenticated.state.as_ref(), authenticated.realm)?,
            true,
            true,
            collateral_observation(authenticated.state.as_ref(), authenticated.realm)?,
            pre.vacant.ok_or(CoreSbfError::Creation)?,
            custody.rent_lamports,
            true,
            complete_child_effect(),
        )
        .map_err(|_| CoreSbfError::Transition)?;
        if creation.after != account(accounts, OPEN_VAULT)?.lamports()
            || receipt.rent_lamports != creation.rent_minimum
        {
            return Err(CoreSbfError::ChildAck.into());
        }
        persist_state(frame.market(), candidate)?;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_open<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    frame: &FixedRoleAccountsV1<'accounts, 'info>,
    request: Request,
    request_bytes: &[u8],
    custody: CustodyRequestV1,
    custody_bytes: &[u8],
) -> Result<AuthenticatedOpenV1, CoreSbfError> {
    let state_bytes = Box::new(read_market_bytes(program_id, frame.market())?);
    let state =
        Box::new(CoreState::decode(state_bytes.as_ref()).map_err(|_| CoreSbfError::Market)?);
    authenticate_market(program_id, frame.market(), *state, request)?;
    if state.phase != dclutch_market_core_codec::Phase::Founding
        || state.readiness != dclutch_market_core_codec::Readiness::Ready
    {
        return Err(CoreSbfError::Transition);
    }
    let continuation_digest = ContentId::new(hashv(&[request_bytes, custody_bytes]).to_bytes())
        .map_err(|_| CoreSbfError::Release)?;
    let continuation_len =
        u32::try_from(OPEN_MARKET_INSTRUCTION_BYTES_V1).map_err(|_| CoreSbfError::Arithmetic)?;
    let admission = accounts.last().ok_or(CoreSbfError::AccountFrame)?;
    let (admissions, continuation) = authenticate_continuation_roles(
        frame.cache(),
        frame.registry(),
        admission,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        &[
            RoleDeploymentAccounts::new(Role::Core, frame.core_program(), frame.core_programdata()),
            RoleDeploymentAccounts::new(
                Role::Custody,
                frame.target_program(),
                frame.target_programdata(),
            ),
        ],
        continuation_digest,
        continuation_len,
    )?;
    let core_admission = Box::new(admissions.admission(Role::Core)?);
    let custody_admission = Box::new(admissions.admission(Role::Custody)?);
    if core_admission.selected != custody_admission.selected {
        return Err(CoreSbfError::Release);
    }
    let expected_parent = hash(request_bytes).to_bytes();
    let expected_beneficiary = state.rent_beneficiary.to_bytes();
    let payer = account(accounts, payer_index(custody.operation))?;
    let inactive_semantic = custody.semantic.candidate == [0; 32]
        && custody.semantic.source_owner == [0; 32]
        && custody.semantic.destination_owner == [0; 32]
        && custody.semantic.order == [0; 32]
        && custody.semantic.order_nonce == 0
        && custody.semantic.page_index == 0
        && custody.semantic.execution_index == 0
        && custody.semantic.transfer_index == 0;
    if custody.caller_role != CallerRoleV1::Core
        || custody.caller_program != program_id.to_bytes()
        || custody.release_set != state.identity.selected_release_set.to_bytes()
        || custody.market != frame.market().key.to_bytes()
        || custody.realm != state.identity.realm_id.to_bytes()
        || custody.context != frame.market().key.to_bytes()
        || custody.semantic.parent_request_digest != expected_parent
        || custody.semantic.generation != state.identity.generation
        || custody.payer != payer.key.to_bytes()
        || custody.rent_refund != expected_beneficiary
        || !inactive_semantic
    {
        return Err(CoreSbfError::Reference);
    }
    authenticate_caller_authority(program_id, frame, custody, custody_bytes)?;
    let rent = read_rent(account(accounts, rent_index(custody.operation))?)?;
    let realm_data = account(accounts, REALM_RAW)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if realm_data.len() != REALM_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        frame.registry().key,
        account(accounts, REALM_RAW)?,
        account(accounts, REALM_STAGING)?,
        &rent,
        REALM_SCHEMA_RELEASE_ID_V1,
        state.identity.realm_id.to_bytes(),
        &realm_data,
    )?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| CoreSbfError::Reference)?;
    authenticate_request_shape(accounts, frame, state.as_ref(), realm, &rent, custody)?;
    Ok(AuthenticatedOpenV1 {
        state,
        state_bytes,
        custody_admission,
        realm,
        rent,
        continuation,
    })
}

fn authenticate_caller_authority(
    program_id: &Pubkey,
    frame: &FixedRoleAccountsV1<'_, '_>,
    custody: CustodyRequestV1,
    custody_bytes: &[u8],
) -> Result<(), CoreSbfError> {
    let digest = hash(custody_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| CoreSbfError::CallerAuthority)?,
        custody.market,
        CallerRoleV1::Core,
        custody.context,
        digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if frame.authority().key != &expected {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

fn authenticate_request_shape(
    accounts: &[AccountInfo<'_>],
    frame: &FixedRoleAccountsV1<'_, '_>,
    state: &CoreState,
    realm: RealmV1,
    rent: &Rent,
    request: CustodyRequestV1,
) -> Result<(), CoreSbfError> {
    let expected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        frame.target_program().key,
    )
    .0;
    if account(accounts, REPLAY)?.key != &expected_replay {
        return Err(CoreSbfError::Reference);
    }
    match request.operation {
        OperationV1::InitializeReplay => {
            if request.source_compartment != CompartmentV1::None
                || request.destination_compartment != CompartmentV1::None
                || request.source != [0; 32]
                || request.destination != [0; 32]
                || request.source_vault_context != [0; 32]
                || request.destination_vault_context != [0; 32]
                || request.mint != [0; 32]
                || request.token_program != [0; 32]
                || request.expected_revision != 0
                || request.resulting_revision != 1
                || request.amount != 0
                || request.rent_lamports != rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1)
            {
                return Err(CoreSbfError::Reference);
            }
        }
        OperationV1::OpenVault => {
            let expected_authority = Pubkey::find_program_address(
                &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
                frame.target_program().key,
            )
            .0;
            let expected_vault = Pubkey::find_program_address(
                &CustodyVaultSeedsV1::from_request(request, false).as_slices(),
                frame.target_program().key,
            )
            .0;
            if request.source_compartment != CompartmentV1::None
                || request.destination_compartment != CompartmentV1::HoardPrincipal
                || request.source != [0; 32]
                || request.destination != expected_vault.to_bytes()
                || request.source_vault_context != [0; 32]
                || request.destination_vault_context != frame.market().key.to_bytes()
                || request.mint != *realm.collateral_mint()
                || request.token_program != *realm.token_program()
                || request.expected_revision != 1
                || request.resulting_revision != 2
                || request.amount != 0
                || request.rent_lamports != rent.minimum_balance(dclutch_token_svm::ACCOUNT_BYTES)
                || account(accounts, OPEN_MINT)?.key.to_bytes() != request.mint
                || account(accounts, OPEN_VAULT)?.key != &expected_vault
                || account(accounts, OPEN_AUTHORITY)?.key != &expected_authority
                || account(accounts, OPEN_TOKEN_PROGRAM)?.key.to_bytes() != request.token_program
                || request.realm != state.identity.realm_id.to_bytes()
            {
                return Err(CoreSbfError::Reference);
            }
        }
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(CoreSbfError::Instruction);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OpenPrestate {
    payer_lamports: u64,
    vacant: Option<VacantAccount>,
}

fn authenticate_prestate(
    accounts: &[AccountInfo<'_>],
    frame: &FixedRoleAccountsV1<'_, '_>,
    state: &CoreState,
    request: CustodyRequestV1,
) -> Result<OpenPrestate, CoreSbfError> {
    let replay = account(accounts, REPLAY)?;
    match request.operation {
        OperationV1::InitializeReplay => {
            if replay.owner != &system_program::ID
                || replay.lamports() != 0
                || replay.data_len() != 0
            {
                return Err(CoreSbfError::Creation);
            }
            Ok(OpenPrestate {
                payer_lamports: account(accounts, INITIALIZE_PAYER)?.lamports(),
                vacant: None,
            })
        }
        OperationV1::OpenVault => {
            if replay.owner != frame.target_program().key
                || replay.data_len() != CUSTODY_REPLAY_BYTES_V1
            {
                return Err(CoreSbfError::Reference);
            }
            let replay_data = replay
                .try_borrow_data()
                .map_err(|_| CoreSbfError::Reference)?;
            let observed =
                CustodyReplayV1::decode(&replay_data).map_err(|_| CoreSbfError::Reference)?;
            require_replay_binding(observed, state, request, 0, 1)?;
            let vault = account(accounts, OPEN_VAULT)?;
            if vault.owner != &system_program::ID || vault.lamports() != 0 || vault.data_len() != 0
            {
                return Err(CoreSbfError::Creation);
            }
            Ok(OpenPrestate {
                payer_lamports: account(accounts, OPEN_PAYER)?.lamports(),
                vacant: Some(VacantAccount {
                    address: identity(vault.key.to_bytes())?,
                    lamports: vault.lamports(),
                    system_owned: true,
                    data_empty: true,
                    executable: false,
                }),
            })
        }
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            Err(CoreSbfError::Instruction)
        }
    }
}

#[inline(never)]
fn invoke_custody<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    frame: &FixedRoleAccountsV1<'accounts, 'info>,
    request: CustodyRequestV1,
    request_bytes: &[u8],
    continuation: RegistryContinuationRequestV1,
) -> Result<(), ProgramError> {
    let indices: &[usize] = match request.operation {
        OperationV1::InitializeReplay => &[0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14],
        OperationV1::OpenVault => &[0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(CoreSbfError::Instruction.into());
        }
    };
    let mut metas = Vec::with_capacity(indices.len());
    let mut infos = Vec::with_capacity(indices.len().saturating_add(1));
    for (child_index, outer_index) in indices.iter().copied().enumerate() {
        let value = account(accounts, outer_index)?;
        let signer = child_index == 0 || value.is_signer;
        let writable = match request.operation {
            OperationV1::InitializeReplay => matches!(child_index, 8 | 9),
            OperationV1::OpenVault => matches!(child_index, 8 | 10 | 13),
            OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
                return Err(CoreSbfError::Instruction.into());
            }
        };
        metas.push(if writable {
            AccountMeta::new(*value.key, signer)
        } else {
            AccountMeta::new_readonly(*value.key, signer)
        });
        infos.push(value.clone());
    }
    infos.push(frame.target_program().clone());
    let mut data = Vec::with_capacity(
        request_bytes
            .len()
            .saturating_add(REGISTRY_CONTINUATION_REQUEST_BYTES_V1),
    );
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&continuation.to_bytes());
    let instruction = Instruction {
        program_id: *frame.target_program().key,
        accounts: metas,
        data,
    };
    let digest = hash(request_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| CoreSbfError::CallerAuthority)?,
        request.market,
        CallerRoleV1::Core,
        request.context,
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

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_receipt_and_poststate(
    accounts: &[AccountInfo<'_>],
    frame: &FixedRoleAccountsV1<'_, '_>,
    state: &CoreState,
    realm: RealmV1,
    rent: Rent,
    request: CustodyRequestV1,
    request_bytes: &[u8],
    pre: OpenPrestate,
) -> Result<CustodyReceiptV1, CoreSbfError> {
    let (producer, bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.target_program().key {
        return Err(CoreSbfError::ChildAck);
    }
    let receipt = CustodyReceiptV1::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let request_digest = hash(request_bytes).to_bytes();
    let replay_account = account(accounts, REPLAY)?;
    if replay_account.owner != frame.target_program().key
        || replay_account.data_len() != CUSTODY_REPLAY_BYTES_V1
        || !rent.is_exempt(replay_account.lamports(), CUSTODY_REPLAY_BYTES_V1)
    {
        return Err(CoreSbfError::ChildAck);
    }
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let replay_digest = hash(&replay_data).to_bytes();
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| CoreSbfError::ChildAck)?;
    let expected_vault_count = u32::from(request.operation == OperationV1::OpenVault);
    require_replay_binding(
        replay,
        state,
        request,
        expected_vault_count,
        request.resulting_revision,
    )?;
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| CoreSbfError::ChildAck)?;
    let resource = match request.operation {
        OperationV1::InitializeReplay => replay_account,
        OperationV1::OpenVault => account(accounts, OPEN_VAULT)?,
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(CoreSbfError::Instruction);
        }
    };
    let expected_poststate = hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &request_digest,
        resource.key.as_ref(),
        resource.key.as_ref(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &request.rent_lamports.to_le_bytes(),
    ])
    .to_bytes();
    let payer = account(accounts, payer_index(request.operation))?;
    if receipt.evidence.poststate_commitment != expected_poststate
        || replay.last_poststate_commitment != expected_poststate
        || receipt.evidence.replay_state_digest != replay_digest
        || pre.payer_lamports.checked_sub(request.rent_lamports) != Some(payer.lamports())
    {
        return Err(CoreSbfError::ChildAck);
    }
    if request.operation == OperationV1::OpenVault {
        authenticate_vault_poststate(accounts, frame, realm, request, &rent)?;
    }
    Ok(receipt)
}

fn authenticate_vault_poststate(
    accounts: &[AccountInfo<'_>],
    frame: &FixedRoleAccountsV1<'_, '_>,
    realm: RealmV1,
    request: CustodyRequestV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let profile = PRODUCTION_ADAPTER_RELEASES
        .iter()
        .find(|release| hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id())
        .map(|release| release.profile())
        .ok_or(CoreSbfError::Reference)?;
    let vault = account(accounts, OPEN_VAULT)?;
    let token_program = account(accounts, OPEN_TOKEN_PROGRAM)?;
    let mint = account(accounts, OPEN_MINT)?;
    let authority = account(accounts, OPEN_AUTHORITY)?;
    if token_program.key.to_bytes() != profile.program_id()
        || vault.owner != token_program.key
        || vault.lamports() != rent.minimum_balance(dclutch_token_svm::ACCOUNT_BYTES)
    {
        return Err(CoreSbfError::ChildAck);
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let token = profile
        .check_custody_account(
            request.token_program,
            &data,
            mint.key.to_bytes(),
            authority.key.to_bytes(),
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    if token.amount != 0 || request.destination != vault.key.to_bytes() {
        return Err(CoreSbfError::ChildAck);
    }
    let expected_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
        frame.target_program().key,
    )
    .0;
    if authority.key != &expected_authority {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn require_replay_binding(
    replay: CustodyReplayV1,
    state: &CoreState,
    request: CustodyRequestV1,
    open_vault_count: u32,
    next_revision: u64,
) -> Result<(), CoreSbfError> {
    if replay.caller_role != CallerRoleV1::Core
        || replay.release_set != state.identity.selected_release_set.to_bytes()
        || replay.market != state.identity.market_id.to_bytes()
        || replay.realm != state.identity.realm_id.to_bytes()
        || replay.context != state.identity.market_id.to_bytes()
        || replay.caller_program != request.caller_program
        || replay.rent_refund != state.rent_beneficiary.to_bytes()
        || replay.open_vault_count != open_vault_count
        || replay.next_revision != next_revision
        || replay.generation != state.identity.generation
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn semantic_realm(state: &CoreState, realm: RealmV1) -> Result<Realm, CoreSbfError> {
    Ok(Realm {
        realm_id: state.identity.realm_id,
        collateral_mint: identity(*realm.collateral_mint())?,
        token_program: identity(*realm.token_program())?,
        collateral_release: identity(*realm.collateral_adapter_release_id())?,
    })
}

fn collateral_observation(
    state: &CoreState,
    realm: RealmV1,
) -> Result<CollateralObservation, CoreSbfError> {
    Ok(CollateralObservation {
        adapter_authenticated: PRODUCTION_ADAPTER_RELEASES.iter().any(|release| {
            hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id()
        }),
        realm_id: state.identity.realm_id,
        collateral_mint: identity(*realm.collateral_mint())?,
        token_program: identity(*realm.token_program())?,
        collateral_release: identity(*realm.collateral_adapter_release_id())?,
    })
}

fn validate_outer_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    operation: OperationV1,
) -> Result<(), CoreSbfError> {
    let expected = match operation {
        OperationV1::InitializeReplay => INITIALIZE_REPLAY_OUTER_ACCOUNT_COUNT_V1,
        OperationV1::OpenVault => OPEN_MARKET_OUTER_ACCOUNT_COUNT_V1,
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(CoreSbfError::Instruction);
        }
    };
    if accounts.len() != expected {
        return Err(CoreSbfError::AccountFrame);
    }
    require_distinct(accounts)?;
    let admission = accounts.last().ok_or(CoreSbfError::AccountFrame)?;
    if !admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    let common = FixedRoleAccountsV1::parse(program_id, accounts)?;
    for index in [REALM_RAW, REALM_STAGING] {
        let value = account(accounts, index)?;
        if value.is_signer || value.is_writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    let replay = account(accounts, REPLAY)?;
    if replay.is_signer || !replay.is_writable || replay.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    match operation {
        OperationV1::InitializeReplay => {
            require_payer(account(accounts, INITIALIZE_PAYER)?)?;
            require_program(account(accounts, INITIALIZE_SYSTEM)?, system_program::ID)?;
            require_sysvar(account(accounts, INITIALIZE_RENT)?, sysvar::rent::ID)?;
        }
        OperationV1::OpenVault => {
            require_readonly(account(accounts, OPEN_MINT)?)?;
            let vault = account(accounts, OPEN_VAULT)?;
            if vault.is_signer || !vault.is_writable || vault.executable {
                return Err(CoreSbfError::AccountFrame);
            }
            require_readonly(account(accounts, OPEN_AUTHORITY)?)?;
            let token_program = account(accounts, OPEN_TOKEN_PROGRAM)?;
            if token_program.is_signer || token_program.is_writable || !token_program.executable {
                return Err(CoreSbfError::AccountFrame);
            }
            require_payer(account(accounts, OPEN_PAYER)?)?;
            require_program(account(accounts, OPEN_SYSTEM)?, system_program::ID)?;
            require_sysvar(account(accounts, OPEN_RENT)?, sysvar::rent::ID)?;
        }
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            return Err(CoreSbfError::Instruction);
        }
    }
    if common.market().owner != program_id {
        return Err(CoreSbfError::Market);
    }
    Ok(())
}

fn require_market_unchanged(
    frame: &FixedRoleAccountsV1<'_, '_>,
    expected: &[u8; dclutch_market_core_codec::STATE_BYTES],
) -> Result<(), CoreSbfError> {
    let observed = frame
        .market()
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    if observed.as_ref() != expected {
        return Err(CoreSbfError::Market);
    }
    Ok(())
}

const fn complete_child_effect() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

const fn rent_index(operation: OperationV1) -> usize {
    match operation {
        OperationV1::InitializeReplay => INITIALIZE_RENT,
        OperationV1::OpenVault => OPEN_RENT,
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => 0,
    }
}

const fn payer_index(operation: OperationV1) -> usize {
    match operation {
        OperationV1::InitializeReplay => INITIALIZE_PAYER,
        OperationV1::OpenVault => OPEN_PAYER,
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => 0,
    }
}

fn read_rent(account: &AccountInfo<'_>) -> Result<Rent, CoreSbfError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(CoreSbfError::AccountFrame);
    }
    Rent::from_account_info(account).map_err(|_| CoreSbfError::AccountFrame)
}

fn require_readonly(account: &AccountInfo<'_>) -> Result<(), CoreSbfError> {
    if account.is_signer || account.is_writable || account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_payer(account: &AccountInfo<'_>) -> Result<(), CoreSbfError> {
    if !account.is_signer || !account.is_writable || account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_sysvar(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_signer || account.is_writable || account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_program(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_signer || account.is_writable || !account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}
