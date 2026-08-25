//! Executable Source/recovery lifecycle and the pinned Pyth provider boundary.

use alloc::vec::Vec;

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, FundingAssetClassV1, FundingCompartment,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_core_contract::{ContentId as CoreContentId, Phase};
use dclutch_market_contract::market::{
    CategoricalMarketV1, CategoricalSettlementSummaryV1, decode_market_outcome_count,
};
use dclutch_product_contract::{ContentId as ProductContentId, terminal::ResolutionKind};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, PostUpdateParamsView, ProgramDataV3View, ProgramV3View, PythReleaseV1,
    ReceiverConfigV2View,
};
use dclutch_rent_contract::{RENT_CREDIT_BYTES_V1, RefundAuthority};
use dclutch_source_contract::{
    AcceptEvidenceInstructionV1, AcceptSharedObservationInstructionV1, CommitFailureInstructionV1,
    ContentId as SourceContentId, CreateResolutionInstructionV1,
    CreateSharedObservationInstructionV1, GenerationInstructionV1, MarketChildDeltaKindV1,
    NormalizedProviderEvidenceV1, PythProviderAdapterObligationV1, RetireInstructionV1,
    SHARED_OBSERVATION_PDA_DOMAIN_V1, SHARED_OBSERVATION_STATE_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_BYTES,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1, SharedObservationPhaseV1, SharedObservationStateViewV1,
    SourceAccessProfile, SourceAccountPrivilegeV1, SourceFrameKindV1, SourceInstructionV1,
    SourceMaterialViewV1, SourceResolutionDecisionV1, SourceResolutionPhaseV1,
    SourceResolutionRouteV1, SourceResolutionStateV1, accept_shared_provider_output_in_place_v1,
    create_shared_observation_state_into_v1, encode_shared_evidence_set_preimage_v1,
    retire_shared_observation_in_place_v1, shared_evidence_set_preimage_len_v1,
    validate_source_frame_v1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::{
    AdapterError,
    authenticate::{MARKET_SEED, PriceFrame, ProviderFacts, SYSTEM_PROGRAM, selected_release},
    provider,
    records::{
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, authenticate_rent_credit, derive_record_pda,
        with_authenticated_finalized_record_v1,
    },
};

const RECEIVER_CONFIG_SEED: &[u8] = b"config";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const UPGRADEABLE_LOADER: Pubkey = Pubkey::new_from_array([
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
]);

/// Dispatch one exact Source instruction after top-level magic routing.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let instruction = SourceInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    match instruction {
        SourceInstructionV1::CreateResolution(request) => {
            process_create_resolution(program_id, accounts, request)
        }
        SourceInstructionV1::AcceptEvidence(request, payload) => {
            process_accept_evidence(program_id, accounts, request, payload)
        }
        SourceInstructionV1::FailNext(request) => process_fail_next(program_id, accounts, request),
        SourceInstructionV1::Exhaust(request) => process_exhaust(program_id, accounts, request),
        SourceInstructionV1::CommitFailure(request) => {
            process_commit_failure(program_id, accounts, request)
        }
        SourceInstructionV1::RetireResolution(request) => {
            process_retire_resolution(program_id, accounts, request)
        }
        SourceInstructionV1::CreateSharedObservation(request) => {
            process_create_shared(program_id, accounts, request)
        }
        SourceInstructionV1::AcceptSharedObservation(request, payload) => {
            process_accept_shared(program_id, accounts, request, payload)
        }
        SourceInstructionV1::RetireSharedObservation(request) => {
            process_retire_shared(program_id, accounts, request)
        }
    }
}

fn process_create_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateResolutionInstructionV1,
) -> Result<(), ProgramError> {
    let kind = if request.reopen_link().is_some() {
        SourceFrameKindV1::CreateResolutionReopen
    } else {
        SourceFrameKindV1::CreateResolutionFresh
    };
    validate_frame(kind, accounts)?;
    let payer = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let predecessor_offset = usize::from(request.reopen_link().is_some());
    let predecessor = request
        .reopen_link()
        .map(|_| account(accounts, 2))
        .transpose()?;
    let market_account = account(accounts, 2 + predecessor_offset)?;
    let material_account = account(accounts, 3 + predecessor_offset)?;
    let material_staging = account(accounts, 4 + predecessor_offset)?;
    let rent_sysvar = account(accounts, 5 + predecessor_offset)?;
    let rent_credit = account(accounts, 6 + predecessor_offset)?;
    let system = account(accounts, 7 + predecessor_offset)?;
    require_fixed_accounts(system, rent_sysvar, None)?;
    if market_account.key.to_bytes() != request.market() {
        return Err(AdapterError::AccountIdentity.into());
    }
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        request.material_id(),
        true,
    )?;
    let outcome_count = with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        request.material_id(),
        |material| {
            material
                .result_domain()
                .map(|domain| domain.outcome_count())
                .map_err(|_| AdapterError::MarketTransition.into())
        },
    )?;
    if outcome_count != market.outcome_count {
        return Err(AdapterError::MarketTransition.into());
    }
    if market.child_count != request.expected_market_child_count() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_existing_rent_credit(
        program_id,
        rent_credit,
        rent_sysvar,
        request.rent_beneficiary(),
    )?;
    let generation = request.generation().to_le_bytes();
    let bump = [request.pda_bump()];
    let signer = [
        SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
        market_account.key.as_ref(),
        generation.as_slice(),
        bump.as_slice(),
    ];
    let expected = Pubkey::create_program_address(&signer, program_id)
        .map_err(|_| AdapterError::AccountIdentity)?;
    if state_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let plan = if let Some(link) = request.reopen_link() {
        let predecessor = predecessor.ok_or(AdapterError::AccountFrameLength)?;
        let predecessor_state = decode_resolution_state(program_id, predecessor)?;
        let predecessor_data = predecessor
            .try_borrow_data()
            .map_err(|_| AdapterError::AccountData)?;
        let predecessor_id = SourceContentId::new(hash(&predecessor_data).to_bytes())
            .map_err(|_| AdapterError::ContentIdentity)?;
        let predecessor_decision = predecessor_state
            .decision(market.outcome_count)
            .map_err(|_| AdapterError::ReplayMismatch)?;
        if predecessor_state.phase() != SourceResolutionPhaseV1::Retired
            || predecessor_state.generation() != link.previous_generation()
            || predecessor_id != link.predecessor_state_id()
            || predecessor_decision.resolution_evidence_id()
                != link.predecessor_terminal_evidence_id()
        {
            return Err(AdapterError::ReplayMismatch.into());
        }
        let link_id = SourceContentId::new(hash(&link.to_bytes()).to_bytes())
            .map_err(|_| AdapterError::ContentIdentity)?;
        SourceResolutionStateV1::reopened(
            request.market(),
            request.generation(),
            request.material_id(),
            request.rent_beneficiary(),
            request.pda_bump(),
            link_id,
            link,
            request.expected_market_child_count(),
            market.child_count,
        )
    } else {
        SourceResolutionStateV1::fresh(
            request.market(),
            request.generation(),
            request.material_id(),
            request.rent_beneficiary(),
            request.pda_bump(),
            request.expected_market_child_count(),
            market.child_count,
        )
    }
    .map_err(|_| AdapterError::MarketTransition)?;
    require_register_delta(plan.market_delta(), market.child_count)?;
    let market_bytes = register_market_child(
        program_id,
        market_account,
        request.generation(),
        request.material_id(),
        request.expected_market_child_count(),
    )?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    create_prefunded_pda(
        payer,
        state_account,
        system,
        rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES),
        SOURCE_RESOLUTION_STATE_BYTES,
        program_id,
        &signer,
    )?;
    persist_exact(state_account, &plan.state().to_bytes())?;
    persist_bytes(market_account, &market_bytes)?;
    Ok(())
}

fn process_create_shared(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateSharedObservationInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::CreateSharedObservation, accounts)?;
    let payer = account(accounts, 0)?;
    let child_account = account(accounts, 1)?;
    let market_account = account(accounts, 2)?;
    let material_account = account(accounts, 3)?;
    let material_staging = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let rent_credit = account(accounts, 6)?;
    let system = account(accounts, 7)?;
    let clock_account = account(accounts, 8)?;
    require_fixed_accounts(system, rent_sysvar, Some(clock_account))?;
    if market_account.key.to_bytes() != request.market() {
        return Err(AdapterError::AccountIdentity.into());
    }
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        request.material_id(),
        true,
    )?;
    if market.child_count != request.expected_market_child_count() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_existing_rent_credit(
        program_id,
        rent_credit,
        rent_sysvar,
        request.rent_beneficiary(),
    )?;
    let generation = request.generation().to_le_bytes();
    let source = request.source_spec_id().to_bytes();
    let window = request.window_spec_id().to_bytes();
    let bump = [request.pda_bump()];
    let signer = [
        SHARED_OBSERVATION_PDA_DOMAIN_V1,
        market_account.key.as_ref(),
        generation.as_slice(),
        source.as_slice(),
        window.as_slice(),
        bump.as_slice(),
    ];
    let expected = Pubkey::create_program_address(&signer, program_id)
        .map_err(|_| AdapterError::AccountIdentity)?;
    if child_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    create_prefunded_pda(
        payer,
        child_account,
        system,
        rent.minimum_balance(SHARED_OBSERVATION_STATE_BYTES),
        SHARED_OBSERVATION_STATE_BYTES,
        program_id,
        &signer,
    )?;
    let clock = clock(clock_account)?;
    let observed_children =
        u32::try_from(market.child_count).map_err(|_| AdapterError::Arithmetic)?;
    let delta = with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        request.material_id(),
        |material| {
            if material
                .result_domain()
                .map_err(|_| AdapterError::MarketTransition)?
                .outcome_count()
                != market.outcome_count
            {
                return Err(AdapterError::MarketTransition.into());
            }
            let mut child_data = child_account
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::AccountData)?;
            create_shared_observation_state_into_v1(
                &mut child_data,
                request.market(),
                request.generation(),
                request.material_id(),
                material,
                request.source_spec_id(),
                observed_children,
                request.window_spec_id(),
                request.rent_beneficiary(),
                request.pda_bump(),
                clock.unix_timestamp,
                request.expected_market_child_count(),
                market.child_count,
            )
            .map_err(|_| AdapterError::MarketTransition.into())
        },
    )?;
    require_register_delta(delta, market.child_count)?;
    let market_bytes = register_market_child(
        program_id,
        market_account,
        request.generation(),
        request.material_id(),
        request.expected_market_child_count(),
    )?;
    persist_bytes(market_account, &market_bytes)
}

fn process_accept_evidence(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: AcceptEvidenceInstructionV1,
    payload: &[u8],
) -> Result<(), ProgramError> {
    let state_account = account(accounts, 0)?;
    let mut state = decode_resolution_state(program_id, state_account)?;
    if state.generation() != request.generation() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let (
        kind,
        shared,
        market_index,
        material_index,
        staging_index,
        rent_index,
        funding_indices,
        clock_index,
        extension_index,
    ) = match (state.phase(), accounts.len()) {
        (SourceResolutionPhaseV1::Primary, 16) => (
            SourceFrameKindV1::AcceptPrimaryInline,
            None,
            1,
            2,
            3,
            4,
            None,
            5,
            Some(6),
        ),
        (SourceResolutionPhaseV1::Primary, 7) => (
            SourceFrameKindV1::AcceptPrimaryShared,
            Some(1),
            2,
            3,
            4,
            5,
            None,
            6,
            None,
        ),
        (SourceResolutionPhaseV1::Recovery, 18) => (
            SourceFrameKindV1::AcceptRecoveryInline,
            None,
            1,
            2,
            3,
            4,
            Some((5, 6)),
            7,
            Some(8),
        ),
        (SourceResolutionPhaseV1::Recovery, 9) => (
            SourceFrameKindV1::AcceptRecoveryShared,
            Some(1),
            2,
            3,
            4,
            5,
            Some((6, 7)),
            8,
            None,
        ),
        _ => return Err(AdapterError::AccountFrameLength.into()),
    };
    validate_frame(kind, accounts)?;
    let market_account = account(accounts, market_index)?;
    let material_account = account(accounts, material_index)?;
    let material_staging = account(accounts, staging_index)?;
    let rent_sysvar = account(accounts, rent_index)?;
    let clock_account = account(accounts, clock_index)?;
    require_rent_clock(rent_sysvar, clock_account)?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        true,
    )?;
    let now = clock(clock_account)?;
    with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        state.material_id(),
        |material| {
            let (source_id, source) = active_source_view(state, material)?;
            let expected_shared =
                source.access_profile() == SourceAccessProfile::SharedObservationChild;
            if expected_shared != shared.is_some() {
                return Err(AdapterError::AccountFrameLength.into());
            }
            let mut funding_after = None;
            let funding_allocation = if let Some((manifest_index, funding_index)) = funding_indices
            {
                let manifest_account = account(accounts, manifest_index)?;
                let funding_account = account(accounts, funding_index)?;
                let anticipated_fee = if extension_index.is_some() {
                    Some(provider_fee_view(
                        material,
                        source_id,
                        accounts,
                        extension_index.ok_or(AdapterError::AccountFrameLength)?,
                        rent_sysvar,
                        payload,
                        &now,
                    )?)
                } else {
                    None
                };
                let authenticated = authenticate_recovery_funding(
                    program_id,
                    market_account,
                    manifest_account,
                    funding_account,
                    rent_sysvar,
                    market.capability_manifest_id,
                    state.material_id(),
                    anticipated_fee,
                    extension_index
                        .map(|index| account(accounts, index))
                        .transpose()?,
                )?;
                funding_after = authenticated.next_state;
                Some(authenticated.allocation_id)
            } else {
                None
            };

            let (decision, pyth_frame) = if let Some(shared_index) = shared {
                if !payload.is_empty() {
                    return Err(AdapterError::InvalidInstruction.into());
                }
                let child_account = account(accounts, shared_index)?;
                let child_data = child_account
                    .try_borrow_data()
                    .map_err(|_| AdapterError::AccountData)?;
                let child = decode_shared_state_view(program_id, child_account, &child_data)?;
                let evidence_id = child
                    .evidence_id()
                    .map_err(|_| AdapterError::AccountData)?
                    .ok_or(AdapterError::ReplayMismatch)?;
                let evidence = collect_observations_view(child)?;
                let decision = state
                    .accept_provider_output_view(
                        state.material_id(),
                        material,
                        evidence_id,
                        &evidence,
                        Some(child),
                        funding_allocation,
                        request.generation(),
                        now.unix_timestamp,
                        request.terminal_sequence(),
                    )
                    .map_err(|_| AdapterError::MarketTransition)?;
                (decision, None)
            } else {
                let extension_index = extension_index.ok_or(AdapterError::AccountFrameLength)?;
                let pyth = authenticate_pyth_view(
                    material,
                    source_id,
                    accounts,
                    extension_index,
                    rent_sysvar,
                    payload,
                    &now,
                    0,
                )?;
                let update = provider::post_and_load(
                    &pyth.frame,
                    pyth.facts,
                    payload,
                    now.slot,
                    pyth.obligation.adapter_config().provider_feed_id(),
                )?;
                let evidence_id = SourceContentId::new(hash(payload).to_bytes())
                    .map_err(|_| AdapterError::ContentIdentity)?;
                let schedule_id = material
                    .window()
                    .map_err(|_| AdapterError::MarketTransition)?
                    .schedule_id();
                let normalized =
                    normalize_update(pyth.obligation, evidence_id, schedule_id, 0, update)?;
                let evidence = [normalized];
                let decision = state
                    .accept_provider_output_view(
                        state.material_id(),
                        material,
                        evidence_id,
                        &evidence,
                        None,
                        funding_allocation,
                        request.generation(),
                        now.unix_timestamp,
                        request.terminal_sequence(),
                    )
                    .map_err(|_| AdapterError::MarketTransition)?;
                (decision, Some(pyth.frame))
            };
            if decision.outcome_count() != market.outcome_count {
                return Err(AdapterError::MarketTransition.into());
            }
            if let Some(frame) = pyth_frame {
                provider::reclaim(&frame)?;
            }
            let market_bytes = settle_market(
                program_id,
                market_account,
                request.generation(),
                state.material_id(),
                decision,
            )?;
            if let (Some((_, funding_index)), Some(next)) = (funding_indices, funding_after) {
                persist_exact(account(accounts, funding_index)?, &next.to_bytes())?;
            }
            persist_exact(state_account, &state.to_bytes())?;
            persist_bytes(market_account, &market_bytes)
        },
    )
}

fn process_fail_next(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: GenerationInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::FailNext, accounts)?;
    let state_account = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let material_account = account(accounts, 2)?;
    let material_staging = account(accounts, 3)?;
    let rent_sysvar = account(accounts, 4)?;
    let manifest_account = account(accounts, 5)?;
    let funding_account = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
    require_rent_clock(rent_sysvar, clock_account)?;
    let mut state = decode_resolution_state(program_id, state_account)?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        true,
    )?;
    let funding = authenticate_recovery_funding(
        program_id,
        market_account,
        manifest_account,
        funding_account,
        rent_sysvar,
        market.capability_manifest_id,
        state.material_id(),
        None,
        None,
    )?;
    let now = clock(clock_account)?;
    with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        state.material_id(),
        |material| {
            state
                .fail_next_view(
                    state.material_id(),
                    material,
                    funding.allocation_id,
                    request.generation(),
                    now.unix_timestamp,
                )
                .map_err(|_| AdapterError::MarketTransition.into())
        },
    )?;
    persist_exact(state_account, &state.to_bytes())
}

fn process_exhaust(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: GenerationInstructionV1,
) -> Result<(), ProgramError> {
    let state_account = account(accounts, 0)?;
    let mut state = decode_resolution_state(program_id, state_account)?;
    let kind = match state.phase() {
        SourceResolutionPhaseV1::Primary => SourceFrameKindV1::ExhaustPrimary,
        SourceResolutionPhaseV1::Recovery => SourceFrameKindV1::ExhaustRecovery,
        _ => return Err(AdapterError::ReplayMismatch.into()),
    };
    validate_frame(kind, accounts)?;
    let market_account = account(accounts, 1)?;
    let material_account = account(accounts, 2)?;
    let material_staging = account(accounts, 3)?;
    let rent_sysvar = account(accounts, 4)?;
    let clock_account = account(accounts, 5)?;
    require_rent_clock(rent_sysvar, clock_account)?;
    market_facts(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        true,
    )?;
    let now = clock(clock_account)?;
    with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        state.material_id(),
        |material| {
            state
                .exhaust_view(
                    state.material_id(),
                    material,
                    request.generation(),
                    now.unix_timestamp,
                )
                .map_err(|_| AdapterError::MarketTransition.into())
        },
    )?;
    persist_exact(state_account, &state.to_bytes())
}

fn process_commit_failure(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CommitFailureInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::CommitFailure, accounts)?;
    let state_account = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let material_account = account(accounts, 2)?;
    let material_staging = account(accounts, 3)?;
    let rent_sysvar = account(accounts, 4)?;
    require_rent(rent_sysvar)?;
    let mut state = decode_resolution_state(program_id, state_account)?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        true,
    )?;
    let now = Clock::get().map_err(|_| AdapterError::AccountData)?;
    let decision = with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        state.material_id(),
        |material| {
            state
                .commit_failure_view(
                    state.material_id(),
                    material,
                    request.generation(),
                    now.unix_timestamp,
                    request.terminal_sequence(),
                )
                .map_err(|_| AdapterError::MarketTransition.into())
        },
    )?;
    if decision.outcome_count() != market.outcome_count {
        return Err(AdapterError::MarketTransition.into());
    }
    let market_bytes = settle_market(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        decision,
    )?;
    persist_exact(state_account, &state.to_bytes())?;
    persist_bytes(market_account, &market_bytes)
}

fn process_retire_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetireInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::RetireResolution, accounts)?;
    let state_account = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let rent_credit = account(accounts, 2)?;
    let clock_account = account(accounts, 3)?;
    require_clock(clock_account)?;
    let mut state = decode_resolution_state(program_id, state_account)?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        false,
    )?;
    authenticate_existing_rent_credit_without_sysvar(
        program_id,
        rent_credit,
        state.rent_beneficiary(),
    )?;
    let delta = state
        .retire(
            request.generation(),
            clock(clock_account)?.unix_timestamp,
            request.expected_market_child_count(),
            market.child_count,
        )
        .map_err(|_| AdapterError::MarketTransition)?;
    require_retire_delta(delta, market.child_count)?;
    let market_bytes = retire_market_child(
        program_id,
        market_account,
        request.generation(),
        state.material_id(),
        request.expected_market_child_count(),
    )?;
    persist_bytes(market_account, &market_bytes)?;
    close_to_rent_credit(state_account, rent_credit)
}

fn process_accept_shared(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: AcceptSharedObservationInstructionV1,
    payload: &[u8],
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::AcceptSharedObservation, accounts)?;
    let child_account = account(accounts, 0)?;
    let material_account = account(accounts, 1)?;
    let material_staging = account(accounts, 2)?;
    let rent_sysvar = account(accounts, 3)?;
    let clock_account = account(accounts, 4)?;
    require_rent_clock(rent_sysvar, clock_account)?;
    let child_data = child_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let child = decode_shared_state_view(program_id, child_account, &child_data)?;
    let seeds = child.pda_seeds().map_err(|_| AdapterError::AccountData)?;
    if u64::from_le_bytes(seeds.generation_le()) != request.generation() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let material_id = child.material_id().map_err(|_| AdapterError::AccountData)?;
    drop(child_data);
    let now = clock(clock_account)?;
    let extension_index = 5;
    with_authenticated_material(
        program_id,
        material_account,
        material_staging,
        rent_sysvar,
        material_id,
        |material| {
            let pyth = authenticate_pyth_view(
                material,
                seeds.source_spec_id(),
                accounts,
                extension_index,
                rent_sysvar,
                payload,
                &now,
                0,
            )?;
            let update = provider::post_and_load(
                &pyth.frame,
                pyth.facts,
                payload,
                now.slot,
                pyth.obligation.adapter_config().provider_feed_id(),
            )?;
            let evidence_id = SourceContentId::new(hash(payload).to_bytes())
                .map_err(|_| AdapterError::ContentIdentity)?;
            let observation_count = {
                let child_data = child_account
                    .try_borrow_data()
                    .map_err(|_| AdapterError::AccountData)?;
                let child = decode_shared_state_view(program_id, child_account, &child_data)?;
                child
                    .observation_count()
                    .map_err(|_| AdapterError::AccountData)?
            };
            let observation = normalize_update(
                pyth.obligation,
                evidence_id,
                material
                    .window()
                    .map_err(|_| AdapterError::MarketTransition)?
                    .schedule_id(),
                observation_count,
                update,
            )?;
            let mut completed = None;
            if let Some(caller_id) = request.completed_evidence_id() {
                let mut observations = {
                    let child_data = child_account
                        .try_borrow_data()
                        .map_err(|_| AdapterError::AccountData)?;
                    let child = decode_shared_state_view(program_id, child_account, &child_data)?;
                    collect_observations_view(child)?
                };
                observations
                    .try_reserve_exact(1)
                    .map_err(|_| AdapterError::Arithmetic)?;
                observations.push(observation);
                let count =
                    u16::try_from(observations.len()).map_err(|_| AdapterError::Arithmetic)?;
                let len = shared_evidence_set_preimage_len_v1(count)
                    .map_err(|_| AdapterError::MarketTransition)?;
                let mut preimage = Vec::new();
                preimage
                    .try_reserve_exact(len)
                    .map_err(|_| AdapterError::Arithmetic)?;
                preimage.resize(len, 0);
                encode_shared_evidence_set_preimage_v1(
                    material_id,
                    seeds.source_spec_id(),
                    material
                        .source(seeds.source_spec_id())
                        .map_err(|_| AdapterError::MarketTransition)?
                        .1,
                    seeds.window_spec_id(),
                    &observations,
                    &mut preimage,
                )
                .map_err(|_| AdapterError::MarketTransition)?;
                let derived = SourceContentId::new(hash(&preimage).to_bytes())
                    .map_err(|_| AdapterError::ContentIdentity)?;
                if derived != caller_id {
                    return Err(AdapterError::ContentIdentity.into());
                }
                completed = Some(derived);
            }
            {
                let mut child_data = child_account
                    .try_borrow_mut_data()
                    .map_err(|_| AdapterError::AccountData)?;
                accept_shared_provider_output_in_place_v1(
                    &mut child_data,
                    material_id,
                    material,
                    completed,
                    observation,
                    request.accepted_sequence(),
                    request.generation(),
                    now.unix_timestamp,
                )
                .map_err(|_| AdapterError::MarketTransition)?;
                let child = SharedObservationStateViewV1::decode(&child_data)
                    .map_err(|_| AdapterError::AccountData)?;
                if (child.phase().map_err(|_| AdapterError::AccountData)?
                    == SharedObservationPhaseV1::Accepted)
                    != completed.is_some()
                {
                    return Err(AdapterError::MarketTransition.into());
                }
            }
            provider::reclaim(&pyth.frame)
        },
    )
}

fn process_retire_shared(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetireInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(SourceFrameKindV1::RetireSharedObservation, accounts)?;
    let child_account = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let rent_credit = account(accounts, 2)?;
    let clock_account = account(accounts, 3)?;
    require_clock(clock_account)?;
    let child_data = child_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let child = decode_shared_state_view(program_id, child_account, &child_data)?;
    let seeds = child.pda_seeds().map_err(|_| AdapterError::AccountData)?;
    let material_id = child.material_id().map_err(|_| AdapterError::AccountData)?;
    let rent_beneficiary = child
        .rent_beneficiary()
        .map_err(|_| AdapterError::AccountData)?;
    drop(child_data);
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        material_id,
        false,
    )?;
    authenticate_existing_rent_credit_without_sysvar(program_id, rent_credit, rent_beneficiary)?;
    if market_account.key.to_bytes() != seeds.market() {
        return Err(AdapterError::AccountIdentity.into());
    }
    let market_bytes = retire_market_child(
        program_id,
        market_account,
        request.generation(),
        material_id,
        request.expected_market_child_count(),
    )?;
    let delta = {
        let mut child_data = child_account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        retire_shared_observation_in_place_v1(
            &mut child_data,
            request.generation(),
            clock(clock_account)?.unix_timestamp,
            request.expected_market_child_count(),
            market.child_count,
        )
        .map_err(|_| AdapterError::MarketTransition)?
    };
    require_retire_delta(delta, market.child_count)?;
    persist_bytes(market_account, &market_bytes)?;
    close_to_rent_credit(child_account, rent_credit)
}

#[derive(Clone, Copy)]
struct MarketFacts {
    outcome_count: u8,
    child_count: u64,
    capability_manifest_id: CoreContentId,
}

fn market_facts(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    require_open: bool,
) -> Result<MarketFacts, ProgramError> {
    if account.owner != program_id || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    match decode_market_outcome_count(&data).map_err(|_| AdapterError::AccountData)? {
        2 => market_facts_width::<2>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        3 => market_facts_width::<3>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        4 => market_facts_width::<4>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        5 => market_facts_width::<5>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        6 => market_facts_width::<6>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        7 => market_facts_width::<7>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        8 => market_facts_width::<8>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        9 => market_facts_width::<9>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        10 => market_facts_width::<10>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        11 => market_facts_width::<11>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        12 => market_facts_width::<12>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        13 => market_facts_width::<13>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        14 => market_facts_width::<14>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        15 => market_facts_width::<15>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        16 => market_facts_width::<16>(
            program_id,
            account.key,
            &data,
            generation,
            material_id,
            require_open,
        ),
        _ => Err(AdapterError::AccountData.into()),
    }
}

fn market_facts_width<const N: usize>(
    program_id: &Pubkey,
    key: &Pubkey,
    data: &[u8],
    generation: u64,
    material_id: SourceContentId,
    require_open: bool,
) -> Result<MarketFacts, ProgramError> {
    let market = CategoricalMarketV1::<N>::decode(data).map_err(|_| AdapterError::AccountData)?;
    let root = market.root();
    let identity = root.identity();
    let digest = hash(&identity.to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &digest], program_id);
    if key != &expected
        || identity.generation() != generation
        || identity.resolution_policy_id().to_bytes() != material_id.to_bytes()
        || (require_open && root.phase() != Phase::Open)
        || (!require_open && root.phase() == Phase::Retired)
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    Ok(MarketFacts {
        outcome_count: u8::try_from(N).map_err(|_| AdapterError::Arithmetic)?,
        child_count: root.outstanding_children(),
        capability_manifest_id: identity.capability_manifest_id(),
    })
}

fn register_market_child(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    expected_child_count: u64,
) -> Result<Vec<u8>, ProgramError> {
    market_mutation_dispatch(
        program_id,
        account,
        generation,
        material_id,
        MarketOperation::Register {
            expected_child_count,
        },
    )
}

fn retire_market_child(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    expected_child_count: u64,
) -> Result<Vec<u8>, ProgramError> {
    market_mutation_dispatch(
        program_id,
        account,
        generation,
        material_id,
        MarketOperation::Retire {
            expected_child_count,
        },
    )
}

fn settle_market(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    decision: SourceResolutionDecisionV1,
) -> Result<Vec<u8>, ProgramError> {
    let route = match decision.route() {
        SourceResolutionRouteV1::Primary => ResolutionKind::Occurrence,
        SourceResolutionRouteV1::Recovery => ResolutionKind::Recovery,
        SourceResolutionRouteV1::Failure => ResolutionKind::Failure,
    };
    let evidence = ProductContentId::new(decision.resolution_evidence_id().to_bytes())
        .map_err(|_| AdapterError::ContentIdentity)?;
    market_mutation_dispatch(
        program_id,
        account,
        generation,
        material_id,
        MarketOperation::Resolve {
            evidence,
            route,
            selector: decision.selector(),
            terminal_sequence: decision.terminal_sequence(),
        },
    )
}

#[derive(Clone, Copy)]
enum MarketOperation {
    Register {
        expected_child_count: u64,
    },
    Retire {
        expected_child_count: u64,
    },
    Resolve {
        evidence: ProductContentId,
        route: ResolutionKind,
        selector: u8,
        terminal_sequence: u64,
    },
}

fn market_mutation_dispatch(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    operation: MarketOperation,
) -> Result<Vec<u8>, ProgramError> {
    let facts = market_facts(program_id, account, generation, material_id, false)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    macro_rules! apply {
        ($n:literal) => {{
            let mut market =
                CategoricalMarketV1::<$n>::decode(&data).map_err(|_| AdapterError::AccountData)?;
            apply_market_operation(&mut market, generation, operation)?;
            encode_market(market)
        }};
    }
    match facts.outcome_count {
        2 => apply!(2),
        3 => apply!(3),
        4 => apply!(4),
        5 => apply!(5),
        6 => apply!(6),
        7 => apply!(7),
        8 => apply!(8),
        9 => apply!(9),
        10 => apply!(10),
        11 => apply!(11),
        12 => apply!(12),
        13 => apply!(13),
        14 => apply!(14),
        15 => apply!(15),
        16 => apply!(16),
        _ => Err(AdapterError::AccountData.into()),
    }
}

fn apply_market_operation<const N: usize>(
    market: &mut CategoricalMarketV1<N>,
    generation: u64,
    operation: MarketOperation,
) -> Result<(), ProgramError> {
    match operation {
        MarketOperation::Register {
            expected_child_count,
        } => market
            .register_child(generation, expected_child_count)
            .map_err(|_| AdapterError::MarketTransition.into()),
        MarketOperation::Retire {
            expected_child_count,
        } => market
            .retire_child(generation, expected_child_count)
            .map_err(|_| AdapterError::MarketTransition.into()),
        MarketOperation::Resolve {
            evidence,
            route,
            selector,
            terminal_sequence,
        } => {
            let settlement = CategoricalSettlementSummaryV1::resolved::<N>(
                evidence,
                route,
                usize::from(selector),
                terminal_sequence,
            )
            .map_err(|_| AdapterError::MarketTransition)?;
            market
                .resolve_with_summary(generation, settlement)
                .map_err(|_| AdapterError::MarketTransition.into())
        }
    }
}

fn encode_market<const N: usize>(market: CategoricalMarketV1<N>) -> Result<Vec<u8>, ProgramError> {
    let len = CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| AdapterError::Arithmetic)?;
    output.resize(len, 0);
    market
        .encode(&mut output)
        .map_err(|_| AdapterError::MarketTransition)?;
    Ok(output)
}

struct FundingAuthentication {
    allocation_id: SourceContentId,
    next_state: Option<FundingStateV1>,
}

#[allow(clippy::too_many_arguments)]
fn authenticate_recovery_funding(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    manifest_id: CoreContentId,
    _material_id: SourceContentId,
    release_amount: Option<u64>,
    resolver: Option<&AccountInfo<'_>>,
) -> Result<FundingAuthentication, ProgramError> {
    require_rent(rent_sysvar)?;
    if manifest_account.owner != program_id
        || manifest_account.executable
        || funding_account.owner != program_id
        || funding_account.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if hash(&manifest_data).to_bytes() != manifest_id.to_bytes() {
        return Err(AdapterError::ContentIdentity.into());
    }
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| AdapterError::AccountData)?;
    if manifest.as_bytes() != &manifest_data[..] {
        return Err(AdapterError::AccountData.into());
    }
    let key = dclutch_record_contract::RecordKeyV1::new(
        dclutch_record_contract::SchemaReleaseId::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1)
            .map_err(|_| AdapterError::AccountData)?,
        dclutch_record_contract::ContentDigest::new(manifest_id.to_bytes())
            .map_err(|_| AdapterError::AccountData)?,
    );
    if derive_record_pda(program_id, key, false).0 != *manifest_account.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let mut funding =
        FundingStateV1::decode(&funding_data).map_err(|_| AdapterError::AccountData)?;
    if funding.to_bytes().as_slice() != &funding_data[..] {
        return Err(AdapterError::AccountData.into());
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let required_rent = rent.minimum_balance(funding_data.len());
    let custody =
        FundingCustodyObservationV1::native_only(funding_account.lamports(), required_rent)
            .map_err(|_| AdapterError::FundUnderfunded)?;
    funding
        .validate_against(manifest_id, manifest, custody)
        .map_err(|_| AdapterError::FundUnderfunded)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market_account.key.to_bytes(),
        market_facts_generation(market_account)?,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| AdapterError::AccountIdentity)?;
    let (expected, _) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    if funding_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let entry = manifest
        .entry(funding.entry_index())
        .map_err(|_| AdapterError::FundUnderfunded)?;
    let provider_allocation = entry.funding_quote().amounts().provider();
    if provider_allocation.asset_class() != FundingAssetClassV1::NativeLamports
        || provider_allocation.amount() == 0
        || funding.remaining().provider().asset_class() != FundingAssetClassV1::NativeLamports
        || funding.remaining().provider().amount() == 0
    {
        return Err(AdapterError::FundUnderfunded.into());
    }
    let allocation_id = SourceContentId::new(entry.config_id().to_bytes())
        .map_err(|_| AdapterError::ContentIdentity)?;
    let next_state = if let Some(amount) = release_amount {
        let resolver = resolver.ok_or(AdapterError::AccountFrameLength)?;
        let plan = funding
            .release(
                manifest_id,
                manifest,
                custody,
                FundingCompartment::Provider,
                amount,
            )
            .map_err(|_| AdapterError::FundUnderfunded)?;
        if plan.asset_class() != FundingAssetClassV1::NativeLamports || plan.amount() != amount {
            return Err(AdapterError::FundUnderfunded.into());
        }
        transfer_program_lamports(funding_account, resolver, amount)?;
        let post_custody =
            FundingCustodyObservationV1::native_only(funding_account.lamports(), required_rent)
                .map_err(|_| AdapterError::FundUnderfunded)?;
        funding
            .validate_against(manifest_id, manifest, post_custody)
            .map_err(|_| AdapterError::FundUnderfunded)?;
        Some(funding)
    } else {
        None
    };
    Ok(FundingAuthentication {
        allocation_id,
        next_state,
    })
}

fn market_facts_generation(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let outcomes = decode_market_outcome_count(&data).map_err(|_| AdapterError::AccountData)?;
    macro_rules! generation {
        ($n:literal) => {
            CategoricalMarketV1::<$n>::decode(&data)
                .map(|market| market.root().identity().generation())
                .map_err(|_| AdapterError::AccountData.into())
        };
    }
    match outcomes {
        2 => generation!(2),
        3 => generation!(3),
        4 => generation!(4),
        5 => generation!(5),
        6 => generation!(6),
        7 => generation!(7),
        8 => generation!(8),
        9 => generation!(9),
        10 => generation!(10),
        11 => generation!(11),
        12 => generation!(12),
        13 => generation!(13),
        14 => generation!(14),
        15 => generation!(15),
        16 => generation!(16),
        _ => Err(AdapterError::AccountData.into()),
    }
}

fn transfer_program_lamports(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    let source_before = source.lamports();
    let destination_before = destination.lamports();
    let source_after = source_before
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut destination_lamports = destination
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **source_lamports = source_after;
        **destination_lamports = destination_after;
    }
    if source.lamports() != source_after || destination.lamports() != destination_after {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

struct PythExecution<'a, 'info> {
    frame: PriceFrame<'a, 'info>,
    facts: ProviderFacts,
    obligation: PythProviderAdapterObligationV1,
}

fn provider_fee_view(
    material: SourceMaterialViewV1<'_>,
    source_id: SourceContentId,
    accounts: &[AccountInfo<'_>],
    extension_index: usize,
    rent_sysvar: &AccountInfo<'_>,
    payload: &[u8],
    clock: &Clock,
) -> Result<u64, ProgramError> {
    authenticate_pyth_static_view(
        material,
        source_id,
        accounts,
        extension_index,
        rent_sysvar,
        payload,
        clock,
    )
    .map(|facts| facts.0.fee)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_pyth_view<'a, 'info>(
    material: SourceMaterialViewV1<'_>,
    source_id: SourceContentId,
    accounts: &'a [AccountInfo<'info>],
    extension_index: usize,
    rent_sysvar: &'a AccountInfo<'info>,
    payload: &[u8],
    clock: &Clock,
    anticipated_credit: u64,
) -> Result<PythExecution<'a, 'info>, ProgramError> {
    let (facts, obligation, release) = authenticate_pyth_static_view(
        material,
        source_id,
        accounts,
        extension_index,
        rent_sysvar,
        payload,
        clock,
    )?;
    let resolver = account(accounts, extension_index)?;
    let available = resolver
        .lamports()
        .checked_add(anticipated_credit)
        .ok_or(AdapterError::Arithmetic)?;
    if available
        < facts
            .update_rent
            .checked_add(facts.fee)
            .ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::FundUnderfunded.into());
    }
    let frame = pyth_price_frame(accounts, extension_index, rent_sysvar)?;
    let _ = release;
    Ok(PythExecution {
        frame,
        facts,
        obligation,
    })
}

fn authenticate_pyth_static_view(
    material: SourceMaterialViewV1<'_>,
    source_id: SourceContentId,
    accounts: &[AccountInfo<'_>],
    extension_index: usize,
    rent_sysvar: &AccountInfo<'_>,
    payload: &[u8],
    clock: &Clock,
) -> Result<
    (
        ProviderFacts,
        PythProviderAdapterObligationV1,
        PythReleaseV1,
    ),
    ProgramError,
> {
    let obligation = PythProviderAdapterObligationV1::from_material_view(material, source_id)
        .map_err(|_| AdapterError::ReleaseUnavailable)?;
    let (facts, release) = authenticate_pyth_obligation(
        obligation,
        accounts,
        extension_index,
        rent_sysvar,
        payload,
        clock,
    )?;
    Ok((facts, obligation, release))
}

fn authenticate_pyth_obligation(
    obligation: PythProviderAdapterObligationV1,
    accounts: &[AccountInfo<'_>],
    extension_index: usize,
    rent_sysvar: &AccountInfo<'_>,
    payload: &[u8],
    clock: &Clock,
) -> Result<(ProviderFacts, PythReleaseV1), ProgramError> {
    let resolver = account(accounts, extension_index)?;
    let update = account(accounts, extension_index + 1)?;
    let receiver = account(accounts, extension_index + 2)?;
    let receiver_data = account(accounts, extension_index + 3)?;
    let config_account = account(accounts, extension_index + 4)?;
    let encoded_vaa = account(accounts, extension_index + 5)?;
    let router = account(accounts, extension_index + 6)?;
    let router_data = account(accounts, extension_index + 7)?;
    let treasury = account(accounts, extension_index + 8)?;
    let system = account(accounts, extension_index + 9)?;
    require_system(system)?;
    require_rent(rent_sysvar)?;
    PostUpdateParamsView::parse(payload).map_err(|_| AdapterError::ProviderAuthentication)?;
    let selected = obligation.provider_release();
    let release = selected_release(
        selected.provider_deployment_release_id().to_bytes(),
        clock.unix_timestamp,
    )?;
    if selected.decoding_rules_id().to_bytes() != release.price_update_codec_id()
        || selected.transport_profile_id().to_bytes() != release.adapter_id()
    {
        return Err(AdapterError::ReleaseUnavailable.into());
    }
    let receiver_key = Pubkey::new_from_array(release.receiver_program());
    let router_key = Pubkey::new_from_array(release.router_program());
    let (canonical_config, _) =
        Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver_key);
    if receiver.key != &receiver_key
        || router.key != &router_key
        || receiver_data.key.to_bytes() != release.receiver_programdata()
        || router_data.key.to_bytes() != release.router_programdata()
        || release.receiver_config() != canonical_config.to_bytes()
        || config_account.key != &canonical_config
        || config_account.owner != &receiver_key
        || encoded_vaa.owner != &router_key
        || receiver.owner != &UPGRADEABLE_LOADER
        || receiver_data.owner != &UPGRADEABLE_LOADER
        || router.owner != &UPGRADEABLE_LOADER
        || router_data.owner != &UPGRADEABLE_LOADER
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    authenticate_loader_link(
        receiver,
        receiver_data,
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_loader_link(
        router,
        router_data,
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let config_data = config_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let config =
        ReceiverConfigV2View::parse(&config_data).map_err(|_| AdapterError::AccountData)?;
    if hash(&config_data).to_bytes() != release.config_digest()
        || config.router_program() != release.router_program()
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let params =
        PostUpdateParamsView::parse(payload).map_err(|_| AdapterError::ProviderAuthentication)?;
    let treasury_id = [params.treasury_id()];
    let (expected_treasury, _) = Pubkey::find_program_address(
        &[RECEIVER_TREASURY_SEED, treasury_id.as_slice()],
        &receiver_key,
    );
    if treasury.key != &expected_treasury
        || update.owner != &SYSTEM_PROGRAM
        || update.lamports() != 0
        || !update
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
        || resolver.owner != &SYSTEM_PROGRAM
        || !resolver
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    Ok((
        ProviderFacts {
            update_rent: rent.minimum_balance(dclutch_pyth_svm::FULL_PRICE_UPDATE_V2_LEN),
            fee: config.fee(),
        },
        release,
    ))
}

fn authenticate_loader_link(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    expected_programdata: [u8; 32],
    expected_slot: u64,
) -> Result<(), ProgramError> {
    let program_data = program
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let view = ProgramV3View::parse(&program_data).map_err(|_| AdapterError::AccountData)?;
    let (derived, _) = Pubkey::find_program_address(&[program.key.as_ref()], &UPGRADEABLE_LOADER);
    if view.programdata_key() != expected_programdata
        || programdata.key.to_bytes() != expected_programdata
        || programdata.key != &derived
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let programdata_data = programdata
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let view =
        ProgramDataV3View::parse(&programdata_data).map_err(|_| AdapterError::AccountData)?;
    if view.deployment_slot() != expected_slot {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    Ok(())
}

fn pyth_price_frame<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    extension_index: usize,
    rent_sysvar: &'a AccountInfo<'info>,
) -> Result<PriceFrame<'a, 'info>, ProgramError> {
    let state = account(accounts, 0)?;
    let material = account(accounts, 1)?;
    Ok(PriceFrame {
        resolver: account(accounts, extension_index)?,
        update: account(accounts, extension_index + 1)?,
        market: state,
        fund: state,
        material,
        manifest: material,
        rent_credit: state,
        receiver: account(accounts, extension_index + 2)?,
        receiver_programdata: account(accounts, extension_index + 3)?,
        config: account(accounts, extension_index + 4)?,
        encoded_vaa: account(accounts, extension_index + 5)?,
        router: account(accounts, extension_index + 6)?,
        router_programdata: account(accounts, extension_index + 7)?,
        treasury: account(accounts, extension_index + 8)?,
        material_staging_cursor: state,
        manifest_staging_cursor: state,
        system: account(accounts, extension_index + 9)?,
        rent_sysvar,
    })
}

fn normalize_update(
    obligation: PythProviderAdapterObligationV1,
    evidence_id: SourceContentId,
    schedule_id: SourceContentId,
    schedule_index: u16,
    update: FullPriceUpdateV2,
) -> Result<NormalizedProviderEvidenceV1, ProgramError> {
    obligation
        .normalize_authenticated_update(
            evidence_id,
            schedule_id,
            schedule_index,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
        )
        .map_err(|_| AdapterError::ProviderAuthentication.into())
}

fn active_source_view(
    state: SourceResolutionStateV1,
    material: SourceMaterialViewV1<'_>,
) -> Result<(SourceContentId, dclutch_source_contract::SourceSpecV1), ProgramError> {
    match state.phase() {
        SourceResolutionPhaseV1::Primary => material
            .primary_source()
            .map_err(|_| AdapterError::ContentIdentity.into()),
        SourceResolutionPhaseV1::Recovery => {
            let slot = material
                .recovery_slot(
                    state
                        .active_recovery_attempt()
                        .ok_or(AdapterError::ReplayMismatch)?,
                )
                .map_err(|_| AdapterError::ContentIdentity)?;
            Ok((slot.source_spec_id(), slot.source()))
        }
        _ => Err(AdapterError::ReplayMismatch.into()),
    }
}

fn collect_observations_view(
    child: SharedObservationStateViewV1<'_>,
) -> Result<Vec<NormalizedProviderEvidenceV1>, ProgramError> {
    let observation_count = child
        .observation_count()
        .map_err(|_| AdapterError::AccountData)?;
    let count = usize::from(observation_count);
    let mut observations = Vec::new();
    observations
        .try_reserve_exact(count)
        .map_err(|_| AdapterError::Arithmetic)?;
    let mut index = 0u16;
    while index < observation_count {
        observations.push(
            child
                .observation(index)
                .map_err(|_| AdapterError::AccountData)?,
        );
        index = index.checked_add(1).ok_or(AdapterError::Arithmetic)?;
    }
    Ok(observations)
}

fn decode_shared_state_view<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    data: &'a [u8],
) -> Result<SharedObservationStateViewV1<'a>, ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != SHARED_OBSERVATION_STATE_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let state =
        SharedObservationStateViewV1::decode(data).map_err(|_| AdapterError::AccountData)?;
    let seeds = state.pda_seeds().map_err(|_| AdapterError::AccountData)?;
    let market = seeds.market();
    let generation = seeds.generation_le();
    let source = seeds.source_spec_id().to_bytes();
    let window = seeds.window_spec_id().to_bytes();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            source.as_slice(),
            window.as_slice(),
            bump.as_slice(),
        ],
        program_id,
    )
    .map_err(|_| AdapterError::AccountIdentity)?;
    if account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(state)
}

fn with_authenticated_material<'a, T>(
    program_id: &Pubkey,
    raw: &AccountInfo<'a>,
    staging: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    expected_id: SourceContentId,
    apply: impl FnOnce(SourceMaterialViewV1<'_>) -> Result<T, ProgramError>,
) -> Result<T, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        raw,
        staging,
        rent_sysvar,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        expected_id.to_bytes(),
        |record| {
            let material = SourceMaterialViewV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData)?;
            apply(material)
        },
    )
}

fn decode_resolution_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Result<SourceResolutionStateV1, ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != SOURCE_RESOLUTION_STATE_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let state = SourceResolutionStateV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    if state.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::AccountData.into());
    }
    let seeds = state.pda_seeds();
    let market = seeds.market();
    let generation = seeds.generation_le();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            bump.as_slice(),
        ],
        program_id,
    )
    .map_err(|_| AdapterError::AccountIdentity)?;
    if account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(state)
}

fn validate_frame(
    kind: SourceFrameKindV1,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut privileges = Vec::new();
    privileges
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for account in accounts {
        privileges.push(SourceAccountPrivilegeV1 {
            key: account.key.to_bytes(),
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            is_executable: account.executable,
        });
    }
    validate_source_frame_v1(kind, &privileges).map_err(|_| AdapterError::AccountPrivilege.into())
}

fn require_register_delta(
    delta: dclutch_source_contract::MarketChildDeltaV1,
    before: u64,
) -> Result<(), ProgramError> {
    if delta.kind() != MarketChildDeltaKindV1::Register
        || delta.before() != before
        || delta.after() != before.checked_add(1).ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::MarketTransition.into());
    }
    Ok(())
}

fn require_retire_delta(
    delta: dclutch_source_contract::MarketChildDeltaV1,
    before: u64,
) -> Result<(), ProgramError> {
    if delta.kind() != MarketChildDeltaKindV1::Retire
        || delta.before() != before
        || delta.after() != before.checked_sub(1).ok_or(AdapterError::Arithmetic)?
    {
        return Err(AdapterError::MarketTransition.into());
    }
    Ok(())
}

fn authenticate_existing_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    beneficiary: [u8; 32],
) -> Result<(), ProgramError> {
    let authority =
        RefundAuthority::new(beneficiary).map_err(|_| AdapterError::RentCreditAuthentication)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    authenticate_rent_credit(
        program_id,
        account,
        authority,
        Some(rent.minimum_balance(RENT_CREDIT_BYTES_V1)),
    )?;
    Ok(())
}

fn authenticate_existing_rent_credit_without_sysvar(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    beneficiary: [u8; 32],
) -> Result<(), ProgramError> {
    let authority =
        RefundAuthority::new(beneficiary).map_err(|_| AdapterError::RentCreditAuthentication)?;
    authenticate_rent_credit(program_id, account, authority, None)?;
    Ok(())
}

fn create_prefunded_pda<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    minimum_balance: u64,
    space: usize,
    owner: &Pubkey,
    signer: &[&[u8]],
) -> Result<(), ProgramError> {
    if payer.owner != &system_program::ID
        || created.owner != &system_program::ID
        || created.executable
        || !created
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let before = created.lamports();
    let top_up = minimum_balance.saturating_sub(before);
    let space_u64 = u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?;
    if before == 0 {
        invoke_signed(
            &create_account(payer.key, created.key, minimum_balance, space_u64, owner),
            &[payer.clone(), created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| AdapterError::MarketCreateCpi)?;
    } else {
        if top_up != 0 {
            invoke(
                &transfer(payer.key, created.key, top_up),
                &[payer.clone(), created.clone(), system.clone()],
            )
            .map_err(|_| AdapterError::MarketCreateCpi)?;
        }
        invoke_signed(
            &allocate(created.key, space_u64),
            &[created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| AdapterError::MarketCreateCpi)?;
        invoke_signed(
            &assign(created.key, owner),
            &[created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| AdapterError::MarketCreateCpi)?;
    }
    if created.owner != owner
        || created.data_len() != space
        || created.lamports() != before.checked_add(top_up).ok_or(AdapterError::Arithmetic)?
        || created.lamports() < minimum_balance
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

fn persist_exact<const N: usize>(
    account: &AccountInfo<'_>,
    bytes: &[u8; N],
) -> Result<(), ProgramError> {
    persist_bytes(account, bytes)
}

fn persist_bytes(account: &AccountInfo<'_>, bytes: &[u8]) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    if data.len() != bytes.len() {
        return Err(AdapterError::AccountData.into());
    }
    data.copy_from_slice(bytes);
    if &data[..] != bytes {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

fn close_to_rent_credit(
    source: &AccountInfo<'_>,
    credit: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let source_balance = source.lamports();
    let credit_after = credit
        .lamports()
        .checked_add(source_balance)
        .ok_or(AdapterError::Arithmetic)?;
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut credit_lamports = credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **source_lamports = 0;
        **credit_lamports = credit_after;
    }
    source.resize(0).map_err(|_| AdapterError::PositionClose)?;
    source.assign(&system_program::ID);
    if source.lamports() != 0
        || source.owner != &system_program::ID
        || !source
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
        || credit.lamports() != credit_after
    {
        return Err(AdapterError::PositionClose.into());
    }
    Ok(())
}

fn clock(account: &AccountInfo<'_>) -> Result<Clock, ProgramError> {
    require_clock(account)?;
    Clock::from_account_info(account).map_err(|_| AdapterError::AccountData.into())
}

fn require_fixed_accounts(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    clock: Option<&AccountInfo<'_>>,
) -> Result<(), ProgramError> {
    require_system(system)?;
    require_rent(rent)?;
    if let Some(clock) = clock {
        require_clock(clock)?;
    }
    Ok(())
}

fn require_rent_clock(rent: &AccountInfo<'_>, clock: &AccountInfo<'_>) -> Result<(), ProgramError> {
    require_rent(rent)?;
    require_clock(clock)
}

fn require_system(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &system_program::ID
        || account.owner != &native_loader::ID
        || !account.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_rent(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID || account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_clock(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID || account.executable {
        return Err(AdapterError::AccountIdentity.into());
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
    use super::*;
    use std::{boxed::Box, vec, vec::Vec};

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

    #[test]
    fn source_adapter_enforces_exact_ordered_privileges_and_count() {
        let owner = Pubkey::new_unique();
        let accounts = [
            test_account(Pubkey::new_unique(), false, true, 1, vec![], owner, false),
            test_account(Pubkey::new_unique(), false, true, 1, vec![], owner, false),
            test_account(Pubkey::new_unique(), false, false, 1, vec![], owner, false),
            test_account(Pubkey::new_unique(), false, false, 1, vec![], owner, false),
            test_account(sysvar::rent::ID, false, false, 1, vec![], sysvar::ID, false),
        ];
        assert_eq!(
            validate_frame(SourceFrameKindV1::CommitFailure, &accounts),
            Ok(())
        );
        assert_eq!(
            validate_frame(SourceFrameKindV1::CommitFailure, &accounts[..4]),
            Err(AdapterError::AccountPrivilege.into())
        );
        let mut hostile = accounts.clone();
        hostile[2].is_writable = true;
        assert_eq!(
            validate_frame(SourceFrameKindV1::CommitFailure, &hostile),
            Err(AdapterError::AccountPrivilege.into())
        );
    }

    #[test]
    fn persisted_resolution_state_must_rederive_its_exact_pda() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let generation = 7u64;
        let generation_bytes = generation.to_le_bytes();
        let (state_key, bump) = Pubkey::find_program_address(
            &[
                SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
                market.as_ref(),
                generation_bytes.as_slice(),
            ],
            &program_id,
        );
        let state = SourceResolutionStateV1::fresh(
            market.to_bytes(),
            generation,
            SourceContentId::new([9; 32]).expect("material"),
            [10; 32],
            bump,
            0,
            0,
        )
        .expect("creation")
        .state();
        let account = test_account(
            state_key,
            false,
            true,
            1,
            state.to_bytes().to_vec(),
            program_id,
            false,
        );
        assert_eq!(decode_resolution_state(&program_id, &account), Ok(state));

        let wrong_key = test_account(
            Pubkey::new_unique(),
            false,
            true,
            1,
            state.to_bytes().to_vec(),
            program_id,
            false,
        );
        assert_eq!(
            decode_resolution_state(&program_id, &wrong_key),
            Err(AdapterError::AccountIdentity.into())
        );
        let trailing = test_account(
            state_key,
            false,
            true,
            1,
            vec![0; SOURCE_RESOLUTION_STATE_BYTES + 1],
            program_id,
            false,
        );
        assert_eq!(
            decode_resolution_state(&program_id, &trailing),
            Err(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn child_delta_checks_refuse_skipping_or_double_accounting() {
        let creation = SourceResolutionStateV1::fresh(
            [1; 32],
            7,
            SourceContentId::new([2; 32]).expect("material"),
            [3; 32],
            1,
            4,
            4,
        )
        .expect("creation");
        assert_eq!(require_register_delta(creation.market_delta(), 4), Ok(()));
        assert_eq!(
            require_register_delta(creation.market_delta(), 5),
            Err(AdapterError::MarketTransition.into())
        );
    }
}
