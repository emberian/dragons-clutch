//! Canonical Market-Core effect route for Source creation, readiness, terminal admission, and close.

use core::convert::TryFrom;

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1,
    FundingStateV1, FundingStatus,
};
use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CORE_EFFECT_ACK_BYTES_V1,
    CORE_EFFECT_DIGEST_DOMAIN_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CapabilityFundingHeaderV1,
    CoreEffectAckV1, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, Identity,
    MarketCoreStateSeedsV1, Phase as CorePhase, Readiness as CoreReadiness, Role,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, RESOLUTION_CORE_ROLE_REQUEST_BYTES,
    RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1, ResolutionCertificateKindV1, ResolutionCertificateV1,
    ResolutionCoreActionV1, ResolutionCoreReceiptKindV1, ResolutionRoleRequestV1,
    SOURCE_CLOSURE_RECEIPT_BYTES, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1, SourceClosureReceiptV1,
};
use dclutch_source_contract::{
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_BYTES, SourceMaterialViewV1,
    SourceResolutionPhaseV1, SourceResolutionStateV1,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign};

use crate::{
    RecordKind, ResolutionError, authenticate_clock, authenticate_finalized_record,
    authenticate_material_components, authenticate_rent, authenticate_state_account,
    deployment_observation,
};

/// Exact fixed instruction width for one canonical Core envelope and Resolution request.
pub(crate) const CORE_EFFECT_INSTRUCTION_BYTES: usize = CORE_EFFECT_ENVELOPE_BYTES_V1
    + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
    + RESOLUTION_CORE_ROLE_REQUEST_BYTES;
/// Create: eight authority accounts, eight Source records/accounts, Rent, and System.
pub(crate) const CREATE_FUND_ACCOUNT_COUNT: usize = 18;
/// Verify: common sixteen, beneficiary, Clock, and Rent.
pub(crate) const VERIFY_FUND_ACCOUNT_COUNT: usize = 19;
/// Terminal admission: common sixteen, terminal certificate, and Rent.
pub(crate) const ADMIT_TERMINAL_ACCOUNT_COUNT: usize = 18;
/// Close: common sixteen, certificate, closure, beneficiary, Clock, Rent, and System.
pub(crate) const CLOSE_FUND_ACCOUNT_COUNT: usize = 22;

const SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/source-funding-set/v1";
const RESOLUTION_FUNDING_COUNT: u8 = 3;

#[derive(Clone, Copy)]
struct CommonAccounts<'a, 'info> {
    caller_authority: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    activated_release_set: &'a AccountInfo<'info>,
    registry_program: &'a AccountInfo<'info>,
    core_program: &'a AccountInfo<'info>,
    core_programdata: &'a AccountInfo<'info>,
    resolution_program: &'a AccountInfo<'info>,
    resolution_programdata: &'a AccountInfo<'info>,
    source_material: &'a AccountInfo<'info>,
    source_material_staging: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_staging: &'a AccountInfo<'info>,
    source_state: &'a AccountInfo<'info>,
    recovery_funding: &'a AccountInfo<'info>,
    exhaustion_funding: &'a AccountInfo<'info>,
    failure_funding: &'a AccountInfo<'info>,
}

struct AuthenticatedCore {
    state: CoreState,
    full_effect_digest: Identity,
}

/// Return whether bytes select the one canonical Core effect route.
pub(crate) fn is_core_effect(instruction_data: &[u8]) -> bool {
    instruction_data.len() == CORE_EFFECT_INSTRUCTION_BYTES
        && instruction_data.get(..8)
            == Some(dclutch_market_core_codec::CORE_EFFECT_MAGIC_V1.as_slice())
}

/// Execute one Core-owned envelope through the sole Resolution semantic request.
#[inline(never)]
pub(crate) fn process_core_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !is_core_effect(instruction_data) {
        return Err(ResolutionError::Instruction.into());
    }
    let envelope_bytes = instruction_data
        .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(ResolutionError::Instruction)?;
    let role_bytes = instruction_data
        .get(CORE_EFFECT_ENVELOPE_BYTES_V1..)
        .ok_or(ResolutionError::Instruction)?;
    let funding_header_bytes = role_bytes
        .get(..CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
        .ok_or(ResolutionError::Instruction)?;
    let request_bytes = role_bytes
        .get(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
        .ok_or(ResolutionError::Instruction)?;
    if request_bytes.len() != RESOLUTION_CORE_ROLE_REQUEST_BYTES {
        return Err(ResolutionError::Instruction.into());
    }
    let funding_header = CapabilityFundingHeaderV1::decode(funding_header_bytes)
        .map_err(|_| ResolutionError::Instruction)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| ResolutionError::Instruction)?;
    let request =
        ResolutionRoleRequestV1::decode(request_bytes).map_err(|_| ResolutionError::Instruction)?;
    authenticate_action(envelope, request)?;
    authenticate_funding_header(funding_header)?;
    let expected_accounts = match request.action {
        ResolutionCoreActionV1::CreateFund => CREATE_FUND_ACCOUNT_COUNT,
        ResolutionCoreActionV1::VerifyFundReady => VERIFY_FUND_ACCOUNT_COUNT,
        ResolutionCoreActionV1::AdmitTerminal => ADMIT_TERMINAL_ACCOUNT_COUNT,
        ResolutionCoreActionV1::CloseFund => CLOSE_FUND_ACCOUNT_COUNT,
    };
    if accounts.len() != expected_accounts {
        return Err(ResolutionError::AccountFrame.into());
    }
    let common = parse_common(accounts)?;
    authenticate_common_frame(program_id, accounts, common, request)?;
    let rent_account = accounts
        .get(match request.action {
            ResolutionCoreActionV1::CreateFund => 16,
            ResolutionCoreActionV1::VerifyFundReady => 18,
            ResolutionCoreActionV1::AdmitTerminal => 17,
            ResolutionCoreActionV1::CloseFund => 20,
        })
        .ok_or(ResolutionError::AccountFrame)?;
    let rent = authenticate_rent(rent_account)?;
    let authenticated = authenticate_core(
        program_id,
        common,
        envelope,
        request,
        envelope_bytes,
        role_bytes,
        &rent,
    )?;
    match request.action {
        ResolutionCoreActionV1::CreateFund => process_create(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::VerifyFundReady => process_verify(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::AdmitTerminal => process_admit(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::CloseFund => process_close(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
    }
}

fn parse_common<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
) -> Result<CommonAccounts<'a, 'info>, ProgramError> {
    let mut iterator = accounts.iter();
    Ok(CommonAccounts {
        caller_authority: next(&mut iterator)?,
        market: next(&mut iterator)?,
        activated_release_set: next(&mut iterator)?,
        registry_program: next(&mut iterator)?,
        core_program: next(&mut iterator)?,
        core_programdata: next(&mut iterator)?,
        resolution_program: next(&mut iterator)?,
        resolution_programdata: next(&mut iterator)?,
        source_material: next(&mut iterator)?,
        source_material_staging: next(&mut iterator)?,
        capability_manifest: next(&mut iterator)?,
        capability_manifest_staging: next(&mut iterator)?,
        source_state: next(&mut iterator)?,
        recovery_funding: next(&mut iterator)?,
        exhaustion_funding: next(&mut iterator)?,
        failure_funding: next(&mut iterator)?,
    })
}

fn authenticate_action(
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
) -> ProgramResult {
    let expected = match request.action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady,
        ResolutionCoreActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal,
        ResolutionCoreActionV1::CloseFund => CoreEffectActionV1::CloseFund,
    };
    if envelope.action() != expected || envelope.target_role() != Role::Resolution {
        return Err(ResolutionError::Instruction.into());
    }
    Ok(())
}

fn authenticate_funding_header(funding_header: CapabilityFundingHeaderV1) -> ProgramResult {
    if funding_header.funding_count() == RESOLUTION_FUNDING_COUNT {
        Ok(())
    } else {
        Err(ResolutionError::Instruction.into())
    }
}

fn authenticate_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    request: ResolutionRoleRequestV1,
) -> ProgramResult {
    if common.resolution_program.key != program_id
        || !common.caller_authority.is_signer
        || common.caller_authority.is_writable
        || !common.registry_program.executable
        || !common.core_program.executable
        || !common.resolution_program.executable
        || common.source_material.is_writable
        || common.source_material_staging.is_writable
        || common.capability_manifest.is_writable
        || common.capability_manifest_staging.is_writable
        || common.market.is_writable
        || common.activated_release_set.is_writable
        || common.core_programdata.is_writable
        || common.resolution_programdata.is_writable
        || common.source_material.executable
        || common.source_material_staging.executable
        || common.capability_manifest.executable
        || common.capability_manifest_staging.executable
        || common.market.executable
        || common.activated_release_set.executable
        || common.core_programdata.executable
        || common.resolution_programdata.executable
        || common.source_state.key.to_bytes() != request.source_state
        || common.source_material.key.to_bytes() != request.source_material
        || common.capability_manifest.key.to_bytes() != request.capability_manifest
        || common.recovery_funding.key.to_bytes() != request.recovery_funding
        || common.exhaustion_funding.key.to_bytes() != request.exhaustion_funding
        || common.failure_funding.key.to_bytes() != request.failure_funding
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer != (index == 0)
            || accounts
                .iter()
                .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let writable = match request.action {
        ResolutionCoreActionV1::CreateFund => [true, true, true, true],
        ResolutionCoreActionV1::VerifyFundReady => [false, true, true, true],
        ResolutionCoreActionV1::AdmitTerminal => [false, false, false, false],
        ResolutionCoreActionV1::CloseFund => [true, true, true, true],
    };
    for (account, expected) in [
        common.source_state,
        common.recovery_funding,
        common.exhaustion_funding,
        common.failure_funding,
    ]
    .into_iter()
    .zip(writable)
    {
        if account.is_writable != expected || account.executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let tail_profile: &[(bool, bool)] = match request.action {
        ResolutionCoreActionV1::CreateFund => &[(false, false), (false, true)],
        ResolutionCoreActionV1::VerifyFundReady => &[(true, false), (false, false), (false, false)],
        ResolutionCoreActionV1::AdmitTerminal => &[(false, false), (false, false)],
        ResolutionCoreActionV1::CloseFund => &[
            (false, false),
            (true, false),
            (true, false),
            (false, false),
            (false, false),
            (false, true),
        ],
    };
    for (account, (writable, executable)) in accounts.iter().skip(16).zip(tail_profile.iter()) {
        if account.is_writable != *writable || account.executable != *executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_core(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
    envelope_bytes: &[u8],
    role_bytes: &[u8],
    rent: &Rent,
) -> Result<AuthenticatedCore, ProgramError> {
    let request_digest =
        Identity::new(hash(role_bytes).to_bytes()).map_err(|_| ResolutionError::Instruction)?;
    envelope
        .validate_role_request(role_bytes.len(), request_digest)
        .map_err(|_| ResolutionError::Instruction)?;
    if envelope.caller_program().to_bytes() != common.core_program.key.to_bytes()
        || envelope.caller_authority().to_bytes() != common.caller_authority.key.to_bytes()
        || envelope.market().to_bytes() != common.market.key.to_bytes()
        || envelope.context().to_bytes() != request.source_state
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let caller_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), common.core_program.key).0;
    if common.caller_authority.key != &expected_caller {
        return Err(ResolutionError::ResolutionRelease.into());
    }

    if common.market.owner != common.core_program.key || common.market.executable {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = common
        .market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    let state_digest = Identity::new(hash(&market_data).to_bytes())
        .map_err(|_| ResolutionError::MarketAuthority)?;
    if envelope.parent_state_digest() != state_digest
        || state.identity.market_id.to_bytes() != common.market.key.to_bytes()
        || state.identity.resolution_policy.to_bytes() != request.source_material
        || state.identity.capability_manifest.to_bytes() != request.capability_manifest
        || state.identity.selected_release_set.to_bytes() != envelope.release_set().to_bytes()
        || state.identity.generation != envelope.generation()
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_seeds = MarketCoreStateSeedsV1::new(state.identity);
    if Pubkey::find_program_address(&market_seeds.as_slices(), common.core_program.key).0
        != *common.market.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    match request.action {
        ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
            if state.phase != CorePhase::Founding || state.readiness != CoreReadiness::Prepaid {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            if state.phase != CorePhase::Open || state.readiness != CoreReadiness::Consumed {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
        ResolutionCoreActionV1::CloseFund => {
            if state.phase != CorePhase::Retiring || state.readiness != CoreReadiness::Consumed {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
    }

    authenticate_activation(program_id, common, envelope)?;
    authenticate_source_records(common, state, request, rent)?;
    let envelope_len = u32::try_from(envelope_bytes.len())
        .map_err(|_| ResolutionError::Arithmetic)?
        .to_le_bytes();
    let request_len = u32::try_from(role_bytes.len())
        .map_err(|_| ResolutionError::Arithmetic)?
        .to_le_bytes();
    let full_effect_digest = Identity::new(
        hashv(&[
            &CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_len,
            envelope_bytes,
            &request_len,
            role_bytes,
        ])
        .to_bytes(),
    )
    .map_err(|_| ResolutionError::Instruction)?;
    Ok(AuthenticatedCore {
        state,
        full_effect_digest,
    })
}

fn authenticate_activation(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
) -> ProgramResult {
    if common.activated_release_set.owner != common.registry_program.key
        || common.activated_release_set.executable
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activation_data = common
        .activated_release_set
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if release_set_id.to_bytes() != envelope.release_set().to_bytes()
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            common.registry_program.key,
        )
        .0 != *common.activated_release_set.key
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if core.release().program().to_bytes() != common.core_program.key.to_bytes()
        || resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V4
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let core_observation = deployment_observation(
        common.core_program,
        common.core_programdata,
        core.release().programdata(),
    )?;
    core.authenticate_current_deployment(core_observation)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let resolution_observation = deployment_observation(
        common.resolution_program,
        common.resolution_programdata,
        resolution.release().programdata(),
    )?;
    resolution
        .authenticate_current_deployment(resolution_observation)
        .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

fn authenticate_source_records(
    common: CommonAccounts<'_, '_>,
    state: CoreState,
    request: ResolutionRoleRequestV1,
    rent: &Rent,
) -> ProgramResult {
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *common.core_program.key,
        common.source_material,
        common.source_material_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        request.source_material,
        &material_data,
        RecordKind::SourceMaterial,
    )?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    authenticate_material_components(material, state.identity.product_id.to_bytes())?;
    if material
        .policy()
        .map_err(|_| ResolutionError::SourceMaterial)?
        .result_domain_id()
        .to_bytes()
        != state.identity.result_domain.to_bytes()
    {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *common.core_program.key,
        common.capability_manifest,
        common.capability_manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_create<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    common: CommonAccounts<'_, 'info>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    require_revisions(envelope, 0, 0)?;
    let system = accounts.get(17).ok_or(ResolutionError::AccountFrame)?;
    if system.key != &system_program::ID
        || !system.executable
        || system.is_writable
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, manifest, request)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;

    let (expected_source, source_bump) = Pubkey::find_program_address(
        &[
            dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
            common.market.key.as_ref(),
            &authenticated.state.identity.generation.to_le_bytes(),
        ],
        program_id,
    );
    if common.source_state.key != &expected_source {
        return Err(ResolutionError::OutputState.into());
    }
    require_prepaid_output(
        common.source_state,
        rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES),
    )?;
    let source_plan = SourceResolutionStateV1::fresh(
        common.market.key.to_bytes(),
        authenticated.state.identity.generation,
        dclutch_source_contract::ContentId::new(request.source_material)
            .map_err(|_| ResolutionError::SourceMaterial)?,
        request.beneficiary,
        source_bump,
        0,
        0,
    )
    .map_err(|_| ResolutionError::Transition)?;
    let source = source_plan.state();

    let recovery = new_funding(
        program_id,
        common.market,
        common.recovery_funding,
        manifest_id,
        manifest,
        request.recovery_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let exhaustion = new_funding(
        program_id,
        common.market,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        request.exhaustion_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let failure = new_funding(
        program_id,
        common.market,
        common.failure_funding,
        manifest_id,
        manifest,
        request.failure_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let source_bytes = source.to_bytes();
    let recovery_bytes = recovery.to_bytes();
    let exhaustion_bytes = exhaustion.to_bytes();
    let failure_bytes = failure.to_bytes();
    let post_digest = poststate_digest(
        request.action,
        &source_bytes,
        &recovery_bytes,
        &exhaustion_bytes,
        &failure_bytes,
        None,
    )?;

    initialize_source_output(
        program_id,
        common.market,
        common.source_state,
        system,
        authenticated.state.identity.generation,
        source_bump,
        rent,
    )?;
    initialize_funding_output(
        program_id,
        common.recovery_funding,
        common.market,
        manifest_id,
        request.recovery_entry_index,
        authenticated.state.identity.generation,
        manifest,
        recovery,
        system,
    )?;
    initialize_funding_output(
        program_id,
        common.exhaustion_funding,
        common.market,
        manifest_id,
        request.exhaustion_entry_index,
        authenticated.state.identity.generation,
        manifest,
        exhaustion,
        system,
    )?;
    initialize_funding_output(
        program_id,
        common.failure_funding,
        common.market,
        manifest_id,
        request.failure_entry_index,
        authenticated.state.identity.generation,
        manifest,
        failure,
        system,
    )?;
    drop(manifest_data);
    drop(material_data);
    write_state(common.source_state, &source_bytes)?;
    write_state(common.recovery_funding, &recovery_bytes)?;
    write_state(common.exhaustion_funding, &exhaustion_bytes)?;
    write_state(common.failure_funding, &failure_bytes)?;
    return_ack(
        program_id,
        envelope,
        authenticated.full_effect_digest,
        post_digest,
        0,
        0,
        0,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_verify(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    require_revisions(envelope, 0, 0)?;
    let beneficiary = accounts.get(16).ok_or(ResolutionError::AccountFrame)?;
    let clock_account = accounts.get(17).ok_or(ResolutionError::AccountFrame)?;
    if beneficiary.key.to_bytes() != request.beneficiary
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
        || !beneficiary.is_writable
        || beneficiary.executable
        || clock_account.is_writable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let clock = authenticate_clock(clock_account)?;
    if clock.slot == 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, manifest, request)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_bytes = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source =
        SourceResolutionStateV1::decode(&source_bytes).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account(program_id, common.source_state, source)?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != common.market.key.to_bytes()
        || source.generation() != authenticated.state.identity.generation
        || source.material_id().to_bytes() != request.source_material
        || source.rent_beneficiary() != request.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    let mut recovery = load_funding(
        program_id,
        common.market,
        common.recovery_funding,
        manifest_id,
        manifest,
        request.recovery_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let mut exhaustion = load_funding(
        program_id,
        common.market,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        request.exhaustion_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let mut failure = load_funding(
        program_id,
        common.market,
        common.failure_funding,
        manifest_id,
        manifest,
        request.failure_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    if recovery.status() != FundingStatus::Pending
        || exhaustion.status() != FundingStatus::Pending
        || failure.status() != FundingStatus::Pending
    {
        return Err(ResolutionError::Funding.into());
    }
    let recovery_debit = activate_funding(
        &mut recovery,
        common.recovery_funding,
        manifest_id,
        manifest,
        clock.slot,
        rent,
    )?;
    let exhaustion_debit = activate_funding(
        &mut exhaustion,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        clock.slot,
        rent,
    )?;
    let failure_debit = activate_funding(
        &mut failure,
        common.failure_funding,
        manifest_id,
        manifest,
        clock.slot,
        rent,
    )?;
    let total_debit = recovery_debit
        .checked_add(exhaustion_debit)
        .and_then(|value| value.checked_add(failure_debit))
        .ok_or(ResolutionError::Arithmetic)?;
    let recovery_lamports = common
        .recovery_funding
        .lamports()
        .checked_sub(recovery_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    let exhaustion_lamports = common
        .exhaustion_funding
        .lamports()
        .checked_sub(exhaustion_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    let failure_lamports = common
        .failure_funding
        .lamports()
        .checked_sub(failure_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = beneficiary
        .lamports()
        .checked_add(total_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    let recovery_bytes = recovery.to_bytes();
    let exhaustion_bytes = exhaustion.to_bytes();
    let failure_bytes = failure.to_bytes();
    validate_post_funding(recovery, recovery_lamports, manifest_id, manifest, rent)?;
    validate_post_funding(exhaustion, exhaustion_lamports, manifest_id, manifest, rent)?;
    validate_post_funding(failure, failure_lamports, manifest_id, manifest, rent)?;
    let post_digest = poststate_digest(
        request.action,
        &source_bytes,
        &recovery_bytes,
        &exhaustion_bytes,
        &failure_bytes,
        None,
    )?;
    drop(source_bytes);
    drop(manifest_data);
    drop(material_data);
    commit_activated_funding(
        common.recovery_funding,
        &recovery_bytes,
        recovery_lamports,
        common.exhaustion_funding,
        &exhaustion_bytes,
        exhaustion_lamports,
        common.failure_funding,
        &failure_bytes,
        failure_lamports,
        beneficiary,
        beneficiary_lamports,
    )?;
    return_ack(
        program_id,
        envelope,
        authenticated.full_effect_digest,
        post_digest,
        0,
        0,
        0,
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_admit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    let certificate_account = accounts.get(16).ok_or(ResolutionError::AccountFrame)?;
    if certificate_account.key.to_bytes() != request.receipt
        || certificate_account.is_writable
        || certificate_account.executable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_data = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source = authenticate_terminal_source(
        program_id,
        common,
        request,
        authenticated.state,
        &source_data,
    )?;
    let decision = source
        .decision(
            material
                .result_domain()
                .map_err(|_| ResolutionError::ProductDomain)?
                .outcome_count(),
        )
        .map_err(|_| ResolutionError::Transition)?;
    if decision.terminal_sequence() != request.receipt_sequence {
        return Err(ResolutionError::Transition.into());
    }
    require_revisions(envelope, request.receipt_sequence, 1)?;
    let recovery = load_active_funding(
        program_id,
        common.market,
        common.recovery_funding,
        manifest_id,
        manifest,
        request.recovery_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let exhaustion = load_active_funding(
        program_id,
        common.market,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        request.exhaustion_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let failure = load_active_funding(
        program_id,
        common.market,
        common.failure_funding,
        manifest_id,
        manifest,
        request.failure_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let certificate = authenticate_terminal_certificate(
        program_id,
        common.source_state,
        certificate_account,
        request.receipt_kind,
        request.receipt_sequence,
        request.source_material,
        common.market.key.to_bytes(),
        authenticated.state.identity.product_id.to_bytes(),
        authenticated.state.identity.generation,
        decision.selector(),
        &certificate_data,
        rent,
    )?;
    let recovery_bytes = recovery.to_bytes();
    let exhaustion_bytes = exhaustion.to_bytes();
    let failure_bytes = failure.to_bytes();
    let post_digest = poststate_digest(
        request.action,
        &source_data,
        &recovery_bytes,
        &exhaustion_bytes,
        &failure_bytes,
        Some(&certificate_data),
    )?;
    let _ = certificate;
    return_ack(
        program_id,
        envelope,
        authenticated.full_effect_digest,
        post_digest,
        request.receipt_sequence,
        request.receipt_sequence,
        1,
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_close<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    common: CommonAccounts<'_, 'info>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    let certificate_account = accounts.get(16).ok_or(ResolutionError::AccountFrame)?;
    let closure_account = accounts.get(17).ok_or(ResolutionError::AccountFrame)?;
    let beneficiary = accounts.get(18).ok_or(ResolutionError::AccountFrame)?;
    let clock_account = accounts.get(19).ok_or(ResolutionError::AccountFrame)?;
    let system = accounts.get(21).ok_or(ResolutionError::AccountFrame)?;
    let expected_terminal = authenticated
        .state
        .terminal_receipt
        .ok_or(ResolutionError::MarketAuthority)?
        .to_bytes();
    if certificate_account.key.to_bytes() != expected_terminal
        || certificate_account.is_writable
        || closure_account.key.to_bytes() != request.receipt
        || !closure_account.is_writable
        || beneficiary.key.to_bytes() != request.beneficiary
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
        || !beneficiary.is_writable
        || beneficiary.executable
        || clock_account.is_writable
        || system.key != &system_program::ID
        || !system.executable
        || system.is_writable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let clock = authenticate_clock(clock_account)?;
    if clock.unix_timestamp <= 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material = SourceMaterialViewV1::decode(&material_data)
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_data = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut source = authenticate_terminal_source(
        program_id,
        common,
        request,
        authenticated.state,
        &source_data,
    )?;
    let decision = source
        .decision(
            material
                .result_domain()
                .map_err(|_| ResolutionError::ProductDomain)?
                .outcome_count(),
        )
        .map_err(|_| ResolutionError::Transition)?;
    let closure_sequence = decision
        .terminal_sequence()
        .checked_add(1)
        .ok_or(ResolutionError::Arithmetic)?;
    if closure_sequence != request.receipt_sequence
        || source.rent_beneficiary() != request.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    require_revisions(envelope, decision.terminal_sequence(), 1)?;
    source
        .retire(
            authenticated.state.identity.generation,
            clock.unix_timestamp,
            1,
            1,
        )
        .map_err(|_| ResolutionError::Transition)?;
    let recovery = load_active_funding(
        program_id,
        common.market,
        common.recovery_funding,
        manifest_id,
        manifest,
        request.recovery_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let exhaustion = load_active_funding(
        program_id,
        common.market,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        request.exhaustion_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let failure = load_active_funding(
        program_id,
        common.market,
        common.failure_funding,
        manifest_id,
        manifest,
        request.failure_entry_index,
        authenticated.state.identity.generation,
        rent,
    )?;
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let terminal_kind = match source.phase() {
        SourceResolutionPhaseV1::Retired => {
            if decision.route() == dclutch_source_contract::SourceResolutionRouteV1::Failure {
                ResolutionCoreReceiptKindV1::TerminalFailure
            } else {
                ResolutionCoreReceiptKindV1::TerminalSuccess
            }
        }
        _ => return Err(ResolutionError::Transition.into()),
    };
    authenticate_terminal_certificate(
        program_id,
        common.source_state,
        certificate_account,
        terminal_kind,
        decision.terminal_sequence(),
        request.source_material,
        common.market.key.to_bytes(),
        authenticated.state.identity.product_id.to_bytes(),
        authenticated.state.identity.generation,
        decision.selector(),
        &certificate_data,
        rent,
    )?;
    let recovery_data = common
        .recovery_funding
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let exhaustion_data = common
        .exhaustion_funding
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let failure_data = common
        .failure_funding
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let funding_set_digest = hashv(&[
        SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1,
        &recovery_data,
        &exhaustion_data,
        &failure_data,
    ])
    .to_bytes();
    let recovery_refund = funding_refund(
        recovery,
        common.recovery_funding,
        manifest_id,
        manifest,
        request.beneficiary,
        rent,
    )?;
    let exhaustion_refund = funding_refund(
        exhaustion,
        common.exhaustion_funding,
        manifest_id,
        manifest,
        request.beneficiary,
        rent,
    )?;
    let failure_refund = funding_refund(
        failure,
        common.failure_funding,
        manifest_id,
        manifest,
        request.beneficiary,
        rent,
    )?;
    let source_refund = common.source_state.lamports();
    if source_refund < rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES) {
        return Err(ResolutionError::Funding.into());
    }
    let refund_lamports = source_refund
        .checked_add(recovery_refund)
        .and_then(|value| value.checked_add(exhaustion_refund))
        .and_then(|value| value.checked_add(failure_refund))
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = beneficiary
        .lamports()
        .checked_add(refund_lamports)
        .ok_or(ResolutionError::Arithmetic)?;
    let closure = SourceClosureReceiptV1 {
        market: common.market.key.to_bytes(),
        source_state: common.source_state.key.to_bytes(),
        source_material: request.source_material,
        capability_manifest: request.capability_manifest,
        terminal_certificate: certificate_account.key.to_bytes(),
        receipt_account: closure_account.key.to_bytes(),
        beneficiary: request.beneficiary,
        source_state_digest: hash(&source_data).to_bytes(),
        terminal_certificate_digest: hash(&certificate_data).to_bytes(),
        funding_set_digest,
        generation: authenticated.state.identity.generation,
        terminal_sequence: decision.terminal_sequence(),
        selector: u32::from(decision.selector()),
        refund_lamports,
        closed_at: u64::try_from(clock.unix_timestamp).map_err(|_| ResolutionError::Arithmetic)?,
    };
    let closure_bytes = closure
        .to_bytes()
        .map_err(|_| ResolutionError::OutputState)?;
    let post_digest = poststate_digest(request.action, &closure_bytes, &[], &[], &[], None)?;
    drop(failure_data);
    drop(exhaustion_data);
    drop(recovery_data);
    drop(certificate_data);
    drop(source_data);
    drop(manifest_data);
    drop(material_data);
    initialize_closure_output(
        program_id,
        common.source_state,
        closure_account,
        request.receipt_sequence,
        system,
        rent,
    )?;
    write_state(closure_account, &closure_bytes)?;
    commit_refund(
        common.source_state,
        common.recovery_funding,
        common.exhaustion_funding,
        common.failure_funding,
        beneficiary,
        beneficiary_lamports,
    )?;
    return_ack(
        program_id,
        envelope,
        authenticated.full_effect_digest,
        post_digest,
        decision.terminal_sequence(),
        closure_sequence,
        1,
        2,
    )
}

fn authenticate_funding_entries(
    material: SourceMaterialViewV1<'_>,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV1,
) -> ProgramResult {
    let (recovery_policy_id, recovery_policy) = material
        .recovery_policy()
        .map_err(|_| ResolutionError::SourceMaterial)?
        .ok_or(ResolutionError::SourceMaterial)?;
    if recovery_policy.attempt_count() != 1 {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let recovery_allocation = recovery_policy
        .attempt(0)
        .map_err(|_| ResolutionError::SourceMaterial)?
        .funding_allocation_id()
        .to_bytes();
    for (index, expected_config) in [
        (request.recovery_entry_index, recovery_allocation),
        (
            request.exhaustion_entry_index,
            recovery_policy_id.to_bytes(),
        ),
        (request.failure_entry_index, request.source_material),
    ] {
        let entry = manifest
            .entry(index)
            .map_err(|_| ResolutionError::Funding)?;
        if entry.config_id().to_bytes() != expected_config
            || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4
        {
            return Err(ResolutionError::Funding.into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn new_funding(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    generation: u64,
    rent: &Rent,
) -> Result<FundingStateV1, ProgramError> {
    require_prepaid_output(account, rent.minimum_balance(FUNDING_STATE_BYTES))?;
    let custody = FundingCustodyObservationV1::native_only(
        account.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    let funding = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *account.key {
        return Err(ResolutionError::Funding.into());
    }
    Ok(funding)
}

#[allow(clippy::too_many_arguments)]
fn load_funding(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    generation: u64,
    rent: &Rent,
) -> Result<FundingStateV1, ProgramError> {
    if account.owner != program_id
        || account.data_len() != FUNDING_STATE_BYTES
        || account.executable
    {
        return Err(ResolutionError::Funding.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| ResolutionError::Funding)?;
    if funding.entry_index() != entry_index {
        return Err(ResolutionError::Funding.into());
    }
    let custody = FundingCustodyObservationV1::native_only(
        account.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    funding
        .validate_against(manifest_id, manifest, custody)
        .map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *account.key {
        return Err(ResolutionError::Funding.into());
    }
    Ok(funding)
}

#[allow(clippy::too_many_arguments)]
fn load_active_funding(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    generation: u64,
    rent: &Rent,
) -> Result<FundingStateV1, ProgramError> {
    let funding = load_funding(
        program_id,
        market,
        account,
        manifest_id,
        manifest,
        entry_index,
        generation,
        rent,
    )?;
    if funding.status() != FundingStatus::Active {
        return Err(ResolutionError::Funding.into());
    }
    Ok(funding)
}

fn activate_funding(
    funding: &mut FundingStateV1,
    account: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    slot: u64,
    rent: &Rent,
) -> Result<u64, ProgramError> {
    let custody = FundingCustodyObservationV1::native_only(
        account.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    let debit = funding
        .activate(manifest_id, manifest, custody, slot)
        .map_err(|_| ResolutionError::Funding)?;
    debit
        .rent_lamports()
        .checked_add(debit.creation_lamports())
        .ok_or(ResolutionError::Arithmetic.into())
}

fn validate_post_funding(
    funding: FundingStateV1,
    lamports: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    rent: &Rent,
) -> ProgramResult {
    funding
        .validate_against(
            manifest_id,
            manifest,
            FundingCustodyObservationV1::native_only(
                lamports,
                rent.minimum_balance(FUNDING_STATE_BYTES),
            )
            .map_err(|_| ResolutionError::Funding)?,
        )
        .map_err(|_| ResolutionError::Funding.into())
}

fn require_revisions(
    envelope: CoreEffectEnvelopeV1,
    resource_a: u64,
    resource_b: u64,
) -> ProgramResult {
    if envelope.expected_resource_a_revision() != resource_a
        || envelope.expected_resource_b_revision() != resource_b
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(())
}

fn action_byte(action: ResolutionCoreActionV1) -> u8 {
    match action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund as u8,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady as u8,
        ResolutionCoreActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal as u8,
        ResolutionCoreActionV1::CloseFund => CoreEffectActionV1::CloseFund as u8,
    }
}

fn poststate_digest(
    action: ResolutionCoreActionV1,
    source_or_closure: &[u8],
    recovery: &[u8],
    exhaustion: &[u8],
    failure: &[u8],
    certificate: Option<&[u8]>,
) -> Result<Identity, ProgramError> {
    let action = [action_byte(action)];
    Identity::new(
        hashv(&[
            RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1,
            &action,
            source_or_closure,
            recovery,
            exhaustion,
            failure,
            certificate.unwrap_or(&[]),
        ])
        .to_bytes(),
    )
    .map_err(|_| ResolutionError::OutputState.into())
}

fn require_prepaid_output(account: &AccountInfo<'_>, minimum_lamports: u64) -> ProgramResult {
    if account.owner != &system_program::ID
        || account.data_len() != 0
        || account.executable
        || !account.is_writable
        || account.lamports() < minimum_lamports
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize_source_output<'info>(
    program_id: &Pubkey,
    market: &AccountInfo<'info>,
    output: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    generation: u64,
    bump: u8,
    rent: &Rent,
) -> ProgramResult {
    let generation_seed = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
        market.key.as_ref(),
        &generation_seed,
        &bump_seed,
    ];
    let space =
        u64::try_from(SOURCE_RESOLUTION_STATE_BYTES).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id
        || output.executable
        || output.data_len() != SOURCE_RESOLUTION_STATE_BYTES
        || output.lamports() < rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES)
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize_funding_output<'info>(
    program_id: &Pubkey,
    output: &AccountInfo<'info>,
    market: &AccountInfo<'info>,
    manifest_id: CapabilityContentId,
    entry_index: u16,
    generation: u64,
    manifest: CapabilityManifestV1<'_>,
    funding: FundingStateV1,
    system: &AccountInfo<'info>,
) -> ProgramResult {
    if funding.entry_index() != entry_index {
        return Err(ResolutionError::Funding.into());
    }
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionError::Funding)?;
    let (_, bump) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    let components = derivation.seed_components();
    let [
        domain,
        market_seed,
        generation_seed,
        entry_seed,
        config_seed,
        release_seed,
    ] = components;
    let bump_seed = [bump];
    let signer: [&[u8]; 7] = [
        domain,
        market_seed,
        generation_seed,
        entry_seed,
        config_seed,
        release_seed,
        &bump_seed,
    ];
    let space = u64::try_from(FUNDING_STATE_BYTES).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id || output.executable || output.data_len() != FUNDING_STATE_BYTES {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn write_state(account: &AccountInfo<'_>, bytes: &[u8]) -> ProgramResult {
    let mut output = account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if output.len() != bytes.len() || output.iter().any(|byte| *byte != 0) {
        return Err(ResolutionError::OutputState.into());
    }
    output.copy_from_slice(bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn return_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    full_effect_digest: Identity,
    post_digest: Identity,
    pre_a: u64,
    post_a: u64,
    pre_b: u64,
    post_b: u64,
) -> ProgramResult {
    let encoded = build_ack(
        program_id,
        envelope,
        full_effect_digest,
        post_digest,
        pre_a,
        post_a,
        pre_b,
        post_b,
    )?;
    set_return_data(&encoded);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    full_effect_digest: Identity,
    post_digest: Identity,
    pre_a: u64,
    post_a: u64,
    pre_b: u64,
    post_b: u64,
) -> Result<[u8; CORE_EFFECT_ACK_BYTES_V1], ProgramError> {
    let role_program =
        Identity::new(program_id.to_bytes()).map_err(|_| ResolutionError::ResolutionRelease)?;
    let ack = CoreEffectAckV1::new(
        envelope.action(),
        Role::Resolution,
        role_program,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        full_effect_digest,
        post_digest,
        pre_a,
        post_a,
        pre_b,
        post_b,
    )
    .map_err(|_| ResolutionError::Transition)?;
    let encoded = ack.encode().map_err(|_| ResolutionError::Transition)?;
    if encoded.len() != CORE_EFFECT_ACK_BYTES_V1 {
        return Err(ResolutionError::Transition.into());
    }
    Ok(encoded)
}

#[allow(clippy::too_many_arguments)]
fn commit_activated_funding(
    recovery: &AccountInfo<'_>,
    recovery_bytes: &[u8; FUNDING_STATE_BYTES],
    recovery_lamports_after: u64,
    exhaustion: &AccountInfo<'_>,
    exhaustion_bytes: &[u8; FUNDING_STATE_BYTES],
    exhaustion_lamports_after: u64,
    failure: &AccountInfo<'_>,
    failure_bytes: &[u8; FUNDING_STATE_BYTES],
    failure_lamports_after: u64,
    beneficiary: &AccountInfo<'_>,
    beneficiary_lamports_after: u64,
) -> ProgramResult {
    let mut recovery_data = recovery
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut exhaustion_data = exhaustion
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut failure_data = failure
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut recovery_lamports = recovery
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut exhaustion_lamports = exhaustion
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut failure_lamports = failure
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut beneficiary_lamports = beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if recovery_data.len() != FUNDING_STATE_BYTES
        || exhaustion_data.len() != FUNDING_STATE_BYTES
        || failure_data.len() != FUNDING_STATE_BYTES
    {
        return Err(ResolutionError::OutputState.into());
    }
    recovery_data.copy_from_slice(recovery_bytes);
    exhaustion_data.copy_from_slice(exhaustion_bytes);
    failure_data.copy_from_slice(failure_bytes);
    **recovery_lamports = recovery_lamports_after;
    **exhaustion_lamports = exhaustion_lamports_after;
    **failure_lamports = failure_lamports_after;
    **beneficiary_lamports = beneficiary_lamports_after;
    Ok(())
}

fn authenticate_terminal_source(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: ResolutionRoleRequestV1,
    state: CoreState,
    bytes: &[u8],
) -> Result<SourceResolutionStateV1, ProgramError> {
    let source =
        SourceResolutionStateV1::decode(bytes).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account(program_id, common.source_state, source)?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) || source.market() != common.market.key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(source)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_terminal_certificate(
    program_id: &Pubkey,
    source_state: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    receipt_kind: ResolutionCoreReceiptKindV1,
    sequence: u64,
    source_material: [u8; 32],
    market: [u8; 32],
    product: [u8; 32],
    generation: u64,
    selector: u8,
    bytes: &[u8],
    rent: &Rent,
) -> Result<ResolutionCertificateV1, ProgramError> {
    let (expected_kind, kind_tag) = match receipt_kind {
        ResolutionCoreReceiptKindV1::TerminalSuccess => {
            (ResolutionCertificateKindV1::ResolutionSuccess, 1_u8)
        }
        ResolutionCoreReceiptKindV1::TerminalFailure => {
            (ResolutionCertificateKindV1::ResolutionFailure, 4_u8)
        }
        ResolutionCoreReceiptKindV1::None | ResolutionCoreReceiptKindV1::Closure => {
            return Err(ResolutionError::Transition.into());
        }
    };
    if account.owner != program_id
        || account.executable
        || account.data_len() != RESOLUTION_CERTIFICATE_BYTES
        || account.lamports() < rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES)
    {
        return Err(ResolutionError::OutputState.into());
    }
    let certificate =
        ResolutionCertificateV1::decode(bytes).map_err(|_| ResolutionError::OutputState)?;
    if certificate.kind != expected_kind
        || certificate.market != market
        || certificate.source_material != source_material
        || certificate.product != product
        || certificate.receipt_account != account.key.to_bytes()
        || certificate.generation != generation
        || certificate.selector != u32::from(selector)
    {
        return Err(ResolutionError::Transition.into());
    }
    let kind_seed = [kind_tag];
    let sequence_seed = sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    )
    .0;
    if account.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(certificate)
}

fn funding_refund(
    funding: FundingStateV1,
    account: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    beneficiary: [u8; 32],
    rent: &Rent,
) -> Result<u64, ProgramError> {
    let custody = FundingCustodyObservationV1::native_only(
        account.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    let plan = funding
        .close(manifest_id, manifest, custody, beneficiary)
        .map_err(|_| ResolutionError::Funding)?;
    if plan.native_rent_credit() != beneficiary
        || plan.realm_token_beneficiary().is_some()
        || plan.remaining_realm_collateral() != 0
        || plan.realm_collateral_donation() != 0
        || plan.vault_rent_lamports() != 0
        || plan.vault_lamport_donation() != 0
    {
        return Err(ResolutionError::Funding.into());
    }
    let refund = plan
        .remaining_native_lamports()
        .checked_add(plan.state_rent_lamports())
        .and_then(|value| value.checked_add(plan.state_lamport_donation()))
        .ok_or(ResolutionError::Arithmetic)?;
    if refund != account.lamports() {
        return Err(ResolutionError::Funding.into());
    }
    Ok(refund)
}

fn initialize_closure_output<'info>(
    program_id: &Pubkey,
    source_state: &AccountInfo<'info>,
    output: &AccountInfo<'info>,
    sequence: u64,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let sequence_seed = sequence.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1,
            source_state.key.as_ref(),
            &sequence_seed,
        ],
        program_id,
    );
    if output.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    require_prepaid_output(output, rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES))?;
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1,
        source_state.key.as_ref(),
        &sequence_seed,
        &bump_seed,
    ];
    let space =
        u64::try_from(SOURCE_CLOSURE_RECEIPT_BYTES).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id
        || output.executable
        || output.data_len() != SOURCE_CLOSURE_RECEIPT_BYTES
        || output.lamports() < rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES)
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn commit_refund(
    source: &AccountInfo<'_>,
    recovery: &AccountInfo<'_>,
    exhaustion: &AccountInfo<'_>,
    failure: &AccountInfo<'_>,
    beneficiary: &AccountInfo<'_>,
    beneficiary_lamports_after: u64,
) -> ProgramResult {
    let mut source_data = source
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut recovery_data = recovery
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut exhaustion_data = exhaustion
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut failure_data = failure
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut source_lamports = source
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut recovery_lamports = recovery
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut exhaustion_lamports = exhaustion
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut failure_lamports = failure
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut beneficiary_lamports = beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if source_data.len() != SOURCE_RESOLUTION_STATE_BYTES
        || recovery_data.len() != FUNDING_STATE_BYTES
        || exhaustion_data.len() != FUNDING_STATE_BYTES
        || failure_data.len() != FUNDING_STATE_BYTES
    {
        return Err(ResolutionError::OutputState.into());
    }
    source_data.fill(0);
    recovery_data.fill(0);
    exhaustion_data.fill(0);
    failure_data.fill(0);
    **source_lamports = 0;
    **recovery_lamports = 0;
    **exhaustion_lamports = 0;
    **failure_lamports = 0;
    **beneficiary_lamports = beneficiary_lamports_after;
    Ok(())
}

fn next<'a, 'info>(
    iterator: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iterator).map_err(|_| ResolutionError::AccountFrame.into())
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{
        CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CapabilityFundingHeaderV1, CoreEffectAckV1,
        CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, Role,
    };
    use dclutch_resolution_codec::{
        RESOLUTION_CORE_ROLE_REQUEST_BYTES, RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1,
        ResolutionCoreActionV1, ResolutionCoreReceiptKindV1, ResolutionRoleRequestV1,
    };
    use solana_program::{
        hash::{hash, hashv},
        pubkey::Pubkey,
    };

    use super::{
        CORE_EFFECT_INSTRUCTION_BYTES, action_byte, authenticate_action,
        authenticate_funding_header, build_ack, is_core_effect, poststate_digest,
        require_revisions,
    };
    use crate::ResolutionError;

    fn identity(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("nonzero identity")
    }

    fn request(action: ResolutionCoreActionV1) -> ResolutionRoleRequestV1 {
        let (receipt_kind, receipt, beneficiary, receipt_sequence) = match action {
            ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
                (ResolutionCoreReceiptKindV1::None, [0; 32], [8; 32], 0)
            }
            ResolutionCoreActionV1::AdmitTerminal => (
                ResolutionCoreReceiptKindV1::TerminalSuccess,
                [7; 32],
                [0; 32],
                3,
            ),
            ResolutionCoreActionV1::CloseFund => {
                (ResolutionCoreReceiptKindV1::Closure, [7; 32], [8; 32], 4)
            }
        };
        ResolutionRoleRequestV1 {
            action,
            receipt_kind,
            source_state: [1; 32],
            source_material: [2; 32],
            capability_manifest: [3; 32],
            recovery_funding: [4; 32],
            exhaustion_funding: [5; 32],
            failure_funding: [6; 32],
            receipt,
            beneficiary,
            recovery_entry_index: 0,
            exhaustion_entry_index: 1,
            failure_entry_index: 2,
            receipt_sequence,
        }
    }

    fn envelope(
        action: ResolutionCoreActionV1,
        expected_a: u64,
        expected_b: u64,
    ) -> CoreEffectEnvelopeV1 {
        let role_bytes = role_bytes(action);
        let core_action = match action {
            ResolutionCoreActionV1::CreateFund => {
                dclutch_market_core_codec::CoreEffectActionV1::CreateFund
            }
            ResolutionCoreActionV1::VerifyFundReady => {
                dclutch_market_core_codec::CoreEffectActionV1::VerifyFundReady
            }
            ResolutionCoreActionV1::AdmitTerminal => {
                dclutch_market_core_codec::CoreEffectActionV1::AdmitTerminal
            }
            ResolutionCoreActionV1::CloseFund => {
                dclutch_market_core_codec::CoreEffectActionV1::CloseFund
            }
        };
        CoreEffectEnvelopeV1::new(
            core_action,
            Role::Resolution,
            identity(9),
            identity(10),
            identity(11),
            identity(12),
            identity(1),
            identity(13),
            Identity::new(hash(&role_bytes).to_bytes()).expect("request digest"),
            1,
            expected_a,
            expected_b,
            u32::try_from(role_bytes.len()).expect("fixed request width"),
        )
        .expect("envelope")
    }

    fn role_bytes(
        action: ResolutionCoreActionV1,
    ) -> [u8; CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1 + RESOLUTION_CORE_ROLE_REQUEST_BYTES] {
        let request = request(action);
        let mut output =
            [0_u8; CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1 + RESOLUTION_CORE_ROLE_REQUEST_BYTES];
        output
            .get_mut(..CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
            .expect("funding prefix")
            .copy_from_slice(
                &CapabilityFundingHeaderV1::new(3)
                    .expect("three funds")
                    .encode(),
            );
        output
            .get_mut(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
            .expect("request tail")
            .copy_from_slice(&request.to_bytes().expect("request encodes"));
        output
    }

    #[test]
    fn exact_core_effect_dispatch_and_action_partition() {
        for action in [
            ResolutionCoreActionV1::CreateFund,
            ResolutionCoreActionV1::VerifyFundReady,
            ResolutionCoreActionV1::AdmitTerminal,
            ResolutionCoreActionV1::CloseFund,
        ] {
            let envelope = envelope(action, 0, 0);
            authenticate_action(envelope, request(action)).expect("matching action");
            assert_eq!(action_byte(action), envelope.action() as u8);
            let role_bytes = role_bytes(action);
            let funding_header = CapabilityFundingHeaderV1::decode(
                role_bytes
                    .get(..CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
                    .expect("funding header"),
            )
            .expect("composite role bytes");
            authenticate_funding_header(funding_header).expect("exact funding count");
            let request_tail = role_bytes
                .get(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
                .expect("request tail");
            assert_eq!(request_tail, request(action).to_bytes().expect("request"));
            let mut instruction = [0_u8; CORE_EFFECT_INSTRUCTION_BYTES];
            let envelope_bytes = envelope.encode().expect("envelope encodes");
            instruction
                .get_mut(..envelope_bytes.len())
                .expect("envelope prefix")
                .copy_from_slice(&envelope_bytes);
            instruction
                .get_mut(envelope_bytes.len()..)
                .expect("request tail")
                .copy_from_slice(&role_bytes);
            assert!(is_core_effect(&instruction));
            let short = instruction
                .get(..instruction.len().saturating_sub(1))
                .expect("short instruction");
            assert!(!is_core_effect(short));
        }
        assert_eq!(
            authenticate_action(
                envelope(ResolutionCoreActionV1::CreateFund, 0, 0),
                request(ResolutionCoreActionV1::VerifyFundReady),
            ),
            Err(ResolutionError::Instruction.into())
        );

        let exact = role_bytes(ResolutionCoreActionV1::CreateFund);
        let wrong_count = CapabilityFundingHeaderV1::new(2).expect("bounded count");
        assert_eq!(
            authenticate_funding_header(wrong_count),
            Err(ResolutionError::Instruction.into())
        );

        let mut hostile_header = exact;
        hostile_header[0] ^= 1;
        assert!(
            CapabilityFundingHeaderV1::decode(
                hostile_header
                    .get(..CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
                    .expect("funding header"),
            )
            .is_err()
        );

        let envelope = envelope(ResolutionCoreActionV1::CreateFund, 0, 0);
        let exact_digest = Identity::new(hash(&exact).to_bytes()).expect("composite digest");
        envelope
            .validate_role_request(exact.len(), exact_digest)
            .expect("full composite is bound");
        let tail = exact
            .get(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
            .expect("child request tail");
        let tail_digest = Identity::new(hash(tail).to_bytes()).expect("tail digest");
        assert!(
            envelope
                .validate_role_request(tail.len(), tail_digest)
                .is_err()
        );
    }

    #[test]
    fn revisions_and_poststate_digest_refuse_substitution() {
        let envelope = envelope(ResolutionCoreActionV1::AdmitTerminal, 3, 1);
        require_revisions(envelope, 3, 1).expect("exact revisions");
        assert_eq!(
            require_revisions(envelope, 2, 1),
            Err(ResolutionError::Transition.into())
        );
        let exact = poststate_digest(
            ResolutionCoreActionV1::AdmitTerminal,
            &[1],
            &[2],
            &[3],
            &[4],
            Some(&[5]),
        )
        .expect("digest");
        let reordered = poststate_digest(
            ResolutionCoreActionV1::AdmitTerminal,
            &[1],
            &[3],
            &[2],
            &[4],
            Some(&[5]),
        )
        .expect("digest");
        let no_certificate = poststate_digest(
            ResolutionCoreActionV1::AdmitTerminal,
            &[1],
            &[2],
            &[3],
            &[4],
            None,
        )
        .expect("digest");
        assert_ne!(exact, reordered);
        assert_ne!(exact, no_certificate);
        let action = [CoreEffectActionV1::AdmitTerminal as u8];
        let core_derived = Identity::new(
            hashv(&[
                RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1,
                &action,
                &[1],
                &[2],
                &[3],
                &[4],
                &[5],
            ])
            .to_bytes(),
        )
        .expect("Core-derived poststate digest");
        assert_eq!(exact, core_derived);
    }

    #[test]
    fn acknowledgement_is_the_only_return_wire_and_binds_effect() {
        let program_id = Pubkey::new_from_array([21; 32]);
        let envelope = envelope(ResolutionCoreActionV1::VerifyFundReady, 0, 0);
        let effect_digest = identity(22);
        let post_digest = identity(23);
        let bytes = build_ack(
            &program_id,
            envelope,
            effect_digest,
            post_digest,
            0,
            0,
            0,
            1,
        )
        .expect("ack encodes");
        let ack = CoreEffectAckV1::decode(&bytes).expect("one exact ack");
        assert_eq!(ack.post_resource_digest(), post_digest);
        ack.validate_for(
            envelope,
            Identity::new(program_id.to_bytes()).expect("program identity"),
            effect_digest,
        )
        .expect("effect binding");
        assert_eq!(ack.post_resource_b_revision(), 1);
    }
}
