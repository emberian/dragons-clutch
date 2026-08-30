//! Physical authentication of family-neutral Execution Strategy V2 records.
//!
//! The selected Capability Program digest comes from the authenticated Trading
//! root. Every semantic child is then reauthenticated as an exact finalized
//! Registry record. AOT dispositions additionally authenticate the exact
//! Certificate, optional Admission, immutable ArtifactRelease, and its current
//! Upgradeable Loader V3 Program/ProgramData/complete-ELF observation. This
//! module is read-only: it grants no accelerator state or effect write authority.

use dclutch_capability_program_contract::v4::{
    CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
    SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AdmittedAotAuthorizationV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_ADMISSION_BYTES_V2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    CertificateArtifactBindingV2, ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2,
    ExecutionStrategyProgramV2,
    StrategyDispositionV2, validate_admitted_aot_v4,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    require_slot_pinned_release_v1,
};
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use dclutch_shadow_accelerator_auth_v4::{ShadowAcceleratorAuthErrorV4, deployment};
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

/// The extracted callback boundary raises Trading's own refusal codes.
///
/// `dclutch-shadow-accelerator-auth-v4` is Trading's published boundary, so its
/// refusals must be indistinguishable from the ones this crate would have
/// raised. These assertions are the binding: the two definitions cannot drift
/// apart without failing the build.
const _: () = assert!(
    ShadowAcceleratorAuthErrorV4::Release as u32 == TradingSbfError::Release as u32,
    "the published Shadow callback boundary must raise Trading's Release code"
);
const _: () = assert!(
    ShadowAcceleratorAuthErrorV4::Content as u32 == TradingSbfError::Content as u32,
    "the published Shadow callback boundary must raise Trading's Content code"
);
const _: () = assert!(
    ShadowAcceleratorAuthErrorV4::ReleaseSuperseded as u32
        == TradingSbfError::ReleaseSuperseded as u32,
    "the published Shadow callback boundary must raise Trading's ReleaseSuperseded code"
);

impl From<ShadowAcceleratorAuthErrorV4> for TradingSbfError {
    fn from(value: ShadowAcceleratorAuthErrorV4) -> Self {
        match value {
            ShadowAcceleratorAuthErrorV4::Release => Self::Release,
            ShadowAcceleratorAuthErrorV4::Content => Self::Content,
            ShadowAcceleratorAuthErrorV4::ReleaseSuperseded => Self::ReleaseSuperseded,
        }
    }
}

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
    capability_program: CapabilityProgramV4,
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
    pub const fn capability_program(self) -> CapabilityProgramV4 {
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
/// common fixed-role boundary. Its manifest release selects the authenticated
/// `CapabilityProgramSetV2`, not an individual descriptor.
/// `selected_capability_program_schema` and `selected_capability_program_id`
/// must therefore be the exact action-selected pair returned by that already-
/// authenticated set. This adapter admits only CapabilityProgramV4, then
/// rejoins its kind and root width to `context`; the common outer separately
/// authenticates the context-selected config under the descriptor's config
/// schema. `registry_program` must be the
/// Registry account already joined to the authenticated Core Market. The
/// adapter nevertheless rechecks its executable/read-only shape and uses it as
/// the sole owner/PDA authority for every supplied finalized record. `accounts`
/// has one of the three exact disposition-derived layouts documented by the
/// count constants.
#[inline(never)]
pub fn authenticate_execution_strategy_v2(
    context: TradingFamilyContextV1,
    selected_capability_program_schema: ContentId,
    selected_capability_program_id: ContentId,
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    let rent = authenticate_common_frame(registry_program, rent_sysvar, accounts)?;
    if accounts.len() < INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2 {
        return Err(TradingSbfError::Content);
    }
    let capability_program_id = selected_capability_program_id;
    let capability_program = authenticate_capability_program(
        registry_program.key,
        &rent,
        account(accounts, CAPABILITY_RAW)?,
        account(accounts, CAPABILITY_STAGING)?,
        selected_capability_program_schema,
        capability_program_id,
    )?;
    authenticate_selected_execution_strategy_v2(
        context,
        capability_program_id,
        &capability_program,
        registry_program.key,
        &rent,
        accounts,
    )
}

/// Authenticate one strategy after Hot has spent a CapabilitySeal token for
/// the selected descriptor.
///
/// The first two account slots are the execution-only raw/raw alias. The Hot
/// caller has already authenticated the exact Registry coordinate, body
/// digest, width, rent and seal row before reaching this crate-private entry;
/// this boundary re-decodes the same borrowed body and requires it to equal the
/// supplied descriptor. Every strategy-owned record and deployment retains the
/// ordinary fully-distinct finalized-record authentication below.
#[inline(never)]
pub(crate) fn authenticate_execution_strategy_from_sealed_capability_v2(
    context: TradingFamilyContextV1,
    capability_program_id: ContentId,
    capability_program: &CapabilityProgramV4,
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    let rent = authenticate_common_frame_with_sealed_capability_alias(
        registry_program,
        rent_sysvar,
        accounts,
        capability_program,
    )?;
    authenticate_selected_execution_strategy_v2(
        context,
        capability_program_id,
        capability_program,
        registry_program.key,
        &rent,
        accounts,
    )
}

#[inline(never)]
fn authenticate_selected_execution_strategy_v2(
    context: TradingFamilyContextV1,
    capability_program_id: ContentId,
    capability_program: &CapabilityProgramV4,
    registry_program: &Pubkey,
    rent: &Rent,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
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

    let strategy_program_id = capability_program.strategy().program();
    let strategy = authenticate_strategy_program(
        registry_program,
        rent,
        account(accounts, STRATEGY_RAW)?,
        account(accounts, STRATEGY_STAGING)?,
        strategy_program_id,
    )?;
    strategy
        .validate_descriptor_selection_v4(strategy_program_id, *capability_program)
        .map_err(|_| TradingSbfError::Content)?;

    match strategy.disposition() {
        StrategyDispositionV2::Interpreted => {
            require_exact_account_count(accounts, INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2)?;
            Ok(AuthenticatedExecutionStrategyV2 {
                capability_program_id,
                capability_program: *capability_program,
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
            registry_program,
            rent,
            accounts,
            capability_program_id,
            *capability_program,
            strategy_program_id,
            strategy,
        ),
        StrategyDispositionV2::AdmittedAot => authenticate_admitted_aot(
            registry_program,
            rent,
            accounts,
            capability_program_id,
            *capability_program,
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
    capability_program: CapabilityProgramV4,
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
        .validate_v4(
            certificate_program_id,
            strategy_program_id,
            strategy,
            capability_program,
            authenticated_interpreter_artifacts(capability_program, strategy),
        )
        .map_err(|_| TradingSbfError::Content)?;
    // Shadow AOT accepts either binding. The exact-release one is re-checked
    // against the Certificate below; the semantic one is joined inside
    // authenticate_pinned_artifact, against the record it selected.
    let binding = certificate.artifact_binding();
    let (artifact_release_id, artifact_release) = authenticate_pinned_artifact(
        registry_program,
        rent,
        account(accounts, SHADOW_ARTIFACT_RAW)?,
        account(accounts, SHADOW_ARTIFACT_STAGING)?,
        binding,
        account(accounts, SHADOW_ACCELERATOR_PROGRAM)?,
        account(accounts, SHADOW_ACCELERATOR_PROGRAMDATA)?,
    )?;
    if let CertificateArtifactBindingV2::Release(_) = binding {
        certificate
            .validate_artifact(artifact_release_id)
            .map_err(|_| TradingSbfError::Content)?;
    }
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
    capability_program: CapabilityProgramV4,
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
    // Admitted AOT takes the exact-release binding and nothing else. Admission
    // is a statement about one built artifact, so a semantically bound
    // Certificate is refused here rather than silently admitting every build of
    // that source -- this call is the enforcement, not a comment about it.
    let artifact_release_id = certificate
        .artifact_release()
        .map_err(|_| TradingSbfError::Content)?;
    let (_, artifact_release) = authenticate_pinned_artifact(
        registry_program,
        rent,
        account(accounts, ADMITTED_ARTIFACT_RAW)?,
        account(accounts, ADMITTED_ARTIFACT_STAGING)?,
        CertificateArtifactBindingV2::Release(artifact_release_id),
        account(accounts, ADMITTED_ACCELERATOR_PROGRAM)?,
        account(accounts, ADMITTED_ACCELERATOR_PROGRAMDATA)?,
    )?;
    let admitted_authorization = validate_admitted_aot_v4(
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
    capability_program: CapabilityProgramV4,
    strategy: ExecutionStrategyProgramV2,
) -> AuthenticatedInterpreterArtifactsV2 {
    AuthenticatedInterpreterArtifactsV2 {
        account_profile_program: capability_program.account_profile().program(),
        request_profile_schema: capability_program.request_profile().schema(),
        request_profile_program: capability_program.request_profile().program(),
        transition_schema: strategy.transition_schema(),
        transition_program: strategy.transition_program(),
        effect_program: capability_program.effect().program(),
    }
}

fn authenticate_capability_program(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected_schema: ContentId,
    expected: ContentId,
) -> Result<CapabilityProgramV4, TradingSbfError> {
    if expected_schema.to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4 {
        return Err(TradingSbfError::UnsupportedContent);
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        expected.to_bytes(),
        &data,
    )?;
    CapabilityProgramV4::decode(&data).map_err(|_| TradingSbfError::Content)
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

/// Authenticate the accelerator's ArtifactRelease under whichever binding the
/// Certificate declares, and join the two.
///
/// Both bindings end at the same two facts, and neither skips one:
///
/// * the record is a Registry-finalized `ArtifactReleaseV1` -- its content
///   digest derives the raw and staging PDAs, so the address proves the bytes;
/// * that record's `elf_digest` equals a hash of the live programdata, checked
///   by `authenticate_current_deployment` on every call.
///
/// They differ only in which fact the Certificate itself supplies. A `Release`
/// binding names the record's exact content identity, so the record is selected
/// by the Certificate. A `Semantic` binding names a source-derived
/// `semantic_release_id`, so the record is selected by its own content -- which
/// the PDA derivation and the Registry's ownership already make load-bearing --
/// and the Certificate is joined to it by the semantic equality instead.
///
/// The semantic binding exists because a Certificate naming an exact
/// `ArtifactReleaseV1` cannot be authored for an accelerator whose ELF embeds
/// that Certificate: its identity would have to contain the digest of the bytes
/// it is compiled into. Measured, not argued, in `23eed7df`. Widening to every
/// build of one exact source is the deliberate price, and the deployment hash
/// above is what keeps it from widening any further than that.
#[allow(clippy::too_many_arguments)]
fn authenticate_pinned_artifact(
    registry_program: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    binding: CertificateArtifactBindingV2,
    accelerator_program: &AccountInfo<'_>,
    accelerator_programdata: &AccountInfo<'_>,
) -> Result<(ArtifactReleaseIdV1, ArtifactReleaseV1), TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != ARTIFACT_RELEASE_BYTES_V1 {
        return Err(TradingSbfError::Content);
    }
    // For a Release binding this is the Certificate's own pin, exactly as
    // before. For a Semantic binding the record identifies itself, and
    // authenticate_finalized_record's own `hash(bytes) == digest` check is then
    // trivially true -- the address derivation below it is what makes the bytes
    // load-bearing, and the semantic equality further down is the real join.
    let digest = match binding {
        CertificateArtifactBindingV2::Release(expected) => expected.to_bytes(),
        CertificateArtifactBindingV2::Semantic(_) => hash(&data).to_bytes(),
    };
    authenticate_finalized_record(
        registry_program,
        rent,
        raw,
        staging,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        digest,
        &data,
    )?;
    let release = ArtifactReleaseV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    require_slot_pinned_release_v1(release).map_err(|_| TradingSbfError::Content)?;
    if let CertificateArtifactBindingV2::Semantic(_) = binding {
        certificate_semantic_join(binding, release)?;
    }
    let artifact_release_id =
        ArtifactReleaseIdV1::decode(&digest).map_err(|_| TradingSbfError::Content)?;
    drop(data);
    authenticate_current_deployment(release, accelerator_program, accelerator_programdata)?;
    Ok((artifact_release_id, release))
}

/// Join a semantically bound Certificate to the release record it selected.
fn certificate_semantic_join(
    binding: CertificateArtifactBindingV2,
    release: ArtifactReleaseV1,
) -> Result<(), TradingSbfError> {
    match binding {
        CertificateArtifactBindingV2::Semantic(semantic)
            if semantic == release.semantic_release_id() =>
        {
            Ok(())
        }
        _ => Err(TradingSbfError::Content),
    }
}

/// Reauthenticate one current Loader V3 deployment by hashing its exact ELF.
///
/// A finalized `ArtifactRelease` record proves only its own content identity.
/// Nothing has bound its `elf_digest` to the account being observed, so this
/// path always hashes the complete observed ELF.  Use
/// `authenticate_activated_current_deployment` only where the Registry
/// activation cache already carries that binding.
///
/// The implementation lives in `dclutch-shadow-accelerator-auth-v4` because an
/// external Shadow accelerator needs exactly this check and nothing else in
/// this crate; Trading calls the same code rather than keeping a second copy.
pub(crate) fn authenticate_current_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), TradingSbfError> {
    deployment::authenticate_current_deployment(release, program, programdata)
        .map_err(TradingSbfError::from)
}

/// Reauthenticate one activated role's current deployment without re-hashing.
///
/// `release` must come from the Registry activation cache, where
/// `activate_execution_role_into_v1` already authenticated a chain-observed
/// deployment — including the complete ELF digest — before persisting it. For
/// an `Immutable` Loader V3 deployment whose release and whose observed
/// ProgramData both carry no upgrade authority, that admitted ELF can never be
/// redeployed, so hashing a megabyte-scale ELF on every hot action recomputes
/// an already-authenticated fact. `dclutch_registry_contract::immutable_registry`
/// owns that argument and the Registry role batch already relies on it.
/// Identity, ProgramData link, Loader ownership, executability, the exact
/// deployment slot, and the absent upgrade authority are still checked here and
/// again by `authenticate_deployment`; an upgradeable activated release keeps
/// the full current-ELF hash.
pub(crate) fn authenticate_activated_current_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), TradingSbfError> {
    deployment::authenticate_activated_current_deployment(release, program, programdata)
        .map_err(TradingSbfError::from)
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

#[inline(never)]
fn authenticate_common_frame_with_sealed_capability_alias(
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
    capability_program: &CapabilityProgramV4,
) -> Result<Rent, TradingSbfError> {
    if accounts.len() < INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2
        || registry_program.is_signer
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
    let raw = account(accounts, CAPABILITY_RAW)?;
    let alias = account(accounts, CAPABILITY_STAGING)?;
    if raw.key != alias.key
        || raw.owner != registry_program.key
        || raw.owner != alias.owner
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || raw.is_signer != alias.is_signer
        || raw.is_writable != alias.is_writable
        || raw.executable != alias.executable
    {
        return Err(TradingSbfError::Content);
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| TradingSbfError::Content)?;
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != CAPABILITY_PROGRAM_V4_BYTES
        || !rent.is_exempt(raw.lamports(), data.len())
        || CapabilityProgramV4::decode(&data).map_err(|_| TradingSbfError::Content)?
            != *capability_program
    {
        return Err(TradingSbfError::Content);
    }
    drop(data);
    for (index, current) in accounts.iter().enumerate() {
        if current.key == registry_program.key
            || current.key == rent_sysvar.key
            || accounts
                .get(index.saturating_add(1)..)
                .ok_or(TradingSbfError::Content)?
                .iter()
                .enumerate()
                .any(|(offset, other)| {
                    let right = index.saturating_add(offset).saturating_add(1);
                    current.key == other.key
                        && !(index == CAPABILITY_RAW && right == CAPABILITY_STAGING)
                })
        {
            return Err(TradingSbfError::Content);
        }
    }
    Ok(rent)
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
