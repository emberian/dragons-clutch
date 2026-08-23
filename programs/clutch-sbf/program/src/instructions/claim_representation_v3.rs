//! Full-width native-Egg materialization and dematerialization.
//!
//! These actions mutate only canonical PositionV3, ClaimLedgerV3, GEN1
//! Replay, and the separately selected Token-2022 claim plane. HoardV2 and its
//! Realm-selected collateral account are authenticated but never debited,
//! credited, or used as mint authority.

use crate::accounts::{require, require_signer, Outcome};
use crate::claim_release::authenticate_claim_issuance_v1;
use crate::claim_truth::{self, ObservedMintSupplies};
use crate::error::{ClutchError, Refusal};
use crate::{seeds, token};
use clutch_collateral_adapter_v2::{
    accept_claim_representation_v3, prepare_claim_representation_v3,
    AdapterClaimRepresentationObservationV3, ClaimRepresentationKindV3, Id as CollateralId,
};
use clutch_general_v2_contract::{
    project_general_replay_transition_v1, GeneralReplayTransitionKindV1, Id32,
};
use clutch_owner_settlement::PositionSettlementPoststateV3;
use clutch_retirement::MAX_OUTCOMES;
use clutch_solana_layout::{Hash32, Intent};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_general_position_replay_v1,
    RuntimeSha256,
};

/// Fixed prefix before one mint per active outcome.
pub const CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3: usize = 14;

const IX_ACTOR: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_POLICY: usize = 3;
const IX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_MARKET_INSTANCE: usize = 7;
const IX_HOARD: usize = 8;
const IX_CLAIM_LEDGER: usize = 9;
const IX_POSITION: usize = 10;
const IX_REPLAY: usize = 11;
const IX_OUTCOME_TOKEN_PROGRAM: usize = 12;
const IX_HOLDER_TOKEN: usize = 13;
const IX_OUTCOME_MINTS: usize = CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClaimRepresentationRequestV3 {
    sequence: u64,
    market_instance_id: Hash32,
    owner: Hash32,
    holder_token_account: Hash32,
    outcome: u8,
    quantity: u64,
    kind: ClaimRepresentationKindV3,
}

fn decode_request(request: &Request) -> Outcome<ClaimRepresentationRequestV3> {
    match request.action {
        Action::Layout(Intent::Materialize {
            market,
            owner,
            destination,
            outcome,
            quantity,
        }) => Ok(ClaimRepresentationRequestV3 {
            sequence: request.sequence,
            market_instance_id: market,
            owner,
            holder_token_account: destination,
            outcome,
            quantity,
            kind: ClaimRepresentationKindV3::Materialize,
        }),
        Action::Layout(Intent::Dematerialize {
            market,
            owner,
            source,
            outcome,
            quantity,
        }) => Ok(ClaimRepresentationRequestV3 {
            sequence: request.sequence,
            market_instance_id: market,
            owner,
            holder_token_account: source,
            outcome,
            quantity,
            kind: ClaimRepresentationKindV3::Dematerialize,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Permit only the unavoidable alias between collateral and outcome token
/// program roles when a Realm itself selects Token-2022.
fn require_distinct_claim_roles(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let allowed_program_alias = left == IX_COLLATERAL_TOKEN_PROGRAM
                && right == IX_OUTCOME_TOKEN_PROGRAM
                && accounts[left].key == accounts[right].key;
            require(
                accounts[left].key != accounts[right].key || allowed_program_alias,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn observe_mints(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_instance_id: [u8; 32],
    outcome_count: u8,
    selected_outcome: u8,
) -> Outcome<ObservedMintSupplies> {
    claim_truth::observe_outcome_mints_v2(
        program_id,
        accounts,
        IX_OUTCOME_MINTS,
        *accounts[IX_MARKET_RUNTIME].key,
        market_instance_id,
        outcome_count,
        Some(selected_outcome),
    )
}

fn token_observation(
    accounts: &[AccountInfo],
    outcome: u8,
) -> Outcome<AdapterClaimRepresentationObservationV3> {
    let mint_index = IX_OUTCOME_MINTS
        .checked_add(usize::from(outcome))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mint = &accounts[mint_index];
    let holder = &accounts[IX_HOLDER_TOKEN];
    let mint_observation = token::admit_mint(
        mint,
        &token::MintPolicy::outcome(*mint.key, *accounts[IX_MARKET_RUNTIME].key),
    )?;
    let holder_observation = token::admit_token_account(
        holder,
        &token::TokenAccountPolicy::holder(*mint.key, *accounts[IX_ACTOR].key),
    )?;
    let mint_authority = mint_observation
        .mint_authority
        .ok_or(Refusal::Adapter(ClutchError::MintNotAdmitted))?;
    Ok(AdapterClaimRepresentationObservationV3 {
        mint: CollateralId::from_bytes(mint.key.to_bytes()),
        mint_authority: CollateralId::from_bytes(mint_authority),
        holder_token_account: CollateralId::from_bytes(holder.key.to_bytes()),
        holder_owner: CollateralId::from_bytes(holder_observation.owner),
        mint_supply_atoms: mint_observation.supply,
        holder_atoms: holder_observation.amount,
    })
}

/// Execute one full-width claim materialization or dematerialization.
pub fn process_claim_representation_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &Request,
) -> Outcome<()> {
    let request = decode_request(request)?;
    require(
        accounts.len() >= CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct_claim_roles(accounts)?;
    require(
        accounts[IX_ACTOR].key.to_bytes() == request.owner.bytes()
            && accounts[IX_HOLDER_TOKEN].key.to_bytes() == request.holder_token_account.bytes(),
        ClutchError::UnauthorizedActor,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD],
        &accounts[IX_CLAIM_LEDGER],
        false,
        true,
    )?;
    let expected_count = CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3
        .checked_add(usize::from(liabilities.market_binding.outcome_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() == expected_count, ClutchError::AccountCount)?;
    require(
        liabilities.market_binding.market_instance_v2_id.bytes()
            == request.market_instance_id.bytes()
            && request.outcome < liabilities.market_binding.outcome_count,
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
    let claim =
        authenticate_claim_issuance_v1(liabilities.bound, &accounts[IX_OUTCOME_TOKEN_PROGRAM])?;
    let observed_before = observe_mints(
        program_id,
        accounts,
        request.market_instance_id.bytes(),
        liabilities.market_binding.outcome_count,
        request.outcome,
    )?;
    let token_before = token_observation(accounts, request.outcome)?;
    let authority = match request.kind {
        ClaimRepresentationKindV3::Materialize => {
            CollateralId::from_bytes(accounts[IX_MARKET_RUNTIME].key.to_bytes())
        }
        ClaimRepresentationKindV3::Dematerialize => {
            CollateralId::from_bytes(accounts[IX_ACTOR].key.to_bytes())
        }
    };
    let prepared = prepare_claim_representation_v3(
        claim,
        CollateralId::from_bytes(accounts[IX_POSITION].key.to_bytes()),
        position.projection,
        liabilities.claim_ledger,
        request.kind,
        request.outcome,
        request.quantity,
        observed_before.values,
        token_before,
        authority,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let intent = prepared.issuance_intent();
    let selected_mint = &accounts[IX_OUTCOME_MINTS + usize::from(request.outcome)];
    require(
        intent.mint == CollateralId::from_bytes(selected_mint.key.to_bytes())
            && intent.holder_token_account
                == CollateralId::from_bytes(accounts[IX_HOLDER_TOKEN].key.to_bytes())
            && intent.authority == authority
            && intent.amount_atoms == request.quantity,
        ClutchError::MismatchedState,
    )?;
    match request.kind {
        ClaimRepresentationKindV3::Materialize => {
            require(
                intent.minting && intent.program_signed,
                ClutchError::MismatchedState,
            )?;
            let binding_key = accounts[IX_MARKET_BINDING].key.to_bytes();
            let bump = [position.market_runtime.stored_bump];
            let signer: [&[u8]; 3] = [seeds::SEED_GENERAL_V2_MARKET_RUNTIME, &binding_key, &bump];
            token::mint_to_signed(
                &accounts[IX_OUTCOME_TOKEN_PROGRAM],
                selected_mint,
                &accounts[IX_HOLDER_TOKEN],
                &accounts[IX_MARKET_RUNTIME],
                request.quantity,
                &signer,
            )?;
        }
        ClaimRepresentationKindV3::Dematerialize => {
            require(
                !intent.minting && !intent.program_signed,
                ClutchError::MismatchedState,
            )?;
            token::burn(
                &accounts[IX_OUTCOME_TOKEN_PROGRAM],
                &accounts[IX_HOLDER_TOKEN],
                selected_mint,
                &accounts[IX_ACTOR],
                request.quantity,
            )?;
        }
    }

    let observed_after = observe_mints(
        program_id,
        accounts,
        request.market_instance_id.bytes(),
        liabilities.market_binding.outcome_count,
        request.outcome,
    )?;
    let token_after = token_observation(accounts, request.outcome)?;
    let accepted = accept_claim_representation_v3(prepared, observed_after.values, token_after)
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
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
    let replay_kind = match request.kind {
        ClaimRepresentationKindV3::Materialize => GeneralReplayTransitionKindV1::Materialize,
        ClaimRepresentationKindV3::Dematerialize => GeneralReplayTransitionKindV1::Dematerialize,
    };
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

    accounts[IX_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &position_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accepted
        .claim_ledger_after()
        .encode(
            &mut accounts[IX_CLAIM_LEDGER]
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

const _: () = assert!(MAX_OUTCOMES == 16);
