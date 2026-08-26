//! Chain-derived inspection of the immutable Core/Registry/Rent authority chain.
//!
//! This module distinguishes two statements that must never be collapsed:
//! an observed chain may be internally consistent, while recognition requires
//! an explicit checked manifest supplied by the caller. There is no embedded
//! official-program list and this module performs no RPC, signing, or mutation.

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_release_tool::CheckedInfrastructureV1;
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{Finality, Observation, ObservedAccount, registry::RegistryFinalizedRecordState};

/// Exact finalized accounts needed to inspect one infrastructure chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolInfrastructureStateV1 {
    /// Core-owned immutable 144-byte infrastructure profile PDA.
    pub profile: ObservedAccount,
    /// Registry-owned activated execution-release-set cache selected by a Market.
    pub activation_cache: ObservedAccount,
    /// Current executable Core Program account.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData account and complete ELF tail.
    pub core_programdata: ObservedAccount,
    /// Finalized exact Registry `ArtifactReleaseV1` raw/staging pair.
    pub registry_artifact: RegistryFinalizedRecordState,
    /// Current executable Registry Program account.
    pub registry_program: ObservedAccount,
    /// Current Registry ProgramData account and complete ELF tail.
    pub registry_programdata: ObservedAccount,
    /// Finalized exact Rent `ArtifactReleaseV1` raw/staging pair.
    pub rent_artifact: RegistryFinalizedRecordState,
    /// Current executable Rent Program account.
    pub rent_program: ObservedAccount,
    /// Current Rent ProgramData account and complete ELF tail.
    pub rent_programdata: ObservedAccount,
    /// Canonical Rent sysvar used for all rent-exemption checks.
    pub rent_sysvar: ObservedAccount,
}

/// Recognition status of an otherwise internally consistent authority chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfrastructureRecognitionV1 {
    /// Loader, PDA, record, and immutable-binding checks passed, but no checked
    /// manifest was supplied by the caller.
    InternallyConsistentUnrecognized,
    /// Every observed fact joined one exact caller-supplied checked manifest.
    RecognizedBySuppliedManifest {
        /// SHA-256 identity of the exact supplied checked-infrastructure bytes.
        checked_infrastructure_id: ContentId,
    },
}

/// Exact chain-derived evidence for one immutable executable component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InfrastructureComponentEvidenceV1 {
    /// Current executable Program identity.
    pub program: Pubkey,
    /// Current ProgramData identity.
    pub programdata: Pubkey,
    /// Exact finalized or activated artifact-record identity.
    pub artifact_release_id: ArtifactReleaseIdV1,
    /// Semantic release implemented by the exact artifact.
    pub semantic_release_id: ContentId,
    /// SHA-256 of the complete current ProgramData ELF tail.
    pub elf_digest: [u8; 32],
    /// Exact current Loader V3 deployment slot.
    pub deployment_slot: u64,
}

/// Fully checked read-only projection of one infrastructure chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolInfrastructureReportV1 {
    /// One finalized observation shared by every hostile input account.
    pub observation: Observation,
    /// Market-selected activated execution release-set identity.
    pub execution_release_set_id: ContentId,
    /// Derived Core-owned infrastructure-profile PDA.
    pub profile_pda: Pubkey,
    /// SHA-256 of the exact 144-byte profile.
    pub profile_digest: ContentId,
    /// Immutable current Core deployment evidence.
    pub core: InfrastructureComponentEvidenceV1,
    /// Immutable profile-selected Registry deployment evidence.
    pub registry: InfrastructureComponentEvidenceV1,
    /// Immutable profile-selected Rent deployment evidence.
    pub rent: InfrastructureComponentEvidenceV1,
    /// Honest recognition classification, never an official-deployment claim.
    pub recognition: InfrastructureRecognitionV1,
}

impl ProtocolInfrastructureReportV1 {
    /// Emit a deterministic human-readable evidence projection.
    pub fn render_text(self) -> String {
        let mut output = String::new();
        push_line(
            &mut output,
            "format",
            "dclutch-infrastructure-observation-v1",
        );
        push_line(
            &mut output,
            "observation_slot",
            &self.observation.slot.to_string(),
        );
        push_line(&mut output, "profile_pda", &self.profile_pda.to_string());
        push_line(
            &mut output,
            "profile_sha256",
            &encode_hex(self.profile_digest.as_bytes()),
        );
        push_line(
            &mut output,
            "execution_release_set_id",
            &encode_hex(self.execution_release_set_id.as_bytes()),
        );
        render_component(&mut output, "core", self.core);
        render_component(&mut output, "registry", self.registry);
        render_component(&mut output, "rent", self.rent);
        match self.recognition {
            InfrastructureRecognitionV1::InternallyConsistentUnrecognized => {
                push_line(
                    &mut output,
                    "recognition",
                    "internally-consistent/unrecognized",
                );
            }
            InfrastructureRecognitionV1::RecognizedBySuppliedManifest {
                checked_infrastructure_id,
            } => {
                push_line(&mut output, "recognition", "supplied-manifest-match");
                push_line(
                    &mut output,
                    "checked_infrastructure_id",
                    &encode_hex(checked_infrastructure_id.as_bytes()),
                );
            }
        }
        output
    }
}

/// Stable refusal from hostile, stale, mutable, or substituted observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfrastructureInspectionErrorV1 {
    /// At least one account was not observed at finalized commitment.
    ObservationNotFinalized,
    /// Accounts did not share one exact slot/time/finality observation.
    ObservationMismatch,
    /// Named roles were improperly aliased.
    AliasedAccount,
    /// Rent sysvar bytes or identity refused.
    InvalidRentSysvar,
    /// Profile bytes, Core ownership, PDA, or rent reserve refused.
    InvalidProfile,
    /// Activation-cache bytes, ownership, PDA, or release-set projection refused.
    InvalidActivationCache,
    /// Artifact bytes, digest, Registry raw/staging PDAs, or reserve refused.
    InvalidArtifactRecord,
    /// Program/ProgramData link, Loader ownership, slot, ELF, or authority refused.
    InvalidDeployment,
    /// Core, Registry, and Rent infrastructure must all be immutable.
    InfrastructureMustBeImmutable,
    /// Selected program or artifact bindings did not join exactly.
    BindingMismatch,
    /// Supplied checked-infrastructure manifest was malformed or did not match.
    CheckedManifestMismatch,
}

/// Inspect one exact finalized chain snapshot without signing or mutation.
///
/// Passing `None` for `checked_manifest` can establish internal consistency,
/// but deliberately cannot recognize the chain. Passing bytes requires an
/// exact join to the current profile, cache, artifact records, and Loader state.
pub fn inspect_protocol_infrastructure_v1(
    state: &ProtocolInfrastructureStateV1,
    checked_manifest: Option<&[u8]>,
) -> Result<ProtocolInfrastructureReportV1, InfrastructureInspectionErrorV1> {
    let observation = same_finalized_observation(state)?;
    require_distinct_keys(state)?;
    let rent = decode_rent(&state.rent_sysvar)?;

    let profile = ProtocolInfrastructureProfileV1::decode(&state.profile.data)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidProfile)?;
    let profile_digest = ContentId::new(hash(&state.profile.data).to_bytes())
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidProfile)?;

    // Decode and hash both artifact bodies, and parse their Loader state,
    // before accepting Registry ownership as authority for those bodies.
    let (registry_release, registry) = authenticate_artifact_component(
        profile.registry(),
        &state.registry_artifact,
        state.registry_program.key,
        &state.registry_program,
        &state.registry_programdata,
        &rent,
    )?;
    let (rent_release, rent_evidence) = authenticate_artifact_component(
        profile.rent(),
        &state.rent_artifact,
        state.registry_program.key,
        &state.rent_program,
        &state.rent_programdata,
        &rent,
    )?;

    let activated =
        authenticate_activation_cache(state, profile, &rent, state.registry_program.key)?;
    let core_role = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?;
    let core_release = core_role.release();
    require_immutable(core_release)?;
    let core = authenticate_deployment(
        core_role.artifact_release_id(),
        core_release,
        &state.core_program,
        &state.core_programdata,
    )?;
    authenticate_profile_account(state, profile, core.program, &rent)?;
    if core.program == registry.program
        || core.program == rent_evidence.program
        || registry.program == rent_evidence.program
    {
        return Err(InfrastructureInspectionErrorV1::AliasedAccount);
    }

    let execution_release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?;
    let recognition = recognize_checked_manifest(
        checked_manifest,
        state,
        profile,
        &activated,
        registry_release,
        rent_release,
    )?;
    Ok(ProtocolInfrastructureReportV1 {
        observation,
        execution_release_set_id,
        profile_pda: state.profile.key,
        profile_digest,
        core,
        registry,
        rent: rent_evidence,
        recognition,
    })
}

fn authenticate_profile_account(
    state: &ProtocolInfrastructureStateV1,
    profile: ProtocolInfrastructureProfileV1,
    core_program: Pubkey,
    rent: &Rent,
) -> Result<(), InfrastructureInspectionErrorV1> {
    let expected = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &core_program,
    )
    .0;
    if state.profile.key != expected
        || state.profile.owner != core_program
        || state.profile.executable
        || state.profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || !rent.is_exempt(state.profile.lamports, state.profile.data.len())
        || profile.registry().program().to_bytes() != state.registry_program.key.to_bytes()
        || profile.rent().program().to_bytes() != state.rent_program.key.to_bytes()
    {
        return Err(InfrastructureInspectionErrorV1::InvalidProfile);
    }
    Ok(())
}

fn authenticate_activation_cache<'a>(
    state: &'a ProtocolInfrastructureStateV1,
    profile: ProtocolInfrastructureProfileV1,
    rent: &Rent,
    registry_program: Pubkey,
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, InfrastructureInspectionErrorV1> {
    if state.activation_cache.owner != registry_program
        || state.activation_cache.executable
        || state.activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !rent.is_exempt(
            state.activation_cache.lamports,
            state.activation_cache.data.len(),
        )
        || profile.registry().program().to_bytes() != registry_program.to_bytes()
    {
        return Err(InfrastructureInspectionErrorV1::InvalidActivationCache);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&state.activation_cache.data)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &registry_program,
    )
    .0;
    if state.activation_cache.key != expected {
        return Err(InfrastructureInspectionErrorV1::InvalidActivationCache);
    }
    Ok(activated)
}

fn authenticate_artifact_component(
    expected: ExecutionRoleBindingV1,
    record: &RegistryFinalizedRecordState,
    registry_program: Pubkey,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    rent: &Rent,
) -> Result<(ArtifactReleaseV1, InfrastructureComponentEvidenceV1), InfrastructureInspectionErrorV1>
{
    let release = ArtifactReleaseV1::decode(&record.record.data)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidArtifactRecord)?;
    let digest = hash(&record.record.data).to_bytes();
    let artifact_release_id = ArtifactReleaseIdV1::new(digest)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidArtifactRecord)?;
    if expected != ExecutionRoleBindingV1::new(release.program(), artifact_release_id) {
        return Err(InfrastructureInspectionErrorV1::BindingMismatch);
    }
    require_immutable(release)?;
    let deployment = authenticate_deployment(artifact_release_id, release, program, programdata)?;

    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &registry_program,
    )
    .0;
    if record.record.key != expected_raw
        || record.record.owner != registry_program
        || record.record.executable
        || !rent.is_exempt(record.record.lamports, record.record.data.len())
        || record.staging_cursor.key != expected_staging
        || record.staging_cursor.owner != system_program::ID
        || record.staging_cursor.executable
        || !record.staging_cursor.data.is_empty()
    {
        return Err(InfrastructureInspectionErrorV1::InvalidArtifactRecord);
    }
    Ok((release, deployment))
}

fn authenticate_deployment(
    artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<InfrastructureComponentEvidenceV1, InfrastructureInspectionErrorV1> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(InfrastructureInspectionErrorV1::InvalidDeployment);
    }
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidDeployment)?;
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidDeployment)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(InfrastructureInspectionErrorV1::InvalidDeployment);
    }
    let deployment = DeploymentObservationV1::new(
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
    .map_err(|_| InfrastructureInspectionErrorV1::InvalidDeployment)?;
    release
        .authenticate_deployment(deployment)
        .map_err(|_| InfrastructureInspectionErrorV1::InvalidDeployment)?;
    Ok(InfrastructureComponentEvidenceV1 {
        program: program.key,
        programdata: programdata.key,
        artifact_release_id,
        semantic_release_id: release.semantic_release_id(),
        elf_digest: release.elf_digest(),
        deployment_slot: release.deployment_slot(),
    })
}

fn require_immutable(release: ArtifactReleaseV1) -> Result<(), InfrastructureInspectionErrorV1> {
    if release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || release.upgrade_authority().is_some()
    {
        return Err(InfrastructureInspectionErrorV1::InfrastructureMustBeImmutable);
    }
    Ok(())
}

fn recognize_checked_manifest(
    checked_manifest: Option<&[u8]>,
    state: &ProtocolInfrastructureStateV1,
    profile: ProtocolInfrastructureProfileV1,
    activated: &ActivatedExecutionReleaseSetViewV1<'_>,
    registry_release: ArtifactReleaseV1,
    rent_release: ArtifactReleaseV1,
) -> Result<InfrastructureRecognitionV1, InfrastructureInspectionErrorV1> {
    let Some(bytes) = checked_manifest else {
        return Ok(InfrastructureRecognitionV1::InternallyConsistentUnrecognized);
    };
    let checked = CheckedInfrastructureV1::decode(bytes)
        .map_err(|_| InfrastructureInspectionErrorV1::CheckedManifestMismatch)?;
    if checked.profile() != profile
        || checked.profile_pda() != state.profile.key.to_bytes()
        || checked.registry_artifact() != registry_release
        || checked.rent_artifact() != rent_release
        || checked.execution().release_set()
            != activated
                .release_set_projection()
                .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?
        || checked
            .execution()
            .execution_release_set_id()
            .map_err(|_| InfrastructureInspectionErrorV1::CheckedManifestMismatch)?
            != activated
                .execution_release_set_id()
                .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?
    {
        return Err(InfrastructureInspectionErrorV1::CheckedManifestMismatch);
    }
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];
    for (role, checked_artifact) in roles.into_iter().zip(checked.execution().artifacts()) {
        let observed = activated
            .role(role)
            .map_err(|_| InfrastructureInspectionErrorV1::InvalidActivationCache)?;
        if observed.release() != checked_artifact
            || observed.artifact_release_id()
                != checked
                    .execution()
                    .release_set()
                    .binding(role)
                    .artifact_release()
        {
            return Err(InfrastructureInspectionErrorV1::CheckedManifestMismatch);
        }
    }
    Ok(InfrastructureRecognitionV1::RecognizedBySuppliedManifest {
        checked_infrastructure_id: checked
            .checked_infrastructure_id()
            .map_err(|_| InfrastructureInspectionErrorV1::CheckedManifestMismatch)?,
    })
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent, InfrastructureInspectionErrorV1> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(InfrastructureInspectionErrorV1::InvalidRentSysvar);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| InfrastructureInspectionErrorV1::InvalidRentSysvar)
}

fn same_finalized_observation(
    state: &ProtocolInfrastructureStateV1,
) -> Result<Observation, InfrastructureInspectionErrorV1> {
    let accounts = accounts(state);
    let observation = accounts[0].observation;
    if observation.finality != Finality::Finalized {
        return Err(InfrastructureInspectionErrorV1::ObservationNotFinalized);
    }
    for account in accounts {
        if account.observation.finality != Finality::Finalized {
            return Err(InfrastructureInspectionErrorV1::ObservationNotFinalized);
        }
        if account.observation != observation {
            return Err(InfrastructureInspectionErrorV1::ObservationMismatch);
        }
    }
    Ok(observation)
}

fn require_distinct_keys(
    state: &ProtocolInfrastructureStateV1,
) -> Result<(), InfrastructureInspectionErrorV1> {
    let accounts = accounts(state);
    for (index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index + 1)
            .any(|right| left.key == right.key)
        {
            return Err(InfrastructureInspectionErrorV1::AliasedAccount);
        }
    }
    Ok(())
}

fn accounts(state: &ProtocolInfrastructureStateV1) -> [&ObservedAccount; 13] {
    [
        &state.profile,
        &state.activation_cache,
        &state.core_program,
        &state.core_programdata,
        &state.registry_artifact.record,
        &state.registry_artifact.staging_cursor,
        &state.registry_program,
        &state.registry_programdata,
        &state.rent_artifact.record,
        &state.rent_artifact.staging_cursor,
        &state.rent_program,
        &state.rent_programdata,
        &state.rent_sysvar,
    ]
}

fn render_component(output: &mut String, label: &str, evidence: InfrastructureComponentEvidenceV1) {
    push_line(
        output,
        &format!("{label}_program"),
        &evidence.program.to_string(),
    );
    push_line(
        output,
        &format!("{label}_programdata"),
        &evidence.programdata.to_string(),
    );
    push_line(
        output,
        &format!("{label}_artifact_release_id"),
        &encode_hex(evidence.artifact_release_id.as_bytes()),
    );
    push_line(
        output,
        &format!("{label}_semantic_release_id"),
        &encode_hex(evidence.semantic_release_id.as_bytes()),
    );
    push_line(
        output,
        &format!("{label}_elf_sha256"),
        &encode_hex(&evidence.elf_digest),
    );
    push_line(
        output,
        &format!("{label}_deployment_slot"),
        &evidence.deployment_slot.to_string(),
    );
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use dclutch_registry_contract::{
        ArtifactActivationInputV1, ExecutionReleaseActivationInputsV1,
        activate_execution_release_set_v1,
    };
    use dclutch_release_set_contract::{ExecutionReleaseSetV1, ProgramIdentityV1};
    use dclutch_release_tool::{
        BuildMetadataV1, CheckedReleaseV1, LOADER_V3_PROGRAMDATA_METADATA_BYTES,
        RELEASE_METADATA_HEADER_V1, ReleaseEvidenceV1, artifact_release_from_checked,
        build_checked_execution_release_set, build_checked_infrastructure_v1,
        build_checked_release,
    };
    use solana_program::sysvar::SysvarSerialize;

    use super::*;

    fn bytes(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn observation() -> Observation {
        Observation {
            slot: 1_024,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    struct ReleaseFixture {
        checked: CheckedReleaseV1,
        artifact: ArtifactReleaseV1,
        program: ObservedAccount,
        programdata: ObservedAccount,
    }

    impl ReleaseFixture {
        fn immutable(seed: u8) -> Self {
            let program = Pubkey::new_from_array(bytes(seed));
            let programdata =
                Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
            let elf = sbf_elf(seed);
            let program_bytes = loader_program_bytes(programdata);
            let programdata_bytes = immutable_programdata_bytes(u64::from(seed), &elf);
            let metadata = BuildMetadataV1::parse(&metadata_text(program, programdata, seed))
                .expect("metadata");
            let semantic = vec![seed; 16];
            let checked = build_checked_release(ReleaseEvidenceV1 {
                elf: &elf,
                semantic_preimage: &semantic,
                program_account_data: &program_bytes,
                programdata_account_data: &programdata_bytes,
                metadata: &metadata,
            })
            .expect("checked release");
            let artifact = artifact_release_from_checked(&checked).expect("artifact release");
            Self {
                checked,
                artifact,
                program: observed(program, bpf_loader_upgradeable::ID, 1, true, program_bytes),
                programdata: observed(
                    programdata,
                    bpf_loader_upgradeable::ID,
                    1,
                    false,
                    programdata_bytes,
                ),
            }
        }

        fn binding(&self) -> ExecutionRoleBindingV1 {
            ExecutionRoleBindingV1::new(
                self.artifact.program(),
                ArtifactReleaseIdV1::new(hash(&self.artifact.to_bytes()).to_bytes())
                    .expect("artifact ID"),
            )
        }

        fn activation(&self) -> ArtifactActivationInputV1 {
            let programdata =
                ProgramDataV3View::parse(&self.programdata.data).expect("ProgramData");
            ArtifactActivationInputV1::new(
                self.binding().artifact_release(),
                self.artifact,
                DeploymentObservationV1::new(
                    self.program.key.to_bytes(),
                    self.program.owner.to_bytes(),
                    self.program.executable,
                    self.programdata.key.to_bytes(),
                    self.programdata.owner.to_bytes(),
                    self.programdata.executable,
                    self.programdata.key.to_bytes(),
                    bpf_loader_upgradeable::ID.to_bytes(),
                    programdata.deployment_slot(),
                    hash(programdata.elf()).to_bytes(),
                    programdata.upgrade_authority(),
                )
                .expect("deployment"),
            )
        }
    }

    struct Fixture {
        state: ProtocolInfrastructureStateV1,
        checked_bytes: Vec<u8>,
    }

    impl Fixture {
        fn new(seed: u8) -> Self {
            let releases = [
                ReleaseFixture::immutable(seed),
                ReleaseFixture::immutable(seed + 10),
                ReleaseFixture::immutable(seed + 20),
                ReleaseFixture::immutable(seed + 30),
                ReleaseFixture::immutable(seed + 40),
            ];
            let [core, claims, trading, resolution, custody] = releases.each_ref();
            let release_set = ExecutionReleaseSetV1::new(
                core.binding(),
                claims.binding(),
                trading.binding(),
                resolution.binding(),
                custody.binding(),
            )
            .expect("release set");
            let release_set_id =
                ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set ID");
            let execution = build_checked_execution_release_set(
                release_set,
                releases.each_ref().map(|release| &release.checked),
            )
            .expect("execution evidence");
            let activated = activate_execution_release_set_v1(
                release_set_id,
                &release_set,
                &ExecutionReleaseActivationInputsV1::new(
                    core.activation(),
                    claims.activation(),
                    trading.activation(),
                    resolution.activation(),
                    custody.activation(),
                ),
            )
            .expect("activation cache");
            let registry = ReleaseFixture::immutable(seed + 50);
            let rent_program = ReleaseFixture::immutable(seed + 60);
            let profile =
                ProtocolInfrastructureProfileV1::new(registry.binding(), rent_program.binding())
                    .expect("profile");
            let checked = build_checked_infrastructure_v1(
                execution,
                profile,
                &core.checked,
                &registry.checked,
                &rent_program.checked,
            )
            .expect("infrastructure evidence");
            let rent = Rent::default();
            let (registry_artifact, _) = finalized_record(
                registry.program.key,
                registry.artifact.to_bytes().to_vec(),
                &rent,
            );
            let (rent_artifact, _) = finalized_record(
                registry.program.key,
                rent_program.artifact.to_bytes().to_vec(),
                &rent,
            );
            let profile_key = Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &core.program.key,
            )
            .0;
            let cache_key = Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
                &registry.program.key,
            )
            .0;
            let state = ProtocolInfrastructureStateV1 {
                profile: observed(
                    profile_key,
                    core.program.key,
                    rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
                    false,
                    profile.to_bytes().to_vec(),
                ),
                activation_cache: observed(
                    cache_key,
                    registry.program.key,
                    rent.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1),
                    false,
                    activated.to_bytes().to_vec(),
                ),
                core_program: core.program.clone(),
                core_programdata: core.programdata.clone(),
                registry_artifact,
                registry_program: registry.program,
                registry_programdata: registry.programdata,
                rent_artifact,
                rent_program: rent_program.program,
                rent_programdata: rent_program.programdata,
                rent_sysvar: rent_account(rent),
            };
            Self {
                state,
                checked_bytes: checked.encode().to_vec(),
            }
        }
    }

    fn sbf_elf(seed: u8) -> Vec<u8> {
        let mut elf = vec![0; 65];
        put(&mut elf, 0, &[0x7f, b'E', b'L', b'F']);
        put(&mut elf, 4, &[2, 1, 1]);
        put(&mut elf, 16, &3_u16.to_le_bytes());
        put(&mut elf, 18, &263_u16.to_le_bytes());
        put(&mut elf, 20, &1_u32.to_le_bytes());
        put(&mut elf, 52, &64_u16.to_le_bytes());
        put(&mut elf, 64, &[seed]);
        elf
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut output = vec![0; 36];
        put(&mut output, 0, &2_u32.to_le_bytes());
        put(&mut output, 4, programdata.as_ref());
        output
    }

    fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
        let mut output = vec![0; LOADER_V3_PROGRAMDATA_METADATA_BYTES + elf.len()];
        put(&mut output, 0, &3_u32.to_le_bytes());
        put(&mut output, 4, &slot.to_le_bytes());
        put(&mut output, LOADER_V3_PROGRAMDATA_METADATA_BYTES, elf);
        output
    }

    fn put(output: &mut [u8], offset: usize, source: &[u8]) {
        let Some(end) = offset.checked_add(source.len()) else {
            return;
        };
        let Some(destination) = output.get_mut(offset..end) else {
            return;
        };
        destination.copy_from_slice(source);
    }

    fn metadata_text(program: Pubkey, programdata: Pubkey, seed: u8) -> String {
        format!(
            "{RELEASE_METADATA_HEADER_V1}\nsemantic_kind=capability\nprogram_id={}\nprogramdata_id={}\nloader_program_id={}\nprogram_owner={}\nprogram_executable=true\nprogramdata_owner={}\nprogramdata_executable=false\nsource_digest={}\ncargo_lock_digest={}\nsource_revision=revision-{seed}\nrustc_version=rustc-1.89\nsolana_version=solana-3.0\ncargo_build_sbf_version=cargo-build-sbf-3.0\ntarget_triple=sbf-solana-solana\nbuild_command=cargo build-sbf\nassumption=offline hostile fixture\n",
            encode_hex(program.as_ref()),
            encode_hex(programdata.as_ref()),
            encode_hex(bpf_loader_upgradeable::ID.as_ref()),
            encode_hex(bpf_loader_upgradeable::ID.as_ref()),
            encode_hex(bpf_loader_upgradeable::ID.as_ref()),
            encode_hex(&bytes(seed.wrapping_add(80))),
            encode_hex(&bytes(seed.wrapping_add(90))),
        )
    }

    fn finalized_record(
        registry: Pubkey,
        data: Vec<u8>,
        rent: &Rent,
    ) -> (RegistryFinalizedRecordState, [u8; 32]) {
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                &digest,
            ],
            &registry,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                &digest,
            ],
            &registry,
        )
        .0;
        (
            RegistryFinalizedRecordState {
                record: observed(raw, registry, rent.minimum_balance(data.len()), false, data),
                staging_cursor: observed(staging, system_program::ID, 0, false, Vec::new()),
            },
            digest,
        )
    }

    fn rent_account(rent: Rent) -> ObservedAccount {
        let mut lamports = 1;
        let mut data = vec![0; Rent::size_of()];
        let key = sysvar::rent::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        assert_eq!(rent.to_account_info(&mut info), Some(()));
        observed(key, owner, 1, false, data)
    }

    #[test]
    fn consistent_chain_is_unrecognized_until_manifest_is_supplied() {
        let fixture = Fixture::new(11);
        let unrecognized = inspect_protocol_infrastructure_v1(&fixture.state, None)
            .expect("internally consistent chain");
        assert_eq!(
            unrecognized.recognition,
            InfrastructureRecognitionV1::InternallyConsistentUnrecognized,
        );
        assert!(
            unrecognized
                .render_text()
                .contains("recognition=internally-consistent/unrecognized\n"),
        );

        let recognized =
            inspect_protocol_infrastructure_v1(&fixture.state, Some(&fixture.checked_bytes))
                .expect("supplied checked manifest matches");
        assert!(matches!(
            recognized.recognition,
            InfrastructureRecognitionV1::RecognizedBySuppliedManifest { .. }
        ));
        assert!(
            recognized
                .render_text()
                .contains("recognition=supplied-manifest-match\n"),
        );
    }

    #[test]
    fn self_consistent_counterfeit_is_never_recognized_by_another_manifest() {
        let known = Fixture::new(11);
        let counterfeit = Fixture::new(101);
        assert_eq!(
            inspect_protocol_infrastructure_v1(&counterfeit.state, None)
                .expect("self-consistent counterfeit")
                .recognition,
            InfrastructureRecognitionV1::InternallyConsistentUnrecognized,
        );
        assert_eq!(
            inspect_protocol_infrastructure_v1(&counterfeit.state, Some(&known.checked_bytes),),
            Err(InfrastructureInspectionErrorV1::CheckedManifestMismatch),
        );
    }

    #[test]
    fn stale_loader_record_substitution_and_nonfinality_refuse() {
        let mut stale = Fixture::new(11);
        put(
            &mut stale.state.rent_programdata.data,
            4,
            &999_u64.to_le_bytes(),
        );
        assert_eq!(
            inspect_protocol_infrastructure_v1(&stale.state, None),
            Err(InfrastructureInspectionErrorV1::InvalidDeployment),
        );

        let mut substituted = Fixture::new(11);
        substituted.state.registry_artifact.record.owner = Pubkey::new_unique();
        assert_eq!(
            inspect_protocol_infrastructure_v1(&substituted.state, None),
            Err(InfrastructureInspectionErrorV1::InvalidArtifactRecord),
        );

        let mut nonfinal = Fixture::new(11);
        nonfinal.state.profile.observation.finality = Finality::Confirmed;
        assert_eq!(
            inspect_protocol_infrastructure_v1(&nonfinal.state, None),
            Err(InfrastructureInspectionErrorV1::ObservationNotFinalized),
        );
    }

    #[test]
    fn profile_cache_checked_manifest_and_dust_boundaries_are_exact() {
        let mut dust = Fixture::new(11);
        dust.state.rent_artifact.staging_cursor.lamports = 7;
        assert!(inspect_protocol_infrastructure_v1(&dust.state, None).is_ok());

        let mut wrong_profile = Fixture::new(11);
        wrong_profile.state.profile.key = Pubkey::new_unique();
        assert_eq!(
            inspect_protocol_infrastructure_v1(&wrong_profile.state, None),
            Err(InfrastructureInspectionErrorV1::InvalidProfile),
        );

        let mut wrong_cache = Fixture::new(11);
        wrong_cache.state.activation_cache.key = Pubkey::new_unique();
        assert_eq!(
            inspect_protocol_infrastructure_v1(&wrong_cache.state, None),
            Err(InfrastructureInspectionErrorV1::InvalidActivationCache),
        );

        let fixture = Fixture::new(11);
        let mut hostile_manifest = fixture.checked_bytes.clone();
        if let Some(byte) = hostile_manifest.first_mut() {
            *byte ^= 1;
        }
        assert_eq!(
            inspect_protocol_infrastructure_v1(&fixture.state, Some(&hostile_manifest)),
            Err(InfrastructureInspectionErrorV1::CheckedManifestMismatch),
        );
    }

    #[test]
    fn profile_width_is_the_frozen_144_byte_wire() {
        let fixture = Fixture::new(11);
        assert_eq!(fixture.state.profile.data.len(), 144);
        assert_eq!(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, 144);
        assert!(ProgramIdentityV1::new([0; 32]).is_err());
    }
}
