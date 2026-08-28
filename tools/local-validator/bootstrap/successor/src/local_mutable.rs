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

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as _},
};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    model::{CheckedLocalMutableRolePinV1, CheckedLocalMutableSetPinV1, ProgramPin, SuccessorPlan},
    plan::{
        PrepareArgs, RecordPublicationV1, RoleDeploymentInputV1, RoleDeploymentsV1, hex, hex32,
        loader_programdata_bytes, pubkey,
    },
    upgrade::{authenticate_checked_release_gate_role_for_local_v1, checked_semantic_release_id},
};

pub(crate) const CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1: &str = "dclutch-checked-local-mutable-set-v1";
const SET_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/checked-local-mutable-set/v1";

/// Operator-independent checked-gate facts supplied to local plan preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedLocalMutableGateInputV1 {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree_sha256: String,
}

/// Build the local provenance pin from already hostile-decoded ProgramData
/// facts.  `plan::prepare` is the sole producer: a caller cannot author any
/// role row directly.
pub(crate) fn build_checked_local_mutable_set_v1(
    gate: &CheckedLocalMutableGateInputV1,
    retained_authority: Pubkey,
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
        if validated.gate_sha256 != gate.sha256
            || validated.source_revision != gate.source_revision
            || validated.source_tree_sha256 != gate.source_tree_sha256
        {
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
        let expected_semantic = checked_semantic_release_id(role, &gate.source_revision)?;
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

    let mut set = CheckedLocalMutableSetPinV1 {
        schema: CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1.into(),
        checked_release_gate_path: gate_path.display().to_string(),
        checked_release_gate_sha256: gate.sha256.clone(),
        source_revision: gate.source_revision.clone(),
        source_tree_sha256: gate.source_tree_sha256.clone(),
        solana_cli_version: solana_cli_version
            .ok_or_else(|| Error::new("checked local release gate projected no roles"))?,
        retained_upgrade_authority: retained,
        set_sha256: String::new(),
        roles: projected,
    };
    set.set_sha256 = checked_local_set_digest_v1(&set)?;
    Ok(set)
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
    };
    let rebuilt = build_checked_local_mutable_set_v1(
        &gate,
        authority,
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
    for role in &set.roles {
        let genesis = plan
            .genesis_accounts
            .values()
            .find(|pin| pin.address == role.programdata_id)
            .ok_or_else(|| {
                Error::new(format!(
                    "checked local {} ProgramData is absent from the genesis account closure",
                    role.role
                ))
            })?;
        if genesis.data_sha256 != role.programdata_account_sha256 {
            return Err(Error::new(format!(
                "checked local {} ProgramData genesis bytes differ from the release pin",
                role.role
            )));
        }
    }
    Ok(())
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
}

const LOCAL_ID_DOMAIN_V1: &[u8] = b"dclutch/private-validator-lifecycle/program-id/v1";
const LOCAL_KEY_DOMAIN_V1: &[u8] = b"dclutch/private-validator-lifecycle/keypair/v1";
const EXTRA_KEY_ROLES_V1: [&str; 6] = [
    "participant",
    "direct-seller",
    "direct-buyer",
    "resolver",
    "payout-owner",
    "retirement-beneficiary",
];

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
    let mut work = None;
    let mut output = None;
    let mut gate_path = None;
    let mut gate_sha256 = None;
    let mut source_revision = None;
    let mut source_tree_sha256 = None;
    let mut seed = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
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
    let work = absolute_new_directory(required(work, "--work")?, "--work")?;
    let output = PathBuf::from(required(output, "--output")?);
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
        path: PathBuf::from(required(gate_path, "--checked-release-gate")?),
        sha256: required(gate_sha256, "--expected-checked-release-gate-sha256")?,
        source_revision: required(source_revision, "--expected-source-revision")?,
        source_tree_sha256: required(source_tree_sha256, "--expected-source-tree-sha256")?,
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
    let seed = hex32(&required(seed, "--seed")?)
        .map_err(|_| Error::new("--seed must be exactly 64 lowercase hex characters"))?;

    fs::create_dir(&work)?;
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
    for role in crate::campaign::KEYPAIR_ROLES
        .iter()
        .copied()
        .chain(EXTRA_KEY_ROLES_V1)
    {
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
            checked_semantic_release_id(role, &gate.source_revision)?,
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
        campaign_keypairs: crate::campaign::KEYPAIR_ROLES
            .iter()
            .map(|role| {
                keypairs
                    .get(*role)
                    .cloned()
                    .map(|path| ((*role).into(), path))
                    .ok_or_else(|| {
                        Error::new(format!("local derivation omitted campaign role {role}"))
                    })
            })
            .collect::<Result<BTreeMap<_, _>>>()?,
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
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &plan_path,
        &required(rpc_url, "--rpc-url")?,
        registry,
        Some(fee_basis_points),
        Some(recipient),
    )?;
    let market = crate::market::demo_market_input(registry, direct.compiler())?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &market)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-mutable-prepare-v1 --work ABSOLUTE_NEW_DIR \\\n     --output ABSOLUTE_NEW_JSON --checked-release-gate ABSOLUTE_CHECKED_UPGRADE_GATE_JSON \\\n     --expected-checked-release-gate-sha256 HEX64 --expected-source-revision HEX40 \\\n     --expected-source-tree-sha256 HEX64 --seed HEX64\n  \\
     dclutch-local-successor-bootstrap local-mutable-plan-authenticate-v1 --plan ABSOLUTE_JSON\n  \\
     dclutch-local-successor-bootstrap local-private-validator-market-v1 --plan ABSOLUTE_JSON \\
     --rpc-url http://127.0.0.1:PORT/ --fee-basis-points U16 \\
     --fee-recipient-keypair ABSOLUTE_DISPOSABLE_JSON\n\nThe prepare and authentication commands are offline and localhost-evidence-only. The first derives disposable role keys, seven pairwise-distinct local program identities, and seven exact mutable ProgramData bodies from one checked-release gate. The market command admits only a literal loopback validator, authenticates the live seven-pair substrate read-only, and prints one canonical local MarketRunInput."
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
    fn set_digest_is_not_self_referential() {
        let mut set = CheckedLocalMutableSetPinV1 {
            schema: CHECKED_LOCAL_MUTABLE_SET_SCHEMA_V1.into(),
            checked_release_gate_path: "/tmp/CHECKED_UPGRADE_GATE.json".into(),
            checked_release_gate_sha256: "11".repeat(32),
            source_revision: "22".repeat(20),
            source_tree_sha256: "33".repeat(32),
            solana_cli_version: "solana-cli 4.0.2".into(),
            retained_upgrade_authority: Pubkey::new_unique().to_string(),
            set_sha256: "first value is ignored".into(),
            roles: Vec::new(),
        };
        let first = checked_local_set_digest_v1(&set).expect("digest");
        set.set_sha256 = "different ignored value".into();
        assert_eq!(first, checked_local_set_digest_v1(&set).expect("digest"));
    }
}
