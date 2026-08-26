//! Unified physical adapter for immutable claim-representation descriptors.

use dclutch_claims_representation_codec::{
    ActionV1, AdapterMutation, ClaimsReleaseAdmission, DescriptorV1, EconomicIntent, EconomicPhase,
    StateV1, prepare,
};
use dclutch_claims_svm::ClaimsPositionSeedsV1;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_economic_slice_kernel::{
    BasketAction, BasketFrame, Phase, execute_basket, market_identity, market_outcome_count,
    market_phase, market_registry_program, market_release_set_id, market_revision,
    position_market_id, position_materialized, position_native, position_owner, position_revision,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{
    COption, ExactTransferProfileV1, MINT_BYTES, Mint, TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use spl_token_2022_interface::{
    extension::{ExtensionType, permissioned_burn},
    instruction::{self as token_instruction, AuthorityType},
};

use super::{
    ClaimsSbfError, REPRESENTATION_ACCOUNT_COUNT, REPRESENTATION_COLLATERAL_MINT_ACCOUNT,
    REPRESENTATION_COLLATERAL_RECIPIENT_ACCOUNT, REPRESENTATION_COLLATERAL_TOKEN_PROGRAM_ACCOUNT,
    REPRESENTATION_CUSTODY_CALLER_AUTHORITY_ACCOUNT, REPRESENTATION_CUSTODY_PROGRAM_ACCOUNT,
    REPRESENTATION_CUSTODY_PROGRAMDATA_ACCOUNT, REPRESENTATION_CUSTODY_REPLAY_ACCOUNT,
    REPRESENTATION_CUSTODY_TRANSFER_AUTHORITY_ACCOUNT, REPRESENTATION_HOARD_VAULT_ACCOUNT,
    REPRESENTATION_REALM_ACCOUNT, REPRESENTATION_STATE_SEED_V1,
    REPRESENTATION_TERMINAL_ACCOUNT_COUNT, RepresentationAccounts, authenticate_core_market,
    phases_join, product_runtime_v2::authenticate_product_runtime_v2, reauthenticate,
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
    action_bytes: &[u8],
    custody_request_bytes: Option<&[u8]>,
) -> Result<(), ProgramError> {
    if account_infos.len() != REPRESENTATION_ACCOUNT_COUNT
        && account_infos.len() != REPRESENTATION_TERMINAL_ACCOUNT_COUNT
    {
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

    let (core, phase, market_revision_before, claimant_revision_before, wrapper_revision_before) =
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
    let expected_payout = if terminal {
        descriptor
            .claim_atoms_per_lot(core.terminal_winner)
            .map_err(|_| ClaimsSbfError::Representation)?
            .checked_mul(action.lots)
            .ok_or(ClaimsSbfError::Representation)?
    } else {
        0
    };
    authenticate_terminal_custody(
        program_id,
        account_infos,
        descriptor,
        action,
        action_bytes,
        core,
        terminal,
        expected_payout,
        custody_request_bytes,
    )?;
    let actual_payout = execute_economics(
        &accounts,
        descriptor,
        prepared.adapter_mutation(),
        terminal,
        market_revision_before,
        claimant_revision_before,
        wrapper_revision_before,
        action.lots,
    )?;
    if actual_payout != expected_payout {
        return Err(ClaimsSbfError::Economic.into());
    }
    execute_token_mutation(
        &accounts,
        prepared.adapter_mutation(),
        &state_seeds.as_signer_seeds(),
    )?;
    if terminal && expected_payout > 0 {
        invoke_terminal_custody(
            program_id,
            account_infos,
            custody_request_bytes.ok_or(ClaimsSbfError::CustodyRequired)?,
        )?;
    }

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

#[derive(Clone, Copy)]
struct TerminalCustodyAccounts<'accounts, 'info> {
    caller_authority: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    realm: &'accounts AccountInfo<'info>,
    replay: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    recipient: &'accounts AccountInfo<'info>,
    custody_authority: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_terminal_custody<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    descriptor: DescriptorV1<'_>,
    action: ActionV1,
    action_bytes: &[u8],
    core: dclutch_market_core_codec::CoreState,
    terminal: bool,
    expected_payout: u64,
    custody_request_bytes: Option<&[u8]>,
) -> Result<(), ProgramError> {
    if !terminal || expected_payout == 0 {
        if custody_request_bytes.is_some() || account_infos.len() != REPRESENTATION_ACCOUNT_COUNT {
            return Err(ClaimsSbfError::Accounts.into());
        }
        return Ok(());
    }
    if account_infos.len() != REPRESENTATION_TERMINAL_ACCOUNT_COUNT {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let request =
        CustodyRequestV1::decode(custody_request_bytes.ok_or(ClaimsSbfError::CustodyRequired)?)
            .map_err(|_| ClaimsSbfError::Instruction)?;
    let caller_authority = terminal_account(
        account_infos,
        REPRESENTATION_CUSTODY_CALLER_AUTHORITY_ACCOUNT,
    )?;
    let custody_program = terminal_account(account_infos, REPRESENTATION_CUSTODY_PROGRAM_ACCOUNT)?;
    let custody_programdata =
        terminal_account(account_infos, REPRESENTATION_CUSTODY_PROGRAMDATA_ACCOUNT)?;
    let realm = terminal_account(account_infos, REPRESENTATION_REALM_ACCOUNT)?;
    let replay = terminal_account(account_infos, REPRESENTATION_CUSTODY_REPLAY_ACCOUNT)?;
    let collateral_mint = terminal_account(account_infos, REPRESENTATION_COLLATERAL_MINT_ACCOUNT)?;
    let hoard = terminal_account(account_infos, REPRESENTATION_HOARD_VAULT_ACCOUNT)?;
    let recipient = terminal_account(account_infos, REPRESENTATION_COLLATERAL_RECIPIENT_ACCOUNT)?;
    let custody_authority = terminal_account(
        account_infos,
        REPRESENTATION_CUSTODY_TRANSFER_AUTHORITY_ACCOUNT,
    )?;
    let token_program = terminal_account(
        account_infos,
        REPRESENTATION_COLLATERAL_TOKEN_PROGRAM_ACCOUNT,
    )?;
    authenticate_terminal_privileges(
        caller_authority,
        custody_program,
        custody_programdata,
        realm,
        replay,
        collateral_mint,
        hoard,
        recipient,
        custody_authority,
        token_program,
    )?;
    let base = RepresentationAccounts::parse(account_infos)?;
    let custody_release = reauthenticate(
        base.registry,
        base.cache,
        ExecutionRoleV1::Custody,
        custody_program,
        custody_programdata,
    )?;
    if custody_release.execution_release_set_id().as_bytes() != &descriptor.release_set_id() {
        return Err(ClaimsSbfError::Release.into());
    }
    require_canonical_terminal_request(
        program_id,
        descriptor,
        action,
        action_bytes,
        core,
        hoard.key.to_bytes(),
        recipient.key.to_bytes(),
        collateral_mint.key.to_bytes(),
        token_program.key.to_bytes(),
        expected_payout,
        request,
    )?;
    let request_digest =
        hash(custody_request_bytes.ok_or(ClaimsSbfError::CustodyRequired)?).to_bytes();
    authenticate_terminal_identities(
        program_id,
        caller_authority,
        custody_program,
        replay,
        hoard,
        custody_authority,
        request,
        request_digest,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_canonical_terminal_request(
    program_id: &Pubkey,
    descriptor: DescriptorV1<'_>,
    action: ActionV1,
    action_bytes: &[u8],
    core: dclutch_market_core_codec::CoreState,
    hoard: [u8; 32],
    recipient: [u8; 32],
    collateral_mint: [u8; 32],
    token_program: [u8; 32],
    amount: u64,
    request: CustodyRequestV1,
) -> Result<(), ProgramError> {
    if request.operation != OperationV1::Transfer
        || request.caller_role != CallerRoleV1::Claims
        || request.source_compartment != CompartmentV1::HoardPrincipal
        || request.destination_compartment != CompartmentV1::External
        || request.release_set != descriptor.release_set_id()
        || request.market != descriptor.market_id()
        || request.realm != core.identity.realm_id.to_bytes()
        || request.context != descriptor.descriptor_id()
        || request.caller_program != program_id.to_bytes()
        || request.semantic
            != (ContextV1 {
                candidate: [0; 32],
                source_owner: [0; 32],
                destination_owner: action.claimant,
                order: [0; 32],
                parent_request_digest: hash(action_bytes).to_bytes(),
                order_nonce: action.expected_next_nonce,
                generation: core.identity.generation,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            })
        || request.source != hoard
        || request.destination != recipient
        || request.source_vault_context != descriptor.market_id()
        || request.destination_vault_context != [0; 32]
        || request.mint != collateral_mint
        || request.token_program != token_program
        || request.payer != [0; 32]
        || request.rent_refund != [0; 32]
        || request.amount != amount
        || request.rent_lamports != 0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn canonical_terminal_request(
    program_id: &Pubkey,
    descriptor: DescriptorV1<'_>,
    action: ActionV1,
    action_bytes: &[u8],
    core: dclutch_market_core_codec::CoreState,
    hoard: [u8; 32],
    recipient: [u8; 32],
    collateral_mint: [u8; 32],
    token_program: [u8; 32],
    amount: u64,
    expected_revision: u64,
    resulting_revision: u64,
) -> CustodyRequestV1 {
    CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: descriptor.release_set_id(),
        market: descriptor.market_id(),
        realm: core.identity.realm_id.to_bytes(),
        context: descriptor.descriptor_id(),
        caller_program: program_id.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: action.claimant,
            order: [0; 32],
            parent_request_digest: hash(action_bytes).to_bytes(),
            order_nonce: action.expected_next_nonce,
            generation: core.identity.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: hoard,
        destination: recipient,
        source_vault_context: descriptor.market_id(),
        destination_vault_context: [0; 32],
        mint: collateral_mint,
        token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision,
        amount,
        rent_lamports: 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_terminal_privileges(
    caller_authority: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    custody_programdata: &AccountInfo<'_>,
    realm: &AccountInfo<'_>,
    replay: &AccountInfo<'_>,
    collateral_mint: &AccountInfo<'_>,
    hoard: &AccountInfo<'_>,
    recipient: &AccountInfo<'_>,
    custody_authority: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if caller_authority.is_signer
        || caller_authority.is_writable
        || caller_authority.executable
        || !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || custody_programdata.executable
        || custody_programdata.is_signer
        || custody_programdata.is_writable
        || realm.executable
        || realm.is_signer
        || realm.is_writable
        || replay.executable
        || replay.is_signer
        || !replay.is_writable
        || collateral_mint.executable
        || collateral_mint.is_signer
        || collateral_mint.is_writable
        || hoard.executable
        || hoard.is_signer
        || !hoard.is_writable
        || recipient.executable
        || recipient.is_signer
        || !recipient.is_writable
        || custody_authority.executable
        || custody_authority.is_signer
        || custody_authority.is_writable
        || !token_program.executable
        || token_program.is_signer
        || token_program.is_writable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_terminal_identities(
    program_id: &Pubkey,
    caller_authority: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    replay: &AccountInfo<'_>,
    hoard: &AccountInfo<'_>,
    custody_authority: &AccountInfo<'_>,
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ClaimsSbfError::Identity)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    if caller_authority.key
        != &Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::Authority.into());
    }
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    if replay.key != &Pubkey::find_program_address(&replay_seeds.as_slices(), custody_program.key).0
        || replay.owner != custody_program.key
        || replay.data_len() != CUSTODY_REPLAY_BYTES_V1
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay_data = replay
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let replay_state =
        CustodyReplayV1::decode(&replay_data).map_err(|_| ClaimsSbfError::Identity)?;
    drop(replay_data);
    if !terminal_replay_matches(replay_state, request) {
        return Err(ClaimsSbfError::Identity.into());
    }
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, true);
    if hoard.key != &Pubkey::find_program_address(&vault_seeds.as_slices(), custody_program.key).0 {
        return Err(ClaimsSbfError::Identity.into());
    }
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    if custody_authority.key
        != &Pubkey::find_program_address(&authority_seeds.as_slices(), custody_program.key).0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn terminal_replay_matches(replay: CustodyReplayV1, request: CustodyRequestV1) -> bool {
    replay.caller_role == CallerRoleV1::Claims
        && replay.release_set == request.release_set
        && replay.market == request.market
        && replay.realm == request.realm
        && replay.context == request.context
        && replay.caller_program == request.caller_program
        && replay.next_revision == request.expected_revision
        && replay.generation == request.semantic.generation
}

#[inline(never)]
fn invoke_terminal_custody<'info>(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'info>],
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let base = RepresentationAccounts::parse(account_infos)?;
    let terminal = terminal_accounts(account_infos)?;
    let request =
        CustodyRequestV1::decode(request_bytes).map_err(|_| ClaimsSbfError::Instruction)?;
    let request_digest = hash(request_bytes).to_bytes();
    let instruction = Instruction {
        program_id: *terminal.custody_program.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*terminal.caller_authority.key, true),
            AccountMeta::new_readonly(*base.cache.key, false),
            AccountMeta::new_readonly(*base.registry.key, false),
            AccountMeta::new_readonly(*base.claims_program.key, false),
            AccountMeta::new_readonly(*base.claims_programdata.key, false),
            AccountMeta::new_readonly(*terminal.realm.key, false),
            AccountMeta::new(*terminal.replay.key, false),
            AccountMeta::new_readonly(*terminal.collateral_mint.key, false),
            AccountMeta::new(*terminal.hoard.key, false),
            AccountMeta::new(*terminal.recipient.key, false),
            AccountMeta::new_readonly(*terminal.custody_authority.key, false),
            AccountMeta::new_readonly(*terminal.token_program.key, false),
        ]),
        data: request_bytes.to_vec(),
    };
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ClaimsSbfError::Identity)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    let bump = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, request_digest_seed] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            terminal.caller_authority.clone(),
            base.cache.clone(),
            base.registry.clone(),
            base.claims_program.clone(),
            base.claims_programdata.clone(),
            terminal.realm.clone(),
            terminal.replay.clone(),
            terminal.collateral_mint.clone(),
            terminal.hoard.clone(),
            terminal.recipient.clone(),
            terminal.custody_authority.clone(),
            terminal.token_program.clone(),
            terminal.custody_program.clone(),
        ],
        &[&[
            domain,
            release,
            market,
            role,
            context,
            request_digest_seed,
            &bump_seed,
        ]],
    )
    .map_err(|_| ClaimsSbfError::CustodyRequired)?;
    verify_terminal_custody_receipt(terminal, request, request_digest)
}

fn verify_terminal_custody_receipt(
    terminal: TerminalCustodyAccounts<'_, '_>,
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let (producer, bytes) = get_return_data().ok_or(ClaimsSbfError::Receipt)?;
    if producer != *terminal.custody_program.key || bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    let receipt = CustodyReceiptV1::decode(&bytes).map_err(|_| ClaimsSbfError::Receipt)?;
    let replay_data = terminal
        .replay
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let replay_digest = hashv(&[&replay_data]).to_bytes();
    drop(replay_data);
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| ClaimsSbfError::Receipt.into())
}

fn terminal_accounts<'accounts, 'info>(
    account_infos: &'accounts [AccountInfo<'info>],
) -> Result<TerminalCustodyAccounts<'accounts, 'info>, ProgramError> {
    Ok(TerminalCustodyAccounts {
        caller_authority: terminal_account(
            account_infos,
            REPRESENTATION_CUSTODY_CALLER_AUTHORITY_ACCOUNT,
        )?,
        custody_program: terminal_account(account_infos, REPRESENTATION_CUSTODY_PROGRAM_ACCOUNT)?,
        realm: terminal_account(account_infos, REPRESENTATION_REALM_ACCOUNT)?,
        replay: terminal_account(account_infos, REPRESENTATION_CUSTODY_REPLAY_ACCOUNT)?,
        collateral_mint: terminal_account(account_infos, REPRESENTATION_COLLATERAL_MINT_ACCOUNT)?,
        hoard: terminal_account(account_infos, REPRESENTATION_HOARD_VAULT_ACCOUNT)?,
        recipient: terminal_account(account_infos, REPRESENTATION_COLLATERAL_RECIPIENT_ACCOUNT)?,
        custody_authority: terminal_account(
            account_infos,
            REPRESENTATION_CUSTODY_TRANSFER_AUTHORITY_ACCOUNT,
        )?,
        token_program: terminal_account(
            account_infos,
            REPRESENTATION_COLLATERAL_TOKEN_PROGRAM_ACCOUNT,
        )?,
    })
}

fn terminal_account<'accounts, 'info>(
    account_infos: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    account_infos
        .get(index)
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
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
) -> Result<(dclutch_market_core_codec::CoreState, Phase, u64, u64, u64), ProgramError> {
    let core = authenticate_core_market(
        program_id,
        accounts.core_market,
        accounts.core_program,
        accounts.market,
        descriptor.market_id(),
        descriptor.release_set_id(),
    )?;
    let rent = Rent::get().map_err(|_| ClaimsSbfError::Accounts)?;
    let product = authenticate_product_runtime_v2(
        accounts.registry.key,
        &rent,
        core.identity.product_record.to_bytes(),
        None,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_domain_record,
                staging: accounts.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
        },
    )?;
    if core.identity.product_id.to_bytes() != product.product_id.to_bytes()
        || product.product_id.to_bytes() != descriptor.product_id()
        || product.result_domain_record.content_digest.to_bytes() != descriptor.result_domain_id()
        || product.outcome_count != descriptor.outcome_count()
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
    Ok((
        core,
        phase,
        market_revision,
        claimant_revision,
        wrapper_revision,
    ))
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
) -> Result<u64, ProgramError> {
    let action = match mutation {
        AdapterMutation::Mint { .. } => BasketAction::Materialize,
        AdapterMutation::Burn { .. } if !terminal => BasketAction::Dematerialize,
        AdapterMutation::Burn { .. } => BasketAction::RedeemMaterializedTerminal,
        AdapterMutation::Retire => return Ok(0),
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
    let payout = if let Some(destination) = destination {
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
    Ok(payout.amount)
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

    fn identity(byte: u8) -> dclutch_market_core_codec::Identity {
        dclutch_market_core_codec::Identity::new([byte; 32]).expect("nonzero identity")
    }

    fn terminal_core() -> dclutch_market_core_codec::CoreState {
        dclutch_market_core_codec::CoreState {
            phase: dclutch_market_core_codec::Phase::Terminal,
            readiness: dclutch_market_core_codec::Readiness::Consumed,
            terminal_winner: 1,
            identity: dclutch_market_core_codec::MarketIdentity {
                market_id: identity(2),
                realm_id: identity(12),
                product_record: identity(4),
                product_id: identity(3),
                resolution_policy: identity(13),
                capability_manifest: identity(14),
                selected_release_set: identity(6),
                registry_program: identity(17),
                generation: 19,
            },
            outstanding_capabilities: 1,
            rent_beneficiary: identity(15),
            terminal_receipt: Some(identity(16)),
        }
    }

    fn terminal_action(descriptor: DescriptorV1<'_>) -> ActionV1 {
        ActionV1 {
            tag: 3,
            descriptor_id: descriptor.descriptor_id(),
            expected_release_set_id: descriptor.release_set_id(),
            claimant: [7; 32],
            expected_next_nonce: 4,
            expected_issued_lots: 2,
            lots: 1,
        }
    }

    fn require_fixture_terminal_request(
        program: &Pubkey,
        descriptor: DescriptorV1<'_>,
        action: ActionV1,
        action_bytes: &[u8],
        request: CustodyRequestV1,
    ) -> Result<(), ProgramError> {
        require_canonical_terminal_request(
            program,
            descriptor,
            action,
            action_bytes,
            terminal_core(),
            [17; 32],
            [18; 32],
            [19; 32],
            [20; 32],
            3,
            request,
        )
    }

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
            product_record: &account(Pubkey::new_unique(), program, Vec::new()),
            product_staging: &account(Pubkey::new_unique(), program, Vec::new()),
            result_domain_record: &account(Pubkey::new_unique(), program, Vec::new()),
            result_domain_staging: &account(Pubkey::new_unique(), program, Vec::new()),
            portfolio_record: &account(Pubkey::new_unique(), program, Vec::new()),
            portfolio_staging: &account(Pubkey::new_unique(), program, Vec::new()),
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
            product_record: &placeholder,
            product_staging: &placeholder,
            result_domain_record: &placeholder,
            result_domain_staging: &placeholder,
            portfolio_record: &placeholder,
            portfolio_staging: &placeholder,
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

    #[test]
    fn terminal_custody_request_binds_every_cross_owner_coordinate() -> Result<(), ProgramError> {
        let descriptor_wire = descriptor_bytes();
        let descriptor =
            DescriptorV1::decode(&descriptor_wire).map_err(|_| ClaimsSbfError::Representation)?;
        let program = Pubkey::new_from_array([9; 32]);
        let action = terminal_action(descriptor);
        let action_bytes = action
            .encode()
            .map_err(|_| ClaimsSbfError::Representation)?;
        let expected = canonical_terminal_request(
            &program,
            descriptor,
            action,
            &action_bytes,
            terminal_core(),
            [17; 32],
            [18; 32],
            [19; 32],
            [20; 32],
            3,
            8,
            9,
        );
        assert_eq!(expected.source_vault_context, descriptor.market_id());
        assert_eq!(expected.context, descriptor.descriptor_id());
        assert_eq!(
            expected.semantic.parent_request_digest,
            hash(&action_bytes).to_bytes()
        );
        assert_eq!(expected.semantic.generation, 19);
        assert_eq!(expected.amount, 3);
        assert_eq!(
            require_fixture_terminal_request(&program, descriptor, action, &action_bytes, expected,),
            Ok(())
        );

        let mut substituted = expected;
        substituted.realm = [21; 32];
        assert_eq!(
            require_fixture_terminal_request(
                &program,
                descriptor,
                action,
                &action_bytes,
                substituted,
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        substituted = expected;
        substituted.semantic.generation = 20;
        assert_eq!(
            require_fixture_terminal_request(
                &program,
                descriptor,
                action,
                &action_bytes,
                substituted,
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        substituted = expected;
        substituted.semantic.parent_request_digest = [22; 32];
        assert_eq!(
            require_fixture_terminal_request(
                &program,
                descriptor,
                action,
                &action_bytes,
                substituted,
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        substituted = expected;
        substituted.amount = 4;
        assert_eq!(
            require_fixture_terminal_request(
                &program,
                descriptor,
                action,
                &action_bytes,
                substituted,
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        substituted = expected;
        substituted.source_vault_context = descriptor.descriptor_id();
        assert_eq!(
            require_fixture_terminal_request(
                &program,
                descriptor,
                action,
                &action_bytes,
                substituted,
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        Ok(())
    }

    #[test]
    fn terminal_custody_replay_refuses_stale_or_transplanted_cursor() -> Result<(), ProgramError> {
        let descriptor_wire = descriptor_bytes();
        let descriptor =
            DescriptorV1::decode(&descriptor_wire).map_err(|_| ClaimsSbfError::Representation)?;
        let action = terminal_action(descriptor);
        let action_bytes = action
            .encode()
            .map_err(|_| ClaimsSbfError::Representation)?;
        let request = canonical_terminal_request(
            &Pubkey::new_from_array([9; 32]),
            descriptor,
            action,
            &action_bytes,
            terminal_core(),
            [17; 32],
            [18; 32],
            [19; 32],
            [20; 32],
            3,
            8,
            9,
        );
        let replay = CustodyReplayV1 {
            caller_role: CallerRoleV1::Claims,
            release_set: request.release_set,
            market: request.market,
            realm: request.realm,
            context: request.context,
            caller_program: request.caller_program,
            rent_refund: [23; 32],
            open_vault_count: 1,
            next_revision: request.expected_revision,
            generation: request.semantic.generation,
            last_request_digest: [24; 32],
            last_poststate_commitment: [25; 32],
        };
        assert!(terminal_replay_matches(replay, request));
        let mut hostile = replay;
        hostile.next_revision = hostile
            .next_revision
            .checked_sub(1)
            .ok_or(ClaimsSbfError::Representation)?;
        assert!(!terminal_replay_matches(hostile, request));
        hostile = replay;
        hostile.context = [26; 32];
        assert!(!terminal_replay_matches(hostile, request));
        hostile = replay;
        hostile.generation = hostile
            .generation
            .checked_add(1)
            .ok_or(ClaimsSbfError::Representation)?;
        assert!(!terminal_replay_matches(hostile, request));
        hostile = replay;
        hostile.caller_program = [27; 32];
        assert!(!terminal_replay_matches(hostile, request));
        Ok(())
    }
}
