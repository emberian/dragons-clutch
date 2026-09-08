//! Typed checked evidence for the first selected Series Shadow accelerator.

use std::{
    fs,
    path::{Path, PathBuf},
};

use dclutch_core_contract::ContentId;
use dclutch_registry::release_set::ArtifactReleaseIdV1;
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1};
use dclutch_release_tool::{CheckedReleaseV1, CheckedTranslationValidationV1};
use sha2::{Digest as _, Sha256};

use crate::{
    Error, Result,
    model::{RecordPair, SuccessorPlan},
};

const MAX_EVIDENCE_BYTES: usize = 1 << 20;

/// Canonical evidence files whose identities the generated Certificate names.
pub(crate) struct SeriesShadowEvidenceFilesV1 {
    pub(crate) compiler_manifest: PathBuf,
    pub(crate) toolchain: PathBuf,
    pub(crate) translation_validation: PathBuf,
}

/// Checked accelerator and evidence material handed to the preselection owner.
pub(crate) struct CheckedSeriesShadowEvidenceV1 {
    pub(crate) accelerator_semantic_release: ContentId,
    pub(crate) accelerator_artifact: ArtifactReleaseV1,
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

/// Authenticate every off-chain and Registry-owned fact needed before Series
/// can generate its first Shadow Certificate.
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
    let checked = CheckedReleaseV1::decode(&gate.checked_build_manifest)
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
        .get("general_accelerator_artifact_release")
        .ok_or_else(|| {
            Error::new(
                "Series checked evidence requires the accelerator ArtifactRelease record pair",
            )
        })?
        .clone();
    if record.schema_id != crate::plan::hex(&ARTIFACT_RELEASE_SCHEMA_ID_V1)
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
    let artifact = ArtifactReleaseV1::decode(&body)
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
    let toolchain = read_regular(&files.toolchain, "Series toolchain")?;
    let translation_bytes = read_regular(
        &files.translation_validation,
        "Series translation validation",
    )?;
    let translation = CheckedTranslationValidationV1::decode(&translation_bytes)
        .map_err(|error| Error::new(format!("Series translation validation: {error:?}")))?
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

fn read_regular(path: &Path, label: &str) -> Result<Vec<u8>> {
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
    if bytes.is_empty() || bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(Error::new(format!("{label} width is invalid")));
    }
    Ok(bytes)
}
