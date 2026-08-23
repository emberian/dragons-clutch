//! Full-width collateral cash boundary for canonical Position V3.
//!
//! This module deliberately does not consult the lowered legacy Market,
//! Hoard, Position, or Replay families. The legacy `WithdrawCash` wire field
//! named `market` is interpreted only as the complete MarketInstanceV2
//! content identity and is joined to the immutable General Market binding,
//! Product artifact, Realm-selected collateral release, HoardV2, PositionV3,
//! and GEN1 Replay before any token CPI is authorized.

use crate::accounts::{require, require_count, require_distinct, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    accept_position_collateral_transfer_v3, accept_position_hoard_cash_transition_v3,
    prepare_position_collateral_transfer_v3, CpiAccountMetaV2, CustodyTransferKindV2,
    Id as CollateralId, PositionCollateralTransferRequestV3, PreparedPositionCollateralTransferV3,
    RuntimeAccountViewV2, TokenAccountRoleV2, TransferAuthorityKindV2, TransferAuthorityV2,
    TransferEndpointV2,
};
use clutch_general_v2_contract::{
    found_general_position_replay_v1, project_general_replay_transition_v1,
    GeneralReplayTransitionKindV1, Id32,
};
use clutch_owner_settlement::PositionSettlementPoststateV3;
use clutch_retirement::{
    admit_deletable_rent, admit_initial_rent_split, Identity32V1, PositionPurposeV3,
    POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES,
};
use clutch_solana_layout::{Hash32, Intent};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    RuntimeSha256,
};
use super::genesis::SYSTEM_PROGRAM_ID;

/// Exact full-width WithdrawCash account list.
pub const WITHDRAW_ACCOUNT_COUNT_V3: usize = 16;
/// Exact full-width Endow account list, including owner-plane construction.
pub const ENDOW_ACCOUNT_COUNT_V3: usize = 18;

const IX_ACTOR: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_POLICY: usize = 3;
const IX_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_MARKET_INSTANCE: usize = 7;
const IX_HOARD: usize = 8;
const IX_CLAIM_LEDGER: usize = 9;
const IX_POSITION: usize = 10;
const IX_REPLAY: usize = 11;
const IX_COLLATERAL_MINT: usize = 12;
const IX_DESTINATION: usize = 13;
const IX_HOARD_AUTHORITY: usize = 14;
const IX_HOARD_TOKEN: usize = 15;
const IX_SYSTEM: usize = 16;
const IX_RENT: usize = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WithdrawCashRequestV3 {
    sequence: u64,
    market_instance_id: Hash32,
    owner: Hash32,
    destination: Hash32,
    amount_atoms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EndowRequestV3 {
    sequence: u64,
    market_instance_id: Hash32,
    owner: Hash32,
    amount_atoms: u64,
}

fn decode_withdraw_request(request: &Request) -> Outcome<WithdrawCashRequestV3> {
    match request.action {
        Action::Layout(Intent::WithdrawCash {
            market,
            owner,
            destination,
            amount,
        }) => Ok(WithdrawCashRequestV3 {
            sequence: request.sequence,
            market_instance_id: market,
            owner,
            destination,
            amount_atoms: amount,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

fn decode_endow_request(request: &Request) -> Outcome<EndowRequestV3> {
    match request.action {
        Action::Layout(Intent::Endow {
            market,
            owner,
            amount,
        }) => Ok(EndowRequestV3 {
            sequence: request.sequence,
            market_instance_id: market,
            owner,
            amount_atoms: amount,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

fn runtime_account_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: CollateralId::from_bytes(account.key.to_bytes()),
        owner_program: CollateralId::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

fn cpi_account_meta(value: CpiAccountMetaV2) -> AccountMeta {
    AccountMeta {
        pubkey: Pubkey::new_from_array(value.address.bytes()),
        is_signer: value.signer,
        is_writable: value.writable,
    }
}

fn identity(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[allow(clippy::too_many_arguments)]
fn create_fully_funded_pda<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    principal_lamports: u64,
    expected_balance_after: u64,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require(
        target.is_writable
            && !target.executable
            && target.data_len() == 0
            && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )?;
    let payer_before = payer.lamports();
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &super::genesis::transfer_data(principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        payer.lamports()
            == payer_before
                .checked_sub(principal_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && target.lamports() == expected_balance_after,
        ClutchError::AccountCreationFailed,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &super::genesis::allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &super::genesis::assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner == program_id
            && target.lamports() == expected_balance_after,
        ClutchError::AccountCreationFailed,
    )
}

#[allow(clippy::too_many_arguments)]
fn ensure_general_owner_plane_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    owner: [u8; 32],
    market_instance: [u8; 32],
    outcome_count: u8,
    realm: [u8; 32],
    collateral_policy: [u8; 32],
    collateral_release: [u8; 32],
    neutral_sink: [u8; 32],
) -> Outcome<()> {
    let position = &accounts[IX_POSITION];
    let replay = &accounts[IX_REPLAY];
    super::genesis::require_system_program(&accounts[IX_SYSTEM])?;
    let rent = super::genesis::read_rent(&accounts[IX_RENT])?;
    let existing = position.owner == program_id
        && position.data_len() == POSITION_V3_BYTES
        && replay.owner == program_id
        && replay.data_len() == clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES;
    if existing {
        return Ok(());
    }
    require(
        *position.owner == SYSTEM_PROGRAM_ID
            && position.data_len() == 0
            && *replay.owner == SYSTEM_PROGRAM_ID
            && replay.data_len() == 0,
        ClutchError::AlreadyInitialized,
    )?;
    let purpose_binding = accounts[IX_MARKET_RUNTIME].key.to_bytes();
    let position_pda = seeds::position_v3_pda(
        program_id,
        &market_instance,
        &owner,
        PositionPurposeV3::General,
        &purpose_binding,
    );
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &position.key.to_bytes(),
        PositionPurposeV3::General,
        &purpose_binding,
    );
    require(
        *position.key == position_pda.0 && *replay.key == replay_pda.0,
        ClutchError::WrongPda,
    )?;

    let position_live_minimum = rent.minimum_balance(POSITION_V3_BYTES)?;
    let position_tombstone_principal = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let position_refundable_principal = position_live_minimum
        .checked_sub(position_tombstone_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let replay_principal =
        rent.minimum_balance(clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    require(
        position_refundable_principal != 0
            && position_tombstone_principal != 0
            && replay_principal != 0,
        ClutchError::WrongRentSysvar,
    )?;
    let payer_before = accounts[IX_ACTOR].lamports();
    let position_admission = admit_initial_rent_split(
        identity(position.key.to_bytes())?,
        identity(owner)?,
        position_refundable_principal,
        position_tombstone_principal,
        position.lamports(),
        payer_before,
        identity(neutral_sink)?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let replay_admission = admit_deletable_rent(
        identity(replay.key.to_bytes())?,
        identity(owner)?,
        replay_principal,
        replay.lamports(),
        position_admission.payer_balance_after(),
        identity(neutral_sink)?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let founding = found_general_position_replay_v1(
        identity(position.key.to_bytes())?,
        identity(replay.key.to_bytes())?,
        identity(market_instance)?,
        identity(realm)?,
        identity(collateral_policy)?,
        identity(collateral_release)?,
        identity(owner)?,
        identity(purpose_binding)?,
        outcome_count,
        position_pda.1,
        replay_pda.1,
        position_admission.rent(),
        replay_admission.rent(),
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let purpose = [u8::from(PositionPurposeV3::General)];
    let position_bump = [position_pda.1];
    let position_seeds: [&[u8]; 6] = [
        clutch_retirement::POSITION_V3_PDA_PREFIX,
        &market_instance,
        &owner,
        &purpose,
        &purpose_binding,
        &position_bump,
    ];
    create_fully_funded_pda(
        program_id,
        &accounts[IX_ACTOR],
        position,
        &accounts[IX_SYSTEM],
        position_live_minimum,
        position_admission.account_balance_after(),
        POSITION_V3_BYTES,
        &position_seeds,
    )?;
    let replay_bump = [replay_pda.1];
    let position_key = position.key.to_bytes();
    let replay_seeds: [&[u8]; 5] = [
        clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
        &position_key,
        &purpose,
        &purpose_binding,
        &replay_bump,
    ];
    create_fully_funded_pda(
        program_id,
        &accounts[IX_ACTOR],
        replay,
        &accounts[IX_SYSTEM],
        replay_principal,
        replay_admission.account_balance_after(),
        clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        &replay_seeds,
    )?;
    position
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(founding.position_body());
    replay
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(founding.replay_body());
    require(
        accounts[IX_ACTOR].lamports() == replay_admission.payer_balance_after(),
        ClutchError::AccountCreationFailed,
    )
}

#[allow(clippy::too_many_arguments)]
fn invoke_deposit<'a>(
    prepared: PreparedPositionCollateralTransferV3,
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    hoard_token: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
) -> Outcome<clutch_collateral_adapter_v2::AcceptedPositionCollateralTransferV3> {
    let cpi = prepared.cpi();
    require(
        !cpi.program_signed
            && cpi.token_program == CollateralId::from_bytes(token_program.key.to_bytes())
            && cpi.accounts[0].address == CollateralId::from_bytes(source.key.to_bytes())
            && cpi.accounts[1].address == CollateralId::from_bytes(mint.key.to_bytes())
            && cpi.accounts[2].address == CollateralId::from_bytes(hoard_token.key.to_bytes())
            && cpi.accounts[3].address == CollateralId::from_bytes(authority.key.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(cpi_account_meta).collect(),
    );
    let account_infos = [
        source.clone(),
        mint.clone(),
        hoard_token.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    invoke(&instruction, &account_infos)
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;

    let mint_after = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_after = source
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_after = hoard_token
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_position_collateral_transfer_v3(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(source, &source_after),
        runtime_account_view(hoard_token, &hoard_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// Deposit exact owner collateral through the Realm-selected token program.
/// A first deposit creates the canonical zero-liability PositionV3/GEN1 pair
/// with independently paid lamport rent before applying sequence-zero Endow.
pub fn process_endow_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &Request,
) -> Outcome<()> {
    let request = decode_endow_request(request)?;
    require_count(accounts, ENDOW_ACCOUNT_COUNT_V3)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require_distinct(accounts)?;
    require(
        accounts[IX_ACTOR].key.to_bytes() == request.owner.bytes(),
        ClutchError::UnauthorizedActor,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_POLICY],
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        true,
        false,
    )?;
    require(
        liabilities.market_binding.market_instance_v2_id.bytes()
            == request.market_instance_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let release_id = liabilities
        .bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    ensure_general_owner_plane_v3(
        program_id,
        accounts,
        request.owner.bytes(),
        liabilities.market_binding.market_instance_v2_id.bytes(),
        liabilities.market_binding.outcome_count,
        liabilities.bound.market().realm.bytes(),
        liabilities.bound.policy_id().bytes(),
        release_id.bytes(),
        liabilities.market_binding.neutral_sink.bytes(),
    )?;
    let position = authenticate_general_position_replay_v1(
        program_id,
        liabilities.bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        request.owner.bytes(),
        request.sequence,
    )?;

    let market_id = liabilities.bound.market().market;
    let owner_id = CollateralId::from_bytes(request.owner.bytes());
    let expected_authority = seeds::hoard_authority_v2_pda(
        program_id,
        &liabilities.market_binding.market_instance_v2_id.bytes(),
    );
    let expected_hoard_token = seeds::hoard_token_v2_pda(
        program_id,
        &liabilities.market_binding.market_instance_v2_id.bytes(),
    );
    require(
        *accounts[IX_HOARD_AUTHORITY].key == expected_authority.0
            && *accounts[IX_HOARD_TOKEN].key == expected_hoard_token.0
            && liabilities.hoard.authority
                == CollateralId::from_bytes(accounts[IX_HOARD_AUTHORITY].key.to_bytes())
            && liabilities.hoard.token_account
                == CollateralId::from_bytes(accounts[IX_HOARD_TOKEN].key.to_bytes()),
        ClutchError::WrongPda,
    )?;

    let prepared = {
        let mint = accounts[IX_COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let source = accounts[IX_DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_token = accounts[IX_HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        prepare_position_collateral_transfer_v3(
            liabilities.bound,
            CollateralId::from_bytes(accounts[IX_POSITION].key.to_bytes()),
            position.projection,
            PositionCollateralTransferRequestV3 {
                kind: CustodyTransferKindV2::HolderDeposit,
                source: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::Holder { owner: owner_id },
                    semantic_owner: owner_id,
                    compartment: 0,
                },
                destination: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::Hoard,
                    semantic_owner: market_id,
                    compartment: 1,
                },
                authority: TransferAuthorityV2 {
                    address: owner_id,
                    kind: TransferAuthorityKindV2::TransactionSigner,
                    is_transaction_signer: true,
                    program_address_authenticated: false,
                    is_writable: accounts[IX_ACTOR].is_writable,
                    executable: accounts[IX_ACTOR].executable,
                    data_is_empty: accounts[IX_ACTOR].data_is_empty(),
                },
                amount_atoms: request.amount_atoms,
                locked_collateral_atoms: liabilities.hoard.locked_claim_principal_atoms,
            },
            runtime_account_view(&accounts[IX_COLLATERAL_MINT], &mint),
            runtime_account_view(&accounts[IX_DESTINATION], &source),
            runtime_account_view(&accounts[IX_HOARD_TOKEN], &hoard_token),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
    };
    let accepted_position = invoke_deposit(
        prepared,
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_COLLATERAL_MINT],
        &accounts[IX_DESTINATION],
        &accounts[IX_HOARD_TOKEN],
        &accounts[IX_ACTOR],
    )?;
    let accepted = accept_position_hoard_cash_transition_v3(
        accepted_position,
        liabilities.hoard,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::HoardMirrorMismatch))?;

    let position_post = accepted.position().position_post();
    let fields = position_post.fields();
    let settlement_post = position
        .position
        .settlement_poststate(
            fields.cash_atoms,
            fields.reserved_cash_atoms,
            fields.native_eggs,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        settlement_post.semantic == position_post,
        ClutchError::MismatchedState,
    )?;
    let replay = project_general_replay_transition_v1(
        position.replay,
        PositionSettlementPoststateV3 {
            semantic: position_post,
            ..settlement_post
        },
        GeneralReplayTransitionKindV1::Endow,
        Id32::new(accepted.transition_id().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Id32::new(
            accepted
                .position()
                .receipt_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &position_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accepted
        .hoard()
        .hoard_after
        .encode(
            &mut accounts[IX_HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[IX_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(replay.replay_poststate_body());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn invoke_withdrawal<'a>(
    prepared: PreparedPositionCollateralTransferV3,
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    hoard_token: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    signer: &[&[u8]],
) -> Outcome<clutch_collateral_adapter_v2::AcceptedPositionCollateralTransferV3> {
    let cpi = prepared.cpi();
    require(
        cpi.program_signed
            && cpi.token_program == CollateralId::from_bytes(token_program.key.to_bytes())
            && cpi.accounts[0].address == CollateralId::from_bytes(hoard_token.key.to_bytes())
            && cpi.accounts[1].address == CollateralId::from_bytes(mint.key.to_bytes())
            && cpi.accounts[2].address == CollateralId::from_bytes(destination.key.to_bytes())
            && cpi.accounts[3].address == CollateralId::from_bytes(authority.key.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(cpi_account_meta).collect(),
    );
    let account_infos = [
        hoard_token.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    invoke_signed(&instruction, &account_infos, &[signer])
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;

    let mint_after = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_after = hoard_token
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_position_collateral_transfer_v3(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(hoard_token, &hoard_after),
        runtime_account_view(destination, &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// Withdraw exact unreserved Position cash through the Realm-selected token
/// program and atomically publish PositionV3, HoardV2, and GEN1 Replay.
pub fn process_withdraw_cash_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &Request,
) -> Outcome<()> {
    let request = decode_withdraw_request(request)?;
    require_count(accounts, WITHDRAW_ACCOUNT_COUNT_V3)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    require(
        accounts[IX_ACTOR].key.to_bytes() == request.owner.bytes()
            && accounts[IX_DESTINATION].key.to_bytes() == request.destination.bytes(),
        ClutchError::UnauthorizedActor,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_POLICY],
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        true,
        false,
    )?;
    require(
        liabilities.market_binding.market_instance_v2_id.bytes()
            == request.market_instance_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let position = authenticate_general_position_replay_v1(
        program_id,
        liabilities.bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        request.owner.bytes(),
        request.sequence,
    )?;

    let market_id = liabilities.bound.market().market;
    let owner_id = CollateralId::from_bytes(request.owner.bytes());
    let expected_authority = seeds::hoard_authority_v2_pda(
        program_id,
        &liabilities.market_binding.market_instance_v2_id.bytes(),
    );
    let expected_hoard_token = seeds::hoard_token_v2_pda(
        program_id,
        &liabilities.market_binding.market_instance_v2_id.bytes(),
    );
    require(
        *accounts[IX_HOARD_AUTHORITY].key == expected_authority.0
            && *accounts[IX_HOARD_TOKEN].key == expected_hoard_token.0
            && liabilities.hoard.authority
                == CollateralId::from_bytes(accounts[IX_HOARD_AUTHORITY].key.to_bytes())
            && liabilities.hoard.token_account
                == CollateralId::from_bytes(accounts[IX_HOARD_TOKEN].key.to_bytes()),
        ClutchError::WrongPda,
    )?;

    let prepared = {
        let mint = accounts[IX_COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_token = accounts[IX_HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination = accounts[IX_DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        prepare_position_collateral_transfer_v3(
            liabilities.bound,
            CollateralId::from_bytes(accounts[IX_POSITION].key.to_bytes()),
            position.projection,
            PositionCollateralTransferRequestV3 {
                kind: CustodyTransferKindV2::HolderWithdrawal,
                source: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::Hoard,
                    semantic_owner: market_id,
                    compartment: 1,
                },
                destination: TransferEndpointV2 {
                    token_role: TokenAccountRoleV2::Holder { owner: owner_id },
                    semantic_owner: owner_id,
                    compartment: 0,
                },
                authority: TransferAuthorityV2 {
                    address: liabilities.hoard.authority,
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[IX_HOARD_AUTHORITY].is_writable,
                    executable: accounts[IX_HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[IX_HOARD_AUTHORITY].data_is_empty(),
                },
                amount_atoms: request.amount_atoms,
                locked_collateral_atoms: liabilities.hoard.locked_claim_principal_atoms,
            },
            runtime_account_view(&accounts[IX_COLLATERAL_MINT], &mint),
            runtime_account_view(&accounts[IX_HOARD_TOKEN], &hoard_token),
            runtime_account_view(&accounts[IX_DESTINATION], &destination),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
    };

    let market_bytes = liabilities.market_binding.market_instance_v2_id.bytes();
    let bump = [expected_authority.1];
    let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
    let accepted_position = invoke_withdrawal(
        prepared,
        &accounts[IX_TOKEN_PROGRAM],
        &accounts[IX_COLLATERAL_MINT],
        &accounts[IX_HOARD_TOKEN],
        &accounts[IX_DESTINATION],
        &accounts[IX_HOARD_AUTHORITY],
        &signer,
    )?;
    let accepted = accept_position_hoard_cash_transition_v3(
        accepted_position,
        liabilities.hoard,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::HoardMirrorMismatch))?;

    let position_post = accepted.position().position_post();
    let fields = position_post.fields();
    let settlement_post = position
        .position
        .settlement_poststate(
            fields.cash_atoms,
            fields.reserved_cash_atoms,
            fields.native_eggs,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        settlement_post.semantic == position_post,
        ClutchError::MismatchedState,
    )?;
    let replay = project_general_replay_transition_v1(
        position.replay,
        PositionSettlementPoststateV3 {
            semantic: position_post,
            ..settlement_post
        },
        GeneralReplayTransitionKindV1::WithdrawCash,
        Id32::new(accepted.transition_id().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Id32::new(
            accepted
                .position()
                .receipt_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;

    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &position_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accepted
        .hoard()
        .hoard_after
        .encode(
            &mut accounts[IX_HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[IX_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(replay.replay_poststate_body());
    Ok(())
}
