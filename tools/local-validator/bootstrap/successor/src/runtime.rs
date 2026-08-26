use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, CompiledProductRecordsV2,
    publication::{
        ProductPublicationContentV2, ProductPublicationMemberV2, ProductPublicationStateV2,
        RecordPublicationActionV1, RecordPublicationContentV1, RecordPublicationStateV1,
        build_product_publication_step_v2, build_record_publication_step_v1,
        derive_record_addresses_v1, product_publication_content_v2,
    },
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry_svm::{REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1, RegistryInstructionV1};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionRoleV1, InitializeProtocolInfrastructureV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1,
    ProtocolInfrastructureProfileV1,
};
use sha2::Digest as _;
use solana_loader_v3_interface::instruction::set_upgrade_authority;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    model::{ProgramPin, RunProgramInput, SuccessorPlan, SuccessorRunEvidence, SuccessorRunSpec},
    plan::{
        PrepareArgs, hex, hex32, loader_programdata_bytes, loader_programdata_bytes_after_revoke,
        pubkey, validate_program_ids,
    },
    rpc::{Rpc, account_evidence, validate_loopback_url},
};

const RUN_SPEC_SCHEMA_V2: &str = "dclutch-local-successor-run-spec-v2";
const RUN_EVIDENCE_SCHEMA_V2: &str = "dclutch-local-successor-run-evidence-v2";
const EXPECTED_RPC_URL: &str = "http://127.0.0.1:20890/";
const AUTHORITY_LAMPORTS: u64 = 5_000_000_000;
const VALIDATOR_READY_TIMEOUT: Duration = Duration::from_secs(60);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublishedRecord {
    pub(crate) schema: [u8; 32],
    pub(crate) digest: [u8; 32],
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

struct ValidatorChild {
    child: Child,
}

impl ValidatorChild {
    fn spawn(spec: &SuccessorRunSpec, plan: &SuccessorPlan, log_path: &Path) -> Result<Self> {
        if plan.core_bootstrap.upgrade_authority == Pubkey::default().to_string() {
            return Err(Error::new("refusing zero in-memory Core authority"));
        }
        let log = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(log_path)
            .map_err(|error| {
                Error::new(format!(
                    "create validator log {}: {error}",
                    log_path.display()
                ))
            })?;
        let stderr = log
            .try_clone()
            .map_err(|error| Error::new(format!("clone validator log: {error}")))?;
        let mut command = Command::new(&spec.launcher);
        command
            .arg("start")
            .arg("--ledger")
            .arg(&spec.ledger)
            .arg("--account-dir")
            .arg(&spec.account_dir)
            .arg("--plan")
            .arg(&spec.plan);
        append_program_args(&mut command, "registry", &spec.registry);
        append_program_args(&mut command, "core", &spec.core);
        append_program_args(&mut command, "claims", &spec.claims);
        append_program_args(&mut command, "trading", &spec.trading);
        append_program_args(&mut command, "resolution", &spec.resolution);
        append_program_args(&mut command, "custody", &spec.custody);
        append_program_args(&mut command, "rent-credit", &spec.rent_credit);
        command
            .arg("--foreground")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr));
        let child = command.spawn().map_err(|error| {
            Error::new(format!(
                "launch guarded successor validator {}: {error}",
                spec.launcher
            ))
        })?;
        Ok(Self { child })
    }

    fn wait_for_rpc(&mut self, plan: &SuccessorPlan) -> Result<Rpc> {
        let expected_programdata = pubkey(&plan.core.programdata_id)?;
        let expected_hash = &plan.core_bootstrap.genesis_programdata_sha256;
        let deadline = Instant::now() + VALIDATOR_READY_TIMEOUT;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| Error::new(format!("poll validator child: {error}")))?
            {
                return Err(Error::new(format!(
                    "successor validator exited before exact health: {status}"
                )));
            }
            if let Ok(mut rpc) = Rpc::connect(EXPECTED_RPC_URL)
                && let Ok(account) = rpc.required_account(expected_programdata, "Core ProgramData")
                && hex(&sha2::Sha256::digest(&account.data)) == *expected_hash
            {
                return Ok(rpc);
            }
            if Instant::now() >= deadline {
                return Err(Error::new(
                    "successor validator did not expose the exact prepared Core ProgramData within 60 seconds",
                ));
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for ValidatorChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn append_program_args(command: &mut Command, label: &str, input: &RunProgramInput) {
    command
        .arg(format!("--{label}-program-id"))
        .arg(&input.program_id)
        .arg(format!("--{label}-elf"))
        .arg(&input.elf_path)
        .arg(format!("--{label}-sha256"))
        .arg(&input.elf_sha256)
        .arg(format!("--{label}-attestation"))
        .arg(&input.attestation);
}

/// Own the complete authority lifetime and leave the ledger as evidence.
pub(crate) fn execute(spec_path: &Path) -> Result<()> {
    validate_existing_canonical_file(spec_path, "--spec")?;
    let spec: SuccessorRunSpec = serde_json::from_slice(&fs::read(spec_path)?)?;
    validate_spec(&spec)?;
    ensure_rpc_port_free()?;

    let authority = Keypair::new();
    let plan = crate::plan::prepare(prepare_args(&spec, authority.pubkey())?)?;
    validate_plan(&plan)?;
    if plan.core_bootstrap.upgrade_authority != authority.pubkey().to_string() {
        return Err(Error::new(
            "prepared Core authority did not equal the in-memory supervisor key",
        ));
    }
    let plan_bytes = fs::read(&spec.plan)?;
    let plan_sha256 = hex(&sha2::Sha256::digest(&plan_bytes));
    let validator_log = validator_log_path(&spec)?;
    let mut validator = ValidatorChild::spawn(&spec, &plan, &validator_log)?;
    let mut rpc = validator.wait_for_rpc(&plan)?;
    if rpc.url() != EXPECTED_RPC_URL {
        return Err(Error::new("healthy RPC origin changed after launch"));
    }

    let hostile = Keypair::new();
    let mut transactions = vec![
        rpc.airdrop(
            "fund ephemeral Core authority",
            authority.pubkey(),
            AUTHORITY_LAMPORTS,
        )?,
        rpc.airdrop(
            "fund hostile wrong authority",
            hostile.pubkey(),
            AUTHORITY_LAMPORTS,
        )?,
    ];
    let profile = pubkey(&plan.infrastructure_profile.address)?;
    if rpc.account(profile)?.is_some() {
        return Err(Error::new(
            "infrastructure profile unexpectedly existed at genesis",
        ));
    }
    transactions.push(rpc.send_expected_failure(
        "wrong authority cannot initialize infrastructure",
        &[initialize_instruction(
            &plan,
            hostile.pubkey(),
            hostile.pubkey(),
        )?],
        &hostile,
    )?);
    if rpc.account(profile)?.is_some() {
        return Err(Error::new(
            "wrong-authority initialization left a profile account",
        ));
    }

    transactions.push(rpc.send(
        "initialize Core infrastructure profile",
        &[initialize_instruction(
            &plan,
            authority.pubkey(),
            authority.pubkey(),
        )?],
        &authority,
    )?);
    verify_profile(&mut rpc, &plan)?;

    let activation = pubkey(&plan.activation)?;
    if rpc.account(activation)?.is_some() {
        return Err(Error::new(
            "release activation cache unexpectedly existed at genesis",
        ));
    }
    transactions.push(rpc.send_expected_failure(
        "immutable release activation refuses pre-revocation Core",
        &[role_activation_instruction(
            &plan,
            authority.pubkey(),
            ExecutionRoleV1::Core,
        )?],
        &authority,
    )?);
    if rpc.account(activation)?.is_some() {
        return Err(Error::new("pre-revocation activation left a cache account"));
    }

    let core_program = pubkey(&plan.core.program_id)?;
    transactions.push(rpc.send(
        "revoke Core Loader-v3 upgrade authority",
        &[set_upgrade_authority(
            &core_program,
            &authority.pubkey(),
            None,
        )],
        &authority,
    )?);
    verify_core_programdata(&mut rpc, &plan)?;

    for (label, instruction) in activation_instructions(&plan, authority.pubkey())? {
        transactions.push(rpc.send(label, &[instruction], &authority)?);
    }
    verify_activation(&mut rpc, &plan)?;

    let rollback_recipient = Pubkey::new_unique();
    let authority_before = rpc.required_account(authority.pubkey(), "Core authority wallet")?;
    if rpc.account(rollback_recipient)?.is_some() {
        return Err(Error::new("rollback recipient unexpectedly existed"));
    }
    let mut late_activation =
        role_activation_instruction(&plan, authority.pubkey(), ExecutionRoleV1::Custody)?;
    substitute_role_programdata(&mut late_activation, pubkey(&plan.core.programdata_id)?)?;
    let late_failure = rpc.send_expected_failure(
        "late activation substitution rolls back prior transfer",
        &[
            transfer(&authority.pubkey(), &rollback_recipient, 1),
            late_activation,
        ],
        &authority,
    )?;
    let fee = late_failure
        .fee_lamports
        .ok_or_else(|| Error::new("late-failure transaction omitted exact fee"))?;
    transactions.push(late_failure);
    if rpc.account(rollback_recipient)?.is_some() {
        return Err(Error::new(
            "late-failure transaction did not roll back the earlier transfer",
        ));
    }
    let authority_after = rpc.required_account(authority.pubkey(), "Core authority wallet")?;
    if authority_after.lamports.checked_add(fee) != Some(authority_before.lamports) {
        return Err(Error::new(
            "late-failure authority delta was not exactly the transaction fee",
        ));
    }
    verify_profile(&mut rpc, &plan)?;
    verify_core_programdata(&mut rpc, &plan)?;
    verify_activation(&mut rpc, &plan)?;

    let market = crate::market::execute_found_market(
        &mut rpc,
        &plan,
        &spec.market,
        &authority,
        &mut transactions,
    )?;

    let mut accounts = BTreeMap::new();
    for (label, address) in [
        ("core_programdata", pubkey(&plan.core.programdata_id)?),
        ("infrastructure_profile", profile),
        ("release_activation", activation),
        ("core_authority_wallet", authority.pubkey()),
    ] {
        let account = rpc.required_account(address, label)?;
        accounts.insert(label.into(), account_evidence(address, &account));
    }
    accounts.extend(market.accounts);
    let mut completed = vec![
        "generated one ephemeral Core authority in process memory".into(),
        "prepared exact public-key-only genesis plan".into(),
        "started and health-bound guarded localhost validator".into(),
        "proved wrong-authority infrastructure refusal".into(),
        "initialized exact Core Registry/Rent infrastructure profile".into(),
        "proved release activation refuses before Core revocation".into(),
        "revoked Core Loader-v3 upgrade authority to None".into(),
        "verified exact immutable Core ProgramData poststate".into(),
        "activated exact immutable five-role release set".into(),
        "proved late-failure atomic rollback".into(),
    ];
    completed.extend(market.completed);
    let evidence = SuccessorRunEvidence {
        schema: RUN_EVIDENCE_SCHEMA_V2.into(),
        rpc_url: rpc.url().into(),
        ledger: spec.ledger.clone(),
        validator_log: validator_log.display().to_string(),
        plan_sha256,
        core_upgrade_authority_pubkey: authority.pubkey().to_string(),
        private_key_persisted: false,
        completed,
        transactions,
        accounts,
        remaining_execution_seam: crate::market::REMAINING_OPEN_SEAM.into(),
    };
    write_evidence(Path::new(&spec.output), &evidence)?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn prepare_args(spec: &SuccessorRunSpec, authority: Pubkey) -> Result<PrepareArgs> {
    Ok(PrepareArgs {
        account_dir: PathBuf::from(&spec.account_dir),
        plan_path: PathBuf::from(&spec.plan),
        registry_program: pubkey(&spec.registry.program_id)?,
        registry_elf: PathBuf::from(&spec.registry.elf_path),
        registry_sha256: spec.registry.elf_sha256.clone(),
        registry_semantic_release_id: spec.registry.semantic_release_id.clone(),
        core_program: pubkey(&spec.core.program_id)?,
        core_elf: PathBuf::from(&spec.core.elf_path),
        core_sha256: spec.core.elf_sha256.clone(),
        core_semantic_release_id: spec.core.semantic_release_id.clone(),
        core_bootstrap_upgrade_authority: authority,
        claims_program: pubkey(&spec.claims.program_id)?,
        claims_elf: PathBuf::from(&spec.claims.elf_path),
        claims_sha256: spec.claims.elf_sha256.clone(),
        claims_semantic_release_id: spec.claims.semantic_release_id.clone(),
        trading_program: pubkey(&spec.trading.program_id)?,
        trading_elf: PathBuf::from(&spec.trading.elf_path),
        trading_sha256: spec.trading.elf_sha256.clone(),
        trading_semantic_release_id: spec.trading.semantic_release_id.clone(),
        resolution_program: pubkey(&spec.resolution.program_id)?,
        resolution_elf: PathBuf::from(&spec.resolution.elf_path),
        resolution_sha256: spec.resolution.elf_sha256.clone(),
        resolution_semantic_release_id: spec.resolution.semantic_release_id.clone(),
        custody_program: pubkey(&spec.custody.program_id)?,
        custody_elf: PathBuf::from(&spec.custody.elf_path),
        custody_sha256: spec.custody.elf_sha256.clone(),
        custody_semantic_release_id: spec.custody.semantic_release_id.clone(),
        rent_credit_program: pubkey(&spec.rent_credit.program_id)?,
        rent_credit_elf: PathBuf::from(&spec.rent_credit.elf_path),
        rent_credit_sha256: spec.rent_credit.elf_sha256.clone(),
        rent_credit_semantic_release_id: spec.rent_credit.semantic_release_id.clone(),
    })
}

fn initialize_instruction(
    plan: &SuccessorPlan,
    payer: Pubkey,
    authority: Pubkey,
) -> Result<Instruction> {
    let registry = record(plan, "registry_artifact_release")?;
    let rent = record(plan, "rent_artifact_release")?;
    Ok(Instruction {
        program_id: pubkey(&plan.core.program_id)?,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pubkey(&plan.infrastructure_profile.address)?, false),
            AccountMeta::new_readonly(pubkey(&plan.core.programdata_id)?, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(registry.0, false),
            AccountMeta::new_readonly(registry.1, false),
            AccountMeta::new_readonly(pubkey(&plan.registry.program_id)?, false),
            AccountMeta::new_readonly(pubkey(&plan.registry.programdata_id)?, false),
            AccountMeta::new_readonly(rent.0, false),
            AccountMeta::new_readonly(rent.1, false),
            AccountMeta::new_readonly(pubkey(&plan.rent_credit.program_id)?, false),
            AccountMeta::new_readonly(pubkey(&plan.rent_credit.programdata_id)?, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: InitializeProtocolInfrastructureV1.to_bytes().to_vec(),
    })
}

/// Canonical order in which the release set is walked up, one role per
/// transaction.
const ACTIVATION_ROLES_V1: [ExecutionRoleV1; 5] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

/// Exact ten-account frame admitting one role into the shared activation cache.
///
/// Activation is one role per transaction: whole-ELF hashing costs about one
/// compute unit per two bytes, and the real seven artifacts total roughly
/// 4.2 MB, so a five-role transaction cannot fit under the 1,400,000 maximum.
fn role_activation_instruction(
    plan: &SuccessorPlan,
    payer: Pubkey,
    role: ExecutionRoleV1,
) -> Result<Instruction> {
    let release_set = record(plan, "execution_release_set")?;
    let (label, pin) = match role {
        ExecutionRoleV1::Core => ("core_artifact_release", &plan.core),
        ExecutionRoleV1::Claims => ("claims_artifact_release", &plan.claims),
        ExecutionRoleV1::Trading => ("trading_artifact_release", &plan.trading),
        ExecutionRoleV1::Resolution => ("resolution_artifact_release", &plan.resolution),
        ExecutionRoleV1::Custody => ("custody_artifact_release", &plan.custody),
    };
    let pair = record(plan, label)?;
    let accounts = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new(pubkey(&plan.activation)?, false),
        AccountMeta::new_readonly(release_set.0, false),
        AccountMeta::new_readonly(release_set.1, false),
        AccountMeta::new_readonly(pair.0, false),
        AccountMeta::new_readonly(pair.1, false),
        AccountMeta::new_readonly(pubkey(&pin.program_id)?, false),
        AccountMeta::new_readonly(pubkey(&pin.programdata_id)?, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];
    if accounts.len() != REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1 {
        return Err(Error::new(
            "internal Registry role-activation frame was not exact ten",
        ));
    }
    Ok(Instruction {
        program_id: pubkey(&plan.registry.program_id)?,
        accounts,
        data: RegistryInstructionV1::ActivateRole(role).to_bytes().to_vec(),
    })
}

/// Ordered per-role activation instructions with a human label for each.
fn activation_instructions(
    plan: &SuccessorPlan,
    payer: Pubkey,
) -> Result<Vec<(&'static str, Instruction)>> {
    let mut ordered = Vec::with_capacity(ACTIVATION_ROLES_V1.len());
    for role in ACTIVATION_ROLES_V1 {
        ordered.push((
            match role {
                ExecutionRoleV1::Core => "activate immutable release-set role: Core",
                ExecutionRoleV1::Claims => "activate immutable release-set role: Claims",
                ExecutionRoleV1::Trading => "activate immutable release-set role: Trading",
                ExecutionRoleV1::Resolution => "activate immutable release-set role: Resolution",
                ExecutionRoleV1::Custody => "activate immutable release-set role: Custody",
            },
            role_activation_instruction(plan, payer, role)?,
        ));
    }
    Ok(ordered)
}

/// Replace the role ProgramData coordinate of one ten-account activation.
fn substitute_role_programdata(instruction: &mut Instruction, replacement: Pubkey) -> Result<()> {
    if instruction.accounts.len() != REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1 {
        return Err(Error::new(
            "late-failure probe requires an exact ten-account role activation",
        ));
    }
    let meta = instruction
        .accounts
        .get_mut(7)
        .ok_or_else(|| Error::new("activation omitted role ProgramData"))?;
    if meta.is_signer || meta.is_writable {
        return Err(Error::new("role ProgramData meta had privileges"));
    }
    meta.pubkey = replacement;
    Ok(())
}

pub(crate) fn record(plan: &SuccessorPlan, label: &str) -> Result<(Pubkey, Pubkey)> {
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("plan omitted record {label}")))?;
    let raw = pubkey(&pair.raw)?;
    let staging = pubkey(&pair.staging)?;
    let schema = hex32(&pair.schema_id)?;
    let content = hex32(&pair.content_sha256)?;
    let registry = pubkey(&plan.registry.program_id)?;
    if raw
        != Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &content], &registry).0
        || staging
            != Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &content],
                &registry,
            )
            .0
    {
        return Err(Error::new(format!("record {label} PDA mismatch")));
    }
    Ok((raw, staging))
}

pub(crate) fn publish_record(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    schema: [u8; 32],
    content: &[u8],
    hostile_refund_wallet: Option<Pubkey>,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<PublishedRecord> {
    let publication = RecordPublicationContentV1 {
        schema_release_id: schema,
        content,
    };
    let (raw, staging, digest) = derive_record_addresses_v1(registry, publication)
        .map_err(|error| Error::new(format!("derive record publication: {error:?}")))?;
    let mut minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .unwrap_or(rpc.finalized_slot()?);
    for _ in 0..1024 {
        let keys = [
            payer.pubkey(),
            raw,
            staging,
            system_program::ID,
            sysvar::rent::ID,
            sysvar::clock::ID,
        ];
        let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
        let observations = publication_observations(slot, &keys, &values)?;
        let state = RecordPublicationStateV1 {
            sponsor: observations[0],
            raw_record: observations[1],
            staging_cursor: observations[2],
            system_program: observations[3],
            rent: observations[4],
            clock: observations[5],
        };
        let plan = build_record_publication_step_v1(registry, publication, state)
            .map_err(|error| Error::new(format!("chain-derived record publication: {error:?}")))?;
        if plan.action == RecordPublicationActionV1::Complete {
            verify_published_record(rpc, registry, raw, staging, content)?;
            return Ok(PublishedRecord {
                schema,
                digest,
                raw,
                staging,
            });
        }
        let instruction = plan
            .instruction
            .ok_or_else(|| Error::new("incomplete record publication omitted instruction"))?;
        if plan.action == RecordPublicationActionV1::Finalize
            && let Some(hostile_wallet) = hostile_refund_wallet
        {
            let mut hostile = instruction.clone();
            hostile
                .accounts
                .get_mut(2)
                .ok_or_else(|| Error::new("Finalize omitted refund-wallet coordinate"))?
                .pubkey = hostile_wallet;
            transactions.push(rpc.send_expected_failure(
                "publish record: substituted refund wallet refuses",
                &[hostile],
                payer,
            )?);
        }
        let label = format!("publish record: {:?}", plan.action);
        let evidence = rpc.send(&label, &[instruction], payer)?;
        minimum_slot = evidence.slot;
        transactions.push(evidence);
    }
    Err(Error::new(
        "record publication exceeded its bounded transition count",
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_product_graph(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    compiled: CompiledProductRecordsV2,
    product: &[u8],
    result_domain: &[u8],
    portfolio: &[u8],
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<(PublishedRecord, PublishedRecord, PublishedRecord)> {
    let content =
        product_publication_content_v2(registry, compiled, product, result_domain, portfolio)
            .map_err(|error| Error::new(format!("Product publication graph: {error:?}")))?;
    let coordinates = product_publication_coordinates(registry, content)?;
    let mut minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .unwrap_or(rpc.finalized_slot()?);
    for _ in 0..3072 {
        let keys = [
            payer.pubkey(),
            coordinates[0].0,
            coordinates[0].1,
            coordinates[1].0,
            coordinates[1].1,
            coordinates[2].0,
            coordinates[2].1,
            system_program::ID,
            sysvar::rent::ID,
            sysvar::clock::ID,
        ];
        let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
        let observations = publication_observations(slot, &keys, &values)?;
        let state = |raw, staging| RecordPublicationStateV1 {
            sponsor: observations[0],
            raw_record: observations[raw],
            staging_cursor: observations[staging],
            system_program: observations[7],
            rent: observations[8],
            clock: observations[9],
        };
        let plan = build_product_publication_step_v2(
            registry,
            content,
            ProductPublicationStateV2 {
                product: state(1, 2),
                result_domain: state(3, 4),
                portfolio: state(5, 6),
            },
        )
        .map_err(|error| Error::new(format!("chain-derived Product publication: {error:?}")))?;
        if plan.member == ProductPublicationMemberV2::Complete {
            for ((raw, staging, _), body) in
                coordinates
                    .iter()
                    .copied()
                    .zip([product, result_domain, portfolio])
            {
                verify_published_record(rpc, registry, raw, staging, body)?;
            }
            let published = |index: usize, schema| PublishedRecord {
                schema,
                digest: coordinates[index].2,
                raw: coordinates[index].0,
                staging: coordinates[index].1,
            };
            return Ok((
                published(
                    0,
                    dclutch_product_runtime_v2_admission::PRODUCT_RECORD_SCHEMA_ID_V2,
                ),
                published(
                    1,
                    dclutch_product_runtime_v2_admission::RESULT_DOMAIN_SCHEMA_ID_V2,
                ),
                published(
                    2,
                    dclutch_product_runtime_v2_admission::PORTFOLIO_SCHEMA_ID_V2,
                ),
            ));
        }
        let instruction = plan
            .record
            .instruction
            .ok_or_else(|| Error::new("incomplete Product publication omitted instruction"))?;
        let label = format!(
            "publish Product graph: {:?} {:?}",
            plan.member, plan.record.action
        );
        let evidence = rpc.send(&label, &[instruction], payer)?;
        minimum_slot = evidence.slot;
        transactions.push(evidence);
    }
    Err(Error::new(
        "Product graph publication exceeded its bounded transition count",
    ))
}

fn product_publication_coordinates(
    registry: Pubkey,
    content: ProductPublicationContentV2<'_>,
) -> Result<[(Pubkey, Pubkey, [u8; 32]); 3]> {
    Ok([
        derive_record_addresses_v1(registry, content.product)
            .map_err(|error| Error::new(format!("Product record address: {error:?}")))?,
        derive_record_addresses_v1(registry, content.result_domain)
            .map_err(|error| Error::new(format!("domain record address: {error:?}")))?,
        derive_record_addresses_v1(registry, content.portfolio)
            .map_err(|error| Error::new(format!("portfolio record address: {error:?}")))?,
    ])
}

fn publication_observations<'a, const N: usize>(
    slot: u64,
    keys: &[Pubkey; N],
    values: &'a [Option<crate::rpc::RpcAccount>],
) -> Result<[AccountObservationV2<'a>; N]> {
    let observations = keys
        .iter()
        .copied()
        .zip(values)
        .map(|(key, value)| match value {
            Some(account) => AccountObservationV2 {
                slot,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: &account.data,
            },
            None => AccountObservationV2 {
                slot,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: &[],
            },
        })
        .collect::<Vec<_>>();
    observations
        .try_into()
        .map_err(|_| Error::new("publication snapshot width changed"))
}

fn verify_published_record(
    rpc: &mut Rpc,
    registry: Pubkey,
    raw: Pubkey,
    staging: Pubkey,
    content: &[u8],
) -> Result<()> {
    let finalized = rpc.required_account(raw, "finalized Registry record")?;
    if finalized.owner != registry
        || finalized.executable
        || finalized.data != content
        || finalized.lamports < rpc.minimum_balance(content.len())?
        || rpc.account(staging)?.is_some()
    {
        return Err(Error::new("finalized Registry record poststate mismatch"));
    }
    Ok(())
}

fn verify_profile(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    let address = pubkey(&plan.infrastructure_profile.address)?;
    let account = rpc.required_account(address, "infrastructure profile")?;
    let expected = decode_hex(&plan.infrastructure_profile.body_hex)?;
    if account.owner != pubkey(&plan.core.program_id)?
        || account.executable
        || account.data != expected
        || hex(&sha2::Sha256::digest(&account.data)) != plan.infrastructure_profile.body_sha256
        || ProtocolInfrastructureProfileV1::decode(&account.data).is_err()
    {
        return Err(Error::new(
            "Core infrastructure profile poststate did not match exact plan bytes",
        ));
    }
    Ok(())
}

fn verify_core_programdata(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    let address = pubkey(&plan.core.programdata_id)?;
    let account = rpc.required_account(address, "Core ProgramData")?;
    if account.owner != bpf_loader_upgradeable::ID
        || account.executable
        || account.data.get(..4) != Some(3_u32.to_le_bytes().as_slice())
        || account.data.get(12) != Some(&0)
        || account.data.get(13..45)
            != Some(pubkey(&plan.core_bootstrap.upgrade_authority)?.as_ref())
        || hex(&sha2::Sha256::digest(&account.data))
            != plan.core_bootstrap.post_revoke_programdata_sha256
    {
        return Err(Error::new(
            "Core Loader-v3 ProgramData was not the exact authority-None poststate",
        ));
    }
    Ok(())
}

fn verify_activation(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    let address = pubkey(&plan.activation)?;
    let account = rpc.required_account(address, "release activation")?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&account.data)
        .map_err(|error| Error::new(format!("decode release activation: {error:?}")))?;
    let release_set_id = view
        .execution_release_set_id()
        .map_err(|error| Error::new(format!("activation release-set ID: {error:?}")))?;
    if account.owner != pubkey(&plan.registry.program_id)?
        || account.executable
        || hex(release_set_id.as_bytes()) != plan.release_set_id
    {
        return Err(Error::new(
            "Registry activation poststate did not match the exact release set",
        ));
    }
    Ok(())
}

fn validate_spec(spec: &SuccessorRunSpec) -> Result<()> {
    if spec.schema != RUN_SPEC_SCHEMA_V2 {
        return Err(Error::new("unsupported successor run-spec schema"));
    }
    crate::market::validate_market_input(&spec.market)?;
    let rpc = validate_loopback_url(&spec.rpc_url)?;
    if rpc.as_str() != EXPECTED_RPC_URL {
        return Err(Error::new(format!(
            "successor launcher is pinned to exact RPC origin {EXPECTED_RPC_URL}"
        )));
    }
    validate_existing_canonical_file(Path::new(&spec.launcher), "launcher")?;
    for (label, input) in [
        ("registry", &spec.registry),
        ("core", &spec.core),
        ("claims", &spec.claims),
        ("trading", &spec.trading),
        ("resolution", &spec.resolution),
        ("custody", &spec.custody),
        ("rent-credit", &spec.rent_credit),
    ] {
        let _ = pubkey(&input.program_id)?;
        let _ = hex32(&input.elf_sha256)?;
        let _ = hex32(&input.semantic_release_id)?;
        validate_existing_canonical_file(Path::new(&input.elf_path), &format!("{label} ELF"))?;
        validate_existing_canonical_file(
            Path::new(&input.attestation),
            &format!("{label} attestation"),
        )?;
    }
    validate_program_ids(&[
        pubkey(&spec.registry.program_id)?,
        pubkey(&spec.core.program_id)?,
        pubkey(&spec.claims.program_id)?,
        pubkey(&spec.trading.program_id)?,
        pubkey(&spec.resolution.program_id)?,
        pubkey(&spec.custody.program_id)?,
        pubkey(&spec.rent_credit.program_id)?,
    ])?;
    for (path, label) in [
        (&spec.ledger, "ledger"),
        (&spec.account_dir, "account_dir"),
        (&spec.plan, "plan"),
        (&spec.output, "output"),
    ] {
        validate_canonical_new_path(Path::new(path), label)?;
    }
    let mut outputs = [&spec.ledger, &spec.account_dir, &spec.plan, &spec.output];
    outputs.sort();
    if outputs.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::new("successor output paths must be distinct"));
    }
    let log = validator_log_path(spec)?;
    validate_canonical_new_path(&log, "validator_log")?;
    if outputs.iter().any(|path| Path::new(path) == log) {
        return Err(Error::new(
            "validator log must be distinct from every requested output path",
        ));
    }
    Ok(())
}

fn ensure_rpc_port_free() -> Result<()> {
    let address: SocketAddr = "127.0.0.1:20890"
        .parse()
        .map_err(|error| Error::new(format!("internal RPC socket: {error}")))?;
    match TcpStream::connect_timeout(&address, Duration::from_millis(250)) {
        Ok(_) => Err(Error::new(
            "refusing to launch while another process listens on 127.0.0.1:20890",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => Ok(()),
        Err(error) => Err(Error::new(format!(
            "could not prove successor RPC port is free: {error}"
        ))),
    }
}

fn validate_plan(plan: &SuccessorPlan) -> Result<()> {
    if plan.schema != "dclutch-local-successor-infrastructure-plan-v2"
        || plan.genesis_boundary.len() != 2
        || plan.bootstrap_order.len() != 5
        || plan.execution_blocker.is_empty()
    {
        return Err(Error::new("invalid successor infrastructure plan header"));
    }
    let programs = [
        pubkey(&plan.registry.program_id)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.claims.program_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.custody.program_id)?,
        pubkey(&plan.rent_credit.program_id)?,
    ];
    validate_program_ids(&programs)?;
    let core_authority = pubkey(&plan.core_bootstrap.upgrade_authority)?;
    if core_authority == Pubkey::default()
        || programs.contains(&core_authority)
        || !plan.core_bootstrap.release_recognition_requires_revoke
        || plan.core_bootstrap.genesis_programdata_sha256
            == plan.core_bootstrap.post_revoke_programdata_sha256
    {
        return Err(Error::new(
            "Core bootstrap authority/revocation boundary is not canonical",
        ));
    }
    for (label, pin, authority) in [
        ("registry", &plan.registry, None),
        ("core", &plan.core, Some(core_authority)),
        ("claims", &plan.claims, None),
        ("trading", &plan.trading, None),
        ("resolution", &plan.resolution, None),
        ("custody", &plan.custody, None),
        ("rent-credit", &plan.rent_credit, None),
    ] {
        validate_program_pin(plan, label, pin, authority)?;
    }
    let core_programdata = plan
        .genesis_accounts
        .get("loader.core.programdata")
        .ok_or_else(|| Error::new("missing Core ProgramData genesis pin"))?;
    if core_programdata.data_sha256 != plan.core_bootstrap.genesis_programdata_sha256 {
        return Err(Error::new(
            "Core genesis ProgramData is not the authority-bearing pre-init observation",
        ));
    }

    let profile_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &programs[1],
    )
    .0;
    if plan.infrastructure_profile.address != profile_address.to_string()
        || hex32(&plan.infrastructure_profile.schema_id)?
            != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1
    {
        return Err(Error::new(
            "infrastructure profile address or schema is not canonical",
        ));
    }
    let body = decode_hex(&plan.infrastructure_profile.body_hex)?;
    let profile = ProtocolInfrastructureProfileV1::decode(&body)
        .map_err(|error| Error::new(format!("infrastructure profile: {error:?}")))?;
    let registry_artifact = artifact_id(&plan.registry.artifact_release_id)?;
    let rent_artifact = artifact_id(&plan.rent_credit.artifact_release_id)?;
    if profile.registry().program().to_bytes() != programs[0].to_bytes()
        || profile.registry().artifact_release() != registry_artifact
        || profile.rent().program().to_bytes() != programs[6].to_bytes()
        || profile.rent().artifact_release() != rent_artifact
        || plan.infrastructure_profile.registry_artifact_release_id
            != plan.registry.artifact_release_id
        || plan.infrastructure_profile.rent_artifact_release_id
            != plan.rent_credit.artifact_release_id
    {
        return Err(Error::new(
            "infrastructure profile substituted a Registry or Rent binding",
        ));
    }
    if plan.infrastructure_profile.body_sha256 != hex(&sha2::Sha256::digest(&body)) {
        return Err(Error::new("infrastructure profile body hash mismatch"));
    }
    for label in [
        "execution_release_set",
        "registry_artifact_release",
        "core_artifact_release",
        "claims_artifact_release",
        "trading_artifact_release",
        "resolution_artifact_release",
        "custody_artifact_release",
        "rent_artifact_release",
        "pyth_release",
    ] {
        let _ = record(plan, label)?;
    }
    if plan.genesis_accounts.len() != 23 {
        return Err(Error::new(
            "infrastructure plan must contain fourteen Loader and nine finalized record accounts",
        ));
    }
    Ok(())
}

fn validate_program_pin(
    plan: &SuccessorPlan,
    label: &str,
    pin: &ProgramPin,
    bootstrap_authority: Option<Pubkey>,
) -> Result<()> {
    let program = pubkey(&pin.program_id)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if pubkey(&pin.programdata_id)? != expected_programdata
        || pin.upgrade_authority.is_some()
        || !PathBuf::from(&pin.elf_path).is_absolute()
    {
        return Err(Error::new(
            "program pin is not an immutable canonical Loader-v3 binding",
        ));
    }
    let expected_elf = hex32(&pin.elf_sha256)?;
    let _ = hex32(&pin.semantic_release_id)?;
    let _ = artifact_id(&pin.artifact_release_id)?;
    let elf = fs::read(&pin.elf_path)?;
    if sha2::Sha256::digest(&elf).as_slice() != expected_elf {
        return Err(Error::new(format!("{label} ELF digest mismatch")));
    }
    let expected_programdata = loader_programdata_bytes(&elf, bootstrap_authority);
    let genesis = plan
        .genesis_accounts
        .get(&format!("loader.{label}.programdata"))
        .ok_or_else(|| Error::new(format!("missing {label} ProgramData genesis pin")))?;
    if genesis.data_sha256 != hex(&sha2::Sha256::digest(&expected_programdata)) {
        return Err(Error::new(format!(
            "{label} ProgramData header/ELF genesis hash mismatch"
        )));
    }
    if label == "core" {
        let post_revoke = loader_programdata_bytes_after_revoke(
            &elf,
            bootstrap_authority.ok_or_else(|| Error::new("Core bootstrap authority was absent"))?,
        );
        if plan.core_bootstrap.post_revoke_programdata_sha256
            != hex(&sha2::Sha256::digest(&post_revoke))
        {
            return Err(Error::new(
                "Core post-revoke immutable ProgramData hash mismatch",
            ));
        }
    }
    Ok(())
}

fn validate_existing_canonical_file(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("inspect {label} {}: {error}", path.display())))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(Error::new(format!(
            "{label} must be a nonsymlink regular file"
        )));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(Error::new(format!("{label} path must be canonical")));
    }
    Ok(())
}

fn validate_canonical_new_path(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() || path.exists() || fs::symlink_metadata(path).is_ok() {
        return Err(Error::new(format!(
            "{label} must be an absolute nonexistent path"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} omitted parent")))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        Error::new(format!(
            "canonicalize {label} parent {}: {error}",
            parent.display()
        ))
    })?;
    if canonical_parent != parent || !parent.is_dir() {
        return Err(Error::new(format!(
            "{label} parent must be an existing canonical directory"
        )));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new(format!("{label} omitted UTF-8 basename")))?;
    if name.is_empty() || name == "." || name == ".." {
        return Err(Error::new(format!("{label} has unsafe basename")));
    }
    Ok(())
}

fn validator_log_path(spec: &SuccessorRunSpec) -> Result<PathBuf> {
    let ledger = Path::new(&spec.ledger);
    let name = ledger
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("ledger omitted UTF-8 basename"))?;
    Ok(ledger.with_file_name(format!("{name}.validator.log")))
}

fn write_evidence(path: &Path, evidence: &SuccessorRunEvidence) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(evidence)?;
    bytes.push(b'\n');
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| Error::new(format!("create evidence {}: {error}", path.display())))?
        .write_all(&bytes)?;
    Ok(())
}

fn artifact_id(value: &str) -> Result<ArtifactReleaseIdV1> {
    ArtifactReleaseIdV1::new(hex32(value)?)
        .map_err(|error| Error::new(format!("artifact release ID: {error:?}")))
}

pub(crate) fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if value.len() & 1 == 1
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new("invalid lowercase hexadecimal bytes"));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = core::str::from_utf8(pair).map_err(|_| Error::new("non-UTF8 hex"))?;
            u8::from_str_radix(pair, 16).map_err(|_| Error::new("invalid hexadecimal byte"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    #[test]
    fn run_spec_refuses_any_persisted_private_key_field() {
        let input = serde_json::json!({
            "program_id": Pubkey::new_unique().to_string(),
            "elf_path": "/tmp/program.so",
            "elf_sha256": "11".repeat(32),
            "semantic_release_id": "22".repeat(32),
            "attestation": "/tmp/attestation.json",
            "private_key": [1, 2, 3]
        });
        assert!(serde_json::from_value::<RunProgramInput>(input).is_err());
    }

    #[test]
    fn late_failure_substitution_only_replaces_role_programdata() {
        let original = Pubkey::new_unique();
        let replacement = Pubkey::new_unique();
        let mut instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: (0..REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1)
                .map(|_| AccountMeta::new_readonly(original, false))
                .collect(),
            data: Vec::new(),
        };
        substitute_role_programdata(&mut instruction, replacement).expect("substitute");
        assert_eq!(instruction.accounts[7].pubkey, replacement);
        assert!(
            instruction
                .accounts
                .iter()
                .enumerate()
                .all(|(index, meta)| index == 7 || meta.pubkey == original)
        );
        // The retired 26-account five-role frame is refused, not reinterpreted.
        let mut retired = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: (0..26)
                .map(|_| AccountMeta::new_readonly(original, false))
                .collect(),
            data: Vec::new(),
        };
        assert!(substitute_role_programdata(&mut retired, replacement).is_err());
    }

    #[test]
    fn rpc_origin_is_the_launcher_fixed_loopback_origin() {
        assert!(validate_loopback_url(EXPECTED_RPC_URL).is_ok());
        assert!(validate_loopback_url("http://8.8.8.8:20890/").is_err());
        assert!(validate_loopback_url("https://127.0.0.1:20890/").is_err());
    }

    #[test]
    fn profile_body_hex_decoder_refuses_odd_uppercase_and_non_hex() {
        assert_eq!(decode_hex("00ff").expect("hex"), [0, 255]);
        assert!(decode_hex("0").is_err());
        assert!(decode_hex("AA").is_err());
        assert!(decode_hex("gg").is_err());
    }

    #[test]
    fn zero_artifact_identity_refuses() {
        assert!(artifact_id(&"00".repeat(32)).is_err());
    }

    #[test]
    fn agave_4_0_2_loader_revoke_retains_inactive_authority_bytes() {
        let validator_path = PathBuf::from(
            which_validator().expect("solana-test-validator 4.0.2 must be installed"),
        );
        let version = Command::new(&validator_path)
            .arg("--version")
            .output()
            .expect("query validator version");
        let version = String::from_utf8(version.stdout).expect("UTF-8 validator version");
        assert!(
            version.starts_with("solana-test-validator 4.0.2 "),
            "pinned Loader runtime changed: {version}"
        );

        let root =
            std::env::temp_dir().join(format!("dclutch-loader-revoke-{}", Pubkey::new_unique()));
        let accounts = root.join("accounts");
        let ledger = root.join("ledger");
        fs::create_dir_all(&accounts).expect("create Loader test directory");
        let authority = Keypair::new();
        let program = Pubkey::new_unique();
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let mut elf = vec![0_u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[48..52].copy_from_slice(&3_u32.to_le_bytes());
        let genesis = loader_programdata_bytes(&elf, Some(authority.pubkey()));
        let body = serde_json::json!({
            "pubkey": programdata.to_string(),
            "account": {
                "lamports": 1_000_000_000_u64,
                "data": [BASE64.encode(&genesis), "base64"],
                "owner": bpf_loader_upgradeable::ID.to_string(),
                "executable": false,
                "rentEpoch": 0_u64,
                "space": genesis.len()
            }
        });
        fs::write(
            accounts.join(format!("{programdata}.json")),
            serde_json::to_vec_pretty(&body).expect("Loader account JSON"),
        )
        .expect("write Loader account");

        let child = Command::new(&validator_path)
            .arg("--config")
            .arg("/dev/null")
            .arg("--ledger")
            .arg(&ledger)
            .arg("--account-dir")
            .arg(&accounts)
            .arg("--mint")
            .arg(system_program::ID.to_string())
            .arg("--bind-address")
            .arg("127.0.0.1")
            .arg("--rpc-port")
            .arg("22090")
            .arg("--faucet-port")
            .arg("22092")
            .arg("--gossip-port")
            .arg("22093")
            .arg("--dynamic-port-range")
            .arg("22100-22131")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn pinned Loader validator");
        let mut validator = ValidatorChild { child };
        let mut rpc =
            wait_test_rpc(&mut validator, "http://127.0.0.1:22090/").expect("pinned Loader RPC");
        rpc.airdrop("fund Loader authority", authority.pubkey(), 1_000_000_000)
            .expect("fund authority");
        rpc.send(
            "real Loader-v3 SetAuthority None",
            &[set_upgrade_authority(&program, &authority.pubkey(), None)],
            &authority,
        )
        .expect("revoke Loader authority");
        let observed = rpc
            .required_account(programdata, "revoked ProgramData")
            .expect("revoked ProgramData");
        let expected = loader_programdata_bytes_after_revoke(&elf, authority.pubkey());
        assert_eq!(observed.data, expected);
        let view = dclutch_registry_svm::ProgramDataV3View::parse(&observed.data)
            .expect("Registry parses real Loader poststate");
        assert_eq!(view.upgrade_authority(), None);
        assert_eq!(&observed.data[13..45], authority.pubkey().as_ref());
        drop(validator);
        fs::remove_dir_all(root).expect("remove scoped Loader test directory");
    }

    #[test]
    fn real_sbf_infrastructure_revoke_and_registry_activation_when_supplied() {
        let (Ok(core_elf), Ok(registry_elf), Ok(rent_elf)) = (
            std::env::var("DCLUTCH_SUCCESSOR_CORE_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_REGISTRY_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_RENT_ELF"),
        ) else {
            return;
        };
        let core_elf = fs::canonicalize(core_elf).expect("canonical Core test ELF");
        let registry_elf = fs::canonicalize(registry_elf).expect("canonical Registry test ELF");
        let rent_elf = fs::canonicalize(rent_elf).expect("canonical Rent test ELF");
        // The Found path invokes only Registry, Core, and Rent. When the real
        // Claims/Trading/Resolution/Custody artifacts are supplied the release
        // set binds them exactly; otherwise each role is a distinct immutable
        // Loader deployment of the Registry ELF, and the evidence says so.
        let role_elf = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| fs::canonicalize(value).expect("canonical role test ELF"))
                .unwrap_or_else(|| registry_elf.clone())
        };
        let claims_elf = role_elf("DCLUTCH_SUCCESSOR_CLAIMS_ELF");
        let trading_elf = role_elf("DCLUTCH_SUCCESSOR_TRADING_ELF");
        let resolution_elf = role_elf("DCLUTCH_SUCCESSOR_RESOLUTION_ELF");
        let custody_elf = role_elf("DCLUTCH_SUCCESSOR_CUSTODY_ELF");
        let authority = Keypair::new();
        let root = std::env::temp_dir().join(format!(
            "dclutch-successor-real-sbf-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("wall clock after epoch")
                .as_nanos()
        ));
        fs::create_dir(&root).expect("create real-SBF test root");
        let digest = |path: &Path| {
            hex(&sha2::Sha256::digest(
                fs::read(path).expect("read real-SBF test ELF"),
            ))
        };
        let registry_sha = digest(&registry_elf);
        let core_sha = digest(&core_elf);
        let rent_sha = digest(&rent_elf);
        let program = |tag| Pubkey::new_from_array([tag; 32]);
        let plan = crate::plan::prepare(PrepareArgs {
            account_dir: root.join("accounts"),
            plan_path: root.join("plan.json"),
            registry_program: program(0x31),
            registry_elf: registry_elf.clone(),
            registry_sha256: registry_sha.clone(),
            registry_semantic_release_id: "11".repeat(32),
            core_program: program(0x32),
            core_elf,
            core_sha256: core_sha,
            core_semantic_release_id: "12".repeat(32),
            core_bootstrap_upgrade_authority: authority.pubkey(),
            claims_program: program(0x33),
            claims_sha256: digest(&claims_elf),
            claims_elf,
            claims_semantic_release_id: "13".repeat(32),
            trading_program: program(0x34),
            trading_sha256: digest(&trading_elf),
            trading_elf,
            trading_semantic_release_id: "14".repeat(32),
            resolution_program: program(0x35),
            resolution_sha256: digest(&resolution_elf),
            resolution_elf,
            resolution_semantic_release_id: hex(
                &dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V4,
            ),
            custody_program: program(0x36),
            custody_sha256: digest(&custody_elf),
            custody_elf,
            custody_semantic_release_id: "16".repeat(32),
            rent_credit_program: program(0x37),
            rent_credit_elf: rent_elf,
            rent_credit_sha256: rent_sha,
            rent_credit_semantic_release_id: "17".repeat(32),
        })
        .expect("prepare real-SBF infrastructure plan");
        validate_plan(&plan).expect("validate real-SBF plan");

        let child = Command::new(which_validator().expect("pinned validator"))
            .arg("--config")
            .arg("/dev/null")
            .arg("--ledger")
            .arg(root.join("ledger"))
            .arg("--account-dir")
            .arg(root.join("accounts"))
            .arg("--mint")
            .arg(system_program::ID.to_string())
            .arg("--bind-address")
            .arg("127.0.0.1")
            .arg("--rpc-port")
            .arg("22290")
            .arg("--faucet-port")
            .arg("22292")
            .arg("--gossip-port")
            .arg("22293")
            .arg("--dynamic-port-range")
            .arg("22300-22331")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn real-SBF validator");
        let mut validator = ValidatorChild { child };
        let mut rpc =
            wait_test_rpc(&mut validator, "http://127.0.0.1:22290/").expect("real-SBF RPC");
        let hostile = Keypair::new();
        rpc.airdrop(
            "fund real-SBF Core authority",
            authority.pubkey(),
            AUTHORITY_LAMPORTS,
        )
        .expect("fund Core authority");
        rpc.airdrop(
            "fund real-SBF hostile authority",
            hostile.pubkey(),
            AUTHORITY_LAMPORTS,
        )
        .expect("fund hostile authority");
        rpc.send_expected_failure(
            "real-SBF wrong-authority initialization",
            &[
                initialize_instruction(&plan, hostile.pubkey(), hostile.pubkey())
                    .expect("hostile init instruction"),
            ],
            &hostile,
        )
        .expect("wrong-authority initialization refuses");
        assert!(
            rpc.account(pubkey(&plan.infrastructure_profile.address).expect("profile key"))
                .expect("profile query")
                .is_none()
        );
        rpc.send(
            "real-SBF infrastructure init",
            &[
                initialize_instruction(&plan, authority.pubkey(), authority.pubkey())
                    .expect("init instruction"),
            ],
            &authority,
        )
        .expect("execute real-SBF init");
        verify_profile(&mut rpc, &plan).expect("real-SBF profile");
        rpc.send_expected_failure(
            "real-SBF activation before revoke",
            &[
                role_activation_instruction(&plan, authority.pubkey(), ExecutionRoleV1::Core)
                    .expect("pre-revoke activation"),
            ],
            &authority,
        )
        .expect("pre-revoke activation refuses");
        assert!(
            rpc.account(pubkey(&plan.activation).expect("activation key"))
                .expect("activation query")
                .is_none()
        );
        rpc.send(
            "real-SBF Core Loader revoke",
            &[set_upgrade_authority(
                &pubkey(&plan.core.program_id).expect("Core program"),
                &authority.pubkey(),
                None,
            )],
            &authority,
        )
        .expect("real-SBF Core revoke");
        verify_core_programdata(&mut rpc, &plan).expect("real-SBF Core poststate");
        for (label, instruction) in
            activation_instructions(&plan, authority.pubkey()).expect("activation walk-up")
        {
            rpc.send(label, &[instruction], &authority)
                .expect("real-SBF role activation succeeds");
        }
        verify_activation(&mut rpc, &plan).expect("real-SBF activation cache");

        let mut publication_transactions = Vec::new();
        let published = publish_record(
            &mut rpc,
            pubkey(&plan.registry.program_id).expect("Registry program"),
            &authority,
            [0x61; 32],
            b"transaction-produced successor Registry record",
            Some(hostile.pubkey()),
            &mut publication_transactions,
        )
        .expect("real-SBF record publication");
        assert_eq!(published.schema, [0x61; 32]);
        assert_eq!(
            published.digest,
            <[u8; 32]>::from(sha2::Sha256::digest(
                b"transaction-produced successor Registry record"
            ))
        );
        assert!(publication_transactions.len() >= 4);

        let market_input = crate::market::demo_market_input(
            pubkey(&plan.registry.program_id).expect("Registry program"),
        )
        .expect("canonical demo market input");
        let market_evidence = crate::market::execute_found_market(
            &mut rpc,
            &plan,
            &market_input,
            &authority,
            &mut publication_transactions,
        )
        .expect("real-SBF RentV2 and Found31 campaign");
        assert!(market_evidence.accounts.contains_key("market"));
        assert!(
            market_evidence
                .accounts
                .contains_key("lifecycle_rent_credit")
        );

        let recipient = Pubkey::new_unique();
        let authority_before = rpc
            .required_account(authority.pubkey(), "real-SBF authority")
            .expect("authority prestate");
        let mut late_activation =
            role_activation_instruction(&plan, authority.pubkey(), ExecutionRoleV1::Custody)
                .expect("late activation");
        substitute_role_programdata(
            &mut late_activation,
            pubkey(&plan.core.programdata_id).expect("substitution key"),
        )
        .expect("substitute late Custody ProgramData");
        let late_failure = rpc
            .send_expected_failure(
                "real-SBF late substituted activation",
                &[
                    transfer(&authority.pubkey(), &recipient, 1),
                    late_activation,
                ],
                &authority,
            )
            .expect("late activation refuses");
        assert!(rpc.account(recipient).expect("recipient query").is_none());
        let fee = late_failure.fee_lamports.expect("late failure fee");
        let authority_after = rpc
            .required_account(authority.pubkey(), "real-SBF authority")
            .expect("authority poststate");
        assert_eq!(
            authority_after.lamports.checked_add(fee),
            Some(authority_before.lamports)
        );
        verify_profile(&mut rpc, &plan).expect("profile survived late rollback");
        verify_core_programdata(&mut rpc, &plan).expect("Core survived late rollback");
        verify_activation(&mut rpc, &plan).expect("activation survived late rollback");
        drop(validator);
        fs::remove_dir_all(root).expect("remove scoped real-SBF test directory");
    }

    fn which_validator() -> Option<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg("command -v solana-test-validator")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()
            .map(|value| value.trim().into())
    }

    fn wait_test_rpc(validator: &mut ValidatorChild, url: &str) -> Result<Rpc> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = validator.child.try_wait()? {
                return Err(Error::new(format!(
                    "pinned Loader validator exited before readiness: {status}"
                )));
            }
            if let Ok(rpc) = Rpc::connect(url) {
                return Ok(rpc);
            }
            if Instant::now() >= deadline {
                return Err(Error::new("pinned Loader validator readiness timeout"));
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
}
