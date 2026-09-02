//! Checked evidence for one pinned Core/Registry/Rent authority chain.
//!
//! This manifest joins a complete checked execution release set to the
//! Core-owned infrastructure profile and independently checked Registry and
//! Rent releases. It is user-supplied recognition evidence, never an embedded
//! official-program list and never a substitute for observing current Loader
//! state.
//!
//! It used to be evidence for an IMMUTABLE chain specifically, and refused
//! anything else. Decision 0012 admits a second substrate — upgradeable by an
//! exact named authority, sound on the Loader's slot write rather than on
//! irrevocability — and the two are not the same evidence. So the manifest
//! carries an `evidence_class` naming which one it is, derived from the
//! components rather than supplied beside them; see
//! [`CheckedInfrastructureV1::evidence_class`].

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
};
use solana_program::pubkey::Pubkey;

use crate::{
    CHECKED_MULTIPROGRAM_BYTES_V1, CheckedExecutionReleaseSetV1, CheckedReleaseV1, Error, Result,
    artifact_release_from_checked, encode_hex, sha256,
};

/// Canonical checked-infrastructure evidence magic.
pub const CHECKED_INFRASTRUCTURE_MAGIC_V1: [u8; 8] = *b"DCLTIEV1";
/// Implemented checked-infrastructure evidence schema.
///
/// Schema 2 embeds the 224-byte succession profile where schema 1 embedded the
/// 144-byte predecessor, which moves every offset after it and the total width
/// (2280 -> 2360). The magic names the evidence family and stays `DCLTIEV1`;
/// this field is what says which layout the bytes are in, so that a reader
/// built for either one refuses the other by name instead of misreading it.
pub const CHECKED_INFRASTRUCTURE_SCHEMA_V2: u16 = 2;
/// Number of checked program components: Core, Registry, and Rent.
pub const CHECKED_INFRASTRUCTURE_COMPONENTS_V1: u16 = 3;
/// Fixed checked-infrastructure header width.
pub const CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1: usize = 16;
/// Bytes in one non-Core artifact record plus checked-release identity.
pub const CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1: usize = ARTIFACT_RELEASE_BYTES_V1 + 32;
/// Exact checked-infrastructure evidence width.
pub const CHECKED_INFRASTRUCTURE_BYTES_V1: usize = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1
    + CHECKED_MULTIPROGRAM_BYTES_V1
    + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    + 32
    + 2 * CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

const SCHEMA_OFFSET: usize = 8;
const COMPONENT_COUNT_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 12;
const EXECUTION_OFFSET: usize = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1;
const PROFILE_OFFSET: usize = EXECUTION_OFFSET + CHECKED_MULTIPROGRAM_BYTES_V1;
const PROFILE_PDA_OFFSET: usize = PROFILE_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2;
const REGISTRY_OFFSET: usize = PROFILE_PDA_OFFSET + 32;
const RENT_OFFSET: usize = REGISTRY_OFFSET + CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

/// Canonical user-supplied evidence recognizing one infrastructure chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedInfrastructureV1 {
    execution: CheckedExecutionReleaseSetV1,
    profile: ProtocolInfrastructureProfileV2,
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
        if read_u16(bytes, SCHEMA_OFFSET)? != CHECKED_INFRASTRUCTURE_SCHEMA_V2 {
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
        let profile = ProtocolInfrastructureProfileV2::decode(subslice(
            bytes,
            PROFILE_OFFSET,
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
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
            &CHECKED_INFRASTRUCTURE_SCHEMA_V2.to_le_bytes(),
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
        // APPENDED, never inserted: tools/release/checked-release-candidate.sh
        // and its relatives scrape these projections line by line.
        push_line(&mut output, "evidence_class", self.evidence_class());
        Ok(output)
    }

    /// Name the substrate class this manifest is evidence for.
    ///
    /// Decision 0012 admits two substrates, and they do not prove the same
    /// thing. An immutable release set proves the bytes can never move. A
    /// slot-pinned one proves only that they have not moved YET, and that a
    /// named authority holds the key that could move them — which is exactly
    /// the trade the decision made on purpose, and exactly the kind of thing a
    /// reader must not have to reconstruct from the artifact bytes.
    ///
    /// This is where release-tool's strictness went when
    /// [`require_pinned_component`] stopped refusing mutable components. It is
    /// a derived FIELD, in the precedent of the checked-release projection's
    /// own `evidence_class=loader-state-carrying-an-observed-retained-authority`:
    /// nothing about it is caller-selectable, and a manifest cannot claim the
    /// stronger class while carrying a component that contradicts it, because
    /// the claim is computed from the components rather than supplied beside
    /// them.
    pub fn evidence_class(self) -> &'static str {
        let core = self.execution.artifacts().first().copied();
        let upgradeable = core.is_some_and(|artifact| {
            artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        }) || self.registry_artifact.upgrade_policy()
            != ArtifactUpgradePolicyV1::Immutable
            || self.rent_artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable;
        if upgradeable {
            INFRASTRUCTURE_EVIDENCE_CLASS_SLOT_PINNED_V1
        } else {
            INFRASTRUCTURE_EVIDENCE_CLASS_IMMUTABLE_V1
        }
    }

    /// Return the complete checked execution-release-set evidence.
    pub const fn execution(self) -> CheckedExecutionReleaseSetV1 {
        self.execution
    }

    /// Return the exact Core-owned infrastructure succession profile.
    pub const fn profile(self) -> ProtocolInfrastructureProfileV2 {
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
        require_pinned_component(core)?;
        require_pinned_component(self.registry_artifact)?;
        require_pinned_component(self.rent_artifact)?;
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
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
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
    profile: ProtocolInfrastructureProfileV2,
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
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

/// Derive the one canonical Core-owned V1 infrastructure profile that checked
/// Registry and Rent manifests already determine.
///
/// Both bindings are pure functions of the supplied checked manifests. The
/// profile stays Core-owned: this derives its exact bytes offline and never
/// initializes, publishes, or signs the PDA that holds it.
///
/// This derives the write-once V1 profile, the one
/// `InitializeProtocolInfrastructureV1` commits at
/// `dclutch:infrastructure:v1`. It cannot derive the succession profile
/// [`CheckedInfrastructureV1`] now carries: V2 additionally pins the two
/// predecessor artifact-release ids, which name what the succession succeeded
/// and are therefore not a function of the successor's own manifests.
pub fn derive_protocol_infrastructure_profile_v1(
    registry_checked: &CheckedReleaseV1,
    rent_checked: &CheckedReleaseV1,
) -> Result<ProtocolInfrastructureProfileV1> {
    let registry = binding_from_checked(registry_checked)?;
    let rent = binding_from_checked(rent_checked)?;
    ProtocolInfrastructureProfileV1::new(registry, rent)
        .map_err(|_| Error::InvalidInfrastructureManifest)
}

/// Derive the succession profile the ceremony commits, offline.
///
/// V2's two predecessor artifact ids are exactly the predecessor profile's own
/// binding ids -- literally what `process_initialize_v2` composes from the V1
/// account it authenticates -- so the dumped predecessor account is the whole
/// of the extra input, and nothing here is invented. An operator holding the
/// predecessor account and the two successor manifests can therefore reproduce
/// the ceremony's exact bytes before the ceremony runs, and compare them
/// against what lands afterwards.
pub fn derive_protocol_infrastructure_profile_v2(
    registry_checked: &CheckedReleaseV1,
    rent_checked: &CheckedReleaseV1,
    predecessor: ProtocolInfrastructureProfileV1,
) -> Result<ProtocolInfrastructureProfileV2> {
    let registry = binding_from_checked(registry_checked)?;
    let rent = binding_from_checked(rent_checked)?;
    ProtocolInfrastructureProfileV2::new(
        registry,
        rent,
        predecessor.registry().artifact_release(),
        predecessor.rent().artifact_release(),
    )
    .map_err(|_| Error::InvalidInfrastructureManifest)
}

fn binding_from_checked(
    checked: &CheckedReleaseV1,
) -> Result<dclutch_release_set_contract::ExecutionRoleBindingV1> {
    let artifact = artifact_release_from_checked(checked)?;
    require_pinned_component(artifact)?;
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

/// Admit one component onto the slot-pinned path, and refuse a non-canonical shape.
///
/// This was `upgrade_policy() != Immutable || upgrade_authority().is_some()`,
/// which decision 0012 retired: a release the whole protocol now admits was
/// refused here, so no checked manifest could describe the iteration substrate
/// at all. What replaces it is
/// [`dclutch_registry_contract::require_slot_pinned_release_v1`] — the SAME
/// predicate every on-chain reader calls, not a second copy of the rule.
///
/// The strictness that used to live in this function did not disappear; it
/// moved to where it can be read. See [`INFRASTRUCTURE_EVIDENCE_CLASS_*`]: a
/// manifest now SAYS which substrate class it describes, derived from the
/// components it actually carries. That is deliberately not a decode flag —
/// making a codec's strictness caller-selectable would make the same bytes mean
/// two things depending on who called, and then the manifest would no longer be
/// evidence of anything on its own.
fn require_pinned_component(artifact: ArtifactReleaseV1) -> Result<()> {
    dclutch_registry_contract::require_slot_pinned_release_v1(artifact)
        .map_err(|_| Error::InfrastructureMustBeImmutable)
}

/// Every component can never be redeployed: the strongest class, and the one
/// the public demo ceremony produces.
pub const INFRASTRUCTURE_EVIDENCE_CLASS_IMMUTABLE_V1: &str = "immutable-release-set";
/// At least one component is upgradeable by an exact named authority.
///
/// Decision 0012's iteration substrate. Soundness here rests on the Loader V3
/// slot write rather than on irrevocability: the named authority CAN ship new
/// bytes, and the instant it does every dependent market refuses by name
/// (`ReleaseSuperseded`) until a re-release re-pins. That is a real and
/// disclosed difference in what this manifest proves, so it is stated rather
/// than left for a reader to infer from the artifact bytes.
pub const INFRASTRUCTURE_EVIDENCE_CLASS_SLOT_PINNED_V1: &str =
    "slot-pinned-release-set-with-a-retained-upgrade-authority";

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
        predecessor_registry_artifact: ArtifactReleaseIdV1,
        profile: ProtocolInfrastructureProfileV2,
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
            let predecessor_registry_artifact = predecessor_artifact(71, None);
            let profile = succeed(
                ProtocolInfrastructureProfileV1::new(
                    binding(artifact_release_from_checked(&registry).expect("Registry artifact")),
                    binding(artifact_release_from_checked(&rent).expect("Rent artifact")),
                )
                .expect("profile"),
                predecessor_registry_artifact,
            );
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
                predecessor_registry_artifact,
                profile,
                checked,
            }
        }

        /// The same shape with every component upgradeable by one exact key.
        ///
        /// Decision 0012's iteration substrate. Before this lane nothing in
        /// this module could construct it: `require_immutable` refused it three
        /// times inside `validate`, so no fixture, no test and no operator
        /// could produce a checked manifest describing the substrate the
        /// project actually iterates on.
        fn slot_pinned(authority: [u8; 32]) -> Self {
            let execution_releases = [
                release(11, Some(authority)),
                release(21, Some(authority)),
                release(31, Some(authority)),
                release(41, Some(authority)),
                release(51, Some(authority)),
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
            let registry = release(71, Some(authority));
            let rent = release(81, Some(authority));
            let predecessor_registry_artifact = predecessor_artifact(71, Some(authority));
            let profile = succeed(
                derive_protocol_infrastructure_profile_v1(&registry, &rent)
                    .expect("slot-pinned profile"),
                predecessor_registry_artifact,
            );
            let checked = build_checked_infrastructure_v1(
                execution,
                profile,
                &execution_releases[0],
                &registry,
                &rent,
            )
            .expect("slot-pinned infrastructure evidence");
            Self {
                execution,
                execution_releases,
                registry,
                rent,
                predecessor_registry_artifact,
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

    /// The artifact-release id of a distinct earlier release of `seed`'s program.
    ///
    /// Same program identity, different artifact bytes and deployment slot:
    /// the shape a succession arm's predecessor has.
    fn predecessor_artifact(seed: u8, authority: Option<[u8; 32]>) -> ArtifactReleaseIdV1 {
        let mut checked = release(seed, authority);
        checked.artifact_digest = [seed.wrapping_add(100); 32];
        checked.deployment_slot = checked.deployment_slot.wrapping_sub(1);
        binding(artifact_release_from_checked(&checked).expect("predecessor artifact"))
            .artifact_release()
    }

    /// Lift one pair of live bindings into the succession profile the manifest carries.
    ///
    /// Registry moved across the succession and Rent did not: the predecessor
    /// Registry id names the distinct release this profile succeeded, while
    /// Rent holds the same id on both sides of it.
    fn succeed(
        live: ProtocolInfrastructureProfileV1,
        predecessor_registry_artifact: ArtifactReleaseIdV1,
    ) -> ProtocolInfrastructureProfileV2 {
        ProtocolInfrastructureProfileV2::new(
            live.registry(),
            live.rent(),
            predecessor_registry_artifact,
            live.rent().artifact_release(),
        )
        .expect("succession profile")
    }

    #[test]
    fn the_infrastructure_profile_is_derivable_and_carries_each_component_policy() {
        let fixture = Fixture::immutable();
        let derived = derive_protocol_infrastructure_profile_v1(&fixture.registry, &fixture.rent)
            .expect("derived profile");
        assert_eq!(derived.registry(), fixture.profile.registry());
        assert_eq!(derived.rent(), fixture.profile.rent());
        assert_eq!(
            build_checked_infrastructure_v1(
                fixture.execution,
                succeed(derived, fixture.predecessor_registry_artifact),
                &fixture.execution_releases[0],
                &fixture.registry,
                &fixture.rent,
            ),
            Ok(fixture.checked),
        );
        // Decision 0012. This used to assert that derivation REFUSED an
        // upgradeable component, so that the manifest would never have to. The
        // protocol admits it now, and derivation carries it — but it can never
        // launder one: an upgradeable Registry produces a DIFFERENT profile
        // from the immutable one, because the artifact bytes carry the policy
        // and the authority and the binding is their digest. So substituting a
        // mutable component for an immutable one is still refused everywhere a
        // profile is compared, by identity rather than by a policy check.
        let upgradeable_registry =
            derive_protocol_infrastructure_profile_v1(&release(71, Some([9; 32])), &fixture.rent)
                .expect("slot-pinned Registry derives");
        assert_ne!(upgradeable_registry, derived);
        let upgradeable_rent = derive_protocol_infrastructure_profile_v1(
            &fixture.registry,
            &release(81, Some([9; 32])),
        )
        .expect("slot-pinned Rent derives");
        assert_ne!(upgradeable_rent, derived);
        // A different Rent release must move the derived profile.
        assert_ne!(
            derive_protocol_infrastructure_profile_v1(&fixture.registry, &release(91, None))
                .expect("other profile"),
            derived
        );
    }

    /// The genesis manifest describes BOTH profiles one instruction commits.
    ///
    /// Schema 3 pinned only the 144-byte V1 -- correct while
    /// `InitializeProtocolInfrastructureV1` wrote only that, and half a chain
    /// act since `c60b25e8`. The V2 half is DERIVED from the V1's two
    /// bindings, so the assertions worth making are that it cannot be
    /// substituted and that the two halves cannot disagree.
    #[test]
    fn a_genesis_manifest_carries_both_profiles_and_neither_can_be_substituted() {
        let fixture = Fixture::immutable();
        let core_program =
            Pubkey::new_from_array(fixture.execution.artifacts()[0].program().to_bytes());
        let v1 = ProtocolInfrastructureProfileV1::new(
            binding(artifact_release_from_checked(&fixture.registry).expect("Registry artifact")),
            binding(artifact_release_from_checked(&fixture.rent).expect("Rent artifact")),
        )
        .expect("genesis V1 profile");
        let genesis = build_checked_genesis_infrastructure_v1(
            fixture.execution,
            v1,
            &fixture.execution_releases[0],
            &fixture.registry,
            &fixture.rent,
        )
        .expect("genesis manifest");
        let bytes = genesis.encode();
        assert_eq!(bytes.len(), CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1);
        assert_ne!(
            CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1,
            CHECKED_GENESIS_INFRASTRUCTURE_BYTES_RETIRED_V3,
            "the widened genesis manifest must not be mistakable for the retired one"
        );
        assert_ne!(
            CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1,
            CHECKED_INFRASTRUCTURE_BYTES_V1
        );
        assert_eq!(
            u16::from_le_bytes([bytes[SCHEMA_OFFSET], bytes[SCHEMA_OFFSET + 1]]),
            CHECKED_INFRASTRUCTURE_SCHEMA_GENESIS_V4
        );
        assert_eq!(CheckedGenesisInfrastructureV1::decode(&bytes), Ok(genesis));

        // The V2 is the one every on-chain reader authenticates, and it is a
        // genesis: both sentinels, same two bindings, its own PDA domain.
        assert!(genesis.genesis_profile_v2().born_at_v2());
        assert_eq!(genesis.genesis_profile_v2().registry(), v1.registry());
        assert_eq!(genesis.genesis_profile_v2().rent(), v1.rent());
        assert_eq!(
            genesis.genesis_profile_v2_pda(),
            Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &core_program,
            )
            .0
            .to_bytes()
        );
        assert_eq!(
            genesis.profile_pda(),
            Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &core_program,
            )
            .0
            .to_bytes()
        );
        assert_ne!(genesis.genesis_profile_v2_pda(), genesis.profile_pda());

        // A SUCCEEDED V2 in the genesis slot is well-formed bytes at the right
        // width and must still refuse: it is not born at V2, so it does not
        // describe a cohort that succeeds nothing.
        let mut forged = bytes;
        forged[GENESIS_PROFILE_V2_OFFSET
            ..GENESIS_PROFILE_V2_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2]
            .copy_from_slice(&succeed(v1, fixture.predecessor_registry_artifact).to_bytes());
        assert_eq!(
            CheckedGenesisInfrastructureV1::decode(&forged),
            Err(Error::InvalidInfrastructureManifest)
        );

        // And a V2 PDA moved to any other address refuses too.
        let mut moved = bytes;
        moved[GENESIS_PROFILE_V2_PDA_OFFSET..GENESIS_PROFILE_V2_PDA_OFFSET + 32]
            .copy_from_slice(&genesis.profile_pda());
        assert_eq!(
            CheckedGenesisInfrastructureV1::decode(&moved),
            Err(Error::InvalidInfrastructureManifest)
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
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
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

    /// Decision 0012, and the whole point of this lane's change to this module.
    ///
    /// This test was `every_infrastructure_release_must_be_immutable` and it
    /// asserted three `Err(InfrastructureMustBeImmutable)`. That is no longer
    /// the protocol: an `ExactAuthority` release with a bound authority is one
    /// of the two canonical pinned shapes and every on-chain reader admits it.
    /// What must remain true, and what this asserts instead, is that the
    /// weaker substrate can never be MISTAKEN for the stronger one — so the
    /// manifest states which it is, and the statement is computed from the
    /// components rather than supplied beside them.
    #[test]
    fn every_infrastructure_release_states_the_substrate_class_it_is_evidence_for() {
        let fixture = Fixture::immutable();
        assert_eq!(
            fixture.checked.evidence_class(),
            INFRASTRUCTURE_EVIDENCE_CLASS_IMMUTABLE_V1
        );
        let pinned = Fixture::slot_pinned([72; 32]);
        assert_eq!(
            pinned.checked.evidence_class(),
            INFRASTRUCTURE_EVIDENCE_CLASS_SLOT_PINNED_V1
        );
        assert!(
            pinned
                .checked
                .render_text()
                .expect("render")
                .contains(INFRASTRUCTURE_EVIDENCE_CLASS_SLOT_PINNED_V1)
        );
        // Round-trips as its own bytes, which the Immutable-only gate made
        // impossible: `decode` calls `validate`, so no such manifest could
        // survive its own codec.
        assert_eq!(
            CheckedInfrastructureV1::decode(&pinned.checked.encode()),
            Ok(pinned.checked)
        );

        // ONE upgradeable component out of three is enough to move the class.
        // A manifest whose Core can be replaced does not become immutable
        // evidence because Registry and Rent cannot.
        for mutable in [release(71, Some([72; 32])), release(81, Some([82; 32]))] {
            let is_registry = mutable.deployment_slot() == 71;
            let mixed = build_checked_infrastructure_v1(
                fixture.execution,
                succeed(
                    derive_protocol_infrastructure_profile_v1(
                        if is_registry {
                            &mutable
                        } else {
                            &fixture.registry
                        },
                        if is_registry { &fixture.rent } else { &mutable },
                    )
                    .expect("mixed profile"),
                    fixture.predecessor_registry_artifact,
                ),
                &fixture.execution_releases[0],
                if is_registry {
                    &mutable
                } else {
                    &fixture.registry
                },
                if is_registry { &fixture.rent } else { &mutable },
            )
            .expect("a mixed substrate is admissible and says so");
            assert_eq!(
                mixed.evidence_class(),
                INFRASTRUCTURE_EVIDENCE_CLASS_SLOT_PINNED_V1
            );
        }

        // The residue of the gate. `require_pinned_component` is TOTAL on
        // anything this tool can build, exactly as the contract's own predicate
        // is total on decoded records: `ArtifactReleaseV1::new` refuses a
        // non-canonical policy/authority pairing before it can reach here, and
        // `artifact_release_from_checked` derives the policy FROM the authority
        // so it cannot construct one either. The check stays because it states
        // the admission out loud in a greppable place, and because a future
        // caller that skipped the constructor must still be refused — not
        // because a fixture in this file can reach it.
        for artifact in [
            artifact_release_from_checked(&fixture.registry).expect("immutable artifact"),
            artifact_release_from_checked(&release(71, Some([72; 32]))).expect("pinned artifact"),
        ] {
            assert_eq!(require_pinned_component(artifact), Ok(()));
        }
        assert!(
            ArtifactReleaseV1::new(
                ProgramIdentityV1::new([1; 32]).expect("program"),
                ProgramIdentityV1::new([2; 32]).expect("loader"),
                [3; 32],
                ContentId::new([4; 32]).expect("semantic"),
                [5; 32],
                7,
                ArtifactUpgradePolicyV1::Immutable,
                Some([6; 32]),
            )
            .is_err(),
            "an Immutable release carrying an authority is not constructible",
        );
        assert!(
            ArtifactReleaseV1::new(
                ProgramIdentityV1::new([1; 32]).expect("program"),
                ProgramIdentityV1::new([2; 32]).expect("loader"),
                [3; 32],
                ContentId::new([4; 32]).expect("semantic"),
                [5; 32],
                7,
                ArtifactUpgradePolicyV1::ExactAuthority,
                None,
            )
            .is_err(),
            "an ExactAuthority release carrying no authority is not constructible",
        );
    }

    #[test]
    fn aliases_and_hostile_headers_are_never_canonical() {
        let fixture = Fixture::immutable();
        let core = fixture.execution.artifacts()[0];
        let alias_profile = ProtocolInfrastructureProfileV2::new(
            ExecutionRoleBindingV1::new(
                core.program(),
                fixture.profile.registry().artifact_release(),
            ),
            fixture.profile.rent(),
            fixture.predecessor_registry_artifact,
            fixture.profile.predecessor_rent_artifact(),
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

// ---------------------------------------------------------------- genesis
//
// A REAL COHORT WITH NO PREDECESSOR, which this manifest family could not
// express until now.
//
// `CheckedInfrastructureV1` embeds a `ProtocolInfrastructureProfileV2` by
// type, and V2 exists precisely to pin the two predecessor artifact-release
// ids a succession copies forward. That is correct for a succession and
// unsatisfiable for a founding: a cohort that succeeds nothing commits the
// write-once V1 profile `InitializeProtocolInfrastructureV1` writes at
// `dclutch:infrastructure:v1`, and feeding those 144 bytes to the succession
// manifest earns `InvalidLength` -- which is where the genesis checked-release
// candidate stopped.
//
// This is the same mechanism the family already uses rather than a new one.
// The header's schema field is what says which layout the bytes are in:
// schema 1 embedded a 144-byte profile, schema 2 embeds the 224-byte
// succession profile, and schema 3 here embeds the 144-byte GENESIS profile.
// A reader built for one refuses the others by name instead of misreading
// them, and no succession manifest changes by a single byte.
//
// The PDA is the other half of the difference and it is not cosmetic: a
// genesis profile lives under `PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1`,
// not the V2 domain, because it is a different account. Deriving it from the
// succession domain would produce a manifest that pins an address the chain
// will never write.

/// Schema discriminant for the genesis (no-predecessor) layout.
///
/// Schema 3 embedded only the 144-byte V1. Since `c60b25e8`
/// `InitializeProtocolInfrastructureV1` commits BOTH profiles in one
/// instruction -- the sealed V1 historical record and the genesis V2 every
/// consumer actually reads -- so a manifest pinning only the V1 describes half
/// the chain act it claims to check. Schema 4 embeds both bodies and both
/// PDAs. Schema 3 is retired rather than kept beside this: it can no longer
/// describe any cohort this tree deploys.
pub const CHECKED_INFRASTRUCTURE_SCHEMA_GENESIS_V4: u16 = 4;

/// Retired genesis width, kept only so a stale manifest refuses by name.
pub const CHECKED_GENESIS_INFRASTRUCTURE_BYTES_RETIRED_V3: usize =
    CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1
        + CHECKED_MULTIPROGRAM_BYTES_V1
        + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        + 32
        + 2 * CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

/// Exact checked genesis-infrastructure evidence width.
pub const CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1: usize = CHECKED_INFRASTRUCTURE_HEADER_BYTES_V1
    + CHECKED_MULTIPROGRAM_BYTES_V1
    + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
    + 32
    + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    + 32
    + 2 * CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

const GENESIS_PROFILE_OFFSET: usize = EXECUTION_OFFSET + CHECKED_MULTIPROGRAM_BYTES_V1;
const GENESIS_PROFILE_PDA_OFFSET: usize =
    GENESIS_PROFILE_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1;
const GENESIS_PROFILE_V2_OFFSET: usize = GENESIS_PROFILE_PDA_OFFSET + 32;
const GENESIS_PROFILE_V2_PDA_OFFSET: usize =
    GENESIS_PROFILE_V2_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2;
const GENESIS_REGISTRY_OFFSET: usize = GENESIS_PROFILE_V2_PDA_OFFSET + 32;
const GENESIS_RENT_OFFSET: usize = GENESIS_REGISTRY_OFFSET + CHECKED_INFRASTRUCTURE_LEAF_BYTES_V1;

/// Checked evidence for one founded Core/Registry/Rent chain that succeeds
/// nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedGenesisInfrastructureV1 {
    execution: CheckedExecutionReleaseSetV1,
    profile: ProtocolInfrastructureProfileV1,
    profile_pda: [u8; 32],
    /// The genesis V2 the same instruction commits at the V2 domain.
    ///
    /// It is a pure function of the V1's two bindings, so it is DERIVED here
    /// and never supplied: a manifest cannot pin a V2 that disagrees with the
    /// V1 beside it.
    genesis_profile_v2: ProtocolInfrastructureProfileV2,
    genesis_profile_v2_pda: [u8; 32],
    registry_artifact: ArtifactReleaseV1,
    registry_checked_release_id: ContentId,
    rent_artifact: ArtifactReleaseV1,
    rent_checked_release_id: ContentId,
}

impl CheckedGenesisInfrastructureV1 {
    /// Decode and revalidate one exact fixed-width genesis manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(CHECKED_INFRASTRUCTURE_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CHECKED_INFRASTRUCTURE_SCHEMA_GENESIS_V4 {
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
            GENESIS_PROFILE_OFFSET,
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
        )?)
        .map_err(|_| Error::InvalidInfrastructureManifest)?;
        let profile_pda = read_array(bytes, GENESIS_PROFILE_PDA_OFFSET)?;
        let genesis_profile_v2 = ProtocolInfrastructureProfileV2::decode(subslice(
            bytes,
            GENESIS_PROFILE_V2_OFFSET,
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
        )?)
        .map_err(|_| Error::InvalidInfrastructureManifest)?;
        let genesis_profile_v2_pda = read_array(bytes, GENESIS_PROFILE_V2_PDA_OFFSET)?;
        let registry_artifact = ArtifactReleaseV1::decode(subslice(
            bytes,
            GENESIS_REGISTRY_OFFSET,
            ARTIFACT_RELEASE_BYTES_V1,
        )?)
        .map_err(|_| Error::InvalidArtifactRelease)?;
        let registry_checked_release_id = ContentId::new(read_array(
            bytes,
            GENESIS_REGISTRY_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
        )?)
        .map_err(|_| Error::ZeroIdentifier)?;
        let rent_artifact = ArtifactReleaseV1::decode(subslice(
            bytes,
            GENESIS_RENT_OFFSET,
            ARTIFACT_RELEASE_BYTES_V1,
        )?)
        .map_err(|_| Error::InvalidArtifactRelease)?;
        let rent_checked_release_id = ContentId::new(read_array(
            bytes,
            GENESIS_RENT_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
        )?)
        .map_err(|_| Error::ZeroIdentifier)?;
        let result = Self {
            execution,
            profile,
            profile_pda,
            genesis_profile_v2,
            genesis_profile_v2_pda,
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

    /// Encode one exact fixed-width genesis infrastructure manifest.
    pub fn encode(self) -> [u8; CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1] {
        let mut output = [0_u8; CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1];
        copy(&mut output, 0, &CHECKED_INFRASTRUCTURE_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &CHECKED_INFRASTRUCTURE_SCHEMA_GENESIS_V4.to_le_bytes(),
        );
        copy(
            &mut output,
            COMPONENT_COUNT_OFFSET,
            &CHECKED_INFRASTRUCTURE_COMPONENTS_V1.to_le_bytes(),
        );
        copy(&mut output, EXECUTION_OFFSET, &self.execution.encode());
        copy(
            &mut output,
            GENESIS_PROFILE_OFFSET,
            &self.profile.to_bytes(),
        );
        copy(&mut output, GENESIS_PROFILE_PDA_OFFSET, &self.profile_pda);
        copy(
            &mut output,
            GENESIS_PROFILE_V2_OFFSET,
            &self.genesis_profile_v2.to_bytes(),
        );
        copy(
            &mut output,
            GENESIS_PROFILE_V2_PDA_OFFSET,
            &self.genesis_profile_v2_pda,
        );
        copy(
            &mut output,
            GENESIS_REGISTRY_OFFSET,
            &self.registry_artifact.to_bytes(),
        );
        copy(
            &mut output,
            GENESIS_REGISTRY_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
            self.registry_checked_release_id.as_bytes(),
        );
        copy(
            &mut output,
            GENESIS_RENT_OFFSET,
            &self.rent_artifact.to_bytes(),
        );
        copy(
            &mut output,
            GENESIS_RENT_OFFSET + ARTIFACT_RELEASE_BYTES_V1,
            self.rent_checked_release_id.as_bytes(),
        );
        output
    }

    /// SHA-256 identity of this exact manifest.
    pub fn checked_infrastructure_id(self) -> Result<ContentId> {
        ContentId::new(sha256(&self.encode())).map_err(|_| Error::ZeroIdentifier)
    }

    /// The founded profile this manifest pins.
    pub fn profile(self) -> ProtocolInfrastructureProfileV1 {
        self.profile
    }

    /// The V1-domain profile PDA this manifest pins.
    pub fn profile_pda(self) -> [u8; 32] {
        self.profile_pda
    }

    /// The genesis V2 the same initialization commits, which every consumer reads.
    pub fn genesis_profile_v2(self) -> ProtocolInfrastructureProfileV2 {
        self.genesis_profile_v2
    }

    /// The V2-domain profile PDA this manifest pins.
    pub fn genesis_profile_v2_pda(self) -> [u8; 32] {
        self.genesis_profile_v2_pda
    }

    /// Which substrate the components describe, derived rather than supplied.
    pub fn evidence_class(self) -> &'static str {
        let mut upgradeable = false;
        for artifact in self.execution.artifacts() {
            if artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable {
                upgradeable = true;
            }
        }
        if self.registry_artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
            || self.rent_artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        {
            upgradeable = true;
        }
        if upgradeable {
            "genesis-exact-authority"
        } else {
            "genesis-immutable"
        }
    }

    /// Render the manifest for an operator, naming its lineage first.
    pub fn render_text(self) -> Result<String> {
        let mut output = String::new();
        push_line(
            &mut output,
            "format",
            "dclutch-checked-genesis-infrastructure-v1",
        );
        // Stated first and unconditionally: a reader must never have to infer
        // from an absent predecessor field that this founds rather than
        // succeeds.
        push_line(&mut output, "infrastructure_lineage", "genesis");
        push_line(&mut output, "predecessor_infrastructure_profile", "none");
        push_line(&mut output, "evidence_class", self.evidence_class());
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
        // `profile_pda`, the same key the succession manifest uses: the
        // candidate summary namespaces these under `infrastructure.`, so a
        // different spelling here would silently omit a required summary field.
        push_line(&mut output, "profile_pda", &encode_hex(&self.profile_pda));
        // The genesis V2 the same instruction commits. Stated beside the V1
        // rather than instead of it, because initialization writes both and a
        // reader who saw only the V1 would believe the cohort stands on a
        // profile nothing reads.
        push_line(
            &mut output,
            "genesis_profile_v2_sha256",
            &encode_hex(&sha256(&self.genesis_profile_v2.to_bytes())),
        );
        push_line(
            &mut output,
            "genesis_profile_v2_pda",
            &encode_hex(&self.genesis_profile_v2_pda),
        );
        push_line(&mut output, "genesis_profile_v2_born_at_v2", "true");
        // The same component keys, in the same order, as the succession
        // manifest. Every scraper that reads `infrastructure.*` out of a
        // candidate summary reads BOTH, so a genesis that spelled these
        // differently would silently omit required summary fields rather than
        // fail loudly.
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

    fn validate(self) -> Result<()> {
        let artifacts = self.execution.artifacts();
        let core = artifacts
            .first()
            .copied()
            .ok_or(Error::InvalidInfrastructureManifest)?;
        require_pinned_component(core)?;
        require_pinned_component(self.registry_artifact)?;
        require_pinned_component(self.rent_artifact)?;
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
        // The V1 domain, because a founded profile is a different account from
        // a succeeded one. Using the V2 domain here would pin an address the
        // chain will never write.
        let expected_profile = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &Pubkey::new_from_array(core_program),
        )
        .0;
        if self.profile_pda != expected_profile.to_bytes() {
            return Err(Error::InvalidInfrastructureManifest);
        }
        // The genesis V2 is DERIVED from the same two bindings, so a manifest
        // whose two profiles disagree -- or whose V2 carries anything but the
        // two sentinels -- cannot decode. There is nothing here a caller
        // supplies and therefore nothing to get wrong.
        let expected_v2 =
            ProtocolInfrastructureProfileV2::genesis(self.profile.registry(), self.profile.rent())
                .map_err(|_| Error::InvalidInfrastructureManifest)?;
        let expected_v2_pda = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
            &Pubkey::new_from_array(core_program),
        )
        .0;
        if self.genesis_profile_v2 != expected_v2
            || !self.genesis_profile_v2.born_at_v2()
            || self.genesis_profile_v2_pda != expected_v2_pda.to_bytes()
            || self.genesis_profile_v2_pda == self.profile_pda
        {
            return Err(Error::InvalidInfrastructureManifest);
        }
        Ok(())
    }
}

/// Build the checked manifest for a cohort that succeeds nothing.
pub fn build_checked_genesis_infrastructure_v1(
    execution: CheckedExecutionReleaseSetV1,
    profile: ProtocolInfrastructureProfileV1,
    core_checked: &CheckedReleaseV1,
    registry_checked: &CheckedReleaseV1,
    rent_checked: &CheckedReleaseV1,
) -> Result<CheckedGenesisInfrastructureV1> {
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
    let genesis_profile_v2 =
        ProtocolInfrastructureProfileV2::genesis(profile.registry(), profile.rent())
            .map_err(|_| Error::InvalidInfrastructureManifest)?;
    let genesis_profile_v2_pda = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &core_program,
    )
    .0
    .to_bytes();
    let result = CheckedGenesisInfrastructureV1 {
        execution,
        profile,
        profile_pda,
        genesis_profile_v2,
        genesis_profile_v2_pda,
        registry_artifact,
        registry_checked_release_id: registry_checked.checked_release_id()?,
        rent_artifact,
        rent_checked_release_id: rent_checked.checked_release_id()?,
    };
    result.validate()?;
    Ok(result)
}

/// Reproduce a genesis manifest from its own inputs and require equality.
pub fn verify_checked_genesis_infrastructure_v1(
    manifest: &[u8],
    execution_manifest: &[u8],
    execution_checked_manifests: [&[u8]; 5],
    registry_checked_manifest: &[u8],
    rent_checked_manifest: &[u8],
) -> Result<CheckedGenesisInfrastructureV1> {
    let expected = CheckedGenesisInfrastructureV1::decode(manifest)?;
    let execution = crate::verify_checked_execution_release_set(
        execution_manifest,
        execution_checked_manifests,
    )?;
    let core_checked = CheckedReleaseV1::decode(execution_checked_manifests[0])?;
    let registry_checked = CheckedReleaseV1::decode(registry_checked_manifest)?;
    let rent_checked = CheckedReleaseV1::decode(rent_checked_manifest)?;
    let rebuilt = build_checked_genesis_infrastructure_v1(
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
