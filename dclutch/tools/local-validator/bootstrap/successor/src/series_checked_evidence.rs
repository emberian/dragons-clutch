//! Typed checked evidence for the first selected Series Shadow accelerator.

use std::{
    fs,
    path::{Path, PathBuf},
};

use dclutch_core_contract::ContentId;
use dclutch_market::execution_strategy::v2::ExecutionStrategyCertificateV2;
use dclutch_registry::release_set::ArtifactReleaseIdV1;
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V2, ArtifactReleaseV2};
use dclutch_release_tool::{
    CheckedReleaseV2, CheckedSeriesTranslationV1, artifact_release_from_checked,
};
use sha2::{Digest as _, Sha256};

use crate::{
    Error, Result,
    model::{RecordPair, SuccessorPlan},
};

const MAX_EVIDENCE_BYTES: usize = 1 << 20;
const MAX_SELECTED_ELF_BYTES: usize = 64 << 20;
// The successor-plan schema has one physical accelerator deployment fact. Its
// historical General label remains the serialized key; Series reads that same
// checked ArtifactRelease rather than inventing a second plan pin.
const SHARED_ACCELERATOR_ARTIFACT_RECORD_LABEL_V1: &str = "general_accelerator_artifact_release";
const SERIES_CONSUME_SEMANTIC_SOURCE_V4: &[u8] = include_bytes!(
    "../../../../../programs/dclutch-trading-sbf/src/series/consume_artifacts_v4.rs"
);

/// Canonical evidence files whose identities the generated Certificate names.
pub(crate) struct SeriesShadowEvidenceFilesV1 {
    pub(crate) compiler_manifest: PathBuf,
    pub(crate) toolchain: PathBuf,
    pub(crate) translation_validation: PathBuf,
}

/// Checked accelerator and evidence material handed to the preselection owner.
pub(crate) struct CheckedSeriesShadowEvidenceV1 {
    pub(crate) accelerator_semantic_release: ContentId,
    pub(crate) accelerator_artifact: ArtifactReleaseV2,
    pub(crate) accelerator_record: RecordPair,
    pub(crate) checked_manifest_path: PathBuf,
    pub(crate) checked_manifest_sha256: String,
    pub(crate) compiler_manifest_path: PathBuf,
    pub(crate) compiler_manifest: Vec<u8>,
    pub(crate) toolchain_path: PathBuf,
    pub(crate) toolchain: Vec<u8>,
    pub(crate) translation_validation_path: PathBuf,
    pub(crate) translation_validation: ContentId,
}

/// Generated material whose exact digests can safely select an accelerator
/// build input.
pub(crate) struct CheckedSeriesShadowGeneratedV1 {
    pub(crate) source_manifest: ContentId,
    pub(crate) bundle: ContentId,
    pub(crate) generated_include: ContentId,
    pub(crate) certificate: ContentId,
}

/// The checked manifest and ELF of the selected accelerator rebuild.
///
/// This is intentionally an artifact fact separate from the semantic
/// Certificate binding: the selected ELF is new, but it must retain the source
/// revision and semantic release authenticated before the Certificate existed.
pub(crate) struct CheckedSelectedSeriesAcceleratorBuildV1 {
    pub(crate) checked_manifest_path: PathBuf,
    pub(crate) checked_manifest_sha256: String,
    pub(crate) artifact_sha256: String,
    pub(crate) artifact_release: ArtifactReleaseV2,
    pub(crate) artifact_release_id: ArtifactReleaseIdV1,
}

/// Authenticate every off-chain and Registry-owned fact needed before Series
/// can generate its first Shadow Certificate.
///
/// The existing `general_accelerator` plan field is the shared physical
/// accelerator deployment, not a Series-specific authority. Series binds its
/// Certificate to that record's semantic release and keeps the exact artifact
/// and deployment joins separate.
pub(crate) fn authenticate_checked_series_shadow_evidence_v1(
    plan: &SuccessorPlan,
    files: SeriesShadowEvidenceFilesV1,
) -> Result<CheckedSeriesShadowEvidenceV1> {
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(plan)?;
    let set = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("Series checked evidence requires a checked-local mutable plan")
    })?;
    let pin = plan.general_accelerator.as_ref().ok_or_else(|| {
        Error::new("Series checked evidence requires the checked accelerator Loader pair")
    })?;
    let gate = crate::upgrade::authenticate_checked_release_gate_role_for_local_v1(
        Path::new(&set.checked_release_gate_path),
        &set.checked_release_gate_sha256,
        &set.source_revision,
        &set.source_tree_sha256,
        "accelerator",
        Path::new(&pin.checked_candidate_elf_path),
    )?;
    let checked = CheckedReleaseV2::decode(&gate.checked_build_manifest)
        .map_err(|error| Error::new(format!("Series accelerator checked manifest: {error:?}")))?;
    if crate::plan::hex(&checked.artifact_digest()) != gate.raw_elf_sha256 {
        return Err(Error::new(
            "Series accelerator checked manifest ELF differs from checked gate",
        ));
    }
    let semantic = checked.semantic_release_id();
    if crate::plan::hex(semantic.as_bytes()) != pin.semantic_release_id {
        return Err(Error::new(
            "Series accelerator plan semantic release differs from checked manifest",
        ));
    }
    let record = plan
        .records
        .get(SHARED_ACCELERATOR_ARTIFACT_RECORD_LABEL_V1)
        .ok_or_else(|| {
            Error::new(
                "Series checked evidence requires the accelerator ArtifactRelease record pair",
            )
        })?
        .clone();
    if record.schema_id != crate::plan::hex(&ARTIFACT_RELEASE_SCHEMA_ID_V2)
        || record.raw.is_empty()
        || record.staging.is_empty()
    {
        return Err(Error::new(
            "Series accelerator ArtifactRelease record pair/schema is invalid",
        ));
    }
    let body = crate::runtime::decode_hex(&record.body_hex)?;
    if crate::plan::hex(&Sha256::digest(&body)) != record.content_sha256 {
        return Err(Error::new(
            "Series accelerator ArtifactRelease body hash differs from its record pair",
        ));
    }
    let artifact = ArtifactReleaseV2::decode(&body)
        .map_err(|error| Error::new(format!("Series accelerator ArtifactRelease: {error:?}")))?;
    let artifact_id = ArtifactReleaseIdV1::new(Sha256::digest(&body).into())
        .map_err(|_| Error::new("Series accelerator ArtifactRelease identity is zero"))?;
    if artifact.semantic_release_id() != semantic
        || crate::plan::hex(artifact_id.as_bytes()) != pin.artifact_release_id
    {
        return Err(Error::new(
            "Series accelerator ArtifactRelease does not match checked semantic/id",
        ));
    }
    let compiler_manifest = read_regular(&files.compiler_manifest, "Series compiler manifest")?;
    authenticate_series_compiler_source_v1(
        &compiler_manifest,
        checked.source_digest(),
        checked.source_revision(),
        &gate.source_tree_sha256,
        &gate.source_revision,
    )?;
    let toolchain = read_regular(&files.toolchain, "Series toolchain")?;
    let translation_bytes = read_regular(
        &files.translation_validation,
        "Series translation validation",
    )?;
    let checked_translation = CheckedSeriesTranslationV1::decode(&translation_bytes)
        .map_err(|error| Error::new(format!("Series Consume comparison evidence: {error:?}")))?;
    checked_translation
        .require_sources(
            SERIES_CONSUME_SEMANTIC_SOURCE_V4,
            &compiler_manifest,
            &toolchain,
        )
        .map_err(|error| Error::new(format!("Series comparison source binding: {error:?}")))?;
    let translation = checked_translation
        .translation_validation_id()
        .map_err(|error| Error::new(format!("Series translation identity: {error:?}")))?;
    Ok(CheckedSeriesShadowEvidenceV1 {
        accelerator_semantic_release: semantic,
        accelerator_artifact: artifact,
        accelerator_record: record,
        checked_manifest_path: gate.checked_build_manifest_path,
        checked_manifest_sha256: gate.checked_build_manifest_sha256,
        compiler_manifest_path: files.compiler_manifest,
        compiler_manifest,
        toolchain_path: files.toolchain,
        toolchain,
        translation_validation_path: files.translation_validation,
        translation_validation: translation,
    })
}

/// Reauthenticate generated Certificate, manifest, and include bytes before
/// the driver makes the include an accelerator build input.
pub(crate) fn authenticate_series_shadow_generated_v1(
    evidence: &CheckedSeriesShadowEvidenceV1,
    built: &dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1,
) -> Result<CheckedSeriesShadowGeneratedV1> {
    use dclutch_series_shadow_bundle_generator::{
        SeriesShadowSourceManifestV1, emit_series_shadow_generated_include_v1,
    };

    let source_manifest = content_id(&built.manifest, "Series Shadow source manifest")?;
    let generated_include = content_id(&built.generated_include, "Series Shadow include")?;
    let certificate = content_id(&built.certificate, "Series Shadow Certificate")?;
    let bundle = SeriesShadowSourceManifestV1::decode(&built.manifest)
        .map_err(|error| Error::new(format!("Series Shadow source manifest: {error:?}")))?;
    let regenerated = emit_series_shadow_generated_include_v1(&built.manifest)
        .map_err(|error| Error::new(format!("Series Shadow generated include: {error:?}")))?;
    let compiler = content_id(&evidence.compiler_manifest, "Series compiler manifest")?;
    let toolchain = content_id(&evidence.toolchain, "Series toolchain")?;
    if built.build_inputs.source_manifest != source_manifest
        || built.build_inputs.bundle != bundle.bundle_digest()
        || built.build_inputs.generated_include != generated_include
        || built.build_inputs.semantic_source
            != content_id(SERIES_CONSUME_SEMANTIC_SOURCE_V4, "Series semantic source")?
        || built.build_inputs.compiler_source != compiler
        || built.build_inputs.toolchain != toolchain
        || built.build_inputs.certificate != certificate
        || bundle.semantic_source() != built.build_inputs.semantic_source
        || bundle.compiler_source() != compiler
        || bundle.toolchain() != toolchain
        || bundle.generated_bundle().certificate_program != certificate
        || regenerated != built.generated_include
    {
        return Err(Error::new(
            "Series Shadow generated manifest/include digest or provenance differs",
        ));
    }
    authenticate_series_shadow_certificate_v1(
        &built.certificate,
        evidence.accelerator_semantic_release,
        compiler,
        toolchain,
        evidence.translation_validation,
    )?;
    Ok(CheckedSeriesShadowGeneratedV1 {
        source_manifest,
        bundle: bundle.bundle_digest(),
        generated_include,
        certificate,
    })
}

/// Authenticate the selected ELF without treating its changed artifact digest
/// as a new semantic release.
pub(crate) fn authenticate_selected_series_accelerator_build_v1(
    evidence: &CheckedSeriesShadowEvidenceV1,
    checked_manifest: &Path,
    selected_elf: &Path,
) -> Result<CheckedSelectedSeriesAcceleratorBuildV1> {
    let manifest = read_regular(checked_manifest, "selected Series checked manifest")?;
    let elf = read_regular_bounded(
        selected_elf,
        "selected Series accelerator ELF",
        MAX_SELECTED_ELF_BYTES,
    )?;
    let selected = CheckedReleaseV2::decode(&manifest)
        .map_err(|error| Error::new(format!("selected Series checked manifest: {error:?}")))?;
    let baseline = read_regular(
        &evidence.checked_manifest_path,
        "baseline Series checked manifest",
    )?;
    if crate::plan::hex(&Sha256::digest(&baseline)) != evidence.checked_manifest_sha256 {
        return Err(Error::new(
            "baseline Series checked manifest changed after authentication",
        ));
    }
    let baseline = CheckedReleaseV2::decode(&baseline)
        .map_err(|error| Error::new(format!("baseline Series checked manifest: {error:?}")))?;
    authenticate_series_selected_source_v1(
        &evidence.compiler_manifest,
        baseline.source_digest(),
        baseline.source_revision(),
        selected.source_digest(),
        selected.source_revision(),
    )?;
    let artifact_sha256 = crate::plan::hex(&Sha256::digest(&elf));
    if crate::plan::hex(&selected.artifact_digest()) != artifact_sha256
        || selected.semantic_release_id() != evidence.accelerator_semantic_release
        || selected.source_revision() != baseline.source_revision()
    {
        return Err(Error::new(
            "selected Series accelerator changed its checked artifact, semantic release, or source revision",
        ));
    }
    let artifact_release = artifact_release_from_checked(&selected)
        .map_err(|error| Error::new(format!("selected Series ArtifactRelease: {error:?}")))?;
    let artifact_release_id =
        ArtifactReleaseIdV1::new(Sha256::digest(artifact_release.to_bytes()).into())
            .map_err(|_| Error::new("selected Series ArtifactRelease identity is zero"))?;
    Ok(CheckedSelectedSeriesAcceleratorBuildV1 {
        checked_manifest_path: checked_manifest.to_path_buf(),
        checked_manifest_sha256: crate::plan::hex(&Sha256::digest(&manifest)),
        artifact_sha256,
        artifact_release,
        artifact_release_id,
    })
}

fn authenticate_series_compiler_source_v1(
    compiler_manifest: &[u8],
    checked_digest: [u8; 32],
    checked_revision: &str,
    gate_digest: &str,
    gate_revision: &str,
) -> Result<()> {
    // The candidate owner emits source-tree.txt with git ls-tree -r --full-tree.
    // Reusing that whole-tree inventory covers every compiler/native/runtime
    // file and binds this comparison to the admitted artifact's actual source.
    let digest: [u8; 32] = Sha256::digest(compiler_manifest).into();
    if digest != checked_digest
        || crate::plan::hex(&digest) != gate_digest
        || checked_revision != gate_revision
    {
        return Err(Error::new(
            "Series compiler inventory differs from checked accelerator source",
        ));
    }
    Ok(())
}

fn authenticate_series_selected_source_v1(
    compiler_manifest: &[u8],
    baseline_digest: [u8; 32],
    baseline_revision: &str,
    selected_digest: [u8; 32],
    selected_revision: &str,
) -> Result<()> {
    let digest: [u8; 32] = Sha256::digest(compiler_manifest).into();
    if baseline_digest != digest
        || selected_digest != digest
        || selected_revision != baseline_revision
    {
        return Err(Error::new(
            "selected Series accelerator source differs from checked compiler inventory",
        ));
    }
    Ok(())
}

fn authenticate_series_shadow_certificate_v1(
    bytes: &[u8],
    semantic_release: ContentId,
    compiler: ContentId,
    toolchain: ContentId,
    translation_validation: ContentId,
) -> Result<()> {
    let certificate = ExecutionStrategyCertificateV2::decode(bytes)
        .map_err(|error| Error::new(format!("Series Shadow Certificate: {error:?}")))?;
    if certificate
        .validate_semantic_release(semantic_release)
        .is_err()
        || certificate.compiler_release() != compiler
        || certificate.toolchain() != toolchain
        || certificate.translation_validation() != translation_validation
    {
        return Err(Error::new(
            "Series Shadow Certificate differs from checked semantic/compiler/toolchain/translation evidence",
        ));
    }
    Ok(())
}

fn content_id(bytes: &[u8], label: &str) -> Result<ContentId> {
    ContentId::new(Sha256::digest(bytes).into())
        .map_err(|_| Error::new(format!("{label} identity is zero")))
}

fn read_regular(path: &Path, label: &str) -> Result<Vec<u8>> {
    read_regular_bounded(path, label, MAX_EVIDENCE_BYTES)
}

fn read_regular_bounded(path: &Path, label: &str, max_bytes: usize) -> Result<Vec<u8>> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} path is not absolute")));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!("{label} is not a regular file")));
    }
    if fs::canonicalize(&path)? != path {
        return Err(Error::new(format!("{label} path is not canonical")));
    }
    let bytes = fs::read(&path)?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(Error::new(format!("{label} width is invalid")));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero identity")
    }

    #[test]
    fn rehashed_foreign_compiler_inventory_cannot_cross_the_release_gate() {
        let inventory = b"the candidate owner's complete source-tree.txt";
        let digest: [u8; 32] = Sha256::digest(inventory).into();
        let gate_digest = crate::plan::hex(&digest);
        authenticate_series_compiler_source_v1(
            inventory,
            digest,
            "revision",
            &gate_digest,
            "revision",
        )
        .expect("actual inventory joins both source pins");
        let foreign = b"unrelated inventory consistently rehashed by a caller";
        let foreign_digest: [u8; 32] = Sha256::digest(foreign).into();
        for (bytes, checked, checked_revision, gate, gate_revision) in [
            (
                foreign.as_slice(),
                digest,
                "revision",
                gate_digest.clone(),
                "revision",
            ),
            (
                foreign.as_slice(),
                foreign_digest,
                "revision",
                gate_digest.clone(),
                "revision",
            ),
            (
                inventory.as_slice(),
                foreign_digest,
                "revision",
                gate_digest.clone(),
                "revision",
            ),
            (
                inventory.as_slice(),
                digest,
                "other",
                gate_digest.clone(),
                "revision",
            ),
        ] {
            assert_eq!(
                authenticate_series_compiler_source_v1(
                    bytes,
                    checked,
                    checked_revision,
                    &gate,
                    gate_revision
                )
                .unwrap_err()
                .to_string(),
                "Series compiler inventory differs from checked accelerator source"
            );
        }
    }

    #[test]
    fn selected_source_digest_must_match_even_when_revision_is_unchanged() {
        let inventory = b"complete source-tree.txt";
        let digest: [u8; 32] = Sha256::digest(inventory).into();
        authenticate_series_selected_source_v1(inventory, digest, "revision", digest, "revision")
            .expect("same source with a separately checked selected artifact");
        for (baseline, selected, revision) in [
            ([7; 32], digest, "revision"),
            (digest, [7; 32], "revision"),
            (digest, digest, "other"),
        ] {
            assert_eq!(
                authenticate_series_selected_source_v1(
                    inventory, baseline, "revision", selected, revision
                )
                .unwrap_err()
                .to_string(),
                "selected Series accelerator source differs from checked compiler inventory"
            );
        }
    }

    #[test]
    fn certificate_refuses_each_checked_evidence_substitution() {
        let certificate = ExecutionStrategyCertificateV2::new_semantic(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
        )
        .to_bytes();
        assert!(
            authenticate_series_shadow_certificate_v1(&certificate, id(7), id(8), id(9), id(10))
                .is_ok()
        );
        for (semantic, compiler, toolchain, translation) in [
            (id(11), id(8), id(9), id(10)),
            (id(7), id(11), id(9), id(10)),
            (id(7), id(8), id(11), id(10)),
            (id(7), id(8), id(9), id(11)),
        ] {
            assert!(
                authenticate_series_shadow_certificate_v1(
                    &certificate,
                    semantic,
                    compiler,
                    toolchain,
                    translation,
                )
                .is_err()
            );
        }
    }
}
