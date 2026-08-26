//! Physical authentication of family-neutral Execution Strategy V2 records.
//!
//! The selected Capability Program digest comes from the authenticated Trading
//! root. Every semantic child is then reauthenticated as an exact finalized
//! Registry record. AOT dispositions additionally authenticate the exact
//! Certificate, optional Admission, immutable ArtifactRelease, and its current
//! Upgradeable Loader V3 Program/ProgramData/complete-ELF observation. This
//! module is read-only: it grants no accelerator state or effect write authority.

use dclutch_capability_program_contract::v3::{
    CapabilityProgramV3, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V3,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AdmittedAotAuthorizationV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_ADMISSION_BYTES_V2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
    StrategyDispositionV2, validate_admitted_aot_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

/// Exact record-account count for the interpreted disposition.
pub const INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2: usize = 4;
/// Exact record/deployment-account count for shadow AOT.
pub const SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2: usize = 10;
/// Exact record/deployment-account count for admitted AOT.
pub const ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2: usize = 12;

const CAPABILITY_RAW: usize = 0;
const CAPABILITY_STAGING: usize = 1;
const STRATEGY_RAW: usize = 2;
const STRATEGY_STAGING: usize = 3;
const CERTIFICATE_RAW: usize = 4;
const CERTIFICATE_STAGING: usize = 5;
const SHADOW_ARTIFACT_RAW: usize = 6;
const SHADOW_ARTIFACT_STAGING: usize = 7;
const SHADOW_ACCELERATOR_PROGRAM: usize = 8;
const SHADOW_ACCELERATOR_PROGRAMDATA: usize = 9;
const ADMITTED_ADMISSION_RAW: usize = 6;
const ADMITTED_ADMISSION_STAGING: usize = 7;
const ADMITTED_ARTIFACT_RAW: usize = 8;
const ADMITTED_ARTIFACT_STAGING: usize = 9;
const ADMITTED_ACCELERATOR_PROGRAM: usize = 10;
const ADMITTED_ACCELERATOR_PROGRAMDATA: usize = 11;

/// Ephemeral result of the complete Registry-to-Trading authentication chain.
///
/// The value is not a persisted DTO and owns no mutation authority. Its private
/// fields ensure the admitted-AOT witness can only originate from the checked
/// record/deployment path in this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedExecutionStrategyV2 {
    capability_program_id: ContentId,
    capability_program: CapabilityProgramV3,
    strategy_program_id: ContentId,
    strategy: ExecutionStrategyProgramV2,
    certificate_program_id: Option<ContentId>,
    certificate: Option<ExecutionStrategyCertificateV2>,
    admission_program_id: Option<ContentId>,
    artifact_release_id: Option<ArtifactReleaseIdV1>,
    artifact_release: Option<ArtifactReleaseV1>,
    admitted_authorization: Option<AdmittedAotAuthorizationV2>,
}

impl AuthenticatedExecutionStrategyV2 {
    /// Return the selected finalized Capability Program content identity.
    pub const fn capability_program_id(self) -> ContentId {
        self.capability_program_id
    }

    /// Return the hostile-decoded selected Capability Program.
    pub const fn capability_program(self) -> CapabilityProgramV3 {
        self.capability_program
    }

    /// Return the selected finalized Strategy content identity.
    pub const fn strategy_program_id(self) -> ContentId {
        self.strategy_program_id
    }

    /// Return the checked family-neutral Strategy.
    pub const fn strategy(self) -> ExecutionStrategyProgramV2 {
        self.strategy
    }

    /// Return the exact optional finalized Certificate identity.
    pub const fn certificate_program_id(self) -> Option<ContentId> {
        self.certificate_program_id
    }

    /// Return the exact optional checked Certificate.
    pub const fn certificate(self) -> Option<ExecutionStrategyCertificateV2> {
        self.certificate
    }

    /// Return the exact optional Registry Admission identity.
    pub const fn admission_program_id(self) -> Option<ContentId> {
        self.admission_program_id
    }

    /// Return the exact optional finalized ArtifactRelease identity.
    pub const fn artifact_release_id(self) -> Option<ArtifactReleaseIdV1> {
        self.artifact_release_id
    }

    /// Return the exact optional immutable ArtifactRelease.
    pub const fn artifact_release(self) -> Option<ArtifactReleaseV1> {
        self.artifact_release
    }

    /// Return the private pure-contract witness only for admitted AOT.
    pub const fn admitted_authorization(self) -> Option<AdmittedAotAuthorizationV2> {
        self.admitted_authorization
    }
}

/// Authenticate one selected Execution Strategy and all disposition-owned records.
///
/// `context` must be the current Trading root/release witness produced by the
/// common fixed-role boundary. `registry_program` must be the Registry account
/// already joined to the authenticated Core Market. The adapter nevertheless
/// rechecks its executable/read-only shape and uses it as the sole owner/PDA
/// authority for every supplied finalized record. `accounts` has one of the
/// three exact disposition-derived layouts documented by the count constants.
#[inline(never)]
pub fn authenticate_execution_strategy_v2(
    context: TradingFamilyContextV1,
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    let rent = authenticate_common_frame(registry_program, rent_sysvar, accounts)?;
    if accounts.len() < INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2 {
        return Err(TradingSbfError::Content);
    }
    let capability_program_id = context.selection().capability_release();
    let capability_program = authenticate_capability_program(
        registry_program.key,
        &rent,
        account(accounts, CAPABILITY_RAW)?,
        account(accounts, CAPABILITY_STAGING)?,
        capability_program_id,
    )?;
    capability_program
        .validate_persisted_selection(context.selection())
        .map_err(|_| TradingSbfError::Content)?;
    if capability_program
        .root_account_bytes()
        .map_err(|_| TradingSbfError::Content)?
        != context.root_account_bytes()
    {
        return Err(TradingSbfError::Root);
    }

    let strategy_program_id = capability_program.transition_program();
    let strategy = authenticate_strategy_program(
        registry_program.key,
        &rent,
        account(accounts, STRATEGY_RAW)?,
        account(accounts, STRATEGY_STAGING)?,
        strategy_program_id,
    )?;
    strategy
        .validate_descriptor_selection(strategy_program_id, capability_program)
        .map_err(|_| TradingSbfError::Content)?;

    match strategy.disposition() {
        StrategyDispositionV2::Interpreted => {
            require_exact_account_count(accounts, INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2)?;
            Ok(AuthenticatedExecutionStrategyV2 {
                capability_program_id,
                capability_program,
                strategy_program_id,
                strategy,
                certificate_program_id: None,
                certificate: None,
                admission_program_id: None,
                artifact_release_id: None,
                artifact_release: None,
                admitted_authorization: None,
            })
        }
        StrategyDispositionV2::ShadowAot => authenticate_shadow_aot(
            registry_program.key,
            &rent,
            accounts,
            capability_program_id,
            capability_program,
            strategy_program_id,
            strategy,
        ),
        StrategyDispositionV2::AdmittedAot => authenticate_admitted_aot(
            registry_program.key,
            &rent,
            accounts,
            capability_program_id,
            capability_program,
            strategy_program_id,
            strategy,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_shadow_aot(
    registry_program: &Pubkey,
    rent: &Rent,
    accounts: &[AccountInfo<'_>],
    capability_program_id: ContentId,
    capability_program: CapabilityProgramV3,
    strategy_program_id: ContentId,
    strategy: ExecutionStrategyProgramV2,
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    require_exact_account_count(accounts, SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2)?;
    let certificate_program_id = strategy
        .certificate_program()
        .ok_or(TradingSbfError::Content)?;
    let certificate = authenticate_certificate(
        registry_program,
        rent,
        account(accounts, CERTIFICATE_RAW)?,
        account(accounts, CERTIFICATE_STAGING)?,
        certificate_program_id,
    )?;
    certificate
        .validate_v3(
            certificate_program_id,
            strategy_program_id,
            strategy,
            capability_program,
            authenticated_interpreter_artifacts(capability_program, strategy),
        )
        .map_err(|_| TradingSbfError::Content)?;
    let artifact_release_id = certificate.artifact_release();
    let artifact_release = authenticate_immutable_artifact(
        registry_program,
        rent,
        account(accounts, SHADOW_ARTIFACT_RAW)?,
        account(accounts, SHADOW_ARTIFACT_STAGING)?,
        artifact_release_id,
        account(accounts, SHADOW_ACCELERATOR_PROGRAM)?,
        account(accounts, SHADOW_ACCELERATOR_PROGRAMDATA)?,
    )?;
    certificate
        .validate_artifact(artifact_release_id)
        .map_err(|_| TradingSbfError::Content)?;
    Ok(AuthenticatedExecutionStrategyV2 {
        capability_program_id,
        capability_program,
        strategy_program_id,
        strategy,
        certificate_program_id: Some(certificate_program_id),
        certificate: Some(certificate),
        admission_program_id: None,
        artifact_release_id: Some(artifact_release_id),
        artifact_release: Some(artifact_release),
        admitted_authorization: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_admitted_aot(
    registry_program: &Pubkey,
    rent: &Rent,
    accounts: &[AccountInfo<'_>],
    capability_program_id: ContentId,
    capability_program: CapabilityProgramV3,
    strategy_program_id: ContentId,
    strategy: ExecutionStrategyProgramV2,
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    require_exact_account_count(accounts, ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2)?;
    let certificate_program_id = strategy
        .certificate_program()
        .ok_or(TradingSbfError::Content)?;
    let certificate = authenticate_certificate(
        registry_program,
        rent,
        account(accounts, CERTIFICATE_RAW)?,
        account(accounts, CERTIFICATE_STAGING)?,
        certificate_program_id,
    )?;
    let admission_program_id = strategy
        .admission_program()
        .ok_or(TradingSbfError::Content)?;
    let admission = authenticate_admission(
        registry_program,
        rent,
        account(accounts, ADMITTED_ADMISSION_RAW)?,
        account(accounts, ADMITTED_ADMISSION_STAGING)?,
        admission_program_id,
    )?;
    let artifact_release_id = certificate.artifact_release();
    let artifact_release = authenticate_immutable_artifact(
        registry_program,
        rent,
        account(accounts, ADMITTED_ARTIFACT_RAW)?,
        account(accounts, ADMITTED_ARTIFACT_STAGING)?,
        artifact_release_id,
        account(accounts, ADMITTED_ACCELERATOR_PROGRAM)?,
        account(accounts, ADMITTED_ACCELERATOR_PROGRAMDATA)?,
    )?;
    let admitted_authorization = validate_admitted_aot_v2(
        strategy_program_id,
        strategy,
        capability_program,
        certificate_program_id,
        certificate,
        authenticated_interpreter_artifacts(capability_program, strategy),
        artifact_release_id,
        Some((admission_program_id, admission)),
    )
    .map_err(|_| TradingSbfError::Content)?;
    Ok(AuthenticatedExecutionStrategyV2 {
        capability_program_id,
        capability_program,
        strategy_program_id,
        strategy,
        certificate_program_id: Some(certificate_program_id),
        certificate: Some(certificate),
        admission_program_id: Some(admission_program_id),
        artifact_release_id: Some(artifact_release_id),
        artifact_release: Some(artifact_release),
        admitted_authorization: Some(admitted_authorization),
    })
}

fn authenticated_interpreter_artifacts(
    capability_program: CapabilityProgramV3,
    strategy: ExecutionStrategyProgramV2,
) -> AuthenticatedInterpreterArtifactsV2 {
    AuthenticatedInterpreterArtifactsV2 {
        account_profile_program: capability_program.account_profile(),
        request_profile_schema: capability_program.request_profile_schema(),
        request_profile_program: capability_program.request_profile_program(),
        transition_schema: strategy.transition_schema(),
        transition_program: strategy.transition_program(),
        effect_program: capability_program.effect_program(),
    }
}

fn authenticate_capability_program(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<CapabilityProgramV3, TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        CAPABILITY_PROGRAM_SCHEMA_ID_V3,
        expected.to_bytes(),
        &data,
    )?;
    CapabilityProgramV3::decode(&data).map_err(|_| TradingSbfError::Content)
}

fn authenticate_strategy_program(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<ExecutionStrategyProgramV2, TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_PROGRAM_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    ExecutionStrategyProgramV2::decode(&data).map_err(|_| TradingSbfError::Content)
}

fn authenticate_certificate(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<ExecutionStrategyCertificateV2, TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    ExecutionStrategyCertificateV2::decode(&data).map_err(|_| TradingSbfError::Content)
}

fn authenticate_admission(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<ExecutionStrategyAdmissionV2, TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_ADMISSION_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    ExecutionStrategyAdmissionV2::decode(&data).map_err(|_| TradingSbfError::Content)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_immutable_artifact(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ArtifactReleaseIdV1,
    accelerator_program: &AccountInfo<'_>,
    accelerator_programdata: &AccountInfo<'_>,
) -> Result<ArtifactReleaseV1, TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(TradingSbfError::Content);
    }
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        expected.to_bytes(),
        &data,
    )?;
    let release = ArtifactReleaseV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || release.upgrade_authority().is_some()
    {
        return Err(TradingSbfError::Content);
    }
    drop(data);
    authenticate_current_deployment(release, accelerator_program, accelerator_programdata)?;
    Ok(release)
}

fn authenticate_current_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), TradingSbfError> {
    if program.is_signer
        || program.is_writable
        || !program.executable
        || programdata.is_signer
        || programdata.is_writable
        || programdata.executable
        || program.key == programdata.key
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != &bpf_loader_upgradeable::ID
        || programdata.owner != &bpf_loader_upgradeable::ID
    {
        return Err(TradingSbfError::Content);
    }
    let program_bytes = program
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let program_view =
        ProgramV3View::parse(&program_bytes).map_err(|_| TradingSbfError::Content)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata()
        || programdata.key != &expected_programdata
    {
        return Err(TradingSbfError::Content);
    }
    drop(program_bytes);
    let programdata_bytes = programdata
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_bytes).map_err(|_| TradingSbfError::Content)?;
    if programdata_view.upgrade_authority().is_some() {
        return Err(TradingSbfError::Content);
    }
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| TradingSbfError::Content)
}

fn authenticate_common_frame(
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<Rent, TradingSbfError> {
    if registry_program.is_signer
        || registry_program.is_writable
        || !registry_program.executable
        || rent_sysvar.key != &sysvar::rent::ID
        || rent_sysvar.owner != &sysvar::ID
        || rent_sysvar.is_signer
        || rent_sysvar.is_writable
        || rent_sysvar.executable
    {
        return Err(TradingSbfError::Content);
    }
    for (index, current) in accounts.iter().enumerate() {
        if current.key == registry_program.key
            || current.key == rent_sysvar.key
            || accounts
                .get(index.saturating_add(1)..)
                .ok_or(TradingSbfError::Content)?
                .iter()
                .any(|other| current.key == other.key)
        {
            return Err(TradingSbfError::Content);
        }
    }
    Rent::from_account_info(rent_sysvar).map_err(|_| TradingSbfError::Content)
}

fn authenticate_finalized_record(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    exact_content: &[u8],
) -> Result<(), TradingSbfError> {
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        registry_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        registry_program,
    )
    .0;
    if raw.key != &expected_raw
        || raw.owner != registry_program
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(exact_content).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), exact_content.len())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.is_signer
        || staging.is_writable
        || staging.executable
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content);
    }
    Ok(())
}

fn require_exact_account_count(
    accounts: &[AccountInfo<'_>],
    expected: usize,
) -> Result<(), TradingSbfError> {
    if accounts.len() == expected {
        Ok(())
    } else {
        Err(TradingSbfError::Content)
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, TradingSbfError> {
    accounts.get(index).ok_or(TradingSbfError::Content)
}

#[cfg(test)]
mod tests;
