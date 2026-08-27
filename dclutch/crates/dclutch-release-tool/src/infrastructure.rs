//! Checked evidence for one immutable Core/Registry/Rent authority chain.
//!
//! This manifest joins a complete checked execution release set to the
//! Core-owned infrastructure profile and independently checked Registry and
//! Rent releases. It is user-supplied recognition evidence, never an embedded
//! official-program list and never a substitute for observing current Loader
//! state.

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use solana_program::pubkey::Pubkey;

use crate::{
    CHECKED_MULTIPROGRAM_BYTES_V1, CheckedExecutionReleaseSetV1, CheckedReleaseV1, Error, Result,
    artifact_release_from_checked, encode_hex, sha256,
};

/// Canonical checked-infrastructure evidence magic.
pub const CHECKED_INFRASTRUCTURE_MAGIC_V1: [u8; 8] = *b"DCLTIEV1";
/// Implemented checked-infrastructure evidence schema.
pub const CHECKED_INFRASTRUCTURE_SCHEMA_V1: u16 = 1;
/// Number of immutable checked program components: Core, Registry, and Rent.
pub const CHECKED_INFRASTRUCTURE_COMPONENTS_V1: u16 = 3;
/// Fixed checked-infrastructure header width.
pub const CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1: usize = 16;
/// Bytes in one non-Core artifact record plus checked-release identity.
pub const CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1: usize = ARTIFACT_RELEASE_BYTES_V1 + 32;
/// Exact checked-infrastructure evidence width.
pub const CHECKED_INFRASTRUCTURE_BYTES_V1: usize = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1
    + CHECKED_MULTIPROGRAM_BYTES_V1
    + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
    + 32
    + 2 * CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

const SCHEMA_OFFSET: usize = 8;
const COMPONENT_COUNT_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 12;
const EXECUTION_OFFSET: usize = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1;
const PROFILE_OFFSET: usize = EXECUTION_OFFSET + CHECKED_MULTIPROGRAM_BYTES_V1;
const PROFILE_PDA_OFFSET: usize = PROFILE_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1;
const REGISTRY_OFFSET: usize = PROFILE_PDA_OFFSET + 32;
const RENT_OFFSET: usize = REGISTRY_OFFSET + CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

/// Canonical user-supplied evidence recognizing one infrastructure chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedInfrastructureV1 {
    execution: CheckedExecutionReleaseSetV1,
    profile: ProtocolInfrastructureProfileV1,
    profile_pda: [u8; 32],
    registry_artifact: ArtifactReleaseV1,
    registry_checked_release_id: ContentId,
    rent_artifact: ArtifactReleaseV1,
    rent_checked_release_id: ContentId,
}

impl CheckedInfrastructureV1 {
    /// Decode and revalidate one exact fixed-width infrastructure manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CHECKED_INFRASTRUCTURE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(CHECKED_INFRASTRUCTURE_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CHECKED_INFRASTRUCTURE_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, COMPONENT_COUNT_OFFSET)? != CHECKED_INFRASTRUCTURE_COMPONENTS_V1 {
            return Err(Error::InvalidInfrastructureManifest);
        }
        if bytes.get(RESERVED_OFFSET..EXECUTION_OFFSET) != Some([0_u8; 4].as_slice()) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let execution = CheckedExecutionReleaseSetV1::decode(subslice(
            bytes,
            EXECUTION_OFFSET,
            CHECKED_MULTIPROGRAM_BYTES_V1,
        )?)?;
        let profile = ProtocolInfrastructureProfileV1::decode(subslice(
            bytes,
            PROFILE_OFFSET,
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
        )?)
        .map_err(|_| Error::InvalidInfrastructureManifest)?;
        let profile_pda = read_array(bytes, PROFILE_PDA_OFFSET)?;
        let registry_artifact =
            ArtifactReleaseV1::decode(subslice(bytes, REGISTRY_OFFSET, ARTIFACT_RELEASE_BYTES_V1)?)
                .map_err(|_| Error::InvalidArtifactRelease)?;
        let registry_checked_release_id = ContentId::new(read_array(
            bytes,
            REGISTRY_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
        )?)
        .map_err(|_| Error::ZeroIdentifier)?;
        let rent_artifact =
            ArtifactReleaseV1::decode(subslice(bytes, RENT_OFFSET, ARTIFACT_RELEASE_BYTES_V1)?)
                .map_err(|_| Error::InvalidArtifactRelease)?;
        let rent_checked_release_id =
            ContentId::new(read_array(bytes, RENT_OFFSET + ARTIFACT_RELEASE_BYTES_V1)?)
                .map_err(|_| Error::ZeroIdentifier)?;
        let result = Self {
            execution,
            profile,
            profile_pda,
            registry_artifact,
            registry_checked_release_id,
            rent_artifact,
            rent_checked_release_id,
        };
        result.validate()?;
        if result.encode().as_slice() != bytes {
            return Err(Error::InvalidInfrastructureManifest);
        }
        Ok(result)
    }

    /// Encode one exact fixed-width infrastructure evidence manifest.
    pub fn encode(self) -> [u8; CHECKED_INFRASTRUCTURE_BYTES_V1] {
        let mut output = [0_u8; CHECKED_INFRASTRUCTURE_BYTES_V1];
        copy(&mut output, 0, &CHECKED_INFRASTRUCTURE_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &CHECKED_INFRASTRUCTURE_SCHEMA_V1.to_le_bytes(),
        );
        copy(
            &mut output,
            COMPONENT_COUNT_OFFSET,
            &CHECKED_INFRASTRUCTURE_COMPONENTS_V1.to_le_bytes(),
        );
        copy(&mut output, EXECUTION_OFFSET, &self.execution.encode());
        copy(&mut output, PROFILE_OFFSET, &self.profile.to_bytes());
        copy(&mut output, PROFILE_PDA_OFFSET, &self.profile_pda);
        copy(
            &mut output,
            REGISTRY_OFFSET,
            &self.registry_artifact.to_bytes(),
        );
        copy(
            &mut output,
            REGISTRY_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
            self.registry_checked_release_id.as_bytes(),
        );
        copy(&mut output, RENT_OFFSET, &self.rent_artifact.to_bytes());
        copy(
            &mut output,
            RENT_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
            self.rent_checked_release_id.as_bytes(),
        );
        output
    }

    /// Compute the SHA-256 identity of this exact user-supplied manifest.
    pub fn checked_infrastructure_id(self) -> Result<ContentId> {
        ContentId::new(sha256(&self.encode())).map_err(|_| Error::ZeroIdentifier)
    }

    /// Emit a deterministic line-oriented inspection projection.
    pub fn render_text(self) -> Result<String> {
        let mut output = String::new();
        push_line(&mut output, "format", "dclutch-checked-infrastructure-v1");
        push_line(
            &mut output,
            "checked_infrastructure_id",
            &encode_hex(self.checked_infrastructure_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "checked_execution_release_set_id",
            &encode_hex(
                self.execution
                    .checked_execution_release_set_id()?
                    .as_bytes(),
            ),
        );
        push_line(
            &mut output,
            "profile_sha256",
            &encode_hex(&sha256(&self.profile.to_bytes())),
        );
        push_line(&mut output, "profile_pda", &encode_hex(&self.profile_pda));
        push_line(
            &mut output,
            "registry_program_id",
            &encode_hex(self.profile.registry().program().as_bytes()),
        );
        push_line(
            &mut output,
            "registry_artifact_release_id",
            &encode_hex(self.profile.registry().artifact_release().as_bytes()),
        );
        push_line(
            &mut output,
            "registry_checked_release_id",
            &encode_hex(self.registry_checked_release_id.as_bytes()),
        );
        push_line(
            &mut output,
            "rent_program_id",
            &encode_hex(self.profile.rent().program().as_bytes()),
        );
        push_line(
            &mut output,
            "rent_artifact_release_id",
            &encode_hex(self.profile.rent().artifact_release().as_bytes()),
        );
        push_line(
            &mut output,
            "rent_checked_release_id",
            &encode_hex(self.rent_checked_release_id.as_bytes()),
        );
        push_line(
            &mut output,
            "recognition_class",
            "user-supplied-checked-manifest",
        );
        Ok(output)
    }

    /// Return the complete checked execution-release-set evidence.
    pub const fn execution(self) -> CheckedExecutionReleaseSetV1 {
        self.execution
    }

    /// Return the exact immutable Core-owned infrastructure profile.
    pub const fn profile(self) -> ProtocolInfrastructureProfileV1 {
        self.profile
    }

    /// Return the profile PDA derived under the checked Core program.
    pub const fn profile_pda(self) -> [u8; 32] {
        self.profile_pda
    }

    /// Return the checked Registry artifact record.
    pub const fn registry_artifact(self) -> ArtifactReleaseV1 {
        self.registry_artifact
    }

    /// Return the checked Registry build-manifest identity.
    pub const fn registry_checked_release_id(self) -> ContentId {
        self.registry_checked_release_id
    }

    /// Return the checked Rent artifact record.
    pub const fn rent_artifact(self) -> ArtifactReleaseV1 {
        self.rent_artifact
    }

    /// Return the checked Rent build-manifest identity.
    pub const fn rent_checked_release_id(self) -> ContentId {
        self.rent_checked_release_id
    }

    fn validate(self) -> Result<()> {
        let artifacts = self.execution.artifacts();
        let core = artifacts
            .first()
            .copied()
            .ok_or(Error::InvalidInfrastructureManifest)?;
        require_immutable(core)?;
        require_immutable(self.registry_artifact)?;
        require_immutable(self.rent_artifact)?;
        let core_program = core.program().to_bytes();
        let registry_program = self.profile.registry().program().to_bytes();
        let rent_program = self.profile.rent().program().to_bytes();
        if core_program == registry_program
            || core_program == rent_program
            || registry_program == rent_program
        {
            return Err(Error::InvalidInfrastructureManifest);
        }
        validate_binding(self.profile.registry(), self.registry_artifact)?;
        validate_binding(self.profile.rent(), self.rent_artifact)?;
        let expected_profile = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &Pubkey::new_from_array(core_program),
        )
        .0;
        if self.profile_pda != expected_profile.to_bytes() {
            return Err(Error::InvalidInfrastructureManifest);
        }
        Ok(())
    }
}

/// Build one checked infrastructure manifest from independently checked inputs.
pub fn build_checked_infrastructure_v1(
    execution: CheckedExecutionReleaseSetV1,
    profile: ProtocolInfrastructureProfileV1,
    core_checked: &CheckedReleaseV1,
    registry_checked: &CheckedReleaseV1,
    rent_checked: &CheckedReleaseV1,
) -> Result<CheckedInfrastructureV1> {
    let execution_artifacts = execution.artifacts();
    let execution_checked = execution.checked_release_ids();
    let core_artifact = artifact_release_from_checked(core_checked)?;
    if execution_artifacts.first() != Some(&core_artifact)
        || execution_checked.first().copied() != Some(core_checked.checked_release_id()?)
    {
        return Err(Error::CheckedInfrastructureManifestMismatch);
    }
    let registry_artifact = artifact_release_from_checked(registry_checked)?;
    let rent_artifact = artifact_release_from_checked(rent_checked)?;
    let core_program = Pubkey::new_from_array(core_artifact.program().to_bytes());
    let profile_pda = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &core_program,
    )
    .0
    .to_bytes();
    let result = CheckedInfrastructureV1 {
        execution,
        profile,
        profile_pda,
        registry_artifact,
        registry_checked_release_id: registry_checked.checked_release_id()?,
        rent_artifact,
        rent_checked_release_id: rent_checked.checked_release_id()?,
    };
    result.validate()?;
    Ok(result)
}

/// Derive the one canonical Core-owned infrastructure profile that checked
/// Registry and Rent manifests already determine.
///
/// Both bindings are pure functions of the supplied checked manifests, exactly
/// as [`build_checked_infrastructure_v1`] will re-derive and require them. The
/// profile stays Core-owned: this derives its exact bytes offline and never
/// initializes, publishes, or signs the PDA that holds it.
pub fn derive_protocol_infrastructure_profile_v1(
    registry_checked: &CheckedReleaseV1,
    rent_checked: &CheckedReleaseV1,
) -> Result<ProtocolInfrastructureProfileV1> {
    let registry = binding_from_checked(registry_checked)?;
    let rent = binding_from_checked(rent_checked)?;
    ProtocolInfrastructureProfileV1::new(registry, rent)
        .map_err(|_| Error::InvalidInfrastructureManifest)
}

fn binding_from_checked(
    checked: &CheckedReleaseV1,
) -> Result<dclutch_release_set_contract::ExecutionRoleBindingV1> {
    let artifact = artifact_release_from_checked(checked)?;
    require_immutable(artifact)?;
    let artifact_id = ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes()))
        .map_err(|_| Error::InvalidArtifactRelease)?;
    Ok(dclutch_release_set_contract::ExecutionRoleBindingV1::new(
        artifact.program(),
        artifact_id,
    ))
}

/// Rebuild and compare exact infrastructure evidence to user-supplied manifests.
pub fn verify_checked_infrastructure_v1(
    manifest: &[u8],
    execution_manifest: &[u8],
    execution_checked_manifests: [&[u8]; 5],
    registry_checked_manifest: &[u8],
    rent_checked_manifest: &[u8],
) -> Result<CheckedInfrastructureV1> {
    let expected = CheckedInfrastructureV1::decode(manifest)?;
    let execution = crate::verify_checked_execution_release_set(
        execution_manifest,
        execution_checked_manifests,
    )?;
    let core_checked = CheckedReleaseV1::decode(execution_checked_manifests[0])?;
    let registry_checked = CheckedReleaseV1::decode(registry_checked_manifest)?;
    let rent_checked = CheckedReleaseV1::decode(rent_checked_manifest)?;
    let rebuilt = build_checked_infrastructure_v1(
        execution,
        expected.profile,
        &core_checked,
        &registry_checked,
        &rent_checked,
    )?;
    if rebuilt != expected {
        return Err(Error::CheckedInfrastructureManifestMismatch);
    }
    Ok(expected)
}

fn validate_binding(
    expected: dclutch_release_set_contract::ExecutionRoleBindingV1,
    artifact: ArtifactReleaseV1,
) -> Result<()> {
    let artifact_id = ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes()))
        .map_err(|_| Error::InvalidArtifactRelease)?;
    if expected.program() != artifact.program() || expected.artifact_release() != artifact_id {
        return Err(Error::InvalidInfrastructureManifest);
    }
    Ok(())
}

fn require_immutable(artifact: ArtifactReleaseV1) -> Result<()> {
    if artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || artifact.upgrade_authority().is_some()
    {
        return Err(Error::InfrastructureMustBeImmutable);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn copy(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use dclutch_release_set_contract::{
        ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };

    use super::*;
    use crate::{SemanticPreimageKindV1, build_checked_execution_release_set};

    struct Fixture {
        execution: CheckedExecutionReleaseSetV1,
        execution_releases: [CheckedReleaseV1; 5],
        registry: CheckedReleaseV1,
        rent: CheckedReleaseV1,
        profile: ProtocolInfrastructureProfileV1,
        checked: CheckedInfrastructureV1,
    }

    impl Fixture {
        fn immutable() -> Self {
            let execution_releases = [
                release(11, None),
                release(21, None),
                release(31, None),
                release(41, None),
                release(51, None),
            ];
            let execution_artifacts = execution_releases
                .each_ref()
                .map(|checked| artifact_release_from_checked(checked).expect("artifact"));
            let bindings = execution_artifacts.map(binding);
            let [core, claims, trading, resolution, custody] = bindings;
            let release_set =
                ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
                    .expect("release set");
            let execution =
                build_checked_execution_release_set(release_set, execution_releases.each_ref())
                    .expect("execution evidence");
            let registry = release(71, None);
            let rent = release(81, None);
            let profile = ProtocolInfrastructureProfileV1::new(
                binding(artifact_release_from_checked(&registry).expect("Registry artifact")),
                binding(artifact_release_from_checked(&rent).expect("Rent artifact")),
            )
            .expect("profile");
            let checked = build_checked_infrastructure_v1(
                execution,
                profile,
                &execution_releases[0],
                &registry,
                &rent,
            )
            .expect("infrastructure evidence");
            Self {
                execution,
                execution_releases,
                registry,
                rent,
                profile,
                checked,
            }
        }

        fn execution_manifests(&self) -> [Vec<u8>; 5] {
            self.execution_releases
                .each_ref()
                .map(|release| release.encode().expect("release manifest"))
        }
    }

    fn release(seed: u8, authority: Option<[u8; 32]>) -> CheckedReleaseV1 {
        CheckedReleaseV1 {
            semantic_kind: SemanticPreimageKindV1::Capability,
            semantic_preimage_len: 16,
            elf_len: 64,
            program_account_len: 36,
            programdata_account_len: 109,
            deployment_slot: u64::from(seed),
            programdata_elf_offset: 45,
            artifact_digest: [seed.wrapping_add(4); 32],
            semantic_release_id: ContentId::new([seed.wrapping_add(5); 32])
                .expect("semantic release"),
            program_account_digest: [seed.wrapping_add(6); 32],
            programdata_account_digest: [seed.wrapping_add(7); 32],
            program_id: [seed; 32],
            programdata_id: [seed.wrapping_add(1); 32],
            loader_program_id: [seed.wrapping_add(2); 32],
            upgrade_authority: authority,
            source_digest: [seed.wrapping_add(8); 32],
            cargo_lock_digest: [seed.wrapping_add(9); 32],
            source_revision: "revision".to_owned(),
            rustc_version: "rustc".to_owned(),
            solana_version: "solana".to_owned(),
            cargo_build_sbf_version: "cargo-build-sbf".to_owned(),
            target_triple: "sbf-solana-solana".to_owned(),
            build_command: "cargo build-sbf".to_owned(),
            assumptions: vec!["offline-fixture".to_owned()],
        }
    }

    fn binding(artifact: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
        let artifact_id =
            ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes())).expect("artifact release id");
        ExecutionRoleBindingV1::new(artifact.program(), artifact_id)
    }

    #[test]
    fn the_infrastructure_profile_is_derivable_and_refuses_upgradeable_components() {
        let fixture = Fixture::immutable();
        let derived = derive_protocol_infrastructure_profile_v1(&fixture.registry, &fixture.rent)
            .expect("derived profile");
        assert_eq!(derived, fixture.profile);
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                derived,
                &fixture.execution_releases[0],
                &fixture.registry,
                &fixture.rent,
            ),
            Ok(fixture.checked),
        );
        // Derivation must never launder an upgradeable component into a profile
        // that the infrastructure manifest would then have to refuse.
        assert_eq!(
            derive_protocol_infrastructure_profile_v1(&release(71, Some([9; 32])), &fixture.rent),
            Err(Error::InfrastructureMustBeImmutable)
        );
        assert_eq!(
            derive_protocol_infrastructure_profile_v1(
                &fixture.registry,
                &release(81, Some([9; 32]))
            ),
            Err(Error::InfrastructureMustBeImmutable)
        );
        // A different Rent release must move the derived profile.
        assert_ne!(
            derive_protocol_infrastructure_profile_v1(&fixture.registry, &release(91, None))
                .expect("other profile"),
            derived
        );
    }

    #[test]
    fn immutable_infrastructure_has_one_exact_checked_manifest() {
        let fixture = Fixture::immutable();
        let bytes = fixture.checked.encode();
        assert_eq!(bytes.len(), CHECKED_INFRASTRUCTURE_BYTES_V1);
        assert_eq!(CheckedInfrastructureV1::decode(&bytes), Ok(fixture.checked));
        assert_eq!(
            fixture.checked.profile_pda(),
            Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &Pubkey::new_from_array(fixture.execution.artifacts()[0].program().to_bytes()),
            )
            .0
            .to_bytes(),
        );
        let execution_manifests = fixture.execution_manifests();
        assert_eq!(
            verify_checked_infrastructure_v1(
                &bytes,
                &fixture.execution.encode(),
                execution_manifests.each_ref().map(Vec::as_slice),
                &fixture.registry.encode().expect("Registry checked release"),
                &fixture.rent.encode().expect("Rent checked release"),
            ),
            Ok(fixture.checked),
        );
        let text = fixture.checked.render_text().expect("render");
        assert!(text.starts_with("format=dclutch-checked-infrastructure-v1\n"));
        assert!(text.contains("recognition_class=user-supplied-checked-manifest\n"));
    }

    #[test]
    fn profile_pda_and_exact_bindings_cannot_be_substituted() {
        let fixture = Fixture::immutable();
        let mut wrong_pda = fixture.checked.encode();
        *wrong_pda.get_mut(PROFILE_PDA_OFFSET).expect("PDA byte") ^= 1;
        assert_eq!(
            CheckedInfrastructureV1::decode(&wrong_pda),
            Err(Error::InvalidInfrastructureManifest),
        );

        let different_registry = release(91, None);
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                fixture.profile,
                &fixture.execution_releases[0],
                &different_registry,
                &fixture.rent,
            ),
            Err(Error::InvalidInfrastructureManifest),
        );
        let different_core = release(12, None);
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                fixture.profile,
                &different_core,
                &fixture.registry,
                &fixture.rent,
            ),
            Err(Error::CheckedInfrastructureManifestMismatch),
        );
    }

    #[test]
    fn checked_manifest_substitution_cannot_claim_recognition() {
        let fixture = Fixture::immutable();
        let execution_manifests = fixture.execution_manifests();
        let substituted_registry = release(91, None).encode().expect("substitute manifest");
        assert_eq!(
            verify_checked_infrastructure_v1(
                &fixture.checked.encode(),
                &fixture.execution.encode(),
                execution_manifests.each_ref().map(Vec::as_slice),
                &substituted_registry,
                &fixture.rent.encode().expect("Rent checked release"),
            ),
            Err(Error::InvalidInfrastructureManifest),
        );
    }

    #[test]
    fn every_infrastructure_release_must_be_immutable() {
        let fixture = Fixture::immutable();
        let mutable_registry = release(71, Some([72; 32]));
        let mutable_rent = release(81, Some([82; 32]));
        let mut mutable_core_releases = fixture.execution_releases.clone();
        mutable_core_releases[0] = release(11, Some([12; 32]));
        let mutable_core_artifacts = mutable_core_releases
            .each_ref()
            .map(|checked| artifact_release_from_checked(checked).expect("artifact"));
        let [core, claims, trading, resolution, custody] = mutable_core_artifacts.map(binding);
        let mutable_execution = build_checked_execution_release_set(
            ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
                .expect("mutable Core release set"),
            mutable_core_releases.each_ref(),
        )
        .expect("mutable Core execution evidence");

        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                fixture.profile,
                &fixture.execution_releases[0],
                &mutable_registry,
                &fixture.rent,
            ),
            Err(Error::InfrastructureMustBeImmutable),
        );
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                fixture.profile,
                &fixture.execution_releases[0],
                &fixture.registry,
                &mutable_rent,
            ),
            Err(Error::InfrastructureMustBeImmutable),
        );
        assert_eq!(
            build_checked_infrastructure_v1(
                mutable_execution,
                fixture.profile,
                &mutable_core_releases[0],
                &fixture.registry,
                &fixture.rent,
            ),
            Err(Error::InfrastructureMustBeImmutable),
        );
    }

    #[test]
    fn aliases_and_hostile_headers_are_never_canonical() {
        let fixture = Fixture::immutable();
        let core = fixture.execution.artifacts()[0];
        let alias_profile = ProtocolInfrastructureProfileV1::new(
            ExecutionRoleBindingV1::new(
                core.program(),
                fixture.profile.registry().artifact_release(),
            ),
            fixture.profile.rent(),
        )
        .expect("profile codec only forbids Registry/Rent aliases");
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                alias_profile,
                &fixture.execution_releases[0],
                &fixture.registry,
                &fixture.rent,
            ),
            Err(Error::InvalidInfrastructureManifest),
        );
        for offset in [0, SCHEMA_OFFSET, COMPONENT_COUNT_OFFSET, RESERVED_OFFSET] {
            let mut hostile = fixture.checked.encode();
            *hostile.get_mut(offset).expect("header byte") ^= 1;
            assert!(CheckedInfrastructureV1::decode(&hostile).is_err());
        }
        let shortened = fixture.checked.encode();
        assert_eq!(
            CheckedInfrastructureV1::decode(
                shortened
                    .get(..shortened.len() - 1)
                    .expect("short manifest"),
            ),
            Err(Error::InvalidLength),
        );
    }

    #[test]
    fn program_identity_helper_rejects_zero() {
        assert!(ProgramIdentityV1::new([0; 32]).is_err());
    }
}
