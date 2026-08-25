//! Exact-width SVM boundary for the first executable General lifecycle.
//!
//! Activation consumes only a typed native-lamport capability quote, authenticates
//! the permanent finalized config, founds root and segregated funding accounts, and leaves
//! the generic funding state at its exact Rent reserve. Batch creation then
//! reimburses its permissionless actor from prepaid liveness before creating
//! the exact batch PDA. Order routes admit, lock, cancel, and close Position
//! plus quote custody without introducing adapter-owned economic state.

use alloc::{boxed::Box, vec::Vec};

use dclutch_capability_contract::{
    CapabilityManifestV1, FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_collateral_contract::COLLATERAL_VAULT_PDA_DOMAIN;
use dclutch_general_contract::{
    ActivateGeneralV1, BATCH_ROOT_BYTES, BatchCapitalizationV1, BatchPhase, BatchRentObservationV1,
    BatchRootV1, CandidateCapitalizationV1, CandidatePageV1, CandidateStateV1,
    GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1, GENERAL_CONFIG_SCHEMA_ID_V1, GENERAL_FUNDING_BYTES,
    GENERAL_ROOT_BYTES, GeneralAccountFrameV1, GeneralAccountMetaV1,
    GeneralActivationCapitalizationV1, GeneralBatchPdaSeedsV1, GeneralBatchReplayV1,
    GeneralCandidatePagePdaSeedsV1, GeneralCandidatePdaSeedsV1, GeneralConfigV1,
    GeneralFundingPdaSeedsV1, GeneralFundingV1, GeneralInstructionTagV1, GeneralInstructionV1,
    GeneralOrderCustodyPdaSeedsV1, GeneralOrderCustodyV1, GeneralOrderStatePdaSeedsV1,
    GeneralQuoteEscrowPdaSeedsV1, GeneralRootPdaSeedsV1, GeneralRootV1,
    GeneralSettlementCursorPdaSeedsV1, GeneralSettlementEscrowPdaSeedsV1,
    MAX_EXECUTIONS_PER_PAGE_V1, ORDER_STATE_BYTES, OrderStateV1, PortfolioOrderV1,
    SettlementCloseObservationV1, SettlementCursorV1, SettlementMaterializationActionV1,
    SettlementRentObservationV1, activate_general_v1, open_general_batch_v1,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_contract::claim::CategoricalUnitV1;
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
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
    records::{
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
        REALM_SCHEMA_RELEASE_ID_V1, with_authenticated_finalized_record_v1,
    },
};

const INSTRUCTION_WIDTH_OFFSET: usize = 11;

/// Decode the exact General width and execute one complete route.
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

#[inline(never)]
fn dispatch_width<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let tag = GeneralInstructionV1::<N>::decode_tag(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    validate_contract_frame(tag, accounts)?;
    match tag {
        GeneralInstructionTagV1::Activate => process_activate::<N>(
            program_id,
            accounts,
            GeneralInstructionV1::<N>::decode_activate(instruction_data)
                .map_err(|_| AdapterError::InvalidInstruction)?,
        ),
        GeneralInstructionTagV1::OpenBatch | GeneralInstructionTagV1::LockBatch => {
            let replay = GeneralInstructionV1::<N>::decode_batch_replay(instruction_data, tag)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            if tag == GeneralInstructionTagV1::OpenBatch {
                process_open_batch::<N>(
                    program_id,
                    accounts,
                    replay.generation,
                    replay.batch_sequence,
                )
            } else {
                process_lock_batch(
                    program_id,
                    accounts,
                    replay.generation,
                    replay.batch_sequence,
                )
            }
        }
        GeneralInstructionTagV1::AdmitOrder
        | GeneralInstructionTagV1::CancelOrder
        | GeneralInstructionTagV1::CloseOrder => {
            let order = GeneralInstructionV1::<N>::decode_order(instruction_data, tag)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            match tag {
                GeneralInstructionTagV1::AdmitOrder => {
                    process_admit_order(program_id, accounts, &order)
                }
                GeneralInstructionTagV1::CancelOrder => {
                    process_release_order(program_id, accounts, &order, true)
                }
                GeneralInstructionTagV1::CloseOrder => {
                    process_release_order(program_id, accounts, &order, false)
                }
                _ => Err(AdapterError::InvalidInstruction.into()),
            }
        }
        GeneralInstructionTagV1::SubmitCandidate => {
            let instruction =
                GeneralInstructionV1::<N>::decode_candidate_submission(instruction_data)
                    .map_err(|_| AdapterError::InvalidInstruction)?;
            process_submit_candidate(program_id, accounts, &instruction)
        }
        GeneralInstructionTagV1::CreateCandidatePage => {
            let instruction = decode_candidate_page_creation_boxed::<N>(instruction_data)?;
            process_create_candidate_page(program_id, accounts, instruction.as_ref())
        }
        GeneralInstructionTagV1::VerifyCandidatePage
        | GeneralInstructionTagV1::CollectSettlementPage
        | GeneralInstructionTagV1::DistributeSettlementPage
        | GeneralInstructionTagV1::CloseCandidatePage => {
            let reference =
                GeneralInstructionV1::<N>::decode_candidate_page_reference(instruction_data, tag)
                    .map_err(|_| AdapterError::InvalidInstruction)?;
            match tag {
                GeneralInstructionTagV1::VerifyCandidatePage => {
                    process_verify_candidate_page::<N>(program_id, accounts, reference)
                }
                GeneralInstructionTagV1::CollectSettlementPage => {
                    process_collect_settlement_page::<N>(program_id, accounts, reference)
                }
                GeneralInstructionTagV1::DistributeSettlementPage => {
                    process_distribute_settlement_page::<N>(program_id, accounts, reference)
                }
                GeneralInstructionTagV1::CloseCandidatePage => {
                    process_close_candidate_page::<N>(program_id, accounts, reference)
                }
                _ => Err(AdapterError::InvalidInstruction.into()),
            }
        }
        GeneralInstructionTagV1::FinishCandidate
        | GeneralInstructionTagV1::ConsiderCandidate
        | GeneralInstructionTagV1::BeginSettlement
        | GeneralInstructionTagV1::MaterializeSettlement
        | GeneralInstructionTagV1::FinishSettlement
        | GeneralInstructionTagV1::CloseCandidate
        | GeneralInstructionTagV1::CloseSettlement
        | GeneralInstructionTagV1::RejectCandidate
        | GeneralInstructionTagV1::ExpireSettlement => {
            let candidate_id =
                GeneralInstructionV1::<N>::decode_candidate_id(instruction_data, tag)
                    .map_err(|_| AdapterError::InvalidInstruction)?;
            match tag {
                GeneralInstructionTagV1::FinishCandidate => {
                    process_finish_candidate::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::ConsiderCandidate => {
                    process_consider_candidate::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::BeginSettlement => {
                    process_begin_settlement::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::MaterializeSettlement => {
                    process_materialize_settlement::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::FinishSettlement => {
                    process_finish_settlement::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::CloseCandidate => {
                    process_close_candidate::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::CloseSettlement => {
                    process_close_settlement::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::RejectCandidate => {
                    process_reject_candidate::<N>(program_id, accounts, candidate_id)
                }
                GeneralInstructionTagV1::ExpireSettlement => {
                    process_expire_settlement::<N>(program_id, accounts, candidate_id)
                }
                _ => Err(AdapterError::InvalidInstruction.into()),
            }
        }
        GeneralInstructionTagV1::LockSelection | GeneralInstructionTagV1::CloseBatch => {
            let replay = GeneralInstructionV1::<N>::decode_batch_replay(instruction_data, tag)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            if tag == GeneralInstructionTagV1::LockSelection {
                process_lock_selection(program_id, accounts, replay)
            } else {
                process_close_batch(program_id, accounts, replay)
            }
        }
        GeneralInstructionTagV1::Quiesce | GeneralInstructionTagV1::CloseGeneral => {
            let generation = GeneralInstructionV1::<N>::decode_generation(instruction_data, tag)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            if tag == GeneralInstructionTagV1::Quiesce {
                process_quiesce(program_id, accounts, generation)
            } else {
                process_close_general::<N>(program_id, accounts, generation)
            }
        }
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
    let execution_count = match tag {
        GeneralInstructionTagV1::VerifyCandidatePage => accounts
            .len()
            .checked_sub(9)
            .and_then(|count| u8::try_from(count).ok())
            .ok_or(AdapterError::AccountFrameLength)?,
        GeneralInstructionTagV1::CollectSettlementPage => accounts
            .len()
            .checked_sub(18)
            .filter(|count| count % 4 == 0)
            .and_then(|count| u8::try_from(count / 4).ok())
            .ok_or(AdapterError::AccountFrameLength)?,
        GeneralInstructionTagV1::DistributeSettlementPage => accounts
            .len()
            .checked_sub(19)
            .filter(|count| count % 2 == 0)
            .and_then(|count| u8::try_from(count / 2).ok())
            .ok_or(AdapterError::AccountFrameLength)?,
        _ => 0,
    };
    GeneralAccountFrameV1::new(tag, execution_count, &metas).map_err(map_general_frame_error)?;
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

#[derive(Clone, Copy)]
struct ActivationPlan<const N: usize> {
    market_after: CategoricalMarketV1<N>,
    capability_funding_after: FundingStateV1,
    root: GeneralRootV1,
    general_funding: GeneralFundingV1,
    root_seeds: GeneralRootPdaSeedsV1,
    funding_seeds: GeneralFundingPdaSeedsV1,
    root_bump: u8,
    funding_bump: u8,
    root_rent: u64,
    funding_rent: u64,
    capability_state_rent: u64,
    creation_lamports: u64,
    general_lamports: u64,
    activator_before: u64,
    market_lamports: u64,
    capability_funding_before: u64,
    root_before: u64,
    funding_before: u64,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
}

fn process_activate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: ActivateGeneralV1,
) -> Result<(), ProgramError> {
    let activator = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let manifest_account = account(accounts, 4)?;
    let config_account = account(accounts, 5)?;
    let realm_cursor = account(accounts, 6)?;
    let claim_cursor = account(accounts, 7)?;
    let manifest_cursor = account(accounts, 8)?;
    let config_cursor = account(accounts, 9)?;
    let mint = account(accounts, 10)?;
    let token_program = account(accounts, 11)?;
    let capability_funding_account = account(accounts, 12)?;
    let root_account = account(accounts, 13)?;
    let general_funding_account = account(accounts, 14)?;
    let rent_credit_account = account(accounts, 15)?;
    let system = account(accounts, 16)?;
    let rent_sysvar = account(accounts, 17)?;
    let clock_sysvar = account(accounts, 18)?;

    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(activator)?;
    require_prefunded_vacant(root_account)?;
    require_prefunded_vacant(general_funding_account)?;
    let market =
        authenticate_market_boxed::<N>(program_id, market_account, market_account.key.to_bytes())?;
    let identity = market.root().identity();
    let _realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        identity.realm_id().to_bytes(),
    )?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        identity.claim_basis_id().to_bytes(),
    )?;
    if capability_funding_account.owner != program_id
        || capability_funding_account.executable
        || capability_funding_account.data_len() != FUNDING_STATE_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let capability_funding = decode_capability_funding_boxed(capability_funding_account)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let clock = authenticate_clock(clock_sysvar)?;
    let root_rent = rent.minimum_balance(GENERAL_ROOT_BYTES);
    let funding_rent = rent.minimum_balance(GENERAL_FUNDING_BYTES);
    let capability_state_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let capability_custody = FundingCustodyObservationV1::native_only(
        capability_funding_account.lamports(),
        capability_state_rent,
    )
    .map_err(|_| AdapterError::FundUnderfunded)?;
    let rent_credit =
        authenticate_rent_credit_key(program_id, rent_credit_account, market.root().rent_refund())?;

    let mut metas = Vec::new();
    metas
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for item in accounts {
        metas.push(GeneralAccountMetaV1 {
            key: item.key.to_bytes(),
            is_signer: item.is_signer,
            is_writable: item.is_writable,
            is_executable: item.executable,
        });
    }
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::Activate, 0, &metas)
        .map_err(map_general_frame_error)?;
    let manifest_id =
        dclutch_general_contract::ContentId::new(identity.capability_manifest_id().to_bytes())
            .map_err(|_| AdapterError::ContentIdentity)?;
    let market_identity_id =
        dclutch_general_contract::ContentId::new(hash(&identity.to_bytes()).to_bytes())
            .map_err(|_| AdapterError::ContentIdentity)?;
    let plan = Box::new(with_authenticated_finalized_record_v1(
        program_id,
        manifest_account,
        manifest_cursor,
        rent_sysvar,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        identity.capability_manifest_id().to_bytes(),
        |record| {
            let manifest = CapabilityManifestV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData)?;
            let entry = manifest
                .entry(capability_funding.entry_index())
                .map_err(|_| AdapterError::FoundingAuthentication)?;
            let config_id = dclutch_general_contract::ContentId::new(entry.config_id().to_bytes())
                .map_err(|_| AdapterError::ContentIdentity)?;
            let config = authenticate_finalized_config(
                program_id,
                config_account,
                config_cursor,
                rent_sysvar,
                entry.config_id().to_bytes(),
            )?;
            authenticate_claim_basis_config::<N>(claim, config)?;
            let root_seeds = GeneralRootPdaSeedsV1::new(
                market_account.key.to_bytes(),
                config.generation(),
                config_id,
            )
            .map_err(|_| AdapterError::FoundingAuthentication)?;
            let (expected_root, root_bump) =
                Pubkey::find_program_address(&root_seeds.seed_components(), program_id);
            let funding_seeds = GeneralFundingPdaSeedsV1::new(
                market_account.key.to_bytes(),
                config.generation(),
                config_id,
                config.capability_release_id(),
            )
            .map_err(|_| AdapterError::FoundingAuthentication)?;
            let (expected_general_funding, funding_bump) =
                Pubkey::find_program_address(&funding_seeds.seed_components(), program_id);
            if root_account.key != &expected_root
                || general_funding_account.key != &expected_general_funding
            {
                return Err(AdapterError::AccountIdentity.into());
            }
            let contract_plan = activate_general_v1(
                frame,
                instruction,
                market.root(),
                config_id,
                config,
                market_identity_id,
                manifest_id,
                manifest,
                *capability_funding,
                capability_custody,
                GeneralActivationCapitalizationV1::new(root_rent, funding_rent),
                clock.slot,
            )
            .map_err(|_| AdapterError::FoundingAuthentication)?;
            let capability_derivation = contract_plan.funding().capability_funding_derivation();
            let (expected_capability_funding, _) =
                Pubkey::find_program_address(&capability_derivation.seed_components(), program_id);
            if capability_funding_account.key != &expected_capability_funding
                || contract_plan.root_seeds() != root_seeds
                || contract_plan.funding_seeds() != funding_seeds
            {
                return Err(AdapterError::AccountIdentity.into());
            }
            let market_after = CategoricalMarketV1::new(
                contract_plan.market_root_after(),
                market.hoard_atoms(),
                *market.supply(),
                market.settlement(),
            )
            .map_err(|_| AdapterError::MarketTransition)?;
            Ok(ActivationPlan {
                market_after,
                capability_funding_after: contract_plan.funding().capability_funding_after(),
                root: contract_plan.root(),
                general_funding: contract_plan.funding().general_funding(),
                root_seeds,
                funding_seeds,
                root_bump,
                funding_bump,
                root_rent,
                funding_rent,
                capability_state_rent,
                creation_lamports: contract_plan.funding().creation_lamports(),
                general_lamports: contract_plan.funding().general_lamports(),
                activator_before: activator.lamports(),
                market_lamports: market_account.lamports(),
                capability_funding_before: capability_funding_account.lamports(),
                root_before: root_account.lamports(),
                funding_before: general_funding_account.lamports(),
                rent_credit,
                rent_credit_lamports: rent_credit_account.lamports(),
            })
        },
    )?);
    preflight_mutable(&[
        activator,
        market_account,
        capability_funding_account,
        root_account,
        general_funding_account,
        rent_credit_account,
    ])?;
    execute_activation(
        program_id,
        activator,
        market_account,
        capability_funding_account,
        root_account,
        general_funding_account,
        rent_credit_account,
        system,
        plan.as_ref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_activation<'info, const N: usize>(
    program_id: &Pubkey,
    activator: &AccountInfo<'info>,
    market_account: &AccountInfo<'info>,
    capability_funding_account: &AccountInfo<'info>,
    root_account: &AccountInfo<'info>,
    general_funding_account: &AccountInfo<'info>,
    rent_credit_account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    plan: &ActivationPlan<N>,
) -> Result<(), ProgramError> {
    let activation_credit = plan
        .root_rent
        .checked_add(plan.funding_rent)
        .and_then(|value| value.checked_add(plan.creation_lamports))
        .ok_or(AdapterError::Arithmetic)?;
    transfer_owned_lamports(capability_funding_account, activator, activation_credit)?;
    create_general_pda_account(
        activator,
        root_account,
        rent_credit_account,
        system,
        program_id,
        plan.root_rent,
        GENERAL_ROOT_BYTES,
        &plan.root_seeds.seed_components(),
        plan.root_bump,
        true,
    )?;
    create_general_pda_account(
        activator,
        general_funding_account,
        rent_credit_account,
        system,
        program_id,
        plan.funding_rent,
        GENERAL_FUNDING_BYTES,
        &plan.funding_seeds.seed_components(),
        plan.funding_bump,
        true,
    )?;
    transfer_owned_lamports(
        capability_funding_account,
        general_funding_account,
        plan.general_lamports,
    )?;
    write_market(market_account, plan.market_after)?;
    write_capability_funding(capability_funding_account, plan.capability_funding_after)?;
    write_root(root_account, plan.root)?;
    write_general_funding(general_funding_account, plan.general_funding)?;

    let expected_activator = plan
        .activator_before
        .checked_add(plan.creation_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    let total_capability_debit = activation_credit
        .checked_add(plan.general_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    if activator.lamports() != expected_activator
        || market_account.lamports() != plan.market_lamports
        || capability_funding_account.lamports()
            != plan
                .capability_funding_before
                .checked_sub(total_capability_debit)
                .ok_or(AdapterError::Arithmetic)?
        || capability_funding_account.lamports() != plan.capability_state_rent
        || root_account.owner != program_id
        || root_account.lamports() != plan.root_rent
        || general_funding_account.owner != program_id
        || general_funding_account.lamports()
            != plan
                .funding_rent
                .checked_add(plan.general_lamports)
                .ok_or(AdapterError::Arithmetic)?
        || rent_credit_account.lamports()
            != plan
                .rent_credit_lamports
                .checked_add(plan.root_before)
                .and_then(|value| value.checked_add(plan.funding_before))
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit_account, plan.rent_credit)?;
    let persisted_market =
        authenticate_market::<N>(program_id, market_account, market_account.key.to_bytes())?;
    if persisted_market != plan.market_after {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    let persisted_capability = decode_capability_funding(capability_funding_account)?;
    let persisted_root = authenticate_root(program_id, root_account)?;
    let persisted_general = decode_general_funding(general_funding_account)?;
    if persisted_capability != plan.capability_funding_after
        || persisted_root != plan.root
        || persisted_general != plan.general_funding
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OpenBatchPlan {
    root: GeneralRootV1,
    batch: BatchRootV1,
    batch_seeds: GeneralBatchPdaSeedsV1,
    batch_bump: u8,
    batch_lamports: u64,
    actor_top_up: u64,
    rent_credit_surplus: u64,
    actor_before: u64,
    market_lamports: u64,
    config_lamports: u64,
    root_lamports: u64,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
}

fn process_open_batch<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    sequence: u64,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let config_account = account(accounts, 2)?;
    let config_cursor = account(accounts, 3)?;
    let root_account = account(accounts, 4)?;
    let batch_account = account(accounts, 5)?;
    let rent_credit_account = account(accounts, 6)?;
    let system = account(accounts, 7)?;
    let rent_sysvar = account(accounts, 8)?;
    let clock_sysvar = account(accounts, 9)?;

    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(actor)?;
    require_prefunded_vacant(batch_account)?;
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, config_id)?;
    let rent_credit =
        authenticate_rent_credit_key(program_id, rent_credit_account, root.rent_beneficiary())?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let clock = authenticate_clock(clock_sysvar)?;
    let batch_rent = rent.minimum_balance(BATCH_ROOT_BYTES);
    let batch_seeds = GeneralBatchPdaSeedsV1::new(root_account.key.to_bytes(), sequence)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_batch, batch_bump) =
        Pubkey::find_program_address(&batch_seeds.seed_components(), program_id);
    if batch_account.key != &expected_batch {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mut metas = Vec::new();
    metas
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for item in accounts {
        metas.push(GeneralAccountMetaV1 {
            key: item.key.to_bytes(),
            is_signer: item.is_signer,
            is_writable: item.is_writable,
            is_executable: item.executable,
        });
    }
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &metas)
        .map_err(map_general_frame_error)?;
    let contract_plan = open_general_batch_v1(
        frame,
        GeneralBatchReplayV1 {
            generation,
            batch_sequence: sequence,
        },
        config_id,
        config,
        root,
        BatchRentObservationV1 {
            exact_batch_rent_lamports: batch_rent,
            precreation_lamports: batch_account.lamports(),
        },
        clock.slot,
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    if contract_plan.batch_seeds() != batch_seeds {
        return Err(AdapterError::AccountIdentity.into());
    }
    let plan = OpenBatchPlan {
        root: contract_plan.root_after(),
        batch: contract_plan.batch(),
        batch_seeds,
        batch_bump,
        batch_lamports: contract_plan.batch_account_lamports(),
        actor_top_up: contract_plan.payer_top_up_lamports(),
        rent_credit_surplus: contract_plan.rent_credit_surplus_lamports(),
        actor_before: actor.lamports(),
        market_lamports: market_account.lamports(),
        config_lamports: config_account.lamports(),
        root_lamports: root_account.lamports(),
        rent_credit,
        rent_credit_lamports: rent_credit_account.lamports(),
    };
    preflight_mutable(&[actor, root_account, batch_account, rent_credit_account])?;
    create_general_pda_account(
        actor,
        batch_account,
        rent_credit_account,
        system,
        program_id,
        plan.batch_lamports,
        BATCH_ROOT_BYTES,
        &plan.batch_seeds.seed_components(),
        plan.batch_bump,
        false,
    )?;
    write_root(root_account, plan.root)?;
    write_batch(batch_account, plan.batch)?;
    if actor.lamports()
        != plan
            .actor_before
            .checked_sub(plan.actor_top_up)
            .ok_or(AdapterError::Arithmetic)?
        || market_account.lamports() != plan.market_lamports
        || config_account.lamports() != plan.config_lamports
        || root_account.lamports() != plan.root_lamports
        || batch_account.owner != program_id
        || batch_account.lamports() != plan.batch_lamports
        || rent_credit_account.lamports()
            != plan
                .rent_credit_lamports
                .checked_add(plan.rent_credit_surplus)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit_account, plan.rent_credit)?;
    if authenticate_root(program_id, root_account)? != plan.root
        || authenticate_batch(program_id, batch_account, root_account, sequence, config_id)?
            != plan.batch
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn process_lock_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    sequence: u64,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let clock_sysvar = account(accounts, 6)?;
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let mut batch =
        authenticate_batch(program_id, batch_account, root_account, sequence, config_id)?;
    if generation != config.generation()
        || generation != root.generation()
        || root.config_id() != config_id
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let clock = authenticate_clock(clock_sysvar)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let reward = batch
        .open_selection(
            config,
            BatchCapitalizationV1 {
                account_lamports: batch_account.lamports(),
                exact_state_rent_lamports: rent.minimum_balance(BATCH_ROOT_BYTES),
            },
            clock.slot,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[actor, batch_account])?;
    transfer_owned_lamports(batch_account, actor, reward)?;
    write_batch(batch_account, batch)?;
    if decode_batch(batch_account)? != batch {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn process_submit_candidate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: &dclutch_general_contract::SubmitGeneralCandidateV1<N>,
) -> Result<(), ProgramError> {
    let submitter = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_credit = account(accounts, 6)?;
    let system = account(accounts, 7)?;
    let rent_sysvar = account(accounts, 8)?;
    let clock_sysvar = account(accounts, 9)?;
    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(submitter)?;
    require_prefunded_vacant(candidate_account)?;
    if instruction.submission.submitter.to_bytes() != submitter.key.to_bytes() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        instruction.submission.batch_sequence,
        config_id,
    )?;
    batch
        .validate_against(config)
        .map_err(|_| AdapterError::FundUnderfunded)?;
    let mut submission_bytes = Vec::new();
    submission_bytes
        .try_reserve_exact(
            dclutch_general_contract::CandidateSubmissionV1::<N>::encoded_len()
                .map_err(|_| AdapterError::Arithmetic)?,
        )
        .map_err(|_| AdapterError::Arithmetic)?;
    submission_bytes.resize(
        dclutch_general_contract::CandidateSubmissionV1::<N>::encoded_len()
            .map_err(|_| AdapterError::Arithmetic)?,
        0,
    );
    instruction
        .submission
        .encode(&mut submission_bytes)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    if hash(&submission_bytes).to_bytes() != instruction.candidate_id.to_bytes() {
        return Err(AdapterError::ContentIdentity.into());
    }
    let seeds =
        GeneralCandidatePdaSeedsV1::new(batch_account.key.to_bytes(), instruction.candidate_id)
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_candidate, bump) =
        Pubkey::find_program_address(&seeds.seed_components(), program_id);
    if candidate_account.key != &expected_candidate {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent_credit_state = authenticate_rent_credit(program_id, rent_credit, submitter.key)?;
    let clock = authenticate_clock(clock_sysvar)?;
    let candidate = CandidateStateV1::submit(
        instruction.candidate_id,
        instruction.submission,
        root,
        config,
        &mut batch,
        clock.slot,
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    let state_bytes = CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let exact_rent = rent.minimum_balance(state_bytes);
    let account_lamports = exact_rent
        .checked_add(instruction.submission.page_rent_reserve_lamports)
        .and_then(|value| {
            value.checked_add(instruction.submission.settlement_rent_reserve_lamports)
        })
        .and_then(|value| value.checked_add(candidate.verification_work_remaining()))
        .and_then(|value| value.checked_add(candidate.settlement_work_remaining()))
        .and_then(|value| value.checked_add(candidate.cleanup_work_remaining()))
        .ok_or(AdapterError::Arithmetic)?;
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports,
            exact_state_rent_lamports: exact_rent,
        })
        .map_err(|_| AdapterError::FundUnderfunded)?;
    preflight_mutable(&[submitter, batch_account, candidate_account, rent_credit])?;
    create_general_pda_account(
        submitter,
        candidate_account,
        rent_credit,
        system,
        program_id,
        account_lamports,
        state_bytes,
        &seeds.seed_components(),
        bump,
        false,
    )?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, candidate)?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)?;
    authenticate_candidate::<N>(
        program_id,
        candidate_account,
        batch_account,
        instruction.candidate_id,
    )?
    .validate_capitalization(CandidateCapitalizationV1 {
        account_lamports: candidate_account.lamports(),
        exact_state_rent_lamports: exact_rent,
    })
    .map_err(|_| AdapterError::PositionPostcondition.into())
}

fn process_create_candidate_page<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: &dclutch_general_contract::CreateGeneralCandidatePageV1<N>,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let page_account = account(accounts, 6)?;
    let rent_credit = account(accounts, 7)?;
    let system = account(accounts, 8)?;
    let rent_sysvar = account(accounts, 9)?;
    if system.key != &system_program::ID || system.owner != &native_loader::ID || !system.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    require_system_wallet(actor)?;
    require_prefunded_vacant(page_account)?;
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        decode_candidate::<N>(candidate_account)?.batch_sequence(),
        config_id,
    )?;
    let mut candidate = authenticate_candidate(
        program_id,
        candidate_account,
        batch_account,
        instruction.candidate_id,
    )?;
    if batch.sequence() != candidate.batch_sequence() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let page_bytes = canonical_page_bytes(&instruction.page)?;
    if hashv(&[GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1, &page_bytes]).to_bytes()
        != instruction.page_id.to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let page_seeds =
        GeneralCandidatePagePdaSeedsV1::new(candidate_account.key.to_bytes(), instruction.page_id)
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_page, page_bump) =
        Pubkey::find_program_address(&page_seeds.seed_components(), program_id);
    if page_account.key != &expected_page {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let page_rent = rent.minimum_balance(page_bytes.len());
    let candidate_before = candidate_account.lamports();
    let actor_before = actor.lamports();
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;
    let plan = candidate
        .create_page(
            instruction.page,
            config,
            page_rent,
            page_account.lamports(),
            CandidateCapitalizationV1 {
                account_lamports: candidate_before,
                exact_state_rent_lamports: candidate_rent,
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[actor, candidate_account, page_account, rent_credit])?;
    transfer_owned_lamports(candidate_account, actor, plan.page_top_up_lamports())?;
    transfer_owned_lamports(
        candidate_account,
        rent_credit,
        plan.candidate_refund_lamports(),
    )?;
    create_general_pda_account(
        actor,
        page_account,
        rent_credit,
        system,
        program_id,
        page_rent,
        page_bytes.len(),
        &page_seeds.seed_components(),
        page_bump,
        false,
    )?;
    write_candidate(candidate_account, candidate)?;
    write_candidate_page(page_account, instruction.page)?;
    if actor.lamports() != actor_before
        || candidate_account.lamports()
            != candidate_before
                .checked_sub(page_rent)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)?;
    authenticate_candidate_page::<N>(
        program_id,
        page_account,
        candidate_account,
        instruction.page_id,
    )?;
    Ok(())
}

fn process_verify_candidate_page<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    reference: dclutch_general_contract::GeneralCandidatePageV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_cursor = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let page_account = account(accounts, 6)?;
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let page = authenticate_candidate_page_boxed::<N>(
        program_id,
        page_account,
        candidate_account,
        reference.page_id,
    )?;
    if usize::from(page.execution_count)
        != accounts
            .len()
            .checked_sub(9)
            .ok_or(AdapterError::AccountFrameLength)?
    {
        return Err(AdapterError::AccountFrameLength.into());
    }
    let rent_sysvar = account(accounts, 7 + usize::from(page.execution_count))?;
    let clock_sysvar = account(accounts, 8 + usize::from(page.execution_count))?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed(
        program_id,
        candidate_account,
        batch_account,
        reference.candidate_id,
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        config_id,
    )?;
    for (index, execution) in page
        .executions
        .iter()
        .take(usize::from(page.execution_count))
        .flatten()
        .enumerate()
    {
        authenticate_order_id(execution.order)?;
        let state_account = account(accounts, 7 + index)?;
        let state_seeds = GeneralOrderStatePdaSeedsV1::new(root.market(), execution.order)
            .map_err(|_| AdapterError::PositionAuthentication)?;
        let (expected_state, _) =
            Pubkey::find_program_address(&state_seeds.seed_components(), program_id);
        if state_account.key != &expected_state
            || decode_order_state(state_account)? != execution.order_state
        {
            return Err(AdapterError::ReplayMismatch.into());
        }
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let before = candidate_account.lamports();
    let reward = candidate
        .verify_page(
            reference.page_id,
            page.as_ref(),
            &root,
            &config,
            &batch,
            authenticate_clock(clock_sysvar)?.slot,
            CandidateCapitalizationV1 {
                account_lamports: before,
                exact_state_rent_lamports: rent.minimum_balance(
                    CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
                ),
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[actor, candidate_account])?;
    transfer_owned_lamports(candidate_account, actor, reward)?;
    write_candidate(candidate_account, *candidate)
}

fn process_finish_candidate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_cursor = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let clock_sysvar = account(accounts, 7)?;
    let (root, config, batch, mut candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        candidate_id,
    )?;
    let reward = candidate
        .finish_verification(config, batch, cap, authenticate_clock(clock_sysvar)?.slot)
        .map_err(|_| AdapterError::MarketTransition)?;
    let _ = root;
    pay_candidate_reward(actor, candidate_account, *candidate, reward)
}

fn process_consider_candidate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let config_account = account(accounts, 2)?;
    let config_cursor = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let clock_sysvar = account(accounts, 7)?;
    let (_, config, mut batch, mut candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        candidate_id,
    )?;
    let reward = batch
        .consider_candidate(
            candidate.as_mut(),
            config,
            cap,
            authenticate_clock(clock_sysvar)?.slot,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[actor, batch_account, candidate_account])?;
    transfer_owned_lamports(candidate_account, actor, reward)?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, *candidate)
}

fn process_lock_selection(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    replay: GeneralBatchReplayV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let clock_sysvar = account(accounts, 6)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    if replay.generation != root.generation() || replay.generation != config.generation() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        replay.batch_sequence,
        root.config_id(),
    )?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let reward = batch
        .close_selection(
            config,
            BatchCapitalizationV1 {
                account_lamports: batch_account.lamports(),
                exact_state_rent_lamports: rent.minimum_balance(BATCH_ROOT_BYTES),
            },
            authenticate_clock(clock_sysvar)?.slot,
        )
        .map_err(|_| AdapterError::MarketTransition)?
        .1;
    preflight_mutable(&[actor, batch_account])?;
    transfer_owned_lamports(batch_account, actor, reward)?;
    write_batch(batch_account, batch)
}

fn process_reject_candidate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_cursor = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let clock_sysvar = account(accounts, 7)?;
    let (_, config, _, mut candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        candidate_id,
    )?;
    let reward = candidate
        .reject(config, cap, authenticate_clock(clock_sysvar)?.slot)
        .map_err(|_| AdapterError::MarketTransition)?;
    pay_candidate_reward(actor, candidate_account, *candidate, reward)
}

fn process_expire_settlement<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_cursor = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let clock_sysvar = account(accounts, 7)?;
    let (_, config, mut batch, mut candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        candidate_id,
    )?;
    let reward = batch
        .expire_unsettled(
            candidate.as_mut(),
            config,
            cap,
            authenticate_clock(clock_sysvar)?.slot,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[actor, batch_account, candidate_account])?;
    transfer_owned_lamports(candidate_account, actor, reward)?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, *candidate)
}

fn process_close_candidate_page<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    reference: dclutch_general_contract::GeneralCandidatePageV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let page_account = account(accounts, 6)?;
    let rent_credit = account(accounts, 7)?;
    let rent_sysvar = account(accounts, 8)?;
    let (_, config, batch, mut candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        reference.candidate_id,
    )?;
    authenticate_candidate_page::<N>(
        program_id,
        page_account,
        candidate_account,
        reference.page_id,
    )?;
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;
    let close = candidate
        .close_page(batch, config, cap, page_account.lamports())
        .map_err(|_| AdapterError::MarketTransition)?;
    if close.rent_credit_lamports != page_account.lamports()
        || close.rent_beneficiary.to_bytes() != candidate.submitter().to_bytes()
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    preflight_mutable(&[actor, candidate_account, page_account, rent_credit])?;
    transfer_owned_lamports(candidate_account, actor, close.cleanup_reward_lamports)?;
    write_candidate(candidate_account, *candidate)?;
    close_program_account(page_account, rent_credit)?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

fn process_close_candidate<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let root_account = account(accounts, 1)?;
    let batch_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let config_cursor = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let rent_credit = account(accounts, 6)?;
    let rent_sysvar = account(accounts, 7)?;
    let (_, config, mut batch, candidate, cap) = authenticate_candidate_transition::<N>(
        program_id,
        root_account,
        batch_account,
        config_account,
        config_cursor,
        candidate_account,
        rent_sysvar,
        candidate_id,
    )?;
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;
    let close = batch
        .close_candidate_child(*candidate, config, cap)
        .map_err(|_| AdapterError::MarketTransition)?;
    if close.rent_credit_lamports
        != candidate_account
            .lamports()
            .checked_sub(close.cleanup_reward_lamports)
            .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    preflight_mutable(&[actor, batch_account, candidate_account, rent_credit])?;
    transfer_owned_lamports(candidate_account, actor, close.cleanup_reward_lamports)?;
    write_batch(batch_account, batch)?;
    close_program_account(candidate_account, rent_credit)?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

fn process_close_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    replay: GeneralBatchReplayV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let batch_account = account(accounts, 4)?;
    let rent_credit = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let mut root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    if replay.generation != root.generation() || replay.generation != config.generation() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        replay.batch_sequence,
        root.config_id(),
    )?;
    let rent_credit_state =
        authenticate_rent_credit_key(program_id, rent_credit, root.rent_beneficiary())?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let close = batch
        .retire(
            &mut root,
            config,
            BatchCapitalizationV1 {
                account_lamports: batch_account.lamports(),
                exact_state_rent_lamports: rent.minimum_balance(BATCH_ROOT_BYTES),
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    if close.rent_beneficiary != rent_credit.key.to_bytes()
        || close.rent_credit_lamports
            != batch_account
                .lamports()
                .checked_sub(close.continuation_reward_lamports)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    preflight_mutable(&[actor, root_account, batch_account, rent_credit])?;
    transfer_owned_lamports(batch_account, actor, close.continuation_reward_lamports)?;
    write_root(root_account, root)?;
    close_program_account(batch_account, rent_credit)?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

fn process_begin_settlement<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let candidate_account = account(accounts, 12)?;
    let cursor_account = account(accounts, 13)?;
    let settlement_position_account = account(accounts, 14)?;
    let settlement_quote_escrow = account(accounts, 15)?;
    let rent_credit = account(accounts, 16)?;
    let system = account(accounts, 17)?;
    let rent_sysvar = account(accounts, 18)?;
    let clock_sysvar = account(accounts, 19)?;

    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(actor)?;
    require_prefunded_vacant(cursor_account)?;
    require_prefunded_vacant(settlement_position_account)?;
    require_prefunded_vacant(settlement_quote_escrow)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config_boxed(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market_boxed::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(*market, *config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, *config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        candidate_id,
    )?;
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;

    let cursor_seeds = GeneralSettlementCursorPdaSeedsV1::new(candidate_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_cursor, cursor_bump) =
        Pubkey::find_program_address(&cursor_seeds.seed_components(), program_id);
    if cursor_account.key != &expected_cursor {
        return Err(AdapterError::AccountIdentity.into());
    }
    let position_components = [
        POSITION_PDA_DOMAIN,
        market_account.key.as_ref(),
        cursor_account.key.as_ref(),
    ];
    let (expected_position, position_bump) =
        Pubkey::find_program_address(&position_components, program_id);
    if settlement_position_account.key != &expected_position {
        return Err(AdapterError::AccountIdentity.into());
    }
    let escrow_seeds = GeneralSettlementEscrowPdaSeedsV1::new(cursor_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_escrow, escrow_bump) =
        Pubkey::find_program_address(&escrow_seeds.seed_components(), program_id);
    if settlement_quote_escrow.key != &expected_escrow {
        return Err(AdapterError::AccountIdentity.into());
    }

    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let cursor_bytes =
        SettlementCursorV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let position_bytes = PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let exact_rents = [
        rent.minimum_balance(cursor_bytes),
        rent.minimum_balance(position_bytes),
        rent.minimum_balance(ACCOUNT_BYTES),
    ];
    let precreation = [
        cursor_account.lamports(),
        settlement_position_account.lamports(),
        settlement_quote_escrow.lamports(),
    ];
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let candidate_before = candidate_account.lamports();
    let actor_before = actor.lamports();
    let rent_credit_before = rent_credit.lamports();
    let begin = begin_settlement_boxed(
        candidate.as_mut(),
        &mut batch,
        root,
        *config,
        CandidateCapitalizationV1 {
            account_lamports: candidate_before,
            exact_state_rent_lamports: candidate_rent,
        },
        SettlementRentObservationV1 {
            exact_rent_lamports: exact_rents,
            precreation_lamports: precreation,
        },
        authenticate_clock(clock_sysvar)?.slot,
    )?;
    let top_ups = begin.temporary_top_up_lamports();
    let actor_reimbursement = top_ups
        .iter()
        .try_fold(begin.reward_lamports(), |total, amount| {
            total.checked_add(*amount)
        })
        .ok_or(AdapterError::Arithmetic)?;
    let settlement_position = empty_position_boxed::<N>(
        market_account.key.to_bytes(),
        cursor_account.key.to_bytes(),
        config.generation(),
    )?;
    preflight_mutable(&[
        actor,
        batch_account,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
        rent_credit,
    ])?;
    transfer_owned_lamports(candidate_account, actor, actor_reimbursement)?;
    transfer_owned_lamports(
        candidate_account,
        rent_credit,
        begin.candidate_refund_lamports(),
    )?;
    create_general_pda_account(
        actor,
        cursor_account,
        rent_credit,
        system,
        program_id,
        exact_rents[0],
        cursor_bytes,
        &cursor_seeds.seed_components(),
        cursor_bump,
        false,
    )?;
    create_general_pda_account(
        actor,
        settlement_position_account,
        rent_credit,
        system,
        program_id,
        exact_rents[1],
        position_bytes,
        &position_components,
        position_bump,
        false,
    )?;
    create_general_pda_account(
        actor,
        settlement_quote_escrow,
        rent_credit,
        system,
        token_program.key,
        exact_rents[2],
        ACCOUNT_BYTES,
        &escrow_seeds.seed_components(),
        escrow_bump,
        false,
    )?;
    let initialize = token_initialize_instruction(
        realm.release,
        *settlement_quote_escrow.key,
        *mint.key,
        *cursor_account.key,
    )?;
    invoke(
        &initialize,
        &[
            settlement_quote_escrow.clone(),
            mint.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| AdapterError::VaultInitializeCpi)?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, *candidate)?;
    write_settlement_cursor(cursor_account, begin.cursor())?;
    write_position(settlement_position_account, *settlement_position)?;

    let surplus = begin
        .temporary_surplus_refund_lamports()
        .iter()
        .try_fold(begin.candidate_refund_lamports(), |total, amount| {
            total.checked_add(*amount)
        })
        .ok_or(AdapterError::Arithmetic)?;
    if actor.lamports()
        != actor_before
            .checked_add(begin.reward_lamports())
            .ok_or(AdapterError::Arithmetic)?
        || rent_credit.lamports()
            != rent_credit_before
                .checked_add(surplus)
                .ok_or(AdapterError::Arithmetic)?
        || cursor_account.lamports() != exact_rents[0]
        || settlement_position_account.lamports() != exact_rents[1]
        || settlement_quote_escrow.lamports() != exact_rents[2]
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition)?;
    authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )
    .and_then(|state| {
        if state.amount == 0 {
            Ok(())
        } else {
            Err(AdapterError::PositionPostcondition.into())
        }
    })?;
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

fn process_collect_settlement_page<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    reference: dclutch_general_contract::GeneralCandidatePageV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let candidate_account = account(accounts, 12)?;
    let cursor_account = account(accounts, 13)?;
    let settlement_position_account = account(accounts, 14)?;
    let settlement_quote_escrow = account(accounts, 15)?;
    let page_account = account(accounts, 16)?;
    let page = authenticate_candidate_page_boxed::<N>(
        program_id,
        page_account,
        candidate_account,
        reference.page_id,
    )?;
    let execution_count = usize::from(page.execution_count);
    if execution_count == 0
        || execution_count > MAX_EXECUTIONS_PER_PAGE_V1
        || accounts.len() != 18usize.saturating_add(execution_count.saturating_mul(4))
    {
        return Err(AdapterError::AccountFrameLength.into());
    }
    let rent_sysvar = account(accounts, 17 + execution_count * 4)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        reference.candidate_id,
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let cursor_before =
        authenticate_settlement_cursor_boxed::<N>(program_id, cursor_account, candidate_account)?;
    let mut cursor = cursor_before.clone();
    let mut settlement_position = authenticate_position_boxed::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )?;
    let settlement_quote_before = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;

    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let result = cursor
        .collect_page(
            reference.page_id,
            &page,
            candidate.as_mut(),
            &root,
            &config,
            &batch,
            *settlement_position.balances(),
            settlement_quote_before.amount,
            CandidateCapitalizationV1 {
                account_lamports: candidate_account.lamports(),
                exact_state_rent_lamports: candidate_rent,
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    if usize::from(result.execution_count) != execution_count || result.page_close.is_some() {
        return Err(AdapterError::PositionPostcondition.into());
    }

    let mut mutable = Vec::new();
    mutable
        .try_reserve_exact(4 + execution_count * 4)
        .map_err(|_| AdapterError::Arithmetic)?;
    mutable.extend_from_slice(&[
        actor,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
    ]);
    for index in 0..execution_count {
        let base = 17 + index * 4;
        mutable.extend_from_slice(&[
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 2)?,
            account(accounts, base + 3)?,
        ]);
    }
    preflight_mutable(&mutable)?;
    for index in 0..execution_count {
        let base = 17 + index * 4;
        collect_settlement_execution(
            program_id,
            market_account,
            mint,
            token_program,
            account(accounts, base)?,
            account(accounts, base + 1)?,
            account(accounts, base + 2)?,
            account(accounts, base + 3)?,
            settlement_quote_escrow,
            realm,
            reference.page_id,
            page.as_ref(),
            candidate.as_ref(),
            &root,
            &config,
            &batch,
            cursor_before.as_ref(),
            index,
            settlement_position.as_mut(),
        )?;
    }
    if settlement_position.balances() != &result.claim_inventory_after {
        return Err(AdapterError::PositionPostcondition.into());
    }
    transfer_owned_lamports(candidate_account, actor, result.settlement_reward_lamports)?;
    write_candidate(candidate_account, *candidate)?;
    write_settlement_cursor(cursor_account, *cursor)?;
    write_position(settlement_position_account, *settlement_position)?;
    let settlement_quote_after = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    if settlement_quote_after.amount != result.quote_inventory_after {
        return Err(AdapterError::PositionPostcondition.into());
    }
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn collect_settlement_execution<'info, const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    state_account: &AccountInfo<'info>,
    custody_account: &AccountInfo<'info>,
    owner_position_account: &AccountInfo<'info>,
    quote_escrow: &AccountInfo<'info>,
    settlement_quote_escrow: &AccountInfo<'info>,
    realm: RealmFacts,
    page_id: dclutch_general_contract::ContentId,
    page: &CandidatePageV1<N>,
    candidate: &CandidateStateV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
    cursor_before: &SettlementCursorV1<N>,
    index: usize,
    settlement_position: &mut PositionV1<N>,
) -> Result<(), ProgramError> {
    let execution = page.executions[index].ok_or(AdapterError::ReplayMismatch)?;
    authenticate_order_id(execution.order)?;
    let state_seeds =
        GeneralOrderStatePdaSeedsV1::new(market_account.key.to_bytes(), execution.order)
            .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_state, _) =
        Pubkey::find_program_address(&state_seeds.seed_components(), program_id);
    let mut state = decode_order_state(state_account)?;
    if state_account.key != &expected_state || state != execution.order_state {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let custody_seeds = GeneralOrderCustodyPdaSeedsV1::new(state_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let (expected_custody, bump) =
        Pubkey::find_program_address(&custody_seeds.seed_components(), program_id);
    let mut custody = decode_custody::<N>(custody_account)?;
    custody
        .authenticate(execution.order, *root, *config)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if custody_account.key != &expected_custody
        || custody.quote_escrow() != quote_escrow.key.to_bytes()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let owner = Pubkey::new_from_array(execution.order.owner().to_bytes());
    authenticate_position::<N>(
        program_id,
        owner_position_account,
        market_account,
        &owner,
        config.generation(),
    )?;
    let quote_before = authenticate_order_quote_escrow(
        program_id,
        quote_escrow,
        custody_account,
        mint,
        token_program,
        realm,
    )?;
    if quote_before.amount != custody.reserved_quote_atoms() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let execution_plan = cursor_before
        .execution_plan(page_id, page, candidate, root, config, batch, index)
        .map_err(|_| AdapterError::MarketTransition)?;
    let effect = custody
        .apply_receipt(
            &mut state,
            execution.order,
            execution_plan.receipt,
            *root,
            *config,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    if state != execution_plan.order_state || effect != execution_plan.custody_effect {
        return Err(AdapterError::PositionPostcondition.into());
    }
    for (outcome, amount) in effect.claim_debits_from_custody().iter().enumerate() {
        if *amount != 0 {
            settlement_position
                .credit_outcome(outcome, *amount)
                .map_err(|_| AdapterError::PositionAuthentication)?;
        }
    }
    let amount = effect.quote_debit_from_escrow();
    if amount != 0 {
        let components = custody_seeds.seed_components();
        let bump_seed = [bump];
        let signer = [components[0], components[1], bump_seed.as_slice()];
        let transfer = token_transfer_instruction(
            realm.release,
            *quote_escrow.key,
            *mint.key,
            *settlement_quote_escrow.key,
            *custody_account.key,
            amount,
            realm.mint.decimals,
        )?;
        invoke_signed(
            &transfer,
            &[
                quote_escrow.clone(),
                mint.clone(),
                settlement_quote_escrow.clone(),
                custody_account.clone(),
                token_program.clone(),
            ],
            &[&signer],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
    }
    write_order_state(state_account, state)?;
    write_custody(custody_account, custody)?;
    let quote_after = authenticate_order_quote_escrow(
        program_id,
        quote_escrow,
        custody_account,
        mint,
        token_program,
        realm,
    )?;
    if quote_after.amount != custody.reserved_quote_atoms() {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn process_materialize_settlement<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let vault = account(accounts, 10)?;
    let root_account = account(accounts, 11)?;
    let batch_account = account(accounts, 12)?;
    let candidate_account = account(accounts, 13)?;
    let cursor_account = account(accounts, 14)?;
    let settlement_position_account = account(accounts, 15)?;
    let settlement_quote_escrow = account(accounts, 16)?;
    let rent_sysvar = account(accounts, 17)?;

    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        candidate_id,
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let mut cursor =
        authenticate_settlement_cursor_boxed::<N>(program_id, cursor_account, candidate_account)?;
    let mut settlement_position = authenticate_position_boxed::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )?;
    let quote_before = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    let vault_before = authenticate_collateral_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let materialization = materialize_settlement_boxed(
        cursor.as_mut(),
        candidate.as_mut(),
        batch,
        root,
        config,
        *settlement_position.balances(),
        quote_before.amount,
        CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        },
    )?;
    let mut market_after = market;
    match materialization.action() {
        SettlementMaterializationActionV1::None => {}
        SettlementMaterializationActionV1::Split(quantity) => {
            market_after
                .split_complete_set(quantity)
                .map_err(|_| AdapterError::MarketTransition)?;
            settlement_position
                .credit_complete_set(quantity)
                .map_err(|_| AdapterError::MarketTransition)?;
        }
        SettlementMaterializationActionV1::Merge(quantity) => {
            market_after
                .merge_complete_set(quantity)
                .map_err(|_| AdapterError::MarketTransition)?;
            settlement_position
                .debit_complete_set(quantity)
                .map_err(|_| AdapterError::MarketTransition)?;
        }
    }
    if settlement_position.balances() != &materialization.claim_inventory_after() {
        return Err(AdapterError::PositionPostcondition.into());
    }
    preflight_mutable(&[
        actor,
        market_account,
        vault,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
    ])?;
    match materialization.action() {
        SettlementMaterializationActionV1::None => {}
        SettlementMaterializationActionV1::Split(quantity) => {
            let seeds = GeneralSettlementCursorPdaSeedsV1::new(candidate_account.key.to_bytes())
                .map_err(|_| AdapterError::PositionAuthentication)?;
            let components = seeds.seed_components();
            let (_, bump) = Pubkey::find_program_address(&components, program_id);
            let bump_seed = [bump];
            let signer = [components[0], components[1], bump_seed.as_slice()];
            let transfer = token_transfer_instruction(
                realm.release,
                *settlement_quote_escrow.key,
                *mint.key,
                *vault.key,
                *cursor_account.key,
                quantity,
                realm.mint.decimals,
            )?;
            invoke_signed(
                &transfer,
                &[
                    settlement_quote_escrow.clone(),
                    mint.clone(),
                    vault.clone(),
                    cursor_account.clone(),
                    token_program.clone(),
                ],
                &[&signer],
            )
            .map_err(|_| AdapterError::CollateralTransferCpi)?;
        }
        SettlementMaterializationActionV1::Merge(quantity) => {
            let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
            let (_, bump) = Pubkey::find_program_address(
                &[MARKET_SEED, identity_digest.as_slice()],
                program_id,
            );
            let bump_seed = [bump];
            let signer = [
                MARKET_SEED,
                identity_digest.as_slice(),
                bump_seed.as_slice(),
            ];
            let transfer = token_transfer_instruction(
                realm.release,
                *vault.key,
                *mint.key,
                *settlement_quote_escrow.key,
                *market_account.key,
                quantity,
                realm.mint.decimals,
            )?;
            invoke_signed(
                &transfer,
                &[
                    vault.clone(),
                    mint.clone(),
                    settlement_quote_escrow.clone(),
                    market_account.clone(),
                    token_program.clone(),
                ],
                &[&signer],
            )
            .map_err(|_| AdapterError::CollateralTransferCpi)?;
        }
    }
    transfer_owned_lamports(candidate_account, actor, materialization.reward_lamports())?;
    write_market(market_account, market_after)?;
    write_candidate(candidate_account, *candidate)?;
    write_settlement_cursor(cursor_account, *cursor)?;
    write_position(settlement_position_account, *settlement_position)?;
    let quote_after = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    let vault_after = authenticate_collateral_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market_after.hoard_atoms(),
    )?;
    let expected_vault = match materialization.action() {
        SettlementMaterializationActionV1::None => vault_before.amount,
        SettlementMaterializationActionV1::Split(quantity) => vault_before
            .amount
            .checked_add(quantity)
            .ok_or(AdapterError::Arithmetic)?,
        SettlementMaterializationActionV1::Merge(quantity) => vault_before
            .amount
            .checked_sub(quantity)
            .ok_or(AdapterError::Arithmetic)?,
    };
    if quote_after.amount != materialization.quote_inventory_after()
        || vault_after.amount != expected_vault
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

fn process_distribute_settlement_page<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    reference: dclutch_general_contract::GeneralCandidatePageV1,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let candidate_account = account(accounts, 12)?;
    let cursor_account = account(accounts, 13)?;
    let settlement_position_account = account(accounts, 14)?;
    let settlement_quote_escrow = account(accounts, 15)?;
    let page_account = account(accounts, 16)?;
    let rent_credit = account(accounts, 17)?;
    let page = authenticate_candidate_page_boxed::<N>(
        program_id,
        page_account,
        candidate_account,
        reference.page_id,
    )?;
    let execution_count = usize::from(page.execution_count);
    if execution_count == 0
        || execution_count > MAX_EXECUTIONS_PER_PAGE_V1
        || accounts.len() != 19usize.saturating_add(execution_count.saturating_mul(2))
    {
        return Err(AdapterError::AccountFrameLength.into());
    }
    let rent_sysvar = account(accounts, 18 + execution_count * 2)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        reference.candidate_id,
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let cursor_before =
        authenticate_settlement_cursor_boxed::<N>(program_id, cursor_account, candidate_account)?;
    let mut cursor = cursor_before.clone();
    let mut settlement_position = authenticate_position_boxed::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )?;
    let settlement_quote_before = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;
    let rent_credit_before = rent_credit.lamports();
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let result = cursor
        .distribute_page(
            reference.page_id,
            &page,
            candidate.as_mut(),
            &root,
            &config,
            &batch,
            *settlement_position.balances(),
            settlement_quote_before.amount,
            page_account.lamports(),
            CandidateCapitalizationV1 {
                account_lamports: candidate_account.lamports(),
                exact_state_rent_lamports: candidate_rent,
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    let page_close = result
        .page_close
        .ok_or(AdapterError::PositionPostcondition)?;
    if usize::from(result.execution_count) != execution_count
        || page_close.rent_credit_lamports != page_account.lamports()
        || page_close.rent_beneficiary.to_bytes() != candidate.submitter().to_bytes()
    {
        return Err(AdapterError::PositionPostcondition.into());
    }

    let mut mutable = Vec::new();
    mutable
        .try_reserve_exact(7 + execution_count * 2)
        .map_err(|_| AdapterError::Arithmetic)?;
    mutable.extend_from_slice(&[
        actor,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
        page_account,
        rent_credit,
    ]);
    for index in 0..execution_count {
        mutable.extend_from_slice(&[
            account(accounts, 18 + index * 2)?,
            account(accounts, 19 + index * 2)?,
        ]);
    }
    preflight_mutable(&mutable)?;
    for index in 0..execution_count {
        distribute_settlement_execution(
            program_id,
            market_account,
            mint,
            token_program,
            candidate_account,
            cursor_account,
            settlement_quote_escrow,
            account(accounts, 18 + index * 2)?,
            account(accounts, 19 + index * 2)?,
            realm,
            reference.page_id,
            page.as_ref(),
            candidate.as_ref(),
            &root,
            &config,
            &batch,
            cursor_before.as_ref(),
            index,
            settlement_position.as_mut(),
        )?;
    }
    if settlement_position.balances() != &result.claim_inventory_after {
        return Err(AdapterError::PositionPostcondition.into());
    }
    let total_reward = result
        .settlement_reward_lamports
        .checked_add(page_close.cleanup_reward_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    transfer_owned_lamports(candidate_account, actor, total_reward)?;
    write_candidate(candidate_account, *candidate)?;
    write_settlement_cursor(cursor_account, *cursor)?;
    write_position(settlement_position_account, *settlement_position)?;
    close_program_account(page_account, rent_credit)?;
    if rent_credit.lamports()
        != rent_credit_before
            .checked_add(page_close.rent_credit_lamports)
            .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)?;
    let settlement_quote_after = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    if settlement_quote_after.amount != result.quote_inventory_after {
        return Err(AdapterError::PositionPostcondition.into());
    }
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn distribute_settlement_execution<'info, const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    candidate_account: &AccountInfo<'info>,
    cursor_account: &AccountInfo<'info>,
    settlement_quote_escrow: &AccountInfo<'info>,
    owner_position_account: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    realm: RealmFacts,
    page_id: dclutch_general_contract::ContentId,
    page: &CandidatePageV1<N>,
    candidate: &CandidateStateV1<N>,
    root: &GeneralRootV1,
    config: &GeneralConfigV1,
    batch: &BatchRootV1,
    cursor_before: &SettlementCursorV1<N>,
    index: usize,
    settlement_position: &mut PositionV1<N>,
) -> Result<(), ProgramError> {
    let execution = page.executions[index].ok_or(AdapterError::ReplayMismatch)?;
    authenticate_order_id(execution.order)?;
    let execution_plan = cursor_before
        .execution_plan(page_id, page, candidate, root, config, batch, index)
        .map_err(|_| AdapterError::MarketTransition)?;
    let owner = Pubkey::new_from_array(execution.order.owner().to_bytes());
    let mut owner_position = authenticate_position::<N>(
        program_id,
        owner_position_account,
        market_account,
        &owner,
        config.generation(),
    )?;
    for (outcome, amount) in execution_plan
        .custody_effect
        .claim_credits_to_owner()
        .iter()
        .enumerate()
    {
        if *amount != 0 {
            settlement_position
                .debit_outcome(outcome, *amount)
                .and_then(|()| owner_position.credit_outcome(outcome, *amount))
                .map_err(|_| AdapterError::PositionAuthentication)?;
        }
    }
    let destination_before = authenticate_quote_destination(
        destination,
        mint,
        token_program,
        realm,
        execution.order.owner().to_bytes(),
    )?;
    let amount = execution_plan.custody_effect.quote_credit_to_owner();
    if amount != 0 {
        let cursor_seeds = GeneralSettlementCursorPdaSeedsV1::new(candidate_account.key.to_bytes())
            .map_err(|_| AdapterError::PositionAuthentication)?;
        let cursor_components = cursor_seeds.seed_components();
        let (_, cursor_bump) = Pubkey::find_program_address(&cursor_components, program_id);
        let cursor_bump_seed = [cursor_bump];
        let cursor_signer = [
            cursor_components[0],
            cursor_components[1],
            cursor_bump_seed.as_slice(),
        ];
        let transfer = token_transfer_instruction(
            realm.release,
            *settlement_quote_escrow.key,
            *mint.key,
            *destination.key,
            *cursor_account.key,
            amount,
            realm.mint.decimals,
        )?;
        invoke_signed(
            &transfer,
            &[
                settlement_quote_escrow.clone(),
                mint.clone(),
                destination.clone(),
                cursor_account.clone(),
                token_program.clone(),
            ],
            &[&cursor_signer],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
    }
    write_position(owner_position_account, owner_position)?;
    let destination_after = authenticate_quote_destination(
        destination,
        mint,
        token_program,
        realm,
        execution.order.owner().to_bytes(),
    )?;
    if destination_after.amount
        != destination_before
            .amount
            .checked_add(amount)
            .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn process_finish_settlement<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let candidate_account = account(accounts, 12)?;
    let cursor_account = account(accounts, 13)?;
    let settlement_position_account = account(accounts, 14)?;
    let settlement_quote_escrow = account(accounts, 15)?;
    let rent_sysvar = account(accounts, 16)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        candidate_id,
    )?;
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let mut cursor =
        authenticate_settlement_cursor_boxed::<N>(program_id, cursor_account, candidate_account)?;
    let settlement_position = authenticate_position::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )?;
    let settlement_quote = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let reward = cursor
        .finish(
            candidate.as_mut(),
            &mut batch,
            root,
            config,
            *settlement_position.balances(),
            settlement_quote.amount,
            CandidateCapitalizationV1 {
                account_lamports: candidate_account.lamports(),
                exact_state_rent_lamports: candidate_rent,
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[
        actor,
        batch_account,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
    ])?;
    transfer_owned_lamports(candidate_account, actor, reward)?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, *candidate)?;
    write_settlement_cursor(cursor_account, *cursor)?;
    if authenticate_position::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )? != settlement_position
        || authenticate_settlement_quote_escrow(
            program_id,
            settlement_quote_escrow,
            mint,
            token_program,
            realm,
            cursor_account.key,
        )? != settlement_quote
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

fn process_close_settlement<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<(), ProgramError> {
    let actor = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let candidate_account = account(accounts, 12)?;
    let cursor_account = account(accounts, 13)?;
    let settlement_position_account = account(accounts, 14)?;
    let settlement_quote_escrow = account(accounts, 15)?;
    let rent_credit = account(accounts, 16)?;
    let rent_sysvar = account(accounts, 17)?;
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let mut candidate = authenticate_candidate_boxed::<N>(
        program_id,
        candidate_account,
        batch_account,
        candidate_id,
    )?;
    let mut batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    let cursor =
        authenticate_settlement_cursor::<N>(program_id, cursor_account, candidate_account)?;
    let settlement_position = authenticate_position::<N>(
        program_id,
        settlement_position_account,
        market_account,
        cursor_account.key,
        config.generation(),
    )?;
    let settlement_quote = authenticate_settlement_quote_escrow(
        program_id,
        settlement_quote_escrow,
        mint,
        token_program,
        realm,
        cursor_account.key,
    )?;
    let rent_credit_state = authenticate_rent_credit(
        program_id,
        rent_credit,
        &Pubkey::new_from_array(candidate.submitter().to_bytes()),
    )?;
    let rent_credit_before = rent_credit.lamports();
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let candidate_rent = rent.minimum_balance(
        CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let exact_rents = [
        rent.minimum_balance(
            SettlementCursorV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
        ),
        rent.minimum_balance(PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?),
        rent.minimum_balance(ACCOUNT_BYTES),
    ];
    let close = cursor
        .close(
            candidate.as_mut(),
            &mut batch,
            root,
            config,
            *settlement_position.balances(),
            settlement_quote.amount,
            SettlementCloseObservationV1 {
                account_lamports: [
                    cursor_account.lamports(),
                    settlement_position_account.lamports(),
                    settlement_quote_escrow.lamports(),
                ],
                exact_rent_lamports: exact_rents,
            },
            CandidateCapitalizationV1 {
                account_lamports: candidate_account.lamports(),
                exact_state_rent_lamports: candidate_rent,
            },
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    if close.rent_beneficiary.to_bytes() != candidate.submitter().to_bytes()
        || close.rent_credit_lamports
            != cursor_account
                .lamports()
                .checked_add(settlement_position_account.lamports())
                .and_then(|value| value.checked_add(settlement_quote_escrow.lamports()))
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    preflight_mutable(&[
        actor,
        batch_account,
        candidate_account,
        cursor_account,
        settlement_position_account,
        settlement_quote_escrow,
        rent_credit,
    ])?;
    transfer_owned_lamports(candidate_account, actor, close.continuation_reward_lamports)?;
    write_batch(batch_account, batch)?;
    write_candidate(candidate_account, *candidate)?;
    let cursor_seeds = GeneralSettlementCursorPdaSeedsV1::new(candidate_account.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let components = cursor_seeds.seed_components();
    let (_, bump) = Pubkey::find_program_address(&components, program_id);
    let bump_seed = [bump];
    let signer = [components[0], components[1], bump_seed.as_slice()];
    let token_close = token_close_instruction(
        realm.release,
        *settlement_quote_escrow.key,
        *rent_credit.key,
        *cursor_account.key,
    )?;
    invoke_signed(
        &token_close,
        &[
            settlement_quote_escrow.clone(),
            rent_credit.clone(),
            cursor_account.clone(),
            token_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| AdapterError::PositionClose)?;
    close_program_account(settlement_position_account, rent_credit)?;
    close_program_account(cursor_account, rent_credit)?;
    if cursor_account.lamports() != 0
        || settlement_position_account.lamports() != 0
        || settlement_quote_escrow.lamports() != 0
        || rent_credit.lamports()
            != rent_credit_before
                .checked_add(close.rent_credit_lamports)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)?;
    candidate
        .validate_capitalization(CandidateCapitalizationV1 {
            account_lamports: candidate_account.lamports(),
            exact_state_rent_lamports: candidate_rent,
        })
        .map_err(|_| AdapterError::PositionPostcondition.into())
}

fn process_quiesce(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
) -> Result<(), ProgramError> {
    let root_account = account(accounts, 0)?;
    let mut root = authenticate_root(program_id, root_account)?;
    if root.generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    root.request_quiescence()
        .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[root_account])?;
    write_root(root_account, root)
}

fn process_close_general<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let config_account = account(accounts, 1)?;
    let config_cursor = account(accounts, 2)?;
    let root_account = account(accounts, 3)?;
    let funding_account = account(accounts, 4)?;
    let rent_credit = account(accounts, 5)?;
    let rent_sysvar = account(accounts, 6)?;
    let mut root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    if generation != root.generation() || generation != config.generation() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, root.config_id())?;
    let mut funding =
        authenticate_general_funding(program_id, funding_account, root, root.config_id(), config)?;
    let rent_credit_state =
        authenticate_rent_credit_key(program_id, rent_credit, root.rent_beneficiary())?;
    let rent_credit_before = rent_credit.lamports();
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let root_rent = rent.minimum_balance(GENERAL_ROOT_BYTES);
    let funding_rent = rent.minimum_balance(GENERAL_FUNDING_BYTES);
    let remaining = funding
        .remaining_lamports()
        .map_err(|_| AdapterError::Arithmetic)?;
    if root_account.lamports() != root_rent
        || funding_account.lamports()
            != funding_rent
                .checked_add(remaining)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::FundUnderfunded.into());
    }
    root.enter_terminal()
        .map_err(|_| AdapterError::MarketTransition)?;
    let refunds = funding
        .refund_terminal(root.phase())
        .map_err(|_| AdapterError::MarketTransition)?;
    let refund_total = refunds
        .iter()
        .try_fold(0u64, |total, amount| total.checked_add(*amount))
        .ok_or(AdapterError::Arithmetic)?;
    if refund_total != remaining {
        return Err(AdapterError::PositionPostcondition.into());
    }
    root.retire(funding)
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut market_root = market.root();
    market_root
        .retire_child(generation, market_root.outstanding_children())
        .map_err(|_| AdapterError::MarketTransition)?;
    let market_after = CategoricalMarketV1::new(
        market_root,
        market.hoard_atoms(),
        *market.supply(),
        market.settlement(),
    )
    .map_err(|_| AdapterError::MarketTransition)?;
    preflight_mutable(&[market_account, root_account, funding_account, rent_credit])?;
    transfer_owned_lamports(funding_account, rent_credit, refund_total)?;
    write_market(market_account, market_after)?;
    close_program_account(funding_account, rent_credit)?;
    close_program_account(root_account, rent_credit)?;
    let total_credit = root_rent
        .checked_add(funding_rent)
        .and_then(|value| value.checked_add(refund_total))
        .ok_or(AdapterError::Arithmetic)?;
    if root_account.lamports() != 0
        || funding_account.lamports() != 0
        || rent_credit.lamports()
            != rent_credit_before
                .checked_add(total_credit)
                .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit, rent_credit_state)
}

fn authenticate_candidate_transition<'info, const N: usize>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'info>,
    batch_account: &AccountInfo<'info>,
    config_account: &AccountInfo<'info>,
    config_cursor: &AccountInfo<'info>,
    candidate_account: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<
    (
        GeneralRootV1,
        GeneralConfigV1,
        BatchRootV1,
        Box<CandidateStateV1<N>>,
        CandidateCapitalizationV1,
    ),
    ProgramError,
> {
    let root = authenticate_root(program_id, root_account)?;
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        root.config_id().to_bytes(),
    )?;
    let candidate =
        authenticate_candidate_boxed(program_id, candidate_account, batch_account, candidate_id)?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        candidate.batch_sequence(),
        root.config_id(),
    )?;
    batch
        .validate_against(config)
        .map_err(|_| AdapterError::FundUnderfunded)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let cap = CandidateCapitalizationV1 {
        account_lamports: candidate_account.lamports(),
        exact_state_rent_lamports: rent.minimum_balance(
            CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
        ),
    };
    candidate
        .validate_capitalization(cap)
        .map_err(|_| AdapterError::FundUnderfunded)?;
    Ok((root, config, batch, candidate, cap))
}

fn pay_candidate_reward<'info, const N: usize>(
    actor: &AccountInfo<'info>,
    candidate_account: &AccountInfo<'info>,
    candidate: CandidateStateV1<N>,
    reward: u64,
) -> Result<(), ProgramError> {
    preflight_mutable(&[actor, candidate_account])?;
    transfer_owned_lamports(candidate_account, actor, reward)?;
    write_candidate(candidate_account, candidate)
}

fn canonical_page_bytes<const N: usize>(
    page: &CandidatePageV1<N>,
) -> Result<Vec<u8>, ProgramError> {
    let length = CandidatePageV1::<N>::encoded_len(page.execution_count)
        .map_err(|_| AdapterError::Arithmetic)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    bytes.resize(length, 0);
    page.encode(&mut bytes)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    Ok(bytes)
}

#[inline(never)]
fn decode_candidate_page_creation_boxed<const N: usize>(
    instruction_data: &[u8],
) -> Result<Box<dclutch_general_contract::CreateGeneralCandidatePageV1<N>>, ProgramError> {
    Ok(Box::new(
        GeneralInstructionV1::<N>::decode_candidate_page_creation(instruction_data)
            .map_err(|_| AdapterError::InvalidInstruction)?,
    ))
}

#[inline(never)]
fn authenticate_candidate_boxed<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    batch: &AccountInfo<'_>,
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<Box<CandidateStateV1<N>>, ProgramError> {
    Ok(Box::new(authenticate_candidate(
        program_id,
        account,
        batch,
        candidate_id,
    )?))
}

#[inline(never)]
fn authenticate_candidate_page_boxed<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    candidate: &AccountInfo<'_>,
    page_id: dclutch_general_contract::ContentId,
) -> Result<Box<CandidatePageV1<N>>, ProgramError> {
    authenticate_candidate_page(program_id, account, candidate, page_id)
}

#[inline(never)]
fn authenticate_settlement_cursor_boxed<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    candidate: &AccountInfo<'_>,
) -> Result<Box<SettlementCursorV1<N>>, ProgramError> {
    Ok(Box::new(authenticate_settlement_cursor(
        program_id, account, candidate,
    )?))
}

#[inline(never)]
fn authenticate_position_boxed<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    owner: &Pubkey,
    generation: u64,
) -> Result<Box<PositionV1<N>>, ProgramError> {
    Ok(Box::new(authenticate_position(
        program_id, account, market, owner, generation,
    )?))
}

#[inline(never)]
fn authenticate_market_boxed<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_key: [u8; 32],
) -> Result<Box<CategoricalMarketV1<N>>, ProgramError> {
    Ok(Box::new(authenticate_market(
        program_id,
        account,
        expected_key,
    )?))
}

#[inline(never)]
fn decode_capability_funding_boxed(
    account: &AccountInfo<'_>,
) -> Result<Box<FundingStateV1>, ProgramError> {
    Ok(Box::new(decode_capability_funding(account)?))
}

#[inline(never)]
fn authenticate_finalized_config_boxed<'info>(
    program_id: &Pubkey,
    raw: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    expected_digest: [u8; 32],
) -> Result<Box<GeneralConfigV1>, ProgramError> {
    Ok(Box::new(authenticate_finalized_config(
        program_id,
        raw,
        cursor,
        rent,
        expected_digest,
    )?))
}

#[inline(never)]
fn empty_position_boxed<const N: usize>(
    market: [u8; 32],
    owner: [u8; 32],
    generation: u64,
) -> Result<Box<PositionV1<N>>, ProgramError> {
    Ok(Box::new(
        PositionV1::<N>::empty(market, owner, generation)
            .map_err(|_| AdapterError::PositionAuthentication)?,
    ))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn begin_settlement_boxed<const N: usize>(
    candidate: &mut CandidateStateV1<N>,
    batch: &mut BatchRootV1,
    root: GeneralRootV1,
    config: GeneralConfigV1,
    capitalization: CandidateCapitalizationV1,
    rent: SettlementRentObservationV1,
    slot: u64,
) -> Result<Box<dclutch_general_contract::SettlementBeginV1<N>>, ProgramError> {
    Ok(Box::new(
        SettlementCursorV1::begin(candidate, batch, root, config, capitalization, rent, slot)
            .map_err(|_| AdapterError::MarketTransition)?,
    ))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn materialize_settlement_boxed<const N: usize>(
    cursor: &mut SettlementCursorV1<N>,
    candidate: &mut CandidateStateV1<N>,
    batch: BatchRootV1,
    root: GeneralRootV1,
    config: GeneralConfigV1,
    claim_inventory: [u64; N],
    quote_inventory: u64,
    capitalization: CandidateCapitalizationV1,
) -> Result<Box<dclutch_general_contract::SettlementMaterializationV1<N>>, ProgramError> {
    Ok(Box::new(
        cursor
            .materialize(
                candidate,
                batch,
                root,
                config,
                claim_inventory,
                quote_inventory,
                capitalization,
            )
            .map_err(|_| AdapterError::MarketTransition)?,
    ))
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
    state_before: u64,
    custody_before: u64,
    escrow_before: u64,
    rent_credit_lamports: u64,
    source_before: TokenAccount,
    realm: RealmFacts,
}

fn process_admit_order<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    order: &PortfolioOrderV1<N>,
) -> Result<(), ProgramError> {
    let owner = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let claim_account = account(accounts, 3)?;
    let config_account = account(accounts, 4)?;
    let realm_cursor = account(accounts, 5)?;
    let claim_cursor = account(accounts, 6)?;
    let config_cursor = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    let root_account = account(accounts, 10)?;
    let batch_account = account(accounts, 11)?;
    let state_account = account(accounts, 12)?;
    let custody_account = account(accounts, 13)?;
    let position_account = account(accounts, 14)?;
    let quote_source = account(accounts, 15)?;
    let quote_escrow = account(accounts, 16)?;
    let rent_credit = account(accounts, 17)?;
    let system = account(accounts, 18)?;
    let rent_sysvar = account(accounts, 19)?;
    let clock_sysvar = account(accounts, 20)?;

    authenticate_system_rent_clock(system, rent_sysvar, clock_sysvar)?;
    require_system_wallet(owner)?;
    require_prefunded_vacant(state_account)?;
    require_prefunded_vacant(custody_account)?;
    require_prefunded_vacant(quote_escrow)?;
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        order.batch_sequence(),
        config_id,
    )?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, config_id)?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
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
    authenticate_order_id(*order)?;
    let rent_credit_state = authenticate_rent_credit(program_id, rent_credit, owner.key)?;
    let mut position = authenticate_position_boxed::<N>(
        program_id,
        position_account,
        market_account,
        owner.key,
        config.generation(),
    )?;

    let state_seeds = GeneralOrderStatePdaSeedsV1::new(market_account.key.to_bytes(), *order)
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

    let admission = Box::new(
        GeneralOrderCustodyV1::admit(
            *order,
            root,
            config,
            rent_credit.key.to_bytes(),
            quote_escrow.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication)?,
    );
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
    let payer_rent = state_rent
        .saturating_sub(state_account.lamports())
        .checked_add(custody_rent.saturating_sub(custody_account.lamports()))
        .and_then(|value| value.checked_add(escrow_rent.saturating_sub(quote_escrow.lamports())))
        .ok_or(AdapterError::Arithmetic)?;
    let payer_before = owner.lamports();
    if payer_before < payer_rent {
        return Err(AdapterError::PositionRentUnderfunded.into());
    }
    preflight_mutable(&[
        owner,
        state_account,
        custody_account,
        position_account,
        quote_source,
        quote_escrow,
        rent_credit,
    ])?;

    let plan = Box::new(AdmissionPlan {
        state: admission.order_state,
        custody: admission.custody,
        position: *position,
        state_bump,
        custody_bump,
        escrow_bump,
        state_rent,
        custody_rent,
        escrow_rent,
        quote_atoms,
        payer_before,
        state_before: state_account.lamports(),
        custody_before: custody_account.lamports(),
        escrow_before: quote_escrow.lamports(),
        rent_credit_lamports: rent_credit.lamports(),
        source_before,
        realm,
    });
    create_order_accounts(
        program_id,
        owner,
        state_account,
        custody_account,
        quote_escrow,
        rent_credit,
        token_program,
        system,
        *order,
        plan.as_ref(),
    )?;
    initialize_and_fund_escrow(
        quote_source,
        quote_escrow,
        mint,
        token_program,
        owner,
        custody_account,
        plan.as_ref(),
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
        plan.as_ref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn create_order_accounts<'info, const N: usize>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    state: &AccountInfo<'info>,
    custody: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    order: PortfolioOrderV1<N>,
    plan: &AdmissionPlan<N>,
) -> Result<(), ProgramError> {
    let state_seeds = GeneralOrderStatePdaSeedsV1::new(*plan.position.market(), order)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    create_general_pda_account(
        payer,
        state,
        rent_credit,
        system,
        program_id,
        plan.state_rent,
        ORDER_STATE_BYTES,
        &state_seeds.seed_components(),
        plan.state_bump,
        false,
    )?;
    let custody_seed = GeneralOrderCustodyPdaSeedsV1::new(state.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    create_general_pda_account(
        payer,
        custody,
        rent_credit,
        system,
        program_id,
        plan.custody_rent,
        GeneralOrderCustodyV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
        &custody_seed.seed_components(),
        plan.custody_bump,
        false,
    )?;
    let escrow_seed = GeneralQuoteEscrowPdaSeedsV1::new(custody.key.to_bytes())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    create_general_pda_account(
        payer,
        escrow,
        rent_credit,
        system,
        token_program.key,
        plan.escrow_rent,
        ACCOUNT_BYTES,
        &escrow_seed.seed_components(),
        plan.escrow_bump,
        false,
    )
}

fn initialize_and_fund_escrow<'info, const N: usize>(
    source: &AccountInfo<'info>,
    escrow: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    custody: &AccountInfo<'info>,
    plan: &AdmissionPlan<N>,
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
    plan: &AdmissionPlan<N>,
) -> Result<(), ProgramError> {
    write_order_state(state_account, plan.state)?;
    write_custody(custody_account, plan.custody)?;
    write_position(position_account, plan.position)?;
    let payer_rent = plan
        .state_rent
        .saturating_sub(plan.state_before)
        .checked_add(plan.custody_rent.saturating_sub(plan.custody_before))
        .and_then(|value| value.checked_add(plan.escrow_rent.saturating_sub(plan.escrow_before)))
        .ok_or(AdapterError::Arithmetic)?;
    let dust_surplus = plan
        .state_before
        .saturating_sub(plan.state_rent)
        .checked_add(plan.custody_before.saturating_sub(plan.custody_rent))
        .and_then(|value| value.checked_add(plan.escrow_before.saturating_sub(plan.escrow_rent)))
        .ok_or(AdapterError::Arithmetic)?;
    if payer.lamports()
        != plan
            .payer_before
            .checked_sub(payer_rent)
            .ok_or(AdapterError::Arithmetic)?
        || state_account.owner != program_id
        || state_account.lamports() != plan.state_rent
        || custody_account.owner != program_id
        || custody_account.lamports() != plan.custody_rent
        || quote_escrow.owner != token_program.key
        || quote_escrow.lamports() != plan.escrow_rent
        || rent_credit.lamports()
            != plan
                .rent_credit_lamports
                .checked_add(dust_surplus)
                .ok_or(AdapterError::Arithmetic)?
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
    order: &PortfolioOrderV1<N>,
    cancellation: bool,
) -> Result<(), ProgramError> {
    let owner = if cancellation {
        Some(account(accounts, 0)?)
    } else {
        None
    };
    let offset = usize::from(cancellation);
    let market_account = account(accounts, offset)?;
    let realm_account = account(accounts, offset + 1)?;
    let claim_account = account(accounts, offset + 2)?;
    let config_account = account(accounts, offset + 3)?;
    let realm_cursor = account(accounts, offset + 4)?;
    let claim_cursor = account(accounts, offset + 5)?;
    let config_cursor = account(accounts, offset + 6)?;
    let root_account = account(accounts, offset + 7)?;
    let batch_account = account(accounts, offset + 8)?;
    let state_account = account(accounts, offset + 9)?;
    let custody_account = account(accounts, offset + 10)?;
    let position_account = account(accounts, offset + 11)?;
    let quote_escrow = account(accounts, offset + 12)?;
    let quote_destination = account(accounts, offset + 13)?;
    let mint = account(accounts, offset + 14)?;
    let token_program = account(accounts, offset + 15)?;
    let rent_credit = account(accounts, offset + 16)?;
    let rent_sysvar = account(accounts, offset + 17)?;
    let clock_sysvar = if cancellation {
        Some(account(accounts, offset + 18)?)
    } else {
        None
    };
    let root = authenticate_root(program_id, root_account)?;
    let config_id = root.config_id();
    let config = authenticate_finalized_config(
        program_id,
        config_account,
        config_cursor,
        rent_sysvar,
        config_id.to_bytes(),
    )?;
    authenticate_order_id(*order)?;
    let market = authenticate_market::<N>(program_id, market_account, root.market())?;
    authenticate_market_config(market, config, root, config_id)?;
    let claim = authenticate_claim_basis(
        program_id,
        claim_account,
        claim_cursor,
        rent_sysvar,
        config.claim_basis_id().to_bytes(),
    )?;
    authenticate_claim_basis_config::<N>(claim, config)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        realm_cursor,
        rent_sysvar,
        mint,
        token_program,
        market.root().identity().realm_id().to_bytes(),
    )?;
    let batch = authenticate_batch(
        program_id,
        batch_account,
        root_account,
        order.batch_sequence(),
        config_id,
    )?;
    let state_seeds = GeneralOrderStatePdaSeedsV1::new(market_account.key.to_bytes(), *order)
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
    let custody = Box::new(decode_custody::<N>(custody_account)?);
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
    let mut position = authenticate_position_boxed::<N>(
        program_id,
        position_account,
        market_account,
        &Pubkey::new_from_array(order.owner().to_bytes()),
        config.generation(),
    )?;
    let release = Box::new(if cancellation {
        let slot = authenticate_clock(clock_sysvar.ok_or(AdapterError::AccountFrameLength)?)?.slot;
        custody
            .cancel_and_release(
                &mut state,
                *order,
                order.owner(),
                slot,
                batch.collection_close(),
                root,
                config,
            )
            .map_err(|_| AdapterError::MarketTransition)?
    } else {
        custody
            .close_after_batch(&mut state, *order, batch, root, config)
            .map_err(|_| AdapterError::MarketTransition)?
    });
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
    let plan = Box::new(ReleasePlan {
        state,
        position: *position,
        release: *release,
        custody_bump,
        realm,
        source_before,
        destination_before,
        escrow_close,
        custody_close,
        rent_credit_state,
    });
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
        plan.as_ref(),
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
    plan: &ReleasePlan<N>,
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

#[inline(never)]
fn authenticate_finalized_config<'info>(
    program_id: &Pubkey,
    raw: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    expected_digest: [u8; 32],
) -> Result<GeneralConfigV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        raw,
        cursor,
        rent,
        GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes(),
        expected_digest,
        |record| {
            let config = GeneralConfigV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData)?;
            if config.to_bytes().as_slice() != record.exact_content() {
                return Err(AdapterError::ContentIdentity.into());
            }
            Ok(config)
        },
    )
}

fn authenticate_claim_basis<'info>(
    program_id: &Pubkey,
    raw: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    expected_digest: [u8; 32],
) -> Result<CategoricalUnitV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        raw,
        cursor,
        rent,
        CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
        expected_digest,
        |record| {
            CategoricalUnitV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData.into())
        },
    )
}

fn authenticate_claim_basis_config<const N: usize>(
    claim: CategoricalUnitV1,
    config: GeneralConfigV1,
) -> Result<(), ProgramError> {
    if usize::try_from(claim.outcome_count()).map_err(|_| AdapterError::Arithmetic)? != N
        || usize::from(config.outcome_count()) != N
        || claim.capacity_profile_id().content_id().as_bytes()
            != config.capacity_profile_id().as_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(())
}

fn decode_capability_funding(account: &AccountInfo<'_>) -> Result<FundingStateV1, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let state = FundingStateV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    if state.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(state)
}

fn decode_general_funding(account: &AccountInfo<'_>) -> Result<GeneralFundingV1, ProgramError> {
    if account.data_len() != GENERAL_FUNDING_BYTES {
        return Err(AdapterError::AccountData.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let funding = GeneralFundingV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let mut canonical = [0; GENERAL_FUNDING_BYTES];
    funding
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(funding)
}

#[allow(dead_code)]
fn authenticate_general_funding(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: GeneralRootV1,
    config_id: dclutch_general_contract::ContentId,
    config: GeneralConfigV1,
) -> Result<GeneralFundingV1, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let seeds = GeneralFundingPdaSeedsV1::new(
        root.market(),
        root.generation(),
        config_id,
        config.capability_release_id(),
    )
    .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    let funding = decode_general_funding(account)?;
    if account.key != &expected
        || root.config_id() != config_id
        || funding.capability_release_id() != config.capability_release_id()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(funding)
}

fn authenticate_rent_credit_key(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_key: [u8; 32],
) -> Result<RentCreditV1, ProgramError> {
    if account.key.to_bytes() != expected_key
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
    let seeds = credit.pda_seeds();
    let authority = seeds.refund_authority().to_bytes();
    let (derived, bump) =
        Pubkey::find_program_address(&[seeds.domain(), authority.as_slice()], program_id);
    if account.key != &derived
        || bump != credit.pda_bump()
        || credit.to_bytes().as_slice() != &data[..]
    {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    Ok(credit)
}

fn decode_batch(account: &AccountInfo<'_>) -> Result<BatchRootV1, ProgramError> {
    if account.owner == &system_program::ID
        || account.executable
        || account.data_len() != BATCH_ROOT_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let batch = BatchRootV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let mut canonical = [0; BATCH_ROOT_BYTES];
    batch
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(batch)
}

fn decode_candidate<const N: usize>(
    account: &AccountInfo<'_>,
) -> Result<CandidateStateV1<N>, ProgramError> {
    let length = CandidateStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    if account.executable || account.data_len() != length {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let candidate = CandidateStateV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    canonical.resize(length, 0);
    candidate
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(candidate)
}

fn authenticate_candidate<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    batch: &AccountInfo<'_>,
    candidate_id: dclutch_general_contract::ContentId,
) -> Result<CandidateStateV1<N>, ProgramError> {
    if account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let seeds = GeneralCandidatePdaSeedsV1::new(batch.key.to_bytes(), candidate_id)
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    let candidate = decode_candidate(account)?;
    if account.key != &expected || candidate.candidate_id() != candidate_id {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(candidate)
}

#[inline(never)]
fn authenticate_candidate_page<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    candidate: &AccountInfo<'_>,
    page_id: dclutch_general_contract::ContentId,
) -> Result<Box<CandidatePageV1<N>>, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let page = decode_candidate_page_boxed::<N>(&data)?;
    let canonical = canonical_page_bytes(page.as_ref())?;
    let digest = hashv(&[GENERAL_CANDIDATE_PAGE_CONTENT_DOMAIN_V1, &canonical]).to_bytes();
    let seeds = GeneralCandidatePagePdaSeedsV1::new(candidate.key.to_bytes(), page_id)
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    if account.key != &expected || digest != page_id.to_bytes() || canonical.as_slice() != &data[..]
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(page)
}

#[inline(never)]
fn decode_candidate_page_boxed<const N: usize>(
    data: &[u8],
) -> Result<Box<CandidatePageV1<N>>, ProgramError> {
    Ok(Box::new(
        CandidatePageV1::decode(data).map_err(|_| AdapterError::AccountData)?,
    ))
}

fn authenticate_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<GeneralRootV1, ProgramError> {
    if account.owner != program_id || account.executable || account.data_len() != GENERAL_ROOT_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let root = GeneralRootV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let config_id = root.config_id();
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
    if identity.claim_basis_id().to_bytes() != config.claim_basis_id().to_bytes()
        || identity.generation() != config.generation()
        || root.config_id() != config_id
        || root.generation() != config.generation()
        || root.market() == [0; 32]
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

fn write_market<const N: usize>(
    account: &AccountInfo<'_>,
    market: CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    market
        .encode(&mut data)
        .map_err(|_| AdapterError::MarketTransition)?;
    if CategoricalMarketV1::<N>::decode(&data) != Ok(market) {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

fn write_capability_funding(
    account: &AccountInfo<'_>,
    funding: FundingStateV1,
) -> Result<(), ProgramError> {
    let bytes = funding.to_bytes();
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    if data.len() != bytes.len() {
        return Err(AdapterError::AccountData.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

fn write_root(account: &AccountInfo<'_>, root: GeneralRootV1) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    root.encode(&mut data)
        .map_err(|_| AdapterError::AccountData)?;
    if GeneralRootV1::decode(&data) != Ok(root) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_general_funding(
    account: &AccountInfo<'_>,
    funding: GeneralFundingV1,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    funding
        .encode(&mut data)
        .map_err(|_| AdapterError::AccountData)?;
    if GeneralFundingV1::decode(&data) != Ok(funding) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
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

fn write_candidate<const N: usize>(
    account: &AccountInfo<'_>,
    candidate: CandidateStateV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    candidate
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if CandidateStateV1::<N>::decode(&data) != Ok(candidate) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_candidate_page<const N: usize>(
    account: &AccountInfo<'_>,
    page: CandidatePageV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    page.encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if CandidatePageV1::<N>::decode(&data) != Ok(page) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn write_settlement_cursor<const N: usize>(
    account: &AccountInfo<'_>,
    cursor: SettlementCursorV1<N>,
) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    cursor
        .encode(&mut data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if SettlementCursorV1::<N>::decode(&data) != Ok(cursor) {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn authenticate_settlement_cursor<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    candidate: &AccountInfo<'_>,
) -> Result<SettlementCursorV1<N>, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let seeds = GeneralSettlementCursorPdaSeedsV1::new(candidate.key.to_bytes())
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let cursor = SettlementCursorV1::<N>::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(
            SettlementCursorV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
        )
        .map_err(|_| AdapterError::Arithmetic)?;
    canonical.resize(data.len(), 0);
    cursor
        .encode(&mut canonical)
        .map_err(|_| AdapterError::AccountData)?;
    if account.key != &expected || canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(cursor)
}

fn authenticate_settlement_quote_escrow(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    cursor: &Pubkey,
) -> Result<TokenAccount, ProgramError> {
    let seeds = GeneralSettlementEscrowPdaSeedsV1::new(cursor.to_bytes())
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    if account.key != &expected || account.owner != token_program.key || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            cursor.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication.into())
}

fn authenticate_order_quote_escrow(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    custody: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<TokenAccount, ProgramError> {
    let seeds = GeneralQuoteEscrowPdaSeedsV1::new(custody.key.to_bytes())
        .map_err(|_| AdapterError::AccountData)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.seed_components(), program_id);
    if account.key != &expected || account.owner != token_program.key || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            custody.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_collateral_vault(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    hoard_atoms: u64,
) -> Result<TokenAccount, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
        program_id,
    );
    if vault.key != &expected || vault.owner != token_program.key || vault.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let state = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            market.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if state.amount < hoard_atoms {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(state)
}

fn authenticate_quote_destination(
    account: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    owner: [u8; 32],
) -> Result<TokenAccount, ProgramError> {
    if account.owner != token_program.key || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let state = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if state.mint != mint.key.to_bytes() || state.owner != owner {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(state)
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

#[allow(clippy::too_many_arguments)]
fn create_general_pda_account<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    owner: &Pubkey,
    exact_rent: u64,
    space: usize,
    seeds: &[&[u8]],
    bump: u8,
    prepaid_rent: bool,
) -> Result<(), ProgramError> {
    require_prefunded_vacant(created)?;
    let before = created.lamports();
    let (top_up, surplus, prepaid_credit) = precreation_amounts(before, exact_rent, prepaid_rent);
    let bump_seed = [bump];
    let mut signer = Vec::new();
    signer
        .try_reserve_exact(seeds.len().saturating_add(1))
        .map_err(|_| AdapterError::Arithmetic)?;
    signer.extend_from_slice(seeds);
    signer.push(bump_seed.as_slice());
    if surplus != 0 {
        invoke_signed(
            &transfer(created.key, rent_credit.key, surplus),
            &[created.clone(), rent_credit.clone(), system.clone()],
            &[signer.as_slice()],
        )
        .map_err(|_| AdapterError::PositionCreateCpi)?;
    }
    if top_up != 0 {
        invoke(
            &transfer(payer.key, created.key, top_up),
            &[payer.clone(), created.clone(), system.clone()],
        )
        .map_err(|_| AdapterError::PositionCreateCpi)?;
    }
    if prepaid_credit != 0 {
        invoke(
            &transfer(payer.key, rent_credit.key, prepaid_credit),
            &[payer.clone(), rent_credit.clone(), system.clone()],
        )
        .map_err(|_| AdapterError::PositionCreateCpi)?;
    }
    let space = u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?;
    invoke_signed(
        &allocate(created.key, space),
        &[created.clone(), system.clone()],
        &[signer.as_slice()],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    invoke_signed(
        &assign(created.key, owner),
        &[created.clone(), system.clone()],
        &[signer.as_slice()],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    if created.owner != owner
        || created.executable
        || created.lamports() != exact_rent
        || created.data_len() != usize::try_from(space).map_err(|_| AdapterError::Arithmetic)?
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn precreation_amounts(before: u64, exact_rent: u64, prepaid_rent: bool) -> (u64, u64, u64) {
    (
        exact_rent.saturating_sub(before),
        before.saturating_sub(exact_rent),
        if prepaid_rent {
            before.min(exact_rent)
        } else {
            0
        },
    )
}

fn transfer_owned_lamports(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    if source.key == destination.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    **source
        .try_borrow_mut_lamports()
        .map_err(|_| AdapterError::AccountData)? = source_after;
    **destination
        .try_borrow_mut_lamports()
        .map_err(|_| AdapterError::AccountData)? = destination_after;
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

fn require_prefunded_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || account.executable || !account.data_is_empty() {
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
        GENERAL_INSTRUCTION_HEADER_BYTES, GeneralInstructionV1, PortfolioOrderV1Input,
    };

    use super::*;

    fn id(value: u8) -> dclutch_general_contract::ContentId {
        dclutch_general_contract::ContentId::new([value; 32]).expect("ID")
    }

    fn exact_order() -> PortfolioOrderV1<2> {
        let provisional = PortfolioOrderV1::new(PortfolioOrderV1Input {
            market: [2; 32],
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
                market: [2; 32],
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
                market: [2; 32],
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
    fn enabled_activation_requires_its_exact_frame() {
        let instruction =
            GeneralInstructionV1::<2>::Activate(dclutch_general_contract::ActivateGeneralV1 {
                expected_market_child_count: 0,
            });
        let mut bytes = std::vec![0; instruction.encoded_len().expect("length")];
        instruction.encode(&mut bytes).expect("instruction");
        assert_eq!(
            dispatch_width::<2>(&Pubkey::new_unique(), &[], &bytes),
            Err(AdapterError::AccountFrameLength.into())
        );
    }

    #[test]
    fn every_settlement_and_terminal_tag_reaches_its_exact_frame() {
        let candidate = id(40);
        let page = dclutch_general_contract::GeneralCandidatePageV1 {
            candidate_id: candidate,
            page_id: id(41),
        };
        let instructions = [
            GeneralInstructionV1::<2>::BeginSettlement(candidate),
            GeneralInstructionV1::<2>::CollectSettlementPage(page),
            GeneralInstructionV1::<2>::MaterializeSettlement(candidate),
            GeneralInstructionV1::<2>::DistributeSettlementPage(page),
            GeneralInstructionV1::<2>::FinishSettlement(candidate),
            GeneralInstructionV1::<2>::CloseSettlement(candidate),
            GeneralInstructionV1::<2>::Quiesce(7),
            GeneralInstructionV1::<2>::CloseGeneral(7),
        ];
        for instruction in instructions {
            let mut bytes = std::vec![0; instruction.encoded_len().expect("length")];
            instruction.encode(&mut bytes).expect("instruction");
            assert_eq!(
                dispatch_width::<2>(&Pubkey::new_unique(), &[], &bytes),
                Err(AdapterError::AccountFrameLength.into())
            );
        }
    }

    #[test]
    fn precreation_dust_cannot_change_exact_rent_or_become_prepaid_actor_income() {
        assert_eq!(precreation_amounts(0, 10, true), (10, 0, 0));
        assert_eq!(precreation_amounts(3, 10, true), (7, 0, 3));
        assert_eq!(precreation_amounts(10, 10, true), (0, 0, 10));
        assert_eq!(precreation_amounts(12, 10, true), (0, 2, 10));
        assert_eq!(precreation_amounts(3, 10, false), (7, 0, 0));
        assert_eq!(precreation_amounts(12, 10, false), (0, 2, 0));

        for before in [0, 3, 10, 12, u64::MAX] {
            let (top_up, surplus, prepaid_credit) = precreation_amounts(before, 10, true);
            assert_eq!(
                before
                    .checked_add(top_up)
                    .and_then(|value| value.checked_sub(surplus)),
                Some(10)
            );
            assert_eq!(surplus.checked_add(prepaid_credit), Some(before));
            assert_eq!(top_up.checked_add(prepaid_credit), Some(10));
        }
    }
}
