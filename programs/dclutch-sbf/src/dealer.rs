//! Hostile SVM adapter for the executable Dealer lifecycle.
//!
//! Every route authenticates the immutable config as a finalized generic raw
//! record, the Pool occurrence that selected it, and all physical claim,
//! collateral, rent, and typed capability-funding custody it changes.

use alloc::vec::Vec;

use dclutch_capability_contract::{
    CapabilityFundingAuthorityDerivationV1, CapabilityFundingDerivationV1,
    CapabilityFundingVaultDerivationV1, CapabilityManifestV1, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1, RealmCollateralCustodyV1,
    RealmCollateralVaultObservationV1,
};
use dclutch_core_contract::ContentId;
use dclutch_dealer_contract::{
    LiquidityAmounts, LiquidityConfigV1, LpPosition, PoolState, RentCreditTerms, TradeSide,
    activation::{activate_pool, retire_pool},
    frame::{
        ConfigPdaSeedsV1, DealerAccountMetaV1, DealerCollateralCompartmentV1,
        DealerCollateralVaultPdaSeedsV1, DealerFrameV1, LpPositionPdaSeedsV1, PoolPdaSeedsV1,
        PoolPositionPdaSeedsV1, validate_market_phase,
    },
    instruction::{DEALER_INSTRUCTION_MAGIC, DealerActionV1, DealerInstructionV1},
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AuthorityRole, CollateralAdapterReleaseV1, ExactTransferInput, Mint,
    TokenAccount, close_account, initialize_account3, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
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
    records::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
};

const ACTION_OFFSET: usize = 10;
const MIN_ACTION: u8 = DealerActionV1::ActivatePool as u8;
const MAX_ACTION: u8 = DealerActionV1::RetirePool as u8;

#[derive(Clone, Copy, Debug)]
struct Route {
    action: DealerActionV1,
    market_index: usize,
    config_index: usize,
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    release: CollateralAdapterReleaseV1,
    mint: Mint,
}

#[derive(Clone, Copy)]
struct TransferFacts {
    source: TokenAccount,
    destination: TokenAccount,
    authority_role: AuthorityRole,
    source_lamports: u64,
    destination_lamports: u64,
    mint_lamports: u64,
}

#[derive(Clone, Copy)]
struct PoolFacts<const N: usize, const B: usize> {
    market: CategoricalMarketV1<N>,
    pool: PoolState<N, B>,
    config: LiquidityConfigV1<N, B>,
    pool_bump: u8,
}

/// Route one Dealer instruction after matching [`DEALER_INSTRUCTION_MAGIC`].
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if instruction_data.get(..DEALER_INSTRUCTION_MAGIC.len()) != Some(&DEALER_INSTRUCTION_MAGIC) {
        return Err(AdapterError::InvalidInstruction.into());
    }
    let route = select_route(instruction_data)?;
    let market = account(accounts, route.market_index)?;
    let market_data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let outcomes = decode_market_outcome_count(&market_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    drop(market_data);
    match outcomes {
        2 => dispatch_bins::<2>(program_id, accounts, instruction_data, route),
        3 => dispatch_bins::<3>(program_id, accounts, instruction_data, route),
        4 => dispatch_bins::<4>(program_id, accounts, instruction_data, route),
        5 => dispatch_bins::<5>(program_id, accounts, instruction_data, route),
        6 => dispatch_bins::<6>(program_id, accounts, instruction_data, route),
        7 => dispatch_bins::<7>(program_id, accounts, instruction_data, route),
        8 => dispatch_bins::<8>(program_id, accounts, instruction_data, route),
        9 => dispatch_bins::<9>(program_id, accounts, instruction_data, route),
        10 => dispatch_bins::<10>(program_id, accounts, instruction_data, route),
        11 => dispatch_bins::<11>(program_id, accounts, instruction_data, route),
        12 => dispatch_bins::<12>(program_id, accounts, instruction_data, route),
        13 => dispatch_bins::<13>(program_id, accounts, instruction_data, route),
        14 => dispatch_bins::<14>(program_id, accounts, instruction_data, route),
        15 => dispatch_bins::<15>(program_id, accounts, instruction_data, route),
        16 => dispatch_bins::<16>(program_id, accounts, instruction_data, route),
        _ => Err(AdapterError::PositionAuthentication.into()),
    }
}

fn select_route(data: &[u8]) -> Result<Route, ProgramError> {
    let action = *data
        .get(ACTION_OFFSET)
        .ok_or(AdapterError::InvalidInstruction)?;
    if !(MIN_ACTION..=MAX_ACTION).contains(&action) {
        return Err(AdapterError::InvalidInstruction.into());
    }
    let (action, market_index, config_index) = match action {
        1 => (DealerActionV1::ActivatePool, 3, 8),
        2 => (DealerActionV1::CreateLpPosition, 2, 4),
        3 => (DealerActionV1::AddLiquidity, 2, 4),
        4 => (DealerActionV1::RemoveLiquidity, 2, 4),
        5 => (DealerActionV1::Trade, 2, 4),
        6 => (DealerActionV1::ResetLadder, 0, 2),
        7 => (DealerActionV1::CloseLpPosition, 1, 3),
        8 => (DealerActionV1::RetirePool, 0, 3),
        _ => return Err(AdapterError::InvalidInstruction.into()),
    };
    Ok(Route {
        action,
        market_index,
        config_index,
    })
}

fn dispatch_bins<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    route: Route,
) -> Result<(), ProgramError> {
    let config = account(accounts, route.config_index)?;
    let width = config.data_len();
    macro_rules! choose {
        ($($b:literal),+ $(,)?) => {$({
            if width == LiquidityConfigV1::<N, $b>::encoded_len()
                .map_err(|_| AdapterError::Arithmetic)?
            {
                return process::<N, $b>(program_id, accounts, data, route);
            }
        })+};
    }
    choose!(1, 2, 3, 4, 5, 6, 7, 8);
    Err(AdapterError::AccountData.into())
}

fn process<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    route: Route,
) -> Result<(), ProgramError> {
    let instruction =
        DealerInstructionV1::<N>::decode(data).map_err(|_| AdapterError::InvalidInstruction)?;
    if instruction.action() != route.action {
        return Err(AdapterError::InvalidInstruction.into());
    }
    validate_frame::<N>(route.action, accounts)?;
    match instruction {
        DealerInstructionV1::ActivatePool(request) => {
            activate::<N, B>(program_id, accounts, request)
        }
        DealerInstructionV1::CreateLpPosition(request) => {
            create_lp_position::<N, B>(program_id, accounts, request)
        }
        DealerInstructionV1::AddLiquidity(request) => change_liquidity::<N, B, _>(
            program_id,
            accounts,
            request.lp_id(),
            request.request(),
            true,
        ),
        DealerInstructionV1::RemoveLiquidity(request) => change_liquidity::<N, B, _>(
            program_id,
            accounts,
            request.lp_id(),
            request.request(),
            false,
        ),
        DealerInstructionV1::Trade(request) => trade::<N, B>(program_id, accounts, request),
        DealerInstructionV1::ResetLadder {
            expected_pool_sequence,
        } => reset_ladder::<N, B>(program_id, accounts, expected_pool_sequence),
        DealerInstructionV1::CloseLpPosition(request) => {
            close_lp_position::<N, B>(program_id, accounts, request)
        }
        DealerInstructionV1::RetirePool {
            expected_pool_sequence,
            expected_market_child_count,
        } => retire::<N, B>(
            program_id,
            accounts,
            expected_pool_sequence,
            expected_market_child_count,
        ),
    }
}

fn validate_frame<const N: usize>(
    action: DealerActionV1,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut metas = Vec::new();
    metas
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for value in accounts {
        metas.push(DealerAccountMetaV1 {
            key: value.key.to_bytes(),
            is_signer: value.is_signer,
            is_writable: value.is_writable,
            is_executable: value.executable,
        });
    }
    DealerFrameV1::<N>::new(action, &metas).map_err(|_| AdapterError::AccountPrivilege)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn activate<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: dclutch_dealer_contract::instruction::ActivatePoolV1,
) -> Result<(), ProgramError> {
    let activator = account(accounts, 0)?;
    let owner = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let market_account = account(accounts, 3)?;
    let manifest_account = account(accounts, 4)?;
    let funding_account = account(accounts, 5)?;
    let funding_authority = account(accounts, 6)?;
    let funding_vault = account(accounts, 7)?;
    let config_account = account(accounts, 8)?;
    let config_staging = account(accounts, 9)?;
    let pool_account = account(accounts, 10)?;
    let lp_account = account(accounts, 11)?;
    let participant_account = account(accounts, 12)?;
    let pool_position_account = account(accounts, 13)?;
    let principal_vault = account(accounts, 14)?;
    let fee_vault = account(accounts, 15)?;
    let service_vault = account(accounts, 16)?;
    let pool_position_credit = account(accounts, 17)?;
    let pool_credit = account(accounts, 18)?;
    let lp_credit = account(accounts, 19)?;
    let mint = account(accounts, 20)?;
    let token_program = account(accounts, 21)?;
    let system = account(accounts, 22)?;
    let rent_sysvar = account(accounts, 23)?;
    authenticate_system_and_rent(system, rent_sysvar)?;
    for vacant in [
        pool_account,
        lp_account,
        pool_position_account,
        principal_vault,
        fee_vault,
        service_vault,
    ] {
        require_vacant(vacant)?;
    }
    if funding_authority.owner != &system_program::ID
        || !funding_authority.data_is_empty()
        || funding_authority.lamports() != 0
    {
        return Err(AdapterError::AccountIdentity.into());
    }

    let market = authenticate_market::<N>(program_id, market_account, request.generation())?;
    let realm = authenticate_realm(program_id, realm_account, mint, token_program, market)?;
    let (config, config_id) =
        authenticate_config::<N, B>(program_id, config_account, config_staging)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let manifest_id = ContentId::new(hash(&manifest_data).to_bytes())
        .map_err(|_| AdapterError::ContentIdentity)?;
    let (expected_manifest, _) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_id.to_bytes().as_slice(),
        ],
        program_id,
    );
    if manifest_account.owner != program_id
        || manifest_account.executable
        || manifest_account.key != &expected_manifest
        || manifest_id != market.root().identity().capability_manifest_id()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if funding_account.owner != program_id || funding_data.len() != FUNDING_STATE_BYTES {
        return Err(AdapterError::AccountIdentity.into());
    }
    let funding =
        FundingStateV1::decode(&funding_data).map_err(|_| AdapterError::PositionAuthentication)?;
    if funding.to_bytes().as_slice() != &funding_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let funding_seeds = CapabilityFundingDerivationV1::new(
        market_account.key.to_bytes(),
        request.generation(),
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_funding, _) = find_pda(program_id, &funding_seeds.seed_components());
    if funding_account.key != &expected_funding {
        return Err(AdapterError::AccountIdentity.into());
    }
    let authority_seeds =
        CapabilityFundingAuthorityDerivationV1::new(funding_account.key.to_bytes())
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_authority, funding_authority_bump) =
        find_pda(program_id, &authority_seeds.seed_components());
    if funding_authority.key != &expected_authority {
        return Err(AdapterError::AccountIdentity.into());
    }
    let selected = manifest
        .entry(funding.entry_index())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let binding = selected
        .funding_quote()
        .realm_collateral()
        .ok_or(AdapterError::PositionAuthentication)?;
    if binding.realm_id() != market.root().identity().realm_id()
        || binding.collateral_release_id().to_bytes()
            != *realm.realm.collateral_adapter_release_id()
        || binding.token_program() != token_program.key.to_bytes()
        || binding.mint() != mint.key.to_bytes()
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let funding_vault_seeds =
        CapabilityFundingVaultDerivationV1::new(funding_authority.key.to_bytes(), binding)
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_funding_vault, _) = find_pda(program_id, &funding_vault_seeds.seed_components());
    if funding_vault.key != &expected_funding_vault || funding_vault.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let funding_vault_data = funding_vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let funding_token = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &funding_vault_data,
            mint.key.to_bytes(),
            funding_authority.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication)?;

    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::PositionAuthentication)?;
    let state_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let funding_vault_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let collateral_observation = RealmCollateralVaultObservationV1::new(
        funding_vault.key.to_bytes(),
        funding_authority.key.to_bytes(),
        token_program.key.to_bytes(),
        mint.key.to_bytes(),
        funding_token.amount,
        funding_vault.lamports(),
        funding_vault_rent,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let collateral_custody = RealmCollateralCustodyV1::new(
        binding.realm_id(),
        binding.collateral_release_id(),
        funding_authority.key.to_bytes(),
        funding_vault.key.to_bytes(),
        collateral_observation,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let funding_custody = FundingCustodyObservationV1::with_realm_collateral(
        funding_account.lamports(),
        state_rent,
        collateral_custody,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;

    let pool_seeds = PoolPdaSeedsV1::new(
        market_account.key.to_bytes(),
        request.generation(),
        config_id,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_pool, pool_bump) = find_pda(program_id, &pool_seeds.seed_components());
    if pool_account.key != &expected_pool {
        return Err(AdapterError::AccountIdentity.into());
    }
    let lp_seeds = LpPositionPdaSeedsV1::new(
        market_account.key.to_bytes(),
        request.generation(),
        config_id,
        request.initial_lp_id(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_lp, lp_bump) = find_pda(program_id, &lp_seeds.seed_components());
    if lp_account.key != &expected_lp {
        return Err(AdapterError::AccountIdentity.into());
    }
    let position_seeds =
        PoolPositionPdaSeedsV1::new(market_account.key.to_bytes(), pool_account.key.to_bytes())
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_position, position_bump) =
        find_pda(program_id, &position_seeds.seed_components());
    if pool_position_account.key != &expected_position {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mut participant_position = authenticate_position::<N>(
        program_id,
        participant_account,
        market_account,
        owner.key,
        request.generation(),
    )?;
    participant_position
        .debit_complete_set(request.initial_claim_quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut pool_position = PositionV1::<N>::empty(
        market_account.key.to_bytes(),
        pool_account.key.to_bytes(),
        request.generation(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    pool_position
        .credit_complete_set(request.initial_claim_quantity())
        .map_err(|_| AdapterError::MarketTransition)?;

    let pool_account_rent = rent
        .minimum_balance(PoolState::<N, B>::encoded_len().map_err(|_| AdapterError::Arithmetic)?);
    let lp_rent = rent.minimum_balance(dclutch_dealer_contract::LP_POSITION_BYTES);
    let position_rent =
        rent.minimum_balance(PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?);
    let token_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let pool_bundle_rent = pool_account_rent
        .checked_add(token_rent.checked_mul(3).ok_or(AdapterError::Arithmetic)?)
        .ok_or(AdapterError::Arithmetic)?;
    let pool_credit_state = authenticate_rent_credit(program_id, pool_credit, owner.key)?;
    let lp_credit_state = authenticate_rent_credit(program_id, lp_credit, owner.key)?;
    let pool_position_credit_state =
        authenticate_rent_credit(program_id, pool_position_credit, pool_account.key)?;
    if config.liquidity_owner() != owner.key.to_bytes() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let now_slot = Clock::get()
        .map_err(|_| AdapterError::PositionAuthentication)?
        .slot;
    let attachment = dclutch_dealer_contract::LiquidityAttachment::new(
        market.root().identity(),
        selected.release_id(),
        config_id,
        owner.key.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let plan = activate_pool(
        market.root(),
        market_account.key.to_bytes(),
        manifest,
        funding,
        funding_account.key.to_bytes(),
        funding_authority.key.to_bytes(),
        funding_custody,
        attachment,
        &config,
        pool_account.key.to_bytes(),
        lp_account.key.to_bytes(),
        owner.key.to_bytes(),
        RentCreditTerms::new(owner.key.to_bytes(), pool_bundle_rent)
            .map_err(|_| AdapterError::PositionAuthentication)?,
        RentCreditTerms::new(owner.key.to_bytes(), lp_rent)
            .map_err(|_| AdapterError::PositionAuthentication)?,
        RentCreditTerms::new(pool_account.key.to_bytes(), position_rent)
            .map_err(|_| AdapterError::PositionAuthentication)?,
        request,
        now_slot,
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    let total_creation_rent = pool_bundle_rent
        .checked_add(lp_rent)
        .and_then(|value| value.checked_add(position_rent))
        .ok_or(AdapterError::Arithmetic)?;
    if plan.funding_debit().activation().rent_lamports() != total_creation_rent
        || plan.funding_debit().activation().creation_lamports() != 0
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let mut market_after = market;
    market_after
        .register_child(request.generation(), request.expected_market_child_count())
        .map_err(|_| AdapterError::MarketTransition)?;
    if market_after.root() != plan.market() {
        return Err(AdapterError::MarketTransition.into());
    }
    let market_bytes = encode_market(market_after)?;
    let funding_bytes = plan.funding().to_bytes();
    let pool_bytes = encode_pool(plan.pool())?;
    let lp_bytes = plan
        .initial_position()
        .to_bytes()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let participant_bytes = encode_position(participant_position)?;
    let pool_position_bytes = encode_position(pool_position)?;
    let activator_before = activator.lamports();
    if activator_before < total_creation_rent {
        return Err(AdapterError::PositionRentUnderfunded.into());
    }
    preflight_mutable(&[
        activator,
        market_account,
        funding_account,
        funding_vault,
        pool_account,
        lp_account,
        participant_account,
        pool_position_account,
        principal_vault,
        fee_vault,
        service_vault,
    ])?;

    create_pda_account(
        program_id,
        activator,
        pool_account,
        system,
        pool_account_rent,
        pool_bytes.len(),
        program_id,
        &pool_seeds.seed_components(),
        pool_bump,
    )?;
    create_pda_account(
        program_id,
        activator,
        lp_account,
        system,
        lp_rent,
        lp_bytes.len(),
        program_id,
        &lp_seeds.seed_components(),
        lp_bump,
    )?;
    create_pda_account(
        program_id,
        activator,
        pool_position_account,
        system,
        position_rent,
        pool_position_bytes.len(),
        program_id,
        &position_seeds.seed_components(),
        position_bump,
    )?;
    for (vault, compartment) in [
        (principal_vault, DealerCollateralCompartmentV1::Principal),
        (fee_vault, DealerCollateralCompartmentV1::RealizedFees),
        (service_vault, DealerCollateralCompartmentV1::Service),
    ] {
        let seeds = DealerCollateralVaultPdaSeedsV1::new(pool_account.key.to_bytes(), compartment)
            .map_err(|_| AdapterError::PositionAuthentication)?;
        let (expected, bump) = find_pda(program_id, &seeds.seed_components());
        if vault.key != &expected {
            return Err(AdapterError::AccountIdentity.into());
        }
        create_pda_account(
            program_id,
            activator,
            vault,
            system,
            token_rent,
            ACCOUNT_BYTES,
            token_program.key,
            &seeds.seed_components(),
            bump,
        )?;
        initialize_token_vault(vault, mint, token_program, realm.release, pool_account.key)?;
    }
    let funding_authority_bump_seed = [funding_authority_bump];
    let funding_signer = append_bump(
        &authority_seeds.seed_components(),
        &funding_authority_bump_seed,
    )?;
    transfer_if_nonzero(
        funding_vault,
        principal_vault,
        mint,
        token_program,
        realm,
        funding_authority,
        plan.funding_debit().liquidity().amount(),
        Some(funding_signer.as_slice()),
    )?;
    if let Some(service) = plan.funding_debit().service() {
        transfer_if_nonzero(
            funding_vault,
            service_vault,
            mint,
            token_program,
            realm,
            funding_authority,
            service.amount(),
            Some(funding_signer.as_slice()),
        )?;
    }
    {
        let mut payer_lamports = activator
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionPostcondition)?;
        let mut funding_lamports = funding_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionPostcondition)?;
        **payer_lamports = (**payer_lamports)
            .checked_add(total_creation_rent)
            .ok_or(AdapterError::Arithmetic)?;
        **funding_lamports = (**funding_lamports)
            .checked_sub(total_creation_rent)
            .ok_or(AdapterError::PositionRentUnderfunded)?;
    }
    persist_market(market_account, &market_bytes, market_after)?;
    persist_bytes(funding_account, &funding_bytes)?;
    persist_pool(pool_account, &pool_bytes, plan.pool())?;
    persist_lp(lp_account, &lp_bytes, plan.initial_position())?;
    persist_position(
        participant_account,
        &participant_bytes,
        participant_position,
    )?;
    persist_position(pool_position_account, &pool_position_bytes, pool_position)?;
    if activator.lamports() != activator_before || funding_account.lamports() != state_rent {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, pool_credit, pool_credit_state)?;
    require_unchanged_rent_credit(program_id, lp_credit, lp_credit_state)?;
    require_unchanged_rent_credit(program_id, pool_position_credit, pool_position_credit_state)
}

fn create_lp_position<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: dclutch_dealer_contract::instruction::CreateLpPositionV1,
) -> Result<(), ProgramError> {
    let payer = account(accounts, 0)?;
    let owner = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let pool_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let config_staging = account(accounts, 5)?;
    let lp_account = account(accounts, 6)?;
    let rent_credit_account = account(accounts, 7)?;
    let system = account(accounts, 8)?;
    let rent_sysvar = account(accounts, 9)?;
    authenticate_system_and_rent(system, rent_sysvar)?;
    require_vacant(lp_account)?;
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market_account,
        pool_account,
        config_account,
        config_staging,
        DealerActionV1::CreateLpPosition,
    )?;
    let generation = facts.market.root().identity().generation();
    let lp_seeds = LpPositionPdaSeedsV1::new(
        market_account.key.to_bytes(),
        generation,
        facts.config.content_id(),
        request.lp_id(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected, bump) = find_pda(program_id, &lp_seeds.seed_components());
    if lp_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent_credit = authenticate_rent_credit(program_id, rent_credit_account, owner.key)?;
    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::PositionAuthentication)?;
    let funded_rent = rent.minimum_balance(dclutch_dealer_contract::LP_POSITION_BYTES);
    let rent_terms = RentCreditTerms::new(owner.key.to_bytes(), funded_rent)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (lp, _) = facts
        .pool
        .create_position(
            pool_account.key.to_bytes(),
            &facts.config,
            request.expected_pool_sequence(),
            lp_account.key.to_bytes(),
            owner.key.to_bytes(),
            rent_terms,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    let pool_bytes = encode_pool(facts.pool)?;
    let lp_bytes = lp
        .to_bytes()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let payer_before = payer.lamports();
    let payer_after = payer_before
        .checked_sub(funded_rent)
        .ok_or(AdapterError::PositionRentUnderfunded)?;
    let create = create_account(
        payer.key,
        lp_account.key,
        funded_rent,
        u64::try_from(lp_bytes.len()).map_err(|_| AdapterError::Arithmetic)?,
        program_id,
    );
    preflight_mutable(&[payer, pool_account, lp_account])?;
    let bump_seed = [bump];
    let signer = append_bump(&lp_seeds.seed_components(), &bump_seed)?;
    let signer_refs: Vec<&[u8]> = signer.iter().map(Vec::as_slice).collect();
    invoke_signed(
        &create,
        &[payer.clone(), lp_account.clone(), system.clone()],
        &[signer_refs.as_slice()],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    if payer.lamports() != payer_after
        || lp_account.lamports() != funded_rent
        || lp_account.owner != program_id
        || lp_account.data_len() != lp_bytes.len()
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    persist_pool(pool_account, &pool_bytes, facts.pool)?;
    persist_lp(lp_account, &lp_bytes, lp)?;
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)
}

#[allow(clippy::too_many_arguments)]
fn change_liquidity<const N: usize, const B: usize, R>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    lp_id: [u8; 32],
    request: R,
    is_add: bool,
) -> Result<(), ProgramError>
where
    R: Copy + LiquidityRequest<N, B>,
{
    let actor = account(accounts, 0)?;
    let realm_account = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let pool_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let config_staging = account(accounts, 5)?;
    let lp_account = account(accounts, 6)?;
    let participant_account = account(accounts, 7)?;
    let pool_position_account = account(accounts, 8)?;
    let user_collateral = account(accounts, 9)?;
    let principal_vault = account(accounts, 10)?;
    let fee_vault = account(accounts, 11)?;
    let mint = account(accounts, 12)?;
    let token_program = account(accounts, 13)?;
    let action = if is_add {
        DealerActionV1::AddLiquidity
    } else {
        DealerActionV1::RemoveLiquidity
    };
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market_account,
        pool_account,
        config_account,
        config_staging,
        action,
    )?;
    let realm = authenticate_realm(program_id, realm_account, mint, token_program, facts.market)?;
    let mut lp = authenticate_lp(
        program_id,
        lp_account,
        market_account,
        pool_account,
        facts.config.content_id(),
        lp_id,
        actor.key,
        facts.market.root().identity().generation(),
    )?;
    let mut participant = authenticate_position::<N>(
        program_id,
        participant_account,
        market_account,
        actor.key,
        facts.market.root().identity().generation(),
    )?;
    let mut pool_position = authenticate_position::<N>(
        program_id,
        pool_position_account,
        market_account,
        pool_account.key,
        facts.market.root().identity().generation(),
    )?;
    let principal_before = authenticate_pool_vault(
        program_id,
        principal_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        facts.pool.liquidity().principal_collateral(),
    )?;
    let fee_before = authenticate_pool_vault(
        program_id,
        fee_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        facts.pool.liquidity().realized_fee_collateral(),
    )?;
    require_claim_coverage(facts.pool.liquidity(), pool_position)?;

    let receipt = request.apply(
        &mut facts.pool,
        pool_account.key.to_bytes(),
        &facts.config,
        lp_account.key.to_bytes(),
        &mut lp,
    )?;
    let amounts = receipt;
    for (index, quantity) in amounts.claim_reserves().iter().copied().enumerate() {
        if quantity == 0 {
            continue;
        }
        if is_add {
            participant
                .debit_outcome(index, quantity)
                .and_then(|()| pool_position.credit_outcome(index, quantity))
                .map_err(|_| AdapterError::MarketTransition)?;
        } else {
            pool_position
                .debit_outcome(index, quantity)
                .and_then(|()| participant.credit_outcome(index, quantity))
                .map_err(|_| AdapterError::MarketTransition)?;
        }
    }
    let principal = amounts.principal_collateral();
    let fees = amounts.realized_fee_collateral();
    let pool_bytes = encode_pool(facts.pool)?;
    let lp_bytes = lp
        .to_bytes()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let participant_bytes = encode_position(participant)?;
    let pool_position_bytes = encode_position(pool_position)?;
    let pool_signer = pool_signer(&facts, market_account, pool_account)?;
    preflight_mutable(&[
        pool_account,
        lp_account,
        participant_account,
        pool_position_account,
        user_collateral,
        principal_vault,
        fee_vault,
    ])?;
    if is_add {
        transfer_if_nonzero(
            user_collateral,
            principal_vault,
            mint,
            token_program,
            realm,
            actor,
            principal,
            None,
        )?;
        transfer_if_nonzero(
            user_collateral,
            fee_vault,
            mint,
            token_program,
            realm,
            actor,
            fees,
            None,
        )?;
    } else {
        transfer_if_nonzero(
            principal_vault,
            user_collateral,
            mint,
            token_program,
            realm,
            pool_account,
            principal,
            Some(pool_signer.as_slice()),
        )?;
        transfer_if_nonzero(
            fee_vault,
            user_collateral,
            mint,
            token_program,
            realm,
            pool_account,
            fees,
            Some(pool_signer.as_slice()),
        )?;
    }
    persist_pool(pool_account, &pool_bytes, facts.pool)?;
    persist_lp(lp_account, &lp_bytes, lp)?;
    persist_position(participant_account, &participant_bytes, participant)?;
    persist_position(pool_position_account, &pool_position_bytes, pool_position)?;
    require_vault_minimum(
        principal_vault,
        token_program,
        realm,
        principal_before.amount,
        principal,
        is_add,
    )?;
    require_vault_minimum(
        fee_vault,
        token_program,
        realm,
        fee_before.amount,
        fees,
        is_add,
    )?;
    Ok(())
}

trait LiquidityRequest<const N: usize, const B: usize>: Copy {
    fn apply(
        self,
        pool: &mut PoolState<N, B>,
        pool_key: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        lp_key: [u8; 32],
        lp: &mut LpPosition,
    ) -> Result<LiquidityAmounts<N>, ProgramError>;
}

impl<const N: usize, const B: usize> LiquidityRequest<N, B>
    for dclutch_dealer_contract::AddLiquidityRequest<N>
{
    fn apply(
        self,
        pool: &mut PoolState<N, B>,
        pool_key: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        lp_key: [u8; 32],
        lp: &mut LpPosition,
    ) -> Result<LiquidityAmounts<N>, ProgramError> {
        pool.add_liquidity(pool_key, config, lp_key, lp, self)
            .map(|receipt| receipt.amounts_transferred())
            .map_err(|_| AdapterError::MarketTransition.into())
    }
}

impl<const N: usize, const B: usize> LiquidityRequest<N, B>
    for dclutch_dealer_contract::RemoveLiquidityRequest<N>
{
    fn apply(
        self,
        pool: &mut PoolState<N, B>,
        pool_key: [u8; 32],
        config: &LiquidityConfigV1<N, B>,
        lp_key: [u8; 32],
        lp: &mut LpPosition,
    ) -> Result<LiquidityAmounts<N>, ProgramError> {
        pool.remove_liquidity(pool_key, config, lp_key, lp, self)
            .map(|receipt| receipt.amounts_transferred())
            .map_err(|_| AdapterError::MarketTransition.into())
    }
}

fn trade<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: dclutch_dealer_contract::TradeRequest,
) -> Result<(), ProgramError> {
    let trader = account(accounts, 0)?;
    let realm_account = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let pool_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let config_staging = account(accounts, 5)?;
    let participant_account = account(accounts, 6)?;
    let pool_position_account = account(accounts, 7)?;
    let user_collateral = account(accounts, 8)?;
    let principal_vault = account(accounts, 9)?;
    let fee_vault = account(accounts, 10)?;
    let mint = account(accounts, 11)?;
    let token_program = account(accounts, 12)?;
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market_account,
        pool_account,
        config_account,
        config_staging,
        DealerActionV1::Trade,
    )?;
    let realm = authenticate_realm(program_id, realm_account, mint, token_program, facts.market)?;
    let generation = facts.market.root().identity().generation();
    let mut trader_position = authenticate_position::<N>(
        program_id,
        participant_account,
        market_account,
        trader.key,
        generation,
    )?;
    let mut pool_position = authenticate_position::<N>(
        program_id,
        pool_position_account,
        market_account,
        pool_account.key,
        generation,
    )?;
    authenticate_pool_vault(
        program_id,
        principal_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        facts.pool.liquidity().principal_collateral(),
    )?;
    authenticate_pool_vault(
        program_id,
        fee_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        facts.pool.liquidity().realized_fee_collateral(),
    )?;
    require_claim_coverage(facts.pool.liquidity(), pool_position)?;
    let receipt = facts
        .pool
        .execute(pool_account.key.to_bytes(), &facts.config, request)
        .map_err(|_| AdapterError::MarketTransition)?;
    let claim = usize::from(receipt.claim_index());
    match receipt.side() {
        TradeSide::BuyClaimFromPool => pool_position
            .debit_outcome(claim, receipt.quantity())
            .and_then(|()| trader_position.credit_outcome(claim, receipt.quantity()))
            .map_err(|_| AdapterError::MarketTransition)?,
        TradeSide::SellClaimToPool => trader_position
            .debit_outcome(claim, receipt.quantity())
            .and_then(|()| pool_position.credit_outcome(claim, receipt.quantity()))
            .map_err(|_| AdapterError::MarketTransition)?,
    }
    let pool_bytes = encode_pool(facts.pool)?;
    let trader_position_bytes = encode_position(trader_position)?;
    let pool_position_bytes = encode_position(pool_position)?;
    let pool_signer = pool_signer(&facts, market_account, pool_account)?;
    preflight_mutable(&[
        pool_account,
        participant_account,
        pool_position_account,
        user_collateral,
        principal_vault,
        fee_vault,
    ])?;
    match receipt.side() {
        TradeSide::BuyClaimFromPool => {
            transfer_if_nonzero(
                user_collateral,
                principal_vault,
                mint,
                token_program,
                realm,
                trader,
                receipt.notional_collateral(),
                None,
            )?;
            transfer_if_nonzero(
                user_collateral,
                fee_vault,
                mint,
                token_program,
                realm,
                trader,
                receipt.trader_fee_collateral(),
                None,
            )?;
        }
        TradeSide::SellClaimToPool => {
            transfer_if_nonzero(
                principal_vault,
                user_collateral,
                mint,
                token_program,
                realm,
                pool_account,
                receipt.notional_collateral(),
                Some(pool_signer.as_slice()),
            )?;
            transfer_if_nonzero(
                user_collateral,
                fee_vault,
                mint,
                token_program,
                realm,
                trader,
                receipt.trader_fee_collateral(),
                None,
            )?;
        }
    }
    persist_pool(pool_account, &pool_bytes, facts.pool)?;
    persist_position(participant_account, &trader_position_bytes, trader_position)?;
    persist_position(pool_position_account, &pool_position_bytes, pool_position)?;
    let mut return_data = exact_zeroed(
        dclutch_dealer_contract::ExecutionReceipt::<B>::encoded_len()
            .map_err(|_| AdapterError::Arithmetic)?,
    )?;
    receipt
        .encode_into(&mut return_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    set_return_data(&return_data);
    Ok(())
}

fn reset_ladder<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_pool_sequence: u64,
) -> Result<(), ProgramError> {
    let market = account(accounts, 0)?;
    let pool = account(accounts, 1)?;
    let config = account(accounts, 2)?;
    let config_staging = account(accounts, 3)?;
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market,
        pool,
        config,
        config_staging,
        DealerActionV1::ResetLadder,
    )?;
    let slot = Clock::get()
        .map_err(|_| AdapterError::PositionAuthentication)?
        .slot;
    facts
        .pool
        .reset_ladder(
            pool.key.to_bytes(),
            &facts.config,
            expected_pool_sequence,
            slot,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    let bytes = encode_pool(facts.pool)?;
    preflight_mutable(&[pool])?;
    persist_pool(pool, &bytes, facts.pool)
}

fn close_lp_position<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: dclutch_dealer_contract::instruction::CloseLpPositionV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, 0)?;
    let market = account(accounts, 1)?;
    let pool = account(accounts, 2)?;
    let config = account(accounts, 3)?;
    let config_staging = account(accounts, 4)?;
    let lp_account = account(accounts, 5)?;
    let rent_credit_account = account(accounts, 6)?;
    authenticate_system(account(accounts, 7)?)?;
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market,
        pool,
        config,
        config_staging,
        DealerActionV1::CloseLpPosition,
    )?;
    let mut lp = authenticate_lp(
        program_id,
        lp_account,
        market,
        pool,
        facts.config.content_id(),
        request.lp_id(),
        owner.key,
        facts.market.root().identity().generation(),
    )?;
    let credit = authenticate_rent_credit(program_id, rent_credit_account, owner.key)?;
    let receipt = facts
        .pool
        .close_position(
            pool.key.to_bytes(),
            lp_account.key.to_bytes(),
            &mut lp,
            request.expected_pool_sequence(),
            request.expected_position_sequence(),
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    if receipt.rent_credit() != lp.rent_credit() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let pool_bytes = encode_pool(facts.pool)?;
    let source_lamports = lp_account.lamports();
    if source_lamports < receipt.rent_credit().funded_rent_principal() {
        return Err(AdapterError::PositionRentUnderfunded.into());
    }
    let plan = SourceCloseCreditPlanV1::new(
        source_lamports,
        rent_credit_account.lamports(),
        source_lamports,
    )
    .map_err(|_| AdapterError::Arithmetic)?;
    preflight_mutable(&[pool, lp_account, rent_credit_account])?;
    persist_pool(pool, &pool_bytes, facts.pool)?;
    close_program_account(lp_account, rent_credit_account, plan)?;
    require_unchanged_rent_credit(program_id, rent_credit_account, credit)
}

fn retire<const N: usize, const B: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_pool_sequence: u64,
    expected_market_child_count: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let realm_account = account(accounts, 1)?;
    let pool_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_staging = account(accounts, 4)?;
    let pool_position_account = account(accounts, 5)?;
    let principal_vault = account(accounts, 6)?;
    let fee_vault = account(accounts, 7)?;
    let service_vault = account(accounts, 8)?;
    let refund_vault = account(accounts, 9)?;
    let refund_position_account = account(accounts, 10)?;
    let pool_position_credit = account(accounts, 11)?;
    let pool_credit = account(accounts, 12)?;
    let mint = account(accounts, 13)?;
    let token_program = account(accounts, 14)?;
    authenticate_system(account(accounts, 15)?)?;
    let mut facts = authenticate_pool::<N, B>(
        program_id,
        market_account,
        pool_account,
        config_account,
        config_staging,
        DealerActionV1::RetirePool,
    )?;
    let realm = authenticate_realm(program_id, realm_account, mint, token_program, facts.market)?;
    let generation = facts.market.root().identity().generation();
    let beneficiary = Pubkey::new_from_array(facts.pool.attachment().service_refund_beneficiary());
    let mut pool_position = authenticate_position::<N>(
        program_id,
        pool_position_account,
        market_account,
        pool_account.key,
        generation,
    )?;
    let mut refund_position = authenticate_position::<N>(
        program_id,
        refund_position_account,
        market_account,
        &beneficiary,
        generation,
    )?;
    let principal = authenticate_pool_vault(
        program_id,
        principal_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::Principal,
        facts.pool.liquidity().principal_collateral(),
    )?;
    let fees = authenticate_pool_vault(
        program_id,
        fee_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::RealizedFees,
        facts.pool.liquidity().realized_fee_collateral(),
    )?;
    let service = authenticate_pool_vault(
        program_id,
        service_vault,
        pool_account,
        mint,
        token_program,
        realm,
        DealerCollateralCompartmentV1::Service,
        facts.pool.service_funding(),
    )?;
    authenticate_destination_vault(refund_vault, mint, token_program, realm, &beneficiary)?;
    let pool_position_rent =
        authenticate_rent_credit(program_id, pool_position_credit, pool_account.key)?;
    let pool_rent = authenticate_rent_credit(
        program_id,
        pool_credit,
        &Pubkey::new_from_array(facts.pool.rent_credit().beneficiary()),
    )?;
    let plan = retire_pool(
        facts.market.root(),
        facts.pool,
        pool_account.key.to_bytes(),
        &facts.config,
        expected_pool_sequence,
        expected_market_child_count,
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    facts.pool = plan.pool();
    let mut market_after = facts.market;
    market_after
        .retire_child(generation, expected_market_child_count)
        .map_err(|_| AdapterError::MarketTransition)?;
    if market_after.root() != plan.market() {
        return Err(AdapterError::MarketTransition.into());
    }
    let pool_claim_gifts = *pool_position.balances();
    for (index, quantity) in pool_claim_gifts.iter().copied().enumerate() {
        if quantity > 0 {
            pool_position
                .debit_outcome(index, quantity)
                .and_then(|()| refund_position.credit_outcome(index, quantity))
                .map_err(|_| AdapterError::MarketTransition)?;
        }
    }
    let market_bytes = encode_market(market_after)?;
    let refund_position_bytes = encode_position(refund_position)?;
    let pool_signer = pool_signer(&facts, market_account, pool_account)?;
    preflight_mutable(&[
        market_account,
        pool_account,
        pool_position_account,
        principal_vault,
        fee_vault,
        service_vault,
        refund_vault,
        refund_position_account,
        pool_position_credit,
        pool_credit,
    ])?;
    for (source, amount) in [
        (principal_vault, principal.amount),
        (fee_vault, fees.amount),
        (service_vault, service.amount),
    ] {
        transfer_if_nonzero(
            source,
            refund_vault,
            mint,
            token_program,
            realm,
            pool_account,
            amount,
            Some(pool_signer.as_slice()),
        )?;
        close_token_vault(
            source,
            pool_credit,
            pool_account,
            token_program,
            realm.release,
            pool_signer.as_slice(),
        )?;
    }
    persist_market(market_account, &market_bytes, market_after)?;
    persist_position(
        refund_position_account,
        &refund_position_bytes,
        refund_position,
    )?;
    close_with_all_lamports(pool_position_account, pool_position_credit)?;
    close_with_all_lamports(pool_account, pool_credit)?;
    require_unchanged_rent_credit(program_id, pool_position_credit, pool_position_rent)?;
    require_unchanged_rent_credit(program_id, pool_credit, pool_rent)
}

fn authenticate_market<const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    if market_account.owner != program_id || market_account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let market = CategoricalMarketV1::<N>::decode(&data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if market.root().identity().generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_account.key != &expected || encode_market(market)?.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(market)
}

fn authenticate_config<const N: usize, const B: usize>(
    program_id: &Pubkey,
    config_account: &AccountInfo<'_>,
    config_staging: &AccountInfo<'_>,
) -> Result<(LiquidityConfigV1<N, B>, ContentId), ProgramError> {
    if config_account.owner != program_id || config_account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = config_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let content_id =
        ContentId::new(hash(&data).to_bytes()).map_err(|_| AdapterError::ContentIdentity)?;
    let config = LiquidityConfigV1::<N, B>::decode(content_id, &data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if encode_config(config)?.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let seeds = ConfigPdaSeedsV1::new(content_id);
    let (expected, _) = find_pda(program_id, &seeds.seed_components());
    if config_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    authenticate_config_staging(program_id, config_staging, content_id)?;
    Ok((config, content_id))
}

fn authenticate_config_staging(
    program_id: &Pubkey,
    config_staging: &AccountInfo<'_>,
    content_id: ContentId,
) -> Result<(), ProgramError> {
    let digest = content_id.to_bytes();
    let (expected, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &dclutch_dealer_contract::frame::DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        program_id,
    );
    if config_staging.key != &expected
        || config_staging.owner != &system_program::ID
        || config_staging.executable
        || !config_staging.try_data_is_empty().unwrap_or(false)
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_pool<const N: usize, const B: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    pool_account: &AccountInfo<'_>,
    config_account: &AccountInfo<'_>,
    config_staging: &AccountInfo<'_>,
    action: DealerActionV1,
) -> Result<PoolFacts<N, B>, ProgramError> {
    if market_account.owner != program_id
        || pool_account.owner != program_id
        || config_account.owner != program_id
        || market_account.executable
        || pool_account.executable
        || config_account.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let market = CategoricalMarketV1::<N>::decode(&market_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_account.key != &expected_market {
        return Err(AdapterError::AccountIdentity.into());
    }
    if encode_market(market)?.as_slice() != &market_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    validate_market_phase(action, market.root().phase())
        .map_err(|_| AdapterError::ReplayMismatch)?;
    drop(market_data);

    let pool_data = pool_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let pool =
        PoolState::<N, B>::decode(&pool_data).map_err(|_| AdapterError::PositionAuthentication)?;
    let attachment = pool.attachment();
    if attachment.market() != market.root().identity() {
        return Err(AdapterError::ContentIdentity.into());
    }
    let pool_seeds = PoolPdaSeedsV1::new(
        market_account.key.to_bytes(),
        market.root().identity().generation(),
        attachment.liquidity_config_id(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_pool, pool_bump) = find_pda(program_id, &pool_seeds.seed_components());
    if pool_account.key != &expected_pool || encode_pool(pool)?.as_slice() != &pool_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(pool_data);

    let config_data = config_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let content_id =
        ContentId::new(hash(&config_data).to_bytes()).map_err(|_| AdapterError::ContentIdentity)?;
    if content_id != attachment.liquidity_config_id() {
        return Err(AdapterError::ContentIdentity.into());
    }
    let config = LiquidityConfigV1::<N, B>::decode(content_id, &config_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let config_seeds = ConfigPdaSeedsV1::new(content_id);
    let (expected_config, _) = find_pda(program_id, &config_seeds.seed_components());
    if config_account.key != &expected_config {
        return Err(AdapterError::AccountIdentity.into());
    }
    let encoded_config = encode_config(config)?;
    if encoded_config.as_slice() != &config_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    authenticate_config_staging(program_id, config_staging, content_id)?;
    pool.validate_against(pool_account.key.to_bytes(), &config)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(PoolFacts {
        market,
        pool,
        config,
        pool_bump,
    })
}

fn authenticate_realm<const N: usize>(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market: CategoricalMarketV1<N>,
) -> Result<RealmFacts, ProgramError> {
    if realm_account.owner != program_id
        || mint_account.owner != token_program.key
        || !recognized_program_loader(token_program.owner)
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let realm_data = realm_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| AdapterError::PositionAuthentication)?;
    if realm.to_bytes().as_slice() != &realm_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let realm_digest = hash(&realm_data).to_bytes();
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], program_id);
    if realm_account.key != &expected_realm
        || market.root().identity().realm_id().to_bytes() != realm_digest
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(RealmFacts {
        realm,
        release,
        mint,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_pool_vault(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    pool: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    compartment: DealerCollateralCompartmentV1,
    categorized_minimum: u64,
) -> Result<TokenAccount, ProgramError> {
    let seeds = DealerCollateralVaultPdaSeedsV1::new(pool.key.to_bytes(), compartment)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected, _) = find_pda(program_id, &seeds.seed_components());
    if vault.key != &expected || vault.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let token = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            pool.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if token.amount < categorized_minimum {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(token)
}

fn authenticate_destination_vault(
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    beneficiary: &Pubkey,
) -> Result<TokenAccount, ProgramError> {
    if vault.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            beneficiary.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_lp(
    program_id: &Pubkey,
    lp_account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    pool: &AccountInfo<'_>,
    config_id: ContentId,
    lp_id: [u8; 32],
    owner: &Pubkey,
    generation: u64,
) -> Result<LpPosition, ProgramError> {
    if lp_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let seeds = LpPositionPdaSeedsV1::new(market.key.to_bytes(), generation, config_id, lp_id)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected, _) = find_pda(program_id, &seeds.seed_components());
    let data = lp_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position = LpPosition::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    if lp_account.key != &expected
        || position.parent().address() != pool.key.to_bytes()
        || position.parent().market_generation() != generation
        || position.owner() != owner.to_bytes()
        || position
            .to_bytes()
            .map_err(|_| AdapterError::PositionAuthentication)?
            .as_slice()
            != &data[..]
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(position)
}

fn authenticate_position<const N: usize>(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    owner: &Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    if position_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (expected, _) = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market.key.as_ref(), owner.as_ref()],
        program_id,
    );
    if position_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position =
        PositionV1::<N>::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    if position.market() != market.key.as_ref()
        || position.owner() != owner.as_ref()
        || position.generation() != generation
        || encode_position(position)?.as_slice() != &data[..]
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(position)
}

fn require_claim_coverage<const N: usize>(
    amounts: LiquidityAmounts<N>,
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
    if position
        .balances()
        .iter()
        .zip(amounts.claim_reserves().iter())
        .any(|(physical, categorized)| physical < categorized)
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    beneficiary: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority = RefundAuthority::new(beneficiary.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
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
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::PositionAuthentication)?;
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
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
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

fn pool_signer<const N: usize, const B: usize>(
    facts: &PoolFacts<N, B>,
    market: &AccountInfo<'_>,
    pool: &AccountInfo<'_>,
) -> Result<Vec<Vec<u8>>, ProgramError> {
    let seeds = PoolPdaSeedsV1::new(
        market.key.to_bytes(),
        facts.market.root().identity().generation(),
        facts.config.content_id(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(5)
        .map_err(|_| AdapterError::Arithmetic)?;
    for seed in seeds.seed_components() {
        output.push(seed.to_vec());
    }
    output.push(Vec::from([facts.pool_bump]));
    if Pubkey::create_program_address(
        &output.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        pool.owner,
    ) != Ok(*pool.key)
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn transfer_if_nonzero<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    realm: RealmFacts,
    authority: &AccountInfo<'a>,
    quantity: u64,
    signer: Option<&[Vec<u8>]>,
) -> Result<(), ProgramError> {
    if quantity == 0 {
        return Ok(());
    }
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let checked = realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: token_program.key.to_bytes(),
            mint_address: mint.key.to_bytes(),
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &destination_data,
            authority: authority.key.to_bytes(),
            amount: quantity,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if checked.mint() != realm.mint {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let before = TransferFacts {
        source: checked.source(),
        destination: checked.destination(),
        authority_role: checked.authority_role(),
        source_lamports: source.lamports(),
        destination_lamports: destination.lamports(),
        mint_lamports: mint.lamports(),
    };
    let instruction = checked_transfer_instruction(
        realm.release,
        source.key,
        mint.key,
        destination.key,
        authority.key,
        quantity,
        realm.mint.decimals,
    )?;
    let infos = [
        source.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    if let Some(seed_bytes) = signer {
        let seed_refs: Vec<&[u8]> = seed_bytes.iter().map(Vec::as_slice).collect();
        invoke_signed(&instruction, &infos, &[seed_refs.as_slice()])
            .map_err(|_| AdapterError::CollateralTransferCpi)?;
    } else {
        invoke(&instruction, &infos).map_err(|_| AdapterError::CollateralTransferCpi)?;
    }
    authenticate_transfer_post(
        source,
        destination,
        mint,
        token_program,
        realm,
        before,
        quantity,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_transfer_post(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: TransferFacts,
    quantity: u64,
) -> Result<(), ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
        || source.lamports() != before.source_lamports
        || destination.lamports() != before.destination_lamports
        || mint.lamports() != before.mint_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mint_after = realm
        .release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let source_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &source_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let destination_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &destination_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mut expected_source = before.source;
    expected_source.amount = expected_source
        .amount
        .checked_sub(quantity)
        .ok_or(AdapterError::PositionPostcondition)?;
    if before.authority_role == AuthorityRole::Delegate {
        expected_source.delegated_amount = expected_source
            .delegated_amount
            .checked_sub(quantity)
            .ok_or(AdapterError::PositionPostcondition)?;
    }
    let mut expected_destination = before.destination;
    expected_destination.amount = expected_destination
        .amount
        .checked_add(quantity)
        .ok_or(AdapterError::PositionPostcondition)?;
    if mint_after != realm.mint
        || source_after != expected_source
        || destination_after != expected_destination
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_transfer_instruction(
    release: CollateralAdapterReleaseV1,
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    quantity: u64,
    decimals: u8,
) -> Result<Instruction, ProgramError> {
    let spec = transfer_checked(
        release.token_program(),
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
        quantity,
        decimals,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let accounts = Vec::from([
        AccountMeta::new(*source, false),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(*destination, false),
        AccountMeta::new_readonly(*authority, true),
    ]);
    Ok(Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts,
        data: Vec::from(*spec.data()),
    })
}

fn close_token_vault<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    release: CollateralAdapterReleaseV1,
    signer: &[Vec<u8>],
) -> Result<(), ProgramError> {
    let spec = close_account(
        release.token_program(),
        source.key.to_bytes(),
        destination.key.to_bytes(),
        authority.key.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let instruction = Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ]),
        data: Vec::from(*spec.data()),
    };
    let seed_refs: Vec<&[u8]> = signer.iter().map(Vec::as_slice).collect();
    invoke_signed(
        &instruction,
        &[
            source.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[seed_refs.as_slice()],
    )
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    if source.lamports() != 0 || !source.data_is_empty() {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn require_vault_minimum(
    vault: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: u64,
    delta: u64,
    is_add: bool,
) -> Result<(), ProgramError> {
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let expected = if is_add {
        before.checked_add(delta)
    } else {
        before.checked_sub(delta)
    }
    .ok_or(AdapterError::PositionPostcondition)?;
    if after.amount != expected {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn authenticate_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    authenticate_system(system)?;
    if rent.key != &sysvar::rent::ID || rent.owner != &sysvar::ID {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

fn authenticate_system(system: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if system.key != &system_program::ID || system.owner != &native_loader::ID {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

fn require_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.lamports() != 0 {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_pda_account<'a>(
    _program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    lamports: u64,
    space: usize,
    owner: &Pubkey,
    seeds: &[&[u8]],
    bump: u8,
) -> Result<(), ProgramError> {
    let instruction = create_account(
        payer.key,
        account.key,
        lamports,
        u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?,
        owner,
    );
    let bump_seed = [bump];
    let signer = append_bump(seeds, &bump_seed)?;
    let signer_refs: Vec<&[u8]> = signer.iter().map(Vec::as_slice).collect();
    invoke_signed(
        &instruction,
        &[payer.clone(), account.clone(), system.clone()],
        &[signer_refs.as_slice()],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    if account.lamports() != lamports || account.owner != owner || account.data_len() != space {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn initialize_token_vault<'a>(
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    release: CollateralAdapterReleaseV1,
    owner: &Pubkey,
) -> Result<(), ProgramError> {
    let spec = initialize_account3(
        release.token_program(),
        vault.key.to_bytes(),
        mint.key.to_bytes(),
        owner.to_bytes(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let instruction = Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(*vault.key, false),
            AccountMeta::new_readonly(*mint.key, false),
        ]),
        data: Vec::from(*spec.data()),
    };
    invoke(
        &instruction,
        &[vault.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let account = release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            owner.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if account.amount != 0 {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn find_pda(program_id: &Pubkey, seeds: &[&[u8]]) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

fn append_bump(seeds: &[&[u8]], bump: &[u8; 1]) -> Result<Vec<Vec<u8>>, ProgramError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(seeds.len().checked_add(1).ok_or(AdapterError::Arithmetic)?)
        .map_err(|_| AdapterError::Arithmetic)?;
    for seed in seeds {
        output.push(seed.to_vec());
    }
    output.push(bump.to_vec());
    Ok(output)
}

fn preflight_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for account in accounts {
        if !account.is_writable {
            return Err(AdapterError::AccountPrivilege.into());
        }
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?;
        drop(lamports);
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::PositionAuthentication)?;
        drop(data);
    }
    Ok(())
}

fn close_program_account(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    plan: SourceCloseCreditPlanV1,
) -> Result<(), ProgramError> {
    {
        let mut destination_balance = destination
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        let mut source_balance = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        **destination_balance = plan.credit_after();
        **source_balance = 0;
    }
    source.resize(0).map_err(|_| AdapterError::PositionClose)?;
    source.assign(&system_program::ID);
    plan.validate_post(source.lamports(), destination.lamports())
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

fn close_with_all_lamports(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let source_lamports = source.lamports();
    let plan =
        SourceCloseCreditPlanV1::new(source_lamports, destination.lamports(), source_lamports)
            .map_err(|_| AdapterError::Arithmetic)?;
    close_program_account(source, destination, plan)
}

fn persist_pool<const N: usize, const B: usize>(
    account: &AccountInfo<'_>,
    bytes: &[u8],
    expected: PoolState<N, B>,
) -> Result<(), ProgramError> {
    persist_bytes(account, bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if PoolState::<N, B>::decode(&data) != Ok(expected) || &data[..] != bytes {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn persist_lp(
    account: &AccountInfo<'_>,
    bytes: &[u8],
    expected: LpPosition,
) -> Result<(), ProgramError> {
    persist_bytes(account, bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if LpPosition::decode(&data) != Ok(expected) || &data[..] != bytes {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn persist_position<const N: usize>(
    account: &AccountInfo<'_>,
    bytes: &[u8],
    expected: PositionV1<N>,
) -> Result<(), ProgramError> {
    persist_bytes(account, bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if PositionV1::<N>::decode(&data) != Ok(expected) || &data[..] != bytes {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn persist_market<const N: usize>(
    account: &AccountInfo<'_>,
    bytes: &[u8],
    expected: CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    persist_bytes(account, bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if CategoricalMarketV1::<N>::decode(&data) != Ok(expected) || &data[..] != bytes {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn persist_bytes(account: &AccountInfo<'_>, bytes: &[u8]) -> Result<(), ProgramError> {
    let mut output = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if output.len() != bytes.len() {
        return Err(AdapterError::PositionPostcondition.into());
    }
    output.copy_from_slice(bytes);
    Ok(())
}

fn encode_pool<const N: usize, const B: usize>(
    value: PoolState<N, B>,
) -> Result<Vec<u8>, ProgramError> {
    let mut bytes =
        exact_zeroed(PoolState::<N, B>::encoded_len().map_err(|_| AdapterError::Arithmetic)?)?;
    value
        .encode_into(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn encode_config<const N: usize, const B: usize>(
    value: LiquidityConfigV1<N, B>,
) -> Result<Vec<u8>, ProgramError> {
    let mut bytes = exact_zeroed(
        LiquidityConfigV1::<N, B>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    )?;
    value
        .encode_into(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn encode_position<const N: usize>(value: PositionV1<N>) -> Result<Vec<u8>, ProgramError> {
    let mut bytes =
        exact_zeroed(PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?)?;
    value
        .encode(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn encode_market<const N: usize>(value: CategoricalMarketV1<N>) -> Result<Vec<u8>, ProgramError> {
    let mut bytes = exact_zeroed(
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    )?;
    value
        .encode(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn exact_zeroed(length: usize) -> Result<Vec<u8>, ProgramError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    output.resize(length, 0);
    Ok(output)
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| AdapterError::AccountFrameLength.into())
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec, vec::Vec};

    use super::*;
    use dclutch_dealer_contract::frame::{
        DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, DealerAccountRoleV1, dealer_account_role,
    };

    fn test_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn header(action: u8) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&DEALER_INSTRUCTION_MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[ACTION_OFFSET] = action;
        bytes
    }

    #[test]
    fn activation_uses_the_typed_funding_frame() {
        let route = select_route(&header(DealerActionV1::ActivatePool as u8)).expect("route");
        assert_eq!(route.action, DealerActionV1::ActivatePool);
        assert_eq!(route.market_index, 3);
        assert_eq!(route.config_index, 8);
    }

    #[test]
    fn exact_routes_select_their_market_and_config_roles() {
        let expected = [
            (1, DealerActionV1::ActivatePool, 3, 8),
            (2, DealerActionV1::CreateLpPosition, 2, 4),
            (3, DealerActionV1::AddLiquidity, 2, 4),
            (4, DealerActionV1::RemoveLiquidity, 2, 4),
            (5, DealerActionV1::Trade, 2, 4),
            (6, DealerActionV1::ResetLadder, 0, 2),
            (7, DealerActionV1::CloseLpPosition, 1, 3),
            (8, DealerActionV1::RetirePool, 0, 3),
        ];
        for (tag, action, market, config) in expected {
            let route = select_route(&header(tag)).expect("route");
            assert_eq!(route.action, action);
            assert_eq!(route.market_index, market);
            assert_eq!(route.config_index, config);
        }
    }

    #[test]
    fn unknown_action_and_truncated_header_refuse() {
        for tag in [0, 9, u8::MAX] {
            assert!(select_route(&header(tag)).is_err());
        }
        assert!(select_route(&[0; ACTION_OFFSET]).is_err());
    }

    #[test]
    fn semantic_routes_agree_with_owned_frame_roles() {
        for action in [
            DealerActionV1::ActivatePool,
            DealerActionV1::CreateLpPosition,
            DealerActionV1::AddLiquidity,
            DealerActionV1::RemoveLiquidity,
            DealerActionV1::Trade,
            DealerActionV1::ResetLadder,
            DealerActionV1::CloseLpPosition,
            DealerActionV1::RetirePool,
        ] {
            let route = select_route(&header(action as u8)).expect("route");
            assert_eq!(
                dealer_account_role::<2>(action, route.market_index),
                Ok(DealerAccountRoleV1::Market)
            );
            assert_eq!(
                dealer_account_role::<16>(action, route.config_index),
                Ok(DealerAccountRoleV1::LiquidityConfig)
            );
            assert_eq!(
                dealer_account_role::<16>(action, route.config_index + 1),
                Ok(DealerAccountRoleV1::LiquidityConfigStaging)
            );
        }
    }

    #[test]
    fn finalized_config_requires_the_exact_vacant_staging_pda() {
        let program_id = Pubkey::new_unique();
        let content_id = ContentId::new([7; 32]).expect("content ID");
        let digest = content_id.to_bytes();
        let (staging_key, _) = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
                &digest,
            ],
            &program_id,
        );
        let finalized = test_account(
            staging_key,
            false,
            false,
            0,
            vec![],
            system_program::ID,
            false,
        );
        assert_eq!(
            authenticate_config_staging(&program_id, &finalized, content_id),
            Ok(())
        );

        // Unsolicited lamports cannot make an otherwise finalized record
        // unusable; the generic record lifecycle classifies this as prefunded
        // vacancy rather than mutable staging authority.
        let prefunded = test_account(
            staging_key,
            false,
            false,
            1,
            vec![],
            system_program::ID,
            false,
        );
        assert_eq!(
            authenticate_config_staging(&program_id, &prefunded, content_id),
            Ok(())
        );

        for hostile in [
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                system_program::ID,
                false,
            ),
            test_account(
                staging_key,
                false,
                false,
                0,
                vec![1],
                system_program::ID,
                false,
            ),
            test_account(staging_key, false, false, 0, vec![], program_id, false),
            test_account(
                staging_key,
                false,
                false,
                0,
                vec![],
                system_program::ID,
                true,
            ),
        ] {
            assert!(authenticate_config_staging(&program_id, &hostile, content_id).is_err());
        }
    }
}
