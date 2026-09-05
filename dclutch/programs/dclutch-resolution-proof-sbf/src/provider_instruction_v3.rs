//! Physical current-release route consuming one real provider-owned update.
//!
//! This instruction never submits or reclaims a Pyth update. Provider posting
//! and reclaiming remain distinct Receiver/router instructions with their own
//! signers and CPI account frames. This route consumes an already-posted,
//! Receiver-owned fully verified update and persists only dClutch Source state
//! and its deterministic terminal certificate.

use core::cell::Ref;

use alloc::boxed::Box;

use dclutch_core_contract::ContentId as CapabilityContentId;
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetV2,
    },
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2};
use dclutch_product::ContentId as ProductContentId;
use dclutch_product::svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV2, authenticate_product_runtime_v2,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV2,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
    require_slot_pinned_release_v1,
};
use dclutch_source::pyth::{
    FullPriceUpdateV2, PostUpdateParamsView, PythReleaseV1, ReceiverConfigV2View,
};
use dclutch_source::resolution::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, PROVIDER_EXECUTION_REQUEST_MAGIC_V3,
    PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3, PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3,
    PROVIDER_RESOLUTION_CORE_TAIL_START_V3, PROVIDER_RESOLUTION_RECOVERY_TAIL_ACCOUNTS_V3,
    PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3, PROVIDER_RESOLUTION_TRADING_TAIL_START_V3,
    PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3, PROVIDER_UPDATE_LIFECYCLE_BYTES_V3,
    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderCallerV3,
    ProviderExecutionRequestV3, ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, provider_resolution_direct_intent_digest_v1,
};
use dclutch_source::{
    ContentId as SourceContentId, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1,
    PYTH_ADAPTER_CONFIG_BYTES, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, RECOVERY_POLICY_BYTES_V2, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_MATERIAL_V3_BYTES, SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_BYTES, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV3,
    SourceResolutionStateV2, SourceSpecV1, StatisticSpecV1, WINDOW_SPEC_BYTES,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::{allocate, assign};

use crate::market_admission_v1::RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::{
    ResolutionError, authenticate_clock, authenticate_rent, cached_deployment_observation,
    pinned_deployment_refusal,
    provider_v3::{
        AuthenticatedProviderObservationV3, AuthenticatedRecoveryLadderV3,
        AuthenticatedSourceRecordsV3, ProviderJoinErrorV3, plan_provider_resolution_v3,
    },
};

/// Return whether bytes select the current provider-resolution instruction.
pub(crate) fn is_provider_resolution_v3(bytes: &[u8]) -> bool {
    bytes.len() > PROVIDER_EXECUTION_REQUEST_BYTES_V3
        && bytes.get(..8) == Some(PROVIDER_EXECUTION_REQUEST_MAGIC_V3.as_slice())
}

/// Consume one already posted provider update through the current Source and
/// Product graph and return a typed receipt to the authenticated caller.
#[inline(never)]
pub(crate) fn process_provider_resolution_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !is_provider_resolution_v3(instruction_data) {
        return Err(ResolutionError::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(..PROVIDER_EXECUTION_REQUEST_BYTES_V3)
        .ok_or(ResolutionError::Instruction)?;
    let post_body = instruction_data
        .get(PROVIDER_EXECUTION_REQUEST_BYTES_V3..)
        .ok_or(ResolutionError::Instruction)?;
    let request = Box::new(
        ProviderExecutionRequestV3::decode(request_bytes)
            .map_err(|_| ResolutionError::Instruction)?,
    );
    PostUpdateParamsView::parse(post_body).map_err(|_| ResolutionError::ProviderObservation)?;
    // A capture on a rung above the primary brings the ladder that names its
    // source, and brings it at the END so that every position a primary capture
    // ever used keeps its index. `source_index` is not yet trusted here -- the
    // Source state settles that below -- but it does have to be self-consistent
    // with the frame it arrived in, and an account count is the cheapest place
    // to say so.
    let (base_count, tail_start) = match request.caller {
        ProviderCallerV3::Core | ProviderCallerV3::Resolution => (
            PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3,
            PROVIDER_RESOLUTION_CORE_TAIL_START_V3,
        ),
        ProviderCallerV3::Trading => (
            PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3,
            PROVIDER_RESOLUTION_TRADING_TAIL_START_V3,
        ),
    };
    let recovery_start = if request.source_index == 0 {
        None
    } else {
        Some(base_count)
    };
    let expected_count = match recovery_start {
        None => base_count,
        Some(_) => base_count
            .checked_add(PROVIDER_RESOLUTION_RECOVERY_TAIL_ACCOUNTS_V3)
            .ok_or(ResolutionError::AccountFrame)?,
    };
    if accounts.len() != expected_count {
        return Err(ResolutionError::AccountFrame.into());
    }
    authenticate_privileges(program_id, accounts, tail_start)?;
    let frame = ProviderFrameV3 {
        accounts,
        tail_start,
        recovery_start,
    };
    authenticate_request_accounts(&request, frame)?;

    let rent = authenticate_rent(frame.rent())?;
    let clock = authenticate_clock(frame.clock())?;
    let market = authenticate_market_and_infrastructure(program_id, &request, frame)?;
    authenticate_activation_and_caller(program_id, &request, frame)?;
    let source_records = boxed_source_records(&request, frame)?;
    let ladder = authenticate_recovery_ladder(&request, frame)?;
    let product_runtime = boxed_product_runtime(&request, frame)?;
    if market.identity.product_record.to_bytes() != request.product_record
        || product_runtime.product_record.content_digest.to_bytes() != request.product_record
        || product_runtime
            .result_domain_record
            .content_digest
            .to_bytes()
            != request.result_domain
    {
        return Err(ResolutionError::ProductDomain.into());
    }

    let pyth_release = boxed_pyth_release(&request, frame)?;
    let update_data = frame
        .update()
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let lifecycle = boxed_provider_lifecycle(program_id, &request, frame, &update_data)?;
    let result_domain_data = frame
        .account(33)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let source_data = frame
        .account(2)
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source = boxed_source_state(&source_data)?;
    let observation = boxed_observation(
        &request,
        frame,
        &pyth_release,
        &product_runtime,
        &result_domain_data,
        &update_data,
        &lifecycle,
        post_body,
        clock,
    )?;
    let plan = plan_provider_resolution_v3(
        request_bytes,
        &source,
        &source_records,
        ladder.as_deref(),
        &observation,
    )
    .map_err(map_provider_join_error)?;
    drop(source_data);
    drop(result_domain_data);
    drop(update_data);
    commit_plan(program_id, &request, frame, &rent, &lifecycle, &plan)
}

const fn map_provider_join_error(error: ProviderJoinErrorV3) -> ResolutionError {
    match error {
        ProviderJoinErrorV3::Request => ResolutionError::Instruction,
        ProviderJoinErrorV3::Source => ResolutionError::SourceMaterial,
        ProviderJoinErrorV3::SourceLadder => ResolutionError::SourceLadder,
        ProviderJoinErrorV3::Product => ResolutionError::ProductDomain,
        ProviderJoinErrorV3::Provider => ResolutionError::ProviderObservation,
        ProviderJoinErrorV3::ProviderWindow => ResolutionError::ProviderWindow,
        ProviderJoinErrorV3::ProviderFreshness => ResolutionError::ProviderFreshness,
        ProviderJoinErrorV3::ProviderConfiguration => ResolutionError::ProviderConfiguration,
        ProviderJoinErrorV3::ProviderScale => ResolutionError::ProviderScale,
        ProviderJoinErrorV3::Transition => ResolutionError::Transition,
        ProviderJoinErrorV3::Arithmetic => ResolutionError::Arithmetic,
    }
}

fn boxed_source_records(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<Box<AuthenticatedSourceRecordsV3>, ProgramError> {
    Ok(Box::new(authenticate_source_records(request, frame)?))
}

fn boxed_product_runtime(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<Box<dclutch_product::svm_reader::AuthenticatedProductRuntimeV2>, ProgramError> {
    Ok(Box::new(
        authenticate_product_runtime_v2(
            frame.registry_program().key,
            ProductContentId::new(request.product_record)
                .map_err(|_| ResolutionError::ProductDomain)?,
            ProductRuntimeFrameV2 {
                product: record_frame(frame, 31),
                result_domain: record_frame(frame, 33),
                portfolio: record_frame(frame, 35),
            },
        )
        .map_err(|_| ResolutionError::ProductDomain)?,
    ))
}

fn boxed_pyth_release(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<Box<PythReleaseV1>, ProgramError> {
    Ok(Box::new(authenticate_pyth_release(request, frame)?))
}

fn boxed_source_state(bytes: &[u8]) -> Result<Box<SourceResolutionStateV2>, ProgramError> {
    Ok(Box::new(
        SourceResolutionStateV2::decode(bytes).map_err(|_| ResolutionError::OutputState)?,
    ))
}

fn boxed_provider_lifecycle(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    update_bytes: &[u8],
) -> Result<Box<ProviderUpdateLifecycleV3>, ProgramError> {
    let lifecycle_account = frame.lifecycle();
    let (expected_lifecycle, bump) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            frame.update().key.as_ref(),
        ],
        program_id,
    );
    let (expected_authority, _) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            frame.update().key.as_ref(),
        ],
        program_id,
    );
    let lifecycle_data = lifecycle_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if lifecycle_account.key != &expected_lifecycle
        || lifecycle_account.owner != program_id
        || lifecycle_account.executable
        || lifecycle_data.len() != PROVIDER_UPDATE_LIFECYCLE_BYTES_V3
        || !funded_rent_persists_v1(lifecycle_account.lamports())
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let lifecycle = ProviderUpdateLifecycleV3::decode(&lifecycle_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update =
        FullPriceUpdateV2::parse(update_bytes).map_err(|_| ResolutionError::ProviderObservation)?;
    if lifecycle.status != ProviderUpdateStatusV3::Submitted
        || lifecycle.bump != bump
        || lifecycle.generation != request.generation
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.source_material != request.source_material
        || lifecycle.provider_release != request.provider_release
        || lifecycle.update_account != request.update_account
        || lifecycle.update_digest != request.expected_update_digest
        || lifecycle.post_body_digest != request.post_params_body_digest
        || lifecycle.provider_submitter != request.provider_submitter
        || lifecycle.release_set != request.release_set
        || lifecycle.registry_program != frame.registry_program().key.to_bytes()
        || lifecycle.update_authority != expected_authority.to_bytes()
        || lifecycle.update_authority != update.write_authority()
        || lifecycle.posted_slot != update.posted_slot()
        || lifecycle.publish_time != update.publish_time()
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    Ok(Box::new(lifecycle))
}

#[allow(clippy::too_many_arguments)]
fn boxed_observation<'a>(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    pyth_release: &PythReleaseV1,
    product_runtime: &dclutch_product::svm_reader::AuthenticatedProductRuntimeV2,
    result_domain_bytes: &'a [u8],
    update_bytes: &'a [u8],
    lifecycle: &ProviderUpdateLifecycleV3,
    post_body: &'a [u8],
    clock: solana_program::clock::Clock,
) -> Result<Box<AuthenticatedProviderObservationV3<'a>>, ProgramError> {
    FullPriceUpdateV2::parse(update_bytes).map_err(|_| ResolutionError::ProviderObservation)?;
    let result_domain = dclutch_product::ResultDomainV2::decode(result_domain_bytes)
        .map_err(|_| ResolutionError::ProductDomain)?;
    Ok(Box::new(AuthenticatedProviderObservationV3 {
        pyth_release_id: request.provider_release,
        pyth_release: *pyth_release,
        product_runtime: *product_runtime,
        result_domain_bytes,
        result_domain,
        update_account: frame.update().key.to_bytes(),
        provider_submitter: lifecycle.provider_submitter,
        expected_update_authority: lifecycle.update_authority,
        update_bytes,
        post_params_body: post_body,
        current_slot: clock.slot,
        current_unix_seconds: clock.unix_timestamp,
    }))
}

fn commit_plan<'info>(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, 'info>,
    rent: &Rent,
    lifecycle: &ProviderUpdateLifecycleV3,
    plan: &crate::provider_v3::ProviderResolutionPlanV3,
) -> ProgramResult {
    let next_source = Box::new(plan.next_source.to_bytes());
    let certificate = boxed_certificate(plan)?;
    let lifecycle_bytes = boxed_consumed_lifecycle(request, lifecycle, plan)?;
    commit_outputs(
        program_id,
        request,
        frame,
        rent,
        &next_source,
        &certificate,
        &lifecycle_bytes,
    )?;
    set_provider_receipt(plan)
}

#[inline(never)]
fn boxed_certificate(
    plan: &crate::provider_v3::ProviderResolutionPlanV3,
) -> Result<Box<[u8; RESOLUTION_CERTIFICATE_BYTES_V2]>, ProgramError> {
    Ok(Box::new(
        plan.certificate
            .to_bytes()
            .map_err(|_| ResolutionError::Transition)?,
    ))
}

#[inline(never)]
fn boxed_consumed_lifecycle(
    request: &ProviderExecutionRequestV3,
    lifecycle: &ProviderUpdateLifecycleV3,
    plan: &crate::provider_v3::ProviderResolutionPlanV3,
) -> Result<Box<[u8; PROVIDER_UPDATE_LIFECYCLE_BYTES_V3]>, ProgramError> {
    let mut next = Box::new(*lifecycle);
    next.consume(
        request.terminal_sequence,
        plan.receipt.provider_evidence,
        request.certificate_account,
    )
    .map_err(|_| ResolutionError::Transition)?;
    Ok(Box::new(
        next.to_bytes().map_err(|_| ResolutionError::Transition)?,
    ))
}

#[inline(never)]
fn set_provider_receipt(plan: &crate::provider_v3::ProviderResolutionPlanV3) -> ProgramResult {
    let receipt = plan
        .receipt
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    set_return_data(&receipt);
    Ok(())
}

#[derive(Clone, Copy)]
struct ProviderFrameV3<'accounts, 'info> {
    accounts: &'accounts [AccountInfo<'info>],
    tail_start: usize,
    /// Index of the `RecoveryPolicyV2` raw record, present exactly when this
    /// capture answers on a rung above the primary.
    recovery_start: Option<usize>,
}

impl<'accounts, 'info> ProviderFrameV3<'accounts, 'info> {
    // Constructed only after one of the two exact account counts is checked;
    // every caller uses a frozen index below the smaller Core count.
    #[allow(clippy::indexing_slicing)]
    fn account(self, index: usize) -> &'accounts AccountInfo<'info> {
        &self.accounts[index]
    }
    fn registry_program(self) -> &'accounts AccountInfo<'info> {
        self.account(7)
    }
    fn lifecycle(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start - 1)
    }
    fn update(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start)
    }
    fn receiver_program(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 1)
    }
    fn receiver_programdata(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 2)
    }
    fn receiver_config(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 3)
    }
    fn router_program(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 4)
    }
    fn router_programdata(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 5)
    }
    fn clock(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 6)
    }
    fn rent(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 7)
    }
    fn system(self) -> &'accounts AccountInfo<'info> {
        self.account(self.tail_start + 8)
    }
    fn recovery_policy(self) -> Option<usize> {
        self.recovery_start
    }
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    tail_start: usize,
) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        let expected_executable = matches!(index, 7 | 11 | 13 | 15)
            || matches!(index, value if value == tail_start + 1 || value == tail_start + 4 || value == tail_start + 8);
        if account.is_signer != matches!(index, 0 | 1)
            || account.is_writable != (matches!(index, 2 | 3) || index == tail_start - 1)
            || account.executable != expected_executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    if accounts.get(15).ok_or(ResolutionError::AccountFrame)?.key != program_id
        || accounts
            .get(tail_start + 8)
            .ok_or(ResolutionError::AccountFrame)?
            .key
            != &system_program::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_request_accounts(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> ProgramResult {
    if frame.account(1).key.to_bytes() != request.resolver
        || frame.account(2).key.to_bytes() != request.source_state
        || frame.account(3).key.to_bytes() != request.certificate_account
        || frame.account(4).key.to_bytes() != request.market
        || frame.update().key.to_bytes() != request.update_account
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_market_and_infrastructure(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<CoreState, ProgramError> {
    let market_account = frame.account(4);
    if market_account.owner != frame.account(11).key || market_account.executable {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let market = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    if !RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1.admits(market.phase, market.readiness)
        || market.identity.market_id.to_bytes() != request.market
        || market.identity.generation != request.generation
        || market.identity.registry_program.to_bytes() != frame.registry_program().key.to_bytes()
        || market.identity.resolution_policy.to_bytes() != request.source_material
        || market.identity.selected_release_set.to_bytes() != request.release_set
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
            frame.account(11).key,
        )
        .0 != *market_account.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    drop(market_data);

    let infrastructure = frame.account(6);
    let expected_infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        frame.account(11).key,
    )
    .0;
    let infrastructure_data = infrastructure
        .try_borrow_data()
        .map_err(|_| ResolutionError::InfrastructureProfile)?;
    if infrastructure.key != &expected_infrastructure
        || infrastructure.owner != frame.account(11).key
        || infrastructure.executable
        || infrastructure_data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || !funded_rent_persists_v1(infrastructure.lamports())
    {
        return Err(ResolutionError::InfrastructureProfile.into());
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&infrastructure_data)
        .map_err(|_| ResolutionError::InfrastructureProfile)?;
    drop(infrastructure_data);
    if profile.registry().program().to_bytes() != frame.registry_program().key.to_bytes() {
        return Err(ResolutionError::InfrastructureProfile.into());
    }
    let artifact_data = frame
        .account(9)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.registry_program().key,
        frame.account(9),
        frame.account(10),
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        profile.registry().artifact_release().to_bytes(),
        &artifact_data,
        ARTIFACT_RELEASE_BYTES_V1,
    )?;
    let artifact = ArtifactReleaseV1::decode(&artifact_data)
        .map_err(|_| ResolutionError::InfrastructureProfile)?;
    if artifact.program().to_bytes() != frame.registry_program().key.to_bytes() {
        return Err(ResolutionError::InfrastructureProfile.into());
    }
    require_slot_pinned_release_v1(artifact).map_err(|_| ResolutionError::InfrastructureProfile)?;
    let observation =
        cached_deployment_observation(frame.registry_program(), frame.account(8), artifact)?;
    artifact
        .authenticate_deployment(observation)
        .map_err(pinned_deployment_refusal)?;
    if frame.account(15).key != program_id {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    Ok(market)
}

fn authenticate_activation_and_caller(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> ProgramResult {
    let activation_data = frame
        .account(5)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ActivationCache)?;
    if frame.account(5).owner != frame.registry_program().key
        || frame.account(5).executable
        || activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
            frame.registry_program().key,
        )
        .0 != *frame.account(5).key
    {
        return Err(ResolutionError::ActivationCache.into());
    }
    let activation = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ActivationCache)?;
    if activation
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ActivationCache)?
        .to_bytes()
        != request.release_set
    {
        return Err(ResolutionError::ActivationCache.into());
    }
    for (role, program, programdata) in [
        (ExecutionRoleV1::Core, frame.account(11), frame.account(12)),
        (
            ExecutionRoleV1::Trading,
            frame.account(13),
            frame.account(14),
        ),
        (
            ExecutionRoleV1::Resolution,
            frame.account(15),
            frame.account(16),
        ),
    ] {
        let resolution_role = role == ExecutionRoleV1::Resolution;
        let activated = activation.role(role).map_err(|_| {
            if resolution_role {
                ResolutionError::ResolutionRelease
            } else {
                ResolutionError::ActivatedRole
            }
        })?;
        if activated.release().program().to_bytes() != program.key.to_bytes() {
            if resolution_role {
                return Err(ResolutionError::ResolutionRelease.into());
            }
            return Err(ResolutionError::ActivatedRole.into());
        }
        if resolution_role
            && (program.key != program_id
                || activated.release().semantic_release_id().to_bytes()
                    != RESOLUTION_CONTROLLER_RELEASE_ID_V7)
        {
            return Err(ResolutionError::ResolutionRelease.into());
        }
        if caller_executes_role(request.caller, role) {
            let observation =
                cached_deployment_observation(program, programdata, activated.release())?;
            activated
                .authenticate_current_deployment(observation)
                .map_err(|_| ResolutionError::ResolutionDeployment)?;
        }
    }
    let caller_role = match request.caller {
        ProviderCallerV3::Core => ExecutionRoleV1::Core,
        ProviderCallerV3::Trading => ExecutionRoleV1::Trading,
        ProviderCallerV3::Resolution => ExecutionRoleV1::Resolution,
    };
    let caller_program = match request.caller {
        ProviderCallerV3::Core => frame.account(11),
        ProviderCallerV3::Trading => frame.account(13),
        ProviderCallerV3::Resolution => frame.account(15),
    };
    if request.caller_program != caller_program.key.to_bytes()
        || (request.caller == ProviderCallerV3::Resolution
            && provider_resolution_direct_intent_digest_v1(*request)
                .map_err(|_| ResolutionError::Instruction)?
                != request.parent_request_digest)
    {
        return Err(ResolutionError::CallerAuthority.into());
    }
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        caller_role,
        request.source_state,
        request.parent_request_digest,
    )
    .map_err(|_| ResolutionError::CallerAuthority)?;
    if Pubkey::find_program_address(&seeds.as_slices(), caller_program.key).0
        != *frame.account(0).key
    {
        return Err(ResolutionError::CallerAuthority.into());
    }
    if request.caller == ProviderCallerV3::Trading {
        authenticate_trading_capability_records(
            frame.registry_program().key,
            frame.account(37),
            frame.account(38),
            frame.account(39),
            frame.account(40),
            request,
        )?;
    }
    Ok(())
}

/// Authenticate the two Registry records a Trading caller binds: the program
/// set its Market selected, and the capability descriptor the request names.
///
/// The schemas are not a choice made here. `ProviderCallerV3::Trading` has
/// exactly one producer in the protocol -- `resolution_composition_v3` in the
/// Trading program, reachable only from common Hot -- and the values it puts in
/// `capability_program_set` and `selected_capability_program` are the ones
/// common Hot itself authenticated: a set under
/// `CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2` and a descriptor under
/// `v4::SCHEMA_RELEASE_ID`. Authenticating them under any other schema is not a
/// stricter check, it is an unsatisfiable one: the schema is a SEED of the
/// finalized record's own address, and the descriptor's expected width is
/// joined to a digest of bytes that are 600 wide.
///
/// What this route can prove about the selection, and what it cannot. Common
/// Hot chooses the entry by reading the set's selector out of the FAMILY
/// request; that byte string is not in this frame and never will be -- only its
/// digest is, as `parent_request_digest`. So the strongest satisfiable
/// statement here is MEMBERSHIP: the descriptor the request names is one this
/// authenticated set admits, under the schema this route decodes. Which entry
/// selected it is Trading's conjunct, bound to this request by the parent
/// digest the caller-authority seeds already carry.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_trading_capability_records(
    registry: &Pubkey,
    set_raw: &AccountInfo<'_>,
    set_staging: &AccountInfo<'_>,
    descriptor_raw: &AccountInfo<'_>,
    descriptor_staging: &AccountInfo<'_>,
    request: &ProviderExecutionRequestV3,
) -> ProgramResult {
    let set_data = set_raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        set_raw,
        set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        request.capability_program_set,
        &set_data,
        set_data.len(),
    )?;
    let set = CapabilityProgramSetV2::decode_selected(
        request.capability_program_set,
        hash(&set_data).to_bytes(),
        &set_data,
    )
    .map_err(|_| ResolutionError::FinalizedRecord)?;
    let admitted = CapabilityDescriptorReferenceV2::new(
        CapabilityContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4)
            .map_err(|_| ResolutionError::FinalizedRecord)?,
        CapabilityContentId::new(request.selected_capability_program)
            .map_err(|_| ResolutionError::FinalizedRecord)?,
    );
    let mut index = 0_u16;
    let mut member = false;
    while index < set.entry_count() {
        member |= set
            .entry(index)
            .map_err(|_| ResolutionError::FinalizedRecord)?
            .descriptor()
            == admitted;
        index = index
            .checked_add(1)
            .ok_or(ResolutionError::FinalizedRecord)?;
    }
    if !member {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    drop(set_data);
    let descriptor_data = descriptor_raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        descriptor_raw,
        descriptor_staging,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        request.selected_capability_program,
        &descriptor_data,
        CAPABILITY_PROGRAM_V4_BYTES,
    )?;
    if boxed_capability_program_v4(&descriptor_data)?
        .request_schema()
        .to_bytes()
        != PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    Ok(())
}

/// Out of line and boxed: the decoded descriptor is 600 bytes wide, and this
/// route's callers are already the deepest frames in the program.
#[inline(never)]
fn boxed_capability_program_v4(bytes: &[u8]) -> Result<Box<CapabilityProgramV4>, ProgramError> {
    CapabilityProgramV4::decode(bytes)
        .map(Box::new)
        .map_err(|_| ResolutionError::FinalizedRecord.into())
}

const fn caller_executes_role(caller: ProviderCallerV3, role: ExecutionRoleV1) -> bool {
    matches!(role, ExecutionRoleV1::Resolution)
        || matches!(
            (caller, role),
            (ProviderCallerV3::Core, ExecutionRoleV1::Core)
                | (ProviderCallerV3::Trading, ExecutionRoleV1::Trading)
                | (ProviderCallerV3::Resolution, ExecutionRoleV1::Core)
        )
}

fn authenticate_source_records(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<AuthenticatedSourceRecordsV3, ProgramError> {
    let material_data = borrow_record(
        frame,
        17,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let source_data = borrow_record(
        frame,
        19,
        SOURCE_SPEC_SCHEMA_ID_V1,
        request.source_spec,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&source_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let provider_id = source.provider_release_id();
    let provider_data = borrow_record(
        frame,
        21,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_id.to_bytes(),
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider_release =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let adapter_id = source.adapter_config_id();
    let adapter_data = borrow_record(
        frame,
        23,
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        adapter_id.to_bytes(),
        PYTH_ADAPTER_CONFIG_BYTES,
    )?;
    let adapter_config =
        PythAdapterConfigV1::decode(&adapter_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_id = material.window_spec();
    let window_data = borrow_record(
        frame,
        25,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_id.to_bytes(),
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let statistic_id = material.statistic_spec();
    let statistic_data = borrow_record(
        frame,
        27,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_id.to_bytes(),
        STATISTIC_SPEC_BYTES,
    )?;
    let statistic =
        StatisticSpecV1::decode(&statistic_data).map_err(|_| ResolutionError::SourceMaterial)?;
    Ok(AuthenticatedSourceRecordsV3 {
        material_id: SourceContentId::new(request.source_material)
            .map_err(|_| ResolutionError::SourceMaterial)?,
        material,
        source_spec_id: SourceContentId::new(request.source_spec)
            .map_err(|_| ResolutionError::SourceMaterial)?,
        source,
        provider_release_id: provider_id,
        provider_release,
        adapter_config_id: adapter_id,
        adapter_config,
        window_spec_id: window_id,
        window,
        statistic_spec_id: statistic_id,
        statistic,
        failure_policy_release: SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)
            .map_err(|_| ResolutionError::SourceMaterial)?,
    })
}

/// Authenticate the funded ordered-recovery ladder a rung capture rides with.
///
/// The digest is the MATERIAL's, never the request's: a caller brings the
/// record but does not get to say which policy this market bought, exactly as
/// it brings the window and the statistic without getting to say which ones
/// those are. A market whose material selects no policy has no ladder to
/// authenticate and refuses on the ladder if a rung was named anyway; a market
/// that has one but was asked on its primary reads no policy at all, which is
/// why a primary capture's frame did not have to grow.
fn authenticate_recovery_ladder(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<Option<Box<AuthenticatedRecoveryLadderV3>>, ProgramError> {
    let Some(index) = frame.recovery_policy() else {
        return Ok(None);
    };
    let material_data = borrow_record(
        frame,
        17,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let policy_id = material
        .recovery_policy()
        .ok_or(ResolutionError::SourceLadder)?;
    let policy_data = borrow_record(
        frame,
        index,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        policy_id.to_bytes(),
        RECOVERY_POLICY_BYTES_V2,
    )?;
    let policy =
        RecoveryPolicyV2::decode(&policy_data).map_err(|_| ResolutionError::SourceMaterial)?;
    Ok(Some(Box::new(AuthenticatedRecoveryLadderV3 {
        policy_id,
        policy,
    })))
}

fn authenticate_pyth_release(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
) -> Result<PythReleaseV1, ProgramError> {
    let bytes = borrow_record(
        frame,
        29,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        request.provider_release,
        dclutch_source::pyth::PYTH_RELEASE_V1_ENCODED_LEN,
    )?;
    let release = PythReleaseV1::decode(&bytes).map_err(|_| ResolutionError::ProviderRelease)?;
    if release.receiver_program() != frame.receiver_program().key.to_bytes()
        || release.receiver_programdata() != frame.receiver_programdata().key.to_bytes()
        || release.receiver_config() != frame.receiver_config().key.to_bytes()
        || release.router_program() != frame.router_program().key.to_bytes()
        || release.router_programdata() != frame.router_programdata().key.to_bytes()
        || frame.update().owner != frame.receiver_program().key
        || frame.receiver_config().owner != frame.receiver_program().key
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_provider_program(
        frame.receiver_program(),
        frame.receiver_programdata(),
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_provider_program(
        frame.router_program(),
        frame.router_programdata(),
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let config_data = frame
        .receiver_config()
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if hash(&config_data).to_bytes() != release.config_digest()
        || !funded_rent_persists_v1(frame.receiver_config().lamports())
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let config = ReceiverConfigV2View::parse(&config_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if config.router_program() != release.router_program() {
        return Err(ResolutionError::ProviderObservation.into());
    }
    Ok(release)
}

pub(crate) fn authenticate_provider_program(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    expected_programdata: [u8; 32],
    expected_slot: u64,
) -> ProgramResult {
    if program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
        || programdata.key.to_bytes() != expected_programdata
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let program_data = program
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderRelease)?;
    let program_view =
        ProgramV3View::parse(&program_data).map_err(|_| ResolutionError::ProviderRelease)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata_key() != expected_programdata || programdata.key != &derived {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let data = programdata
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderRelease)?;
    let view = ProgramDataV3View::parse(&data).map_err(|_| ResolutionError::ProviderRelease)?;
    if view.deployment_slot() != expected_slot {
        return Err(ResolutionError::ProviderRelease.into());
    }
    Ok(())
}

fn record_frame<'accounts, 'info>(
    frame: ProviderFrameV3<'accounts, 'info>,
    index: usize,
) -> FinalizedRecordFrameV2<'accounts, 'info> {
    FinalizedRecordFrameV2 {
        raw: frame.account(index),
        staging: frame.account(index + 1),
    }
}

fn borrow_record<'a>(
    frame: ProviderFrameV3<'a, '_>,
    index: usize,
    schema: [u8; 32],
    digest: [u8; 32],
    expected_len: usize,
) -> Result<Ref<'a, &'a mut [u8]>, ProgramError> {
    let data = frame
        .account(index)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.registry_program().key,
        frame.account(index),
        frame.account(index + 1),
        schema,
        digest,
        &data,
        expected_len,
    )?;
    Ok(data)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &[u8],
    expected_len: usize,
) -> ProgramResult {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry).0;
    if bytes.len() != expected_len
        || digest == [0; 32]
        || raw.key != &expected_raw
        || raw.owner != registry
        || raw.executable
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.executable
        || staging.lamports() != 0
        || staging.data_len() != 0
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    Ok(())
}

fn commit_outputs<'info>(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, 'info>,
    rent: &Rent,
    source: &[u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES_V2],
    lifecycle: &[u8; PROVIDER_UPDATE_LIFECYCLE_BYTES_V3],
) -> ProgramResult {
    let source_account = frame.account(2);
    let certificate_account = frame.account(3);
    let lifecycle_account = frame.lifecycle();
    if source_account.owner != program_id
        || source_account.data_len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || source_account.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    source_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if lifecycle_account.owner != program_id
        || lifecycle_account.data_len() != PROVIDER_UPDATE_LIFECYCLE_BYTES_V3
        || lifecycle_account.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    lifecycle_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    initialize_certificate(
        program_id,
        request,
        source_account,
        certificate_account,
        frame.system(),
        rent,
    )?;
    let mut source_output = source_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_output = certificate_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut lifecycle_output = lifecycle_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    source_output.copy_from_slice(source);
    certificate_output.copy_from_slice(certificate);
    lifecycle_output.copy_from_slice(lifecycle);
    Ok(())
}

fn initialize_certificate<'info>(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    source: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    // Lean-owned Runtime V2 wire tag for ResolutionSuccess.
    let kind_seed = [1_u8];
    let sequence_seed = request.terminal_sequence.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    );
    if certificate.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    let minimum = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
    if certificate.owner == program_id {
        if certificate.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
            || certificate.lamports() < minimum
            || certificate.executable
        {
            return Err(ResolutionError::OutputState.into());
        }
        return Ok(());
    }
    if certificate.owner != &system_program::ID
        || certificate.data_len() != 0
        || certificate.lamports() < minimum
        || certificate.executable
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    let bump_seed = [bump];
    let signer = [
        RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
        source.key.as_ref(),
        kind_seed.as_slice(),
        sequence_seed.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            certificate.key,
            u64::try_from(RESOLUTION_CERTIFICATE_BYTES_V2)
                .map_err(|_| ResolutionError::Arithmetic)?,
        ),
        &[certificate.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(certificate.key, program_id),
        &[certificate.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec::Vec};

    use dclutch_market::capability_program::{
        set_v2::{
            CapabilityProgramSetEntryV2, SelectorWidthV2, encode_program_set_v2,
            encoded_program_set_bytes_v2,
        },
        v4::{ArtifactReferenceV4, CapabilityArtifactsV4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5},
    };
    use dclutch_source::resolution::PROVIDER_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3;
    use dclutch_source::{
        PROVIDER_RELEASE_SCHEMA_PREIMAGE_V1, PYTH_ADAPTER_CONFIG_SCHEMA_PREIMAGE_V1,
        SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V3, SOURCE_MATERIAL_V3_MAGIC,
        SOURCE_SPEC_SCHEMA_PREIMAGE_V1, STATISTIC_SPEC_SCHEMA_PREIMAGE_V1,
        WINDOW_SPEC_SCHEMA_PREIMAGE_V1,
    };

    use super::*;

    const CAPTURED_POST_UPDATE: &[u8] = include_bytes!(
        "../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
    );

    fn test_account(
        key: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    #[test]
    fn finalized_profile_release_identity_refuses_before_cached_deployment_auth() {
        let registry = Pubkey::new_from_array([0xd1; 32]);
        let schema = ARTIFACT_RELEASE_SCHEMA_ID_V1;
        let artifact_bytes = std::vec![0x44; ARTIFACT_RELEASE_BYTES_V1];
        let digest = hash(&artifact_bytes).to_bytes();
        let raw_key =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
        let staging_key = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &registry,
        )
        .0;
        let rent = Rent::default();
        let raw = test_account(
            raw_key,
            rent.minimum_balance(artifact_bytes.len()),
            artifact_bytes.clone(),
            registry,
        );
        let staging = test_account(staging_key, 0, Vec::new(), system_program::ID);
        assert_eq!(
            authenticate_record(
                &registry,
                &raw,
                &staging,
                schema,
                digest,
                &artifact_bytes,
                ARTIFACT_RELEASE_BYTES_V1
            ),
            Ok(()),
        );

        let substituted_profile_release = [0xd2; 32];
        assert_eq!(
            authenticate_record(
                &registry,
                &raw,
                &staging,
                schema,
                substituted_profile_release,
                &artifact_bytes,
                ARTIFACT_RELEASE_BYTES_V1
            ),
            Err(ProgramError::Custom(
                ResolutionError::FinalizedRecord as u32
            )),
            "the infrastructure profile's finalized ArtifactRelease identity remains authoritative",
        );
    }

    fn capability_content(tag: u8) -> CapabilityContentId {
        CapabilityContentId::new([tag; 32]).expect("nonzero fixture identity")
    }

    /// One exact V4 descriptor, built the way every family's bundle builds one.
    fn trading_descriptor(request_schema: [u8; 32]) -> [u8; CAPABILITY_PROGRAM_V4_BYTES] {
        CapabilityProgramV4::new(
            capability_content(0x11),
            capability_content(0x12),
            CapabilityContentId::new(request_schema).expect("request schema"),
            capability_content(0x14),
            capability_content(0x15),
            capability_content(0x16),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(
                    capability_content(0x21),
                    capability_content(0x22),
                ),
                request_profile: ArtifactReferenceV4::new(
                    capability_content(0x23),
                    capability_content(0x24),
                ),
                lifecycle: ArtifactReferenceV4::new(
                    CapabilityContentId::new(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)
                        .expect("production lifecycle schema"),
                    capability_content(0x26),
                ),
                strategy: ArtifactReferenceV4::new(
                    capability_content(0x27),
                    capability_content(0x28),
                ),
                transition: ArtifactReferenceV4::new(
                    capability_content(0x29),
                    capability_content(0x2a),
                ),
                effect: ArtifactReferenceV4::new(
                    capability_content(0x2b),
                    capability_content(0x2c),
                ),
            },
            64,
        )
        .expect("V4 descriptor")
        .encode()
    }

    fn trading_program_set(entries: &[CapabilityProgramSetEntryV2]) -> Vec<u8> {
        let mut bytes =
            std::vec![0_u8; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
        // Offset 8 of the FAMILY request, four bytes: the shape every family's
        // own set uses. Nothing in this frame can read that request, which is
        // exactly why this route proves membership rather than selection.
        encode_program_set_v2(8, SelectorWidthV2::U32, entries, &mut bytes).expect("encoded set");
        bytes
    }

    fn trading_request(
        capability_program_set: [u8; 32],
        selected_capability_program: [u8; 32],
    ) -> ProviderExecutionRequestV3 {
        ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Trading,
            source_index: 0,
            generation: 7,
            terminal_sequence: 1,
            market: [41; 32],
            source_state: [42; 32],
            certificate_account: [43; 32],
            source_material: [44; 32],
            source_spec: [45; 32],
            product_record: [46; 32],
            result_domain: [47; 32],
            provider_release: [48; 32],
            update_account: [49; 32],
            expected_update_digest: [50; 32],
            provider_submitter: [51; 32],
            resolver: [52; 32],
            caller_program: [53; 32],
            release_set: [54; 32],
            capability_program_set,
            selected_capability_program,
            parent_request_digest: [55; 32],
            post_params_body_digest: [56; 32],
        }
    }

    /// Place one finalized record at the Registry address its schema and digest
    /// derive, and return the raw/staging pair.
    fn finalized_record(
        registry: &Pubkey,
        schema: [u8; 32],
        bytes: &[u8],
    ) -> (AccountInfo<'static>, AccountInfo<'static>) {
        let digest = hash(bytes).to_bytes();
        let raw_key =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry).0;
        let staging_key =
            Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry)
                .0;
        (
            test_account(
                raw_key,
                Rent::default().minimum_balance(bytes.len()),
                Vec::from(bytes),
                *registry,
            ),
            test_account(staging_key, 0, Vec::new(), system_program::ID),
        )
    }

    /// The Trading caller's two record conjuncts, on the exact artifacts common
    /// Hot authenticates before it composes a Resolution invocation.
    ///
    /// This route had no test and no reachable caller for six days, and in that
    /// window it went unsatisfiable: `017033a2` wrote it against
    /// `set_v1`/`v3::CapabilityProgramV3` at 02:09 on 2026-08-26, `f99b5334`
    /// introduced the schema-bound successors at 07:02, and `50ce684b` moved
    /// Trading's own selection onto them at 07:45 without it. Both halves of
    /// the old check were then impossible, and the second one provably so: the
    /// digest a Trading caller carries is `hash` of a 600-byte descriptor, and
    /// the check joined it to an expected width of 408.
    #[test]
    fn a_trading_caller_authenticates_the_schema_bound_records_common_hot_produces() {
        let registry = Pubkey::new_from_array([0xd7; 32]);
        let descriptor_bytes = trading_descriptor(PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3);
        let descriptor_id = hash(&descriptor_bytes).to_bytes();
        let descriptor_schema =
            CapabilityContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).expect("V4 schema");
        let set_bytes = trading_program_set(&[
            CapabilityProgramSetEntryV2::new(
                3,
                CapabilityDescriptorReferenceV2::new(descriptor_schema, capability_content(0x31)),
            ),
            CapabilityProgramSetEntryV2::new(
                9,
                CapabilityDescriptorReferenceV2::new(
                    descriptor_schema,
                    CapabilityContentId::new(descriptor_id).expect("descriptor identity"),
                ),
            ),
        ]);
        let set_id = hash(&set_bytes).to_bytes();
        let (set_raw, set_staging) = finalized_record(
            &registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            &set_bytes,
        );
        let (descriptor_raw, descriptor_staging) = finalized_record(
            &registry,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            &descriptor_bytes,
        );

        assert_eq!(
            authenticate_trading_capability_records(
                &registry,
                &set_raw,
                &set_staging,
                &descriptor_raw,
                &descriptor_staging,
                &trading_request(set_id, descriptor_id),
            ),
            Ok(()),
            "the exact set and descriptor common Hot authenticates are the ones this route admits",
        );

        // The membership conjunct: a descriptor this authenticated set does not
        // admit is refused even though its own record authenticates perfectly.
        let stranger_bytes = trading_descriptor([0x63; 32]);
        let stranger_id = hash(&stranger_bytes).to_bytes();
        let (stranger_raw, stranger_staging) =
            finalized_record(&registry, CAPABILITY_PROGRAM_SCHEMA_ID_V4, &stranger_bytes);
        assert_eq!(
            authenticate_trading_capability_records(
                &registry,
                &set_raw,
                &set_staging,
                &stranger_raw,
                &stranger_staging,
                &trading_request(set_id, stranger_id),
            ),
            Err(ProgramError::Custom(
                ResolutionError::FinalizedRecord as u32
            )),
            "a descriptor outside the Market-selected set is not a Trading capability",
        );

        // The request-schema conjunct: a descriptor the set does admit, whose
        // request schema is some other family's, is not a provider request.
        let wrong_schema_bytes = trading_descriptor([0x64; 32]);
        let wrong_schema_id = hash(&wrong_schema_bytes).to_bytes();
        let wrong_schema_set = trading_program_set(&[CapabilityProgramSetEntryV2::new(
            9,
            CapabilityDescriptorReferenceV2::new(
                descriptor_schema,
                CapabilityContentId::new(wrong_schema_id).expect("descriptor identity"),
            ),
        )]);
        let wrong_schema_set_id = hash(&wrong_schema_set).to_bytes();
        let (wrong_set_raw, wrong_set_staging) = finalized_record(
            &registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            &wrong_schema_set,
        );
        let (wrong_raw, wrong_staging) = finalized_record(
            &registry,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            &wrong_schema_bytes,
        );
        assert_eq!(
            authenticate_trading_capability_records(
                &registry,
                &wrong_set_raw,
                &wrong_set_staging,
                &wrong_raw,
                &wrong_staging,
                &trading_request(wrong_schema_set_id, wrong_schema_id),
            ),
            Err(ProgramError::Custom(
                ResolutionError::FinalizedRecord as u32
            )),
            "only a descriptor whose request schema is the provider request is executable here",
        );
    }

    /// Why the superseded labels could not have been a stricter check.
    ///
    /// The schema is a SEED of the finalized record's address, so authenticating
    /// the same bytes under a different schema label does not fail a comparison,
    /// it looks at a different account entirely; and the descriptor half is
    /// second-preimage hard, not merely wrong.
    #[test]
    fn the_superseded_capability_labels_address_other_accounts_entirely() {
        let registry = Pubkey::new_from_array([0xd8; 32]);
        let set_bytes = trading_program_set(&[CapabilityProgramSetEntryV2::new(
            9,
            CapabilityDescriptorReferenceV2::new(
                CapabilityContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).expect("V4 schema"),
                capability_content(0x33),
            ),
        )]);
        let digest = hash(&set_bytes).to_bytes();
        let under = |schema: [u8; 32]| {
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0
        };
        assert_ne!(
            under(CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2),
            under(dclutch_market::capability_program::set_v1::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1),
        );
        assert_eq!(
            dclutch_market::capability_program::v3::CAPABILITY_PROGRAM_V3_BYTES,
            408,
        );
        assert_eq!(CAPABILITY_PROGRAM_V4_BYTES, 600);
    }

    #[test]
    fn discriminator_requires_a_distinct_exact_post_body_suffix() {
        let mut prefix = [0_u8; PROVIDER_EXECUTION_REQUEST_BYTES_V3];
        prefix[..8].copy_from_slice(&PROVIDER_EXECUTION_REQUEST_MAGIC_V3);
        assert!(!is_provider_resolution_v3(&prefix));
        let mut complete = std::vec::Vec::from(prefix);
        complete.extend_from_slice(
            CAPTURED_POST_UPDATE
                .get(8..)
                .expect("captured Receiver instruction body"),
        );
        assert!(is_provider_resolution_v3(&complete));
        assert!(
            PostUpdateParamsView::parse(
                complete
                    .get(PROVIDER_EXECUTION_REQUEST_BYTES_V3..)
                    .expect("provider body suffix"),
            )
            .is_ok()
        );
        assert!(PostUpdateParamsView::parse(CAPTURED_POST_UPDATE).is_err());
    }

    #[test]
    fn frozen_frame_has_one_four_account_trading_extension() {
        assert_eq!(PROVIDER_RESOLUTION_CORE_TAIL_START_V3, 38);
        assert_eq!(PROVIDER_RESOLUTION_TRADING_TAIL_START_V3, 42);
        assert_eq!(PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3, 47);
        assert_eq!(PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3, 51);
        assert_eq!(
            PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3
                - PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3,
            4,
        );
    }

    #[test]
    fn only_the_resolution_and_active_caller_deployments_execute() {
        for (caller, active, inactive) in [
            (
                ProviderCallerV3::Core,
                ExecutionRoleV1::Core,
                ExecutionRoleV1::Trading,
            ),
            (
                ProviderCallerV3::Trading,
                ExecutionRoleV1::Trading,
                ExecutionRoleV1::Core,
            ),
        ] {
            assert!(caller_executes_role(caller, ExecutionRoleV1::Resolution));
            assert!(caller_executes_role(caller, active));
            assert!(!caller_executes_role(caller, inactive));
            assert!(!caller_executes_role(caller, ExecutionRoleV1::Claims));
            assert!(!caller_executes_role(caller, ExecutionRoleV1::Custody));
        }
    }

    #[test]
    fn finalized_source_and_request_schema_ids_match_their_labels() {
        for (preimage, expected) in [
            (
                SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V3,
                SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            ),
            (SOURCE_SPEC_SCHEMA_PREIMAGE_V1, SOURCE_SPEC_SCHEMA_ID_V1),
            (
                PROVIDER_RELEASE_SCHEMA_PREIMAGE_V1,
                PROVIDER_RELEASE_SCHEMA_ID_V1,
            ),
            (
                PYTH_ADAPTER_CONFIG_SCHEMA_PREIMAGE_V1,
                PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            ),
            (WINDOW_SPEC_SCHEMA_PREIMAGE_V1, WINDOW_SPEC_SCHEMA_ID_V1),
            (
                STATISTIC_SPEC_SCHEMA_PREIMAGE_V1,
                STATISTIC_SPEC_SCHEMA_ID_V1,
            ),
            (
                PROVIDER_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3,
                PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3,
            ),
        ] {
            assert_eq!(hash(preimage).to_bytes(), expected);
        }
    }

    #[test]
    fn source_material_v3_is_exact_width_and_refuses_hostile_schema_bytes() {
        let id = |tag| SourceContentId::new([tag; 32]).expect("nonzero fixture identity");
        let material =
            SourceMaterialV3::explicitly_unbounded(id(1), id(2), id(3), id(4), None, id(5));
        let exact = material.to_bytes();
        assert_eq!(SOURCE_MATERIAL_V3_BYTES, 240);
        assert_eq!(exact.len(), SOURCE_MATERIAL_V3_BYTES);
        assert_eq!(SourceMaterialV3::decode(&exact), Ok(material));
        assert!(
            SourceMaterialV3::decode(
                exact
                    .get(..SOURCE_MATERIAL_V3_BYTES - 1)
                    .expect("short hostile material"),
            )
            .is_err()
        );
        let mut long = std::vec::Vec::from(exact);
        long.push(0);
        assert!(SourceMaterialV3::decode(&long).is_err());

        let mut wrong_magic = exact;
        wrong_magic[0] ^= 1;
        assert!(SourceMaterialV3::decode(&wrong_magic).is_err());
        let mut wrong_schema = exact;
        wrong_schema[SOURCE_MATERIAL_V3_MAGIC.len()] ^= 1;
        assert!(SourceMaterialV3::decode(&wrong_schema).is_err());
    }
}
