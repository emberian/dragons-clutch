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
    project_general_replay_transition_v1, GeneralReplayTransitionKindV1, Id32,
};
use clutch_owner_settlement::PositionSettlementPoststateV3;
use clutch_solana_layout::{Hash32, Intent};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    RuntimeSha256,
};

/// Exact full-width WithdrawCash account list.
pub const WITHDRAW_ACCOUNT_COUNT_V3: usize = 16;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WithdrawCashRequestV3 {
    sequence: u64,
    market_instance_id: Hash32,
    owner: Hash32,
    destination: Hash32,
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
