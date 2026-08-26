//! Funded ordered-recovery and explicit-failure controller specialization.

use core::convert::TryFrom;

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAssetClassV1, FundingCompartment,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_product_contract::product::PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1;
use dclutch_resolution_codec::{
    FUNDED_POSTSTATE_DIGEST_DOMAIN_V1, FUNDED_TRANSITION_REQUEST_BYTES, FundedReceiptPostPhaseV1,
    FundedTerminalRefundPhaseV1, FundedTransitionActionV3, FundedTransitionReceiptV1,
    FundedTransitionRequestV3, RESOLUTION_CERTIFICATE_BYTES, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
    ResolutionCertificateKindV1, ResolutionCertificateV1,
};
use dclutch_source_contract::{
    ContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_BYTES,
    SourceMaterialViewV1, SourceResolutionPhaseV1, SourceResolutionStateV1,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;

use crate::{
    MarketAuthority, RecordKind, ResolutionError, authenticate_clock,
    authenticate_finalized_record, authenticate_market_and_resolution_release,
    authenticate_material_components, authenticate_product_instance, authenticate_rent,
    authenticate_state_account, initialize_certificate_output,
};

/// Exact funded-transition account count.
pub(crate) const FUNDED_TRANSITION_ACCOUNT_COUNT: usize = 17;

struct SourceTransitionPlan {
    next_state: SourceResolutionStateV1,
    kind: ResolutionCertificateKindV1,
    route: [u8; 32],
    attempt_index: u32,
    schedule_index: u32,
    selector: u32,
    observed_at: u64,
    certificate_sequence: u64,
}

/// Execute one bounded funded liveness transition.
#[inline(never)]
pub(crate) fn process_funded_transition(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != FUNDED_TRANSITION_REQUEST_BYTES
        || accounts.len() != FUNDED_TRANSITION_ACCOUNT_COUNT
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let request = FundedTransitionRequestV3::decode(instruction_data)
        .map_err(|_| ResolutionError::Instruction)?;
    validate_funded_frame(accounts, program_id)?;

    let mut iterator = accounts.iter();
    let source_state = next(&mut iterator)?;
    let certificate = next(&mut iterator)?;
    let funding_state = next(&mut iterator)?;
    let worker = next(&mut iterator)?;
    let market = next(&mut iterator)?;
    let activated_release_set = next(&mut iterator)?;
    let resolution_program = next(&mut iterator)?;
    let resolution_programdata = next(&mut iterator)?;
    let source_material = next(&mut iterator)?;
    let source_material_staging = next(&mut iterator)?;
    let product_instance = next(&mut iterator)?;
    let product_instance_staging = next(&mut iterator)?;
    let capability_manifest = next(&mut iterator)?;
    let capability_manifest_staging = next(&mut iterator)?;
    let clock_sysvar = next(&mut iterator)?;
    let rent_sysvar = next(&mut iterator)?;
    let system = next(&mut iterator)?;

    let clock = authenticate_clock(clock_sysvar)?;
    let rent = authenticate_rent(rent_sysvar)?;
    if clock.slot == 0 || clock.unix_timestamp <= 0 {
        return Err(ResolutionError::Sysvar.into());
    }

    let source_state_data = source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let prior_state = SourceResolutionStateV1::decode(&source_state_data)
        .map_err(|_| ResolutionError::OutputState)?;
    let pre_source_digest = hash(&source_state_data).to_bytes();
    authenticate_state_account(program_id, source_state, prior_state)?;
    let authority = authenticate_market_and_resolution_release(
        program_id,
        market,
        prior_state,
        request.expected_generation,
        activated_release_set,
        resolution_program,
        resolution_programdata,
        &rent,
    )?;

    let material_data = source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    let material_id = prior_state.material_id();
    authenticate_finalized_record(
        authority.registry_program,
        source_material,
        source_material_staging,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        material_id.to_bytes(),
        &material_data,
        RecordKind::SourceMaterial,
    )?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    authenticate_material_components(material, authority.product_instance_id)?;

    let product_data = product_instance
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        authority.registry_program,
        product_instance,
        product_instance_staging,
        &rent,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        authority.product_instance_id,
        &product_data,
        RecordKind::ProductInstance,
    )?;
    let outcome_count = authenticate_product_instance(
        material,
        authority,
        request.expected_result_domain_id,
        &product_data,
    )?;

    let mut source_plan = plan_source_transition(
        prior_state,
        material_id,
        material,
        request,
        clock.unix_timestamp,
    )?;
    if matches!(request.action, FundedTransitionActionV3::CommitFailure) {
        let sequence = u64::from(source_plan.attempt_index)
            .checked_add(1)
            .ok_or(ResolutionError::Arithmetic)?;
        let decision = source_plan
            .next_state
            .commit_failure_view(
                material_id,
                material,
                request.expected_generation,
                clock.unix_timestamp,
                sequence,
            )
            .map_err(|_| ResolutionError::Transition)?;
        if decision.outcome_count() != outcome_count
            || u32::from(decision.selector()) != source_plan.selector
            || decision.selector()
                != material
                    .result_domain()
                    .map_err(|_| ResolutionError::ProductDomain)?
                    .failure_selector()
        {
            return Err(ResolutionError::Transition.into());
        }
    }

    let (next_funding, work_paid, funding_remaining) = plan_funding_release(
        program_id,
        funding_state,
        market,
        capability_manifest,
        capability_manifest_staging,
        &rent,
        authority,
        material_id,
        request,
    )?;

    let certificate_value = ResolutionCertificateV1 {
        kind: source_plan.kind,
        market: market.key.to_bytes(),
        route: source_plan.route,
        source_material: material_id.to_bytes(),
        product: material
            .product_instance_id()
            .map_err(|_| ResolutionError::SourceMaterial)?
            .to_bytes(),
        provider_evidence: [0; 32],
        funding_allocation: request.expected_funding_allocation_id,
        receipt_account: certificate.key.to_bytes(),
        generation: request.expected_generation,
        attempt_index: source_plan.attempt_index,
        schedule_index: source_plan.schedule_index,
        selector: source_plan.selector,
        work_paid,
        funding_remaining,
        result_numerator: 0,
        result_denominator: 0,
        observed_at: source_plan.observed_at,
    };
    let next_state_bytes = source_plan.next_state.to_bytes();
    let next_funding_bytes = next_funding.to_bytes();
    let certificate_bytes = certificate_value
        .to_bytes()
        .map_err(|_| ResolutionError::OutputState)?;
    let funding_lamports_after = funding_state
        .lamports()
        .checked_sub(work_paid)
        .ok_or(ResolutionError::Arithmetic)?;
    let worker_lamports_after = worker
        .lamports()
        .checked_add(work_paid)
        .ok_or(ResolutionError::Arithmetic)?;
    let exact_funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let manifest_data = capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let post_custody =
        FundingCustodyObservationV1::native_only(funding_lamports_after, exact_funding_rent)
            .map_err(|_| ResolutionError::Funding)?;
    next_funding
        .validate_against(
            CapabilityContentId::new(authority.semantic_capability_manifest_id)
                .map_err(|_| ResolutionError::Funding)?,
            manifest,
            post_custody,
        )
        .map_err(|_| ResolutionError::Funding)?;

    drop(manifest_data);
    drop(product_data);
    drop(material_data);
    drop(source_state_data);
    commit_funded_transition_and_emit_receipt(
        program_id,
        instruction_data,
        source_state,
        funding_state,
        worker,
        certificate,
        pre_source_digest,
        &next_state_bytes,
        &certificate_bytes,
        &next_funding_bytes,
        request,
        &source_plan,
        work_paid,
        funding_remaining,
        funding_lamports_after,
        worker_lamports_after,
        system,
        &rent,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn commit_funded_transition_and_emit_receipt<'info>(
    program_id: &Pubkey,
    instruction_data: &[u8],
    source_state: &AccountInfo<'info>,
    funding_state: &AccountInfo<'info>,
    worker: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    pre_source_digest: [u8; 32],
    next_state: &[u8; SOURCE_RESOLUTION_STATE_BYTES],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES],
    next_funding: &[u8; FUNDING_STATE_BYTES],
    request: FundedTransitionRequestV3,
    source_plan: &SourceTransitionPlan,
    work_paid: u64,
    funding_remaining: u64,
    funding_lamports_after: u64,
    worker_lamports_after: u64,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let funding_lamports_after_bytes = funding_lamports_after.to_le_bytes();
    let receipt = FundedTransitionReceiptV1 {
        action: request.action,
        post_phase: receipt_post_phase(request.action, source_plan.next_state.phase())?,
        certificate_kind: source_plan.kind,
        terminal_refund_phase: receipt_terminal_refund_phase(request.action),
        producer_program: program_id.to_bytes(),
        producer_release: RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        request_digest: hash(instruction_data).to_bytes(),
        source_state: source_state.key.to_bytes(),
        funding_state: funding_state.key.to_bytes(),
        worker: worker.key.to_bytes(),
        certificate: certificate.key.to_bytes(),
        pre_source_digest,
        post_source_digest: hash(next_state).to_bytes(),
        funding_post_digest: hashv(&[
            FUNDED_POSTSTATE_DIGEST_DOMAIN_V1,
            next_funding,
            &funding_lamports_after_bytes,
        ])
        .to_bytes(),
        generation: request.expected_generation,
        replay_sequence: source_plan.certificate_sequence,
        work_paid,
        funding_remaining,
        selector: source_plan.selector,
    }
    .to_bytes()
    .map_err(|_| ResolutionError::OutputState)?;
    commit_funded_outputs(
        program_id,
        source_state,
        certificate,
        funding_state,
        worker,
        next_state,
        next_certificate,
        next_funding,
        source_plan.kind,
        source_plan.certificate_sequence,
        funding_lamports_after,
        worker_lamports_after,
        system,
        rent,
    )?;
    set_return_data(&receipt);
    Ok(())
}

fn receipt_post_phase(
    action: FundedTransitionActionV3,
    post_phase: SourceResolutionPhaseV1,
) -> Result<FundedReceiptPostPhaseV1, ProgramError> {
    match (action, post_phase) {
        (FundedTransitionActionV3::FailNext, SourceResolutionPhaseV1::Recovery) => {
            Ok(FundedReceiptPostPhaseV1::Recovery)
        }
        (FundedTransitionActionV3::Exhaust, SourceResolutionPhaseV1::Exhausted) => {
            Ok(FundedReceiptPostPhaseV1::Exhausted)
        }
        (FundedTransitionActionV3::CommitFailure, SourceResolutionPhaseV1::FailureCommitted) => {
            Ok(FundedReceiptPostPhaseV1::FailureCommitted)
        }
        _ => Err(ResolutionError::Transition.into()),
    }
}

const fn receipt_terminal_refund_phase(
    action: FundedTransitionActionV3,
) -> FundedTerminalRefundPhaseV1 {
    match action {
        FundedTransitionActionV3::FailNext => FundedTerminalRefundPhaseV1::Continuing,
        FundedTransitionActionV3::Exhaust => FundedTerminalRefundPhaseV1::AwaitingFailure,
        FundedTransitionActionV3::CommitFailure => {
            FundedTerminalRefundPhaseV1::TerminalRefundPending
        }
    }
}

fn plan_source_transition(
    prior: SourceResolutionStateV1,
    material_id: ContentId,
    material: SourceMaterialViewV1<'_>,
    request: FundedTransitionRequestV3,
    now: i64,
) -> Result<SourceTransitionPlan, ProgramError> {
    let recovery = material
        .recovery_policy()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    match request.action {
        FundedTransitionActionV3::FailNext => {
            let mut next = prior;
            next.fail_next_view(
                material_id,
                material,
                ContentId::new(request.expected_funding_allocation_id)
                    .map_err(|_| ResolutionError::Funding)?,
                request.expected_generation,
                now,
            )
            .map_err(|_| ResolutionError::Transition)?;
            let active = next
                .active_recovery_attempt()
                .ok_or(ResolutionError::Transition)?;
            if u32::from(active) != request.expected_recovery_index {
                return Err(ResolutionError::Transition.into());
            }
            let (_, policy) = recovery.ok_or(ResolutionError::Transition)?;
            let attempt = policy
                .attempt(active)
                .map_err(|_| ResolutionError::Transition)?;
            Ok(SourceTransitionPlan {
                next_state: next,
                kind: ResolutionCertificateKindV1::RecoveryAdvanced,
                route: attempt.provider_release_id().to_bytes(),
                attempt_index: u32::from(active)
                    .checked_add(1)
                    .ok_or(ResolutionError::Arithmetic)?,
                schedule_index: 0,
                selector: 0,
                observed_at: u64::try_from(now).map_err(|_| ResolutionError::Arithmetic)?,
                certificate_sequence: u64::from(active)
                    .checked_add(1)
                    .ok_or(ResolutionError::Arithmetic)?,
            })
        }
        FundedTransitionActionV3::Exhaust => {
            let (recovery_policy_id, policy) = recovery.ok_or(ResolutionError::Transition)?;
            let recovery_count = policy.attempt_count();
            let active = prior
                .active_recovery_attempt()
                .ok_or(ResolutionError::Transition)?;
            if active.checked_add(1) != Some(recovery_count)
                || u32::from(recovery_count) != request.expected_recovery_index
                || request.expected_funding_allocation_id != recovery_policy_id.to_bytes()
            {
                return Err(ResolutionError::Transition.into());
            }
            let attempt = policy
                .attempt(active)
                .map_err(|_| ResolutionError::Transition)?;
            let mut next = prior;
            next.exhaust_view(material_id, material, request.expected_generation, now)
                .map_err(|_| ResolutionError::Transition)?;
            Ok(SourceTransitionPlan {
                next_state: next,
                kind: ResolutionCertificateKindV1::Exhausted,
                route: attempt.provider_release_id().to_bytes(),
                attempt_index: u32::from(recovery_count),
                schedule_index: 0,
                selector: 0,
                observed_at: u64::try_from(now).map_err(|_| ResolutionError::Arithmetic)?,
                certificate_sequence: u64::from(recovery_count)
                    .checked_add(1)
                    .ok_or(ResolutionError::Arithmetic)?,
            })
        }
        FundedTransitionActionV3::CommitFailure => {
            let recovery_count = recovery.map_or(0, |(_, policy)| policy.attempt_count());
            if u32::from(recovery_count) != request.expected_recovery_index
                || request.expected_funding_allocation_id != material_id.to_bytes()
            {
                return Err(ResolutionError::Transition.into());
            }
            let selector = u32::from(
                material
                    .result_domain()
                    .map_err(|_| ResolutionError::ProductDomain)?
                    .failure_selector(),
            );
            Ok(SourceTransitionPlan {
                next_state: prior,
                kind: ResolutionCertificateKindV1::ResolutionFailure,
                route: [0; 32],
                attempt_index: u32::from(recovery_count),
                schedule_index: 0,
                selector,
                observed_at: 0,
                certificate_sequence: u64::from(recovery_count)
                    .checked_add(2)
                    .ok_or(ResolutionError::Arithmetic)?,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_funding_release(
    program_id: &Pubkey,
    funding_account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    manifest_staging: &AccountInfo<'_>,
    rent: &Rent,
    authority: MarketAuthority,
    material_id: ContentId,
    request: FundedTransitionRequestV3,
) -> Result<(FundingStateV1, u64, u64), ProgramError> {
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    authenticate_finalized_record(
        authority.registry_program,
        manifest_account,
        manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        authority.semantic_capability_manifest_id,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(authority.semantic_capability_manifest_id)
        .map_err(|_| ResolutionError::Funding)?;
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let mut funding =
        FundingStateV1::decode(&funding_data).map_err(|_| ResolutionError::Funding)?;
    if funding_account.owner != program_id || funding_account.data_len() != FUNDING_STATE_BYTES {
        return Err(ResolutionError::Funding.into());
    }
    let custody = FundingCustodyObservationV1::native_only(
        funding_account.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    funding
        .validate_against(manifest_id, manifest, custody)
        .map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        request.expected_generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionError::Funding)?;
    let expected = Pubkey::find_program_address(&derivation.seed_components(), program_id).0;
    if funding_account.key != &expected {
        return Err(ResolutionError::Funding.into());
    }
    let entry = manifest
        .entry(funding.entry_index())
        .map_err(|_| ResolutionError::Funding)?;
    let expected_allocation = match request.action {
        FundedTransitionActionV3::FailNext | FundedTransitionActionV3::Exhaust => {
            request.expected_funding_allocation_id
        }
        FundedTransitionActionV3::CommitFailure => material_id.to_bytes(),
    };
    if entry.config_id().to_bytes() != expected_allocation
        || request.expected_funding_allocation_id != expected_allocation
    {
        return Err(ResolutionError::Funding.into());
    }
    let quote = entry.funding_quote().amounts().bounty();
    if quote.asset_class() != FundingAssetClassV1::NativeLamports || quote.amount() == 0 {
        return Err(ResolutionError::Funding.into());
    }
    let plan = funding
        .release(
            manifest_id,
            manifest,
            custody,
            FundingCompartment::Bounty,
            quote.amount(),
        )
        .map_err(|_| ResolutionError::Funding)?;
    if plan.asset_class() != FundingAssetClassV1::NativeLamports || plan.amount() != quote.amount()
    {
        return Err(ResolutionError::Funding.into());
    }
    Ok((
        funding,
        quote.amount(),
        funding.remaining().bounty().amount(),
    ))
}

fn validate_funded_frame(accounts: &[AccountInfo<'_>], program_id: &Pubkey) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer {
            return Err(ResolutionError::AccountFrame.into());
        }
        if account.is_writable != (index <= 3) || account.executable != matches!(index, 6 | 16) {
            return Err(ResolutionError::AccountFrame.into());
        }
        if accounts
            .iter()
            .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
            .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let worker = accounts.get(3).ok_or(ResolutionError::AccountFrame)?;
    if accounts.get(6).ok_or(ResolutionError::AccountFrame)?.key != program_id
        || accounts.get(16).ok_or(ResolutionError::AccountFrame)?.key != &system_program::ID
        || worker.owner != &system_program::ID
        || worker.data_len() != 0
        || worker.executable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_funded_outputs<'info>(
    program_id: &Pubkey,
    state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    funding: &AccountInfo<'info>,
    worker: &AccountInfo<'info>,
    next_state: &[u8; SOURCE_RESOLUTION_STATE_BYTES],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES],
    next_funding: &[u8; FUNDING_STATE_BYTES],
    kind: ResolutionCertificateKindV1,
    sequence: u64,
    funding_lamports_after: u64,
    worker_lamports_after: u64,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    initialize_certificate_output(program_id, state, certificate, system, rent, kind, sequence)?;

    let mut state_output = state
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_output = certificate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut funding_output = funding
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut funding_lamports = funding
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut worker_lamports = worker
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES
        || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES
        || funding_output.len() != FUNDING_STATE_BYTES
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(next_state);
    certificate_output.copy_from_slice(next_certificate);
    funding_output.copy_from_slice(next_funding);
    **funding_lamports = funding_lamports_after;
    **worker_lamports = worker_lamports_after;
    Ok(())
}

fn next<'a, 'info>(
    iterator: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iterator).map_err(|_| ResolutionError::AccountFrame.into())
}
