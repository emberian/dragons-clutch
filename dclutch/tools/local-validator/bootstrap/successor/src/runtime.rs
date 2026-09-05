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

use dclutch_core_contract::ContentId;
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    registry::{RegistryReauthenticationState, build_registry_reauthentication_v1},
};
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, CompiledProductRecordsV2,
    publication::{
        ProductPublicationContentV2, ProductPublicationMemberV2, ProductPublicationStateV2,
        RecordPublicationActionV1, RecordPublicationContentV1, RecordPublicationStateV1,
        build_product_publication_step_v2, build_record_publication_step_v1,
        derive_record_addresses_v1, product_publication_content_v2,
    },
};
use dclutch_registry::record::{AbortRecordV1, RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ActivationCacheProgressV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, DeploymentObservationV1, ExecutionReleaseActivationInputsV1,
    activate_execution_release_set_v1, activation_cache_progress_v1,
};
use dclutch_registry::svm::{
    LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataMetadataV3View,
    REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1, RegistryInstructionV1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleV1,
    InitializeProtocolInfrastructureV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
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
    cluster::ClusterOriginV1,
    model::{ProgramPin, RunProgramInput, SuccessorPlan, SuccessorRunEvidence, SuccessorRunSpec},
    plan::{
        PrepareArgs, RoleDeploymentInputV1, RoleDeploymentsV1, hex, hex32,
        loader_programdata_bytes, programdata_bytes_after_revoke, pubkey, validate_program_ids,
    },
    rpc::{Rpc, account_evidence},
    seed::{KeyForge, role},
};

const RUN_SPEC_SCHEMA_V2: &str = "dclutch-local-successor-run-spec-v2";
const RUN_EVIDENCE_SCHEMA_V2: &str = "dclutch-local-successor-run-evidence-v2";
/// The historical origin, and still the default nothing has to ask for.
///
/// # Why this stopped being a constant
///
/// This used to be `EXPECTED_RPC_URL`, a hardcoded `http://127.0.0.1:20890/`
/// that `validate_spec` required a run spec to equal exactly. That made a
/// local-validator campaign a SINGLE GLOBAL SLOT on the machine: three lanes
/// contending for one socket, losing six-minute races to each other, and a
/// leaked validator blocking the whole repository's tier-1 path.
///
/// Nothing about the protocol wanted that. The origin is in no authenticated
/// material — not in the keypair derivation (`seed.rs` reads the origin only to
/// decide whether a seed is ADMISSIBLE, never to derive from it), not in a
/// program address, not in a semantic release ID, not in an artifact
/// attestation, not in the genesis plan. It was configuration wearing a
/// constant's clothes.
///
/// What the constant DID buy is real and is kept: this process must talk to THE
/// VALIDATOR IT STARTED and never to some other process that happens to answer
/// on a loopback socket. That is now enforced by TELLING rather than by
/// assuming — the spec's origin is passed down to the launcher as `--rpc-port`,
/// the port is proved free before the launch, and the healthy RPC origin is
/// compared back against the spec — instead of two sides independently
/// believing in the same magic number.
const DEFAULT_RPC_PORT: u16 = 20890;
const AUTHORITY_LAMPORTS: u64 = 5_000_000_000;
const VALIDATOR_READY_TIMEOUT: Duration = Duration::from_secs(60);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublishedRecord {
    pub(crate) schema: [u8; 32],
    pub(crate) digest: [u8; 32],
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

pub(crate) struct ValidatorChild {
    child: Child,
}

impl ValidatorChild {
    fn spawn(
        spec: &SuccessorRunSpec,
        plan: &SuccessorPlan,
        log_path: &Path,
        rpc_port: u16,
    ) -> Result<Self> {
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
            // TELL the launcher which origin to serve rather than trusting that
            // both sides independently believe in the same magic number. The
            // launcher derives its whole port block from this base.
            .arg("--rpc-port")
            .arg(rpc_port.to_string())
            // Bind the validator's lifetime to THIS process, structurally.
            //
            // `Drop` below already kills the child, and it is not enough: a
            // supervisor that is SIGKILLed never runs Drop, and the validator
            // reparents to PID 1 and outlives everything. That is not a
            // hypothetical -- on 2026-08-27 a finished campaign left a
            // validator with PPID 1 holding the one port every tier-1 run
            // needed, and the chain it held was unusable by anyone because the
            // founder key had died with the supervisor's memory.
            //
            // Given this pid the launcher starts a watchdog before it execs,
            // so the containment survives a signal this process cannot catch.
            .arg("--supervisor-pid")
            .arg(std::process::id().to_string())
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

    fn wait_for_rpc(&mut self, plan: &SuccessorPlan, rpc_url: &str) -> Result<Rpc> {
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
            if let Ok(mut rpc) = Rpc::connect(rpc_url)
                && let Ok(account) = rpc.required_account(expected_programdata, "Core ProgramData")
                && hex(&sha2::Sha256::digest(&account.data)) == *expected_hash
            {
                return Ok(rpc);
            }
            if Instant::now() >= deadline {
                return Err(Error::new(format!(
                    "successor validator at {rpc_url} did not expose the exact prepared Core \
                     ProgramData within 60 seconds"
                )));
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

/// A live campaign that has reached an OPEN Market.
///
/// The guarded validator is still running, the ephemeral Core authority is
/// still only in process memory, and every founding poststate is on chain.
/// Dropping this kills the validator, which is why the founder key and the
/// chain it founded on can only be used by a caller that holds the session:
/// nothing here is ever persisted, so a second process cannot sign as the
/// founder no matter what it reads off the ledger.
pub(crate) struct OpenMarketSessionV1 {
    /// Kept solely to own the child's lifetime; `Drop` kills the validator.
    #[allow(dead_code)]
    pub(crate) validator: ValidatorChild,
    pub(crate) rpc: Rpc,
    pub(crate) spec: SuccessorRunSpec,
    /// The founding's checked plan. This binary is finished with it by the
    /// time the session exists; a post-Open campaign is not.
    #[allow(dead_code)]
    pub(crate) plan: SuccessorPlan,
    pub(crate) plan_sha256: String,
    pub(crate) authority: Keypair,
    pub(crate) validator_log: PathBuf,
    pub(crate) transactions: Vec<crate::model::TransactionEvidence>,
    pub(crate) accounts: BTreeMap<String, crate::model::AccountEvidence>,
    pub(crate) completed: Vec<String>,
    pub(crate) founding_custody_context: String,
    pub(crate) direct_selected_manifest_entry_index: u16,
    /// The campaign's key source, kept so a post-Open campaign draws its keys
    /// from the same forge and inherits the same reproducibility.
    pub(crate) forge: KeyForge,
}

impl OpenMarketSessionV1 {
    /// Render the campaign's evidence document without ending the session.
    pub(crate) fn evidence(&self) -> SuccessorRunEvidence {
        SuccessorRunEvidence {
            schema: RUN_EVIDENCE_SCHEMA_V2.into(),
            rpc_url: self.rpc.url().into(),
            ledger: self.spec.ledger.clone(),
            validator_log: self.validator_log.display().to_string(),
            plan_sha256: self.plan_sha256.clone(),
            core_upgrade_authority_pubkey: self.authority.pubkey().to_string(),
            private_key_persisted: false,
            // A seeded campaign persists no key either -- but its keys are
            // REPRODUCIBLE from a seed somebody else holds, which is a
            // different claim from "unreproducible", and the evidence has to
            // make that difference visible rather than let one bool cover both.
            keypair_derivation: self.forge.derivation_label().into(),
            keypair_seed_sha256: self.forge.seed_sha256(),
            founding_custody_context: self.founding_custody_context.clone(),
            direct_selected_manifest_entry_index: self.direct_selected_manifest_entry_index,
            completed: self.completed.clone(),
            transactions: self.transactions.clone(),
            accounts: self.accounts.clone(),
            remaining_execution_seam: crate::market::REMAINING_OPEN_SEAM.into(),
        }
    }
}

/// Own the complete authority lifetime and leave the ledger as evidence.
pub(crate) fn execute(spec_path: &Path, keypair_seed: Option<&str>) -> Result<()> {
    let session = found_through_open(spec_path, keypair_seed)?;
    let evidence = session.evidence();
    write_evidence(Path::new(&session.spec.output), &evidence)?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Drive a fresh guarded validator from genesis to an OPEN Market.
///
/// This is the whole of the tier-1 campaign. It returns rather than writing,
/// so that a longer campaign - one that lives the Market's life after Open -
/// runs against the same validator, the same activation cache, and the same
/// in-memory founder, instead of reconstructing a founding it cannot sign for.
pub(crate) fn found_through_open(
    spec_path: &Path,
    keypair_seed: Option<&str>,
) -> Result<OpenMarketSessionV1> {
    validate_existing_canonical_file(spec_path, "--spec")?;
    let spec: SuccessorRunSpec = serde_json::from_slice(&fs::read(spec_path)?)?;
    validate_spec(&spec)?;
    // The seed gate reads the SPEC's declared origin, and it runs before any
    // key exists. `validate_spec` has already pinned that origin to the
    // launcher's loopback one; the gate does not assume that, because a
    // TEST-ONLY affordance whose safety depends on an unrelated check upstream
    // is one refactor away from not being safe at all.
    let forge = KeyForge::parse(keypair_seed, &spec.rpc_url)?;
    let (rpc_url, rpc_port) = rpc_origin(&spec.rpc_url)?;
    ensure_rpc_port_free(rpc_port)?;

    let authority = forge.keypair(role::CORE_UPGRADE_AUTHORITY);
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
    let mut validator = ValidatorChild::spawn(&spec, &plan, &validator_log, rpc_port)?;
    let mut rpc = validator.wait_for_rpc(&plan, &rpc_url)?;
    if rpc.url() != rpc_url {
        return Err(Error::new(format!(
            "healthy RPC origin changed after launch: asked for {rpc_url}, answering on {}",
            rpc.url()
        )));
    }

    let observed_slots = observe_deployment_slots(&mut rpc, &plan)?;

    let hostile = forge.keypair(role::HOSTILE_AUTHORITY);
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
    let mut publication_steps: Vec<String> = Vec::new();
    if plan.record_publication == "transaction" {
        let count = publish_infrastructure_records(&mut rpc, &plan, &authority, &mut transactions)?;
        publication_steps.push(format!(
            "published {count} infrastructure record bodies as real Registry transactions -- \
             nothing about the protocol existed at genesis"
        ));
    }

    let profile = pubkey(&plan.infrastructure_profile.address)?;
    if rpc.account(profile)?.is_some() {
        return Err(Error::new(
            "infrastructure profile unexpectedly existed at genesis",
        ));
    }
    // The same statement for the V2 domain. One instruction writes both, and
    // the program refuses a non-vacant genesis PDA, so an occupied V2 here
    // would strand the whole initialize stage on a fact the supervisor could
    // have read for free.
    let genesis_profile = pubkey(&plan.genesis_infrastructure_profile.address)?;
    if rpc.account(genesis_profile)?.is_some() {
        return Err(Error::new(
            "genesis V2 infrastructure profile unexpectedly existed at genesis",
        ));
    }
    transactions.push(
        rpc.send_expected_failure(
            "wrong authority cannot initialize infrastructure",
            &[initialize_instruction(
                &plan,
                hostile.pubkey(),
                hostile.pubkey(),
            )?],
            &hostile,
        )?
        // CoreSbfError::Infrastructure: the bootstrap's own immutability-authority
        // check refused. An account-frame refusal instead would mean the hostile
        // never reached that check.
        .refusing(0x300F)?,
    );
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
    transactions.push(
        rpc.send_expected_failure(
            "immutable release activation refuses pre-revocation Core",
            &[role_activation_instruction(
                &plan,
                authority.pubkey(),
                ExecutionRoleV1::Core,
            )?],
            &authority,
        )?
        // RegistryError::Release: the release-set admission refused because Core is
        // still mutable. RegistryError::Deployment (0x1003) is the neighbouring
        // wall -- Loader/ProgramData/slot -- and passing this probe on THAT code
        // would mean the immutability requirement was never the thing tested.
        .refusing(0x1004)?,
    );
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

    // One reauthentication per activated role, read back off the chain.
    //
    // `registry/process_reauthenticate#Reauthenticate` is the route every role
    // adapter's rule is written in: a child entered under a Registry
    // continuation cannot CPI back here at all -- the Registry is already on
    // the stack -- so the adapters read the cache directly and share this
    // function's body rather than reimplementing it. That makes this the one
    // place the shared rule can be exercised as a route rather than as a
    // subroutine, and until now nothing anywhere had ever submitted it: the
    // register read NEVER-EXECUTED while `docs/design/PACKET_LIMIT_2026_09_01.md`
    // listed it among the routes tier 1 drives.
    //
    // It is a separate transaction from the activation that precedes it, and
    // has to be: the builder is handed a finalized observation of the cache and
    // refuses a role the cache does not yet carry, so an activation and its
    // reauthentication cannot share a frame. Read against a slot no earlier
    // than the activation's, which is what makes this a read-back rather than a
    // second opinion about the same bytes.
    for (label, instruction) in
        reauthentication_instructions(&mut rpc, &plan, transactions.last().map(|t| t.slot))?
    {
        transactions.push(rpc.send(&label, &[instruction], &authority)?);
    }

    // One record published and immediately reclaimed, which is what the Abort
    // route is FOR: an abandoned record set can always be reclaimed and never
    // strands its rent or its prepaid bounty. Nothing consumes this record --
    // it exists to be abandoned -- and until now nothing in this repository had
    // ever submitted `registry/process_abort#4`, in any campaign, on any
    // substrate. Its hostile rides in front of it because an unwind route that
    // let a stranger redirect a sponsor's refund would be a worse defect than
    // the stranding it was written to prevent.
    abandon_and_reclaim_one_record(&mut rpc, &plan, &authority, &mut transactions)?;

    let rollback_recipient = crate::seed::fresh_probe_address();
    let authority_before = rpc.required_account(authority.pubkey(), "Core authority wallet")?;
    if rpc.account(rollback_recipient)?.is_some() {
        return Err(Error::new("rollback recipient unexpectedly existed"));
    }
    let mut late_activation =
        role_activation_instruction(&plan, authority.pubkey(), ExecutionRoleV1::Custody)?;
    substitute_role_programdata(&mut late_activation, pubkey(&plan.core.programdata_id)?)?;
    let late_failure = rpc
        .send_expected_failure(
            "late activation substitution rolls back prior transfer",
            &[
                transfer(&authority.pubkey(), &rollback_recipient, 1),
                late_activation,
            ],
            &authority,
        )?
        // RegistryError::Deployment: the substituted ProgramData broke the
        // Loader/ProgramData linkage. This probe substitutes a coordinate, so it
        // must die at the linkage wall and not at release admission (0x1004) --
        // the two are one apart and this case is the reason they are distinct.
        .refusing(0x1003)?;
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

    // A spec that carries no market gets a fixture compiled from its own plan.
    // `market: None` is loopback-only by construction -- `rpc_origin` above
    // already refused every origin this process does not launch itself -- and
    // it is the same two calls this file's `real_sbf_*` test makes, so the
    // path was exercised before it was supported. See `SuccessorRunSpec::market`.
    // One writable spelling per run. The market input carries the route and so
    // does the spec's fixture selector; a spec holding both would let a reader
    // of either file be wrong about which route founded the Market.
    if spec.market.is_some() && spec.founding_route.is_some() {
        return Err(Error::new(
            "a run spec that carries `market` must not also carry `founding_route`: the market \
             input already names the route, and two spellings of one selection is how a run \
             founds by a route neither of its files names",
        ));
    }
    let mut compiled;
    let market_input = match spec.market.as_ref() {
        Some(input) => input,
        None => {
            let registry = pubkey(&plan.registry.program_id)?;
            let finalized_slot = rpc.finalized_slot()?;
            let root_rent =
                rpc.minimum_balance(crate::direct_market::DIRECT_CAPABILITY_ROOT_BYTES_V1)?;
            let direct =
                crate::direct_market::DirectMarketCompilerOwnedV1::for_loopback_plan_fixture(
                    registry,
                    &plan,
                    finalized_slot,
                    root_rent,
                )?;
            compiled = crate::market::demo_market_input(registry, direct.compiler())?;
            compiled.founding_route = spec.founding_route.unwrap_or_default();
            crate::market::validate_market_input(&compiled)?;
            &compiled
        }
    };
    let market = crate::market::execute_found_market(
        &mut rpc,
        &plan,
        market_input,
        &authority,
        &forge,
        &mut transactions,
    )?;

    let mut accounts = BTreeMap::new();
    for (label, address) in [
        ("core_programdata", pubkey(&plan.core.programdata_id)?),
        ("infrastructure_profile", profile),
        ("genesis_infrastructure_profile", genesis_profile),
        ("release_activation", activation),
        ("core_authority_wallet", authority.pubkey()),
    ] {
        let account = rpc.required_account(address, label)?;
        accounts.insert(label.into(), account_evidence(address, &account));
    }
    accounts.extend(market.accounts);
    let mut completed: Vec<String> = vec![
        "generated one ephemeral Core authority in process memory".into(),
        "prepared exact public-key-only genesis plan".into(),
        "started and health-bound guarded localhost validator".into(),
        observed_slots,
        "proved wrong-authority infrastructure refusal".into(),
    ];
    // Whatever the publication mode contributed happened after the chain was
    // up and its deployment slots were read, and before the profile existed,
    // so it belongs here in the order the chain saw it.
    completed.splice(4..4, publication_steps);
    completed.extend::<Vec<String>>(vec![
        "initialized exact Core Registry/Rent infrastructure profile".into(),
        "proved release activation refuses before Core revocation".into(),
        "revoked Core Loader-v3 upgrade authority to None".into(),
        "verified exact immutable Core ProgramData poststate".into(),
        "activated exact immutable five-role release set".into(),
        "proved late-failure atomic rollback".into(),
    ]);
    completed.extend(market.completed);
    Ok(OpenMarketSessionV1 {
        validator,
        rpc,
        spec,
        plan,
        plan_sha256,
        authority,
        validator_log,
        transactions,
        accounts,
        completed,
        founding_custody_context: market.founding_custody_context,
        direct_selected_manifest_entry_index: market.direct_selected_manifest_entry_index,
        forge,
    })
}

/// The seven role pins in the canonical launcher order.
pub(crate) fn role_pins(plan: &SuccessorPlan) -> [(&'static str, &ProgramPin); 7] {
    [
        ("registry", &plan.registry),
        ("core", &plan.core),
        ("claims", &plan.claims),
        ("trading", &plan.trading),
        ("resolution", &plan.resolution),
        ("custody", &plan.custody),
        ("rent-credit", &plan.rent_credit),
    ]
}

/// Read every role's deployment slot back off the live chain, before the first
/// record body exists.
///
/// The plan decoded each slot out of a `ProgramData` account image and every
/// minted body binds it; `ArtifactReleaseV1::authenticate_deployment` refuses
/// `DeploymentSlotMismatch` on chain if that number is wrong. This step turns
/// the plan's number into an observation *of this chain*, through the same
/// `ProgramDataMetadataV3View` parse the contract itself runs — so a
/// disagreement costs a refusal here, before rent is spent, instead of at
/// activation.
///
/// It also waits out the Loader's own rule. A Loader V3 program becomes
/// executable only *after* the slot it was deployed in, so a campaign whose
/// programs claim slot `s` cannot invoke them until the chain is past `s`.
/// That rule is invisible when every slot is zero, which is exactly why it was
/// never enforced before.
fn observe_deployment_slots(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<String> {
    let mut highest = 0_u64;
    let mut observed: Vec<String> = Vec::new();
    for (label, pin) in role_pins(plan) {
        let programdata = pubkey(&pin.programdata_id)?;
        let account = rpc.required_account(programdata, label)?;
        if account.owner != bpf_loader_upgradeable::ID || account.executable {
            return Err(Error::new(format!(
                "{label} ProgramData is not a nonexecutable Loader-v3 account"
            )));
        }
        let view = ProgramDataMetadataV3View::parse(&account.data).map_err(|error| {
            Error::new(format!(
                "{label} ProgramData did not hostile-decode as Loader v3: {error:?}"
            ))
        })?;
        if view.deployment_slot() != pin.deployment_slot {
            return Err(Error::new(format!(
                "{label} deployment slot on chain is {} but every minted release body binds {}",
                view.deployment_slot(),
                pin.deployment_slot
            )));
        }
        if hex(&sha2::Sha256::digest(&account.data)) != pin.programdata_sha256 {
            return Err(Error::new(format!(
                "{label} ProgramData on chain is not the image its release was decoded from"
            )));
        }
        highest = highest.max(view.deployment_slot());
        observed.push(format!("{label}={}", view.deployment_slot()));
    }
    let deadline = Instant::now() + VALIDATOR_READY_TIMEOUT;
    loop {
        let current = rpc.finalized_slot()?;
        if current > highest {
            break;
        }
        if Instant::now() >= deadline {
            return Err(Error::new(format!(
                "chain is still at slot {current} and no Loader-v3 program is executable until after slot {highest}"
            )));
        }
        thread::sleep(Duration::from_millis(250));
    }
    Ok(format!(
        "observed seven Loader-v3 deployment slots off the live chain and matched every minted release body: {}",
        observed.join(" ")
    ))
}

fn prepare_args(spec: &SuccessorRunSpec, authority: Pubkey) -> Result<PrepareArgs> {
    Ok(PrepareArgs {
        observed_upgrade_authority: None,
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
        checked_upgrade_set: None,
        record_publication: match spec.record_publication.as_deref() {
            None => crate::plan::RecordPublicationV1::Genesis,
            Some(value) => crate::plan::RecordPublicationV1::parse(value)?,
        },
        deployments: RoleDeploymentsV1 {
            registry: role_deployment_input(&spec.registry),
            core: role_deployment_input(&spec.core),
            claims: role_deployment_input(&spec.claims),
            trading: role_deployment_input(&spec.trading),
            resolution: role_deployment_input(&spec.resolution),
            custody: role_deployment_input(&spec.custody),
            rent_credit: role_deployment_input(&spec.rent_credit),
        },
        general_accelerator: None,
    })
}

fn role_deployment_input(input: &RunProgramInput) -> RoleDeploymentInputV1 {
    RoleDeploymentInputV1 {
        observed_programdata: input.observed_programdata.as_deref().map(PathBuf::from),
        observed_programdata_bytes: None,
        expected_live_elf_sha256: input.observed_elf_sha256.clone(),
        genesis_deployment_slot: input.genesis_deployment_slot.unwrap_or(0),
        // The supervised `run` substrate is the genesis install this campaign
        // materializes and then revokes, so there is no mutable deployment for
        // a caller to declare here. Decision 0012's mutable roles arrive
        // through `prepare` for the external driver, and a plan built from them
        // is one this supervisor deliberately refuses.
        expected_upgrade_authority: None,
    }
}

pub(crate) fn initialize_instruction(
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
            // The genesis V2, committed by this same instruction at the V2
            // domain. `InitializeInfrastructureAccounts::parse` reads it third
            // -- writable, non-signer, and distinct from the V1 -- and refuses
            // the frame otherwise.
            AccountMeta::new(pubkey(&plan.genesis_infrastructure_profile.address)?, false),
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

/// Number of exact roles in the profile-1 activation walk-up.
pub(crate) const ACTIVATION_ROLE_COUNT_V1: usize = ACTIVATION_ROLES_V1.len();

/// Transaction ceiling requested by every successor campaign instruction.
pub(crate) const ACTIVATION_TRANSACTION_CU_LIMIT_V1: u64 = 1_400_000;
/// Agave's SHA-256 syscall base charge under the pinned 4.0.2 runtime.
const ACTIVATION_SHA256_BASE_CU_V1: u64 = 85;
/// Conservative allowance for every activation cost other than hashing the
/// live ELF. The largest measured residual among the five permanent roles is
/// 79,855 CU; this reserve adds the required 20,000-CU measurement tolerance
/// and more than 50,000 CU of explicit growth/noise margin.
const ACTIVATION_NON_HASH_CU_RESERVE_V1: u64 = 150_000;
/// Largest live ELF tail admitted by the size-only reachability preflight.
/// At the pinned SHA-256 schedule this consumes at most the 1.4M transaction
/// ceiling after the conservative non-hash reserve. A measured CU gate remains
/// mandatory: this bound catches impossible payloads, it does not predict a
/// candidate's actual compute consumption.
pub(crate) const MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1: u64 = 2_499_831;

/// Conservative size-only compute projection for one first-time role
/// activation. Agave charges SHA-256 at `85 + max(10, bytes / 2)` CU; Registry
/// then performs record, Loader, release, rent, and cache authentication.
pub(crate) fn activation_compute_upper_bound_v1(live_elf_bytes: u64) -> Result<u64> {
    let hash = ACTIVATION_SHA256_BASE_CU_V1
        .checked_add(10_u64.max(live_elf_bytes / 2))
        .ok_or_else(|| Error::new("activation SHA-256 compute projection overflow"))?;
    ACTIVATION_NON_HASH_CU_RESERVE_V1
        .checked_add(hash)
        .ok_or_else(|| Error::new("activation total compute projection overflow"))
}

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
        data: RegistryInstructionV1::ActivateRole(role)
            .to_bytes()
            .to_vec(),
    })
}

/// Ordered per-role activation instructions with a human label for each.
pub(crate) fn activation_instructions(
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

/// Schema identity of the record this campaign publishes in order to abandon it.
///
/// A domain digest rather than a first-party schema: the record family is a
/// content-addressed store and the Abort route reads a cursor, never a schema,
/// so borrowing a real schema id here would put a record claiming to be an
/// execution release set on the chain for two transactions. This one names
/// itself and is a member of no family.
fn abandoned_record_schema_id_v1() -> [u8; 32] {
    sha2::Sha256::digest(b"dclutch.tier1.abandoned-record-probe.v1").into()
}

/// The exact five-account Abort frame.
///
/// `AbortRecordV1` is encoded by `dclutch-registry::record` and by nothing else,
/// and until this function existed no host could build the instruction that
/// carries it -- which is the whole reason `registry/process_abort#4` had never
/// executed anywhere. The frame is spelled here rather than in an operator
/// crate because every operator crate that would be its proper home is in the
/// path-dependency closure of at least one SBF link, and a host-only builder is
/// not worth moving a program digest for under a live cohort. When that changes,
/// this belongs beside `build_record_publication_step_v1`.
///
/// Nothing here is trusted: the program re-derives both PDAs from the cursor,
/// reads the sponsor refund identity out of the cursor rather than out of this
/// frame, and requires the actor's signature whenever the contract says a
/// pre-expiry abort needs one.
fn record_abort_instruction_v1(
    registry: Pubkey,
    raw: Pubkey,
    cursor: Pubkey,
    sponsor_wallet: Pubkey,
    abort_actor: Pubkey,
) -> Instruction {
    Instruction {
        program_id: registry,
        accounts: vec![
            AccountMeta::new(raw, false),
            AccountMeta::new(cursor, false),
            AccountMeta::new(sponsor_wallet, false),
            AccountMeta::new(abort_actor, true),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ],
        data: AbortRecordV1.to_bytes().to_vec(),
    }
}

/// Publish one record's Begin, refuse a substituted refund, then reclaim it.
///
/// Three transactions and the campaign's only use of the record family's
/// liveness guarantee. The Begin is an ordinary publication and is bound by the
/// campaign's existing `publish record: Begin *` pattern; the two after it are
/// this route's own. The record is never appended to and never finalized, which
/// is the state the Abort route exists to resolve.
fn abandon_and_reclaim_one_record(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    payer: &Keypair,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    let registry = pubkey(&plan.registry.program_id)?;
    // Unique to this run's authority, so a resumed ledger cannot find the
    // record already published and read a `Complete` where a `Begin` is meant.
    let mut body = b"dclutch tier-1 abandoned record probe v1: ".to_vec();
    body.extend_from_slice(payer.pubkey().as_ref());
    let publication = RecordPublicationContentV1 {
        schema_release_id: abandoned_record_schema_id_v1(),
        content: &body,
    };
    let (raw, cursor, _digest) = derive_record_addresses_v1(registry, publication)
        .map_err(|error| Error::new(format!("derive abandoned record: {error:?}")))?;

    let minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .unwrap_or(rpc.finalized_slot()?);
    let keys = [
        payer.pubkey(),
        raw,
        cursor,
        system_program::ID,
        sysvar::rent::ID,
        sysvar::clock::ID,
    ];
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    let observations = publication_observations(slot, &keys, &values)?;
    let step = build_record_publication_step_v1(
        registry,
        publication,
        RecordPublicationStateV1 {
            sponsor: observations[0],
            raw_record: observations[1],
            staging_cursor: observations[2],
            system_program: observations[3],
            rent: observations[4],
            clock: observations[5],
        },
    )
    .map_err(|error| Error::new(format!("chain-derived abandoned Begin: {error:?}")))?;
    if step.action != RecordPublicationActionV1::Begin {
        return Err(Error::new(format!(
            "the abandoned record probe found {:?} where a vacant pair was required",
            step.action
        )));
    }
    let begin = step
        .instruction
        .ok_or_else(|| Error::new("the abandoned Begin carried no instruction"))?;
    // The same label shape every other publication uses, so the campaign's one
    // `publish record: Begin *` binding covers this one too rather than the
    // route acquiring a second owner.
    transactions.push(rpc.send(&format!("publish record: Begin {raw}"), &[begin], payer)?);

    let staged = rpc
        .account(cursor)?
        .ok_or_else(|| Error::new("the abandoned record's staging cursor was not created"))?;
    if staged.owner != registry {
        return Err(Error::new(
            "the abandoned record's staging cursor is not Registry-owned",
        ));
    }

    // The boundary this route is judged on. The cursor is the SOLE author of
    // the sponsor refund identity; a frame naming someone else's wallet must
    // die on that comparison before a single lamport moves, and the record must
    // survive it.
    let stranger = crate::seed::fresh_probe_address();
    transactions.push(
        rpc.send_expected_failure(
            "Abort refuses a substituted sponsor refund wallet",
            &[record_abort_instruction_v1(
                registry,
                raw,
                cursor,
                stranger,
                payer.pubkey(),
            )],
            payer,
        )?
        // RegistryError::Record: the record family's one refusal. It is coarse
        // -- every conjunct in `record_v1` wears it -- so this asserts the wall
        // and not the conjunct, and says so rather than implying more.
        .refusing(0x100C)?,
    );
    if rpc.account(raw)?.is_none() || rpc.account(cursor)?.is_none() {
        return Err(Error::new(
            "the refused Abort destroyed the record it was refused for",
        ));
    }

    let before = rpc.required_account(payer.pubkey(), "abandoned record sponsor")?;
    let staged_lamports = staged
        .lamports
        .checked_add(rpc.required_account(raw, "abandoned raw record")?.lamports)
        .ok_or_else(|| Error::new("abandoned record lamports overflowed"))?;
    let reclaim = rpc.send(
        "reclaim an abandoned record's rent and staging bounty (Abort)",
        &[record_abort_instruction_v1(
            registry,
            raw,
            cursor,
            payer.pubkey(),
            payer.pubkey(),
        )],
        payer,
    )?;
    let fee = reclaim
        .fee_lamports
        .ok_or_else(|| Error::new("the Abort transaction omitted its exact fee"))?;
    transactions.push(reclaim);

    if rpc.account(raw)?.is_some() || rpc.account(cursor)?.is_some() {
        return Err(Error::new(
            "the Abort left one of the two record accounts behind",
        ));
    }
    // Sponsor and actor are the same wallet on an early abort, so the whole
    // balance returns to it and the only net movement is the fee. Checked here
    // rather than asserted in a witness, because a witness reads a document and
    // this reads the chain.
    let after = rpc.required_account(payer.pubkey(), "abandoned record sponsor")?;
    let expected = before
        .lamports
        .checked_add(staged_lamports)
        .and_then(|total| total.checked_sub(fee))
        .ok_or_else(|| Error::new("abandoned record refund arithmetic overflowed"))?;
    if after.lamports != expected {
        return Err(Error::new(format!(
            "the Abort returned {} lamports where the two closed accounts held {staged_lamports} \
             and the fee was {fee}",
            after.lamports.saturating_sub(before.lamports),
        )));
    }
    Ok(())
}

/// Ordered per-role reauthentication instructions, built from the chain.
///
/// The exact read-only three-account frame `build_registry_reauthentication_v1`
/// ships and nothing in this repository had ever submitted. Every input is an
/// account read at ONE finalized observation at or after `minimum_slot`, and
/// the builder refuses before any send if the cache does not carry the role, if
/// the four accounts disagree about their slot, if two of them alias, or if the
/// deployment the cache pinned is not the deployment the chain now shows -- so a
/// label reaching the campaign means the frame was authenticated off the chain
/// first and the transaction is the second opinion, not the first.
fn reauthentication_instructions(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    minimum_slot: Option<u64>,
) -> Result<Vec<(String, Instruction)>> {
    let registry_program = pubkey(&plan.registry.program_id)?;
    let cache = pubkey(&plan.activation)?;
    let mut ordered = Vec::with_capacity(ACTIVATION_ROLES_V1.len());
    for role in ACTIVATION_ROLES_V1 {
        let pin = match role {
            ExecutionRoleV1::Core => &plan.core,
            ExecutionRoleV1::Claims => &plan.claims,
            ExecutionRoleV1::Trading => &plan.trading,
            ExecutionRoleV1::Resolution => &plan.resolution,
            ExecutionRoleV1::Custody => &plan.custody,
        };
        let keys = [
            registry_program,
            cache,
            pubkey(&pin.program_id)?,
            pubkey(&pin.programdata_id)?,
        ];
        let (slot, accounts) = rpc.finalized_accounts(&keys, minimum_slot.unwrap_or(0))?;
        let observation = Observation {
            slot,
            unix_timestamp: rpc.block_time(slot)?,
            finality: Finality::Finalized,
        };
        let mut observed = Vec::with_capacity(keys.len());
        for (key, account) in keys.iter().zip(accounts) {
            // Every one of the four must exist. A vacant address here is not a
            // real observation the way the lineage record's is: the cache was
            // just written and the role programs were deployed at genesis, so
            // an absence is a defect and is named rather than projected as an
            // empty System account the builder would refuse three layers down.
            let account = account.ok_or_else(|| {
                Error::new(format!(
                    "reauthentication input {key} is absent at slot {slot}"
                ))
            })?;
            observed.push(ObservedAccount {
                observation,
                key: *key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            });
        }
        let mut observed = observed.into_iter();
        let state = RegistryReauthenticationState {
            registry_program: observed.next().expect("registry program"),
            cache: observed.next().expect("activation cache"),
            role_program: observed.next().expect("role program"),
            role_programdata: observed.next().expect("role ProgramData"),
        };
        let report = build_registry_reauthentication_v1(&state, role).map_err(|error| {
            Error::new(format!(
                "chain-derived {role:?} reauthentication frame: {error:?}"
            ))
        })?;
        // The label is a BINDING KEY read by tools/gauntlet/tier1/bindings.json.
        ordered.push((
            format!("reauthenticate the activated role: {role:?}"),
            report.instruction,
        ));
    }
    Ok(ordered)
}

/// Ordered instructions for roles that the exact observed activation cache has
/// not admitted yet. A missing cache starts all five; an exact partial starts
/// only its zero role slots; a mismatched cache refuses before any send.
pub(crate) fn pending_activation_instructions(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    payer: Pubkey,
) -> Result<Vec<(&'static str, Instruction)>> {
    let progress = activation_progress(rpc, plan)?;
    activation_instructions(plan, payer).map(|instructions| {
        instructions
            .into_iter()
            .zip(ACTIVATION_ROLES_V1)
            .filter_map(|((label, instruction), role)| {
                activation_role_is_pending(progress, role).then_some((label, instruction))
            })
            .collect()
    })
}

fn activation_role_is_pending(
    progress: Option<ActivationCacheProgressV1>,
    role: ExecutionRoleV1,
) -> bool {
    !progress
        .map(|progress| progress.is_written(role))
        .unwrap_or(false)
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

/// Publish the nine infrastructure record bodies with real transactions.
///
/// This is the only path a cluster can take. A local validator can be handed
/// finalized raw-record accounts at genesis; devnet cannot be handed anything.
/// Every body here therefore goes through the same permissionless Registry
/// `Begin -> Append -> Finalize` state machine that every market record uses,
/// paying rent from `sponsor`, and each resulting account is required to land
/// at exactly the coordinate the plan derived offline.
///
/// Order matters: Core's infrastructure initialization reads the Registry and
/// Rent artifact records, and role activation reads the five role records plus
/// the release set, so all nine must be finalized before either runs.
pub(crate) fn publish_infrastructure_records(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    sponsor: &Keypair,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<usize> {
    let registry = pubkey(&plan.registry.program_id)?;
    let mut published = 0_usize;
    for (label, pair) in &plan.records {
        let (expected_raw, expected_staging) = record(plan, label)?;
        let schema = hex32(&pair.schema_id)?;
        let body = decode_hex(&pair.body_hex)?;
        if hex(&sha2::Sha256::digest(&body)) != pair.content_sha256 {
            return Err(Error::new(format!(
                "record {label} body does not match its content coordinate"
            )));
        }
        let minimum_balance = rpc.minimum_balance(body.len())?;
        let raw_account = rpc.account(expected_raw)?;
        let staging_account = rpc.account(expected_staging)?;
        if existing_finalized_record_is_exact(
            registry,
            raw_account.as_ref(),
            staging_account.as_ref(),
            &body,
            minimum_balance,
        )? {
            continue;
        }
        // Raw absent includes both a vacant pair and an in-flight staging
        // cursor. `publish_record` snapshots both accounts and asks the shared
        // publication contract for Begin/Append/Finalize/Complete, so an exact
        // partial resumes and any substituted cursor refuses.
        let record = publish_record(rpc, registry, sponsor, schema, &body, None, transactions)?;
        if record.raw != expected_raw || hex(&record.digest) != pair.content_sha256 {
            return Err(Error::new(format!(
                "published record {label} did not land at its derived coordinate"
            )));
        }
        published += 1;
    }
    Ok(published)
}

/// Admit an already-finalized record only when its full poststate is exact.
/// Raw absence is not itself an error: a staging cursor may hold a legitimate
/// partial, and the chain-derived publication state machine decides that case.
pub(crate) fn existing_finalized_record_is_exact(
    registry: Pubkey,
    raw: Option<&crate::rpc::RpcAccount>,
    staging: Option<&crate::rpc::RpcAccount>,
    content: &[u8],
    minimum_balance: u64,
) -> Result<bool> {
    // A live staging cursor means publication has not finalized yet, even if
    // every raw byte has already been appended. Route the pair through the
    // contract state machine: it will select Append or Finalize for an exact
    // cursor and refuse any substituted owner, coordinate, sponsor, length,
    // page index, offset, or already-written prefix.
    if staging.is_some() {
        return Ok(false);
    }
    let Some(raw) = raw else {
        return Ok(false);
    };
    if raw.owner != registry
        || raw.executable
        || raw.data != content
        || raw.lamports < minimum_balance
    {
        return Err(Error::new(
            "existing finalized Registry record did not match exact plan poststate",
        ));
    }
    Ok(true)
}

/// Price only the rent that one exact Registry publication coordinate still
/// needs. The shared publication planner is the semantic owner for vacant,
/// partial, complete, and conflicting record pairs, so budget arithmetic
/// cannot silently disagree with the executor's resumability detector.
pub(crate) fn remaining_record_publication_rent(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    label: &str,
    sponsor: Pubkey,
) -> Result<u64> {
    let registry = pubkey(&plan.registry.program_id)?;
    let (raw, staging) = record(plan, label)?;
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("plan omitted record {label}")))?;
    let schema = hex32(&pair.schema_id)?;
    let body = decode_hex(&pair.body_hex)?;
    if hex(&sha2::Sha256::digest(&body)) != pair.content_sha256 {
        return Err(Error::new(format!(
            "record {label} body does not match its content coordinate"
        )));
    }
    let keys = [
        sponsor,
        raw,
        staging,
        system_program::ID,
        sysvar::rent::ID,
        sysvar::clock::ID,
    ];
    let (slot, values) = rpc.finalized_accounts(&keys, 0)?;
    let mut observations = publication_observations(slot, &keys, &values)?;
    // Budgeting must report a shortfall rather than letting the planner's
    // sponsor-balance admission hide the amount required. Identity, owner,
    // data, and the exact refund destination remain those observed above.
    observations[0].lamports = u64::MAX;
    let state = RecordPublicationStateV1 {
        sponsor: observations[0],
        raw_record: observations[1],
        staging_cursor: observations[2],
        system_program: observations[3],
        rent: observations[4],
        clock: observations[5],
    };
    let publication = RecordPublicationContentV1 {
        schema_release_id: schema,
        content: &body,
    };
    remaining_record_publication_rent_from_state(registry, publication, state, label)
}

fn remaining_record_publication_rent_from_state(
    registry: Pubkey,
    publication: RecordPublicationContentV1<'_>,
    state: RecordPublicationStateV1<'_>,
    label: &str,
) -> Result<u64> {
    let publication =
        build_record_publication_step_v1(registry, publication, state).map_err(|error| {
            Error::new(format!(
                "record {label} publication coordinate conflicts with the exact plan: {error:?}"
            ))
        })?;
    Ok(publication.sponsor_debit)
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
            transactions.push(
                rpc.send_expected_failure(
                    "publish record: substituted refund wallet refuses",
                    &[hostile],
                    payer,
                )?
                // RegistryError::Record: the immutable-record publication refused
                // the substituted refund coordinate. A record whose address is the
                // hash of its own body has many ways to refuse; this pins that the
                // refund wallet was the one that did it.
                .refusing(0x100C)?,
            );
        }
        // The raw record address keeps every publication row's label distinct
        // across the many records one campaign publishes; classifiers match
        // only the "publish record: " prefix.
        let label = format!("publish record: {:?} {raw}", plan.action);
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
                    dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
                ),
                published(
                    1,
                    dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
                ),
                published(
                    2,
                    dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
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

pub(crate) fn verify_profile(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    let core = pubkey(&plan.core.program_id)?;
    let address = pubkey(&plan.infrastructure_profile.address)?;
    let account = rpc.required_account(address, "infrastructure profile")?;
    let expected = decode_hex(&plan.infrastructure_profile.body_hex)?;
    if account.owner != core
        || account.executable
        || account.data != expected
        || hex(&sha2::Sha256::digest(&account.data)) != plan.infrastructure_profile.body_sha256
        || ProtocolInfrastructureProfileV1::decode(&account.data).is_err()
    {
        return Err(Error::new(
            "Core infrastructure profile poststate did not match exact plan bytes",
        ));
    }
    // Both, in the same verification, because one instruction wrote both. A
    // cohort standing with a V1 nothing reads and no V2 to found against is
    // exactly the state `c60b25e8` exists to make unreachable, and a
    // verification that looked only at the V1 would report it as complete.
    let genesis_address = pubkey(&plan.genesis_infrastructure_profile.address)?;
    let genesis_account =
        rpc.required_account(genesis_address, "genesis infrastructure profile")?;
    let genesis_expected = decode_hex(&plan.genesis_infrastructure_profile.body_hex)?;
    let genesis_decoded = ProtocolInfrastructureProfileV2::decode(&genesis_account.data)
        .map_err(|error| Error::new(format!("genesis infrastructure profile: {error:?}")))?;
    if genesis_account.owner != core
        || genesis_account.executable
        || genesis_account.data != genesis_expected
        || hex(&sha2::Sha256::digest(&genesis_account.data))
            != plan.genesis_infrastructure_profile.body_sha256
        || !genesis_decoded.born_at_v2()
    {
        return Err(Error::new(
            "Core genesis V2 infrastructure profile poststate did not match exact plan bytes",
        ));
    }
    Ok(())
}

fn activation_artifact(
    plan: &SuccessorPlan,
    label: &str,
) -> Result<(ArtifactReleaseIdV1, ArtifactReleaseV1)> {
    let pin = match label {
        "core_artifact_release" => &plan.core,
        "claims_artifact_release" => &plan.claims,
        "trading_artifact_release" => &plan.trading,
        "resolution_artifact_release" => &plan.resolution,
        "custody_artifact_release" => &plan.custody,
        _ => return Err(Error::new(format!("no execution-role pin for {label}"))),
    };
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("plan omitted record {label}")))?;
    let body = decode_hex(&pair.body_hex)?;
    let digest: [u8; 32] = sha2::Sha256::digest(&body).into();
    if hex(&digest) != pair.content_sha256 {
        return Err(Error::new(format!(
            "activation record {label} body digest does not match its plan coordinate"
        )));
    }
    let release = ArtifactReleaseV1::decode(&body)
        .map_err(|error| Error::new(format!("decode {label}: {error:?}")))?;
    let release_id = ArtifactReleaseIdV1::new(digest)
        .map_err(|error| Error::new(format!("decode {label} content ID: {error:?}")))?;
    let expected_authority = pin
        .upgrade_authority
        .as_deref()
        .map(pubkey)
        .transpose()?
        .map(|authority| authority.to_bytes());
    if hex(&digest) != pin.artifact_release_id
        || release.program().to_bytes() != pubkey(&pin.program_id)?.to_bytes()
        || release.programdata() != pubkey(&pin.programdata_id)?.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.elf_digest() != hex32(&pin.live_elf_sha256)?
        || release.deployment_slot() != pin.deployment_slot
        || release.upgrade_authority() != expected_authority
    {
        return Err(Error::new(format!(
            "activation record {label} does not match its exact serialized deployment pin"
        )));
    }
    Ok((release_id, release))
}

fn activation_input(plan: &SuccessorPlan, label: &str) -> Result<ArtifactActivationInputV1> {
    let (release_id, release) = activation_artifact(plan, label)?;
    let loader = release.loader_program().to_bytes();
    let programdata = release.programdata();
    let deployment = DeploymentObservationV1::new(
        release.program().to_bytes(),
        loader,
        true,
        programdata,
        loader,
        false,
        programdata,
        loader,
        release.deployment_slot(),
        release.elf_digest(),
        release.upgrade_authority(),
    )
    .map_err(|error| Error::new(format!("construct {label} projection: {error:?}")))?;
    Ok(ArtifactActivationInputV1::new(
        release_id, release, deployment,
    ))
}

/// Reconstruct the one complete activation cache this plan authorizes from its
/// finalized record bodies. This is a projection builder, not a deployment
/// authenticator: live ProgramData is admitted separately by substrate
/// preflight and again by Registry while each role transaction executes.
pub(crate) fn expected_activation(plan: &SuccessorPlan) -> Result<ActivatedExecutionReleaseSetV1> {
    let release_set_pair = plan
        .records
        .get("execution_release_set")
        .ok_or_else(|| Error::new("plan omitted execution_release_set record"))?;
    let release_set_body = decode_hex(&release_set_pair.body_hex)?;
    let release_set_digest: [u8; 32] = sha2::Sha256::digest(&release_set_body).into();
    if hex(&release_set_digest) != release_set_pair.content_sha256
        || hex(&release_set_digest) != plan.release_set_id
    {
        return Err(Error::new(
            "execution release-set body digest does not match the plan coordinate",
        ));
    }
    let release_set = ExecutionReleaseSetV1::decode(&release_set_body)
        .map_err(|error| Error::new(format!("decode execution release set: {error:?}")))?;
    let inputs = ExecutionReleaseActivationInputsV1::new(
        activation_input(plan, "core_artifact_release")?,
        activation_input(plan, "claims_artifact_release")?,
        activation_input(plan, "trading_artifact_release")?,
        activation_input(plan, "resolution_artifact_release")?,
        activation_input(plan, "custody_artifact_release")?,
    );
    let release_set_id = ContentId::new(release_set_digest)
        .map_err(|error| Error::new(format!("execution release-set ID: {error:?}")))?;
    activate_execution_release_set_v1(release_set_id, &release_set, &inputs)
        .map_err(|error| Error::new(format!("construct expected activation: {error:?}")))
}

/// Rebuild the exact Registry/Rent infrastructure profile from its saved body.
/// External campaign admission calls this without invoking the local
/// supervisor's deliberately immutable-only plan validator.
pub(crate) fn authenticate_infrastructure_profile_projection(plan: &SuccessorPlan) -> Result<()> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let rent = pubkey(&plan.rent_credit.program_id)?;
    validate_program_ids(&[registry, core, rent])?;
    let address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
    if plan.infrastructure_profile.address != address.to_string()
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
    if profile.registry().program().to_bytes() != registry.to_bytes()
        || profile.registry().artifact_release() != registry_artifact
        || profile.rent().program().to_bytes() != rent.to_bytes()
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
    authenticate_genesis_infrastructure_profile_projection(plan, profile)?;
    Ok(())
}

/// Rebuild the genesis V2 from the SAME two bindings the V1 carries.
///
/// Nothing here is taken from the plan except the pin being checked: the body
/// is a pure function of the V1's Registry and Rent bindings, so a plan whose
/// two profiles disagree -- or whose V2 came from anywhere but
/// `ProtocolInfrastructureProfileV2::genesis` -- refuses here rather than at the
/// fifteenth account of a live transaction.
fn authenticate_genesis_infrastructure_profile_projection(
    plan: &SuccessorPlan,
    v1: ProtocolInfrastructureProfileV1,
) -> Result<()> {
    let core = pubkey(&plan.core.program_id)?;
    let address =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2], &core).0;
    if plan.genesis_infrastructure_profile.address != address.to_string()
        || hex32(&plan.genesis_infrastructure_profile.schema_id)?
            != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V2
        || plan.genesis_infrastructure_profile.address == plan.infrastructure_profile.address
    {
        return Err(Error::new(
            "genesis infrastructure profile address or schema is not canonical",
        ));
    }
    let expected = ProtocolInfrastructureProfileV2::genesis(v1.registry(), v1.rent())
        .map_err(|error| Error::new(format!("genesis infrastructure profile: {error:?}")))?;
    let body = decode_hex(&plan.genesis_infrastructure_profile.body_hex)?;
    let observed = ProtocolInfrastructureProfileV2::decode(&body)
        .map_err(|error| Error::new(format!("genesis infrastructure profile: {error:?}")))?;
    if observed != expected
        || !observed.born_at_v2()
        || plan
            .genesis_infrastructure_profile
            .registry_artifact_release_id
            != plan.infrastructure_profile.registry_artifact_release_id
        || plan.genesis_infrastructure_profile.rent_artifact_release_id
            != plan.infrastructure_profile.rent_artifact_release_id
    {
        return Err(Error::new(
            "genesis infrastructure profile substituted a Registry or Rent binding",
        ));
    }
    if plan.genesis_infrastructure_profile.body_sha256 != hex(&sha2::Sha256::digest(&body)) {
        return Err(Error::new(
            "genesis infrastructure profile body hash mismatch",
        ));
    }
    Ok(())
}

/// Rebuild the activation projection from the saved record bodies and require
/// its PDA to match. External campaign admission calls this without invoking
/// the local supervisor's deliberately immutable-only plan validator.
pub(crate) fn authenticate_checked_activation_projection(plan: &SuccessorPlan) -> Result<()> {
    let expected = expected_activation(plan)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let address = Pubkey::find_program_address(
        &[
            ACTIVATION_PDA_DOMAIN_V1,
            expected.execution_release_set_id().as_bytes(),
        ],
        &registry,
    )
    .0;
    if address != pubkey(&plan.activation)? {
        return Err(Error::new(
            "activation address was not derived from the exact release-set body",
        ));
    }
    Ok(())
}

fn checked_activation_progress(
    registry: Pubkey,
    account: &crate::rpc::RpcAccount,
    expected: ActivatedExecutionReleaseSetV1,
) -> Result<ActivationCacheProgressV1> {
    if account.owner != registry || account.executable {
        return Err(Error::new(
            "activation cache did not have exact Registry-owned non-executable shape",
        ));
    }
    activation_cache_progress_v1(&account.data, expected)
        .map_err(|error| Error::new(format!("activation cache progress: {error:?}")))
}

/// Read and authenticate zero through five exact role activations. A partial
/// cache is an inert, resumable state; nonzero bytes that differ from the plan
/// are a conflict.
pub(crate) fn activation_progress(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<Option<ActivationCacheProgressV1>> {
    let expected = expected_activation(plan)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let address = pubkey(&plan.activation)?;
    let derived = Pubkey::find_program_address(
        &[
            ACTIVATION_PDA_DOMAIN_V1,
            expected.execution_release_set_id().as_bytes(),
        ],
        &registry,
    )
    .0;
    if address != derived {
        return Err(Error::new(
            "activation address was not derived from the exact release-set body",
        ));
    }
    let Some(account) = rpc.account(address)? else {
        return Ok(None);
    };
    checked_activation_progress(registry, &account, expected).map(Some)
}

/// Price exactly the activation cache that this plan derives. A valid partial
/// already owns the full rent-exempt cache and needs no second rent payment;
/// a substituted or underfunded existing account is a conflict, not a reason
/// to budget over it.
pub(crate) fn remaining_activation_rent(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<u64> {
    let expected = expected_activation(plan)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let address = Pubkey::find_program_address(
        &[
            ACTIVATION_PDA_DOMAIN_V1,
            expected.execution_release_set_id().as_bytes(),
        ],
        &registry,
    )
    .0;
    if address != pubkey(&plan.activation)? {
        return Err(Error::new(
            "activation address was not derived from the exact release-set body",
        ));
    }
    let minimum = rpc.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?;
    remaining_activation_rent_for_account(
        rpc.account(address)?.as_ref(),
        registry,
        expected,
        minimum,
    )
}

fn remaining_activation_rent_for_account(
    account: Option<&crate::rpc::RpcAccount>,
    registry: Pubkey,
    expected: ActivatedExecutionReleaseSetV1,
    minimum_balance: u64,
) -> Result<u64> {
    let Some(account) = account else {
        return Ok(minimum_balance);
    };
    checked_activation_progress(registry, account, expected)?;
    if account.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || account.lamports < minimum_balance
    {
        return Err(Error::new(
            "existing activation cache is not rent-exempt at its exact contract width",
        ));
    }
    Ok(0)
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

pub(crate) fn verify_activation(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    match activation_progress(rpc, plan)? {
        Some(progress) if progress.is_complete() => Ok(()),
        Some(progress) => Err(Error::new(format!(
            "Registry activation poststate had only {} of {} exact roles",
            progress.written_count(),
            ACTIVATION_ROLE_COUNT_V1
        ))),
        None => Err(Error::new("Registry activation poststate was absent")),
    }
}

fn validate_spec(spec: &SuccessorRunSpec) -> Result<()> {
    if spec.schema != RUN_SPEC_SCHEMA_V2 {
        return Err(Error::new("unsupported successor run-spec schema"));
    }
    // An ABSENT market is validated after it is compiled from the plan, which
    // cannot happen here: the plan does not exist until the run builds it.
    if let Some(market) = spec.market.as_ref() {
        crate::market::validate_market_input(market)?;
    }
    let _ = rpc_origin(&spec.rpc_url)?;
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
        match (
            input.observed_programdata.as_deref(),
            input.observed_elf_sha256.as_deref(),
        ) {
            (Some(_), Some(digest)) => {
                let _ = hex32(digest)?;
            }
            (Some(_), None) => {
                return Err(Error::new(format!(
                    "{label} observed ProgramData omitted its complete live ELF SHA-256"
                )));
            }
            (None, Some(_)) => {
                return Err(Error::new(format!(
                    "{label} live ELF SHA-256 was supplied without observed ProgramData"
                )));
            }
            (None, None) => {}
        }
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

/// The campaign's own RPC origin, normalized, with the port it names.
///
/// The rule itself now lives in [`crate::cluster`], which owns "which origin is
/// this run allowed to use" for the whole tool. This function is the
/// *supervisor's* narrower question on top of it: the supervisor starts a
/// validator, so its origin must be the loopback one, and it needs that
/// origin's port to tell the launcher which block to derive.
///
/// Passing `None` for the acknowledgment is what makes this narrow. A run spec
/// has no field that could carry the devnet acknowledgment, and adding one
/// would make the supervisor — the thing that launches a validator and airdrops
/// to it — reachable from a file rather than from a command line somebody typed.
fn rpc_origin(rpc_url: &str) -> Result<(String, u16)> {
    let origin = ClusterOriginV1::parse(rpc_url, None)?;
    if !origin.may_launch_validator() {
        return Err(Error::new(format!(
            "the successor supervisor launches and owns a validator, so its origin must be \
             loopback; {} is {}. The `campaign` subcommand is the entry that may leave this \
             machine, because it launches nothing.",
            origin.redacted_url(),
            origin.label()
        )));
    }
    let port = origin
        .loopback_port()
        .ok_or_else(|| Error::new("a validator-launching origin must name an explicit port"))?;
    Ok((origin.url().to_owned(), port))
}

fn ensure_rpc_port_free(port: u16) -> Result<()> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpStream::connect_timeout(&address, Duration::from_millis(250)) {
        Ok(_) => Err(Error::new(format!(
            "refusing to launch while another process listens on 127.0.0.1:{port}. Pick a free \
             base with the run spec's rpc_url: this campaign is no longer pinned to \
             {DEFAULT_RPC_PORT} and N campaigns can share one machine on disjoint bases."
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => Ok(()),
        Err(error) => Err(Error::new(format!(
            "could not prove successor RPC port {port} is free: {error}"
        ))),
    }
}

fn validate_plan(plan: &SuccessorPlan) -> Result<()> {
    crate::model::authenticate_successor_plan_schema_v3(&plan.schema)?;
    if plan.genesis_boundary.len() != 2
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

    authenticate_infrastructure_profile_projection(plan)?;
    // The accelerator's row is present exactly when the plan carries its pin,
    // and the two must agree: a record with no pin is a body nothing observed,
    // and a pin with no record is a publication that will never be finalized --
    // which under `90a8563f` is the same as never being observed at all.
    let accelerator_record = plan
        .records
        .contains_key("general_accelerator_artifact_release");
    if accelerator_record != plan.general_accelerator.is_some() {
        return Err(Error::new(
            "the General accelerator's ArtifactRelease record and its plan pin must be present \
             together or absent together",
        ));
    }
    let mut labels = vec![
        "execution_release_set",
        "registry_artifact_release",
        "core_artifact_release",
        "claims_artifact_release",
        "trading_artifact_release",
        "resolution_artifact_release",
        "custody_artifact_release",
        "rent_artifact_release",
        "pyth_release",
    ];
    if accelerator_record {
        labels.push("general_accelerator_artifact_release");
    }
    for label in labels {
        let (raw, _) = record(plan, label)?;
        let pair = &plan.records[label];
        let body = decode_hex(&pair.body_hex)?;
        if body.is_empty() || hex(&sha2::Sha256::digest(&body)) != pair.content_sha256 {
            return Err(Error::new(format!(
                "record {label} carries a body that is not the body its coordinate commits to"
            )));
        }
        // Under transaction publication the record must NOT be at genesis: the
        // whole point is that the chain, not the fixture, produced it.
        if plan.record_publication == "transaction"
            && plan
                .genesis_accounts
                .values()
                .any(|account| account.address == raw.to_string())
        {
            return Err(Error::new(format!(
                "record {label} was genesis-injected under transaction publication"
            )));
        }
    }
    // Under genesis publication every finalized record is also injected as a
    // genesis account, so an eighth ArtifactRelease adds exactly one. Under
    // transaction publication the chain produces the record and the count does
    // not move. The numbers are pinned rather than derived on purpose: a record
    // silently dropping out of the plan is what this check exists to catch.
    //
    // NINE upgradeable programs are injected, not seven: `plan.rs` writes the
    // seven roles and then, unconditionally, the local Pyth receiver and
    // router, each as a program/programdata pair. So genesis is 18 loader
    // accounts plus 9 records = 27, and transaction is the 18 alone. Both pins
    // were four short and had been since the Pyth pair was added, invisible
    // because the ONLY caller of this validation is `run`, and `run`'s only
    // caller is the tier-1 gauntlet, which was parked at the retired
    // demo-market boundary. A pin nothing executes is not a pin. Restated
    // as arithmetic over the two populations rather than as one opaque
    // constant, so the next program pair to arrive fails here readably.
    const LOADER_ACCOUNTS: usize = 9 * 2;
    const GENESIS_RECORDS: usize = 9;
    let accelerator_accounts = usize::from(plan.general_accelerator.is_some());
    let expected_accounts = match plan.record_publication.as_str() {
        "genesis" => LOADER_ACCOUNTS + GENESIS_RECORDS + accelerator_accounts,
        "transaction" => LOADER_ACCOUNTS,
        other => {
            return Err(Error::new(format!(
                "unknown record publication mode {other}"
            )));
        }
    };
    if plan.genesis_accounts.len() != expected_accounts {
        return Err(Error::new(format!(
            "infrastructure plan under {} publication must contain exactly {expected_accounts} genesis accounts",
            plan.record_publication
        )));
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
        || pin.elf_path != pin.checked_candidate_elf_path
        || pin.elf_sha256 != pin.checked_candidate_elf_sha256
        || !PathBuf::from(&pin.checked_candidate_elf_path).is_absolute()
    {
        return Err(Error::new(
            "program pin is not an immutable canonical Loader-v3 binding",
        ));
    }
    let expected_candidate = hex32(&pin.checked_candidate_elf_sha256)?;
    let expected_live = hex32(&pin.live_elf_sha256)?;
    let _ = hex32(&pin.semantic_release_id)?;
    let _ = artifact_id(&pin.artifact_release_id)?;
    let elf = fs::read(&pin.checked_candidate_elf_path)?;
    if sha2::Sha256::digest(&elf).as_slice() != expected_candidate {
        return Err(Error::new(format!(
            "{label} checked candidate ELF digest mismatch"
        )));
    }
    let record_label = match label {
        "rent-credit" => "rent_artifact_release".to_owned(),
        other => format!("{other}_artifact_release"),
    };
    let pair = plan
        .records
        .get(&record_label)
        .ok_or_else(|| Error::new(format!("missing {record_label}")))?;
    let release = ArtifactReleaseV1::decode(&decode_hex(&pair.body_hex)?)
        .map_err(|error| Error::new(format!("decode {record_label}: {error:?}")))?;
    if release.program().to_bytes() != program.to_bytes()
        || release.programdata() != expected_programdata.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.elf_digest() != expected_live
        || release.deployment_slot() != pin.deployment_slot
        || release.upgrade_authority().is_some()
    {
        return Err(Error::new(format!(
            "{label} artifact release did not match its serialized dual-digest deployment pin"
        )));
    }
    let genesis = plan
        .genesis_accounts
        .get(&format!("loader.{label}.programdata"))
        .ok_or_else(|| Error::new(format!("missing {label} ProgramData genesis pin")))?;
    if genesis.data_sha256 != pin.programdata_sha256 {
        return Err(Error::new(format!(
            "{label} genesis ProgramData is not the image its deployment facts were decoded from"
        )));
    }
    if genesis.data_len
        != elf
            .len()
            .saturating_add(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
    {
        return Err(Error::new(format!(
            "{label} ProgramData width is not the Loader-v3 45-byte metadata plus its exact ELF"
        )));
    }
    // Only a genesis install can be regenerated from `(elf, slot, authority)`.
    // A real revoked account retains its former authority in bytes 13..45 and
    // no triple reproduces that litter, which is exactly why an observed image
    // is carried by digest instead of rebuilt.
    match pin.deployment_source.as_str() {
        "genesis-install" => {
            if expected_live != expected_candidate || pin.live_elf_padding_bytes != 0 {
                return Err(Error::new(format!(
                    "{label} genesis install did not preserve one exact unpadded ELF identity"
                )));
            }
            let expected = loader_programdata_bytes(&elf, pin.deployment_slot, bootstrap_authority);
            if genesis.data_sha256 != hex(&sha2::Sha256::digest(&expected)) {
                return Err(Error::new(format!(
                    "{label} ProgramData header/ELF genesis hash mismatch"
                )));
            }
            if label == "core" {
                let post_revoke = programdata_bytes_after_revoke(&expected)?;
                if plan.core_bootstrap.post_revoke_programdata_sha256
                    != hex(&sha2::Sha256::digest(&post_revoke))
                {
                    return Err(Error::new(
                        "Core post-revoke immutable ProgramData hash mismatch",
                    ));
                }
            }
        }
        // An observed account's poststate is not reconstructible here, and it
        // does not have to be: the supervisor reads the real ProgramData back
        // off the chain after `SetAuthority(None)` and compares it against
        // `post_revoke_programdata_sha256`, which is a stronger check than
        // rebuilding the bytes this process already believes.
        "observed-programdata-account" => {}
        other => {
            return Err(Error::new(format!(
                "{label} names an unknown deployment source {other}"
            )));
        }
    }
    if label == "core"
        && (bootstrap_authority.is_none()
            || genesis.data_sha256 != plan.core_bootstrap.genesis_programdata_sha256
            || plan.core_bootstrap.post_revoke_programdata_sha256
                == plan.core_bootstrap.genesis_programdata_sha256)
    {
        return Err(Error::new(
            "Core genesis ProgramData is not the authority-bearing pre-init observation",
        ));
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
    fn the_rpc_origin_is_any_loopback_port_and_nothing_else() {
        use crate::{
            cluster::{DEVNET_GENESIS_HASH, MAX_RPC_PORT, MIN_RPC_PORT},
            rpc::validate_loopback_url,
        };
        // The DEFAULT still resolves, byte for byte, so nothing that never
        // asked for a port notices this became a parameter.
        assert_eq!(
            rpc_origin("http://127.0.0.1:20890/").expect("default origin"),
            ("http://127.0.0.1:20890/".to_owned(), DEFAULT_RPC_PORT)
        );
        // And so does any other admissible base, which is the whole point: N
        // campaigns on one machine instead of one global slot.
        assert_eq!(
            rpc_origin("http://127.0.0.1:31890/").expect("nonstandard origin"),
            ("http://127.0.0.1:31890/".to_owned(), 31890)
        );
        assert_eq!(
            rpc_origin(&format!("http://127.0.0.1:{MIN_RPC_PORT}/"))
                .expect("the lowest admissible base")
                .1,
            MIN_RPC_PORT
        );
        assert_eq!(
            rpc_origin(&format!("http://127.0.0.1:{MAX_RPC_PORT}/"))
                .expect("the highest admissible base")
                .1,
            MAX_RPC_PORT
        );

        // What is NOT relaxed. The host rule is unchanged...
        assert!(rpc_origin("http://8.8.8.8:20890/").is_err());
        assert!(rpc_origin("https://127.0.0.1:20890/").is_err());
        assert!(rpc_origin("http://127.0.0.1:20890/path").is_err());
        assert!(rpc_origin("http://user@127.0.0.1:20890/").is_err());
        assert!(rpc_origin("http://127.0.0.1/").is_err());
        // ...and it is TIGHTER than validate_loopback_url alone, because the
        // launcher binds 127.0.0.1 and nothing else. Both of these are honest
        // loopback origins that no validator of ours would answer on.
        assert!(validate_loopback_url("http://localhost:20890/").is_ok());
        assert!(rpc_origin("http://localhost:20890/").is_err());
        assert!(validate_loopback_url("http://[::1]:20890/").is_ok());
        assert!(rpc_origin("http://[::1]:20890/").is_err());

        // A base whose 42-port block would not fit under 65535 is refused
        // here rather than by a launcher that has already spent a minute.
        assert!(rpc_origin(&format!("http://127.0.0.1:{}/", MAX_RPC_PORT + 1)).is_err());
        assert!(rpc_origin(&format!("http://127.0.0.1:{}/", MIN_RPC_PORT - 1)).is_err());

        // And the new one: an origin the DRIVER may target is still refused
        // HERE. `campaign` may leave the machine because it launches nothing;
        // this supervisor starts a validator, airdrops to it, and holds an
        // ephemeral authority in memory, so its origin is loopback or nothing.
        // A run spec carries no acknowledgment field, which is what keeps this
        // narrow: the devnet rail is reachable from a typed command line only.
        assert!(
            ClusterOriginV1::parse("https://api.devnet.solana.com/", Some(DEVNET_GENESIS_HASH))
                .is_ok(),
            "the driver's rail admits acknowledged devnet"
        );
        let refusal = rpc_origin("https://api.devnet.solana.com/")
            .err()
            .expect("the supervisor must refuse devnet");
        assert!(
            refusal.0.contains("launches and owns a validator")
                || refusal.0.contains("not loopback"),
            "the refusal must say why the SUPERVISOR cannot, got: {}",
            refusal.0
        );
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

    fn rpc_account(
        owner: Pubkey,
        executable: bool,
        lamports: u64,
        data: &[u8],
    ) -> crate::rpc::RpcAccount {
        crate::rpc::RpcAccount {
            lamports,
            owner,
            executable,
            rent_epoch: 0,
            data: data.to_vec(),
        }
    }

    #[test]
    fn infrastructure_publication_reuses_only_exact_finalized_records_and_routes_partials() {
        let registry = Pubkey::new_unique();
        let body = b"one exact infrastructure record";
        let minimum = 90;
        let finalized = rpc_account(registry, false, minimum, body);
        assert!(
            existing_finalized_record_is_exact(registry, Some(&finalized), None, body, minimum)
                .expect("exact finalized record")
        );

        // Raw absence is the gate to the chain-derived state machine. Both a
        // vacant pair and an in-flight cursor must reach it; the cursor's own
        // bytes, address, sponsor, offsets, and owner are authenticated there.
        let cursor = rpc_account(registry, false, 123, &[0x55; 32]);
        assert!(
            !existing_finalized_record_is_exact(registry, None, None, body, minimum)
                .expect("vacant pair")
        );
        assert!(
            !existing_finalized_record_is_exact(registry, None, Some(&cursor), body, minimum)
                .expect("partial pair is routed to publication state machine")
        );
        let partially_written = rpc_account(registry, false, minimum, &[0; 31]);
        assert!(
            !existing_finalized_record_is_exact(
                registry,
                Some(&partially_written),
                Some(&cursor),
                body,
                minimum,
            )
            .expect("Begin/Append partial is routed to publication state machine")
        );

        let mut wrong = finalized.clone();
        wrong.owner = Pubkey::new_unique();
        assert!(
            existing_finalized_record_is_exact(registry, Some(&wrong), None, body, minimum)
                .is_err()
        );
        let mut wrong = finalized.clone();
        wrong.executable = true;
        assert!(
            existing_finalized_record_is_exact(registry, Some(&wrong), None, body, minimum)
                .is_err()
        );
        let mut wrong = finalized.clone();
        wrong.data[0] ^= 1;
        assert!(
            existing_finalized_record_is_exact(registry, Some(&wrong), None, body, minimum)
                .is_err()
        );
        let mut wrong = finalized.clone();
        wrong.lamports = minimum - 1;
        assert!(
            existing_finalized_record_is_exact(registry, Some(&wrong), None, body, minimum)
                .is_err()
        );
        assert!(
            !existing_finalized_record_is_exact(
                registry,
                Some(&finalized),
                Some(&cursor),
                body,
                minimum,
            )
            .expect("a complete raw body with a cursor still needs Finalize"),
            "the publication state machine must authenticate and finalize the live cursor"
        );
    }

    #[test]
    fn record_budget_prices_vacancy_reuses_exact_finalized_and_refuses_conflict() {
        use solana_program::{
            account_info::AccountInfo, clock::Clock, rent::Rent, sysvar::SysvarSerialize as _,
        };
        use solana_sdk_ids::native_loader;

        let registry = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let content_bytes = b"record budget exact body";
        let content = RecordPublicationContentV1 {
            schema_release_id: [0x43; 32],
            content: content_bytes,
        };
        let (raw, staging, _) = derive_record_addresses_v1(registry, content).expect("addresses");
        let rent = Rent::default();
        let mut rent_lamports = 1;
        let mut rent_bytes = vec![0; Rent::size_of()];
        let rent_key = sysvar::rent::ID;
        let sysvar_owner = sysvar::ID;
        let mut rent_info = AccountInfo::new(
            &rent_key,
            false,
            false,
            &mut rent_lamports,
            &mut rent_bytes,
            &sysvar_owner,
            false,
        );
        rent.to_account_info(&mut rent_info).expect("Rent bytes");
        let clock = Clock {
            slot: 19_000,
            ..Clock::default()
        };
        let mut clock_lamports = 1;
        let mut clock_bytes = vec![0; Clock::size_of()];
        let clock_key = sysvar::clock::ID;
        let mut clock_info = AccountInfo::new(
            &clock_key,
            false,
            false,
            &mut clock_lamports,
            &mut clock_bytes,
            &sysvar_owner,
            false,
        );
        clock.to_account_info(&mut clock_info).expect("Clock bytes");
        fn observation(
            key: Pubkey,
            owner: Pubkey,
            lamports: u64,
            executable: bool,
            data: &[u8],
        ) -> AccountObservationV2<'_> {
            AccountObservationV2 {
                slot: 730,
                key,
                owner,
                lamports,
                executable,
                data,
            }
        }
        fn state<'a>(
            sponsor: Pubkey,
            raw: Pubkey,
            staging: Pubkey,
            raw_owner: Pubkey,
            raw_lamports: u64,
            raw_data: &'a [u8],
            rent_bytes: &'a [u8],
            clock_bytes: &'a [u8],
        ) -> RecordPublicationStateV1<'a> {
            RecordPublicationStateV1 {
                sponsor: observation(sponsor, system_program::ID, u64::MAX, false, &[]),
                raw_record: observation(raw, raw_owner, raw_lamports, false, raw_data),
                staging_cursor: observation(staging, system_program::ID, 0, false, &[]),
                system_program: observation(
                    system_program::ID,
                    native_loader::ID,
                    1,
                    true,
                    b"system_program",
                ),
                rent: observation(sysvar::rent::ID, sysvar::ID, 1, false, rent_bytes),
                clock: observation(sysvar::clock::ID, sysvar::ID, 1, false, clock_bytes),
            }
        }
        let vacant = remaining_record_publication_rent_from_state(
            registry,
            content,
            state(
                sponsor,
                raw,
                staging,
                system_program::ID,
                0,
                &[],
                &rent_bytes,
                &clock_bytes,
            ),
            "test",
        )
        .expect("vacant coordinate");
        assert!(vacant > rent.minimum_balance(content_bytes.len()));

        let exact = remaining_record_publication_rent_from_state(
            registry,
            content,
            state(
                sponsor,
                raw,
                staging,
                registry,
                rent.minimum_balance(content_bytes.len()),
                content_bytes,
                &rent_bytes,
                &clock_bytes,
            ),
            "test",
        )
        .expect("exact finalized coordinate");
        assert_eq!(exact, 0);

        let refusal = remaining_record_publication_rent_from_state(
            registry,
            content,
            state(
                sponsor,
                raw,
                staging,
                Pubkey::new_unique(),
                1,
                content_bytes,
                &rent_bytes,
                &clock_bytes,
            ),
            "test",
        )
        .expect_err("conflicting raw coordinate");
        assert!(refusal.0.contains("conflicts"), "{}", refusal.0);
    }

    fn activation_role(
        tag: u8,
        loader: dclutch_registry::release_set::ProgramIdentityV1,
    ) -> (
        dclutch_registry::release_set::ExecutionRoleBindingV1,
        ArtifactActivationInputV1,
    ) {
        use dclutch_registry::ArtifactUpgradePolicyV1;
        use dclutch_registry::release_set::{ExecutionRoleBindingV1, ProgramIdentityV1};

        let program = ProgramIdentityV1::new([tag; 32]).expect("program");
        let programdata = [tag.saturating_add(20); 32];
        let release_id =
            ArtifactReleaseIdV1::new([tag.saturating_add(40); 32]).expect("release ID");
        let authority = [tag.saturating_add(60); 32];
        let release = ArtifactReleaseV1::new(
            program,
            loader,
            programdata,
            ContentId::new([tag.saturating_add(80); 32]).expect("semantic release"),
            [tag.saturating_add(100); 32],
            1_000 + u64::from(tag),
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        )
        .expect("artifact release");
        let observation = DeploymentObservationV1::new(
            program.to_bytes(),
            loader.to_bytes(),
            true,
            programdata,
            loader.to_bytes(),
            false,
            programdata,
            loader.to_bytes(),
            release.deployment_slot(),
            release.elf_digest(),
            Some(authority),
        )
        .expect("deployment observation");
        (
            ExecutionRoleBindingV1::new(program, release_id),
            ArtifactActivationInputV1::new(release_id, release, observation),
        )
    }

    fn expected_activation_fixture() -> ActivatedExecutionReleaseSetV1 {
        use dclutch_registry::release_set::{ExecutionReleaseSetV1, ProgramIdentityV1};

        let loader = ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader");
        let (core_binding, core) = activation_role(1, loader);
        let (claims_binding, claims) = activation_role(2, loader);
        let (trading_binding, trading) = activation_role(3, loader);
        let (resolution_binding, resolution) = activation_role(4, loader);
        let (custody_binding, custody) = activation_role(5, loader);
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            claims_binding,
            trading_binding,
            resolution_binding,
            custody_binding,
        )
        .expect("release set");
        let inputs =
            ExecutionReleaseActivationInputsV1::new(core, claims, trading, resolution, custody);
        activate_execution_release_set_v1(
            ContentId::new([0xf0; 32]).expect("release-set ID"),
            &release_set,
            &inputs,
        )
        .expect("activation")
    }

    #[test]
    fn activation_progress_resumes_every_exact_partial_and_refuses_mismatches() {
        use dclutch_registry::ACTIVATED_ROLE_BYTES_V1;

        let registry = Pubkey::new_unique();
        let expected = expected_activation_fixture();
        let complete = expected.to_bytes();
        let roles_offset = complete.len() - ACTIVATION_ROLE_COUNT_V1 * ACTIVATED_ROLE_BYTES_V1;

        for written_count in 1..ACTIVATION_ROLE_COUNT_V1 {
            let mut partial = complete;
            partial[roles_offset + written_count * ACTIVATED_ROLE_BYTES_V1..].fill(0);
            let account = rpc_account(registry, false, 1, &partial);
            let progress = checked_activation_progress(registry, &account, expected)
                .expect("exact partial is resumable");
            assert_eq!(progress.written_count(), written_count);
            assert_eq!(
                remaining_activation_rent_for_account(Some(&account), registry, expected, 1)
                    .expect("exact partial already owns its full cache rent"),
                0
            );
            for (index, role) in ACTIVATION_ROLES_V1.into_iter().enumerate() {
                assert_eq!(progress.is_written(role), index < written_count);
                assert_eq!(
                    activation_role_is_pending(Some(progress), role),
                    index >= written_count
                );
            }
        }

        let complete_account = rpc_account(registry, false, 1, &complete);
        let complete_progress = checked_activation_progress(registry, &complete_account, expected)
            .expect("exact retry is idempotent");
        assert!(complete_progress.is_complete());
        assert_eq!(
            remaining_activation_rent_for_account(None, registry, expected, 99)
                .expect("missing cache"),
            99
        );
        assert_eq!(
            remaining_activation_rent_for_account(Some(&complete_account), registry, expected, 1)
                .expect("exact complete cache"),
            0
        );
        assert!(
            ACTIVATION_ROLES_V1
                .into_iter()
                .all(|role| !activation_role_is_pending(Some(complete_progress), role))
        );

        let mut substituted = complete;
        substituted[roles_offset + 3] ^= 1;
        let substituted = rpc_account(registry, false, 1, &substituted);
        assert!(checked_activation_progress(registry, &substituted, expected).is_err());

        let mut junk_in_unwritten = complete;
        junk_in_unwritten[roles_offset + ACTIVATED_ROLE_BYTES_V1..].fill(0);
        junk_in_unwritten[roles_offset + ACTIVATED_ROLE_BYTES_V1] = 1;
        let junk_in_unwritten = rpc_account(registry, false, 1, &junk_in_unwritten);
        assert!(checked_activation_progress(registry, &junk_in_unwritten, expected).is_err());

        let wrong_owner = rpc_account(Pubkey::new_unique(), false, 1, &complete);
        assert!(checked_activation_progress(registry, &wrong_owner, expected).is_err());
        assert!(
            remaining_activation_rent_for_account(Some(&wrong_owner), registry, expected, 1)
                .is_err()
        );
        let executable = rpc_account(registry, true, 1, &complete);
        assert!(checked_activation_progress(registry, &executable, expected).is_err());
        let underfunded = rpc_account(registry, false, 0, &complete);
        assert!(
            remaining_activation_rent_for_account(Some(&underfunded), registry, expected, 1)
                .is_err()
        );
    }

    #[test]
    fn activation_compute_guard_admits_canonical_roles_and_refuses_impossible_payloads() {
        let canonical_live_elf_bytes = [
            ("core", 934_088_u64),
            ("claims", 1_010_496),
            ("trading", 1_325_848),
            ("resolution", 588_336),
            ("custody", 360_328),
        ];
        for (role, bytes) in canonical_live_elf_bytes {
            let upper = activation_compute_upper_bound_v1(bytes).expect("bounded canonical role");
            assert!(
                upper < ACTIVATION_TRANSACTION_CU_LIMIT_V1,
                "canonical {role} needs size-only headroom"
            );
        }
        assert_eq!(
            activation_compute_upper_bound_v1(MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1)
                .expect("exact size ceiling"),
            ACTIVATION_TRANSACTION_CU_LIMIT_V1
        );
        assert!(
            activation_compute_upper_bound_v1(MAX_ACTIVATABLE_LIVE_ELF_BYTES_V1 + 1)
                .expect("one-byte overflow is representable")
                > ACTIVATION_TRANSACTION_CU_LIMIT_V1
        );
        assert!(
            activation_compute_upper_bound_v1(9_034_536)
                .expect("hostile Source substitution is representable")
                > ACTIVATION_TRANSACTION_CU_LIMIT_V1
        );
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
        let genesis = loader_programdata_bytes(&elf, 0, Some(authority.pubkey()));
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
        let expected = programdata_bytes_after_revoke(&loader_programdata_bytes(
            &elf,
            0,
            Some(authority.pubkey()),
        ))
        .expect("revoke poststate");
        assert_eq!(observed.data, expected);
        let view = dclutch_registry::svm::ProgramDataV3View::parse(&observed.data)
            .expect("Registry parses real Loader poststate");
        assert_eq!(view.upgrade_authority(), None);
        assert_eq!(&observed.data[13..45], authority.pubkey().as_ref());
        drop(validator);
        fs::remove_dir_all(root).expect("remove scoped Loader test directory");
    }

    #[test]
    fn real_sbf_infrastructure_revoke_and_registry_activation_when_supplied() {
        let (
            Ok(core_elf),
            Ok(registry_elf),
            Ok(rent_elf),
            Ok(claims_elf),
            Ok(trading_elf),
            Ok(resolution_elf),
            Ok(custody_elf),
        ) = (
            std::env::var("DCLUTCH_SUCCESSOR_CORE_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_REGISTRY_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_RENT_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_CLAIMS_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_TRADING_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_RESOLUTION_ELF"),
            std::env::var("DCLUTCH_SUCCESSOR_CUSTODY_ELF"),
        )
        else {
            return;
        };
        let checked_gate = PathBuf::from(
            std::env::var("DCLUTCH_SUCCESSOR_CHECKED_GATE")
                .expect("real-SBF role set requires its checked-release gate"),
        );
        let expected_gate_sha256 = std::env::var("DCLUTCH_SUCCESSOR_CHECKED_GATE_SHA256")
            .expect("real-SBF role set requires the explicit gate digest");
        let expected_source_revision = std::env::var("DCLUTCH_SUCCESSOR_SOURCE_REVISION")
            .expect("real-SBF role set requires the explicit source revision");
        let expected_source_tree_sha256 = std::env::var("DCLUTCH_SUCCESSOR_SOURCE_TREE_SHA256")
            .expect("real-SBF role set requires the explicit source-tree digest");
        let core_elf = fs::canonicalize(core_elf).expect("canonical Core test ELF");
        let registry_elf = fs::canonicalize(registry_elf).expect("canonical Registry test ELF");
        let rent_elf = fs::canonicalize(rent_elf).expect("canonical Rent test ELF");
        let claims_elf = fs::canonicalize(claims_elf).expect("canonical Claims test ELF");
        let trading_elf = fs::canonicalize(trading_elf).expect("canonical Trading test ELF");
        let resolution_elf =
            fs::canonicalize(resolution_elf).expect("canonical Resolution test ELF");
        let custody_elf = fs::canonicalize(custody_elf).expect("canonical Custody test ELF");
        let authenticate_role = |role: &str, elf: &Path| {
            crate::upgrade::authenticate_checked_release_gate_role_for_local_v1(
                &checked_gate,
                &expected_gate_sha256,
                &expected_source_revision,
                &expected_source_tree_sha256,
                role,
                elf,
            )
            .unwrap_or_else(|error| panic!("checked-release {role} admission: {error}"));
        };
        for (role, elf) in [
            ("registry", registry_elf.as_path()),
            ("rent", rent_elf.as_path()),
            ("custody", custody_elf.as_path()),
            ("resolution", resolution_elf.as_path()),
            ("claims", claims_elf.as_path()),
            ("trading", trading_elf.as_path()),
            ("core", core_elf.as_path()),
        ] {
            authenticate_role(role, elf);
        }
        for (role, elf) in [
            ("core", core_elf.as_path()),
            ("claims", claims_elf.as_path()),
            ("trading", trading_elf.as_path()),
            ("resolution", resolution_elf.as_path()),
            ("custody", custody_elf.as_path()),
        ] {
            let bytes = u64::try_from(fs::metadata(elf).expect("role ELF metadata").len())
                .expect("role ELF width");
            let upper =
                activation_compute_upper_bound_v1(bytes).expect("activation compute upper bound");
            assert!(
                upper <= ACTIVATION_TRANSACTION_CU_LIMIT_V1,
                "checked-release {role} ELF has {bytes} bytes and a conservative {upper}-CU \
                 first-activation bound, above the {}-CU transaction ceiling",
                ACTIVATION_TRANSACTION_CU_LIMIT_V1
            );
        }
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
            observed_upgrade_authority: None,
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
                &dclutch_source::resolution::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            ),
            custody_program: program(0x36),
            custody_sha256: digest(&custody_elf),
            custody_elf,
            custody_semantic_release_id: "16".repeat(32),
            rent_credit_program: program(0x37),
            rent_credit_elf: rent_elf,
            rent_credit_sha256: rent_sha,
            rent_credit_semantic_release_id: "17".repeat(32),
            checked_upgrade_set: None,
            record_publication: crate::plan::RecordPublicationV1::Genesis,
            deployments: RoleDeploymentsV1::default(),
            general_accelerator: None,
        })
        .expect("prepare real-SBF infrastructure plan");
        validate_plan(&plan).expect("validate real-SBF plan");

        let spawn_validator = |genesis: bool| {
            let mut command = Command::new(which_validator().expect("pinned validator"));
            command
                .arg("--config")
                .arg("/dev/null")
                .arg("--ledger")
                .arg(root.join("ledger"));
            if genesis {
                command
                    .arg("--account-dir")
                    .arg(root.join("accounts"))
                    .arg("--mint")
                    .arg(system_program::ID.to_string());
            }
            let child = command
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
            ValidatorChild { child }
        };
        let mut validator = spawn_validator(true);
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
        let activation =
            activation_instructions(&plan, authority.pubkey()).expect("activation walk-up");
        for (label, instruction) in activation.iter().take(2) {
            rpc.send(label, std::slice::from_ref(instruction), &authority)
                .expect("real-SBF partial role activation succeeds");
        }
        let progress = activation_progress(&mut rpc, &plan)
            .expect("exact partial activation progress")
            .expect("partial activation cache exists");
        assert!(progress.is_written(ExecutionRoleV1::Core));
        assert!(progress.is_written(ExecutionRoleV1::Claims));
        assert!(!progress.is_written(ExecutionRoleV1::Trading));

        // Stop the validator after a valid two-role prefix, then resume the
        // exact ledger without replaying genesis inputs. The chain-derived
        // detector must select only the three still-empty roles after the
        // process boundary; no local campaign state file participates.
        drop(rpc);
        drop(validator);
        let mut validator = spawn_validator(false);
        let mut rpc =
            wait_test_rpc(&mut validator, "http://127.0.0.1:22290/").expect("resumed real-SBF RPC");
        let pending = pending_activation_instructions(&mut rpc, &plan, authority.pubkey())
            .expect("chain-derived activation resume");
        assert_eq!(pending.len(), 3);
        assert!(pending[0].0.ends_with("Trading"));
        for (label, instruction) in pending {
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

        let registry = pubkey(&plan.registry.program_id).expect("Registry program");
        let direct =
            crate::direct_market::DirectMarketCompilerOwnedV1::for_test_plan(registry, &plan)
                .expect("test Direct compiler");
        let market_input = crate::market::demo_market_input(registry, direct.compiler())
            .expect("canonical demo market input");
        let market_evidence = crate::market::execute_found_market(
            &mut rpc,
            &plan,
            &market_input,
            &authority,
            &KeyForge::random(),
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
