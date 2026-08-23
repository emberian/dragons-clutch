//! Full-width positionless bearer-claim redemption.
//!
//! This route authenticates ResolutionV5, HoardV2, and ClaimLedgerV3 against
//! the complete MarketInstanceV2 identity. It has no legacy Market, Terms,
//! Kernel, SupplyLedger, or lowered Resolution fallback. The direct path is
//! admitted only for an exact whole-atom payout; nonintegral value is retained
//! by the separately owned Fractional route through Resolution's quotient and
//! remainder projection.

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::claim_release::authenticate_claim_issuance_v1;
use crate::claim_truth::{self, ObservedMintSupplies};
use crate::error::{ClutchError, Refusal};
use crate::{seeds, token};
use clutch_collateral_adapter_v2::{
    accept_bearer_claim_burn_v3, accept_bearer_claim_redemption_v3,
    accept_claim_redemption_collateral_v2, accept_zero_claim_redemption_collateral_v2,
    prepare_bearer_claim_redemption_v3, prepare_claim_redemption_collateral_v2,
    prepare_zero_claim_redemption_collateral_v2, AcceptedBearerRedemptionCollateralV3,
    AdapterBearerClaimObservationV3, CpiAccountMetaV2, Id as CollateralId,
    PreparedClaimRedemptionCollateralV2, PreparedZeroClaimRedemptionCollateralV2,
    RuntimeAccountViewV2, TransferAuthorityKindV2, TransferAuthorityV2,
};
use clutch_solana_layout::collateral_v3_accounts::external_redemption_indices_v3 as ix;
use clutch_solana_layout::collateral_v3_accounts::CollateralActionV3;
use clutch_solana_layout::{Hash32, Intent};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, authenticate_resolution_v5,
    validate_full_width_collateral_accounts_v3, RuntimeSha256,
};

/// Fixed account prefix before one mint per active native outcome.
pub const EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3: usize =
    clutch_solana_layout::collateral_v3_accounts::EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExternalRedemptionRequestV3 {
    market_instance_id: Hash32,
    claimant: Hash32,
    source: Hash32,
    destination: Hash32,
    outcome: u8,
    quantity: u64,
}

fn decode_request(request: &Request) -> Outcome<ExternalRedemptionRequestV3> {
    require(request.sequence == 0, ClutchError::Replay)?;
    match request.action {
        Action::Layout(Intent::RedeemExternal {
            market,
            claimant,
            source,
            destination,
            outcome,
            quantity,
        }) => Ok(ExternalRedemptionRequestV3 {
            market_instance_id: market,
            claimant,
            source,
            destination,
            outcome,
            quantity,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

fn require_distinct_roles(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let allowed_program_alias = left == ix::COLLATERAL_TOKEN_PROGRAM
                && right == ix::OUTCOME_TOKEN_PROGRAM
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
        ix::OUTCOME_MINTS,
        *accounts[ix::MARKET_RUNTIME].key,
        market_instance_id,
        outcome_count,
        Some(selected_outcome),
    )
}

fn bearer_observation(
    accounts: &[AccountInfo],
    outcome: u8,
) -> Outcome<AdapterBearerClaimObservationV3> {
    let mint = &accounts[ix::OUTCOME_MINTS + usize::from(outcome)];
    let source = &accounts[ix::SOURCE];
    let mint_observation = token::admit_mint(
        mint,
        &token::MintPolicy::outcome(*mint.key, *accounts[ix::MARKET_RUNTIME].key),
    )?;
    let source_observation = token::admit_token_account(
        source,
        &token::TokenAccountPolicy::holder(*mint.key, *accounts[ix::CLAIMANT].key),
    )?;
    let mint_authority = mint_observation
        .mint_authority
        .ok_or(Refusal::Adapter(ClutchError::MintNotAdmitted))?;
    Ok(AdapterBearerClaimObservationV3 {
        mint: CollateralId::from_bytes(mint.key.to_bytes()),
        mint_authority: CollateralId::from_bytes(mint_authority),
        source_token_account: CollateralId::from_bytes(source.key.to_bytes()),
        source_owner: CollateralId::from_bytes(source_observation.owner),
        mint_supply_atoms: mint_observation.supply,
        source_atoms: source_observation.amount,
    })
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
fn invoke_claim_collateral_payout<'a>(
    prepared: PreparedClaimRedemptionCollateralV2,
    mint: &AccountInfo<'a>,
    hoard: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer: &[&[u8]],
) -> Outcome<clutch_collateral_adapter_v2::AcceptedClaimRedemptionCollateralV2> {
    let cpi = prepared.cpi();
    require(
        cpi.program_signed
            && cpi.token_program == CollateralId::from_bytes(token_program.key.to_bytes())
            && cpi.accounts[0].address == CollateralId::from_bytes(hoard.key.to_bytes())
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
        hoard.clone(),
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
    let hoard_after = hoard
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_claim_redemption_collateral_v2(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(hoard, &hoard_after),
        runtime_account_view(destination, &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

fn accept_zero_claim_collateral_payout(
    prepared: PreparedZeroClaimRedemptionCollateralV2,
    mint: &AccountInfo<'_>,
    hoard: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
) -> Outcome<clutch_collateral_adapter_v2::AcceptedZeroClaimRedemptionCollateralV2> {
    let mint_after = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_after = hoard
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_zero_claim_redemption_collateral_v2(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(hoard, &hoard_after),
        runtime_account_view(destination, &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// Execute one exact whole-atom V5 bearer redemption.
pub fn process_external_redemption_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &Request,
) -> Outcome<()> {
    let request = decode_request(request)?;
    require(
        accounts.len() >= EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[ix::CLAIMANT])?;
    require(
        !accounts[ix::CLAIMANT].is_writable
            && request.claimant.bytes() == accounts[ix::CLAIMANT].key.to_bytes()
            && request.source.bytes() == accounts[ix::SOURCE].key.to_bytes()
            && request.destination.bytes() == accounts[ix::DESTINATION].key.to_bytes(),
        ClutchError::UnauthorizedActor,
    )?;

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[ix::REALM],
        &accounts[ix::PROFILE],
        &accounts[ix::POLICY],
        &accounts[ix::COLLATERAL_TOKEN_PROGRAM],
        &accounts[ix::MARKET_BINDING],
        &accounts[ix::MARKET_RUNTIME],
        &accounts[ix::MARKET_INSTANCE],
        &accounts[ix::HOARD],
        &accounts[ix::CLAIM_LEDGER],
        true,
        true,
    )?;
    let expected_count = EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3
        .checked_add(usize::from(liabilities.market_binding.outcome_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() == expected_count, ClutchError::AccountCount)?;
    validate_full_width_collateral_accounts_v3(
        accounts,
        CollateralActionV3::RedeemExternal,
        liabilities.hoard.outcome_count,
        Some(request.outcome),
    )?;
    require_distinct_roles(accounts)?;
    require(
        request.market_instance_id.bytes()
            == liabilities.market_binding.market_instance_v2_id.bytes()
            && request.outcome < liabilities.market_binding.outcome_count
            && accounts[ix::COLLATERAL_MINT].key.to_bytes()
                == liabilities.bound.policy().mint.bytes()
            && accounts[ix::HOARD_TOKEN].key.to_bytes() == liabilities.hoard.token_account.bytes()
            && accounts[ix::HOARD_AUTHORITY].key.to_bytes() == liabilities.hoard.authority.bytes()
            && !accounts[ix::HOARD_AUTHORITY].is_writable
            && !accounts[ix::HOARD_AUTHORITY].executable
            && accounts[ix::HOARD_AUTHORITY].data_is_empty(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[ix::HOARD_AUTHORITY].key,
        seeds::hoard_authority_v2_pda(program_id, &request.market_instance_id.bytes()),
        None,
    )?;
    expect_pda(
        accounts[ix::HOARD_TOKEN].key,
        seeds::hoard_token_v2_pda(program_id, &request.market_instance_id.bytes()),
        None,
    )?;
    let resolution =
        authenticate_resolution_v5(program_id, &accounts[ix::RESOLUTION], liabilities)?;
    let claim =
        authenticate_claim_issuance_v1(liabilities.bound, &accounts[ix::OUTCOME_TOKEN_PROGRAM])?;
    let observed_before = observe_mints(
        program_id,
        accounts,
        request.market_instance_id.bytes(),
        liabilities.market_binding.outcome_count,
        request.outcome,
    )?;
    let token_before = bearer_observation(accounts, request.outcome)?;
    let prepared = prepare_bearer_claim_redemption_v3(
        claim,
        resolution.account_id,
        resolution.resolution,
        CollateralId::from_bytes(accounts[ix::MARKET_RUNTIME].key.to_bytes()),
        liabilities.hoard,
        liabilities.claim_ledger,
        CollateralId::from_bytes(accounts[ix::CLAIMANT].key.to_bytes()),
        CollateralId::from_bytes(accounts[ix::DESTINATION].key.to_bytes()),
        request.outcome,
        request.quantity,
        observed_before.values,
        token_before,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let burn = prepared.burn_intent();
    let selected_mint = &accounts[ix::OUTCOME_MINTS + usize::from(request.outcome)];
    require(
        burn.mint == CollateralId::from_bytes(selected_mint.key.to_bytes())
            && burn.source_token_account
                == CollateralId::from_bytes(accounts[ix::SOURCE].key.to_bytes())
            && burn.claimant == CollateralId::from_bytes(accounts[ix::CLAIMANT].key.to_bytes())
            && burn.quantity == request.quantity,
        ClutchError::MismatchedState,
    )?;
    token::burn(
        &accounts[ix::OUTCOME_TOKEN_PROGRAM],
        &accounts[ix::SOURCE],
        selected_mint,
        &accounts[ix::CLAIMANT],
        request.quantity,
    )?;
    let observed_after = observe_mints(
        program_id,
        accounts,
        request.market_instance_id.bytes(),
        liabilities.market_binding.outcome_count,
        request.outcome,
    )?;
    let token_after = bearer_observation(accounts, request.outcome)?;
    let accepted_burn = accept_bearer_claim_burn_v3(prepared, observed_after.values, token_after)
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    let collateral_request = accepted_burn.collateral_request();
    let collateral = {
        let mint_data = accounts[ix::COLLATERAL_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data = accounts[ix::HOARD_TOKEN]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination_data = accounts[ix::DESTINATION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        if collateral_request.payout_atoms == 0 {
            let prepared = prepare_zero_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                runtime_account_view(&accounts[ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            AcceptedBearerRedemptionCollateralV3::Zero(accept_zero_claim_collateral_payout(
                prepared,
                &accounts[ix::COLLATERAL_MINT],
                &accounts[ix::HOARD_TOKEN],
                &accounts[ix::DESTINATION],
            )?)
        } else {
            let prepared = prepare_claim_redemption_collateral_v2(
                liabilities.bound,
                collateral_request,
                TransferAuthorityV2 {
                    address: CollateralId::from_bytes(accounts[ix::HOARD_AUTHORITY].key.to_bytes()),
                    kind: TransferAuthorityKindV2::ProgramDerived,
                    is_transaction_signer: false,
                    program_address_authenticated: true,
                    is_writable: accounts[ix::HOARD_AUTHORITY].is_writable,
                    executable: accounts[ix::HOARD_AUTHORITY].executable,
                    data_is_empty: accounts[ix::HOARD_AUTHORITY].data_is_empty(),
                },
                runtime_account_view(&accounts[ix::COLLATERAL_MINT], &mint_data),
                runtime_account_view(&accounts[ix::HOARD_TOKEN], &hoard_data),
                runtime_account_view(&accounts[ix::DESTINATION], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            drop((mint_data, hoard_data, destination_data));
            let market_bytes = request.market_instance_id.bytes();
            let bump = [seeds::hoard_authority_v2_pda(program_id, &market_bytes).1];
            let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
            AcceptedBearerRedemptionCollateralV3::Nonzero(invoke_claim_collateral_payout(
                prepared,
                &accounts[ix::COLLATERAL_MINT],
                &accounts[ix::HOARD_TOKEN],
                &accounts[ix::DESTINATION],
                &accounts[ix::HOARD_AUTHORITY],
                &accounts[ix::COLLATERAL_TOKEN_PROGRAM],
                &signer,
            )?)
        }
    };
    let accepted = accept_bearer_claim_redemption_v3(accepted_burn, collateral)
        .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    accepted
        .claim_ledger_after()
        .encode(
            &mut accounts[ix::CLAIM_LEDGER]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    accepted
        .hoard_after()
        .encode(
            &mut accounts[ix::HOARD]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}
