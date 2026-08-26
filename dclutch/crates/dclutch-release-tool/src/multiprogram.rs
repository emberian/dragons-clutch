//! Checked evidence for one complete five-role execution release set.
//!
//! The onchain Registry remains the sole runtime authority. This module binds
//! its canonical release-set preimage to five exact `CheckedReleaseV1`
//! manifests and the five compact `ArtifactReleaseV1` records derived from
//! them. It performs no RPC, signing, deployment, or account mutation.

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_BYTES_V1, EXECUTION_ROLE_COUNT_V1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};

use crate::{CheckedReleaseV1, Error, Result, encode_hex, sha256};

/// Canonical checked multiprogram-manifest magic.
pub const CHECKED_MULTIPROGRAM_MAGIC_V1: [u8; 8] = *b"DCLTMPR1";
/// Implemented checked multiprogram-manifest schema.
pub const CHECKED_MULTIPROGRAM_SCHEMA_V1: u16 = 1;
/// Fixed header before the release set and role evidence.
pub const CHECKED_MULTIPROGRAM_HEADER_BYTES_V1: usize = 16;
/// Bytes in one role's compact artifact record and checked-release identity.
pub const CHECKED_MULTIPROGRAM_ROLE_BYTES_V1: usize = ARTIFACT_RELEASE_BYTES_V1 + 32;
/// Exact width of one checked five-role execution-set manifest.
pub const CHECKED_MULTIPROGRAM_BYTES_V1: usize = CHECKED_MULTIPROGRAM_HEADER_BYTES_V1
    + EXECUTION_RELEASE_SET_BYTES_V1
    + EXECUTION_ROLE_COUNT_V1 * CHECKED_MULTIPROGRAM_ROLE_BYTES_V1;

const SCHEMA_OFFSET: usize = 8;
const ROLE_COUNT_OFFSET: usize = 10;
const EXECUTION_ROLE_COUNT_WIRE_V1: u16 = 5;
const RESERVED_OFFSET: usize = 12;
const RELEASE_SET_OFFSET: usize = CHECKED_MULTIPROGRAM_HEADER_BYTES_V1;
const ROLES_OFFSET: usize = RELEASE_SET_OFFSET + EXECUTION_RELEASE_SET_BYTES_V1;

const ROLES: [ExecutionRoleV1; EXECUTION_ROLE_COUNT_V1] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

/// Canonical offline evidence for one complete Registry execution release set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedExecutionReleaseSetV1 {
    release_set: ExecutionReleaseSetV1,
    artifacts: [ArtifactReleaseV1; EXECUTION_ROLE_COUNT_V1],
    checked_release_ids: [ContentId; EXECUTION_ROLE_COUNT_V1],
}

impl CheckedExecutionReleaseSetV1 {
    /// Decode and revalidate one exact fixed-width multiprogram manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CHECKED_MULTIPROGRAM_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(CHECKED_MULTIPROGRAM_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CHECKED_MULTIPROGRAM_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if usize::from(read_u16(bytes, ROLE_COUNT_OFFSET)?) != EXECUTION_ROLE_COUNT_V1 {
            return Err(Error::InvalidMultiprogramManifest);
        }
        if bytes.get(RESERVED_OFFSET..RELEASE_SET_OFFSET) != Some([0_u8; 4].as_slice()) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let release_set = ExecutionReleaseSetV1::decode(subslice(
            bytes,
            RELEASE_SET_OFFSET,
            EXECUTION_RELEASE_SET_BYTES_V1,
        )?)
        .map_err(|_| Error::InvalidExecutionReleaseSet)?;
        let mut artifacts = [artifact_placeholder()?; EXECUTION_ROLE_COUNT_V1];
        let mut checked_release_ids = [content_placeholder()?; EXECUTION_ROLE_COUNT_V1];
        for (((index, role), artifact_slot), checked_release_id_slot) in ROLES
            .iter()
            .copied()
            .enumerate()
            .zip(artifacts.iter_mut())
            .zip(checked_release_ids.iter_mut())
        {
            let offset = role_offset(index)?;
            let artifact =
                ArtifactReleaseV1::decode(subslice(bytes, offset, ARTIFACT_RELEASE_BYTES_V1)?)
                    .map_err(|_| Error::InvalidArtifactRelease)?;
            let checked_release_id =
                ContentId::new(read_array(bytes, offset + ARTIFACT_RELEASE_BYTES_V1)?)
                    .map_err(|_| Error::ZeroIdentifier)?;
            validate_role_binding(release_set, role, artifact)?;
            *artifact_slot = artifact;
            *checked_release_id_slot = checked_release_id;
        }
        let result = Self {
            release_set,
            artifacts,
            checked_release_ids,
        };
        if result.encode() != bytes {
            return Err(Error::InvalidMultiprogramManifest);
        }
        Ok(result)
    }

    /// Encode the one canonical fixed-width multiprogram evidence manifest.
    pub fn encode(self) -> [u8; CHECKED_MULTIPROGRAM_BYTES_V1] {
        let mut output = [0_u8; CHECKED_MULTIPROGRAM_BYTES_V1];
        copy(&mut output, 0, &CHECKED_MULTIPROGRAM_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &CHECKED_MULTIPROGRAM_SCHEMA_V1.to_le_bytes(),
        );
        copy(
            &mut output,
            ROLE_COUNT_OFFSET,
            &EXECUTION_ROLE_COUNT_WIRE_V1.to_le_bytes(),
        );
        copy(
            &mut output,
            RELEASE_SET_OFFSET,
            &self.release_set.to_bytes(),
        );
        for (index, (artifact, checked_release_id)) in self
            .artifacts
            .iter()
            .zip(self.checked_release_ids.iter())
            .enumerate()
        {
            let offset = ROLES_OFFSET + index * CHECKED_MULTIPROGRAM_ROLE_BYTES_V1;
            copy(&mut output, offset, &artifact.to_bytes());
            copy(
                &mut output,
                offset + ARTIFACT_RELEASE_BYTES_V1,
                checked_release_id.as_bytes(),
            );
        }
        output
    }

    /// Compute the content identity of this exact evidence manifest.
    pub fn checked_execution_release_set_id(self) -> Result<ContentId> {
        ContentId::new(sha256(&self.encode())).map_err(|_| Error::ZeroIdentifier)
    }

    /// Compute the content identity selected by a Market and Registry cache.
    pub fn execution_release_set_id(self) -> Result<ContentId> {
        ContentId::new(sha256(&self.release_set.to_bytes())).map_err(|_| Error::ZeroIdentifier)
    }

    /// Emit a deterministic line-oriented projection of the complete set.
    pub fn render_text(self) -> Result<String> {
        let mut output = String::new();
        push_line(&mut output, "format", "dclutch-checked-multiprogram-v1");
        push_line(
            &mut output,
            "checked_execution_release_set_id",
            &encode_hex(self.checked_execution_release_set_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "execution_release_set_id",
            &encode_hex(self.execution_release_set_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "manifest_bytes",
            &CHECKED_MULTIPROGRAM_BYTES_V1.to_string(),
        );
        for (role, checked_release_id) in ROLES.iter().copied().zip(self.checked_release_ids.iter())
        {
            let prefix = role_label(role);
            let binding = self.release_set.binding(role);
            push_line(
                &mut output,
                &format!("{prefix}_program_id"),
                &encode_hex(binding.program().as_bytes()),
            );
            push_line(
                &mut output,
                &format!("{prefix}_artifact_release_id"),
                &encode_hex(binding.artifact_release().as_bytes()),
            );
            push_line(
                &mut output,
                &format!("{prefix}_checked_release_id"),
                &encode_hex(checked_release_id.as_bytes()),
            );
        }
        Ok(output)
    }

    /// Return the canonical Registry execution release set.
    pub const fn release_set(self) -> ExecutionReleaseSetV1 {
        self.release_set
    }

    /// Return the role-ordered compact artifact-release records.
    pub const fn artifacts(self) -> [ArtifactReleaseV1; EXECUTION_ROLE_COUNT_V1] {
        self.artifacts
    }

    /// Return the role-ordered checked-release manifest identities.
    pub const fn checked_release_ids(self) -> [ContentId; EXECUTION_ROLE_COUNT_V1] {
        self.checked_release_ids
    }
}

/// Build one exact five-role evidence manifest from independently checked
/// artifact manifests and the Registry release set they must implement.
pub fn build_checked_execution_release_set(
    release_set: ExecutionReleaseSetV1,
    checked: [&CheckedReleaseV1; EXECUTION_ROLE_COUNT_V1],
) -> Result<CheckedExecutionReleaseSetV1> {
    let mut artifacts = [artifact_placeholder()?; EXECUTION_ROLE_COUNT_V1];
    let mut checked_release_ids = [content_placeholder()?; EXECUTION_ROLE_COUNT_V1];
    for (((role, checked_release), artifact_slot), checked_release_id_slot) in ROLES
        .iter()
        .copied()
        .zip(checked)
        .zip(artifacts.iter_mut())
        .zip(checked_release_ids.iter_mut())
    {
        let artifact = artifact_release_from_checked(checked_release)?;
        validate_role_binding(release_set, role, artifact)?;
        *artifact_slot = artifact;
        *checked_release_id_slot = checked_release.checked_release_id()?;
    }
    Ok(CheckedExecutionReleaseSetV1 {
        release_set,
        artifacts,
        checked_release_ids,
    })
}

/// Decode a multiprogram manifest and require exact identity with all five
/// supplied checked-release manifests.
pub fn verify_checked_execution_release_set(
    manifest: &[u8],
    checked_manifests: [&[u8]; EXECUTION_ROLE_COUNT_V1],
) -> Result<CheckedExecutionReleaseSetV1> {
    let expected = CheckedExecutionReleaseSetV1::decode(manifest)?;
    let [core, claims, trading, resolution, custody] = checked_manifests;
    let checked = [
        CheckedReleaseV1::decode(core)?,
        CheckedReleaseV1::decode(claims)?,
        CheckedReleaseV1::decode(trading)?,
        CheckedReleaseV1::decode(resolution)?,
        CheckedReleaseV1::decode(custody)?,
    ];
    let rebuilt = build_checked_execution_release_set(expected.release_set, checked.each_ref())?;
    if rebuilt != expected {
        return Err(Error::CheckedMultiprogramManifestMismatch);
    }
    Ok(expected)
}

/// Derive the sole compact onchain artifact-release record from one complete
/// checked build manifest. The returned bytes are suitable for content hashing
/// and finalized-record publication; this function performs no publication.
pub fn artifact_release_from_checked(checked: &CheckedReleaseV1) -> Result<ArtifactReleaseV1> {
    let program =
        ProgramIdentityV1::new(checked.program_id()).map_err(|_| Error::InvalidArtifactRelease)?;
    let loader = ProgramIdentityV1::new(checked.loader_program_id())
        .map_err(|_| Error::InvalidArtifactRelease)?;
    let (policy, authority) = match checked.upgrade_authority() {
        None => (ArtifactUpgradePolicyV1::Immutable, None),
        Some(authority) => (ArtifactUpgradePolicyV1::ExactAuthority, Some(authority)),
    };
    ArtifactReleaseV1::new(
        program,
        loader,
        checked.programdata_id(),
        checked.semantic_release_id(),
        checked.artifact_digest(),
        checked.deployment_slot(),
        policy,
        authority,
    )
    .map_err(|_| Error::InvalidArtifactRelease)
}

fn validate_role_binding(
    release_set: ExecutionReleaseSetV1,
    role: ExecutionRoleV1,
    artifact: ArtifactReleaseV1,
) -> Result<()> {
    let expected = release_set.binding(role);
    let artifact_id = ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes()))
        .map_err(|_| Error::InvalidArtifactRelease)?;
    if expected != ExecutionRoleBindingV1::new(artifact.program(), artifact_id) {
        return Err(Error::InvalidExecutionReleaseSet);
    }
    Ok(())
}

fn role_offset(index: usize) -> Result<usize> {
    index
        .checked_mul(CHECKED_MULTIPROGRAM_ROLE_BYTES_V1)
        .and_then(|relative| ROLES_OFFSET.checked_add(relative))
        .ok_or(Error::ArithmeticOverflow)
}

fn artifact_placeholder() -> Result<ArtifactReleaseV1> {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new([1; 32]).map_err(|_| Error::InvalidArtifactRelease)?,
        ProgramIdentityV1::new([2; 32]).map_err(|_| Error::InvalidArtifactRelease)?,
        [3; 32],
        content_placeholder()?,
        [4; 32],
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .map_err(|_| Error::InvalidArtifactRelease)
}

fn content_placeholder() -> Result<ContentId> {
    ContentId::new([1; 32]).map_err(|_| Error::ZeroIdentifier)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn subslice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn copy(output: &mut [u8], offset: usize, input: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + input.len()) {
        target.copy_from_slice(input);
    }
}

fn role_label(role: ExecutionRoleV1) -> &'static str {
    match role {
        ExecutionRoleV1::Core => "core",
        ExecutionRoleV1::Claims => "claims",
        ExecutionRoleV1::Trading => "trading",
        ExecutionRoleV1::Resolution => "resolution",
        ExecutionRoleV1::Custody => "custody",
    }
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SemanticPreimageKindV1;

    fn checked(seed: u8) -> CheckedReleaseV1 {
        CheckedReleaseV1 {
            semantic_kind: SemanticPreimageKindV1::Capability,
            semantic_preimage_len: 16,
            elf_len: 64,
            program_account_len: 36,
            programdata_account_len: 109,
            deployment_slot: u64::from(seed),
            programdata_elf_offset: 45,
            artifact_digest: [seed.wrapping_add(4); 32],
            semantic_release_id: ContentId::new([seed.wrapping_add(5); 32]).expect("semantic"),
            program_account_digest: [seed.wrapping_add(6); 32],
            programdata_account_digest: [seed.wrapping_add(7); 32],
            program_id: [seed; 32],
            programdata_id: [seed.wrapping_add(1); 32],
            loader_program_id: [seed.wrapping_add(2); 32],
            upgrade_authority: Some([seed.wrapping_add(3); 32]),
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

    fn fixture() -> (
        ExecutionReleaseSetV1,
        [CheckedReleaseV1; EXECUTION_ROLE_COUNT_V1],
    ) {
        let checked = [
            checked(11),
            checked(21),
            checked(31),
            checked(41),
            checked(51),
        ];
        let bindings = checked.each_ref().map(|release| {
            let artifact = artifact_release_from_checked(release).expect("artifact");
            let artifact_id =
                ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes())).expect("artifact id");
            ExecutionRoleBindingV1::new(artifact.program(), artifact_id)
        });
        let [core, claims, trading, resolution, custody] = bindings;
        let release_set = ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
            .expect("release set");
        (release_set, checked)
    }

    #[test]
    fn five_checked_artifacts_have_one_canonical_manifest() {
        let (release_set, checked_releases) = fixture();
        let built = build_checked_execution_release_set(release_set, checked_releases.each_ref())
            .expect("checked set");
        let bytes = built.encode();
        assert_eq!(bytes.len(), CHECKED_MULTIPROGRAM_BYTES_V1);
        assert_eq!(CheckedExecutionReleaseSetV1::decode(&bytes), Ok(built));
        let text = built.render_text().expect("text");
        assert!(text.starts_with("format=dclutch-checked-multiprogram-v1\n"));
        assert_eq!(text.matches("_checked_release_id=").count(), 5);

        let manifests = checked_releases
            .each_ref()
            .map(|release| release.encode().expect("manifest"));
        let verified =
            verify_checked_execution_release_set(&bytes, manifests.each_ref().map(Vec::as_slice))
                .expect("verify");
        assert_eq!(verified, built);
    }

    #[test]
    fn role_and_checked_manifest_substitution_refuse() {
        let (release_set, checked_releases) = fixture();
        let built = build_checked_execution_release_set(release_set, checked_releases.each_ref())
            .expect("checked set");
        let mut bytes = built.encode();
        *bytes
            .get_mut(ROLES_OFFSET + ARTIFACT_RELEASE_BYTES_V1)
            .expect("checked-release identity byte") ^= 1;
        assert!(CheckedExecutionReleaseSetV1::decode(&bytes).is_ok());

        let manifests = checked_releases
            .each_ref()
            .map(|release| release.encode().expect("manifest"));
        assert_eq!(
            verify_checked_execution_release_set(&bytes, manifests.each_ref().map(Vec::as_slice),),
            Err(Error::CheckedMultiprogramManifestMismatch)
        );

        let mut substituted = checked(61).encode().expect("substitute");
        let [core, claims, _trading, resolution, custody] = manifests.each_ref().map(Vec::as_slice);
        assert_eq!(
            verify_checked_execution_release_set(
                &built.encode(),
                [core, claims, &substituted, resolution, custody],
            ),
            Err(Error::InvalidExecutionReleaseSet)
        );
        substituted.clear();
    }

    #[test]
    fn hostile_lengths_headers_and_artifact_bytes_refuse() {
        let (release_set, checked) = fixture();
        let built = build_checked_execution_release_set(release_set, checked.each_ref())
            .expect("checked set");
        let bytes = built.encode();
        let shortened = bytes
            .get(..bytes.len().saturating_sub(1))
            .expect("shortened");
        assert_eq!(
            CheckedExecutionReleaseSetV1::decode(shortened),
            Err(Error::InvalidLength)
        );
        for offset in [0, SCHEMA_OFFSET, ROLE_COUNT_OFFSET, RESERVED_OFFSET] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(CheckedExecutionReleaseSetV1::decode(&hostile).is_err());
        }
        let mut hostile_artifact = bytes;
        *hostile_artifact
            .get_mut(ROLES_OFFSET)
            .expect("artifact byte") ^= 1;
        assert_eq!(
            CheckedExecutionReleaseSetV1::decode(&hostile_artifact),
            Err(Error::InvalidArtifactRelease)
        );
    }
}
