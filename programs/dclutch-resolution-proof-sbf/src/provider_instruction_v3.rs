//! Physical current-release route consuming one real provider-owned update.
//!
//! This instruction never submits or reclaims a Pyth update. Provider posting
//! and reclaiming remain distinct Receiver/router instructions with their own
//! signers and CPI account frames. This route consumes an already-posted,
//! Receiver-owned fully verified update and persists only dClutch Source state
//! and its deterministic terminal certificate.

use core::cell::Ref;

use alloc::boxed::Box;

use dclutch_capability_program_contract::{
    set_v1::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1, CapabilityProgramSetV1},
    v3::{CapabilityProgramV3, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V3},
};
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, Readiness as CoreReadiness,
};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV2, authenticate_product_runtime_v2,
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, PostUpdateParamsView, PythReleaseV1, ReceiverConfigV2View,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    CallerAuthoritySeedsV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use dclutch_resolution_codec::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, PROVIDER_EXECUTION_REQUEST_MAGIC_V3,
    PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3, PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3,
    PROVIDER_RESOLUTION_CORE_TAIL_START_V3, PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3,
    PROVIDER_RESOLUTION_TRADING_TAIL_START_V3, PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderCallerV3, ProviderExecutionRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1,
    PYTH_ADAPTER_CONFIG_BYTES, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_MATERIAL_V2_BYTES, SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_BYTES, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV2,
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

use crate::{
    ResolutionError, authenticate_clock, authenticate_rent, deployment_observation,
    provider_v3::{
        AuthenticatedProviderObservationV3, AuthenticatedSourceRecordsV3, ProviderJoinErrorV3,
        plan_provider_resolution_v3,
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
    let tail_start = match request.caller {
        ProviderCallerV3::Core => {
            if accounts.len() != PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3 {
                return Err(ResolutionError::AccountFrame.into());
            }
            PROVIDER_RESOLUTION_CORE_TAIL_START_V3
        }
        ProviderCallerV3::Trading => {
            if accounts.len() != PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3 {
                return Err(ResolutionError::AccountFrame.into());
            }
            PROVIDER_RESOLUTION_TRADING_TAIL_START_V3
        }
    };
    authenticate_privileges(program_id, accounts, tail_start)?;
    let frame = ProviderFrameV3 {
        accounts,
        tail_start,
    };
    authenticate_request_accounts(&request, frame)?;

    let rent = authenticate_rent(frame.rent())?;
    let clock = authenticate_clock(frame.clock())?;
    let market = authenticate_market_and_infrastructure(program_id, &request, frame, &rent)?;
    authenticate_activation_and_caller(program_id, &request, request_bytes, frame)?;
    let source_records = boxed_source_records(&request, frame, &rent)?;
    let product_runtime = boxed_product_runtime(&request, frame, &rent)?;
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

    let pyth_release = boxed_pyth_release(&request, frame, &rent)?;
    let update_data = frame
        .update()
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let lifecycle = boxed_provider_lifecycle(program_id, &request, frame, &rent, &update_data)?;
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
    let plan = plan_provider_resolution_v3(request_bytes, &source, &source_records, &observation)
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
        ProviderJoinErrorV3::Product => ResolutionError::ProductDomain,
        ProviderJoinErrorV3::Provider => ResolutionError::ProviderObservation,
        ProviderJoinErrorV3::Transition => ResolutionError::Transition,
        ProviderJoinErrorV3::Arithmetic => ResolutionError::Arithmetic,
    }
}

fn boxed_source_records(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    rent: &Rent,
) -> Result<Box<AuthenticatedSourceRecordsV3>, ProgramError> {
    Ok(Box::new(authenticate_source_records(request, frame, rent)?))
}

fn boxed_product_runtime(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    rent: &Rent,
) -> Result<Box<dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2>, ProgramError>
{
    Ok(Box::new(
        authenticate_product_runtime_v2(
            frame.registry_program().key,
            rent,
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
    rent: &Rent,
) -> Result<Box<PythReleaseV1>, ProgramError> {
    Ok(Box::new(authenticate_pyth_release(request, frame, rent)?))
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
    rent: &Rent,
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
        || !rent.is_exempt(lifecycle_account.lamports(), lifecycle_data.len())
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
    product_runtime: &dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2,
    result_domain_bytes: &'a [u8],
    update_bytes: &'a [u8],
    lifecycle: &ProviderUpdateLifecycleV3,
    post_body: &'a [u8],
    clock: solana_program::clock::Clock,
) -> Result<Box<AuthenticatedProviderObservationV3<'a>>, ProgramError> {
    FullPriceUpdateV2::parse(update_bytes).map_err(|_| ResolutionError::ProviderObservation)?;
    let result_domain = dclutch_product_runtime_v2::ResultDomainV2::decode(result_domain_bytes)
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
    rent: &Rent,
) -> Result<CoreState, ProgramError> {
    let market_account = frame.account(4);
    if market_account.owner != frame.account(11).key || market_account.executable {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let market = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    if market.phase != CorePhase::Open
        || market.readiness != CoreReadiness::Consumed
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        frame.account(11).key,
    )
    .0;
    let infrastructure_data = infrastructure
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if infrastructure.key != &expected_infrastructure
        || infrastructure.owner != frame.account(11).key
        || infrastructure.executable
        || infrastructure_data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || !rent.is_exempt(infrastructure.lamports(), infrastructure_data.len())
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let profile = ProtocolInfrastructureProfileV1::decode(&infrastructure_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    drop(infrastructure_data);
    if profile.registry().program().to_bytes() != frame.registry_program().key.to_bytes() {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let artifact_data = frame
        .account(9)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.registry_program().key,
        frame.account(9),
        frame.account(10),
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        profile.registry().artifact_release().to_bytes(),
        &artifact_data,
        ARTIFACT_RELEASE_BYTES_V1,
    )?;
    let artifact = ArtifactReleaseV1::decode(&artifact_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || artifact.program().to_bytes() != frame.registry_program().key.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let observation = deployment_observation(
        frame.registry_program(),
        frame.account(8),
        artifact.programdata(),
    )?;
    artifact
        .authenticate_deployment(observation)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    if frame.account(15).key != program_id {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    Ok(market)
}

fn authenticate_activation_and_caller(
    program_id: &Pubkey,
    request: &ProviderExecutionRequestV3,
    request_bytes: &[u8],
    frame: ProviderFrameV3<'_, '_>,
) -> ProgramResult {
    let activation_data = frame
        .account(5)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if frame.account(5).owner != frame.registry_program().key
        || frame.account(5).executable
        || activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
            frame.registry_program().key,
        )
        .0 != *frame.account(5).key
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activation = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?
        .to_bytes()
        != request.release_set
    {
        return Err(ResolutionError::ResolutionRelease.into());
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
        let activated = activation
            .role(role)
            .map_err(|_| ResolutionError::ResolutionRelease)?;
        if activated.release().program().to_bytes() != program.key.to_bytes() {
            return Err(ResolutionError::ResolutionRelease.into());
        }
        if role == ExecutionRoleV1::Resolution
            && (program.key != program_id
                || activated.release().semantic_release_id().to_bytes()
                    != RESOLUTION_CONTROLLER_RELEASE_ID_V4)
        {
            return Err(ResolutionError::ResolutionRelease.into());
        }
        if caller_executes_role(request.caller, role) {
            let observation =
                deployment_observation(program, programdata, activated.release().programdata())?;
            activated
                .authenticate_current_deployment(observation)
                .map_err(|_| ResolutionError::ResolutionDeployment)?;
        }
    }
    let caller_role = match request.caller {
        ProviderCallerV3::Core => ExecutionRoleV1::Core,
        ProviderCallerV3::Trading => ExecutionRoleV1::Trading,
    };
    let caller_program = match request.caller {
        ProviderCallerV3::Core => frame.account(11),
        ProviderCallerV3::Trading => frame.account(13),
    };
    if request.caller_program != caller_program.key.to_bytes() {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        caller_role,
        request.source_state,
        request.parent_request_digest,
    )
    .map_err(|_| ResolutionError::ResolutionRelease)?;
    if Pubkey::find_program_address(&seeds.as_slices(), caller_program.key).0
        != *frame.account(0).key
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    if request.caller == ProviderCallerV3::Trading {
        let set_data = frame
            .account(37)
            .try_borrow_data()
            .map_err(|_| ResolutionError::FinalizedRecord)?;
        authenticate_record(
            frame.registry_program().key,
            frame.account(37),
            frame.account(38),
            &authenticate_rent(frame.rent())?,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1,
            request.capability_program_set,
            &set_data,
            set_data.len(),
        )?;
        let set = CapabilityProgramSetV1::decode_selected(
            request.capability_program_set,
            hash(&set_data).to_bytes(),
            &set_data,
        )
        .map_err(|_| ResolutionError::FinalizedRecord)?;
        if set
            .select(request_bytes)
            .map_err(|_| ResolutionError::FinalizedRecord)?
            .to_bytes()
            != request.selected_capability_program
        {
            return Err(ResolutionError::FinalizedRecord.into());
        }
        let descriptor_data = frame
            .account(39)
            .try_borrow_data()
            .map_err(|_| ResolutionError::FinalizedRecord)?;
        authenticate_record(
            frame.registry_program().key,
            frame.account(39),
            frame.account(40),
            &authenticate_rent(frame.rent())?,
            CAPABILITY_PROGRAM_SCHEMA_ID_V3,
            request.selected_capability_program,
            &descriptor_data,
            dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES,
        )?;
        let descriptor = CapabilityProgramV3::decode(&descriptor_data)
            .map_err(|_| ResolutionError::FinalizedRecord)?;
        if descriptor.request_schema().to_bytes() != PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3 {
            return Err(ResolutionError::FinalizedRecord.into());
        }
    }
    Ok(())
}

const fn caller_executes_role(caller: ProviderCallerV3, role: ExecutionRoleV1) -> bool {
    matches!(role, ExecutionRoleV1::Resolution)
        || matches!(
            (caller, role),
            (ProviderCallerV3::Core, ExecutionRoleV1::Core)
                | (ProviderCallerV3::Trading, ExecutionRoleV1::Trading)
        )
}

fn authenticate_source_records(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    rent: &Rent,
) -> Result<AuthenticatedSourceRecordsV3, ProgramError> {
    let material_data = borrow_record(
        frame,
        rent,
        17,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        request.source_material,
        SOURCE_MATERIAL_V2_BYTES,
    )?;
    let material =
        SourceMaterialV2::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let source_data = borrow_record(
        frame,
        rent,
        19,
        SOURCE_SPEC_SCHEMA_ID_V1,
        request.source_spec,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&source_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let provider_id = source.provider_release_id();
    let provider_data = borrow_record(
        frame,
        rent,
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
        rent,
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
        rent,
        25,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_id.to_bytes(),
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let statistic_id = material.statistic_spec();
    let statistic_data = borrow_record(
        frame,
        rent,
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

fn authenticate_pyth_release(
    request: &ProviderExecutionRequestV3,
    frame: ProviderFrameV3<'_, '_>,
    rent: &Rent,
) -> Result<PythReleaseV1, ProgramError> {
    let bytes = borrow_record(
        frame,
        rent,
        29,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        request.provider_release,
        dclutch_pyth_svm::PYTH_RELEASE_V1_ENCODED_LEN,
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
        || !rent.is_exempt(frame.receiver_config().lamports(), config_data.len())
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
    rent: &Rent,
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
        rent,
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
    rent: &Rent,
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
        || !rent.is_exempt(raw.lamports(), bytes.len())
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
    use dclutch_resolution_codec::PROVIDER_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3;
    use dclutch_source_contract::{
        PROVIDER_RELEASE_SCHEMA_PREIMAGE_V1, PYTH_ADAPTER_CONFIG_SCHEMA_PREIMAGE_V1,
        SOURCE_SPEC_SCHEMA_PREIMAGE_V1, STATISTIC_SPEC_SCHEMA_PREIMAGE_V1,
        WINDOW_SPEC_SCHEMA_PREIMAGE_V1,
    };

    use super::*;

    const CAPTURED_POST_UPDATE: &[u8] = include_bytes!(
        "../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
    );

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
}
