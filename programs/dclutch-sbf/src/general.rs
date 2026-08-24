//! Exact-width SVM boundary for General order custody.
//!
//! Capability activation and liveness-funded batch creation intentionally remain
//! outside this module until the shared capability ledger distinguishes native
//! lamports from Realm collateral.  The routes below are independent of that
//! seam: they authenticate an existing canonical General cluster, admit signed
//! orders into Position plus quote custody, lock collection, and atomically
//! cancel or close custody into permanent RentCredit.

use alloc::vec::Vec;

use dclutch_general_contract::{
    BATCH_ROOT_BYTES, BatchPhase, BatchRootV1, GENERAL_CONFIG_BYTES, GENERAL_ROOT_BYTES,
    GeneralAccountFrameV1, GeneralAccountMetaV1, GeneralBatchPdaSeedsV1, GeneralConfigPdaSeedsV1,
    GeneralConfigV1, GeneralInstructionTagV1, GeneralInstructionV1, GeneralOrderCustodyPdaSeedsV1,
    GeneralOrderCustodyV1, GeneralOrderStatePdaSeedsV1, GeneralQuoteEscrowPdaSeedsV1,
    GeneralRootPdaSeedsV1, GeneralRootV1, ORDER_STATE_BYTES, OrderStateV1, PortfolioOrderV1,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1, RealmV1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AuthorityRole, COption, ExactTransferInput, TokenAccount, close_account,
    initialize_account3, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
    records::{REALM_SCHEMA_RELEASE_ID_V1, with_authenticated_finalized_record_v1},
};

const INSTRUCTION_WIDTH_OFFSET: usize = 11;

/// Decode the exact General width and execute one currently complete route.
#[allow(dead_code)] // Root routing is integrated after this independently owned module lands.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match instruction_data.get(INSTRUCTION_WIDTH_OFFSET).copied() {
        Some(2) => dispatch_width::<2>(program_id, accounts, instruction_data),
        Some(3) => dispatch_width::<3>(program_id, accounts, instruction_data),
        Some(4) => dispatch_width::<4>(program_id, accounts, instruction_data),
        Some(5) => dispatch_width::<5>(program_id, accounts, instruction_data),
        Some(6) => dispatch_width::<6>(program_id, accounts, instruction_data),
        Some(7) => dispatch_width::<7>(program_id, accounts, instruction_data),
        Some(8) => dispatch_width::<8>(program_id, accounts, instruction_data),
        Some(9) => dispatch_width::<9>(program_id, accounts, instruction_data),
        Some(10) => dispatch_width::<10>(program_id, accounts, instruction_data),
        Some(11) => dispatch_width::<11>(program_id, accounts, instruction_data),
        Some(12) => dispatch_width::<12>(program_id, accounts, instruction_data),
        Some(13) => dispatch_width::<13>(program_id, accounts, instruction_data),
        Some(14) => dispatch_width::<14>(program_id, accounts, instruction_data),
        Some(15) => dispatch_width::<15>(program_id, accounts, instruction_data),
        Some(16) => dispatch_width::<16>(program_id, accounts, instruction_data),
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

fn dispatch_width<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let instruction = GeneralInstructionV1::<N>::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let tag = instruction.tag();
    if !matches!(
        tag,
        GeneralInstructionTagV1::LockBatch
            | GeneralInstructionTagV1::AdmitOrder
            | GeneralInstructionTagV1::CancelOrder
            | GeneralInstructionTagV1::CloseOrder
    ) {
        return Err(AdapterError::InvalidInstruction.into());
    }
    validate_contract_frame(tag, accounts)?;
    match instruction {
        GeneralInstructionV1::LockBatch(replay) => process_lock_batch(
            program_id,
            accounts,
            replay.generation,
            replay.batch_sequence,
        ),
        GeneralInstructionV1::AdmitOrder(order) => process_admit_order(program_id, accounts, order),
        GeneralInstructionV1::CancelOrder(order) => {
            process_release_order(program_id, accounts, order, true)
        }
        GeneralInstructionV1::CloseOrder(order) => {
            process_release_order(program_id, accounts, order, false)
        }
        // These actions depend on the pending typed capability-funding physical
        // boundary or on the later candidate-settlement integration slice.
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

fn validate_contract_frame(
    tag: GeneralInstructionTagV1,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut metas = Vec::new();
    metas
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for account in accounts {
        metas.push(GeneralAccountMetaV1 {
            key: account.key.to_bytes(),
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            is_executable: account.executable,
        });
    }
    GeneralAccountFrameV1::new(tag, 0, &metas).map_err(map_general_frame_error)?;
    Ok(())
}

fn map_general_frame_error(error: dclutch_general_contract::Error) -> ProgramError {
    match error {
        dclutch_general_contract::Error::InvalidLength => AdapterError::AccountFrameLength.into(),
        dclutch_general_contract::Error::InvalidAccountPrivilege => {
            AdapterError::AccountPrivilege.into()
        }
        dclutch_general_contract::Error::AccountAlias
        | dclutch_general_contract::Error::ZeroIdentifier => AdapterError::AccountIdentity.into(),
        _ => AdapterError::InvalidInstruction.into(),
    }
}

fn process_lock_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    sequence: u64,
) -> Result<(), ProgramError> {
    let config_account = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let clock_sysvar = account(accounts, 3)?;
    let (config, config_id) = authenticate_config(program_id, config_account)?;
    let root = authenticate_root(program_id, root_account, config_id)?;
    let mut batch =
        authenticate_batch(program_id, batch_account, root_account, sequence, config_id)?;
    if generation != config.generation()
        || generation != root.generation()
        || root.config_id() != config_id
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let clock = authenticate_clock(clock_sysvar)?;
    batch
        .open_selection(clock.slot)
        .map_err(|_| AdapterError::MarketTransition)?;
    write_batch(batch_account, batch)
}

#[derive(Clone, Copy)]
struct RealmFacts {
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
    mint: dclutch_token_svm::Mint,
}

#[derive(Clone, Copy)]
struct AdmissionPlan<const N: usize> {
    state: OrderStateV1,
    custody: GeneralOrderCustodyV1<N>,
    position: PositionV1<N>,
    state_bump: u8,
    custody_bump: u8,
    escrow_bump: u8,
    state_rent: u64,
    custody_rent: u64,
    escrow_rent: u64,
    quote_atoms: u64,
    payer_before: u64,
    source_before: TokenAccount,
    realm: RealmFacts,
}

fn process_admit_order<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    order: PortfolioOrderV1<N>,
) -> Result<(), ProgramError> {
    let owner = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let realm_cursor = account(accounts, 3)?;
    let mint = account(accounts, 4)?;
    let token_program = account(accounts, 5)?;
    let config_account = account(accounts, 6)?;
    let root_account = account(accounts, 7)?;
    let batch_account = account(accounts, 8)?;
    let state_account = account(accounts, 9)?;
    let custody_account = account(accounts, 10)?;
    let position_account = account(accounts, 11)?;
    let quote_source = account(accounts, 12)?;
    let quote_escrow = account(accounts, 13)?;
    let rent_credit = account(accounts, 14)?;
    let system = account(accounts, 15)?;
    let rent_sysvar = account(accounts, 16)?;
    let clock_sysvar = account(accounts, 17)?;

    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(owner)?;
    require_vacant(state_account)?;
    require_vacant(custody_account)?;
    require_vacant(quote_escrow)?;
    let (config, config_id) = authenticate_config(program_id, config_account)?;
    let root = authenticate_root(program_id, root_account, config_id)?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        order.batch_sequence(),
        config_id,
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, config_id)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let clock = authenticate_clock(clock_sysvar)?;
    if batch.phase() != BatchPhase::Collecting
        || clock.slot >= batch.collection_close()
        || order.valid_until_slot() < batch.settlement_close()
        || order.owner().to_bytes() != owner.key.to_bytes()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_order_id(order)?;
    let rent_credit_state = authenticate_rent_credit(program_id, rent_credit, owner.key)?;
    let mut position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        owner.key,
        config.generation(),
    )?;

    let state_seeds = GeneralOrderStatePdaSeedsV1::new(market_account.key.to_bytes(), order)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_state, state_bump) =
        Pubkey::find_program_address(&state_seeds.seed_components(), program_id);
    if state_account.key != &expected_state {
        return Err(AdapterError::AccountIdentity.into());
    }
    let custody_seeds = GeneralOrderCustodyPdaSeedsV1::new(state_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_custody, custody_bump) =
        Pubkey::find_program_address(&custody_seeds.seed_components(), program_id);
    if custody_account.key != &expected_custody {
        return Err(AdapterError::AccountIdentity.into());
    }
    let escrow_seeds = GeneralQuoteEscrowPdaSeedsV1::new(custody_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_escrow, escrow_bump) =
        Pubkey::find_program_address(&escrow_seeds.seed_components(), program_id);
    if quote_escrow.key != &expected_escrow {
        return Err(AdapterError::AccountIdentity.into());
    }

    let admission = GeneralOrderCustodyV1::admit(
        order,
        config,
        rent_credit.key.to_bytes(),
        quote_escrow.key.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    for (index, amount) in admission.reserve.claim_atoms().iter().enumerate() {
        if *amount != 0 {
            position
                .debit_outcome(index, *amount)
                .map_err(|_| AdapterError::PositionAuthentication)?;
        }
    }
    let quote_atoms = admission.reserve.quote_atoms();
    let source_before =
        authenticate_quote_source(quote_source, mint, token_program, owner, realm, quote_atoms)?;
    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::PositionAuthentication)?;
    let state_rent = rent.minimum_balance(ORDER_STATE_BYTES);
    let custody_rent = rent.minimum_balance(
        GeneralOrderCustodyV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let escrow_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let total_rent = state_rent
        .checked_add(custody_rent)
        .and_then(|value| value.checked_add(escrow_rent))
        .ok_or(AdapterError::Arithmetic)?;
    let payer_before = owner.lamports();
    if payer_before < total_rent {
        return Err(AdapterError::PositionRentUnderfunded.into());
    }
    preflight_mutable(&[
        owner,
        state_account,
        custody_account,
        position_account,
        quote_source,
        quote_escrow,
    ])?;

    let plan = AdmissionPlan {
        state: admission.order_state,
        custody: admission.custody,
        position,
        state_bump,
        custody_bump,
        escrow_bump,
        state_rent,
        custody_rent,
        escrow_rent,
        quote_atoms,
        payer_before,
        source_before,
        realm,
    };
    create_order_accounts(
        program_id,
        owner,
        state_account,
        custody_account,
        quote_escrow,
        token_program,
        system,
        order,
        plan,
    )?;
    initialize_and_fund_escrow(
        quote_source,
        quote_escrow,
        mint,
        token_program,
        owner,
        custody_account,
        plan,
    )?;
    persist_admission(
        program_id,
        owner,
        state_account,
        custody_account,
        position_account,
        quote_source,
        quote_escrow,
        mint,
        token_program,
        rent_credit,
        rent_credit_state,
        plan,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_order_accounts<'info, const N: usize>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    state: &AccountInfo<'info>,
    custody: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    order: PortfolioOrderV1<N>,
    plan: AdmissionPlan<N>,
) -> Result<(), ProgramError> {
    let state_space = u64::try_from(ORDER_STATE_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let custody_space = u64::try_from(
        GeneralOrderCustodyV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    )
    .map_err(|_| AdapterError::Arithmetic)?;
    let escrow_space = u64::try_from(ACCOUNT_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let state_instruction = create_account(
        payer.key,
        state.key,
        plan.state_rent,
        state_space,
        program_id,
    );
    let state_seeds = GeneralOrderStatePdaSeedsV1::new(*plan.position.market(), order)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let state_bump = [plan.state_bump];
    let state_components = state_seeds.seed_components();
    invoke_signed(
        &state_instruction,
        &[payer.clone(), state.clone(), system.clone()],
        &[&[
            state_components[0],
            state_components[1],
            state_components[2],
            state_components[3],
            state_components[4],
            state_components[5],
            state_bump.as_slice(),
        ]],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    let custody_instruction = create_account(
        payer.key,
        custody.key,
        plan.custody_rent,
        custody_space,
        program_id,
    );
    let custody_seed = GeneralOrderCustodyPdaSeedsV1::new(state.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let custody_components = custody_seed.seed_components();
    let custody_bump = [plan.custody_bump];
    invoke_signed(
        &custody_instruction,
        &[payer.clone(), custody.clone(), system.clone()],
        &[&[
            custody_components[0],
            custody_components[1],
            custody_bump.as_slice(),
        ]],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;

    let escrow_instruction = create_account(
        payer.key,
        escrow.key,
        plan.escrow_rent,
        escrow_space,
        token_program.key,
    );
    let escrow_seed = GeneralQuoteEscrowPdaSeedsV1::new(custody.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let escrow_components = escrow_seed.seed_components();
    let escrow_bump = [plan.escrow_bump];
    invoke_signed(
        &escrow_instruction,
        &[payer.clone(), escrow.clone(), system.clone()],
        &[&[
            escrow_components[0],
            escrow_components[1],
            escrow_bump.as_slice(),
        ]],
    )
    .map_err(|_| AdapterError::VaultCreateCpi)?;
    Ok(())
}

fn initialize_and_fund_escrow<'info, const N: usize>(
    source: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    custody: &AccountInfo<'info>,
    plan: AdmissionPlan<N>,
) -> Result<(), ProgramError> {
    let initialize =
        token_initialize_instruction(plan.realm.release, *escrow.key, *mint.key, *custody.key)?;
    invoke(
        &initialize,
        &[escrow.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| AdapterError::VaultInitializeCpi)?;
    let transfer = token_transfer_instruction(
        plan.realm.release,
        *source.key,
        *mint.key,
        *escrow.key,
        *owner.key,
        plan.quote_atoms,
        plan.realm.mint.decimals,
    )?;
    invoke(
        &transfer,
        &[
            source.clone(),
            mint.clone(),
            escrow.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_admission<const N: usize>(
    program_id: &Pubkey,
    payer: &AccountInfo<'_>,
    state_account: &AccountInfo<'_>,
    custody_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    quote_source: &AccountInfo<'_>,
    quote_escrow: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    rent_credit_state: RentCreditV1,
    plan: AdmissionPlan<N>,
) -> Result<(), ProgramError> {
    write_order_state(state_account, plan.state)?;
    write_custody(custody_account, plan.custody)?;
    write_position(position_account, plan.position)?;
    let total_rent = plan
        .state_rent
        .checked_add(plan.custody_rent)
        .and_then(|value| value.checked_add(plan.escrow_rent))
        .ok_or(AdapterError::Arithmetic)?;
    if payer.lamports()
        != plan
            .payer_before
            .checked_sub(total_rent)
            .ok_or(AdapterError::Arithmetic)?
        || state_account.owner != program_id
        || state_account.lamports() != plan.state_rent
        || custody_account.owner != program_id
        || custody_account.lamports() != plan.custody_rent
        || quote_escrow.owner != token_program.key
        || quote_escrow.lamports() != plan.escrow_rent
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    authenticate_quote_post(
        quote_source,
        quote_escrow,
        mint,
        token_program,
        plan.realm,
        plan.source_before,
        plan.quote_atoms,
        custody_account.key.to_bytes(),
        payer.key.to_bytes(),
    )?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

#[derive(Clone, Copy)]
struct ReleasePlan<const N: usize> {
    state: OrderStateV1,
    position: PositionV1<N>,
    release: dclutch_general_contract::GeneralCustodyReleaseV1<N>,
    custody_bump: u8,
    realm: RealmFacts,
    source_before: TokenAccount,
    destination_before: TokenAccount,
    escrow_close: SourceCloseCreditPlanV1,
    custody_close: SourceCloseCreditPlanV1,
    rent_credit_state: RentCreditV1,
}

fn process_release_order<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    order: PortfolioOrderV1<N>,
    cancellation: bool,
) -> Result<(), ProgramError> {
    let owner = if cancellation {
        Some(account(accounts, 0)?)
    } else {
        None
    };
    let market_account = account(accounts, usize::from(cancellation))?;
    let realm_account = account(accounts, usize::from(cancellation) + 1)?;
    let realm_cursor = account(accounts, usize::from(cancellation) + 2)?;
    let config_account = account(accounts, usize::from(cancellation) + 3)?;
    let batch_account = account(accounts, usize::from(cancellation) + 4)?;
    let state_account = account(accounts, usize::from(cancellation) + 5)?;
    let custody_account = account(accounts, usize::from(cancellation) + 6)?;
    let position_account = account(accounts, usize::from(cancellation) + 7)?;
    let quote_escrow = account(accounts, usize::from(cancellation) + 8)?;
    let quote_destination = account(accounts, usize::from(cancellation) + 9)?;
    let mint = account(accounts, usize::from(cancellation) + 10)?;
    let token_program = account(accounts, usize::from(cancellation) + 11)?;
    let rent_credit = account(accounts, usize::from(cancellation) + 12)?;
    let rent_sysvar = account(accounts, usize::from(cancellation) + 13)?;
    let clock_sysvar = if cancellation {
        Some(account(accounts, 15)?)
    } else {
        None
    };
    let (config, config_id) = authenticate_config(program_id, config_account)?;
    authenticate_order_id(order)?;
    let market =
        authenticate_market::<N>(program_id, market_account, market_account.key.to_bytes())?;
    if hash(&market.root().identity().to_bytes()).to_bytes()
        != config.market_identity_id().to_bytes()
        || market.root().identity().claim_basis_id().to_bytes()
            != config.claim_basis_id().to_bytes()
        || market.root().identity().generation() != config.generation()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let root_seeds = GeneralRootPdaSeedsV1::new(
        market_account.key.to_bytes(),
        config.generation(),
        config_id,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (root_key, _) = Pubkey::find_program_address(&root_seeds.seed_components(), program_id);
    let batch = authenticate_batch_by_root_key(
        program_id,
        batch_account,
        root_key,
        order.batch_sequence(),
        config_id,
    )?;
    let state_seeds = GeneralOrderStatePdaSeedsV1::new(market_account.key.to_bytes(), order)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_state, _) =
        Pubkey::find_program_address(&state_seeds.seed_components(), program_id);
    if state_account.key != &expected_state || state_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mut state = decode_order_state(state_account)?;
    let custody_seeds = GeneralOrderCustodyPdaSeedsV1::new(state_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_custody, custody_bump) =
        Pubkey::find_program_address(&custody_seeds.seed_components(), program_id);
    if custody_account.key != &expected_custody || custody_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let custody = decode_custody::<N>(custody_account)?;
    if custody.quote_escrow() != quote_escrow.key.to_bytes()
        || custody.rent_beneficiary() != rent_credit.key.to_bytes()
        || custody.owner() != order.owner()
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    if let Some(owner) = owner
        && owner.key.to_bytes() != order.owner().to_bytes()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(order.owner().to_bytes()),
    )?;
    let mut position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        &Pubkey::new_from_array(order.owner().to_bytes()),
        config.generation(),
    )?;
    let release = if cancellation {
        let slot = authenticate_clock(clock_sysvar.ok_or(AdapterError::AccountFrameLength)?)?.slot;
        custody
            .cancel_and_release(
                &mut state,
                order,
                order.owner(),
                slot,
                batch.collection_close(),
                config,
            )
            .map_err(|_| AdapterError::MarketTransition)?
    } else {
        custody
            .close_after_batch(&mut state, order, batch, config)
            .map_err(|_| AdapterError::MarketTransition)?
    };
    for (index, amount) in release.claim_atoms.iter().enumerate() {
        if *amount != 0 {
            position
                .credit_outcome(index, *amount)
                .map_err(|_| AdapterError::PositionAuthentication)?;
        }
    }
    let (source_before, destination_before) = authenticate_release_tokens(
        quote_escrow,
        quote_destination,
        mint,
        token_program,
        custody_account,
        order.owner().to_bytes(),
        release.quote_atoms,
        realm,
    )?;
    let escrow_close = SourceCloseCreditPlanV1::new(
        quote_escrow.lamports(),
        rent_credit.lamports(),
        quote_escrow.lamports(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let custody_close = SourceCloseCreditPlanV1::new(
        custody_account.lamports(),
        escrow_close.credit_after(),
        custody_account.lamports(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    preflight_mutable(&[
        state_account,
        custody_account,
        position_account,
        quote_escrow,
        quote_destination,
        rent_credit,
    ])?;
    let plan = ReleasePlan {
        state,
        position,
        release,
        custody_bump,
        realm,
        source_before,
        destination_before,
        escrow_close,
        custody_close,
        rent_credit_state,
    };
    execute_release(
        program_id,
        state_account,
        custody_account,
        position_account,
        quote_escrow,
        quote_destination,
        mint,
        token_program,
        rent_credit,
        plan,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_release<'info, const N: usize>(
    program_id: &Pubkey,
    state_account: &AccountInfo<'info>,
    custody_account: &AccountInfo<'info>,
    position_account: &AccountInfo<'info>,
    quote_escrow: &AccountInfo<'info>,
    quote_destination: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    plan: ReleasePlan<N>,
) -> Result<(), ProgramError> {
    let custody_seed = GeneralOrderCustodyPdaSeedsV1::new(state_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let components = custody_seed.seed_components();
    let bump = [plan.custody_bump];
    let signer = [components[0], components[1], bump.as_slice()];
    if plan.release.quote_atoms != 0 {
        let transfer = token_transfer_instruction(
            plan.realm.release,
            *quote_escrow.key,
            *mint.key,
            *quote_destination.key,
            *custody_account.key,
            plan.release.quote_atoms,
            plan.realm.mint.decimals,
        )?;
        invoke_signed(
            &transfer,
            &[
                quote_escrow.clone(),
                mint.clone(),
                quote_destination.clone(),
                custody_account.clone(),
                token_program.clone(),
            ],
            &[&signer],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
    }
    let close = token_close_instruction(
        plan.realm.release,
        *quote_escrow.key,
        *rent_credit.key,
        *custody_account.key,
    )?;
    invoke_signed(
        &close,
        &[
            quote_escrow.clone(),
            rent_credit.clone(),
            custody_account.clone(),
            token_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| AdapterError::PositionClose)?;
    plan.escrow_close
        .validate_post(quote_escrow.lamports(), rent_credit.lamports())
        .map_err(|_| AdapterError::PositionClose)?;
    write_order_state(state_account, plan.state)?;
    write_position(position_account, plan.position)?;
    close_program_account(custody_account, rent_credit)?;
    plan.custody_close
        .validate_post(custody_account.lamports(), rent_credit.lamports())
        .map_err(|_| AdapterError::PositionClose)?;
    let closed_escrow_data = quote_escrow
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if quote_escrow.lamports() != 0 || closed_escrow_data.iter().any(|byte| *byte != 0) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit, plan.rent_credit_state)?;
    authenticate_release_token_post(
        quote_destination,
        mint,
        token_program,
        plan.realm,
        plan.destination_before,
        plan.release.quote_atoms,
    )?;
    // The closed source was authenticated before CPI and is now absent. Its
    // exact prior token amount was the release quantity.
    if plan.source_before.amount != plan.release.quote_atoms {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn authenticate_config(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<(GeneralConfigV1, dclutch_general_contract::ContentId), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != GENERAL_CONFIG_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let config = GeneralConfigV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    if config.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let config_id = dclutch_general_contract::ContentId::new(hash(&data).to_bytes())
        .map_err(|_| AdapterError::ContentIdentity)?;
    let seeds = GeneralConfigPdaSeedsV1::new(config_id);
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    if account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok((config, config_id))
}

fn authenticate_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    config_id: dclutch_general_contract::ContentId,
) -> Result<GeneralRootV1, ProgramError> {
    if account.owner != program_id || account.executable || account.data_len() != GENERAL_ROOT_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let root = GeneralRootV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let seeds = GeneralRootPdaSeedsV1::new(root.market(), root.generation(), config_id)
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    let mut canonical = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if account.key != &expected
        || root.config_id() != config_id
        || canonical.as_slice() != &data[..]
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(root)
}

fn authenticate_batch(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AccountInfo<'_>,
    sequence: u64,
    config_id: dclutch_general_contract::ContentId,
) -> Result<BatchRootV1, ProgramError> {
    authenticate_batch_by_root_key(program_id, account, *root.key, sequence, config_id)
}

fn authenticate_batch_by_root_key(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: Pubkey,
    sequence: u64,
    config_id: dclutch_general_contract::ContentId,
) -> Result<BatchRootV1, ProgramError> {
    if account.owner != program_id || account.executable || account.data_len() != BATCH_ROOT_BYTES {
        return Err(AdapterError::AccountIdentity.into());
    }
    let seeds = GeneralBatchPdaSeedsV1::new(root.to_bytes(), sequence)
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let batch = BatchRootV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let mut canonical = [0; BATCH_ROOT_BYTES];
    batch
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if account.key != &expected
        || batch.sequence() != sequence
        || batch.config_id() != config_id
        || canonical.as_slice() != &data[..]
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(batch)
}

fn authenticate_market<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_key: [u8; 32],
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    if account.owner != program_id || account.executable || account.key.to_bytes() != expected_key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if decode_market_outcome_count(&data).map_err(|_| AdapterError::AccountData)?
        != u8::try_from(N).map_err(|_| AdapterError::Arithmetic)?
    {
        return Err(AdapterError::AccountData.into());
    }
    let market = CategoricalMarketV1::<N>::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &digest], program_id);
    if account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(market)
}

fn authenticate_market_config<const N: usize>(
    market: CategoricalMarketV1<N>,
    config: GeneralConfigV1,
    root: GeneralRootV1,
    config_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let identity = market.root().identity();
    if hash(&identity.to_bytes()).to_bytes() != config.market_identity_id().to_bytes()
        || identity.claim_basis_id().to_bytes() != config.claim_basis_id().to_bytes()
        || identity.generation() != config.generation()
        || root.config_id() != config_id
        || root.generation() != config.generation()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_realm<'info>(
    program_id: &Pubkey,
    raw: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    expected_digest: [u8; 32],
) -> Result<RealmFacts, ProgramError> {
    let realm = with_authenticated_finalized_record_v1(
        program_id,
        raw,
        cursor,
        rent,
        REALM_SCHEMA_RELEASE_ID_V1,
        expected_digest,
        |record| {
            RealmV1::decode(record.exact_content()).map_err(|_| AdapterError::AccountData.into())
        },
    )?;
    authenticate_live_realm(realm, mint, token_program)
}

fn authenticate_live_realm(
    realm: RealmV1,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<RealmFacts, ProgramError> {
    if realm.collateral_mint() != &mint.key.to_bytes()
        || realm.token_program() != &token_program.key.to_bytes()
        || mint.owner != token_program.key
        || !token_program.executable
        || !recognized_program_loader(token_program.owner)
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mint_state = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint_state.mint_authority)?;
    require_freeze_policy(
        realm.freeze_authority_policy(),
        &mint_state.freeze_authority,
    )?;
    Ok(RealmFacts {
        release,
        mint: mint_state,
    })
}

fn authenticate_order_id<const N: usize>(order: PortfolioOrderV1<N>) -> Result<(), ProgramError> {
    let mut preimage = Vec::new();
    preimage
        .try_reserve_exact(
            PortfolioOrderV1::<N>::signing_preimage_len().map_err(|_| AdapterError::Arithmetic)?,
        )
        .map_err(|_| AdapterError::Arithmetic)?;
    preimage.resize(
        PortfolioOrderV1::<N>::signing_preimage_len().map_err(|_| AdapterError::Arithmetic)?,
        0,
    );
    order
        .encode_signing_preimage(&mut preimage)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    if hash(&preimage).to_bytes() != order.order_id().to_bytes() {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(())
}

fn authenticate_position<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    owner: &Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (expected, _) = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market.key.as_ref(), owner.as_ref()],
        program_id,
    );
    if account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let position = PositionV1::<N>::decode(&data).map_err(|_| AdapterError::AccountData)?;
    if position.market() != &market.key.to_bytes()
        || position.owner() != &owner.to_bytes()
        || position.generation() != generation
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?)
        .map_err(|_| AdapterError::Arithmetic)?;
    canonical.resize(
        PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
        0,
    );
    position
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(position)
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    owner: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority = RefundAuthority::new(owner.to_bytes())
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id || account.data_len() != RENT_CREDIT_BYTES_V1 {
        return Err(AdapterError::PositionPostcondition.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(expected) || expected.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn authenticate_quote_source(
    source: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    owner: &AccountInfo<'_>,
    realm: RealmFacts,
    amount: u64,
) -> Result<TokenAccount, ProgramError> {
    if source.owner != token_program.key || source.executable {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let source_state = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if source_state.mint != mint.key.to_bytes() || source_state.amount < amount {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let authorized = if source_state.owner == owner.key.to_bytes() {
        true
    } else {
        matches!(source_state.delegate, COption::Some(delegate) if delegate == owner.key.to_bytes())
            && source_state.delegated_amount >= amount
    };
    if !authorized {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(source_state)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_quote_post(
    source: &AccountInfo<'_>,
    escrow: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    source_before: TokenAccount,
    amount: u64,
    escrow_owner: [u8; 32],
    transfer_authority: [u8; 32],
) -> Result<(), ProgramError> {
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let source_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &source_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let escrow_data = escrow
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let escrow_after = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &escrow_data,
            mint.key.to_bytes(),
            escrow_owner,
        )
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mut expected_source = source_before;
    expected_source.amount = expected_source
        .amount
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    if source_before.owner != transfer_authority {
        expected_source.delegated_amount = expected_source
            .delegated_amount
            .checked_sub(amount)
            .ok_or(AdapterError::Arithmetic)?;
    }
    if source_after != expected_source || escrow_after.amount != amount {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_release_tokens(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    owner: [u8; 32],
    amount: u64,
    realm: RealmFacts,
) -> Result<(TokenAccount, TokenAccount), ProgramError> {
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let facts = realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: token_program.key.to_bytes(),
            mint_address: mint.key.to_bytes(),
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &destination_data,
            authority: authority.key.to_bytes(),
            amount,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if facts.authority_role() != AuthorityRole::Owner
        || facts.source().owner != authority.key.to_bytes()
        || facts.source().amount != amount
        || facts.destination().owner != owner
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok((facts.source(), facts.destination()))
}

fn authenticate_release_token_post(
    destination: &AccountInfo<'_>,
    _mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: TokenAccount,
    amount: u64,
) -> Result<(), ProgramError> {
    let data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mut expected = before;
    expected.amount = expected
        .amount
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    if after != expected {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn decode_order_state(account: &AccountInfo<'_>) -> Result<OrderStateV1, ProgramError> {
    if account.data_len() != ORDER_STATE_BYTES {
        return Err(AdapterError::AccountData.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    OrderStateV1::decode(&data).map_err(|_| AdapterError::AccountData.into())
}

fn decode_custody<const N: usize>(
    account: &AccountInfo<'_>,
) -> Result<GeneralOrderCustodyV1<N>, ProgramError> {
    let expected =
        GeneralOrderCustodyV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    if account.data_len() != expected {
        return Err(AdapterError::AccountData.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    GeneralOrderCustodyV1::decode(&data).map_err(|_| AdapterError::AccountData.into())
}

fn write_batch(account: &AccountInfo<'_>, batch: BatchRootV1) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    batch
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if BatchRootV1::decode(&data) != Ok(batch) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_order_state(account: &AccountInfo<'_>, state: OrderStateV1) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    state
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if OrderStateV1::decode(&data) != Ok(state) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_custody<const N: usize>(
    account: &AccountInfo<'_>,
    custody: GeneralOrderCustodyV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    custody
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if GeneralOrderCustodyV1::<N>::decode(&data) != Ok(custody) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_position<const N: usize>(
    account: &AccountInfo<'_>,
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    position
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if PositionV1::<N>::decode(&data) != Ok(position) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn close_program_account(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let amount = source.lamports();
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    **source
        .try_borrow_mut_lamports()
        .map_err(|_| AdapterError::PositionClose)? = 0;
    **destination
        .try_borrow_mut_lamports()
        .map_err(|_| AdapterError::PositionClose)? = destination_after;
    source.resize(0).map_err(|_| AdapterError::PositionClose)?;
    source.assign(&system_program::ID);
    Ok(())
}

fn token_initialize_instruction(
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
    account: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
) -> Result<Instruction, ProgramError> {
    let spec = initialize_account3(
        release.token_program(),
        account.to_bytes(),
        mint.to_bytes(),
        owner.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(account, false),
            AccountMeta::new_readonly(mint, false),
        ]),
        data: Vec::from(*spec.data()),
    })
}

#[allow(clippy::too_many_arguments)]
fn token_transfer_instruction(
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
    source: Pubkey,
    mint: Pubkey,
    destination: Pubkey,
    authority: Pubkey,
    amount: u64,
    decimals: u8,
) -> Result<Instruction, ProgramError> {
    let spec = transfer_checked(
        release.token_program(),
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
        amount,
        decimals,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(source, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(authority, true),
        ]),
        data: Vec::from(*spec.data()),
    })
}

fn token_close_instruction(
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
    source: Pubkey,
    destination: Pubkey,
    authority: Pubkey,
) -> Result<Instruction, ProgramError> {
    let spec = close_account(
        release.token_program(),
        source.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(source, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(authority, true),
        ]),
        data: Vec::from(*spec.data()),
    })
}

fn authenticate_system_rent_clock(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    clock: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if system.key != &system_program::ID
        || system.owner != &native_loader::ID
        || !system.executable
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
        || rent.executable
        || clock.key != &sysvar::clock::ID
        || clock.owner != &sysvar::ID
        || clock.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Rent::from_account_info(rent).map_err(|_| AdapterError::AccountData)?;
    Clock::from_account_info(clock).map_err(|_| AdapterError::AccountData)?;
    Ok(())
}

fn authenticate_clock(account: &AccountInfo<'_>) -> Result<Clock, ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    Clock::from_account_info(account).map_err(|_| AdapterError::AccountData.into())
}

fn require_system_wallet(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID
        || account.executable
        || account.lamports() != 0
        || !account.data_is_empty()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn preflight_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for account in accounts {
        drop(
            account
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::PositionAuthentication)?,
        );
        drop(
            account
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::PositionAuthentication)?,
        );
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

#[cfg(test)]
mod tests {
    use dclutch_general_contract::{
        GENERAL_INSTRUCTION_HEADER_BYTES, GeneralConfigV1Input, GeneralInstructionV1,
        PortfolioOrderV1Input,
    };

    use super::*;

    fn id(value: u8) -> dclutch_general_contract::ContentId {
        dclutch_general_contract::ContentId::new([value; 32]).expect("ID")
    }

    fn exact_order() -> PortfolioOrderV1<2> {
        let provisional = PortfolioOrderV1::new(PortfolioOrderV1Input {
            market_identity_id: id(2),
            claim_basis_id: id(3),
            owner: dclutch_general_contract::OwnerKeyV1::new([4; 32]).expect("owner"),
            order_id: id(5),
            generation: 7,
            batch_sequence: 0,
            nonce: 9,
            valid_until_slot: 30,
            max_lots: 1,
            max_quote_debit_per_lot_numerator: 100,
            coefficients: [-1, 1],
            outcome_count: 2,
        })
        .expect("order");
        let mut bytes = [0; 184];
        provisional
            .encode_signing_preimage(&mut bytes)
            .expect("preimage");
        PortfolioOrderV1::new(PortfolioOrderV1Input {
            order_id: dclutch_general_contract::ContentId::new(hash(&bytes).to_bytes())
                .expect("digest"),
            ..PortfolioOrderV1Input {
                market_identity_id: id(2),
                claim_basis_id: id(3),
                owner: dclutch_general_contract::OwnerKeyV1::new([4; 32]).expect("owner"),
                order_id: id(5),
                generation: 7,
                batch_sequence: 0,
                nonce: 9,
                valid_until_slot: 30,
                max_lots: 1,
                max_quote_debit_per_lot_numerator: 100,
                coefficients: [-1, 1],
                outcome_count: 2,
            }
        })
        .expect("committed order")
    }

    #[test]
    fn exact_n_dispatch_refuses_width_and_order_id_substitution_before_accounts() {
        let order = exact_order();
        assert_eq!(authenticate_order_id(order), Ok(()));
        let substituted = PortfolioOrderV1::new(PortfolioOrderV1Input {
            order_id: id(99),
            ..PortfolioOrderV1Input {
                market_identity_id: id(2),
                claim_basis_id: id(3),
                owner: dclutch_general_contract::OwnerKeyV1::new([4; 32]).expect("owner"),
                order_id: id(5),
                generation: 7,
                batch_sequence: 0,
                nonce: 9,
                valid_until_slot: 30,
                max_lots: 1,
                max_quote_debit_per_lot_numerator: 100,
                coefficients: [-1, 1],
                outcome_count: 2,
            }
        })
        .expect("substitution");
        assert_eq!(
            authenticate_order_id(substituted),
            Err(AdapterError::ContentIdentity.into())
        );
        let instruction = GeneralInstructionV1::AdmitOrder(order);
        let mut bytes = std::vec![0; instruction.encoded_len().expect("length")];
        instruction.encode(&mut bytes).expect("instruction");
        assert_eq!(bytes.get(INSTRUCTION_WIDTH_OFFSET), Some(&2));
        *bytes.get_mut(INSTRUCTION_WIDTH_OFFSET).expect("width") = 16;
        assert!(GeneralInstructionV1::<16>::decode(&bytes).is_err());
        assert!(bytes.len() > GENERAL_INSTRUCTION_HEADER_BYTES);
    }

    #[test]
    fn only_complete_general_routes_are_enabled() {
        let config = GeneralConfigV1::new(GeneralConfigV1Input {
            capacity_profile_id: id(1),
            market_identity_id: id(2),
            claim_basis_id: id(3),
            capability_release_id: dclutch_general_contract::GENERAL_CAPABILITY_RELEASE_ID_V1,
            generation: 7,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 10,
            settlement_slots: 10,
            max_orders_per_candidate: 4,
            max_pages_per_candidate: 1,
            outcome_count: 2,
        })
        .expect("config");
        let instruction =
            GeneralInstructionV1::<2>::Activate(dclutch_general_contract::ActivateGeneralV1 {
                expected_market_child_count: 0,
                config,
            });
        let mut bytes = std::vec![0; instruction.encoded_len().expect("length")];
        instruction.encode(&mut bytes).expect("instruction");
        assert_eq!(
            dispatch_width::<2>(&Pubkey::new_unique(), &[], &bytes),
            Err(AdapterError::InvalidInstruction.into())
        );
    }
}
