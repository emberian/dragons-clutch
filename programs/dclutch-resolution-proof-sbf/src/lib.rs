#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Registry-bound Core-effect Source Resolution controller.

extern crate alloc;
extern crate std;

use dclutch_capability_contract::CapabilityManifestV1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::DeploymentObservationV1;
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_source_contract::{RecoveryPolicyV2, SourceMaterialV2};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, hash::hash,
    program_error::ProgramError, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

mod core_effect;
mod provider_instruction_v3;
mod provider_transport_v3;
mod relay_transport_v1;
/// Current-ABI real-provider evidence composition shared by fixed Core and
/// data-defined Trading callers.
pub mod provider_v3;

/// Stable Resolution controller refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionError {
    /// Account count, order, privilege, executable state, or aliasing was invalid.
    AccountFrame = 0,
    /// The generated fixed-layout request refused hostile bytes.
    Instruction = 1,
    /// A writable Source state or certificate account was not canonical.
    OutputState = 2,
    /// Market owner, root, lifecycle, generation, or Source binding was invalid.
    MarketAuthority = 3,
    /// A finalized raw-record owner, PDA, digest, rent, or vacancy proof was invalid.
    FinalizedRecord = 4,
    /// The Market-selected Registry activation did not authorize this Resolution release.
    ResolutionRelease = 5,
    /// Current Loader V3 Program, ProgramData, ELF, slot, or upgrade policy was substituted.
    ResolutionDeployment = 6,
    /// Source material or one of its embedded content identities was inconsistent.
    SourceMaterial = 7,
    /// The external Product-owned result-domain identity or bytes differed.
    ProductDomain = 8,
    /// The selected Pyth provider-release record or Loader accounts differed.
    ProviderRelease = 9,
    /// Pyth configuration or fully verified update authentication failed.
    ProviderObservation = 10,
    /// Clock or Rent sysvar identity or bytes were invalid.
    Sysvar = 11,
    /// Provider-neutral Source admission or Product mapping refused.
    Transition = 12,
    /// Checked physical arithmetic or signed timestamp conversion failed.
    Arithmetic = 13,
    /// Canonical capability funding, typed custody, or exact bounty debit failed.
    Funding = 14,
}

impl From<ResolutionError> for ProgramError {
    fn from(value: ResolutionError) -> Self {
        Self::Custom(value as u32)
    }
}

pub(crate) enum RecordKind {
    CapabilityManifest,
    SourceMaterialV2,
    RecoveryPolicyV2,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate one exact Resolution frame and atomically persist its outputs.
///
/// Direct funded transitions return the canonical funded-transition receipt
/// only after Source, certificate, FundingState, and worker payout commit.
/// Core-effect routes retain their sole canonical Core acknowledgment wire.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if core_effect::is_core_effect(instruction_data) {
        return core_effect::process_core_effect(program_id, accounts, instruction_data);
    }
    if provider_instruction_v3::is_provider_resolution_v3(instruction_data) {
        return provider_instruction_v3::process_provider_resolution_v3(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if provider_transport_v3::is_provider_transport_v3(instruction_data) {
        return provider_transport_v3::process_provider_transport_v3(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if relay_transport_v1::is_relay_transport_v1(instruction_data) {
        return relay_transport_v1::process_relay_transport_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    Err(ResolutionError::Instruction.into())
}

#[cfg(any())]
fn removed_legacy_v1_direct_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() == FUNDED_TRANSITION_REQUEST_BYTES {
        return funded::process_funded_transition(program_id, accounts, instruction_data);
    }
    if accounts.len() != ACCEPT_PYTH_ACCOUNT_COUNT {
        return Err(ResolutionError::AccountFrame.into());
    }
    let request =
        AcceptPythRequestV1::decode(instruction_data).map_err(|_| ResolutionError::Instruction)?;

    let mut iterator = accounts.iter();
    let source_state = next(&mut iterator)?;
    let certificate = next(&mut iterator)?;
    let market = next(&mut iterator)?;
    let activated_release_set = next(&mut iterator)?;
    let resolution_program = next(&mut iterator)?;
    let resolution_programdata = next(&mut iterator)?;
    let source_material = next(&mut iterator)?;
    let source_material_staging = next(&mut iterator)?;
    let product_instance = next(&mut iterator)?;
    let product_instance_staging = next(&mut iterator)?;
    let provider_release = next(&mut iterator)?;
    let provider_release_staging = next(&mut iterator)?;
    let price_update = next(&mut iterator)?;
    let receiver_program = next(&mut iterator)?;
    let receiver_programdata = next(&mut iterator)?;
    let receiver_config = next(&mut iterator)?;
    let router_program = next(&mut iterator)?;
    let router_programdata = next(&mut iterator)?;
    let clock_sysvar = next(&mut iterator)?;
    let rent_sysvar = next(&mut iterator)?;
    let system = next(&mut iterator)?;

    validate_frame(accounts, program_id)?;
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
    authenticate_state_account(program_id, source_state, prior_state)?;
    if prior_state.phase() != SourceResolutionPhaseV1::Primary {
        return Err(ResolutionError::Transition.into());
    }
    let market_authority = authenticate_market_and_resolution_release(
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
    let material_id = prior_state.material_id().to_bytes();
    authenticate_finalized_record(
        market_authority.registry_program,
        source_material,
        source_material_staging,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        material_id,
        &material_data,
        RecordKind::SourceMaterial,
    )?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    authenticate_material_components(material, market_authority.product_instance_id)?;

    let product_data = product_instance
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        market_authority.registry_program,
        product_instance,
        product_instance_staging,
        &rent,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        market_authority.product_instance_id,
        &product_data,
        RecordKind::ProductInstance,
    )?;
    let domain_outcome_count = authenticate_product_instance(
        material,
        market_authority,
        request.expected_result_domain_id,
        &product_data,
    )?;

    let (source_id, source) = material
        .primary_source()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    if source.access_profile() != SourceAccessProfile::PythTerminalOneTransaction {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let (_, selected_provider) = material
        .primary_provider_release()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let deployment_release_id = selected_provider
        .provider_deployment_release_id()
        .to_bytes();
    if request.expected_provider_release_id != deployment_release_id {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_pyth_release_record(
        market_authority.registry_program,
        provider_release,
        provider_release_staging,
        &rent,
        deployment_release_id,
        selected_provider,
        receiver_program,
        receiver_programdata,
        receiver_config,
        router_program,
        router_programdata,
        &clock,
    )?;

    let update_data = price_update
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if price_update.owner != receiver_program.key
        || !rent.is_exempt(price_update.lamports(), update_data.len())
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let update =
        FullPriceUpdateV2::parse(&update_data).map_err(|_| ResolutionError::ProviderObservation)?;
    if update.posted_slot() > clock.slot || update.publish_time() <= 0 {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let provider_evidence_id = hashv(&[
        PYTH_EVIDENCE_CONTENT_DOMAIN_V1,
        &[0],
        source_id.as_bytes(),
        &deployment_release_id,
        &update_data,
    ])
    .to_bytes();
    let provider_evidence = dclutch_source_contract::ContentId::new(provider_evidence_id)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let obligation = dclutch_source_contract::PythProviderAdapterObligationV1::from_material_view(
        material, source_id,
    )
    .map_err(|_| ResolutionError::ProviderObservation)?;
    if obligation.provider_release() != selected_provider {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let window = material
        .window()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let normalized = obligation
        .normalize_authenticated_update(
            provider_evidence,
            window.schedule_id(),
            0,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
        )
        .map_err(|_| ResolutionError::ProviderObservation)?;

    let mut next_state = prior_state;
    let decision = next_state
        .accept_provider_output_view(
            dclutch_source_contract::ContentId::new(material_id)
                .map_err(|_| ResolutionError::SourceMaterial)?,
            material,
            provider_evidence,
            core::slice::from_ref(&normalized),
            None,
            None,
            request.expected_generation,
            clock.unix_timestamp,
            PRIMARY_CERTIFICATE_SEQUENCE_V3,
        )
        .map_err(|_| ResolutionError::Transition)?;
    if decision.outcome_count() != domain_outcome_count
        || decision.selector() >= domain_outcome_count.saturating_sub(1)
    {
        return Err(ResolutionError::Transition.into());
    }
    let observed_at =
        u64::try_from(update.publish_time()).map_err(|_| ResolutionError::Arithmetic)?;
    let certificate_value = ResolutionCertificateV1 {
        kind: ResolutionCertificateKindV1::ResolutionSuccess,
        market: market.key.to_bytes(),
        route: deployment_release_id,
        source_material: material_id,
        product: material
            .product_instance_id()
            .map_err(|_| ResolutionError::SourceMaterial)?
            .to_bytes(),
        provider_evidence: provider_evidence_id,
        funding_allocation: [0; 32],
        receipt_account: certificate.key.to_bytes(),
        generation: request.expected_generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: u32::from(decision.selector()),
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: i128::from(update.price()),
        result_denominator: 1,
        observed_at,
    };
    let next_state_bytes = next_state.to_bytes();
    let certificate_bytes = certificate_value
        .to_bytes()
        .map_err(|_| ResolutionError::OutputState)?;

    drop(update_data);
    drop(product_data);
    drop(material_data);
    drop(source_state_data);
    commit_outputs(
        source_state,
        certificate,
        &next_state_bytes,
        &certificate_bytes,
        ResolutionCertificateKindV1::ResolutionSuccess,
        PRIMARY_CERTIFICATE_SEQUENCE_V3,
        program_id,
        system,
        &rent,
    )
}

#[cfg(any())]
fn validate_frame(accounts: &[AccountInfo<'_>], program_id: &Pubkey) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer {
            return Err(ResolutionError::AccountFrame.into());
        }
        let writable = index <= 1;
        let executable = matches!(index, 4 | 13 | 16 | 20);
        if account.is_writable != writable || account.executable != executable {
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
    if accounts.get(4).ok_or(ResolutionError::AccountFrame)?.key != program_id
        || accounts.get(20).ok_or(ResolutionError::AccountFrame)?.key != &system_program::ID
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

pub(crate) fn authenticate_clock(account: &AccountInfo<'_>) -> Result<Clock, ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID {
        return Err(ResolutionError::Sysvar.into());
    }
    Clock::from_account_info(account).map_err(|_| ResolutionError::Sysvar.into())
}

pub(crate) fn authenticate_rent(account: &AccountInfo<'_>) -> Result<Rent, ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(ResolutionError::Sysvar.into());
    }
    Rent::from_account_info(account).map_err(|_| ResolutionError::Sysvar.into())
}

#[cfg(any())]
pub(crate) fn authenticate_state_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    state: SourceResolutionStateV1,
) -> ProgramResult {
    if account.owner != program_id
        || account.data_len() != SOURCE_RESOLUTION_STATE_BYTES
        || account.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    let seeds = state.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        program_id,
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if account.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[cfg(any())]
pub(crate) fn authenticate_market_and_resolution_release(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    state: SourceResolutionStateV1,
    expected_generation: u64,
    activated_release_set: &AccountInfo<'_>,
    resolution_program: &AccountInfo<'_>,
    resolution_programdata: &AccountInfo<'_>,
    rent: &Rent,
) -> Result<MarketAuthority, ProgramError> {
    if market.executable || market.owner == &system_program::ID {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let data = market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let core_state = CoreState::decode(&data).map_err(|_| ResolutionError::MarketAuthority)?;
    let market_seeds = MarketCoreStateSeedsV2::new(core_state.identity);
    let expected_market = Pubkey::find_program_address(&market_seeds.as_slices(), market.owner).0;
    if !rent.is_exempt(market.lamports(), data.len())
        || core_state.phase != CorePhase::Open
        || core_state.readiness != CoreReadiness::Consumed
        || market.key.to_bytes() != state.market()
        || market.key.to_bytes() != core_state.identity.market_id.to_bytes()
        || market.key != &expected_market
        || core_state.identity.generation != state.generation()
        || expected_generation != state.generation()
        || core_state.identity.resolution_policy.to_bytes() != state.material_id().to_bytes()
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let registry_program = Pubkey::new_from_array(core_state.identity.registry_program.to_bytes());
    let activated_data = activated_release_set
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if !rent.is_exempt(activated_release_set.lamports(), activated_data.len()) {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = authenticate_activation_cache(
        activated_release_set,
        registry_program,
        *market.owner,
        core_state.identity.selected_release_set.to_bytes(),
        &activated_data,
    )?;
    authenticate_resolution_release(
        program_id,
        resolution_program,
        resolution_programdata,
        activated,
    )?;
    Ok(MarketAuthority {
        product_instance_id: core_state.identity.product_id.to_bytes(),
        // The legacy direct V1 path has no authenticated Runtime V2 domain
        // projection and must fail closed until its separate physical cut.
        result_domain_id: [0_u8; 32],
        registry_program,
        semantic_capability_manifest_id: core_state.identity.capability_manifest.to_bytes(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_finalized_record(
    core_program: Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema_id: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &[u8],
    kind: RecordKind,
) -> ProgramResult {
    if raw.owner != &core_program
        || raw.executable
        || hash(bytes).to_bytes() != expected_digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_id, &expected_digest],
        &core_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema_id, &expected_digest],
        &core_program,
    )
    .0;
    if raw.key != &expected_raw
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.lamports() != 0
        || staging.data_len() != 0
        || staging.executable
    {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    let valid = match kind {
        RecordKind::CapabilityManifest => CapabilityManifestV1::decode(bytes).is_ok(),
        RecordKind::SourceMaterialV2 => SourceMaterialV2::decode(bytes).is_ok(),
        RecordKind::RecoveryPolicyV2 => RecoveryPolicyV2::decode(bytes).is_ok(),
    };
    if !valid {
        return Err(ResolutionError::FinalizedRecord.into());
    }
    Ok(())
}

#[cfg(any())]
fn authenticate_activation_cache<'a>(
    account: &AccountInfo<'_>,
    registry_program: Pubkey,
    core_program: Pubkey,
    release_set_id: [u8; 32],
    bytes: &'a [u8],
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, ProgramError> {
    if account.owner != &registry_program
        || account.executable
        || bytes.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(bytes)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?
        .to_bytes()
        != release_set_id
        || activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| ResolutionError::ResolutionRelease)?
            .release()
            .program()
            .to_bytes()
            != core_program.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &registry_program,
    )
    .0;
    if account.key != &expected {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    Ok(activated)
}

#[allow(clippy::too_many_arguments)]
#[cfg(any())]
fn authenticate_resolution_release(
    program_id: &Pubkey,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> ProgramResult {
    let role = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let release = role.release();
    if release.program().to_bytes() != program_id.to_bytes()
        || release.semantic_release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let observation = deployment_observation(program, programdata, release.programdata())?;
    role.authenticate_current_deployment(observation)
        .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

pub(crate) fn deployment_observation(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    expected_programdata: [u8; 32],
) -> Result<DeploymentObservationV1, ProgramError> {
    if program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
        || programdata.key.to_bytes() != expected_programdata
    {
        return Err(ResolutionError::ResolutionDeployment.into());
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| ResolutionError::ResolutionDeployment)?;
    let expected_derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata_key() != expected_programdata
        || programdata.key != &expected_derived
    {
        return Err(ResolutionError::ResolutionDeployment.into());
    }
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let view = ProgramDataV3View::parse(&programdata_bytes)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata_key(),
        bpf_loader_upgradeable::ID.to_bytes(),
        view.deployment_slot(),
        hash(view.elf()).to_bytes(),
        view.upgrade_authority(),
    )
    .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

#[cfg(any())]
pub(crate) fn authenticate_material_components(
    material: SourceMaterialViewV1<'_>,
    market_product_instance_id: [u8; 32],
) -> ProgramResult {
    let policy = material
        .policy()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let (capacity_id, capacity) = material
        .capacity_profile()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let (source_id, source) = material
        .primary_source()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let (window_id, window) = material
        .window_spec()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let statistic = material
        .statistic()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let (provider_id, provider) = material
        .primary_provider_release()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let adapter = material
        .primary_adapter_config()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    if hash(&capacity.to_bytes()).to_bytes() != capacity_id.to_bytes()
        || hash(&source.to_bytes()).to_bytes() != source_id.to_bytes()
        || hash(&window.to_bytes()).to_bytes() != window_id.to_bytes()
        || hash(&statistic.to_bytes()).to_bytes() != policy.statistic_spec_id().to_bytes()
        || hash(&provider.to_bytes()).to_bytes() != provider_id.to_bytes()
        || hash(&adapter.to_bytes()).to_bytes() != source.adapter_config_id().to_bytes()
        || provider.adapter_release_id().to_bytes() != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        || policy.product_instance_id().to_bytes() != market_product_instance_id
        || material
            .product_instance_id()
            .map_err(|_| ResolutionError::SourceMaterial)?
            .to_bytes()
            != market_product_instance_id
    {
        return Err(ResolutionError::SourceMaterial.into());
    }
    Ok(())
}

#[cfg(any())]
pub(crate) fn authenticate_product_instance(
    material: SourceMaterialViewV1<'_>,
    authority: MarketAuthority,
    expected_result_domain_id: [u8; 32],
    bytes: &[u8],
) -> Result<u8, ProgramError> {
    let instance = InstanceV1::decode(bytes).map_err(|_| ResolutionError::ProductDomain)?;
    let embedded = material
        .result_domain()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let embedded_bytes = embedded.to_bytes();
    let domain_id = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        &embedded_bytes,
    ])
    .to_bytes();
    let policy = material
        .policy()
        .map_err(|_| ResolutionError::ProductDomain)?;
    if domain_id != expected_result_domain_id
        || domain_id != authority.result_domain_id
        || domain_id != policy.result_domain_id().to_bytes()
        || instance.result_domain_id().to_bytes() != domain_id
        || instance.partition_cell_count() != u32::from(embedded.outcome_count())
        || material
            .product_instance_id()
            .map_err(|_| ResolutionError::ProductDomain)?
            .to_bytes()
            != authority.product_instance_id
    {
        return Err(ResolutionError::ProductDomain.into());
    }
    Ok(embedded.outcome_count())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[cfg(any())]
fn authenticate_pyth_release_record(
    registry_program: Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    expected_digest: [u8; 32],
    selected: dclutch_source_contract::ProviderReleaseV1,
    receiver: &AccountInfo<'_>,
    receiver_programdata: &AccountInfo<'_>,
    config: &AccountInfo<'_>,
    router: &AccountInfo<'_>,
    router_programdata: &AccountInfo<'_>,
    clock: &Clock,
) -> ProgramResult {
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        registry_program,
        raw,
        staging,
        rent,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        expected_digest,
        &bytes,
        RecordKind::PythRelease,
    )?;
    let release = PythReleaseV1::decode(&bytes).map_err(|_| ResolutionError::ProviderRelease)?;
    authenticate_provider_release(
        selected,
        release,
        receiver,
        receiver_programdata,
        config,
        router,
        router_programdata,
        clock,
        rent,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(any())]
fn authenticate_provider_release(
    selected: dclutch_source_contract::ProviderReleaseV1,
    release: PythReleaseV1,
    receiver: &AccountInfo<'_>,
    receiver_programdata: &AccountInfo<'_>,
    config: &AccountInfo<'_>,
    router: &AccountInfo<'_>,
    router_programdata: &AccountInfo<'_>,
    clock: &Clock,
    rent: &Rent,
) -> ProgramResult {
    if selected.decoding_rules_id().to_bytes() != release.price_update_codec_id()
        || selected.transport_profile_id().to_bytes() != release.adapter_id()
        || clock.unix_timestamp < release.activation_time()
        || receiver.key.to_bytes() != release.receiver_program()
        || receiver_programdata.key.to_bytes() != release.receiver_programdata()
        || router.key.to_bytes() != release.router_program()
        || router_programdata.key.to_bytes() != release.router_programdata()
        || config.key.to_bytes() != release.receiver_config()
        || config.owner != receiver.key
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_provider_loader(
        receiver,
        receiver_programdata,
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_provider_loader(
        router,
        router_programdata,
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let config_data = config
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let parsed = ReceiverConfigV2View::parse(&config_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if !rent.is_exempt(config.lamports(), config_data.len())
        || hash(&config_data).to_bytes() != release.config_digest()
        || parsed.router_program() != release.router_program()
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    Ok(())
}

#[cfg(any())]
fn authenticate_provider_loader(
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
    let programdata_data = programdata
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderRelease)?;
    let view = ProgramDataV3View::parse(&programdata_data)
        .map_err(|_| ResolutionError::ProviderRelease)?;
    if view.deployment_slot() != expected_slot {
        return Err(ResolutionError::ProviderRelease.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[cfg(any())]
fn commit_outputs<'info>(
    state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    next_state: &[u8; SOURCE_RESOLUTION_STATE_BYTES],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES],
    kind: ResolutionCertificateKindV1,
    sequence: u64,
    program_id: &Pubkey,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    initialize_certificate_output(program_id, state, certificate, system, rent, kind, sequence)?;
    if certificate.owner != state.owner {
        return Err(ResolutionError::OutputState.into());
    }
    let mut state_output = state
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_output = certificate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES
        || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(next_state);
    certificate_output.copy_from_slice(next_certificate);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[cfg(any())]
pub(crate) fn initialize_certificate_output<'info>(
    program_id: &Pubkey,
    state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    kind: ResolutionCertificateKindV1,
    sequence: u64,
) -> ProgramResult {
    let kind_seed = [match kind {
        ResolutionCertificateKindV1::ResolutionSuccess => 1,
        ResolutionCertificateKindV1::RecoveryAdvanced => 2,
        ResolutionCertificateKindV1::Exhausted => 3,
        ResolutionCertificateKindV1::ResolutionFailure => 4,
    }];
    let sequence_seed = sequence.to_le_bytes();
    let (expected_certificate, bump) = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            state.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    );
    if certificate.key != &expected_certificate {
        return Err(ResolutionError::OutputState.into());
    }
    let minimum = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES);
    if certificate.owner == program_id {
        if certificate.executable
            || certificate.data_len() != RESOLUTION_CERTIFICATE_BYTES
            || certificate.lamports() < minimum
        {
            return Err(ResolutionError::OutputState.into());
        }
        return Ok(());
    }
    if certificate.owner != &system_program::ID
        || certificate.executable
        || certificate.data_len() != 0
        || certificate.lamports() < minimum
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    let bump_seed = [bump];
    let sequence_seed = sequence.to_le_bytes();
    let signer: [&[u8]; 5] = [
        RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
        state.key.as_ref(),
        &kind_seed,
        &sequence_seed,
        &bump_seed,
    ];
    let space =
        u64::try_from(RESOLUTION_CERTIFICATE_BYTES).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(certificate.key, space),
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
    if certificate.owner != program_id
        || certificate.executable
        || certificate.data_len() != RESOLUTION_CERTIFICATE_BYTES
        || certificate.lamports() < minimum
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[cfg(any())]
fn next<'a, 'info>(
    iterator: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iterator).map_err(|_| ResolutionError::AccountFrame.into())
}

#[cfg(test)]
mod tests;
