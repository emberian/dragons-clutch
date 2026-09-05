//! Checked, decision-0012 mutable substrate for a fresh localhost validator.
//!
//! This module is deliberately not the permanent-devnet Upgrade journal.  It
//! authenticates the same fresh thirteen-link checked-release gate, then binds
//! seven disposable local Program/ProgramData pairs with one retained local
//! authority and seven exact synthetic deployment slots.  The external
//! campaign consumes the resulting plan exactly as it consumes a checked
//! devnet plan; only the provenance envelope differs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_registry::release_set::{SourceSemanticRoleV1, source_semantic_release_preimage_v1};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_tool::{
    CHECKED_MULTIPROGRAM_BYTES_V1, CheckedReleaseV1, RedeployedReleaseEvidenceV1,
    SemanticPreimageKindV1, artifact_release_from_checked, build_checked_execution_release_set,
    build_redeployed_checked_release, derive_execution_release_set,
    verify_checked_execution_release_set,
};
use dclutch_source::pyth::local_validator_release_v1;
use dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V7;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_program::rent::Rent;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as _},
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    Error, Result,
    model::{
        CheckedLocalExecutionReleaseRolePinV1, CheckedLocalExecutionReleaseSetPinV1,
        CheckedLocalMutableRolePinV1, CheckedLocalMutableSetPinV1, ProgramPin, SuccessorPlan,
    },
    plan::{
        LOCAL_PYTH_RECEIVER_ELF, LOCAL_PYTH_ROUTER_ELF, PrepareArgs, RecordPublicationV1,
        RoleDeploymentInputV1, RoleDeploymentsV1, hex, hex32, loader_programdata_bytes, pubkey,
    },
    upgrade::{authenticate_checked_release_gate_role_for_local_v1, checked_semantic_release_id},
};

pub(crate) const CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1: &str = "dclutch-checked-local-mutable-set-v1";
pub(crate) const CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1: &str =
    "dclutch-checked-local-execution-release-set-v1";
const SET_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/checked-local-mutable-set/v1";
const EXECUTION_ROLE_ORDER_V1: [&str; 5] = ["core", "claims", "trading", "resolution", "custody"];
const DIRECT_SEMANTIC_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/release/direct-compiled-controller-v1";

/// Operator-independent checked-gate facts supplied to local plan preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedLocalMutableGateInputV1 {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree_sha256: String,
    build_mode: LocalMutableBuildModeV1,
}

/// The sole diagnostic build-basis variation accepted by local preparation.
/// It is deliberately not persisted: the checked manifest remains the
/// authoritative basis, and persisted-plan authentication derives this mode
/// from that authenticated Trading manifest before rebuilding the set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalMutableBuildModeV1 {
    Ordinary,
    HotCuProfile,
}

/// Build the local provenance pin from already hostile-decoded ProgramData
/// facts.  `plan::prepare` is the sole producer: a caller cannot author any
/// role row directly.
pub(crate) fn build_checked_local_mutable_set_v1(
    gate: &CheckedLocalMutableGateInputV1,
    retained_authority: Pubkey,
    expected_execution_release_set_id: &str,
    roles: [(&str, &ProgramPin); 7],
) -> Result<CheckedLocalMutableSetPinV1> {
    if roles
        .iter()
        .map(|(role, _)| *role)
        .ne(crate::upgrade::CHECKED_ROLE_ORDER_V1)
    {
        return Err(Error::new(
            "checked local mutable roles are not in canonical decision-0012 order",
        ));
    }
    if retained_authority == Pubkey::default() || retained_authority == bpf_loader_upgradeable::ID {
        return Err(Error::new(
            "checked local mutable authority is not a distinct signer identity",
        ));
    }

    let gate_path = canonical_regular(&gate.path, "checked local release gate")?;
    let retained = retained_authority.to_string();
    let mut projected = Vec::with_capacity(crate::upgrade::CHECKED_ROLE_ORDER_V1.len());
    let mut programs = BTreeSet::new();
    let mut programdata = BTreeSet::new();
    let mut execution_checked = BTreeMap::new();
    let mut solana_cli_version = None;
    for (ordinal, (role, pin)) in roles.into_iter().enumerate() {
        let validated = authenticate_checked_release_gate_role_for_local_v1(
            &gate_path,
            &gate.sha256,
            &gate.source_revision,
            &gate.source_tree_sha256,
            role,
            Path::new(&pin.checked_candidate_elf_path),
        )?;
        if validated.gate_sha256 != gate.sha256 {
            return Err(Error::new(
                "checked local role projection changed the selected release gate",
            ));
        }
        match &solana_cli_version {
            None => solana_cli_version = Some(validated.solana_cli_version.clone()),
            Some(version) if *version == validated.solana_cli_version => {}
            Some(_) => {
                return Err(Error::new(
                    "checked local roles projected different Solana toolchain identities",
                ));
            }
        }
        let expected_semantic = checked_semantic_release_id(role, &validated.raw_elf_sha256)?;
        let expected_slot = u64::try_from(ordinal)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| Error::new("checked local deployment slot overflowed"))?;
        let expected_programdata = Pubkey::find_program_address(
            &[pubkey(&pin.program_id)?.as_ref()],
            &bpf_loader_upgradeable::ID,
        )
        .0;
        if pin.programdata_id != expected_programdata.to_string()
            || pin.checked_candidate_elf_sha256 != validated.raw_elf_sha256
            || pin.elf_sha256 != validated.raw_elf_sha256
            || pin.live_elf_sha256 != validated.raw_elf_sha256
            || pin.live_elf_padding_bytes != 0
            || pin.semantic_release_id != expected_semantic
            || pin.upgrade_authority.as_deref() != Some(retained.as_str())
            || pin.deployment_source != "observed-programdata-account"
            || pin.deployment_slot != expected_slot
        {
            return Err(Error::new(format!(
                "checked local {role} plan pin differs from its gate ELF, canonical slot, retained authority, or Loader linkage"
            )));
        }
        hex32(&pin.programdata_sha256).map_err(|_| {
            Error::new(format!(
                "checked local {role} ProgramData digest is not canonical SHA-256"
            ))
        })?;
        if !programs.insert(pin.program_id.clone())
            || !programdata.insert(pin.programdata_id.clone())
        {
            return Err(Error::new(
                "checked local mutable Program or ProgramData identities are not pairwise distinct",
            ));
        }
        if EXECUTION_ROLE_ORDER_V1.contains(&role) {
            let (checked, encoded) = build_local_checked_release_v1(
                role,
                pin,
                retained_authority,
                &gate.source_revision,
                &validated,
                gate.build_mode,
            )?;
            if execution_checked.insert(role, (checked, encoded)).is_some() {
                return Err(Error::new(
                    "checked local execution release projection duplicated a role",
                ));
            }
        }
        projected.push(CheckedLocalMutableRolePinV1 {
            role: role.into(),
            program_id: pin.program_id.clone(),
            programdata_id: pin.programdata_id.clone(),
            checked_candidate_elf_path: pin.checked_candidate_elf_path.clone(),
            checked_candidate_elf_sha256: pin.checked_candidate_elf_sha256.clone(),
            live_elf_sha256: pin.live_elf_sha256.clone(),
            programdata_account_sha256: pin.programdata_sha256.clone(),
            deployment_slot: pin.deployment_slot,
            semantic_release_id: pin.semantic_release_id.clone(),
        });
    }

    let execution_release_set = build_local_execution_release_set_pin_v1(
        expected_execution_release_set_id,
        &execution_checked,
    )?;

    let mut set = CheckedLocalMutableSetPinV1 {
        schema: CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1.into(),
        checked_release_gate_path: gate_path.display().to_string(),
        checked_release_gate_sha256: gate.sha256.clone(),
        source_revision: gate.source_revision.clone(),
        source_tree_sha256: gate.source_tree_sha256.clone(),
        solana_cli_version: solana_cli_version
            .ok_or_else(|| Error::new("checked local release gate projected no roles"))?,
        retained_upgrade_authority: retained,
        execution_release_set,
        set_sha256: String::new(),
        roles: projected,
    };
    set.set_sha256 = checked_local_set_digest_v1(&set)?;
    Ok(set)
}

fn build_local_checked_release_v1(
    role: &str,
    pin: &ProgramPin,
    retained_authority: Pubkey,
    source_revision: &str,
    validated: &crate::upgrade::CheckedLocalGateRoleV1,
    build_mode: LocalMutableBuildModeV1,
) -> Result<(CheckedReleaseV1, Vec<u8>)> {
    let basis = CheckedReleaseV1::decode(&validated.checked_build_manifest).map_err(|error| {
        Error::new(format!(
            "checked local {role} build basis is not CheckedReleaseV1: {error:?}"
        ))
    })?;
    let expected_manifest_suffix = Path::new("evidence").join(role).join("checked.bin");
    if hex(&Sha256::digest(&validated.checked_build_manifest))
        != validated.checked_build_manifest_sha256
        || hex(basis
            .checked_release_id()
            .map_err(release_error)?
            .as_bytes())
            != validated.checked_build_manifest_sha256
        || !validated
            .checked_build_manifest_path
            .ends_with(expected_manifest_suffix)
        || basis.semantic_kind() != SemanticPreimageKindV1::Unowned
        || hex(&basis.artifact_digest()) != validated.raw_elf_sha256
        || basis.loader_program_id() != bpf_loader_upgradeable::ID.to_bytes()
        || basis.deployment_slot() != 0
        || basis.upgrade_authority().is_some()
        || hex(&basis.source_digest()) != validated.source_tree_sha256
        || basis.source_revision() != validated.source_revision
        || basis.solana_version() != validated.solana_cli_version
        || basis.target_triple() != "sbpf-solana-solana"
        || !checked_build_command_matches_v1(basis.build_command(), role, build_mode)?
    {
        return Err(Error::new(format!(
            "checked local {role} build basis differs from its authenticated gate role"
        )));
    }

    let elf = fs::read(&pin.checked_candidate_elf_path)?;
    if hex(&Sha256::digest(&elf)) != pin.checked_candidate_elf_sha256
        || pin.checked_candidate_elf_sha256 != validated.raw_elf_sha256
    {
        return Err(Error::new(format!(
            "checked local {role} ELF changed after gate authentication"
        )));
    }
    let program_id = pubkey(&pin.program_id)?;
    let programdata_id = pubkey(&pin.programdata_id)?;
    let mut program = [0_u8; 36];
    program[..4].copy_from_slice(&2_u32.to_le_bytes());
    program[4..].copy_from_slice(programdata_id.as_ref());
    let programdata = loader_programdata_bytes(&elf, pin.deployment_slot, Some(retained_authority));
    if hex(&Sha256::digest(&programdata)) != pin.programdata_sha256 {
        return Err(Error::new(format!(
            "checked local {role} reconstructed ProgramData differs from its exact plan evidence"
        )));
    }
    let semantic_preimage = local_semantic_release_preimage_v1(role, &validated.raw_elf_sha256)?;
    if hex(&Sha256::digest(&semantic_preimage)) != pin.semantic_release_id {
        return Err(Error::new(format!(
            "checked local {role} semantic preimage differs from the plan's protocol owner"
        )));
    }
    let mut assumptions = vec![
        format!(
            "artifact and build facts are inherited from checked release {} in the exact checked gate",
            validated.checked_build_manifest_sha256
        ),
        "deployment slot and upgrade authority were hostile-decoded from the exact localhost Loader V3 ProgramData image".into(),
        "Program and ProgramData identities are disposable localhost coordinates, not public-cluster deployment evidence".into(),
        "semantic_kind is unowned because no first-party contract decodes a role-program release preimage".into(),
    ];
    assumptions.sort();
    let checked = build_redeployed_checked_release(RedeployedReleaseEvidenceV1 {
        build_basis: &basis,
        elf: &elf,
        semantic_preimage: &semantic_preimage,
        program_id: program_id.to_bytes(),
        programdata_id: programdata_id.to_bytes(),
        program_account_data: &program,
        programdata_account_data: &programdata,
        deployment_assumptions: &assumptions,
    })
    .map_err(release_error)?;
    let artifact = artifact_release_from_checked(&checked).map_err(release_error)?;
    if hex(&Sha256::digest(artifact.to_bytes())) != pin.artifact_release_id
        || checked.program_id() != program_id.to_bytes()
        || checked.programdata_id() != programdata_id.to_bytes()
        || checked.semantic_release_id().to_bytes() != hex32(&pin.semantic_release_id)?
        || checked.deployment_slot() != pin.deployment_slot
        || checked.upgrade_authority() != Some(retained_authority.to_bytes())
    {
        return Err(Error::new(format!(
            "checked local {role} deployment manifest differs from its ArtifactRelease or Loader pin"
        )));
    }
    let encoded = checked.encode().map_err(release_error)?;
    Ok((checked, encoded))
}

fn build_local_execution_release_set_pin_v1(
    expected_execution_release_set_id: &str,
    checked: &BTreeMap<&str, (CheckedReleaseV1, Vec<u8>)>,
) -> Result<CheckedLocalExecutionReleaseSetPinV1> {
    let get = |role| {
        checked
            .get(role)
            .ok_or_else(|| Error::new(format!("checked local execution projection omitted {role}")))
    };
    let core = get("core")?;
    let claims = get("claims")?;
    let trading = get("trading")?;
    let resolution = get("resolution")?;
    let custody = get("custody")?;
    let checked_refs = [&core.0, &claims.0, &trading.0, &resolution.0, &custody.0];
    let release_set = derive_execution_release_set(checked_refs).map_err(release_error)?;
    let complete =
        build_checked_execution_release_set(release_set, checked_refs).map_err(release_error)?;
    let execution_release_set_id = hex(complete
        .execution_release_set_id()
        .map_err(release_error)?
        .as_bytes());
    if execution_release_set_id != expected_execution_release_set_id {
        return Err(Error::new(
            "deployment-bound checked manifests derive another execution release set",
        ));
    }
    let roles = EXECUTION_ROLE_ORDER_V1
        .into_iter()
        .map(|role| {
            let (_, encoded) = get(role)?;
            let release = CheckedReleaseV1::decode(encoded).map_err(release_error)?;
            Ok(CheckedLocalExecutionReleaseRolePinV1 {
                role: role.into(),
                checked_release_id: hex(release
                    .checked_release_id()
                    .map_err(release_error)?
                    .as_bytes()),
                checked_release_base64: BASE64.encode(encoded),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let pin = CheckedLocalExecutionReleaseSetPinV1 {
        schema: CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1.into(),
        checked_execution_release_set_id: hex(complete
            .checked_execution_release_set_id()
            .map_err(release_error)?
            .as_bytes()),
        execution_release_set_id,
        checked_execution_release_set_base64: BASE64.encode(complete.encode()),
        roles,
    };
    let _ =
        authenticate_persisted_execution_release_set_v1(&pin, expected_execution_release_set_id)?;
    Ok(pin)
}

fn authenticate_persisted_execution_release_set_v1(
    pin: &CheckedLocalExecutionReleaseSetPinV1,
    expected_execution_release_set_id: &str,
) -> Result<[u8; CHECKED_MULTIPROGRAM_BYTES_V1]> {
    if pin.schema != CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1
        || pin.execution_release_set_id != expected_execution_release_set_id
        || pin.roles.len() != EXECUTION_ROLE_ORDER_V1.len()
        || pin
            .roles
            .iter()
            .map(|role| role.role.as_str())
            .ne(EXECUTION_ROLE_ORDER_V1)
    {
        return Err(Error::new(
            "persisted checked local execution release set has another schema, identity, or role order",
        ));
    }
    let bytes = BASE64
        .decode(&pin.checked_execution_release_set_base64)
        .map_err(|error| Error::new(format!("checked execution release set base64: {error}")))?;
    if BASE64.encode(&bytes) != pin.checked_execution_release_set_base64 {
        return Err(Error::new(
            "checked execution release set base64 is not canonical",
        ));
    }
    let bytes: [u8; CHECKED_MULTIPROGRAM_BYTES_V1] = bytes
        .try_into()
        .map_err(|_| Error::new("checked execution release set has the wrong width"))?;
    let mut manifests = Vec::with_capacity(EXECUTION_ROLE_ORDER_V1.len());
    for role in &pin.roles {
        let manifest = BASE64
            .decode(&role.checked_release_base64)
            .map_err(|error| {
                Error::new(format!("checked {} release base64: {error}", role.role))
            })?;
        if BASE64.encode(&manifest) != role.checked_release_base64 {
            return Err(Error::new(format!(
                "checked {} release base64 is not canonical",
                role.role
            )));
        }
        let checked = CheckedReleaseV1::decode(&manifest).map_err(release_error)?;
        if hex(checked
            .checked_release_id()
            .map_err(release_error)?
            .as_bytes())
            != role.checked_release_id
        {
            return Err(Error::new(format!(
                "checked {} release identity differs from its exact bytes",
                role.role
            )));
        }
        manifests.push(manifest);
    }
    let manifest_refs: [&[u8]; 5] = [
        &manifests[0],
        &manifests[1],
        &manifests[2],
        &manifests[3],
        &manifests[4],
    ];
    let checked =
        verify_checked_execution_release_set(&bytes, manifest_refs).map_err(release_error)?;
    if hex(checked
        .checked_execution_release_set_id()
        .map_err(release_error)?
        .as_bytes())
        != pin.checked_execution_release_set_id
        || hex(checked
            .execution_release_set_id()
            .map_err(release_error)?
            .as_bytes())
            != pin.execution_release_set_id
    {
        return Err(Error::new(
            "persisted checked local execution envelope differs from its complete manifests or identities",
        ));
    }
    Ok(bytes)
}

/// Re-authenticate and return Direct's one canonical checked execution wire.
/// The five complete deployment-bound manifests are re-decoded first; the
/// compact envelope alone is never treated as checked-release authority.
pub(crate) fn checked_execution_release_set_bytes_v1(
    plan: &SuccessorPlan,
) -> Result<[u8; CHECKED_MULTIPROGRAM_BYTES_V1]> {
    authenticate_checked_local_mutable_plan_v1(plan)?;
    let set = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("mutable localhost plan omitted checked local release-set evidence")
    })?;
    authenticate_persisted_execution_release_set_v1(
        &set.execution_release_set,
        &plan.release_set_id,
    )
}

/// The loopback substrate derives its semantic ids through the SAME owner as
/// devnet. It used to keep a parallel copy that hashed the source revision, and
/// a parallel copy of an identity rule is how two substrates come to disagree
/// about what a release is.
fn local_semantic_release_preimage_v1(role: &str, shipped_elf_sha256: &str) -> Result<Vec<u8>> {
    crate::upgrade::checked_semantic_release_preimage_v1(role, shipped_elf_sha256)
}

fn checked_build_command_v1(role: &str, build_mode: LocalMutableBuildModeV1) -> Result<String> {
    let package = match role {
        "core" => "dclutch-core-sbf",
        "claims" => "dclutch-claims-sbf",
        "trading" => "dclutch-trading-sbf",
        "resolution" => "dclutch-resolution-proof-sbf",
        "custody" => "dclutch-custody-sbf",
        _ => {
            return Err(Error::new(format!(
                "role {role:?} has no checked execution build command"
            )));
        }
    };
    if build_mode == LocalMutableBuildModeV1::HotCuProfile && role == "trading" {
        return Ok(
            "cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml --features hot-cu-profile -- --locked".into(),
        );
    }
    Ok(format!(
        "cargo build-sbf --manifest-path programs/{package}/Cargo.toml -- --locked"
    ))
}

fn checked_build_command_matches_v1(
    actual: &str,
    role: &str,
    build_mode: LocalMutableBuildModeV1,
) -> Result<bool> {
    Ok(actual == checked_build_command_v1(role, build_mode)?)
}

fn authenticated_gate_build_mode_v1(
    gate: &CheckedLocalMutableGateInputV1,
    trading: &ProgramPin,
) -> Result<LocalMutableBuildModeV1> {
    let validated = authenticate_checked_release_gate_role_for_local_v1(
        &gate.path,
        &gate.sha256,
        &gate.source_revision,
        &gate.source_tree_sha256,
        "trading",
        Path::new(&trading.checked_candidate_elf_path),
    )?;
    let basis = CheckedReleaseV1::decode(&validated.checked_build_manifest).map_err(|error| {
        Error::new(format!(
            "checked local Trading build basis is not CheckedReleaseV1: {error:?}"
        ))
    })?;
    let ordinary = checked_build_command_v1("trading", LocalMutableBuildModeV1::Ordinary)?;
    let hot_cu = checked_build_command_v1("trading", LocalMutableBuildModeV1::HotCuProfile)?;
    match basis.build_command() {
        command if command == ordinary => Ok(LocalMutableBuildModeV1::Ordinary),
        command if command == hot_cu => Ok(LocalMutableBuildModeV1::HotCuProfile),
        _ => Err(Error::new(
            "authenticated Trading build basis names neither the ordinary command nor the exact hot-cu-profile command",
        )),
    }
}

fn release_error(error: dclutch_release_tool::Error) -> Error {
    Error::new(format!("checked local release evidence: {error:?}"))
}

/// Re-authenticate a persisted local mutable plan before the campaign is
/// allowed to read any role key or connect with write permission.
pub(crate) fn authenticate_checked_local_mutable_plan_v1(plan: &SuccessorPlan) -> Result<()> {
    let set = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("mutable localhost plan omitted checked local release-set evidence")
    })?;
    if plan.checked_upgrade_set.is_some()
        || plan.record_publication != "transaction"
        || plan.core_bootstrap.release_recognition_requires_revoke
        || set.schema != CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1
    {
        return Err(Error::new(
            "checked local mutable plan mixed devnet evidence, genesis records, or immutable-Core semantics",
        ));
    }
    let authority = pubkey(&set.retained_upgrade_authority)?;
    let gate = CheckedLocalMutableGateInputV1 {
        path: PathBuf::from(&set.checked_release_gate_path),
        sha256: set.checked_release_gate_sha256.clone(),
        source_revision: set.source_revision.clone(),
        source_tree_sha256: set.source_tree_sha256.clone(),
        build_mode: LocalMutableBuildModeV1::Ordinary,
    };
    let build_mode = authenticated_gate_build_mode_v1(&gate, &plan.trading)?;
    let gate = CheckedLocalMutableGateInputV1 { build_mode, ..gate };
    let rebuilt = build_checked_local_mutable_set_v1(
        &gate,
        authority,
        &plan.release_set_id,
        [
            ("registry", &plan.registry),
            ("rent", &plan.rent_credit),
            ("custody", &plan.custody),
            ("resolution", &plan.resolution),
            ("claims", &plan.claims),
            ("trading", &plan.trading),
            ("core", &plan.core),
        ],
    )?;
    if &rebuilt != set || checked_local_set_digest_v1(set)? != set.set_sha256 {
        return Err(Error::new(
            "checked local mutable plan evidence changed after preparation",
        ));
    }
    authenticate_exact_local_account_dir_v1(plan, set, authority)
}

pub(crate) fn authenticate_exact_local_account_dir_v1(
    plan: &SuccessorPlan,
    set: &CheckedLocalMutableSetPinV1,
    authority: Pubkey,
) -> Result<()> {
    let directory = PathBuf::from(&plan.account_dir);
    if !directory.is_absolute() {
        return Err(Error::new(
            "checked local account directory must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Error::new(
            "checked local account directory must be one non-symlink directory",
        ));
    }
    if fs::canonicalize(&directory)? != directory {
        return Err(Error::new(
            "checked local account directory path is not canonical",
        ));
    }

    let mut expected_labels = BTreeSet::new();
    for role in crate::upgrade::CHECKED_ROLE_ORDER_V1 {
        let (_, loader_label) = local_role_projection(plan, role)?;
        expected_labels.insert(format!("loader.{loader_label}.program"));
        expected_labels.insert(format!("loader.{loader_label}.programdata"));
    }
    for role in ["pyth-receiver", "pyth-router"] {
        expected_labels.insert(format!("loader.{role}.program"));
        expected_labels.insert(format!("loader.{role}.programdata"));
    }
    expected_labels.insert(crate::plan::REGISTRY_SUCCESSION_BUFFER_LABEL_V1.into());
    if plan.genesis_accounts.len() != 19
        || plan
            .genesis_accounts
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_labels
    {
        return Err(Error::new(
            "checked local plan does not contain the exact eighteen Loader pair pins plus one Registry succession Buffer pin",
        ));
    }

    let expected_files = plan
        .genesis_accounts
        .values()
        .map(|pin| format!("{}.json", pin.address))
        .collect::<BTreeSet<_>>();
    let mut observed_files = BTreeSet::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::new("checked local account filename is not UTF-8"))?;
        let entry_metadata = fs::symlink_metadata(entry.path())?;
        if entry_metadata.file_type().is_symlink() || !entry_metadata.file_type().is_file() {
            return Err(Error::new(
                "checked local account directory contains a non-regular entry",
            ));
        }
        if !observed_files.insert(name) {
            return Err(Error::new(
                "checked local account directory repeated a filename",
            ));
        }
    }
    if observed_files != expected_files {
        return Err(Error::new(
            "checked local account directory has a missing, extra, or renamed account JSON",
        ));
    }

    let mut coordinates = BTreeSet::new();
    for role in &set.roles {
        let (pin, loader_label) = local_role_projection(plan, &role.role)?;
        let program_label = format!("loader.{loader_label}.program");
        let programdata_label = format!("loader.{loader_label}.programdata");
        let program_pin = plan
            .genesis_accounts
            .get(&program_label)
            .ok_or_else(|| Error::new(format!("checked local plan omitted {program_label}")))?;
        let programdata_pin = plan
            .genesis_accounts
            .get(&programdata_label)
            .ok_or_else(|| Error::new(format!("checked local plan omitted {programdata_label}")))?;
        let program = crate::plan::authenticate_cli_account_file_v1(
            &directory.join(format!("{}.json", program_pin.address)),
            program_pin,
        )?;
        let programdata = crate::plan::authenticate_cli_account_file_v1(
            &directory.join(format!("{}.json", programdata_pin.address)),
            programdata_pin,
        )?;
        let program_key = pubkey(&role.program_id)?;
        let programdata_key = pubkey(&role.programdata_id)?;
        for coordinate in [program_key, programdata_key] {
            if coordinate == Pubkey::default()
                || coordinate == system_program::ID
                || coordinate == bpf_loader_upgradeable::ID
                || !coordinates.insert(coordinate)
            {
                return Err(Error::new(
                    "checked local Loader coordinates are aliased or reserved",
                ));
            }
        }
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{} Program: {error:?}", role.role)))?;
        let programdata_view = ProgramDataV3View::parse(&programdata.data)
            .map_err(|error| Error::new(format!("{} ProgramData: {error:?}", role.role)))?;
        if program.pubkey != program_key
            || program_pin.address != role.program_id
            || program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || program.rent_epoch != 0
            || program.lamports != Rent::default().minimum_balance(program.data.len())
            || program_view.programdata() != programdata_key.to_bytes()
            || programdata.pubkey != programdata_key
            || programdata_pin.address != role.programdata_id
            || programdata.owner != bpf_loader_upgradeable::ID
            || programdata.executable
            || programdata.rent_epoch != 0
            || programdata.lamports != Rent::default().minimum_balance(programdata.data.len())
            || programdata_view.deployment_slot() != role.deployment_slot
            || programdata_view.upgrade_authority() != Some(authority.to_bytes())
            || hex(&Sha256::digest(programdata_view.elf())) != role.live_elf_sha256
            || programdata_pin.data_sha256 != role.programdata_account_sha256
            || pin.program_id != role.program_id
            || pin.programdata_id != role.programdata_id
        {
            return Err(Error::new(format!(
                "checked local {} Loader pair or on-disk account evidence changed",
                role.role
            )));
        }
    }
    let provider = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local Pyth release projection: {error:?}")))?;
    let provider = provider.release();
    for (role, program_key, programdata_key, deployment_slot, elf_sha256, elf) in [
        (
            "pyth-receiver",
            Pubkey::new_from_array(provider.receiver_program()),
            Pubkey::new_from_array(provider.receiver_programdata()),
            provider.receiver_deployment_slot(),
            provider.receiver_abi_id(),
            LOCAL_PYTH_RECEIVER_ELF,
        ),
        (
            "pyth-router",
            Pubkey::new_from_array(provider.router_program()),
            Pubkey::new_from_array(provider.router_programdata()),
            provider.router_deployment_slot(),
            provider.router_abi_id(),
            LOCAL_PYTH_ROUTER_ELF,
        ),
    ] {
        let program_label = format!("loader.{role}.program");
        let programdata_label = format!("loader.{role}.programdata");
        let program_pin = plan
            .genesis_accounts
            .get(&program_label)
            .ok_or_else(|| Error::new(format!("checked local plan omitted {program_label}")))?;
        let programdata_pin = plan
            .genesis_accounts
            .get(&programdata_label)
            .ok_or_else(|| Error::new(format!("checked local plan omitted {programdata_label}")))?;
        let program = crate::plan::authenticate_cli_account_file_v1(
            &directory.join(format!("{}.json", program_pin.address)),
            program_pin,
        )?;
        let programdata = crate::plan::authenticate_cli_account_file_v1(
            &directory.join(format!("{}.json", programdata_pin.address)),
            programdata_pin,
        )?;
        let derived_programdata =
            Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
        let expected_programdata_body = loader_programdata_bytes(elf, 0, None);
        let mut expected_program_body = [0_u8; 36];
        expected_program_body[..4].copy_from_slice(&2_u32.to_le_bytes());
        expected_program_body[4..].copy_from_slice(programdata_key.as_ref());
        let program_view = ProgramV3View::parse(&program.data)
            .map_err(|error| Error::new(format!("{role} Program: {error:?}")))?;
        let programdata_view = ProgramDataV3View::parse(&programdata.data)
            .map_err(|error| Error::new(format!("{role} ProgramData: {error:?}")))?;
        for coordinate in [program_key, programdata_key] {
            if coordinate == Pubkey::default()
                || coordinate == system_program::ID
                || coordinate == bpf_loader_upgradeable::ID
                || !coordinates.insert(coordinate)
            {
                return Err(Error::new(
                    "checked local provider Loader coordinates are aliased or reserved",
                ));
            }
        }
        if deployment_slot != 0
            || programdata_key != derived_programdata
            || hex(&Sha256::digest(elf)) != hex(&elf_sha256)
            || program.pubkey != program_key
            || program_pin.address != program_key.to_string()
            || program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || program.rent_epoch != 0
            || program.lamports != Rent::default().minimum_balance(program.data.len())
            || program.data != expected_program_body
            || program_view.programdata() != programdata_key.to_bytes()
            || programdata.pubkey != programdata_key
            || programdata_pin.address != programdata_key.to_string()
            || programdata.owner != bpf_loader_upgradeable::ID
            || programdata.executable
            || programdata.rent_epoch != 0
            || programdata.lamports != Rent::default().minimum_balance(programdata.data.len())
            || programdata.data != expected_programdata_body
            || programdata_view.deployment_slot() != 0
            || programdata_view.upgrade_authority().is_some()
            || hex(&Sha256::digest(programdata_view.elf())) != hex(&elf_sha256)
            || programdata_pin.data_sha256 != hex(&Sha256::digest(&programdata.data))
        {
            return Err(Error::new(format!(
                "checked local {role} Loader pair is not the exact immutable slot-zero fixture"
            )));
        }
    }
    let succession = plan.infrastructure_succession.as_ref().ok_or_else(|| {
        Error::new("checked local plan omitted its Registry infrastructure succession pin")
    })?;
    let buffer_pin = plan
        .genesis_accounts
        .get(crate::plan::REGISTRY_SUCCESSION_BUFFER_LABEL_V1)
        .ok_or_else(|| Error::new("checked local plan omitted its Registry succession Buffer"))?;
    let buffer = crate::plan::authenticate_cli_account_file_v1(
        &directory.join(format!("{}.json", buffer_pin.address)),
        buffer_pin,
    )?;
    let expected_buffer = crate::plan::registry_succession_buffer_address_v1(
        pubkey(&plan.registry.program_id)?,
        pubkey(&plan.core.program_id)?,
    )?;
    let metadata =
        solana_loader_v3_interface::state::UpgradeableLoaderState::size_of_buffer_metadata();
    let state: solana_loader_v3_interface::state::UpgradeableLoaderState =
        bincode::deserialize(buffer.data.get(..metadata).ok_or_else(|| {
            Error::new("Registry succession Buffer is shorter than Loader metadata")
        })?)
        .map_err(|error| Error::new(format!("decode Registry succession Buffer: {error}")))?;
    let elf = buffer
        .data
        .get(metadata..)
        .ok_or_else(|| Error::new("Registry succession Buffer omitted its ELF"))?;
    if succession.schema != crate::plan::INFRASTRUCTURE_SUCCESSION_SCHEMA_V1
        || succession.registry_upgrade_buffer != expected_buffer.to_string()
        || succession.registry_upgrade_buffer != buffer_pin.address
        || succession.registry_candidate_elf_sha256 != hex(&Sha256::digest(elf))
        || succession.registry_candidate_elf_sha256 != plan.registry.checked_candidate_elf_sha256
        || succession.predecessor_registry_artifact_release_id != plan.registry.artifact_release_id
        || succession.predecessor_rent_artifact_release_id != plan.rent_credit.artifact_release_id
        || buffer.pubkey != expected_buffer
        || buffer.owner != bpf_loader_upgradeable::ID
        || buffer.executable
        || buffer.rent_epoch != 0
        || buffer.lamports != Rent::default().minimum_balance(buffer.data.len())
        || state
            != (solana_loader_v3_interface::state::UpgradeableLoaderState::Buffer {
                authority_address: Some(authority),
            })
        || !coordinates.insert(expected_buffer)
    {
        return Err(Error::new(
            "checked local Registry succession Buffer or plan pin changed",
        ));
    }
    if coordinates.len() != 19 {
        return Err(Error::new(
            "checked local account directory does not close over nineteen distinct coordinates",
        ));
    }
    Ok(())
}

fn local_role_projection<'a>(
    plan: &'a SuccessorPlan,
    role: &str,
) -> Result<(&'a ProgramPin, &'static str)> {
    match role {
        "registry" => Ok((&plan.registry, "registry")),
        "rent" => Ok((&plan.rent_credit, "rent-credit")),
        "custody" => Ok((&plan.custody, "custody")),
        "resolution" => Ok((&plan.resolution, "resolution")),
        "claims" => Ok((&plan.claims, "claims")),
        "trading" => Ok((&plan.trading, "trading")),
        "core" => Ok((&plan.core, "core")),
        _ => Err(Error::new(format!(
            "unknown checked local Loader role {role}"
        ))),
    }
}

fn checked_local_set_digest_v1(set: &CheckedLocalMutableSetPinV1) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(SET_DIGEST_DOMAIN_V1);
    for value in [
        set.schema.as_str(),
        set.checked_release_gate_path.as_str(),
        set.checked_release_gate_sha256.as_str(),
        set.source_revision.as_str(),
        set.source_tree_sha256.as_str(),
        set.solana_cli_version.as_str(),
        set.retained_upgrade_authority.as_str(),
    ] {
        hash_text(&mut hasher, value)?;
    }
    for value in [
        set.execution_release_set.schema.as_str(),
        set.execution_release_set
            .checked_execution_release_set_id
            .as_str(),
        set.execution_release_set.execution_release_set_id.as_str(),
        set.execution_release_set
            .checked_execution_release_set_base64
            .as_str(),
    ] {
        hash_text(&mut hasher, value)?;
    }
    let execution_count = u64::try_from(set.execution_release_set.roles.len())
        .map_err(|_| Error::new("checked local execution role count overflowed u64"))?;
    hasher.update(execution_count.to_le_bytes());
    for role in &set.execution_release_set.roles {
        for value in [
            role.role.as_str(),
            role.checked_release_id.as_str(),
            role.checked_release_base64.as_str(),
        ] {
            hash_text(&mut hasher, value)?;
        }
    }
    let count = u64::try_from(set.roles.len())
        .map_err(|_| Error::new("checked local role count overflowed u64"))?;
    hasher.update(count.to_le_bytes());
    for role in &set.roles {
        for value in [
            role.role.as_str(),
            role.program_id.as_str(),
            role.programdata_id.as_str(),
            role.checked_candidate_elf_path.as_str(),
            role.checked_candidate_elf_sha256.as_str(),
            role.live_elf_sha256.as_str(),
            role.programdata_account_sha256.as_str(),
            role.semantic_release_id.as_str(),
        ] {
            hash_text(&mut hasher, value)?;
        }
        hasher.update(role.deployment_slot.to_le_bytes());
    }
    Ok(hex(&hasher.finalize()))
}

fn hash_text(hasher: &mut Sha256, value: &str) -> Result<()> {
    let length = u64::try_from(value.len())
        .map_err(|_| Error::new("checked local evidence text exceeded u64"))?;
    hasher.update(length.to_le_bytes());
    hasher.update(value.as_bytes());
    Ok(())
}

fn canonical_regular(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} path must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "{label} must be one regular non-symlink file"
        )));
    }
    Ok(fs::canonicalize(path)?)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalMutablePrepareReportV1 {
    pub(crate) schema: String,
    pub(crate) plan: String,
    pub(crate) account_dir: String,
    pub(crate) checked_local_mutable_set_sha256: String,
    pub(crate) retained_upgrade_authority: String,
    pub(crate) programs: BTreeMap<String, String>,
    pub(crate) keypairs: BTreeMap<String, String>,
    /// Exact campaign flag surface, projected from the campaign role owner.
    /// The lifecycle supervisor must not grow its own stale copy of this list.
    pub(crate) campaign_keypairs: BTreeMap<String, String>,
    /// Administration-only keypair flags. The local supervisor consumes this
    /// exact projection rather than filtering the union above itself.
    pub(crate) campaign_administration_keypairs: BTreeMap<String, String>,
    /// Founding-only local-lifecycle keypair flags, including the exact two
    /// authenticated participant-fixture roles used by the private run.
    pub(crate) campaign_founding_keypairs: BTreeMap<String, String>,
    /// Public-only founding identities. Their secret material is never exposed
    /// through the campaign keypair surface.
    pub(crate) campaign_public_identities: BTreeMap<String, String>,
}

const LOCAL_ID_DOMAIN_V1: &[u8] = b"dclutch/private-validator-lifecycle/program-id/v1";
const LOCAL_KEY_DOMAIN_V1: &[u8] = b"dclutch/private-validator-lifecycle/keypair/v1";
const EXTRA_KEY_ROLES_V1: [&str; 9] = [
    crate::seed::role::FOUNDING_FOUNDER,
    "participant",
    "direct-seller",
    "direct-buyer",
    "pyth-encoded-vaa",
    "pyth-update-account",
    "resolver",
    "payout-owner",
    "retirement-beneficiary",
];

/// Every disposable local signer identity, deduplicated at the one boundary
/// where the campaign and lifecycle role surfaces deliberately overlap.
fn local_key_roles_v1() -> BTreeSet<&'static str> {
    crate::campaign::KEYPAIR_ROLES
        .iter()
        .copied()
        .chain(EXTRA_KEY_ROLES_V1)
        .collect()
}

fn local_campaign_public_identities_v1(seed: [u8; 32]) -> Result<BTreeMap<String, String>> {
    let founder = Keypair::new_from_array(derive(
        LOCAL_KEY_DOMAIN_V1,
        seed,
        crate::seed::role::FOUNDING_FOUNDER,
    ))
    .pubkey();
    let substituted_founder = Keypair::new_from_array(derive(
        LOCAL_KEY_DOMAIN_V1,
        seed,
        crate::seed::role::SUBSTITUTED_FOUNDER,
    ))
    .pubkey();
    if founder == substituted_founder {
        return Err(Error::new(
            "local founding and substituted-founder public identities aliased",
        ));
    }
    Ok(BTreeMap::from([
        (
            crate::seed::role::FOUNDING_FOUNDER.into(),
            founder.to_string(),
        ),
        (
            crate::seed::role::SUBSTITUTED_FOUNDER.into(),
            substituted_founder.to_string(),
        ),
    ]))
}

fn project_keypair_roles_v1(
    keypairs: &BTreeMap<String, String>,
    roles: &[&str],
    label: &str,
) -> Result<BTreeMap<String, String>> {
    roles
        .iter()
        .map(|role| {
            keypairs
                .get(*role)
                .cloned()
                .map(|path| ((*role).into(), path))
                .ok_or_else(|| Error::new(format!("local derivation omitted {label} role {role}")))
        })
        .collect()
}

/// Prepare one exact checked mutable plan and its disposable role key files.
/// The command is deliberately offline: no validator exists yet and no RPC
/// origin is accepted as an argument.
pub(crate) fn run_prepare(arguments: Vec<String>) -> Result<()> {
    let report = prepare_local_mutable_v1(arguments)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn prepare_local_mutable_v1(
    arguments: Vec<String>,
) -> Result<LocalMutablePrepareReportV1> {
    let arguments = parse_prepare_arguments_v1(arguments)?;
    let work = absolute_new_directory(required(arguments.work, "--work")?, "--work")?;
    let output = PathBuf::from(required(arguments.output, "--output")?);
    if !output.is_absolute() || output.exists() || fs::symlink_metadata(&output).is_ok() {
        return Err(Error::new(
            "--output must be an absolute path that does not exist",
        ));
    }
    if output.parent() != Some(work.as_path()) && output.parent() != work.parent() {
        return Err(Error::new(
            "--output must be a direct child of --work or its existing parent",
        ));
    }
    let gate = CheckedLocalMutableGateInputV1 {
        path: PathBuf::from(required(arguments.gate_path, "--checked-release-gate")?),
        sha256: required(
            arguments.gate_sha256,
            "--expected-checked-release-gate-sha256",
        )?,
        source_revision: required(arguments.source_revision, "--expected-source-revision")?,
        source_tree_sha256: required(
            arguments.source_tree_sha256,
            "--expected-source-tree-sha256",
        )?,
        build_mode: arguments.build_mode,
    };
    hex32(&gate.sha256)
        .map_err(|_| Error::new("--expected-checked-release-gate-sha256 is not SHA-256"))?;
    if gate.source_revision.len() != 40
        || !gate
            .source_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(
            "--expected-source-revision is not 40 lowercase hex",
        ));
    }
    hex32(&gate.source_tree_sha256)
        .map_err(|_| Error::new("--expected-source-tree-sha256 is not SHA-256"))?;
    let seed = hex32(&required(arguments.seed, "--seed")?)
        .map_err(|_| Error::new("--seed must be exactly 64 lowercase hex characters"))?;

    prepare_local_mutable_parsed_v1(work, output, gate, seed)
}

struct ParsedPrepareArgumentsV1 {
    work: Option<String>,
    output: Option<String>,
    gate_path: Option<String>,
    gate_sha256: Option<String>,
    source_revision: Option<String>,
    source_tree_sha256: Option<String>,
    seed: Option<String>,
    build_mode: LocalMutableBuildModeV1,
}

fn parse_prepare_arguments_v1(arguments: Vec<String>) -> Result<ParsedPrepareArgumentsV1> {
    let mut work = None;
    let mut output = None;
    let mut gate_path = None;
    let mut gate_sha256 = None;
    let mut source_revision = None;
    let mut source_tree_sha256 = None;
    let mut seed = None;
    let mut build_mode = LocalMutableBuildModeV1::Ordinary;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--hot-cu-profile" {
            if build_mode == LocalMutableBuildModeV1::HotCuProfile {
                return Err(Error::new("--hot-cu-profile may be supplied only once"));
            }
            build_mode = LocalMutableBuildModeV1::HotCuProfile;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--work" => &mut work,
            "--output" => &mut output,
            "--checked-release-gate" => &mut gate_path,
            "--expected-checked-release-gate-sha256" => &mut gate_sha256,
            "--expected-source-revision" => &mut source_revision,
            "--expected-source-tree-sha256" => &mut source_tree_sha256,
            "--seed" => &mut seed,
            _ => {
                return Err(Error::new(format!(
                    "unknown local-mutable-prepare-v1 argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    Ok(ParsedPrepareArgumentsV1 {
        work,
        output,
        gate_path,
        gate_sha256,
        source_revision,
        source_tree_sha256,
        seed,
        build_mode,
    })
}

fn prepare_local_mutable_parsed_v1(
    work: PathBuf,
    output: PathBuf,
    gate: CheckedLocalMutableGateInputV1,
    seed: [u8; 32],
) -> Result<LocalMutablePrepareReportV1> {
    fs::create_dir(&work)?;
    let work = fs::canonicalize(&work)?;
    let output_name = output
        .file_name()
        .ok_or_else(|| Error::new("--output omitted its filename"))?;
    let output_parent = output
        .parent()
        .ok_or_else(|| Error::new("--output omitted its parent"))?;
    let output = fs::canonicalize(output_parent)?.join(output_name);
    let key_dir = work.join("keys");
    let programdata_dir = work.join("programdata");
    fs::create_dir(&key_dir)?;
    fs::create_dir(&programdata_dir)?;
    let account_dir = work.join("accounts");
    let gate_root = canonical_regular(&gate.path, "checked local release gate")?
        .parent()
        .ok_or_else(|| Error::new("checked local release gate has no root"))?
        .to_path_buf();

    let mut keypairs = BTreeMap::new();
    for role in local_key_roles_v1() {
        let secret = derive(LOCAL_KEY_DOMAIN_V1, seed, role);
        let keypair = Keypair::new_from_array(secret);
        let path = key_dir.join(format!("{role}.json"));
        write_keypair_create_new(&path, &keypair)?;
        keypairs.insert(role.into(), path.display().to_string());
    }
    let authority_path = keypairs
        .get(crate::seed::role::CORE_UPGRADE_AUTHORITY)
        .ok_or_else(|| Error::new("local role derivation omitted the retained authority"))?;
    let authority_secret = crate::campaign::read_keypair_file(
        Path::new(authority_path),
        crate::seed::role::CORE_UPGRADE_AUTHORITY,
    )?;
    let authority = Keypair::new_from_array(authority_secret).pubkey();
    let founder_path = keypairs
        .get(crate::seed::role::FOUNDING_FOUNDER)
        .ok_or_else(|| Error::new("local role derivation omitted the founding founder"))?;
    let founder_secret = crate::campaign::read_keypair_file(
        Path::new(founder_path),
        crate::seed::role::FOUNDING_FOUNDER,
    )?;
    let founder = Keypair::new_from_array(founder_secret).pubkey();
    let campaign_public_identities = local_campaign_public_identities_v1(seed)?;
    if campaign_public_identities
        .get(crate::seed::role::FOUNDING_FOUNDER)
        .map(String::as_str)
        != Some(founder.to_string().as_str())
    {
        return Err(Error::new(
            "local founding public identity changed from its retained signer",
        ));
    }

    let mut programs = BTreeMap::new();
    let mut artifacts = BTreeMap::new();
    let mut deployments = RoleDeploymentsV1::default();
    for (ordinal, role) in crate::upgrade::CHECKED_ROLE_ORDER_V1
        .into_iter()
        .enumerate()
    {
        let program = Pubkey::new_from_array(derive(LOCAL_ID_DOMAIN_V1, seed, role));
        programs.insert(role.into(), program.to_string());
        let elf = gate_root.join(format!("elf/{role}.so"));
        let validated = authenticate_checked_release_gate_role_for_local_v1(
            &gate.path,
            &gate.sha256,
            &gate.source_revision,
            &gate.source_tree_sha256,
            role,
            &elf,
        )?;
        let bytes = fs::read(&elf)?;
        let slot = u64::try_from(ordinal)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| Error::new("local deployment slot overflowed"))?;
        let programdata = loader_programdata_bytes(&bytes, slot, Some(authority));
        let programdata_path = programdata_dir.join(format!("{role}.bin"));
        write_bytes_create_new(&programdata_path, &programdata, 0o600)?;
        let deployment = RoleDeploymentInputV1 {
            observed_programdata: Some(programdata_path),
            observed_programdata_bytes: None,
            expected_live_elf_sha256: Some(validated.raw_elf_sha256.clone()),
            genesis_deployment_slot: 0,
            expected_upgrade_authority: Some(authority),
        };
        match role {
            "registry" => deployments.registry = deployment,
            "rent" => deployments.rent_credit = deployment,
            "custody" => deployments.custody = deployment,
            "resolution" => deployments.resolution = deployment,
            "claims" => deployments.claims = deployment,
            "trading" => deployments.trading = deployment,
            "core" => deployments.core = deployment,
            _ => return Err(Error::new("canonical local role order changed")),
        }
        artifacts.insert(role, (program, elf, validated.raw_elf_sha256));
    }

    let artifact = |role: &'static str| -> Result<(Pubkey, PathBuf, String, String)> {
        let (program, elf, sha256) = artifacts
            .get(role)
            .ok_or_else(|| Error::new(format!("checked gate omitted local {role} artifact")))?;
        Ok((
            *program,
            elf.clone(),
            sha256.clone(),
            checked_semantic_release_id(role, &sha256)?,
        ))
    };
    let (registry_program, registry_elf, registry_sha256, registry_semantic_release_id) =
        artifact("registry")?;
    let (rent_credit_program, rent_credit_elf, rent_credit_sha256, rent_credit_semantic_release_id) =
        artifact("rent")?;
    let (custody_program, custody_elf, custody_sha256, custody_semantic_release_id) =
        artifact("custody")?;
    let (resolution_program, resolution_elf, resolution_sha256, resolution_semantic_release_id) =
        artifact("resolution")?;
    let (claims_program, claims_elf, claims_sha256, claims_semantic_release_id) =
        artifact("claims")?;
    let (trading_program, trading_elf, trading_sha256, trading_semantic_release_id) =
        artifact("trading")?;
    let (core_program, core_elf, core_sha256, core_semantic_release_id) = artifact("core")?;

    let prepared = crate::plan::prepare_checked_local_mutable(
        PrepareArgs {
            // The local-mutable plan fabricates its own genesis install, so
            // there is no observed account whose authority could be declared.
            observed_upgrade_authority: None,
            account_dir: account_dir.clone(),
            plan_path: output.clone(),
            registry_program,
            registry_elf,
            registry_sha256,
            registry_semantic_release_id,
            core_program,
            core_elf,
            core_sha256,
            core_semantic_release_id,
            core_bootstrap_upgrade_authority: authority,
            claims_program,
            claims_elf,
            claims_sha256,
            claims_semantic_release_id,
            trading_program,
            trading_elf,
            trading_sha256,
            trading_semantic_release_id,
            resolution_program,
            resolution_elf,
            resolution_sha256,
            resolution_semantic_release_id,
            custody_program,
            custody_elf,
            custody_sha256,
            custody_semantic_release_id,
            record_publication: RecordPublicationV1::Transaction,
            deployments,
            rent_credit_program,
            rent_credit_elf,
            rent_credit_sha256,
            rent_credit_semantic_release_id,
            checked_upgrade_set: None,
            general_accelerator: None,
        },
        &gate,
    )?;
    authenticate_checked_local_mutable_plan_v1(&prepared)?;
    let pin = prepared
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("local plan preparation omitted its checked set"))?;
    let report = LocalMutablePrepareReportV1 {
        schema: "dclutch-local-mutable-prepare-report-v1".into(),
        plan: output.display().to_string(),
        account_dir: account_dir.display().to_string(),
        checked_local_mutable_set_sha256: pin.set_sha256.clone(),
        retained_upgrade_authority: authority.to_string(),
        programs,
        campaign_keypairs: project_keypair_roles_v1(
            &keypairs,
            crate::campaign::KEYPAIR_ROLES,
            "campaign",
        )?,
        campaign_administration_keypairs: project_keypair_roles_v1(
            &keypairs,
            crate::campaign::ADMIN_ALLOWED_ROLES,
            "administration campaign",
        )?,
        campaign_founding_keypairs: project_keypair_roles_v1(
            &keypairs,
            &crate::campaign::FOUNDING_REQUIRED_ROLES
                .iter()
                .copied()
                .chain([
                    crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
                    crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
                ])
                .collect::<Vec<_>>(),
            "founding campaign",
        )?,
        campaign_public_identities,
        keypairs,
    };
    Ok(report)
}

pub(crate) fn run_authenticate(arguments: Vec<String>) -> Result<()> {
    if arguments.len() != 2 || arguments.first().map(String::as_str) != Some("--plan") {
        return Err(Error::new(
            "Usage: local-mutable-plan-authenticate-v1 --plan ABSOLUTE_JSON",
        ));
    }
    let path = PathBuf::from(
        arguments
            .get(1)
            .ok_or_else(|| Error::new("--plan is required"))?,
    );
    let path = canonical_regular(&path, "checked local mutable plan")?;
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&path)?)?;
    authenticate_checked_local_mutable_plan_v1(&plan)?;
    let set = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("checked local plan pin disappeared"))?;
    println!(
        "{{\"schema\":\"dclutch-local-mutable-plan-authentication-v1\",\"plan\":{},\"set_sha256\":\"{}\"}}",
        serde_json::to_string(&path.display().to_string())?,
        set.set_sha256
    );
    Ok(())
}

/// Compile the canonical private-validator market against the exact live
/// checked-mutable substrate. This is a read-only loopback command; the fee
/// recipient must itself be one disposable local key file.
pub(crate) fn run_market(arguments: Vec<String>) -> Result<()> {
    let mut plan_path = None;
    let mut rpc_url = None;
    let mut fee_basis_points = None;
    let mut fee_recipient_keypair = None;
    // The shape knobs. Absent means "the value this fixture has always
    // emitted", so every command line written before they existed compiles the
    // same market it always did -- and a caller who wants a different width
    // says so rather than editing a constant.
    let mut cuts_raw = None;
    let mut cut_denominator_raw = None;
    let mut coefficients_raw = None;
    let mut initial_collateral_raw = None;
    let mut window_width_raw = None;
    let mut generation_raw = None;
    let mut recovery_rungs_raw = None;
    let mut terminal_max_age_raw = None;
    let mut band_anchor_raw = None;
    let mut band_volatility_raw = None;
    let mut band_window_slots_raw = None;
    let mut band_half_widths_raw = None;
    let mut band_max_cell_share_raw = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--plan" => &mut plan_path,
            "--rpc-url" => &mut rpc_url,
            "--fee-basis-points" => &mut fee_basis_points,
            "--fee-recipient-keypair" => &mut fee_recipient_keypair,
            "--cuts" => &mut cuts_raw,
            "--cut-denominator" => &mut cut_denominator_raw,
            "--coefficients" => &mut coefficients_raw,
            "--initial-collateral-atoms" => &mut initial_collateral_raw,
            "--terminal-window-width-seconds" => &mut window_width_raw,
            "--generation" => &mut generation_raw,
            "--recovery-rungs" => &mut recovery_rungs_raw,
            "--terminal-max-age-seconds" => &mut terminal_max_age_raw,
            "--band-anchor" => &mut band_anchor_raw,
            "--band-volatility-bps" => &mut band_volatility_raw,
            "--band-window-slots" => &mut band_window_slots_raw,
            "--band-plausible-half-widths" => &mut band_half_widths_raw,
            "--band-max-cell-share-bps" => &mut band_max_cell_share_raw,
            _ => {
                return Err(Error::new(format!(
                    "unknown local-private-validator-market-v1 argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let plan_path = canonical_regular(
        &PathBuf::from(required(plan_path, "--plan")?),
        "checked local mutable plan",
    )?;
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&plan_path)?)?;
    authenticate_checked_local_mutable_plan_v1(&plan)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let fee_basis_points = required(fee_basis_points, "--fee-basis-points")?
        .parse::<u16>()
        .map_err(|_| Error::new("--fee-basis-points must be a decimal u16"))?;
    let recipient_path = canonical_regular(
        &PathBuf::from(required(fee_recipient_keypair, "--fee-recipient-keypair")?),
        "local fee-recipient keypair",
    )?;
    let recipient_secret =
        crate::campaign::read_keypair_file(&recipient_path, "retirement-beneficiary")?;
    let recipient = Keypair::new_from_array(recipient_secret).pubkey();
    let rpc_url = required(rpc_url, "--rpc-url")?;
    // ALL FIVE OR NONE. A partial band is refused by the name of the field
    // left out rather than completed from a default, because the missing
    // number is exactly the author's belief and nobody else can supply it.
    let band = founding_band_from_arguments_v1(
        band_anchor_raw,
        band_volatility_raw,
        band_window_slots_raw,
        band_half_widths_raw,
        band_max_cell_share_raw,
    )?;
    let shape = market_shape_from_arguments_v1(
        cuts_raw,
        cut_denominator_raw,
        coefficients_raw,
        initial_collateral_raw,
        window_width_raw,
        generation_raw,
        recovery_rungs_raw,
        terminal_max_age_raw,
        band,
    )?;
    // Capability selection follows the split-founding precedent: an
    // environment toggle rather than a new required flag, so every existing
    // caller keeps compiling the Direct-selected demo market unchanged.
    let market = match std::env::var("DCLUTCH_MARKET_CAPABILITY") {
        Ok(family) if family == "general" => crate::general_market::demo_general_market_input(
            &plan_path, &rpc_url, registry, recipient, &shape,
        )?,
        // Rational needs one fact General does not: its config record binds
        // the immutable Realm, and the Realm is RealmV1 over the collateral
        // Mint. That is an ORDERING constraint rather than a fixed point --
        // mint -> realm -> config -> manifest -> market runs strictly one way,
        // and the Realm is itself a Market-PDA seed -- but it does mean the
        // Mint must be chosen BEFORE this input is compiled, while the local
        // founding pipeline creates it from the run's own key forge partway
        // through. Until the founding driver can be told which Mint to use,
        // the Mint is named here and the caller is responsible for founding
        // over that exact one.
        Ok(family) if family == "rational" => {
            let mint = std::env::var("DCLUTCH_RATIONAL_COLLATERAL_MINT").map_err(|_| {
                Error::new(
                    "DCLUTCH_MARKET_CAPABILITY=rational also requires \
                     DCLUTCH_RATIONAL_COLLATERAL_MINT=BASE58_MINT. Rational's config record \
                     binds the immutable Realm, and the Realm is derived from the collateral \
                     Mint, so the Mint must be chosen before the capability closure is \
                     compiled. This is an ordering constraint, not a cycle: the Realm is a \
                     seed of the Market PDA, never an output of it.",
                )
            })?;
            let mint = mint
                .parse::<solana_sdk::pubkey::Pubkey>()
                .map_err(|_| Error::new("DCLUTCH_RATIONAL_COLLATERAL_MINT must be base58"))?;
            crate::rational_market::demo_rational_market_input(
                &plan_path, &rpc_url, registry, mint, &shape,
            )?
        }
        // Structured INHERITS Rational's config type -- TokenBehaviorSelectionV2
        // -- and therefore inherits the same Mint-before-closure ordering,
        // for the same reason and with the same one-way dependency.
        Ok(family) if family == "structured" => {
            let mint = std::env::var("DCLUTCH_STRUCTURED_COLLATERAL_MINT").map_err(|_| {
                Error::new(
                    "DCLUTCH_MARKET_CAPABILITY=structured also requires \
                     DCLUTCH_STRUCTURED_COLLATERAL_MINT=BASE58_MINT. Structured's config \
                     record is a TokenBehaviorSelectionV2, exactly as Rational's is, so it \
                     binds the immutable Realm and the Realm is derived from the collateral \
                     Mint. The Mint must be chosen before the capability closure is compiled. \
                     This is an ordering constraint, not a cycle: the Realm is a seed of the \
                     Market PDA, never an output of it.",
                )
            })?;
            let mint = mint
                .parse::<solana_sdk::pubkey::Pubkey>()
                .map_err(|_| Error::new("DCLUTCH_STRUCTURED_COLLATERAL_MINT must be base58"))?;
            crate::structured_market::demo_structured_market_input(
                &plan_path, &rpc_url, registry, mint, &shape,
            )?
        }
        Ok(family) if family != "direct" => {
            return Err(Error::new(format!(
                "DCLUTCH_MARKET_CAPABILITY={family} names no selectable capability compiler; \
                 this command compiles direct (default), general, rational or structured"
            )));
        }
        _ => {
            let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
                &plan_path,
                &rpc_url,
                registry,
                Some(fee_basis_points),
                Some(recipient),
            )?;
            crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?
        }
    };
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &market)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Turn the six optional shape flags into a [`LocalMarketShapeV1`].
///
/// Every one of them defaults to the value this fixture has always emitted, so
/// omitting all six is byte-for-byte the market the command compiled before
/// they existed. The pair rule is the one that matters: `--cuts` decides the
/// market's WIDTH and `--coefficients` must then have `cuts + 2` entries, so
/// passing one without the other is refused rather than silently padded --
/// a padded payout vector is a market whose caption and payoff disagree.
/// Assemble the author's founding band from its five flags, or nothing.
///
/// All five or none. A partial band refuses by the name of the field left out:
/// the missing number is the author's own belief about the outcome, and a
/// default for it would be this function inventing that belief silently, which
/// is the single failure the partition-quality gate exists to prevent.
fn founding_band_from_arguments_v1(
    anchor: Option<String>,
    volatility_bps: Option<String>,
    window_slots: Option<String>,
    plausible_half_widths: Option<String>,
    max_cell_share_bps: Option<String>,
) -> Result<Option<crate::model::FoundingBandInputV1>> {
    let named = [
        ("--band-anchor", anchor.is_some()),
        ("--band-volatility-bps", volatility_bps.is_some()),
        ("--band-window-slots", window_slots.is_some()),
        (
            "--band-plausible-half-widths",
            plausible_half_widths.is_some(),
        ),
        ("--band-max-cell-share-bps", max_cell_share_bps.is_some()),
    ];
    let present = named.iter().filter(|(_, given)| *given).count();
    if present == 0 {
        return Ok(None);
    }
    if present != named.len() {
        let missing: Vec<&str> = named
            .iter()
            .filter(|(_, given)| !*given)
            .map(|(flag, _)| *flag)
            .collect();
        return Err(Error::new(format!(
            "an incomplete founding band was stated; missing {}. All five are \
             required together: the band is the author's belief about the \
             outcome and no part of it has a default",
            missing.join(", ")
        )));
    }
    let number = |raw: Option<String>, flag: &str| -> Result<String> {
        raw.ok_or_else(|| Error::new(format!("{flag} is required")))
    };
    let anchor = number(anchor, "--band-anchor")?
        .parse::<i128>()
        .map_err(|_| Error::new("--band-anchor must be a decimal i128"))?;
    let volatility_bps = number(volatility_bps, "--band-volatility-bps")?
        .parse::<u32>()
        .map_err(|_| Error::new("--band-volatility-bps must be a decimal u32"))?;
    let window_slots = number(window_slots, "--band-window-slots")?
        .parse::<u64>()
        .map_err(|_| Error::new("--band-window-slots must be a decimal u64"))?;
    let plausible_half_widths = number(plausible_half_widths, "--band-plausible-half-widths")?
        .parse::<u32>()
        .map_err(|_| Error::new("--band-plausible-half-widths must be a decimal u32"))?;
    let max_cell_share_bps = number(max_cell_share_bps, "--band-max-cell-share-bps")?
        .parse::<u32>()
        .map_err(|_| Error::new("--band-max-cell-share-bps must be a decimal u32"))?;
    Ok(Some(crate::model::FoundingBandInputV1::spot_band(
        anchor,
        volatility_bps,
        window_slots,
        plausible_half_widths,
        max_cell_share_bps,
    )))
}

fn market_shape_from_arguments_v1(
    cuts: Option<String>,
    cut_denominator: Option<String>,
    coefficients: Option<String>,
    initial_collateral_atoms: Option<String>,
    terminal_window_width_seconds: Option<String>,
    generation: Option<String>,
    recovery_rungs: Option<String>,
    terminal_max_age_seconds: Option<String>,
    band: Option<crate::model::FoundingBandInputV1>,
) -> Result<crate::market::LocalMarketShapeV1> {
    let default = crate::market::LocalMarketShapeV1::default();
    let parse_list = |raw: &str| -> Vec<String> {
        raw.split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect()
    };
    // `--cuts ""` is the two-outcome market -- the whole coordinate domain as
    // one region plus the explicit failure outcome -- and is the only way this
    // compiler reaches a two-cell market at all, so an empty list is a value
    // here rather than a mistake.
    let cuts = match &cuts {
        None => default.cuts.clone(),
        Some(raw) => parse_list(raw)
            .into_iter()
            .map(|part| {
                part.parse::<i128>()
                    .map_err(|_| Error::new("--cuts must be a comma-separated list of i128"))
            })
            .collect::<Result<Vec<_>>>()?,
    };
    let coefficients = match &coefficients {
        None => default.coefficients.clone(),
        Some(raw) => parse_list(raw)
            .into_iter()
            .map(|part| {
                part.parse::<u64>()
                    .map_err(|_| Error::new("--coefficients must be a comma-separated list of u64"))
            })
            .collect::<Result<Vec<_>>>()?,
    };
    let scalar = |value: Option<String>, label: &str, fallback: u64| -> Result<u64> {
        match value {
            None => Ok(fallback),
            Some(raw) => raw
                .parse::<u64>()
                .map_err(|_| Error::new(format!("{label} must be a decimal u64"))),
        }
    };
    // THE ABSENT RESERVE IS DERIVED FROM THIS MARKET'S OWN WIDTH, not carried
    // over from the default shape's. `default.initial_collateral_atoms` is the
    // reserve the four-outcome default founds with; a caller who states five
    // cuts and no reserve founds a seven-outcome market at payout scale 6, and
    // the default's number is not a multiple of that. Stating the reserve
    // still means stating it: `--initial-collateral-atoms` is never rounded
    // here, and an indivisible one is held to the founding guard by name.
    let derived_reserve_atoms = {
        let basis_width = u32::try_from(coefficients.len())
            .map_err(|_| Error::new("Product outcome width overflow"))?;
        let payout_scale = crate::market::categorical_founding_payout_scale_v3(basis_width);
        crate::market::derived_founding_reserve_atoms_v1(
            crate::market::INTENDED_FOUNDING_RESERVE_ATOMS_V1,
            payout_scale,
        )
        .ok_or_else(|| {
            Error::new(format!(
                "no founding collateral reserve at or above {} atoms has a founding budget that \
                 is an exact multiple of the derived payout scale {payout_scale}",
                crate::market::INTENDED_FOUNDING_RESERVE_ATOMS_V1
            ))
        })?
    };
    let shape = crate::market::LocalMarketShapeV1 {
        // The one field that must fall back like all the others, and did not.
        //
        // `founding_band_from_arguments_v1` returns `None` for "the caller
        // stated no band", not for "this market has no band" — and the fixture's
        // default STATES one, in the words of its own declaration: "what the
        // fixture's author believes, written down where the compiler can hold
        // them to it. A caller that means something else states something else."
        // Passing that `None` straight through made a caller who stated nothing
        // an author who declared nothing, and the Pyth compiler refuses that by
        // name — which is how the loopback lifecycle's market stage died one
        // stage past the succession wall.
        founding_band: band.or(default.founding_band.clone()),
        cut_denominator: scalar(
            cut_denominator,
            "--cut-denominator",
            default.cut_denominator,
        )?,
        cuts,
        coefficients,
        initial_collateral_atoms: scalar(
            initial_collateral_atoms,
            "--initial-collateral-atoms",
            derived_reserve_atoms,
        )?,
        // ABSENT IS THE FIXTURE'S OWN DECLARED SHELF LIFE, unchanged, so every
        // command line written before this flag existed compiles the market it
        // always did. Stated, it is the number a campaign needs so its market's
        // primary leg closes inside a bounded run -- the honest alternative to
        // warping a validator's clock, and no more of a lab setting than the
        // constant it replaces.
        terminal_max_age_seconds: match terminal_max_age_seconds {
            None => default.terminal_max_age_seconds,
            Some(raw) => Some(
                raw.parse::<u32>()
                    .map_err(|_| Error::new("--terminal-max-age-seconds must be a decimal u32"))?,
            ),
        },
        terminal_window_width_seconds: match terminal_window_width_seconds {
            None => default.terminal_window_width_seconds,
            Some(raw) => raw
                .parse::<i64>()
                .map_err(|_| Error::new("--terminal-window-width-seconds must be a decimal i64"))?,
        },
        generation: scalar(generation, "--generation", default.generation)?,
        // `--recovery-rungs BPS:SECONDS[,BPS:SECONDS]` buys a funded ordered
        // ladder. Absent is the no-recovery market this fixture has always
        // compiled; `--recovery-rungs ""` is a REFUSAL rather than a synonym
        // for absent, because a caller who typed the flag meant to buy
        // something and a policy funding no attempt is not a thing to buy.
        recovery: match &recovery_rungs {
            None => default.recovery.clone(),
            Some(raw) => Some(parse_recovery_rungs_v1(raw)?),
        },
    };
    shape.validate()?;
    Ok(shape)
}

/// Parse `--recovery-rungs BPS:SECONDS_AFTER_PREVIOUS[,...]`.
///
/// The second half is an OFFSET and not an instant, because the primary leg's
/// deadline is the captured publication plus the fixture's declared shelf life
/// and a caller has no way to name it. Both halves are required on every rung:
/// a rung is a confidence bound and a lifetime, and defaulting either would
/// author a market dimension nobody stated.
pub(crate) fn parse_recovery_rungs_v1(
    raw: &str,
) -> Result<Vec<crate::market::RelativeRecoveryRungV1>> {
    let mut rungs = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (bps, seconds) = part.split_once(':').ok_or_else(|| {
            Error::new(format!(
                "--recovery-rungs takes BPS:SECONDS_AFTER_PREVIOUS per rung; {part:?} names no \
                 lifetime"
            ))
        })?;
        rungs.push(crate::market::RelativeRecoveryRungV1 {
            max_confidence_bps: bps.trim().parse::<u16>().map_err(|_| {
                Error::new(format!("--recovery-rungs confidence {bps:?} is not a u16"))
            })?,
            deadline_after_previous_seconds: seconds.trim().parse::<i64>().map_err(|_| {
                Error::new(format!(
                    "--recovery-rungs lifetime {seconds:?} is not an i64"
                ))
            })?,
        });
    }
    if rungs.is_empty() {
        return Err(Error::new(
            "--recovery-rungs was given and names no rung: omit the flag for the no-recovery \
             market rather than asking for a ladder with no legs",
        ));
    }
    Ok(rungs)
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-mutable-prepare-v1 --work ABSOLUTE_NEW_DIR \\\n     --output ABSOLUTE_NEW_JSON --checked-release-gate ABSOLUTE_CHECKED_UPGRADE_GATE_JSON \\\n     --expected-checked-release-gate-sha256 HEX64 --expected-source-revision HEX40 \\\n     --expected-source-tree-sha256 HEX64 --seed HEX64 [--hot-cu-profile]\n  \\
     dclutch-local-successor-bootstrap local-mutable-plan-authenticate-v1 --plan ABSOLUTE_JSON\n  \\
     dclutch-local-successor-bootstrap local-private-validator-market-v1 --plan ABSOLUTE_JSON \\
     --rpc-url http://127.0.0.1:PORT/ --fee-basis-points U16 \\
     --fee-recipient-keypair ABSOLUTE_DISPOSABLE_JSON \\
     [--cuts I128,..] [--cut-denominator U64] [--coefficients U64,..] \\
     [--initial-collateral-atoms U64] [--terminal-window-width-seconds I64] \\
     [--generation U64]\n\nThe prepare and authentication commands are offline and localhost-evidence-only. The first derives disposable role keys, seven pairwise-distinct local program identities, and seven exact mutable ProgramData bodies from one checked-release gate. --hot-cu-profile is diagnostic-only: it requires the authenticated Trading basis to name exactly that feature, keeps every non-Trading basis ordinary, and refuses an ordinary Trading gate. The market command admits only a literal loopback validator, authenticates the live seven-pair substrate read-only, and prints one canonical local MarketRunInput. The six shape flags are optional and each defaults to the value this fixture has always emitted, so a command line written without them compiles the same market it always did; --cuts sets the market's WIDTH (outcomes = cuts + 2, the two tails plus the explicit failure outcome) and --coefficients must then carry exactly that many payouts. The claim unit is NOT a flag: compile_linked_basis_v3 hard-wires it beside the categorical basis kind, so varying it is the same edit as emitting a graded basis."
}

fn derive(domain: &[u8], seed: [u8; 32], label: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(seed);
    hasher.update([0]);
    hasher.update(label.as_bytes());
    hasher.finalize().into()
}

fn write_keypair_create_new(path: &Path, keypair: &Keypair) -> Result<()> {
    let bytes = keypair.to_bytes();
    let json = serde_json::to_vec(&bytes.as_slice())?;
    write_bytes_create_new(path, &json, 0o600)
}

fn write_bytes_create_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute_new_directory(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.exists() || fs::symlink_metadata(&path).is_ok() {
        return Err(Error::new(format!(
            "{label} must be an absolute path that does not exist"
        )));
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(Error::new(format!("{label} parent does not exist")));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Omitting every shape flag compiles the fixture's own market, band included.
    ///
    /// The band is the only one of the seven that could regress silently: the
    /// other six have values a caller can read back off the shape, while an
    /// absent band is not visible until a Pyth compile refuses. It regressed,
    /// and cost the cold machine's loopback a whole stage.
    ///
    /// The ladder joined the flags in `6a3079454` and is read back the same
    /// way: a caller that buys no rung founds the market it always founded.
    #[test]
    fn omitting_every_shape_flag_keeps_the_fixtures_own_stated_band() {
        let default = crate::market::LocalMarketShapeV1::default();
        let shape =
            market_shape_from_arguments_v1(None, None, None, None, None, None, None, None, None)
                .expect("no flags is the fixture's own shape");
        let inherited = shape.founding_band.as_ref().expect(
            "the fixture's author states a band, and a caller who states nothing inherits it",
        );
        let stated = default
            .founding_band
            .as_ref()
            .expect("the default states its band");
        assert_eq!(format!("{inherited:?}"), format!("{stated:?}"));
        assert_eq!(shape.cuts, default.cuts);
        assert_eq!(shape.coefficients, default.coefficients);
        assert_eq!(shape.cut_denominator, default.cut_denominator);
        assert!(
            shape.recovery.is_none(),
            "a caller that states no rung buys no ladder"
        );
    }

    /// A caller that means something else states something else — all six.
    #[test]
    fn a_stated_band_replaces_the_fixtures_and_a_partial_one_refuses_by_name() {
        let stated = market_shape_from_arguments_v1(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(crate::model::FoundingBandInputV1::spot_band(
                21_000, 350, 8_000, 4, 8_500,
            )),
        )
        .expect("a stated band is the shape's band");
        assert_ne!(
            format!("{:?}", stated.founding_band),
            format!(
                "{:?}",
                crate::market::LocalMarketShapeV1::default().founding_band
            )
        );

        assert!(
            founding_band_from_arguments_v1(None, None, None, None, None)
                .expect("no band flags at all is not a refusal")
                .is_none()
        );
        let partial = founding_band_from_arguments_v1(
            Some("15000".into()),
            Some("200".into()),
            None,
            None,
            None,
        )
        .expect_err("a partial band must refuse");
        let text = format!("{partial:?}");
        for flag in [
            "--band-window-slots",
            "--band-plausible-half-widths",
            "--band-max-cell-share-bps",
        ] {
            assert!(text.contains(flag), "the refusal must name {flag}: {text}");
        }
    }

    #[test]
    fn prepare_parser_accepts_only_one_explicit_hot_cu_profile_token() {
        assert_eq!(
            parse_prepare_arguments_v1(Vec::new())
                .expect("empty parser input has the ordinary mode")
                .build_mode,
            LocalMutableBuildModeV1::Ordinary
        );
        assert_eq!(
            parse_prepare_arguments_v1(vec!["--hot-cu-profile".into()])
                .expect("hot-cu profile token")
                .build_mode,
            LocalMutableBuildModeV1::HotCuProfile
        );
        let duplicate =
            parse_prepare_arguments_v1(vec!["--hot-cu-profile".into(), "--hot-cu-profile".into()]);
        assert!(
            duplicate.is_err(),
            "duplicate hot-cu profile token must refuse"
        );
        assert!(
            duplicate
                .err()
                .expect("duplicate error")
                .to_string()
                .contains("may be supplied only once")
        );
    }

    #[test]
    fn hot_cu_profile_accepts_only_the_exact_trading_build_basis() {
        let ordinary = checked_build_command_v1("trading", LocalMutableBuildModeV1::Ordinary)
            .expect("ordinary Trading command");
        let hot_cu = checked_build_command_v1("trading", LocalMutableBuildModeV1::HotCuProfile)
            .expect("profiled Trading command");
        assert_eq!(
            hot_cu,
            "cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml --features hot-cu-profile -- --locked"
        );
        assert!(
            checked_build_command_matches_v1(
                &ordinary,
                "trading",
                LocalMutableBuildModeV1::Ordinary,
            )
            .expect("ordinary Trading basis")
        );
        assert!(
            !checked_build_command_matches_v1(
                &hot_cu,
                "trading",
                LocalMutableBuildModeV1::Ordinary,
            )
            .expect("profiled Trading must refuse without token")
        );
        assert!(
            !checked_build_command_matches_v1(
                &ordinary,
                "trading",
                LocalMutableBuildModeV1::HotCuProfile,
            )
            .expect("ordinary Trading must refuse in profile mode")
        );
        assert!(
            checked_build_command_matches_v1(
                &hot_cu,
                "trading",
                LocalMutableBuildModeV1::HotCuProfile,
            )
            .expect("exact profiled Trading basis")
        );

        for role in ["core", "claims", "resolution", "custody"] {
            let ordinary = checked_build_command_v1(role, LocalMutableBuildModeV1::Ordinary)
                .expect("ordinary execution command");
            assert!(
                checked_build_command_matches_v1(
                    &ordinary,
                    role,
                    LocalMutableBuildModeV1::HotCuProfile,
                )
                .expect("profile mode leaves non-Trading commands ordinary")
            );
            let profiled = format!(
                "{} --features hot-cu-profile -- --locked",
                ordinary
                    .strip_suffix(" -- --locked")
                    .expect("ordinary command has its locked Cargo suffix")
            );
            assert!(
                !checked_build_command_matches_v1(
                    &profiled,
                    role,
                    LocalMutableBuildModeV1::HotCuProfile,
                )
                .expect("profiled non-Trading basis must refuse")
            );
        }

        assert!(!checked_build_command_matches_v1(
            "cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml --features unknown-feature -- --locked",
            "trading",
            LocalMutableBuildModeV1::HotCuProfile,
        )
        .expect("unknown Trading feature must refuse"));
    }

    #[test]
    fn resolution_local_projection_selects_v7_and_refuses_v6() {
        let preimage = local_semantic_release_preimage_v1(
            "resolution",
            "0123456789abcdef0123456789abcdef01234567",
        )
        .expect("Resolution V7 semantic preimage");
        assert_eq!(
            preimage,
            dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V7
        );
        assert_eq!(
            Sha256::digest(&preimage).as_slice(),
            dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7
        );
        assert_ne!(
            Sha256::digest(&preimage).as_slice(),
            dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V6
        );
    }

    fn test_checked_execution_pin_v1() -> (CheckedLocalExecutionReleaseSetPinV1, String) {
        use dclutch_release_tool::{BuildMetadataV1, ReleaseEvidenceV1, build_checked_release};

        let mut releases = Vec::new();
        let mut manifests = Vec::new();
        for (ordinal, role) in EXECUTION_ROLE_ORDER_V1.into_iter().enumerate() {
            let seed = u8::try_from(ordinal).expect("test role ordinal") + 1;
            let mut elf = vec![0_u8; 64];
            elf[..4].copy_from_slice(b"\x7fELF");
            elf[4] = 2;
            elf[5] = 1;
            elf[6] = 1;
            elf[16..18].copy_from_slice(&3_u16.to_le_bytes());
            elf[18..20].copy_from_slice(&263_u16.to_le_bytes());
            elf[20..24].copy_from_slice(&1_u32.to_le_bytes());
            elf[52..54].copy_from_slice(&64_u16.to_le_bytes());
            elf[63] = seed;
            let program_id = [seed; 32];
            let programdata_id = [seed + 10; 32];
            let loader = bpf_loader_upgradeable::ID.to_bytes();
            let authority = [99; 32];
            let mut program = [0_u8; 36];
            program[..4].copy_from_slice(&2_u32.to_le_bytes());
            program[4..].copy_from_slice(&programdata_id);
            let mut programdata = vec![0_u8; 45];
            programdata[..4].copy_from_slice(&3_u32.to_le_bytes());
            programdata[4..12].copy_from_slice(&u64::from(seed).to_le_bytes());
            programdata[12] = 1;
            programdata[13..45].copy_from_slice(&authority);
            programdata.extend_from_slice(&elf);
            let semantic = format!("dclutch/test/local-checked/{role}/v1");
            let metadata = BuildMetadataV1::parse(&format!(
                "dclutch-release-metadata-v1\nsemantic_kind=unowned\nprogram_id={}\nprogramdata_id={}\nloader_program_id={}\nprogram_owner={}\nprogram_executable=true\nprogramdata_owner={}\nprogramdata_executable=false\nsource_digest={}\ncargo_lock_digest={}\nsource_revision=0123456789abcdef0123456789abcdef01234567\nrustc_version=rustc 1.89.0\nsolana_version=solana-cli 4.0.2\ncargo_build_sbf_version=cargo-build-sbf 4.0.2\ntarget_triple=sbpf-solana-solana\nbuild_command=cargo build-sbf --manifest-path programs/test/Cargo.toml -- --locked\nassumption=synthetic exact Loader evidence is scoped to this hostile unit test\n",
                hex(&program_id),
                hex(&programdata_id),
                hex(&loader),
                hex(&loader),
                hex(&loader),
                "44".repeat(32),
                "55".repeat(32),
            ))
            .expect("canonical metadata");
            let checked = build_checked_release(ReleaseEvidenceV1 {
                elf: &elf,
                semantic_preimage: semantic.as_bytes(),
                program_account_data: &program,
                programdata_account_data: &programdata,
                metadata: &metadata,
            })
            .expect("checked test release");
            manifests.push(checked.encode().expect("encode checked test release"));
            releases.push(checked);
        }
        let refs = [
            &releases[0],
            &releases[1],
            &releases[2],
            &releases[3],
            &releases[4],
        ];
        let release_set = derive_execution_release_set(refs).expect("derive test release set");
        let complete = build_checked_execution_release_set(release_set, refs)
            .expect("build checked test release set");
        let execution_release_set_id = hex(complete
            .execution_release_set_id()
            .expect("execution release set ID")
            .as_bytes());
        let roles = EXECUTION_ROLE_ORDER_V1
            .into_iter()
            .zip(manifests)
            .map(|(role, manifest)| {
                let checked = CheckedReleaseV1::decode(&manifest).expect("decode checked role");
                CheckedLocalExecutionReleaseRolePinV1 {
                    role: role.into(),
                    checked_release_id: hex(checked
                        .checked_release_id()
                        .expect("checked role ID")
                        .as_bytes()),
                    checked_release_base64: BASE64.encode(manifest),
                }
            })
            .collect();
        (
            CheckedLocalExecutionReleaseSetPinV1 {
                schema: CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1.into(),
                checked_execution_release_set_id: hex(complete
                    .checked_execution_release_set_id()
                    .expect("checked set ID")
                    .as_bytes()),
                execution_release_set_id: execution_release_set_id.clone(),
                checked_execution_release_set_base64: BASE64.encode(complete.encode()),
                roles,
            },
            execution_release_set_id,
        )
    }

    #[test]
    fn local_launcher_uses_the_upgrade_role_owner_without_an_alias() {
        assert_eq!(crate::upgrade::CHECKED_ROLE_ORDER_V1.len(), 7);
        assert_eq!(
            crate::upgrade::CHECKED_ROLE_ORDER_V1
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len(),
            crate::upgrade::CHECKED_ROLE_ORDER_V1.len()
        );
    }

    #[test]
    fn local_lifecycle_roles_include_distinct_vacant_pyth_accounts() {
        let roles = EXTRA_KEY_ROLES_V1.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(roles.len(), EXTRA_KEY_ROLES_V1.len());
        assert!(roles.contains(crate::seed::role::FOUNDING_FOUNDER));
        assert!(roles.contains("pyth-encoded-vaa"));
        assert!(roles.contains("pyth-update-account"));
        let campaign_roles = crate::campaign::KEYPAIR_ROLES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(
            !campaign_roles.contains(crate::seed::role::FOUNDING_FOUNDER),
            "the later lifecycle keeps the founder signer, but public founding receives only its pubkey"
        );
        assert_eq!(
            roles
                .intersection(&campaign_roles)
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
                crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
            ]),
            "only the fixture source and its owner intentionally cross the local-lifecycle/campaign boundary"
        );
        assert_eq!(
            local_key_roles_v1().len(),
            campaign_roles.len() + roles.len() - 2,
            "the local key surface is the canonical set union, not a sequence that writes shared roles twice"
        );
    }

    #[test]
    fn local_campaign_projects_two_public_founders_without_a_substituted_signer_file() {
        let roles = local_key_roles_v1();
        assert!(roles.contains(crate::seed::role::FOUNDING_FOUNDER));
        assert!(!roles.contains(crate::seed::role::SUBSTITUTED_FOUNDER));
        assert!(
            !crate::campaign::KEYPAIR_ROLES.contains(&crate::seed::role::FOUNDING_FOUNDER)
                && !crate::campaign::KEYPAIR_ROLES
                    .contains(&crate::seed::role::SUBSTITUTED_FOUNDER)
        );
        let public = local_campaign_public_identities_v1([0x73; 32])
            .expect("two public founding identities");
        assert_eq!(
            public.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![
                crate::seed::role::FOUNDING_FOUNDER,
                crate::seed::role::SUBSTITUTED_FOUNDER,
            ]
        );
        let founder = public[crate::seed::role::FOUNDING_FOUNDER]
            .parse::<Pubkey>()
            .expect("founder public key");
        let substituted = public[crate::seed::role::SUBSTITUTED_FOUNDER]
            .parse::<Pubkey>()
            .expect("substituted-founder public key");
        assert_ne!(founder, substituted);
    }

    #[test]
    fn local_report_projects_exact_mode_specific_campaign_keypairs() {
        let keypairs = local_key_roles_v1()
            .into_iter()
            .map(|role| (role.to_owned(), format!("/tmp/{role}.json")))
            .collect::<BTreeMap<_, _>>();
        let administration = project_keypair_roles_v1(
            &keypairs,
            crate::campaign::ADMIN_ALLOWED_ROLES,
            "administration",
        )
        .expect("administration projection");
        assert_eq!(
            administration
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec![
                crate::seed::role::CAMPAIGN_PAYER,
                crate::seed::role::CORE_UPGRADE_AUTHORITY,
            ]
        );
        let founding_roles = crate::campaign::FOUNDING_REQUIRED_ROLES
            .iter()
            .copied()
            .chain([
                crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
                crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
            ])
            .collect::<Vec<_>>();
        let founding = project_keypair_roles_v1(&keypairs, &founding_roles, "founding")
            .expect("founding projection");
        assert_eq!(founding.len(), 8);
        assert_eq!(
            administration
                .keys()
                .filter(|role| founding.contains_key(*role))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec![crate::seed::role::CAMPAIGN_PAYER],
            "the ordinary payer is shared across modes; the succession consent authority is not"
        );
        assert!(!founding.contains_key(crate::seed::role::FOUNDING_FOUNDER));
        assert!(!founding.contains_key(crate::seed::role::SUBSTITUTED_FOUNDER));
    }

    #[test]
    fn set_digest_is_not_self_referential() {
        let mut set = CheckedLocalMutableSetPinV1 {
            schema: CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1.into(),
            checked_release_gate_path: "/tmp/CHECKED_UPGRADE_GATE.json".into(),
            checked_release_gate_sha256: "11".repeat(32),
            source_revision: "22".repeat(20),
            source_tree_sha256: "33".repeat(32),
            solana_cli_version: "solana-cli 4.0.2".into(),
            retained_upgrade_authority: Pubkey::new_unique().to_string(),
            execution_release_set: CheckedLocalExecutionReleaseSetPinV1 {
                schema: CHECKED_LOCAL_EXECUTION_RELEASE_SET_SCHEMA_V1.into(),
                checked_execution_release_set_id: "44".repeat(32),
                execution_release_set_id: "55".repeat(32),
                checked_execution_release_set_base64: String::new(),
                roles: Vec::new(),
            },
            set_sha256: "first value is ignored".into(),
            roles: Vec::new(),
        };
        let first = checked_local_set_digest_v1(&set).expect("digest");
        set.set_sha256 = "different ignored value".into();
        assert_eq!(first, checked_local_set_digest_v1(&set).expect("digest"));
    }

    #[test]
    fn persisted_execution_set_refuses_role_manifest_and_envelope_substitution() {
        let (pin, release_set_id) = test_checked_execution_pin_v1();
        assert_eq!(
            authenticate_persisted_execution_release_set_v1(&pin, &release_set_id)
                .expect("canonical checked execution set")
                .len(),
            CHECKED_MULTIPROGRAM_BYTES_V1
        );

        let mut reordered = pin.clone();
        reordered.roles.swap(0, 1);
        assert!(
            authenticate_persisted_execution_release_set_v1(&reordered, &release_set_id).is_err()
        );

        let mut replaced_id = pin.clone();
        replaced_id.roles[2].checked_release_id = "66".repeat(32);
        assert!(
            authenticate_persisted_execution_release_set_v1(&replaced_id, &release_set_id).is_err()
        );

        let mut replaced_manifest = pin.clone();
        let mut manifest = BASE64
            .decode(&replaced_manifest.roles[3].checked_release_base64)
            .expect("checked role base64");
        *manifest.last_mut().expect("checked role byte") ^= 1;
        replaced_manifest.roles[3].checked_release_base64 = BASE64.encode(manifest);
        assert!(
            authenticate_persisted_execution_release_set_v1(&replaced_manifest, &release_set_id)
                .is_err()
        );

        let mut replaced_envelope = pin;
        let mut envelope = BASE64
            .decode(&replaced_envelope.checked_execution_release_set_base64)
            .expect("checked set base64");
        *envelope.last_mut().expect("checked set byte") ^= 1;
        replaced_envelope.checked_execution_release_set_base64 = BASE64.encode(envelope);
        assert!(
            authenticate_persisted_execution_release_set_v1(&replaced_envelope, &release_set_id)
                .is_err()
        );
    }
}
