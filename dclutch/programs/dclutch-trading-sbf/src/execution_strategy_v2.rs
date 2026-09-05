//! Physical authentication of family-neutral Execution Strategy V2 records.
//!
//! The selected Capability Program digest comes from the authenticated Trading
//! root. Every semantic child is then reauthenticated as an exact finalized
//! Registry record. AOT dispositions additionally authenticate the exact
//! Certificate, optional Admission, immutable ArtifactRelease, and its current
//! Upgradeable Loader V3 Program/ProgramData/complete-ELF observation. This
//! module is read-only: it grants no accelerator state or effect write authority.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::v4::{
    CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
    SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_market::execution_strategy::v2::{
    AdmittedAotAuthorizationV2, AuthenticatedInterpreterArtifactsV2, CertificateArtifactBindingV2,
    EXECUTION_STRATEGY_ADMISSION_BYTES_V2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
    StrategyDispositionV2, validate_admitted_aot_v4,
};
use dclutch_registry::activation_auth_v1::ActivationAuthErrorV1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::ArtifactReleaseIdV1;
use dclutch_registry::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    require_slot_pinned_release_v1,
};
use dclutch_trading::shadow_accelerator_auth::{ShadowAcceleratorAuthErrorV4, deployment};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    TradingSbfError, dispatch::TradingFamilyContextV1, hot_v3::AuthenticatedAcceleratorCallerV4,
};

/// The extracted callback boundary raises Trading's own refusal codes.
///
/// `dclutch-trading::shadow_accelerator_auth` is Trading's published boundary, so its
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
            ShadowAcceleratorAuthErrorV4::DeploymentSlotMismatch => Self::DeploymentSlotMismatch,
        }
    }
}

/// The CPI-free activation-cache read raises Trading's own refusal codes.
///
/// Decision 0017's option B replaced Trading's top-level
/// `RegistryInstructionV1::Reauthenticate` invocations with the local read every
/// child role already performs. The invocation arm mapped EVERY failure --
/// a malformed frame, a foreign cache, a moved deployment, an upgraded
/// substrate -- onto `Release`, because a failed `invoke` carries no band:
/// `.map_err(|_| TradingSbfError::Release)`. Three of this enum's four variants
/// keep that code, so the conversion is refusal-invisible where it was.
///
/// The fourth does not, and the difference is deliberate.
/// [`ActivationAuthErrorV1::ReleaseSuperseded`] is decision 0012's
/// operator-actionable refusal -- the substrate's upgrade authority shipped new
/// bytes and every open market on the previous generation must wait for a
/// re-release. Under the CPI that fact reached the caller as a generic `Release`
/// and the operator had to read Registry logs to learn it. Reading the cache
/// directly, Trading knows it, so it says it: `0x4007`, the same band the
/// continuation arm and `docs/reference/refusals.md` already carry for it.
impl From<ActivationAuthErrorV1> for TradingSbfError {
    fn from(value: ActivationAuthErrorV1) -> Self {
        match value {
            ActivationAuthErrorV1::AccountFrame
            | ActivationAuthErrorV1::ActivationCache
            | ActivationAuthErrorV1::Deployment => Self::Release,
            ActivationAuthErrorV1::ReleaseSuperseded => Self::ReleaseSuperseded,
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
    record_bumps: StrategyRecordBumpsV2,
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

/// The canonical PDA bumps of one Registry record pair, as the derivation that
/// established them found them.
///
/// Zero on both halves is UNRECORDED and means "search", which is what a caller
/// that never derived them holds. A bump can never be zero for a real record:
/// `find_program_address` starts at 255 and refuses the off-curve search past
/// 0, so the value is free as a sentinel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RecordPairBumpsV2 {
    raw: u8,
    staging: u8,
}

impl RecordPairBumpsV2 {
    /// The two bumps one `find_program_address` pair produced.
    pub const fn new(raw: u8, staging: u8) -> Self {
        Self { raw, staging }
    }

    /// The raw record's canonical bump, or `None` when nothing derived it.
    pub const fn raw(self) -> Option<u8> {
        if self.raw == 0 { None } else { Some(self.raw) }
    }

    /// The staging cursor's canonical bump, or `None` when nothing derived it.
    pub const fn staging(self) -> Option<u8> {
        if self.staging == 0 {
            None
        } else {
            Some(self.staging)
        }
    }
}

/// The canonical PDA bumps this module's own walk derived, carried forward so
/// that a second walk over the SAME five record pairs in the SAME instruction
/// reproduces each address with one `create_program_address` instead of
/// searching for it again.
///
/// # The measurement this exists for
///
/// `admitted_composition_v3::validate_authenticated_frame` re-derives every one
/// of these pairs a few thousand instructions after this module derived them,
/// to hold the frame it hands the accelerator to the same Registry addresses.
/// Both walks ran `find_program_address`. Measured 2026-09-03 on real SBF ELFs
/// by doubling each walk's searches: **this walk's cost 37,640 CU and the
/// second walk's 29,235**, over one set of ten addresses whose seeds are a PDA
/// domain, a canonical schema id and a content digest -- none of which moves
/// with the release-set id, which is why both spans read draw-free while being
/// almost entirely search.
///
/// Nothing here is taken on anyone's word. A bump is fed to a derivation the
/// reader builds for itself and the address is compared against the account the
/// frame supplied, by the equality that was always there; a wrong bump names a
/// different address, or none, and refuses. Canonicality is enforced where the
/// record is MADE -- the Registry finalizes records only at the canonical bump
/// -- so a non-canonical hint names an address at which no Registry-owned
/// record exists. This is `hot_v3::borrow_finalized_record_at`'s argument, one
/// module over.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StrategyRecordBumpsV2 {
    /// The selected capability descriptor's record pair.
    pub capability: RecordPairBumpsV2,
    /// The execution strategy record's pair.
    pub strategy: RecordPairBumpsV2,
    /// The strategy certificate's pair.
    pub certificate: RecordPairBumpsV2,
    /// The admission record's pair, on the admitted disposition only.
    pub admission: RecordPairBumpsV2,
    /// The pinned artifact release's pair.
    pub artifact: RecordPairBumpsV2,
}

#[derive(Clone, Copy)]
enum CurrentDeploymentAuthenticationV2 {
    /// Authenticate the accelerator's deployment by its slot pin.
    ///
    /// This was `CompleteElf`, and it hashed the complete observed ELF on every
    /// admitted-AOT and shadow-AOT action. Measured on real ELFs 2026-09-02:
    /// **370,983 CU of a 1,399,700 budget** to hash the 744,840-byte Dealer
    /// accelerator, inside a 419,775-CU strategy authentication that was 30% of
    /// the whole transaction.
    ///
    /// It hashed because of a precondition that no longer holds. The doc it
    /// replaced said "a finalized `ArtifactRelease` record proves only its own
    /// content identity; nothing has bound its `elf_digest` to the account
    /// being observed" -- true while release finalization observed nothing.
    /// Registry finalization now performs the deployment observation itself and
    /// refuses a release whose program is not deployed or whose ELF differs, so
    /// the bound digest is a chain-observed fact about this exact address
    /// before any hot route reads it. Decision 0012's argument then applies
    /// unchanged, and it is the argument, not this variant, that owns the
    /// soundness: `slot_pinned_release_elf_digest_v1` proves observed-slot
    /// equality means the admitted digest is the exact current digest.
    ///
    /// A hot route therefore never hashes an ELF. An accelerator upgraded in
    /// place moves its slot and refuses `ReleaseSuperseded` until it is
    /// re-released, which is exactly the pin's own guarantee and is asserted by
    /// name in `slot_pin_supersession.rs`.
    SlotPinnedRelease,
    /// The same pin, plus the caller attestation Trading already authenticated.
    AttestedAccelerator(AuthenticatedAcceleratorCallerV4),
}

impl AuthenticatedExecutionStrategyV2 {
    /// The canonical record-pair bumps this authentication itself derived.
    ///
    /// A second walk over the same five pairs in the same instruction
    /// reproduces each address from these instead of searching. See
    /// [`StrategyRecordBumpsV2`].
    pub const fn record_bumps(self) -> StrategyRecordBumpsV2 {
        self.record_bumps
    }

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
    authenticate_untrusted_frame_coordinates_v2(registry_program, rent_sysvar)?;
    if accounts.len() < INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2 {
        return Err(TradingSbfError::Content);
    }
    let capability_program_id = selected_capability_program_id;
    let (capability_program, capability_bumps) = authenticate_capability_program(
        registry_program.key,
        account(accounts, CAPABILITY_RAW)?,
        account(accounts, CAPABILITY_STAGING)?,
        selected_capability_program_schema,
        capability_program_id,
    )?;
    authenticate_selected_execution_strategy_v2(
        context,
        capability_program_id,
        &capability_program,
        capability_bumps,
        registry_program.key,
        accounts,
        CurrentDeploymentAuthenticationV2::SlotPinnedRelease,
    )
}

/// Authenticate one strategy after Hot has spent a CapabilitySeal token for
/// the selected descriptor.
///
/// The first two account slots retain the exact descriptor pair authenticated
/// by Hot. Direct execution may carry its already-authorized raw/raw alias;
/// other families carry the canonical finalized raw plus vacant staging PDA.
/// This boundary independently rechecks the pair, body digest, width and rent,
/// then requires the decoded body to equal the supplied descriptor. Every
/// strategy-owned record and deployment remains fully distinct below.
#[inline(never)]
pub(crate) fn authenticate_execution_strategy_from_sealed_capability_v2(
    context: TradingFamilyContextV1,
    capability_program_id: ContentId,
    capability_program: &CapabilityProgramV4,
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    let capability_bumps = authenticate_common_frame_with_sealed_capability_pair(
        registry_program,
        rent_sysvar,
        accounts,
        capability_program_id,
        capability_program,
    )?;
    authenticate_selected_execution_strategy_v2(
        context,
        capability_program_id,
        capability_program,
        capability_bumps,
        registry_program.key,
        accounts,
        CurrentDeploymentAuthenticationV2::SlotPinnedRelease,
    )
}

#[inline(never)]
fn authenticate_selected_execution_strategy_v2(
    context: TradingFamilyContextV1,
    capability_program_id: ContentId,
    capability_program: &CapabilityProgramV4,
    capability_bumps: RecordPairBumpsV2,
    registry_program: &Pubkey,
    accounts: &[AccountInfo<'_>],
    deployment_authentication: CurrentDeploymentAuthenticationV2,
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
    if let CurrentDeploymentAuthenticationV2::AttestedAccelerator(caller) =
        deployment_authentication
        && !caller.binds_context(context)
    {
        return Err(TradingSbfError::Release);
    }

    let strategy_program_id = capability_program.strategy().program();
    let (strategy, strategy_bumps) = authenticate_strategy_program(
        registry_program,
        account(accounts, STRATEGY_RAW)?,
        account(accounts, STRATEGY_STAGING)?,
        strategy_program_id,
    )?;
    let record_bumps = StrategyRecordBumpsV2 {
        capability: capability_bumps,
        strategy: strategy_bumps,
        ..StrategyRecordBumpsV2::default()
    };
    strategy
        .validate_descriptor_selection_v4(strategy_program_id, *capability_program)
        .map_err(|_| TradingSbfError::Content)?;

    match strategy.disposition() {
        StrategyDispositionV2::Interpreted => {
            if matches!(
                deployment_authentication,
                CurrentDeploymentAuthenticationV2::AttestedAccelerator(_)
            ) {
                return Err(TradingSbfError::Content);
            }
            require_exact_account_count(accounts, INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2)?;
            Ok(AuthenticatedExecutionStrategyV2 {
                record_bumps,
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
        StrategyDispositionV2::ShadowAot => {
            if matches!(
                deployment_authentication,
                CurrentDeploymentAuthenticationV2::AttestedAccelerator(_)
            ) {
                return Err(TradingSbfError::Content);
            }
            authenticate_shadow_aot(
                registry_program,
                accounts,
                capability_program_id,
                *capability_program,
                strategy_program_id,
                strategy,
                record_bumps,
            )
        }
        StrategyDispositionV2::AdmittedAot => authenticate_admitted_aot(
            registry_program,
            accounts,
            capability_program_id,
            *capability_program,
            strategy_program_id,
            strategy,
            record_bumps,
            deployment_authentication,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_shadow_aot(
    registry_program: &Pubkey,
    accounts: &[AccountInfo<'_>],
    capability_program_id: ContentId,
    capability_program: CapabilityProgramV4,
    strategy_program_id: ContentId,
    strategy: ExecutionStrategyProgramV2,
    mut record_bumps: StrategyRecordBumpsV2,
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    require_exact_account_count(accounts, SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2)?;
    let certificate_program_id = strategy
        .certificate_program()
        .ok_or(TradingSbfError::Content)?;
    let (certificate, certificate_bumps) = authenticate_certificate(
        registry_program,
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
    record_bumps.certificate = certificate_bumps;
    let (artifact_release_id, artifact_release, artifact_bumps) = authenticate_pinned_artifact(
        registry_program,
        account(accounts, SHADOW_ARTIFACT_RAW)?,
        account(accounts, SHADOW_ARTIFACT_STAGING)?,
        binding,
        account(accounts, SHADOW_ACCELERATOR_PROGRAM)?,
        account(accounts, SHADOW_ACCELERATOR_PROGRAMDATA)?,
        CurrentDeploymentAuthenticationV2::SlotPinnedRelease,
    )?;
    if let CertificateArtifactBindingV2::Release(_) = binding {
        certificate
            .validate_artifact(artifact_release_id)
            .map_err(|_| TradingSbfError::Content)?;
    }
    record_bumps.artifact = artifact_bumps;
    Ok(AuthenticatedExecutionStrategyV2 {
        record_bumps,
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
    accounts: &[AccountInfo<'_>],
    capability_program_id: ContentId,
    capability_program: CapabilityProgramV4,
    strategy_program_id: ContentId,
    strategy: ExecutionStrategyProgramV2,
    mut record_bumps: StrategyRecordBumpsV2,
    deployment_authentication: CurrentDeploymentAuthenticationV2,
) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
    require_exact_account_count(accounts, ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2)?;
    let certificate_program_id = strategy
        .certificate_program()
        .ok_or(TradingSbfError::Content)?;
    let (certificate, certificate_bumps) = authenticate_certificate(
        registry_program,
        account(accounts, CERTIFICATE_RAW)?,
        account(accounts, CERTIFICATE_STAGING)?,
        certificate_program_id,
    )?;
    let admission_program_id = strategy
        .admission_program()
        .ok_or(TradingSbfError::Content)?;
    let (admission, admission_bumps) = authenticate_admission(
        registry_program,
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
    record_bumps.certificate = certificate_bumps;
    record_bumps.admission = admission_bumps;
    let (_, artifact_release, artifact_bumps) = authenticate_pinned_artifact(
        registry_program,
        account(accounts, ADMITTED_ARTIFACT_RAW)?,
        account(accounts, ADMITTED_ARTIFACT_STAGING)?,
        CertificateArtifactBindingV2::Release(artifact_release_id),
        account(accounts, ADMITTED_ACCELERATOR_PROGRAM)?,
        account(accounts, ADMITTED_ACCELERATOR_PROGRAMDATA)?,
        deployment_authentication,
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
    record_bumps.artifact = artifact_bumps;
    Ok(AuthenticatedExecutionStrategyV2 {
        record_bumps,
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
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected_schema: ContentId,
    expected: ContentId,
) -> Result<(CapabilityProgramV4, RecordPairBumpsV2), TradingSbfError> {
    if expected_schema.to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4 {
        return Err(TradingSbfError::UnsupportedContent);
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let bumps = authenticate_finalized_record(
        registry_program,
        raw,
        staging,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        expected.to_bytes(),
        &data,
    )?;
    let decoded = CapabilityProgramV4::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((decoded, bumps))
}

fn authenticate_strategy_program(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<(ExecutionStrategyProgramV2, RecordPairBumpsV2), TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_PROGRAM_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    let bumps = authenticate_finalized_record(
        registry_program,
        raw,
        staging,
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    let decoded =
        ExecutionStrategyProgramV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((decoded, bumps))
}

fn authenticate_certificate(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<(ExecutionStrategyCertificateV2, RecordPairBumpsV2), TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    let bumps = authenticate_finalized_record(
        registry_program,
        raw,
        staging,
        EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    let decoded =
        ExecutionStrategyCertificateV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((decoded, bumps))
}

fn authenticate_admission(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    expected: ContentId,
) -> Result<(ExecutionStrategyAdmissionV2, RecordPairBumpsV2), TradingSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != EXECUTION_STRATEGY_ADMISSION_BYTES_V2 {
        return Err(TradingSbfError::Content);
    }
    let bumps = authenticate_finalized_record(
        registry_program,
        raw,
        staging,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
        expected.to_bytes(),
        &data,
    )?;
    let decoded =
        ExecutionStrategyAdmissionV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    Ok((decoded, bumps))
}

/// Authenticate the accelerator's ArtifactRelease under whichever binding the
/// Certificate declares, and join the two.
///
/// Both bindings end at the same two facts, and neither skips one:
///
/// * the record is a Registry-finalized `ArtifactReleaseV1` -- its content
///   digest derives the raw and staging PDAs, so the address proves the bytes;
/// * that record's `elf_digest` equals the live ProgramData ELF. Normal callers
///   prove this by hashing the complete ELF here. The one private admitted
///   accelerator callback may instead spend Trading's exact post-full-auth CPI
///   caller token, but only after rejoining the exact immutable release,
///   Program, ProgramData, deployment slot, and absent upgrade authority.
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
/// build of one exact source is the deliberate price, and the complete normal
/// deployment proof above is what keeps it from widening any further than that.
#[allow(clippy::too_many_arguments)]
fn authenticate_pinned_artifact(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    binding: CertificateArtifactBindingV2,
    accelerator_program: &AccountInfo<'_>,
    accelerator_programdata: &AccountInfo<'_>,
    deployment_authentication: CurrentDeploymentAuthenticationV2,
) -> Result<(ArtifactReleaseIdV1, ArtifactReleaseV1, RecordPairBumpsV2), TradingSbfError> {
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
    let bumps = authenticate_finalized_record(
        registry_program,
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
    match deployment_authentication {
        CurrentDeploymentAuthenticationV2::SlotPinnedRelease => {
            authenticate_slot_pinned_deployment(
                release,
                accelerator_program,
                accelerator_programdata,
            )?;
        }
        CurrentDeploymentAuthenticationV2::AttestedAccelerator(caller) => {
            if !caller.binds_immutable_deployment(
                artifact_release_id,
                release,
                accelerator_program,
                accelerator_programdata,
            ) {
                return Err(TradingSbfError::Release);
            }
            authenticate_activated_current_deployment(
                release,
                accelerator_program,
                accelerator_programdata,
            )?;
        }
    }
    Ok((artifact_release_id, release, bumps))
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

/// Reauthenticate one accelerator's current deployment by its slot pin.
///
/// The same function `authenticate_activated_current_deployment` calls, under
/// the name that says what the release must be rather than where it came from.
/// An accelerator's `ArtifactRelease` is certificate-pinned, not activated, and
/// the two now share one precondition: the Registry observed the deployment
/// once, against the live ProgramData, before the record was finalized.
///
/// Trading no longer hashes an ELF on any route. `deployment::authenticate_current_deployment`
/// -- the hashing form -- survives for its one honest caller, the Registry's
/// `ArtifactRelease` finalize route, where it is paid once per release instead
/// of once per action.
pub(crate) fn authenticate_slot_pinned_deployment(
    release: ArtifactReleaseV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<(), TradingSbfError> {
    deployment::authenticate_activated_current_deployment(release, program, programdata)
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
/// an already-authenticated fact. `dclutch_registry::immutable_registry`
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

/// The frame's Registry and Rent COORDINATES, which is a different question
/// from any rate.
///
/// `a4b2cbb17` repriced this boundary's floors through
/// `funded_rent_persists_v1`, and the interpreted entry point's `Rent`
/// parameter died with the decode -- taking with it the only thing that
/// checked the caller handed over the real sysvar and an unwritable Registry
/// rather than accounts of its own choosing. `registry_rent_privileges_and_
/// account_width_are_not_caller_trust` had been red ever since. The sysvar
/// stays in the frame and stays authenticated; what left is the deserialize.
/// This is that authentication at ONE author, so neither entry point can lose
/// it again while the other keeps it.
fn authenticate_untrusted_frame_coordinates_v2(
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Result<(), TradingSbfError> {
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
    Ok(())
}

#[inline(never)]
fn authenticate_common_frame_with_sealed_capability_pair(
    registry_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    accounts: &[AccountInfo<'_>],
    capability_program_id: ContentId,
    capability_program: &CapabilityProgramV4,
) -> Result<RecordPairBumpsV2, TradingSbfError> {
    authenticate_untrusted_frame_coordinates_v2(registry_program, rent_sysvar)?;
    if accounts.len() < INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2 {
        return Err(TradingSbfError::Content);
    }
    let raw = account(accounts, CAPABILITY_RAW)?;
    let staging = account(accounts, CAPABILITY_STAGING)?;
    let (expected_raw, raw_bump) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            &capability_program_id.to_bytes(),
        ],
        registry_program.key,
    );
    if raw.key != &expected_raw
        || raw.owner != registry_program.key
        || raw.is_signer
        || raw.is_writable
        || raw.executable
    {
        return Err(TradingSbfError::Content);
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if data.len() != CAPABILITY_PROGRAM_V4_BYTES
        || !funded_rent_persists_v1(raw.lamports())
        || hash(&data).to_bytes() != capability_program_id.to_bytes()
        || CapabilityProgramV4::decode(&data).map_err(|_| TradingSbfError::Content)?
            != *capability_program
    {
        return Err(TradingSbfError::Content);
    }
    drop(data);
    let capability_is_aliased = raw.key == staging.key;
    // Under the aliased shape the staging coordinate REPEATS the raw record, so
    // the raw bump is the one a second walk needs for both halves, and no
    // staging address is derived here at all.
    let mut bumps = RecordPairBumpsV2::new(raw_bump, raw_bump);
    if capability_is_aliased {
        if raw.owner != staging.owner
            || raw.is_signer != staging.is_signer
            || raw.is_writable != staging.is_writable
            || raw.executable != staging.executable
        {
            return Err(TradingSbfError::Content);
        }
    } else {
        let (expected_staging, staging_bump) = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                &capability_program_id.to_bytes(),
            ],
            registry_program.key,
        );
        bumps = RecordPairBumpsV2::new(raw_bump, staging_bump);
        if staging.key != &expected_staging
            || staging.owner != &system_program::ID
            || staging.is_signer
            || staging.is_writable
            || staging.executable
            || staging.data_len() != 0
        {
            return Err(TradingSbfError::Content);
        }
    }
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
                        && !(capability_is_aliased
                            && index == CAPABILITY_RAW
                            && right == CAPABILITY_STAGING)
                })
        {
            return Err(TradingSbfError::Content);
        }
    }
    Ok(bumps)
}

fn authenticate_finalized_record(
    registry_program: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    exact_content: &[u8],
) -> Result<RecordPairBumpsV2, TradingSbfError> {
    let (expected_raw, raw_bump) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        registry_program,
    );
    let (expected_staging, staging_bump) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        registry_program,
    );
    if raw.key != &expected_raw
        || raw.owner != registry_program
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(exact_content).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.is_signer
        || staging.is_writable
        || staging.executable
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content);
    }
    Ok(RecordPairBumpsV2::new(raw_bump, staging_bump))
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
