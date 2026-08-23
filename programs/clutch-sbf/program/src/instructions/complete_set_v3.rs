//! Full-width internal complete-set Split/Merge execution.
//!
//! Split and Merge move no collateral tokens. They atomically reclassify
//! HoardV2 cash and locked principal, mutate canonical PositionV3 native Eggs,
//! update ClaimLedgerV3 aggregate internal supply, and advance GEN1 Replay.
//! Read-only mint and Hoard-token observations prove that no external custody
//! balance or mint supply moved while those four semantic owners advanced.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_collateral_adapter_v2::{
    accept_complete_set_position_transition_v3, prepare_complete_set_position_transition_v3,
    CompleteSetReclassificationKindV3, Id as CollateralId, RuntimeAccountViewV2,
};
use clutch_general_v2_contract::{
    project_general_replay_transition_v1, GeneralReplayTransitionKindV1, Id32,
};
use clutch_owner_settlement::PositionSettlementPoststateV3;
use clutch_solana_layout::collateral_v3_accounts::complete_set_indices_v3 as ix;
use clutch_solana_layout::collateral_v3_accounts::CollateralActionV3;
use clutch_solana_layout::Hash32;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    validate_full_width_collateral_accounts_v3, RuntimeSha256,
};

/// Exact full-width Split/Merge account list.
pub const COMPLETE_SET_ACCOUNT_COUNT_V3: usize =
    clutch_solana_layout::collateral_v3_accounts::COMPLETE_SET_ACCOUNT_COUNT_V3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompleteSetActionV3 {
    Split,
    Merge,
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

/// Execute one full-width Split or Merge without authorizing a token CPI.
#[allow(clippy::too_many_arguments)]
pub fn process_complete_set_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    market_instance_id: Hash32,
    owner: Hash32,
    quantity: u64,
    action: CompleteSetActionV3,
) -> Outcome<()> {
    validate_full_width_collateral_accounts_v3(
        accounts,
        match action {
            CompleteSetActionV3::Split => CollateralActionV3::Split,
            CompleteSetActionV3::Merge => CollateralActionV3::Merge,
        },
        None,
    )?;
    require(
        accounts[ix::ACTOR].key.to_bytes() == owner.bytes(),
        ClutchError::UnauthorizedActor,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[ix::REALM],
        &accounts[ix::PROFILE],
        &accounts[ix::POLICY],
        &accounts[ix::TOKEN_PROGRAM],
        &accounts[ix::MARKET_BINDING],
        &accounts[ix::MARKET_RUNTIME],
        &accounts[ix::MARKET_INSTANCE],
        &accounts[ix::HOARD],
        &accounts[ix::CLAIM_LEDGER],
        true,
        true,
    )?;
    require(
        liabilities.market_binding.market_instance_v2_id.bytes() == market_instance_id.bytes()
            && accounts[ix::HOARD_TOKEN].key.to_bytes() == liabilities.hoard.token_account.bytes(),
        ClutchError::MismatchedState,
    )?;
    let position = authenticate_general_position_replay_v1(
        program_id,
        liabilities.bound,
        &accounts[ix::MARKET_BINDING],
        &accounts[ix::MARKET_RUNTIME],
        &accounts[ix::POSITION],
        &accounts[ix::REPLAY],
        owner.bytes(),
        sequence,
    )?;
    let (kind, replay_kind) = match action {
        CompleteSetActionV3::Split => (
            CompleteSetReclassificationKindV3::Split,
            GeneralReplayTransitionKindV1::Split,
        ),
        CompleteSetActionV3::Merge => (
            CompleteSetReclassificationKindV3::Merge,
            GeneralReplayTransitionKindV1::Merge,
        ),
    };

    let prepared = {
        let mint = accounts[ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_token = accounts[ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        prepare_complete_set_position_transition_v3(
            liabilities.bound,
            CollateralId::from_bytes(accounts[ix::POSITION].key.to_bytes()),
            position.projection,
            liabilities.hoard,
            liabilities.claim_ledger,
            kind,
            quantity,
            runtime_account_view(&accounts[ix::COLLATERAL_MINT], &mint),
            runtime_account_view(&accounts[ix::HOARD_TOKEN], &hoard_token),
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
    };
    let accepted = {
        let mint = accounts[ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_token = accounts[ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        accept_complete_set_position_transition_v3(
            prepared,
            runtime_account_view(&accounts[ix::COLLATERAL_MINT], &mint),
            runtime_account_view(&accounts[ix::HOARD_TOKEN], &hoard_token),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?
    };

    let position_post = accepted.position_after();
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
        replay_kind,
        Id32::new(accepted.transition_id().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Id32::new(accepted.receipt_id().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;

    accounts[ix::POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &position_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accepted
        .liability()
        .hoard_after
        .encode(
            &mut accounts[ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accepted
        .liability()
        .claim_ledger_after
        .encode(
            &mut accounts[ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accounts[ix::REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(replay.replay_poststate_body());
    Ok(())
}
