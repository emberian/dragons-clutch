//! Real Receiver/router submission and permissionless reclaim under Resolution custody.

use alloc::{boxed::Box, vec::Vec};

use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, Readiness as CoreReadiness,
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, GuardianSetV1, PostUpdateParamsView, PythReleaseV1, ReceiverConfigV2View,
    VerifiedEncodedVaaV1,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use dclutch_resolution_codec::{
    PROVIDER_RECLAIM_REQUEST_BYTES_V3, PROVIDER_RECLAIM_REQUEST_MAGIC_V3,
    PROVIDER_SUBMIT_REQUEST_BYTES_V3, PROVIDER_SUBMIT_REQUEST_MAGIC_V3,
    PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3, PROVIDER_UPDATE_LIFECYCLE_BYTES_V3,
    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
    ProviderReclaimReceiptV3, ProviderReclaimRequestV3, ProviderSubmitReceiptV3,
    ProviderSubmitRequestV3, ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateV2,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_MATERIAL_V2_BYTES, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1,
    WINDOW_SPEC_BYTES, WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    ResolutionError, authenticate_clock, authenticate_rent, deployment_observation,
    provider_instruction_v3::{authenticate_provider_program, authenticate_record},
};

/// Frozen real-provider submission account count.
pub const PROVIDER_SUBMIT_ACCOUNT_COUNT_V3: usize = 38;
/// Frozen permissionless provider reclaim account count.
pub const PROVIDER_RECLAIM_ACCOUNT_COUNT_V3: usize = 18;

const POST_UPDATE_DISCRIMINATOR: [u8; 8] = [133, 95, 207, 175, 11, 79, 118, 44];
const RECLAIM_RENT_DISCRIMINATOR: [u8; 8] = [218, 200, 19, 197, 227, 89, 192, 22];

/// Return whether bytes select one real provider transport route.
pub(crate) fn is_provider_transport_v3(bytes: &[u8]) -> bool {
    matches!(bytes.get(..8), Some(magic) if magic == PROVIDER_SUBMIT_REQUEST_MAGIC_V3 || magic == PROVIDER_RECLAIM_REQUEST_MAGIC_V3)
}

/// Dispatch exact provider submission or permissionless reclaim.
#[inline(never)]
pub(crate) fn process_provider_transport_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.get(..8) {
        Some(magic) if magic == PROVIDER_SUBMIT_REQUEST_MAGIC_V3 => {
            process_submit(program_id, accounts, instruction_data)
        }
        Some(magic) if magic == PROVIDER_RECLAIM_REQUEST_MAGIC_V3 => {
            process_reclaim(program_id, accounts, instruction_data)
        }
        _ => Err(ResolutionError::Instruction.into()),
    }
}

#[inline(never)]
fn process_submit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != PROVIDER_SUBMIT_ACCOUNT_COUNT_V3
        || instruction_data.len() <= PROVIDER_SUBMIT_REQUEST_BYTES_V3
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let request_bytes = instruction_data
        .get(..PROVIDER_SUBMIT_REQUEST_BYTES_V3)
        .ok_or(ResolutionError::Instruction)?;
    let post_body = instruction_data
        .get(PROVIDER_SUBMIT_REQUEST_BYTES_V3..)
        .ok_or(ResolutionError::Instruction)?;
    let request = Box::new(
        ProviderSubmitRequestV3::decode(request_bytes).map_err(|_| ResolutionError::Instruction)?,
    );
    PostUpdateParamsView::parse(post_body).map_err(|_| ResolutionError::ProviderObservation)?;
    authenticate_submit_privileges(program_id, accounts)?;
    let frame = SubmitFrameV3 { accounts };
    if frame.account(0).key.to_bytes() != request.provider_submitter
        || frame.account(1).key.to_bytes() != request.update_account
        || frame.account(2).key.to_bytes() != request.lifecycle
        || frame.account(4).key.to_bytes() != request.refund_recipient
        || frame.account(5).key.to_bytes() != request.market
        || frame.account(16).key.to_bytes() != request.source_state
        || frame.account(32).key.to_bytes() != request.encoded_vaa
        || hash(post_body).to_bytes() != request.post_body_digest
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let rent = authenticate_rent(frame.account(36))?;
    let clock = authenticate_clock(frame.account(35))?;
    authenticate_current_submission(program_id, &request, frame, &rent)?;
    let (release, config) = authenticate_submission_records(&request, frame, &rent)?;
    if release.activation_time() > clock.unix_timestamp {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let authority =
        authenticate_submission_provider(program_id, &request, frame, &rent, release, config)?;
    preflight_submit_outputs(program_id, &request, frame, &rent, authority)?;

    let submitter_before = frame.account(0).lamports();
    let treasury_before = frame.account(34).lamports();
    invoke_post(frame, post_body, &request, authority)?;
    let update_data = frame
        .account(1)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update =
        FullPriceUpdateV2::parse(&update_data).map_err(|_| ResolutionError::ProviderObservation)?;
    let update_digest = hash(&update_data).to_bytes();
    let update_rent = frame.account(1).lamports();
    let expected_update_rent = rent.minimum_balance(update_data.len());
    let fee = config.fee;
    let expected_submitter = submitter_before
        .checked_sub(
            update_rent
                .checked_add(fee)
                .ok_or(ResolutionError::Arithmetic)?,
        )
        .ok_or(ResolutionError::Arithmetic)?;
    let expected_treasury = treasury_before
        .checked_add(fee)
        .ok_or(ResolutionError::Arithmetic)?;
    if frame.account(1).owner != frame.account(27).key
        || update.write_authority() != authority.to_bytes()
        || update.posted_slot() == 0
        || update.posted_slot() > clock.slot
        || update.publish_time() <= 0
        || request.reclaim_after_unix_seconds < update.publish_time()
        || update_rent != expected_update_rent
        || frame.account(0).lamports() != expected_submitter
        || frame.account(34).lamports() != expected_treasury
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    drop(update_data);
    let lifecycle = ProviderUpdateLifecycleV3::submitted(
        *request,
        lifecycle_bump(program_id, frame.account(1).key, frame.account(2).key)?,
        authority.to_bytes(),
        frame.account(8).key.to_bytes(),
        update_digest,
        update.publish_time(),
        update.posted_slot(),
        update_rent,
        fee,
    )
    .map_err(|_| ResolutionError::Transition)?;
    initialize_lifecycle(program_id, frame, &rent, lifecycle)?;
    let receipt = ProviderSubmitReceiptV3 {
        request_digest: hash(request_bytes).to_bytes(),
        lifecycle: request.lifecycle,
        update_account: request.update_account,
        update_digest,
        provider_submitter: request.provider_submitter,
        update_authority: authority.to_bytes(),
        refund_recipient: request.refund_recipient,
        provider_release: request.provider_release,
        post_body_digest: request.post_body_digest,
        market: request.market,
        generation: request.generation,
        posted_slot: update.posted_slot(),
        publish_time: update.publish_time(),
        update_rent_lamports: update_rent,
        provider_fee_lamports: fee,
    }
    .to_bytes()
    .map_err(|_| ResolutionError::Transition)?;
    set_return_data(&receipt);
    Ok(())
}

#[inline(never)]
fn process_reclaim(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != PROVIDER_RECLAIM_ACCOUNT_COUNT_V3
        || instruction_data.len() != PROVIDER_RECLAIM_REQUEST_BYTES_V3
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let request = Box::new(
        ProviderReclaimRequestV3::decode(instruction_data)
            .map_err(|_| ResolutionError::Instruction)?,
    );
    authenticate_reclaim_privileges(program_id, accounts)?;
    let frame = ReclaimFrameV3 { accounts };
    if frame.account(0).key.to_bytes() != request.resolver
        || frame.account(1).key.to_bytes() != request.lifecycle
        || frame.account(2).key.to_bytes() != request.update_account
        || frame.account(4).key.to_bytes() != request.refund_recipient
        || frame.account(5).key.to_bytes() != request.certificate
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let rent = authenticate_rent(frame.account(16))?;
    let clock = authenticate_clock(frame.account(15))?;
    let lifecycle =
        authenticate_reclaim_state(program_id, &request, frame, &rent, clock.unix_timestamp)?;
    authenticate_reclaim_release(program_id, &request, frame, &rent, lifecycle)?;

    let authority_before = frame.account(3).lamports();
    let refund_before = frame.account(4).lamports();
    if authority_before != 0
        || frame.account(3).owner != &system_program::ID
        || frame.account(3).data_len() != 0
    {
        return Err(ResolutionError::OutputState.into());
    }
    invoke_reclaim(program_id, frame, lifecycle)?;
    if frame.account(2).lamports() != 0
        || frame.account(2).owner != &system_program::ID
        || frame.account(2).data_len() != 0
        || frame.account(3).lamports() != lifecycle.update_rent_lamports
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    transfer_reclaimed_rent(program_id, frame, lifecycle)?;
    let lifecycle_lamports = frame.account(1).lamports();
    let total_refund = lifecycle
        .update_rent_lamports
        .checked_add(lifecycle_lamports)
        .ok_or(ResolutionError::Arithmetic)?;
    close_lifecycle(frame.account(1), frame.account(4), lifecycle_lamports)?;
    if frame.account(3).lamports() != 0
        || frame.account(4).lamports()
            != refund_before
                .checked_add(total_refund)
                .ok_or(ResolutionError::Arithmetic)?
    {
        return Err(ResolutionError::OutputState.into());
    }
    let receipt = ProviderReclaimReceiptV3 {
        request_digest: hash(instruction_data).to_bytes(),
        lifecycle: request.lifecycle,
        update_account: request.update_account,
        certificate: request.certificate,
        resolver: request.resolver,
        refund_recipient: request.refund_recipient,
        update_digest: lifecycle.update_digest,
        provider_evidence: lifecycle.provider_evidence,
        generation: request.generation,
        terminal_sequence: request.terminal_sequence,
        refunded_lamports: lifecycle.update_rent_lamports,
    }
    .to_bytes()
    .map_err(|_| ResolutionError::Transition)?;
    set_return_data(&receipt);
    Ok(())
}

#[derive(Clone, Copy)]
struct SubmitFrameV3<'accounts, 'info> {
    accounts: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> SubmitFrameV3<'accounts, 'info> {
    #[allow(clippy::indexing_slicing)]
    fn account(self, index: usize) -> &'accounts AccountInfo<'info> {
        &self.accounts[index]
    }
}

#[derive(Clone, Copy)]
struct ReclaimFrameV3<'accounts, 'info> {
    accounts: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> ReclaimFrameV3<'accounts, 'info> {
    #[allow(clippy::indexing_slicing)]
    fn account(self, index: usize) -> &'accounts AccountInfo<'info> {
        &self.accounts[index]
    }
}

fn authenticate_submit_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        let executable = matches!(index, 8 | 12 | 14 | 27 | 30 | 37);
        if account.is_signer != matches!(index, 0 | 1)
            || account.is_writable != matches!(index, 0 | 1 | 2 | 34)
            || account.executable != executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    if accounts.get(14).ok_or(ResolutionError::AccountFrame)?.key != program_id
        || accounts.get(37).ok_or(ResolutionError::AccountFrame)?.key != &system_program::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_reclaim_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        let executable = matches!(index, 7 | 9 | 13 | 17);
        if account.is_signer != (index == 0)
            || account.is_writable != matches!(index, 1..=4)
            || account.executable != executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    if accounts.get(9).ok_or(ResolutionError::AccountFrame)?.key != program_id
        || accounts.get(17).ok_or(ResolutionError::AccountFrame)?.key != &system_program::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_current_submission(
    program_id: &Pubkey,
    request: &ProviderSubmitRequestV3,
    frame: SubmitFrameV3<'_, '_>,
    rent: &Rent,
) -> ProgramResult {
    let market_data = frame
        .account(5)
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let market = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    if frame.account(5).owner != frame.account(12).key
        || market.phase != CorePhase::Open
        || market.readiness != CoreReadiness::Consumed
        || market.identity.market_id.to_bytes() != request.market
        || market.identity.generation != request.generation
        || market.identity.registry_program.to_bytes() != frame.account(8).key.to_bytes()
        || market.identity.resolution_policy.to_bytes() != request.source_material
        || market.identity.selected_release_set.to_bytes() != request.release_set
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
            frame.account(12).key,
        )
        .0 != *frame.account(5).key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    drop(market_data);
    let source_data = frame
        .account(16)
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source =
        SourceResolutionStateV2::decode(&source_data).map_err(|_| ResolutionError::OutputState)?;
    if frame.account(16).owner != program_id
        || source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(ResolutionError::OutputState.into());
    }
    drop(source_data);
    authenticate_infrastructure(
        program_id,
        request.release_set,
        frame.account(7),
        frame.account(8),
        frame.account(9),
        frame.account(10),
        frame.account(11),
        frame.account(12),
        frame.account(13),
        frame.account(14),
        frame.account(15),
        frame.account(6),
        rent,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_infrastructure<'info>(
    program_id: &Pubkey,
    release_set: [u8; 32],
    infrastructure: &AccountInfo<'info>,
    registry: &AccountInfo<'info>,
    registry_programdata: &AccountInfo<'info>,
    registry_artifact: &AccountInfo<'info>,
    registry_staging: &AccountInfo<'info>,
    core: &AccountInfo<'info>,
    core_programdata: &AccountInfo<'info>,
    resolution: &AccountInfo<'info>,
    resolution_programdata: &AccountInfo<'info>,
    activation: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let profile_data = infrastructure
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if infrastructure.owner != core.key
        || infrastructure.key
            != &Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                core.key,
            )
            .0
        || profile_data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || !rent.is_exempt(infrastructure.lamports(), profile_data.len())
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let profile = ProtocolInfrastructureProfileV1::decode(&profile_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if profile.registry().program().to_bytes() != registry.key.to_bytes() {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    drop(profile_data);
    let artifact_data = registry_artifact
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry.key,
        registry_artifact,
        registry_staging,
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        profile.registry().artifact_release().to_bytes(),
        &artifact_data,
        ARTIFACT_RELEASE_BYTES_V1,
    )?;
    let artifact = ArtifactReleaseV1::decode(&artifact_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || artifact.program().to_bytes() != registry.key.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    artifact
        .authenticate_deployment(deployment_observation(
            registry,
            registry_programdata,
            artifact.programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let activation_data = activation
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation.owner != registry.key
        || activation.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || activation.key
            != &Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
                registry.key,
            )
            .0
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?
        .to_bytes()
        != release_set
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    for (role, role_program, programdata) in [
        (ExecutionRoleV1::Core, core, core_programdata),
        (
            ExecutionRoleV1::Resolution,
            resolution,
            resolution_programdata,
        ),
    ] {
        let selected = activated
            .role(role)
            .map_err(|_| ResolutionError::ResolutionRelease)?;
        if selected.release().program().to_bytes() != role_program.key.to_bytes()
            || (role == ExecutionRoleV1::Resolution
                && (role_program.key != program_id
                    || selected.release().semantic_release_id().to_bytes()
                        != RESOLUTION_CONTROLLER_RELEASE_ID_V4))
        {
            return Err(ResolutionError::ResolutionRelease.into());
        }
        selected
            .authenticate_current_deployment(deployment_observation(
                role_program,
                programdata,
                selected.release().programdata(),
            )?)
            .map_err(|_| ResolutionError::ResolutionDeployment)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ReceiverConfigFactsV3 {
    fee: u64,
    router: [u8; 32],
}

fn authenticate_submission_records(
    request: &ProviderSubmitRequestV3,
    frame: SubmitFrameV3<'_, '_>,
    rent: &Rent,
) -> Result<(PythReleaseV1, ReceiverConfigFactsV3), ProgramError> {
    let material_data = frame
        .account(17)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(8).key,
        frame.account(17),
        frame.account(18),
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        request.source_material,
        &material_data,
        SOURCE_MATERIAL_V2_BYTES,
    )?;
    let material =
        SourceMaterialV2::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(material_data);
    let source_data = frame
        .account(19)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(8).key,
        frame.account(19),
        frame.account(20),
        rent,
        SOURCE_SPEC_SCHEMA_ID_V1,
        material.primary_source_spec().to_bytes(),
        &source_data,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&source_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(source_data);
    let provider_data = frame
        .account(21)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(8).key,
        frame.account(21),
        frame.account(22),
        rent,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        source.provider_release_id().to_bytes(),
        &provider_data,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider = dclutch_source_contract::ProviderReleaseV1::decode(&provider_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    drop(provider_data);
    let release_data = frame
        .account(23)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(8).key,
        frame.account(23),
        frame.account(24),
        rent,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        request.provider_release,
        &release_data,
        dclutch_pyth_svm::PYTH_RELEASE_V1_ENCODED_LEN,
    )?;
    let release =
        PythReleaseV1::decode(&release_data).map_err(|_| ResolutionError::ProviderRelease)?;
    drop(release_data);
    let window_data = frame
        .account(25)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(8).key,
        frame.account(25),
        frame.account(26),
        rent,
        WINDOW_SPEC_SCHEMA_ID_V1,
        material.window_spec().to_bytes(),
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);
    if source.access_profile() != SourceAccessProfile::PythTerminalOneTransaction
        || provider.provider_deployment_release_id().to_bytes() != request.provider_release
        || window.source_spec_id() != material.primary_source_spec()
        || request.reclaim_after_unix_seconds < window.end_unix_seconds()
    {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let config_data = frame
        .account(29)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if hash(&config_data).to_bytes() != release.config_digest() {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let config = ReceiverConfigV2View::parse(&config_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    // The view borrows account data, so return only a same-layout static copy is impossible.
    // Callers need only its copied fee/router facts; reconstruct from pinned bytes below.
    let fee = config.fee();
    let router = config.router_program();
    drop(config_data);
    if router != release.router_program() {
        return Err(ResolutionError::ProviderObservation.into());
    }
    Ok((release, ReceiverConfigFactsV3 { fee, router }))
}

fn authenticate_submission_provider(
    program_id: &Pubkey,
    request: &ProviderSubmitRequestV3,
    frame: SubmitFrameV3<'_, '_>,
    rent: &Rent,
    release: PythReleaseV1,
    config: ReceiverConfigFactsV3,
) -> Result<Pubkey, ProgramError> {
    if request.registry_program != frame.account(8).key.to_bytes()
        || release.receiver_program() != frame.account(27).key.to_bytes()
        || release.receiver_programdata() != frame.account(28).key.to_bytes()
        || release.receiver_config() != frame.account(29).key.to_bytes()
        || release.router_program() != frame.account(30).key.to_bytes()
        || release.router_programdata() != frame.account(31).key.to_bytes()
        || config.router != release.router_program()
        || frame.account(29).owner != frame.account(27).key
        || frame.account(32).owner != frame.account(30).key
        || frame.account(33).owner != frame.account(30).key
        || !rent.is_exempt(frame.account(29).lamports(), frame.account(29).data_len())
        || !rent.is_exempt(frame.account(32).lamports(), frame.account(32).data_len())
        || !rent.is_exempt(frame.account(33).lamports(), frame.account(33).data_len())
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_provider_program(
        frame.account(27),
        frame.account(28),
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_provider_program(
        frame.account(30),
        frame.account(31),
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let expected_treasury =
        Pubkey::find_program_address(&[b"treasury", &[0]], frame.account(27).key).0;
    if frame.account(34).key != &expected_treasury
        || frame.account(34).owner != &system_program::ID
        || frame.account(34).executable
        || !frame.account(34).data_is_empty()
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let encoded_data = frame
        .account(32)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let encoded = VerifiedEncodedVaaV1::parse(&encoded_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if encoded.write_authority() != request.provider_submitter {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let guardian_index = encoded.guardian_set_index().to_be_bytes();
    let expected_guardians =
        Pubkey::find_program_address(&[b"GuardianSet", &guardian_index], frame.account(30).key).0;
    let guardian_data = frame
        .account(33)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let guardians =
        GuardianSetV1::parse(&guardian_data).map_err(|_| ResolutionError::ProviderObservation)?;
    if frame.account(33).key != &expected_guardians
        || guardians
            .authenticate(
                encoded,
                release.guardian_set_count(),
                release.required_guardian_count(),
            )
            .is_err()
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        program_id,
    )
    .0;
    if frame.account(3).key != &authority {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(authority)
}

fn preflight_submit_outputs(
    program_id: &Pubkey,
    request: &ProviderSubmitRequestV3,
    frame: SubmitFrameV3<'_, '_>,
    rent: &Rent,
    authority: Pubkey,
) -> ProgramResult {
    let (expected_lifecycle, _) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            &request.update_account,
        ],
        program_id,
    );
    if frame.account(2).key != &expected_lifecycle
        || frame.account(2).owner != &system_program::ID
        || frame.account(2).data_len() != 0
        || frame.account(2).lamports() < rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)
        || frame.account(1).owner != &system_program::ID
        || frame.account(1).data_len() != 0
        || frame.account(1).lamports() != 0
        || frame.account(3).key != &authority
        || frame.account(3).owner != &system_program::ID
        || frame.account(3).data_len() != 0
        || frame.account(3).lamports() != 0
        || frame.account(37).key != &system_program::ID
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn invoke_post(
    frame: SubmitFrameV3<'_, '_>,
    body: &[u8],
    request: &ProviderSubmitRequestV3,
    authority: Pubkey,
) -> ProgramResult {
    let mut data = Vec::new();
    data.try_reserve_exact(
        POST_UPDATE_DISCRIMINATOR
            .len()
            .checked_add(body.len())
            .ok_or(ResolutionError::Arithmetic)?,
    )
    .map_err(|_| ResolutionError::Arithmetic)?;
    data.extend_from_slice(&POST_UPDATE_DISCRIMINATOR);
    data.extend_from_slice(body);
    let instruction = Instruction {
        program_id: *frame.account(27).key,
        accounts: Vec::from([
            AccountMeta::new(*frame.account(0).key, true),
            AccountMeta::new_readonly(*frame.account(32).key, false),
            AccountMeta::new_readonly(*frame.account(29).key, false),
            AccountMeta::new(*frame.account(34).key, false),
            AccountMeta::new(*frame.account(1).key, true),
            AccountMeta::new_readonly(*frame.account(37).key, false),
            AccountMeta::new_readonly(authority, true),
        ]),
        data,
    };
    let bump = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        frame.account(14).key,
    )
    .1;
    let bump_seed = [bump];
    let signer = [
        PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
        request.market.as_slice(),
        request.source_state.as_slice(),
        request.update_account.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &instruction,
        &[
            frame.account(0).clone(),
            frame.account(32).clone(),
            frame.account(29).clone(),
            frame.account(34).clone(),
            frame.account(1).clone(),
            frame.account(37).clone(),
            frame.account(3).clone(),
            frame.account(27).clone(),
        ],
        &[&signer],
    )
    .map_err(|_| ResolutionError::ProviderObservation.into())
}

fn lifecycle_bump(
    program_id: &Pubkey,
    update: &Pubkey,
    lifecycle: &Pubkey,
) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(
        &[PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, update.as_ref()],
        program_id,
    );
    if lifecycle != &expected {
        Err(ResolutionError::OutputState.into())
    } else {
        Ok(bump)
    }
}

fn initialize_lifecycle(
    program_id: &Pubkey,
    frame: SubmitFrameV3<'_, '_>,
    rent: &Rent,
    lifecycle: ProviderUpdateLifecycleV3,
) -> ProgramResult {
    let lifecycle_account = frame.account(2);
    let minimum = rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
    if lifecycle_account.owner != &system_program::ID
        || lifecycle_account.data_len() != 0
        || lifecycle_account.lamports() < minimum
    {
        return Err(ResolutionError::OutputState.into());
    }
    let bump_seed = [lifecycle.bump];
    let signer = [
        PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
        frame.account(1).key.as_ref(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &allocate(
            lifecycle_account.key,
            u64::try_from(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)
                .map_err(|_| ResolutionError::Arithmetic)?,
        ),
        &[lifecycle_account.clone(), frame.account(37).clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(lifecycle_account.key, program_id),
        &[lifecycle_account.clone(), frame.account(37).clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    let bytes = lifecycle
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    let mut output = lifecycle_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if output.len() != PROVIDER_UPDATE_LIFECYCLE_BYTES_V3 || output.iter().any(|byte| *byte != 0) {
        return Err(ResolutionError::OutputState.into());
    }
    output.copy_from_slice(&bytes);
    Ok(())
}

fn authenticate_reclaim_state(
    program_id: &Pubkey,
    request: &ProviderReclaimRequestV3,
    frame: ReclaimFrameV3<'_, '_>,
    rent: &Rent,
    current_unix_seconds: i64,
) -> Result<ProviderUpdateLifecycleV3, ProgramError> {
    let data = frame
        .account(1)
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let lifecycle =
        ProviderUpdateLifecycleV3::decode(&data).map_err(|_| ResolutionError::OutputState)?;
    let (expected_lifecycle, bump) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            frame.account(2).key.as_ref(),
        ],
        program_id,
    );
    let authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        program_id,
    )
    .0;
    if frame.account(1).key != &expected_lifecycle
        || frame.account(1).owner != program_id
        || !rent.is_exempt(frame.account(1).lamports(), data.len())
        || lifecycle.status != ProviderUpdateStatusV3::Consumed
        || lifecycle.bump != bump
        || lifecycle.generation != request.generation
        || lifecycle.terminal_sequence != request.terminal_sequence
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.certificate != request.certificate
        || lifecycle.update_account != request.update_account
        || lifecycle.refund_recipient != request.refund_recipient
        || lifecycle.release_set != request.release_set
        || lifecycle.update_authority != authority.to_bytes()
        || frame.account(3).key != &authority
        || current_unix_seconds < lifecycle.reclaim_after_unix_seconds
    {
        return Err(ResolutionError::OutputState.into());
    }
    let certificate_data = frame
        .account(5)
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let certificate = ResolutionCertificateV2::decode(&certificate_data)
        .map_err(|_| ResolutionError::OutputState)?;
    if frame.account(5).owner != program_id
        || certificate.market != lifecycle.market
        || certificate.source_material != lifecycle.source_material
        || certificate.provider_evidence != lifecycle.provider_evidence
        || certificate.receipt_account != lifecycle.certificate
        || certificate.generation != lifecycle.generation
    {
        return Err(ResolutionError::OutputState.into());
    }
    let update_data = frame
        .account(2)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update =
        FullPriceUpdateV2::parse(&update_data).map_err(|_| ResolutionError::ProviderObservation)?;
    if frame.account(2).owner != frame.account(13).key
        || hash(&update_data).to_bytes() != lifecycle.update_digest
        || frame.account(2).lamports() != lifecycle.update_rent_lamports
        || update.write_authority() != lifecycle.update_authority
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    Ok(lifecycle)
}

fn authenticate_reclaim_release(
    program_id: &Pubkey,
    request: &ProviderReclaimRequestV3,
    frame: ReclaimFrameV3<'_, '_>,
    rent: &Rent,
    lifecycle: ProviderUpdateLifecycleV3,
) -> ProgramResult {
    if frame.account(7).key.to_bytes() != lifecycle.registry_program {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let expected_registry_programdata = Pubkey::find_program_address(
        &[frame.account(7).key.as_ref()],
        &bpf_loader_upgradeable::ID,
    )
    .0;
    let registry_program_data = frame
        .account(7)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let registry_program = ProgramV3View::parse(&registry_program_data)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let registry_programdata_data = frame
        .account(8)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let registry_programdata = ProgramDataV3View::parse(&registry_programdata_data)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    if frame.account(7).owner != &bpf_loader_upgradeable::ID
        || frame.account(8).owner != &bpf_loader_upgradeable::ID
        || !frame.account(7).executable
        || frame.account(8).executable
        || frame.account(8).key != &expected_registry_programdata
        || registry_program.programdata() != frame.account(8).key.to_bytes()
        || registry_programdata.upgrade_authority().is_some()
    {
        return Err(ResolutionError::ResolutionDeployment.into());
    }
    let activation_data = frame
        .account(6)
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if frame.account(6).owner != frame.account(7).key
        || frame.account(6).data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || frame.account(6).key
            != &Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
                frame.account(7).key,
            )
            .0
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let selected = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?
        .to_bytes()
        != request.release_set
        || selected.release().program().to_bytes() != program_id.to_bytes()
        || selected.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V4
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    selected
        .authenticate_current_deployment(deployment_observation(
            frame.account(9),
            frame.account(10),
            selected.release().programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;

    let release_data = frame
        .account(11)
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        frame.account(7).key,
        frame.account(11),
        frame.account(12),
        rent,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        lifecycle.provider_release,
        &release_data,
        dclutch_pyth_svm::PYTH_RELEASE_V1_ENCODED_LEN,
    )?;
    let release =
        PythReleaseV1::decode(&release_data).map_err(|_| ResolutionError::ProviderRelease)?;
    if release.receiver_program() != frame.account(13).key.to_bytes()
        || release.receiver_programdata() != frame.account(14).key.to_bytes()
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_provider_program(
        frame.account(13),
        frame.account(14),
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )
}

fn invoke_reclaim(
    program_id: &Pubkey,
    frame: ReclaimFrameV3<'_, '_>,
    lifecycle: ProviderUpdateLifecycleV3,
) -> ProgramResult {
    let instruction = Instruction {
        program_id: *frame.account(13).key,
        accounts: Vec::from([
            AccountMeta::new(*frame.account(3).key, true),
            AccountMeta::new(*frame.account(2).key, false),
        ]),
        data: Vec::from(RECLAIM_RENT_DISCRIMINATOR),
    };
    let bump = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &lifecycle.market,
            &lifecycle.source_state,
            &lifecycle.update_account,
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    let signer = [
        PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
        lifecycle.market.as_slice(),
        lifecycle.source_state.as_slice(),
        lifecycle.update_account.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &instruction,
        &[
            frame.account(3).clone(),
            frame.account(2).clone(),
            frame.account(13).clone(),
        ],
        &[&signer],
    )
    .map_err(|_| ResolutionError::ProviderObservation.into())
}

fn transfer_reclaimed_rent(
    program_id: &Pubkey,
    frame: ReclaimFrameV3<'_, '_>,
    lifecycle: ProviderUpdateLifecycleV3,
) -> ProgramResult {
    let bump = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &lifecycle.market,
            &lifecycle.source_state,
            &lifecycle.update_account,
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    let signer = [
        PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
        lifecycle.market.as_slice(),
        lifecycle.source_state.as_slice(),
        lifecycle.update_account.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &transfer(
            frame.account(3).key,
            frame.account(4).key,
            lifecycle.update_rent_lamports,
        ),
        &[
            frame.account(3).clone(),
            frame.account(4).clone(),
            frame.account(17).clone(),
        ],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState.into())
}

fn close_lifecycle(
    lifecycle: &AccountInfo<'_>,
    refund: &AccountInfo<'_>,
    lamports: u64,
) -> ProgramResult {
    let expected = refund
        .lamports()
        .checked_add(lamports)
        .ok_or(ResolutionError::Arithmetic)?;
    **lifecycle
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)? = 0;
    **refund
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)? = expected;
    lifecycle.resize(0)?;
    lifecycle.assign(&system_program::ID);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_transport_discriminators_and_frames_are_frozen() {
        assert_eq!(PROVIDER_SUBMIT_ACCOUNT_COUNT_V3, 38);
        assert_eq!(PROVIDER_RECLAIM_ACCOUNT_COUNT_V3, 18);
        assert_eq!(
            POST_UPDATE_DISCRIMINATOR,
            [133, 95, 207, 175, 11, 79, 118, 44]
        );
        assert_eq!(
            RECLAIM_RENT_DISCRIMINATOR,
            [218, 200, 19, 197, 227, 89, 192, 22]
        );
    }

    #[test]
    fn transport_dispatch_does_not_accept_terminal_or_truncated_bytes() {
        assert!(!is_provider_transport_v3(b"DCLTPRQ3"));
        assert!(is_provider_transport_v3(&PROVIDER_SUBMIT_REQUEST_MAGIC_V3));
        assert!(is_provider_transport_v3(&PROVIDER_RECLAIM_REQUEST_MAGIC_V3));
    }
}
