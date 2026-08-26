//! Checked evidence for one V4 capability's external executable strategy.
//!
//! The five-role execution set is not the whole executable release when a
//! capability selects Shadow-AOT or admitted-AOT. This module joins the exact
//! finalized descriptor/strategy/certificate/admission records to one checked,
//! immutable accelerator deployment. It performs no RPC, publication, signing,
//! deployment, or runtime admission; Registry-owned records remain onchain
//! authority.

use dclutch_capability_program_contract::v4::{CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AuthenticatedInterpreterArtifactsV2, EXECUTION_STRATEGY_ADMISSION_BYTES_V2,
    EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
    validate_admitted_aot_v4,
};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_BYTES_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::ArtifactReleaseIdV1;

use crate::{CheckedReleaseV1, Error, Result, artifact_release_from_checked, encode_hex, sha256};

/// Canonical checked capability-execution evidence magic.
pub const CHECKED_CAPABILITY_EXECUTION_MAGIC_V1: [u8; 8] = *b"DCLTCEV1";
/// Implemented checked capability-execution evidence schema.
pub const CHECKED_CAPABILITY_EXECUTION_SCHEMA_V1: u16 = 1;
/// Fixed header before exact semantic-owner and artifact bytes.
pub const CHECKED_CAPABILITY_EXECUTION_HEADER_BYTES_V1: usize = 16;
/// Exact fixed width of one checked external capability execution bundle.
pub const CHECKED_CAPABILITY_EXECUTION_BYTES_V1: usize =
    CHECKED_CAPABILITY_EXECUTION_HEADER_BYTES_V1
        + CAPABILITY_PROGRAM_V4_BYTES
        + EXECUTION_STRATEGY_PROGRAM_BYTES_V2
        + EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2
        + EXECUTION_STRATEGY_ADMISSION_BYTES_V2
        + ARTIFACT_RELEASE_BYTES_V1
        + 32;

const SCHEMA_OFFSET: usize = 8;
const RESERVED_OFFSET: usize = 10;
const DESCRIPTOR_OFFSET: usize = CHECKED_CAPABILITY_EXECUTION_HEADER_BYTES_V1;
const STRATEGY_OFFSET: usize = DESCRIPTOR_OFFSET + CAPABILITY_PROGRAM_V4_BYTES;
const CERTIFICATE_OFFSET: usize = STRATEGY_OFFSET + EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
const ADMISSION_OFFSET: usize = CERTIFICATE_OFFSET + EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2;
const ARTIFACT_OFFSET: usize = ADMISSION_OFFSET + EXECUTION_STRATEGY_ADMISSION_BYTES_V2;
const CHECKED_RELEASE_ID_OFFSET: usize = ARTIFACT_OFFSET + ARTIFACT_RELEASE_BYTES_V1;

/// Complete offline evidence for one external capability execution program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCapabilityExecutionV1 {
    descriptor: CapabilityProgramV4,
    strategy: ExecutionStrategyProgramV2,
    certificate: ExecutionStrategyCertificateV2,
    admission: Option<ExecutionStrategyAdmissionV2>,
    artifact: ArtifactReleaseV1,
    checked_release_id: ContentId,
}

impl CheckedCapabilityExecutionV1 {
    /// Decode and revalidate one exact fixed-width capability execution bundle.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CHECKED_CAPABILITY_EXECUTION_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(CHECKED_CAPABILITY_EXECUTION_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CHECKED_CAPABILITY_EXECUTION_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(RESERVED_OFFSET..DESCRIPTOR_OFFSET)
            .is_none_or(|reserved| reserved.iter().any(|value| *value != 0))
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let descriptor = CapabilityProgramV4::decode(subslice(
            bytes,
            DESCRIPTOR_OFFSET,
            CAPABILITY_PROGRAM_V4_BYTES,
        )?)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
        let strategy = ExecutionStrategyProgramV2::decode(subslice(
            bytes,
            STRATEGY_OFFSET,
            EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
        )?)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
        let certificate = ExecutionStrategyCertificateV2::decode(subslice(
            bytes,
            CERTIFICATE_OFFSET,
            EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2,
        )?)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
        let admission_bytes = subslice(
            bytes,
            ADMISSION_OFFSET,
            EXECUTION_STRATEGY_ADMISSION_BYTES_V2,
        )?;
        let admission = match strategy.disposition() {
            StrategyDispositionV2::ShadowAot => {
                if admission_bytes.iter().any(|value| *value != 0) {
                    return Err(Error::InvalidCapabilityExecutionManifest);
                }
                None
            }
            StrategyDispositionV2::AdmittedAot => Some(
                ExecutionStrategyAdmissionV2::decode(admission_bytes)
                    .map_err(|_| Error::InvalidCapabilityExecutionManifest)?,
            ),
            StrategyDispositionV2::Interpreted => {
                return Err(Error::InvalidCapabilityExecutionManifest);
            }
        };
        let artifact =
            ArtifactReleaseV1::decode(subslice(bytes, ARTIFACT_OFFSET, ARTIFACT_RELEASE_BYTES_V1)?)
                .map_err(|_| Error::InvalidArtifactRelease)?;
        let checked_release_id = ContentId::new(read_array(bytes, CHECKED_RELEASE_ID_OFFSET)?)
            .map_err(|_| Error::ZeroIdentifier)?;
        let value = Self {
            descriptor,
            strategy,
            certificate,
            admission,
            artifact,
            checked_release_id,
        };
        value.validate()?;
        if value.encode().as_slice() != bytes {
            return Err(Error::InvalidCapabilityExecutionManifest);
        }
        Ok(value)
    }

    /// Encode the sole canonical fixed-width capability execution evidence.
    pub fn encode(self) -> [u8; CHECKED_CAPABILITY_EXECUTION_BYTES_V1] {
        let mut output = [0_u8; CHECKED_CAPABILITY_EXECUTION_BYTES_V1];
        copy(&mut output, 0, &CHECKED_CAPABILITY_EXECUTION_MAGIC_V1);
        copy(
            &mut output,
            SCHEMA_OFFSET,
            &CHECKED_CAPABILITY_EXECUTION_SCHEMA_V1.to_le_bytes(),
        );
        copy(&mut output, DESCRIPTOR_OFFSET, &self.descriptor.encode());
        copy(&mut output, STRATEGY_OFFSET, &self.strategy.to_bytes());
        copy(
            &mut output,
            CERTIFICATE_OFFSET,
            &self.certificate.to_bytes(),
        );
        if let Some(admission) = self.admission {
            copy(&mut output, ADMISSION_OFFSET, &admission.to_bytes());
        }
        copy(&mut output, ARTIFACT_OFFSET, &self.artifact.to_bytes());
        copy(
            &mut output,
            CHECKED_RELEASE_ID_OFFSET,
            self.checked_release_id.as_bytes(),
        );
        output
    }

    /// SHA-256 identity of this complete checked evidence bundle.
    pub fn checked_capability_execution_id(self) -> Result<ContentId> {
        content_id(&self.encode())
    }

    /// Exact content identity of the CapabilityProgramV4 record.
    pub fn capability_program_id(self) -> Result<ContentId> {
        content_id(&self.descriptor.encode())
    }

    /// Exact content identity of the ExecutionStrategyProgramV2 record.
    pub fn strategy_program_id(self) -> Result<ContentId> {
        content_id(&self.strategy.to_bytes())
    }

    /// Exact content identity of the ExecutionStrategyCertificateV2 record.
    pub fn certificate_program_id(self) -> Result<ContentId> {
        content_id(&self.certificate.to_bytes())
    }

    /// Optional exact Registry admission identity for admitted-AOT.
    pub fn admission_program_id(self) -> Result<Option<ContentId>> {
        self.admission
            .map(|admission| content_id(&admission.to_bytes()))
            .transpose()
    }

    /// Selected execution disposition.
    pub const fn disposition(self) -> StrategyDispositionV2 {
        self.strategy.disposition()
    }

    /// Checked immutable accelerator artifact.
    pub const fn artifact(self) -> ArtifactReleaseV1 {
        self.artifact
    }

    /// Identity of the complete accelerator checked-release manifest.
    pub const fn checked_release_id(self) -> ContentId {
        self.checked_release_id
    }

    /// Emit a deterministic inspection projection without claiming deployment.
    pub fn render_text(self) -> Result<String> {
        let mut output = String::new();
        push_line(
            &mut output,
            "format",
            "dclutch-checked-capability-execution-v1",
        );
        push_line(
            &mut output,
            "checked_capability_execution_id",
            &encode_hex(self.checked_capability_execution_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "capability_program_id",
            &encode_hex(self.capability_program_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "capability_kind_id",
            &encode_hex(self.descriptor.kind().as_bytes()),
        );
        push_line(
            &mut output,
            "strategy_program_id",
            &encode_hex(self.strategy_program_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "certificate_program_id",
            &encode_hex(self.certificate_program_id()?.as_bytes()),
        );
        push_line(
            &mut output,
            "admission_program_id",
            &self
                .admission_program_id()?
                .map_or_else(|| "none".to_owned(), |value| encode_hex(value.as_bytes())),
        );
        push_line(
            &mut output,
            "disposition",
            match self.strategy.disposition() {
                StrategyDispositionV2::ShadowAot => "shadow-aot",
                StrategyDispositionV2::AdmittedAot => "admitted-aot",
                StrategyDispositionV2::Interpreted => "interpreted",
            },
        );
        push_line(
            &mut output,
            "accelerator_program_id",
            &encode_hex(self.artifact.program().as_bytes()),
        );
        push_line(
            &mut output,
            "accelerator_artifact_release_id",
            &encode_hex(artifact_id(self.artifact)?.as_bytes()),
        );
        push_line(
            &mut output,
            "accelerator_checked_release_id",
            &encode_hex(self.checked_release_id.as_bytes()),
        );
        push_line(&mut output, "upgrade_policy", "immutable");
        push_line(
            &mut output,
            "recognition_class",
            "offline-checked-capability-evidence",
        );
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.artifact.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable {
            return Err(Error::CapabilityAcceleratorMustBeImmutable);
        }
        let descriptor_strategy = self.descriptor.strategy();
        if descriptor_strategy.schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
            || descriptor_strategy.program() != self.strategy_program_id()?
        {
            return Err(Error::InvalidCapabilityExecutionManifest);
        }
        let certificate_program = self.certificate_program_id()?;
        let strategy_program = self.strategy_program_id()?;
        let artifacts = AuthenticatedInterpreterArtifactsV2 {
            account_profile_program: self.descriptor.account_profile().program(),
            request_profile_schema: self.descriptor.request_profile().schema(),
            request_profile_program: self.descriptor.request_profile().program(),
            transition_schema: self.descriptor.transition().schema(),
            transition_program: self.descriptor.transition().program(),
            effect_program: self.descriptor.effect().program(),
        };
        let release = artifact_id(self.artifact)?;
        match self.strategy.disposition() {
            StrategyDispositionV2::ShadowAot => {
                if self.admission.is_some() || self.strategy.admission_program().is_some() {
                    return Err(Error::InvalidCapabilityExecutionManifest);
                }
                self.certificate
                    .validate_v4(
                        certificate_program,
                        strategy_program,
                        self.strategy,
                        self.descriptor,
                        artifacts,
                    )
                    .and_then(|_| self.certificate.validate_artifact(release))
                    .map_err(|_| Error::InvalidCapabilityExecutionManifest)
            }
            StrategyDispositionV2::AdmittedAot => {
                let admission = self
                    .admission
                    .ok_or(Error::InvalidCapabilityExecutionManifest)?;
                let admission_program = self
                    .admission_program_id()?
                    .ok_or(Error::InvalidCapabilityExecutionManifest)?;
                validate_admitted_aot_v4(
                    strategy_program,
                    self.strategy,
                    self.descriptor,
                    certificate_program,
                    self.certificate,
                    artifacts,
                    release,
                    Some((admission_program, admission)),
                )
                .map(|_| ())
                .map_err(|_| Error::InvalidCapabilityExecutionManifest)
            }
            StrategyDispositionV2::Interpreted => Err(Error::InvalidCapabilityExecutionManifest),
        }
    }
}

/// Build one checked capability execution bundle from independently decoded
/// semantic-owner records and complete accelerator deployment evidence.
pub fn build_checked_capability_execution_v1(
    descriptor: CapabilityProgramV4,
    strategy: ExecutionStrategyProgramV2,
    certificate: ExecutionStrategyCertificateV2,
    admission: Option<ExecutionStrategyAdmissionV2>,
    checked_release: &CheckedReleaseV1,
) -> Result<CheckedCapabilityExecutionV1> {
    let value = CheckedCapabilityExecutionV1 {
        descriptor,
        strategy,
        certificate,
        admission,
        artifact: artifact_release_from_checked(checked_release)?,
        checked_release_id: checked_release.checked_release_id()?,
    };
    value.validate()?;
    Ok(value)
}

/// Hostile-decode exact finalized semantic-owner records, then build one
/// checked capability execution bundle. `admission_bytes` must be absent for
/// Shadow-AOT and present for admitted-AOT.
pub fn build_checked_capability_execution_from_bytes_v1(
    descriptor_bytes: &[u8],
    strategy_bytes: &[u8],
    certificate_bytes: &[u8],
    admission_bytes: Option<&[u8]>,
    checked_release_manifest: &[u8],
) -> Result<CheckedCapabilityExecutionV1> {
    let descriptor = CapabilityProgramV4::decode(descriptor_bytes)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
    let strategy = ExecutionStrategyProgramV2::decode(strategy_bytes)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
    let certificate = ExecutionStrategyCertificateV2::decode(certificate_bytes)
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
    let admission = admission_bytes
        .map(ExecutionStrategyAdmissionV2::decode)
        .transpose()
        .map_err(|_| Error::InvalidCapabilityExecutionManifest)?;
    let checked = CheckedReleaseV1::decode(checked_release_manifest)?;
    build_checked_capability_execution_v1(descriptor, strategy, certificate, admission, &checked)
}

/// Verify exact accelerator build evidence against a checked capability bundle.
pub fn verify_checked_capability_execution_v1(
    manifest: &[u8],
    checked_release_manifest: &[u8],
) -> Result<CheckedCapabilityExecutionV1> {
    let expected = CheckedCapabilityExecutionV1::decode(manifest)?;
    let checked = CheckedReleaseV1::decode(checked_release_manifest)?;
    let rebuilt = build_checked_capability_execution_v1(
        expected.descriptor,
        expected.strategy,
        expected.certificate,
        expected.admission,
        &checked,
    )?;
    if rebuilt != expected {
        return Err(Error::CheckedCapabilityExecutionManifestMismatch);
    }
    Ok(expected)
}

fn artifact_id(artifact: ArtifactReleaseV1) -> Result<ArtifactReleaseIdV1> {
    ArtifactReleaseIdV1::new(sha256(&artifact.to_bytes()))
        .map_err(|_| Error::InvalidArtifactRelease)
}

fn content_id(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(sha256(bytes)).map_err(|_| Error::ZeroIdentifier)
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

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    };
    use dclutch_execution_strategy_contract::{
        shadow_v3::{SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3},
        v2::{
            ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
            EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        },
    };

    use super::*;
    use crate::SemanticPreimageKindV1;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero content id")
    }

    fn checked(seed: u8, immutable: bool) -> CheckedReleaseV1 {
        CheckedReleaseV1 {
            semantic_kind: SemanticPreimageKindV1::Capability,
            semantic_preimage_len: 16,
            elf_len: 64,
            program_account_len: 36,
            programdata_account_len: 109,
            deployment_slot: u64::from(seed),
            programdata_elf_offset: 45,
            artifact_digest: [seed.wrapping_add(4); 32],
            semantic_release_id: id(seed.wrapping_add(5)),
            program_account_digest: [seed.wrapping_add(6); 32],
            programdata_account_digest: [seed.wrapping_add(7); 32],
            program_id: [seed; 32],
            programdata_id: [seed.wrapping_add(1); 32],
            loader_program_id: [seed.wrapping_add(2); 32],
            upgrade_authority: (!immutable).then_some([seed.wrapping_add(3); 32]),
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

    fn fixture(
        disposition: StrategyDispositionV2,
    ) -> (
        CapabilityProgramV4,
        ExecutionStrategyProgramV2,
        ExecutionStrategyCertificateV2,
        Option<ExecutionStrategyAdmissionV2>,
        CheckedReleaseV1,
    ) {
        let checked = checked(31, true);
        let artifact = artifact_release_from_checked(&checked).expect("artifact");
        let artifact_release = artifact_id(artifact).expect("artifact id");
        let account_profile = id(10);
        let request_profile_schema = id(11);
        let request_profile = id(12);
        let transition_schema = id(13);
        let transition = id(14);
        let effect = id(15);
        let certificate = ExecutionStrategyCertificateV2::new(
            account_profile,
            request_profile_schema,
            request_profile,
            transition_schema,
            transition,
            effect,
            artifact_release,
            id(16),
            id(17),
            id(18),
        );
        let certificate_program = content_id(&certificate.to_bytes()).expect("certificate id");
        let admission = (disposition == StrategyDispositionV2::AdmittedAot)
            .then(|| ExecutionStrategyAdmissionV2::new(certificate_program));
        let admission_program =
            admission.map(|value| content_id(&value.to_bytes()).expect("admission id"));
        let (request_schema, ack_schema) = if disposition == StrategyDispositionV2::ShadowAot {
            (
                ContentId::new(SHADOW_REQUEST_SCHEMA_ID_V3).expect("shadow request schema"),
                ContentId::new(SHADOW_ACK_SCHEMA_ID_V3).expect("shadow ack schema"),
            )
        } else {
            (
                ContentId::new(ACCELERATOR_REQUEST_SCHEMA_ID_V2)
                    .expect("accelerator request schema"),
                ContentId::new(ACCELERATOR_ACK_SCHEMA_ID_V2).expect("accelerator ack schema"),
            )
        };
        let strategy = ExecutionStrategyProgramV2::new(
            disposition,
            transition_schema,
            transition,
            ContentId::new(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)
                .expect("certificate schema"),
            Some(certificate_program),
            ContentId::new(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2).expect("admission schema"),
            admission_program,
            request_schema,
            ack_schema,
        )
        .expect("strategy");
        let strategy_program = content_id(&strategy.to_bytes()).expect("strategy id");
        let descriptor = CapabilityProgramV4::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(id(7), account_profile),
                request_profile: ArtifactReferenceV4::new(request_profile_schema, request_profile),
                lifecycle: ArtifactReferenceV4::new(
                    ContentId::new(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)
                        .expect("lifecycle schema"),
                    id(19),
                ),
                strategy: ArtifactReferenceV4::new(
                    ContentId::new(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)
                        .expect("strategy schema"),
                    strategy_program,
                ),
                transition: ArtifactReferenceV4::new(transition_schema, transition),
                effect: ArtifactReferenceV4::new(id(20), effect),
            },
            128,
        )
        .expect("descriptor");
        (descriptor, strategy, certificate, admission, checked)
    }

    #[test]
    fn shadow_and_admitted_execution_have_exact_self_contained_evidence() {
        for disposition in [
            StrategyDispositionV2::ShadowAot,
            StrategyDispositionV2::AdmittedAot,
        ] {
            let (descriptor, strategy, certificate, admission, checked) = fixture(disposition);
            let built = build_checked_capability_execution_v1(
                descriptor,
                strategy,
                certificate,
                admission,
                &checked,
            )
            .expect("checked capability execution");
            let bytes = built.encode();
            assert_eq!(bytes.len(), CHECKED_CAPABILITY_EXECUTION_BYTES_V1);
            assert_eq!(CheckedCapabilityExecutionV1::decode(&bytes), Ok(built));
            assert_eq!(built.disposition(), disposition);
            let checked_bytes = checked.encode().expect("checked release");
            assert_eq!(
                verify_checked_capability_execution_v1(&bytes, &checked_bytes),
                Ok(built)
            );
            let text = built.render_text().expect("text");
            assert!(text.contains("recognition_class=offline-checked-capability-evidence\n"));
            assert!(text.contains("upgrade_policy=immutable\n"));
        }
    }

    #[test]
    fn semantic_record_artifact_and_checked_release_substitutions_refuse() {
        let (descriptor, strategy, certificate, admission, checked_release) =
            fixture(StrategyDispositionV2::ShadowAot);
        let built = build_checked_capability_execution_v1(
            descriptor,
            strategy,
            certificate,
            admission,
            &checked_release,
        )
        .expect("checked capability execution");
        let bytes = built.encode();

        let mut hostile = bytes;
        *hostile
            .get_mut(DESCRIPTOR_OFFSET + 432)
            .expect("strategy content byte") ^= 1;
        assert_eq!(
            CheckedCapabilityExecutionV1::decode(&hostile),
            Err(Error::InvalidCapabilityExecutionManifest)
        );

        let mut false_admission = bytes;
        *false_admission
            .get_mut(ADMISSION_OFFSET)
            .expect("shadow admission byte") = 1;
        assert_eq!(
            CheckedCapabilityExecutionV1::decode(&false_admission),
            Err(Error::InvalidCapabilityExecutionManifest)
        );

        let substitute = checked(61, true)
            .encode()
            .expect("substitute checked release");
        assert_eq!(
            verify_checked_capability_execution_v1(&bytes, &substitute),
            Err(Error::InvalidCapabilityExecutionManifest)
        );
        assert_eq!(
            CheckedCapabilityExecutionV1::decode(
                bytes
                    .get(..bytes.len().saturating_sub(1))
                    .expect("shortened manifest"),
            ),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn upgradeable_accelerator_never_becomes_checked_capability_evidence() {
        let (descriptor, strategy, certificate, admission, _) =
            fixture(StrategyDispositionV2::AdmittedAot);
        assert_eq!(
            build_checked_capability_execution_v1(
                descriptor,
                strategy,
                certificate,
                admission,
                &checked(31, false),
            ),
            Err(Error::CapabilityAcceleratorMustBeImmutable)
        );
    }
}
