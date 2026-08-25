//! Unified physical adapter for immutable claim-representation descriptors.

use dclutch_claims_representation_codec::{
    ActionV1, AdapterMutation, ClaimsReleaseAdmission, DescriptorV1, EconomicIntent, EconomicPhase,
    StateV1, prepare,
};
use dclutch_claims_svm::ClaimsPositionSeedsV1;
use dclutch_economic_slice_kernel::{
    BasketAction, BasketFrame, Phase, execute_basket, market_identity, market_outcome_count,
    market_phase, market_registry_program, market_release_set_id, market_revision,
    position_market_id, position_materialized, position_native, position_owner, position_revision,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_token_svm::{
    COption, ExactTransferProfileV1, MINT_BYTES, Mint, TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use solana_program::{
    account_info::AccountInfo,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_token_2022_interface::{
    extension::{ExtensionType, permissioned_burn},
    instruction::{self as token_instruction, AuthorityType},
};

use super::{
    ClaimsSbfError, REPRESENTATION_ACCOUNT_COUNT, REPRESENTATION_STATE_SEED_V1,
    RepresentationAccounts, authenticate_core_market, phases_join, reauthenticate,
};

const MINT_PADDING_START: usize = MINT_BYTES;
const MINT_ACCOUNT_TYPE_OFFSET: usize = 165;
const MINT_TLV_START: usize = 166;
const TLV_HEADER_BYTES: usize = 4;
const AUTHORITY_BYTES: usize = 32;

#[derive(Clone, Copy)]
struct RepresentationMint {
    base: Mint,
    close_authority: [u8; 32],
    permissioned_burn_authority: [u8; 32],
}

pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    action: ActionV1,
) -> Result<(), ProgramError> {
    if account_infos.len() != REPRESENTATION_ACCOUNT_COUNT {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let accounts = RepresentationAccounts::parse(account_infos)?;
    authenticate_privileges(program_id, &accounts)?;

    let descriptor_data = accounts
        .descriptor
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let descriptor =
        DescriptorV1::decode(&descriptor_data).map_err(|_| ClaimsSbfError::Representation)?;
    authenticate_descriptor(program_id, &accounts, descriptor, action)?;

    let state_data = accounts
        .state
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let state = StateV1::decode(&state_data).map_err(|_| ClaimsSbfError::Representation)?;
    drop(state_data);

    let (phase, market_revision_before, claimant_revision_before, wrapper_revision_before) =
        authenticate_economic_state(program_id, &accounts, descriptor, state)?;
    let release = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Claims,
        accounts.claims_program,
        accounts.claims_programdata,
    )?;
    let core_release = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )?;
    if release.execution_release_set_id().as_bytes() != &descriptor.release_set_id()
        || core_release.execution_release_set_id().as_bytes() != &descriptor.release_set_id()
    {
        return Err(ClaimsSbfError::Release.into());
    }
    let prepared = prepare(
        descriptor,
        state,
        action,
        economic_phase(phase),
        ClaimsReleaseAdmission {
            selected_release_set_id: descriptor.release_set_id(),
            receipt_release_set_id: *release.execution_release_set_id().as_bytes(),
            registry_authenticated: true,
            claims_role_authenticated: true,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;

    let state_seeds = state_seeds(program_id, accounts.descriptor.key, accounts.state.key)?;
    let mint_before = parse_mint(accounts.mint, accounts.state.key, true)?;
    let holder_before = parse_holder(&accounts, descriptor)?;
    authenticate_token_conservation(descriptor, state, mint_before, holder_before)?;

    let terminal = prepared
        .economic_intents()
        .any(|intent| matches!(intent, EconomicIntent::RedeemTerminal { .. }));
    execute_economics(
        &accounts,
        descriptor,
        prepared.adapter_mutation(),
        terminal,
        market_revision_before,
        claimant_revision_before,
        wrapper_revision_before,
        action.lots,
    )?;
    execute_token_mutation(
        &accounts,
        prepared.adapter_mutation(),
        &state_seeds.as_signer_seeds(),
    )?;

    authenticate_postconditions(
        &accounts,
        descriptor,
        prepared.post_state(),
        prepared.adapter_mutation(),
        mint_before,
        holder_before,
    )?;
    let encoded = prepared
        .post_state()
        .encode()
        .map_err(|_| ClaimsSbfError::Representation)?;
    let mut output = accounts
        .state
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if output.len() != encoded.len() {
        return Err(ClaimsSbfError::Accounts.into());
    }
    output.copy_from_slice(&encoded);
    drop(output);
    set_return_data(&encoded);
    Ok(())
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &RepresentationAccounts<'_, '_>,
) -> Result<(), ProgramError> {
    if !accounts.claimant.is_signer
        || accounts.claimant.is_writable
        || accounts.claimant.executable
        || accounts.descriptor.is_signer
        || accounts.descriptor.is_writable
        || accounts.descriptor.executable
        || !accounts.state.is_writable
        || accounts.state.is_signer
        || accounts.state.executable
        || !accounts.market.is_writable
        || accounts.market.is_signer
        || accounts.market.executable
        || !accounts.claimant_position.is_writable
        || accounts.claimant_position.is_signer
        || accounts.claimant_position.executable
        || !accounts.wrapper_position.is_writable
        || accounts.wrapper_position.is_signer
        || accounts.wrapper_position.executable
        || accounts.cache.is_writable
        || accounts.cache.is_signer
        || accounts.cache.executable
        || !accounts.claims_program.executable
        || accounts.claims_program.is_writable
        || accounts.claims_program.is_signer
        || accounts.claims_program.key != program_id
        || accounts.claims_programdata.is_writable
        || accounts.claims_programdata.is_signer
        || accounts.claims_programdata.executable
        || !accounts.registry.executable
        || accounts.registry.is_writable
        || accounts.registry.is_signer
        || !accounts.mint.is_writable
        || accounts.mint.is_signer
        || accounts.mint.executable
        || !accounts.holder_token.is_writable
        || accounts.holder_token.is_signer
        || accounts.holder_token.executable
        || !accounts.token_program.executable
        || accounts.token_program.is_writable
        || accounts.token_program.is_signer
        || accounts.token_program.key.to_bytes() != TOKEN_2022_PROGRAM_ID
        || accounts.core_market.is_writable
        || accounts.core_market.is_signer
        || accounts.core_market.executable
        || !accounts.core_program.executable
        || accounts.core_program.is_writable
        || accounts.core_program.is_signer
        || accounts.core_programdata.is_writable
        || accounts.core_programdata.is_signer
        || accounts.core_programdata.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for owned in [
        accounts.descriptor,
        accounts.state,
        accounts.market,
        accounts.claimant_position,
        accounts.wrapper_position,
    ] {
        if owned.owner != program_id {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    if accounts.mint.owner != accounts.token_program.key
        || accounts.holder_token.owner != accounts.token_program.key
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn authenticate_descriptor(
    program_id: &Pubkey,
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
    action: ActionV1,
) -> Result<(), ProgramError> {
    let (expected_state, _) = Pubkey::find_program_address(
        &[
            REPRESENTATION_STATE_SEED_V1,
            accounts.descriptor.key.as_ref(),
        ],
        program_id,
    );
    if accounts.descriptor.key.to_bytes() != descriptor.descriptor_id()
        || accounts.state.key != &expected_state
        || accounts.mint.key.to_bytes() != descriptor.adapter_asset_id()
        || accounts.claimant.key.to_bytes() != action.claimant
        || action.descriptor_id != descriptor.descriptor_id()
        || action.expected_release_set_id != descriptor.release_set_id()
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(())
}

fn authenticate_economic_state(
    program_id: &Pubkey,
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
    state: StateV1,
) -> Result<(Phase, u64, u64, u64), ProgramError> {
    let core = authenticate_core_market(
        program_id,
        accounts.core_market,
        accounts.core_program,
        accounts.market,
        descriptor.market_id(),
        descriptor.release_set_id(),
    )?;
    if core.identity.product_id.to_bytes() != descriptor.product_id()
        || core.identity.result_domain.to_bytes() != descriptor.result_domain_id()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let market = accounts
        .market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if market_identity(&market).map_err(|_| ClaimsSbfError::Economic)? != descriptor.market_id()
        || market_release_set_id(&market).map_err(|_| ClaimsSbfError::Economic)?
            != descriptor.release_set_id()
        || market_registry_program(&market).map_err(|_| ClaimsSbfError::Economic)?
            != accounts.registry.key.to_bytes()
        || market_outcome_count(&market).map_err(|_| ClaimsSbfError::Economic)?
            != descriptor.outcome_count()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let phase = market_phase(&market).map_err(|_| ClaimsSbfError::Economic)?;
    if !phases_join(core.phase, core.terminal_winner, phase) {
        return Err(ClaimsSbfError::Identity.into());
    }
    let market_revision = market_revision(&market).map_err(|_| ClaimsSbfError::Economic)?;
    drop(market);

    let claimant_revision = authenticate_position(
        accounts.claimant_position,
        program_id,
        descriptor,
        accounts.claimant.key.to_bytes(),
    )?;
    let wrapper_revision = authenticate_position(
        accounts.wrapper_position,
        program_id,
        descriptor,
        accounts.state.key.to_bytes(),
    )?;
    authenticate_wrapper_projection(accounts, descriptor, state.issued_lots)?;
    Ok((phase, market_revision, claimant_revision, wrapper_revision))
}

fn authenticate_position(
    account: &AccountInfo<'_>,
    program_id: &Pubkey,
    descriptor: DescriptorV1<'_>,
    expected_owner: [u8; 32],
) -> Result<u64, ProgramError> {
    if account.owner != program_id {
        return Err(ClaimsSbfError::Identity.into());
    }
    let seeds = ClaimsPositionSeedsV1::new(descriptor.market_id(), expected_owner)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if account.key != &expected {
        return Err(ClaimsSbfError::Identity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if position_market_id(&data, descriptor.outcome_count())
        .map_err(|_| ClaimsSbfError::Economic)?
        != descriptor.market_id()
        || position_owner(&data, descriptor.outcome_count())
            .map_err(|_| ClaimsSbfError::Economic)?
            != expected_owner
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    position_revision(&data, descriptor.outcome_count())
        .map_err(|_| ClaimsSbfError::Economic.into())
}

fn authenticate_wrapper_projection(
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
    issued_lots: u64,
) -> Result<(), ProgramError> {
    let data = accounts
        .wrapper_position
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mut outcome = 0_u32;
    while outcome < descriptor.outcome_count() {
        let expected = descriptor
            .claim_atoms_per_lot(outcome)
            .map_err(|_| ClaimsSbfError::Representation)?
            .checked_mul(issued_lots)
            .ok_or(ClaimsSbfError::Representation)?;
        if position_native(&data, descriptor.outcome_count(), outcome)
            .map_err(|_| ClaimsSbfError::Economic)?
            != 0
            || position_materialized(&data, descriptor.outcome_count(), outcome)
                .map_err(|_| ClaimsSbfError::Economic)?
                != expected
        {
            return Err(ClaimsSbfError::Representation.into());
        }
        outcome = outcome
            .checked_add(1)
            .ok_or(ClaimsSbfError::Representation)?;
    }
    Ok(())
}

fn economic_phase(phase: Phase) -> EconomicPhase {
    match phase {
        Phase::Open => EconomicPhase::Open,
        Phase::Terminal(_) => EconomicPhase::Terminal,
        Phase::Retiring(_) => EconomicPhase::Retiring,
        Phase::Retired => EconomicPhase::Retired,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_economics(
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
    mutation: AdapterMutation,
    terminal: bool,
    market_revision: u64,
    claimant_revision: u64,
    wrapper_revision: u64,
    lots: u64,
) -> Result<(), ProgramError> {
    let action = match mutation {
        AdapterMutation::Mint { .. } => BasketAction::Materialize,
        AdapterMutation::Burn { .. } if !terminal => BasketAction::Dematerialize,
        AdapterMutation::Burn { .. } => return Err(ClaimsSbfError::CustodyRequired.into()),
        AdapterMutation::Retire => return Ok(()),
    };
    let (source, destination, expected_source, expected_destination) = match action {
        BasketAction::Materialize => (
            accounts.claimant_position,
            Some(accounts.wrapper_position),
            claimant_revision,
            Some(wrapper_revision),
        ),
        BasketAction::Dematerialize => (
            accounts.wrapper_position,
            Some(accounts.claimant_position),
            wrapper_revision,
            Some(claimant_revision),
        ),
        BasketAction::RedeemMaterializedTerminal => {
            (accounts.wrapper_position, None, wrapper_revision, None)
        }
        _ => return Err(ClaimsSbfError::Representation.into()),
    };
    let mut market = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mut source_data = source
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let frame = BasketFrame {
        expected_market_revision: market_revision,
        expected_source_revision: Some(expected_source),
        expected_destination_revision: expected_destination,
        action,
        quantities: descriptor.claim_atoms_bytes(),
        quantity_multiplier: lots,
    };
    if let Some(destination) = destination {
        let mut destination_data = destination
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        execute_basket(
            &mut market,
            Some(&mut source_data),
            Some(&mut destination_data),
            frame,
        )
    } else {
        execute_basket(&mut market, Some(&mut source_data), None, frame)
    }
    .map_err(|_| ClaimsSbfError::Economic)?;
    Ok(())
}

fn parse_mint(
    account: &AccountInfo<'_>,
    state: &Pubkey,
    require_mint_authority: bool,
) -> Result<RepresentationMint, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if data.len() != super::REPRESENTATION_MINT_BYTES_V1
        || data.get(MINT_PADDING_START..MINT_ACCOUNT_TYPE_OFFSET) != Some(&[0; 83])
        || data.get(MINT_ACCOUNT_TYPE_OFFSET).copied() != Some(1)
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let base = Mint::parse(data.get(..MINT_BYTES).ok_or(ClaimsSbfError::Token)?)
        .map_err(|_| ClaimsSbfError::Token)?;
    if !base.is_initialized
        || base.decimals != 0
        || !base.freeze_authority.is_none()
        || (require_mint_authority && base.mint_authority != COption::Some(state.to_bytes()))
        || (!require_mint_authority && !base.mint_authority.is_none())
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let mut close = None;
    let mut burn = None;
    let mut offset = MINT_TLV_START;
    while offset < data.len() {
        let kind = u16_at(&data, offset)?;
        let length = usize::from(u16_at(
            &data,
            offset.checked_add(2).ok_or(ClaimsSbfError::Token)?,
        )?);
        if length != AUTHORITY_BYTES {
            return Err(ClaimsSbfError::Token.into());
        }
        let value_offset = offset
            .checked_add(TLV_HEADER_BYTES)
            .ok_or(ClaimsSbfError::Token)?;
        let next = value_offset
            .checked_add(length)
            .ok_or(ClaimsSbfError::Token)?;
        let authority: [u8; 32] = data
            .get(value_offset..next)
            .ok_or(ClaimsSbfError::Token)?
            .try_into()
            .map_err(|_| ClaimsSbfError::Token)?;
        match kind {
            value
                if value == ExtensionType::MintCloseAuthority as u16
                    && close.replace(authority).is_none() => {}
            value
                if value == ExtensionType::PermissionedBurn as u16
                    && burn.replace(authority).is_none() => {}
            _ => return Err(ClaimsSbfError::Token.into()),
        }
        offset = next;
    }
    let close_authority = close.ok_or(ClaimsSbfError::Token)?;
    let permissioned_burn_authority = burn.ok_or(ClaimsSbfError::Token)?;
    if close_authority != state.to_bytes() || permissioned_burn_authority != state.to_bytes() {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(RepresentationMint {
        base,
        close_authority,
        permissioned_burn_authority,
    })
}

fn parse_holder(
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
) -> Result<TokenAccount, ProgramError> {
    ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1
        .check_custody_account(
            accounts.token_program.key.to_bytes(),
            &accounts
                .holder_token
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?,
            descriptor.adapter_asset_id(),
            accounts.claimant.key.to_bytes(),
        )
        .map_err(|_| ClaimsSbfError::Token.into())
}

fn authenticate_token_conservation(
    descriptor: DescriptorV1<'_>,
    state: StateV1,
    mint: RepresentationMint,
    holder: TokenAccount,
) -> Result<(), ProgramError> {
    let expected_supply = descriptor
        .receipt_units_per_lot()
        .checked_mul(state.issued_lots)
        .ok_or(ClaimsSbfError::Token)?;
    if mint.base.supply != expected_supply || holder.amount > mint.base.supply {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

fn execute_token_mutation(
    accounts: &RepresentationAccounts<'_, '_>,
    mutation: AdapterMutation,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let instruction = match mutation {
        AdapterMutation::Mint { receipt_units, .. } => token_instruction::mint_to_checked(
            accounts.token_program.key,
            accounts.mint.key,
            accounts.holder_token.key,
            accounts.state.key,
            &[],
            receipt_units,
            0,
        ),
        AdapterMutation::Burn { receipt_units, .. } => {
            permissioned_burn::instruction::burn_checked(
                accounts.token_program.key,
                accounts.holder_token.key,
                accounts.mint.key,
                accounts.state.key,
                accounts.claimant.key,
                &[],
                receipt_units,
                0,
            )
        }
        AdapterMutation::Retire => token_instruction::set_authority(
            accounts.token_program.key,
            accounts.mint.key,
            None,
            AuthorityType::MintTokens,
            accounts.state.key,
            &[],
        ),
    }
    .map_err(|_| ClaimsSbfError::Token)?;
    let infos: &[AccountInfo<'_>] = match mutation {
        AdapterMutation::Mint { .. } => &[
            accounts.mint.clone(),
            accounts.holder_token.clone(),
            accounts.state.clone(),
            accounts.token_program.clone(),
        ],
        AdapterMutation::Burn { .. } => &[
            accounts.holder_token.clone(),
            accounts.mint.clone(),
            accounts.state.clone(),
            accounts.claimant.clone(),
            accounts.token_program.clone(),
        ],
        AdapterMutation::Retire => &[
            accounts.mint.clone(),
            accounts.state.clone(),
            accounts.token_program.clone(),
        ],
    };
    invoke_signed(&instruction, infos, &[signer_seeds]).map_err(|_| ClaimsSbfError::Token.into())
}

fn authenticate_postconditions(
    accounts: &RepresentationAccounts<'_, '_>,
    descriptor: DescriptorV1<'_>,
    post_state: StateV1,
    mutation: AdapterMutation,
    mint_before: RepresentationMint,
    holder_before: TokenAccount,
) -> Result<(), ProgramError> {
    let retired = matches!(mutation, AdapterMutation::Retire);
    let mint_after = parse_mint(accounts.mint, accounts.state.key, !retired)?;
    let holder_after = parse_holder(accounts, descriptor)?;
    let units = match mutation {
        AdapterMutation::Mint { receipt_units, .. }
        | AdapterMutation::Burn { receipt_units, .. } => receipt_units,
        AdapterMutation::Retire => 0,
    };
    let expected_supply = match mutation {
        AdapterMutation::Mint { .. } => mint_before.base.supply.checked_add(units),
        AdapterMutation::Burn { .. } => mint_before.base.supply.checked_sub(units),
        AdapterMutation::Retire => Some(mint_before.base.supply),
    }
    .ok_or(ClaimsSbfError::Token)?;
    let expected_holder = match mutation {
        AdapterMutation::Mint { .. } => holder_before.amount.checked_add(units),
        AdapterMutation::Burn { .. } => holder_before.amount.checked_sub(units),
        AdapterMutation::Retire => Some(holder_before.amount),
    }
    .ok_or(ClaimsSbfError::Token)?;
    if mint_after.base.supply != expected_supply
        || holder_after.amount != expected_holder
        || mint_after.close_authority != mint_before.close_authority
        || mint_after.permissioned_burn_authority != mint_before.permissioned_burn_authority
    {
        return Err(ClaimsSbfError::Token.into());
    }
    authenticate_token_conservation(descriptor, post_state, mint_after, holder_after)?;
    authenticate_wrapper_projection(accounts, descriptor, post_state.issued_lots)
}

struct StateSeeds<'a> {
    descriptor: &'a [u8],
    bump: [u8; 1],
}

impl StateSeeds<'_> {
    fn as_signer_seeds(&self) -> [&[u8]; 3] {
        [REPRESENTATION_STATE_SEED_V1, self.descriptor, &self.bump]
    }
}

fn state_seeds<'a>(
    program_id: &Pubkey,
    descriptor: &'a Pubkey,
    state: &Pubkey,
) -> Result<StateSeeds<'a>, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(
        &[REPRESENTATION_STATE_SEED_V1, descriptor.as_ref()],
        program_id,
    );
    if state != &expected {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(StateSeeds {
        descriptor: descriptor.as_ref(),
        bump: [bump],
    })
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ProgramError> {
    let end = offset.checked_add(2).ok_or(ClaimsSbfError::Token)?;
    let value: [u8; 2] = bytes
        .get(offset..end)
        .ok_or(ClaimsSbfError::Token)?
        .try_into()
        .map_err(|_| ClaimsSbfError::Token)?;
    Ok(u16::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_claims_representation_codec::StateV1;
    use dclutch_economic_slice_kernel::{
        MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, SCALAR_BYTES, initialize_market,
        initialize_position,
    };

    use super::*;

    fn account(key: Pubkey, owner: Pubkey, data: Vec<u8>) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            true,
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn descriptor_bytes() -> Vec<u8> {
        let mut bytes = vec![0_u8; 240];
        bytes
            .get_mut(..8)
            .expect("fixed fixture")
            .copy_from_slice(b"DCLWRPD1");
        bytes
            .get_mut(8..10)
            .expect("fixed fixture")
            .copy_from_slice(&1_u16.to_le_bytes());
        for (start, value) in [(16, 1), (48, 2), (80, 3), (112, 4), (144, 5), (176, 6)] {
            bytes
                .get_mut(start..start + 32)
                .expect("fixed fixture")
                .fill(value);
        }
        bytes
            .get_mut(208..212)
            .expect("fixed fixture")
            .copy_from_slice(&2_u32.to_le_bytes());
        bytes
            .get_mut(216..224)
            .expect("fixed fixture")
            .copy_from_slice(&10_u64.to_le_bytes());
        bytes
            .get_mut(224..232)
            .expect("fixed fixture")
            .copy_from_slice(&2_u64.to_le_bytes());
        bytes
            .get_mut(232..240)
            .expect("fixed fixture")
            .copy_from_slice(&3_u64.to_le_bytes());
        bytes
    }

    #[test]
    fn wrapper_projection_accepts_runtime_descriptor_without_fixed_width()
    -> Result<(), ProgramError> {
        let program = Pubkey::new_from_array([9; 32]);
        let descriptor_wire = descriptor_bytes();
        let descriptor =
            DescriptorV1::decode(&descriptor_wire).map_err(|_| ClaimsSbfError::Representation)?;
        let mut wrapper = vec![0_u8; POSITION_HEADER_BYTES + 2 * 2 * SCALAR_BYTES];
        initialize_position(&mut wrapper, descriptor.market_id(), [7; 32], 2)
            .map_err(|_| ClaimsSbfError::Economic)?;
        let mut market = vec![0_u8; MARKET_HEADER_BYTES + 2 * 3 * SCALAR_BYTES];
        initialize_market(
            &mut market,
            descriptor.market_id(),
            [6; 32],
            [8; 32],
            2,
            Phase::Open,
            0,
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
        let mut claimant = vec![0_u8; POSITION_HEADER_BYTES + 2 * 2 * SCALAR_BYTES];
        initialize_position(&mut claimant, descriptor.market_id(), [9; 32], 2)
            .map_err(|_| ClaimsSbfError::Economic)?;
        let quantities = descriptor.claim_atoms_bytes();
        execute_basket(
            &mut market,
            Some(&mut claimant),
            Some(&mut wrapper),
            BasketFrame {
                expected_market_revision: 0,
                expected_source_revision: Some(0),
                expected_destination_revision: Some(0),
                action: BasketAction::Materialize,
                quantities,
                quantity_multiplier: 0,
            },
        )
        .expect_err("zero lots are refused before mutation");
        let wrapper_account = account(Pubkey::new_from_array([7; 32]), program, wrapper);
        let accounts = RepresentationAccounts {
            claimant: &account(Pubkey::new_from_array([9; 32]), program, Vec::new()),
            descriptor: &account(Pubkey::new_from_array([1; 32]), program, descriptor_bytes()),
            state: &account(Pubkey::new_from_array([7; 32]), program, Vec::new()),
            market: &account(Pubkey::new_from_array([2; 32]), program, market),
            claimant_position: &account(Pubkey::new_unique(), program, claimant),
            wrapper_position: &wrapper_account,
            cache: &account(Pubkey::new_unique(), program, Vec::new()),
            claims_program: &account(program, program, Vec::new()),
            claims_programdata: &account(Pubkey::new_unique(), program, Vec::new()),
            registry: &account(Pubkey::new_from_array([8; 32]), program, Vec::new()),
            mint: &account(Pubkey::new_from_array([5; 32]), program, Vec::new()),
            holder_token: &account(Pubkey::new_unique(), program, Vec::new()),
            token_program: &account(Pubkey::new_unique(), program, Vec::new()),
            core_market: &account(Pubkey::new_unique(), program, Vec::new()),
            core_program: &account(Pubkey::new_unique(), program, Vec::new()),
            core_programdata: &account(Pubkey::new_unique(), program, Vec::new()),
        };
        authenticate_wrapper_projection(&accounts, descriptor, 0)
    }

    #[test]
    fn wrapper_projection_refuses_hidden_materialized_claim() -> Result<(), ProgramError> {
        let program = Pubkey::new_from_array([9; 32]);
        let descriptor_bytes = descriptor_bytes();
        let descriptor =
            DescriptorV1::decode(&descriptor_bytes).map_err(|_| ClaimsSbfError::Representation)?;
        let mut wrapper = vec![0_u8; POSITION_HEADER_BYTES + 2 * 2 * SCALAR_BYTES];
        initialize_position(&mut wrapper, descriptor.market_id(), [7; 32], 2)
            .map_err(|_| ClaimsSbfError::Economic)?;
        wrapper
            .get_mut(
                POSITION_HEADER_BYTES + 2 * SCALAR_BYTES..POSITION_HEADER_BYTES + 3 * SCALAR_BYTES,
            )
            .ok_or(ClaimsSbfError::Economic)?
            .copy_from_slice(&1_u64.to_le_bytes());
        let wrapper_account = account(Pubkey::new_from_array([7; 32]), program, wrapper);
        let placeholder = account(Pubkey::new_unique(), program, Vec::new());
        let accounts = RepresentationAccounts {
            claimant: &placeholder,
            descriptor: &placeholder,
            state: &placeholder,
            market: &placeholder,
            claimant_position: &placeholder,
            wrapper_position: &wrapper_account,
            cache: &placeholder,
            claims_program: &placeholder,
            claims_programdata: &placeholder,
            registry: &placeholder,
            mint: &placeholder,
            holder_token: &placeholder,
            token_program: &placeholder,
            core_market: &placeholder,
            core_program: &placeholder,
            core_programdata: &placeholder,
        };
        assert_eq!(
            authenticate_wrapper_projection(&accounts, descriptor, 0),
            Err(ClaimsSbfError::Representation.into())
        );
        let empty = StateV1 {
            descriptor_id: descriptor.descriptor_id(),
            next_nonce: 0,
            issued_lots: 0,
            retired: false,
        };
        assert_eq!(empty.issued_lots, 0);
        Ok(())
    }
}
