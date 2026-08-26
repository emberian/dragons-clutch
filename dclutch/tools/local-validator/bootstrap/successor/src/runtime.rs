use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use dclutch_pyth_svm::{FullPriceUpdateV2, local_validator_release_v1};
use dclutch_registry_svm::RegistryInstructionV1;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    AcceptPythRequestV1, FundedTransitionActionV3, FundedTransitionRequestV3,
    RESOLUTION_CERTIFICATE_BYTES, ResolutionCertificateV1,
};
use dclutch_source_contract::{SourceResolutionPhaseV1, SourceResolutionStateV1};
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Error, Result,
    model::{
        AccountEvidence, ExecutionEvidence, LoaderProgramEvidence, ProviderEvidenceInput,
        ProviderProgramInput, RecordPair, ReplayEvidence, RollbackEvidence, SourceCase,
        SuccessorPlan,
        TransactionEvidence,
    },
    plan::{GENERATION, hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, account_evidence, validate_loopback_url},
};

const PAYER_AIRDROP_LAMPORTS: u64 = 10_000_000_000;
const WORKER_AIRDROP_LAMPORTS: u64 = 1_000_000;

#[derive(Debug)]
pub(crate) struct RunArgs {
    pub(crate) rpc_url: String,
    pub(crate) plan_path: PathBuf,
    pub(crate) provider_evidence_path: PathBuf,
    pub(crate) output: PathBuf,
}

struct Runtime<'a> {
    rpc: Rpc,
    plan: &'a SuccessorPlan,
    provider: &'a ProviderEvidenceInput,
    registry: Pubkey,
    core: Pubkey,
    core_programdata: Pubkey,
    claims: Pubkey,
    claims_programdata: Pubkey,
    trading: Pubkey,
    trading_programdata: Pubkey,
    resolution: Pubkey,
    resolution_programdata: Pubkey,
    custody: Pubkey,
    custody_programdata: Pubkey,
    payer: Keypair,
    worker: Keypair,
    transactions: Vec<TransactionEvidence>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn execute(args: &RunArgs) -> Result<ExecutionEvidence> {
    validate_path(&args.plan_path, "--plan")?;
    validate_path(&args.provider_evidence_path, "--provider-evidence")?;
    if !args.output.is_absolute() || args.output.exists() {
        return Err(Error::new("--output must be an absolute nonexistent file"));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&args.plan_path)?)?;
    let provider: ProviderEvidenceInput =
        serde_json::from_slice(&fs::read(&args.provider_evidence_path)?)?;
    validate_inputs(args, &plan, &provider)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let custody_programdata = pubkey(&plan.custody.programdata_id)?;
    let rpc = Rpc::connect(&args.rpc_url)?;
    let payer = Keypair::new();
    let worker = Keypair::new();
    let mut runtime = Runtime {
        rpc,
        plan: &plan,
        provider: &provider,
        registry,
        core,
        core_programdata,
        claims,
        claims_programdata,
        trading,
        trading_programdata,
        resolution,
        resolution_programdata,
        custody,
        custody_programdata,
        payer,
        worker,
        transactions: Vec::new(),
    };
    runtime.transactions.push(runtime.rpc.airdrop(
        "airdrop_ephemeral_payer",
        runtime.payer.pubkey(),
        PAYER_AIRDROP_LAMPORTS,
    )?);
    runtime.transactions.push(runtime.rpc.airdrop(
        "airdrop_ephemeral_worker",
        runtime.worker.pubkey(),
        WORKER_AIRDROP_LAMPORTS,
    )?);

    let programs = BTreeMap::from([
        (
            "registry".into(),
            authenticate_local_program(&mut runtime.rpc, "Registry", &plan.registry)?,
        ),
        (
            "core".into(),
            authenticate_local_program(&mut runtime.rpc, "Core", &plan.core)?,
        ),
        (
            "claims".into(),
            authenticate_local_program(&mut runtime.rpc, "Claims", &plan.claims)?,
        ),
        (
            "trading".into(),
            authenticate_local_program(&mut runtime.rpc, "Trading", &plan.trading)?,
        ),
        (
            "resolution".into(),
            authenticate_local_program(&mut runtime.rpc, "Resolution", &plan.resolution)?,
        ),
        (
            "custody".into(),
            authenticate_local_program(&mut runtime.rpc, "Custody", &plan.custody)?,
        ),
    ]);
    authenticate_provider(&mut runtime.rpc, &provider)?;
    authenticate_genesis(&mut runtime.rpc, &plan)?;

    runtime.activate_registry()?;
    runtime.reauthenticate_core()?;
    runtime.reauthenticate_claims()?;
    runtime.reauthenticate_trading()?;
    runtime.reauthenticate_resolution()?;
    runtime.reauthenticate_custody()?;
    runtime.accept_primary()?;
    let primary_replay = runtime.prove_primary_replay()?;
    runtime.run_funded_lifecycle(&plan.lifecycle, "lifecycle")?;
    runtime.run_funded_prefix(&plan.rollback, "rollback")?;
    let rollback = runtime.prove_rollback()?;

    let primary_state = runtime.account(pubkey(&plan.primary.state)?, "primary state")?;
    let primary_value = SourceResolutionStateV1::decode(&primary_state.data)
        .map_err(|error| Error::new(format!("primary Source state: {error:?}")))?;
    if primary_value.phase() != SourceResolutionPhaseV1::Resolved {
        return Err(Error::new("primary Source state is not Resolved"));
    }
    let lifecycle_state = runtime.account(pubkey(&plan.lifecycle.state)?, "lifecycle state")?;
    let lifecycle_value = SourceResolutionStateV1::decode(&lifecycle_state.data)
        .map_err(|error| Error::new(format!("lifecycle Source state: {error:?}")))?;
    if lifecycle_value.phase() != SourceResolutionPhaseV1::FailureCommitted {
        return Err(Error::new(
            "funded lifecycle did not reach FailureCommitted",
        ));
    }

    let mut accounts = BTreeMap::new();
    for (label, pin) in &plan.genesis_accounts {
        let key = pubkey(&pin.address)?;
        if let Some(account) = runtime.rpc.account(key)? {
            accounts.insert(label.clone(), account_evidence(key, &account));
        }
    }
    accounts.insert(
        "registry.activation".into(),
        account_evidence(
            pubkey(&plan.activation)?,
            &runtime.account(pubkey(&plan.activation)?, "Registry activation")?,
        ),
    );
    accounts.insert(
        "ephemeral.worker".into(),
        account_evidence(
            runtime.worker.pubkey(),
            &runtime.account(runtime.worker.pubkey(), "worker")?,
        ),
    );
    let evidence = ExecutionEvidence {
        schema: "dclutch-local-successor-bootstrap-evidence-v1",
        evidence_class: "localhost-real-registry-resolution-and-pyth-execution",
        rpc_url: runtime.rpc.url().to_owned(),
        provider_evidence_path: args.provider_evidence_path.display().to_string(),
        plan_path: args.plan_path.display().to_string(),
        genesis_fixture_boundary: plan.genesis_boundary.clone(),
        semantic_records_created_onchain: false,
        markets_created_onchain: false,
        source_states_created_onchain: false,
        funding_created_onchain: false,
        certificates_created_by_resolution: true,
        captured_release_identity_claimed: false,
        checked_production_release_claimed: false,
        registry_activated: true,
        core_reauthenticated: true,
        claims_reauthenticated: true,
        trading_reauthenticated: true,
        custody_reauthenticated: true,
        registry_reauthenticated: true,
        real_pyth_price_update_consumed: true,
        primary_resolution_executed: true,
        primary_replay_refused: primary_replay.state_unchanged
            && primary_replay.certificate_unchanged,
        sequential_recovery_exhaustion_failure_executed: true,
        rollback_proved: rollback.state_unchanged
            && rollback.certificate_unchanged
            && rollback.funding_unchanged
            && rollback.worker_unchanged,
        programs,
        accounts,
        transactions: runtime.transactions,
        primary_replay,
        rollback,
    };
    if !evidence.primary_replay_refused {
        return Err(Error::new(
            "primary replay refusal changed the Source state or certificate",
        ));
    }
    if !evidence.rollback_proved {
        return Err(Error::new(
            "hostile transaction did not roll back all four outputs",
        ));
    }
    Ok(evidence)
}

impl Runtime<'_> {
    fn activate_registry(&mut self) -> Result<()> {
        let release = record(self.plan, "execution_release_set")?;
        let core_artifact = record(self.plan, "core_artifact_release")?;
        let claims_artifact = record(self.plan, "claims_artifact_release")?;
        let trading_artifact = record(self.plan, "trading_artifact_release")?;
        let resolution_artifact = record(self.plan, "resolution_artifact_release")?;
        let mut accounts = vec![
            AccountMeta::new(self.payer.pubkey(), true),
            AccountMeta::new(pubkey(&self.plan.activation)?, false),
            AccountMeta::new_readonly(pubkey(&release.raw)?, false),
            AccountMeta::new_readonly(pubkey(&release.staging)?, false),
        ];
        append_role(
            &mut accounts,
            core_artifact,
            self.core,
            self.core_programdata,
        )?;
        append_role(
            &mut accounts,
            claims_artifact,
            self.claims,
            self.claims_programdata,
        )?;
        append_role(
            &mut accounts,
            trading_artifact,
            self.trading,
            self.trading_programdata,
        )?;
        append_role(
            &mut accounts,
            resolution_artifact,
            self.resolution,
            self.resolution_programdata,
        )?;
        let custody_artifact = record(self.plan, "custody_artifact_release")?;
        append_role(
            &mut accounts,
            custody_artifact,
            self.custody,
            self.custody_programdata,
        )?;
        accounts.push(AccountMeta::new_readonly(system_program::ID, false));
        accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
        if accounts.len() != 26 {
            return Err(Error::new("Registry Activate account count is not 26"));
        }
        let transaction = self.rpc.send(
            "registry_activate_release_set",
            &[Instruction {
                program_id: self.registry,
                accounts,
                data: RegistryInstructionV1::Activate.to_bytes().to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        let activation = self.account(pubkey(&self.plan.activation)?, "Registry activation")?;
        if activation.owner != self.registry || activation.data.len() != 1_288 {
            return Err(Error::new(
                "Registry activation cache has wrong owner or width",
            ));
        }
        Ok(())
    }

    fn reauthenticate_core(&mut self) -> Result<()> {
        let transaction = self.rpc.send(
            "registry_reauthenticate_core",
            &[Instruction {
                program_id: self.registry,
                accounts: vec![
                    AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
                    AccountMeta::new_readonly(self.core, false),
                    AccountMeta::new_readonly(self.core_programdata, false),
                ],
                data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core)
                    .to_bytes()
                    .to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        Ok(())
    }

    fn reauthenticate_resolution(&mut self) -> Result<()> {
        let transaction = self.rpc.send(
            "registry_reauthenticate_resolution",
            &[Instruction {
                program_id: self.registry,
                accounts: vec![
                    AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
                    AccountMeta::new_readonly(self.resolution, false),
                    AccountMeta::new_readonly(self.resolution_programdata, false),
                ],
                data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Resolution)
                    .to_bytes()
                    .to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        Ok(())
    }

    fn reauthenticate_claims(&mut self) -> Result<()> {
        let transaction = self.rpc.send(
            "registry_reauthenticate_claims",
            &[Instruction {
                program_id: self.registry,
                accounts: vec![
                    AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
                    AccountMeta::new_readonly(self.claims, false),
                    AccountMeta::new_readonly(self.claims_programdata, false),
                ],
                data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Claims)
                    .to_bytes()
                    .to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        Ok(())
    }

    fn reauthenticate_trading(&mut self) -> Result<()> {
        let transaction = self.rpc.send(
            "registry_reauthenticate_trading",
            &[Instruction {
                program_id: self.registry,
                accounts: vec![
                    AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
                    AccountMeta::new_readonly(self.trading, false),
                    AccountMeta::new_readonly(self.trading_programdata, false),
                ],
                data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Trading)
                    .to_bytes()
                    .to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        Ok(())
    }

    fn reauthenticate_custody(&mut self) -> Result<()> {
        let transaction = self.rpc.send(
            "registry_reauthenticate_custody",
            &[Instruction {
                program_id: self.registry,
                accounts: vec![
                    AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
                    AccountMeta::new_readonly(self.custody, false),
                    AccountMeta::new_readonly(self.custody_programdata, false),
                ],
                data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Custody)
                    .to_bytes()
                    .to_vec(),
            }],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        Ok(())
    }

    fn accept_primary(&mut self) -> Result<()> {
        let instruction = self.primary_instruction()?;
        let transaction = self.rpc.send(
            "resolution_accept_real_pyth_primary",
            &[instruction],
            &self.payer,
        )?;
        self.transactions.push(transaction);
        let certificate = self.account(
            case_key(&self.plan.primary, "success", true)?,
            "success certificate",
        )?;
        authenticate_certificate(&certificate, self.resolution)
    }

    fn primary_instruction(&self) -> Result<Instruction> {
        let provider_release = hex32(&self.plan.provider_release_id)?;
        let result_domain = hex32(&self.plan.result_domain_id)?;
        let request = AcceptPythRequestV1 {
            expected_generation: self.plan.generation,
            expected_result_domain_id: result_domain,
            expected_provider_release_id: provider_release,
        }
        .to_bytes()
        .map_err(|error| Error::new(format!("primary request: {error:?}")))?;
        let update = pubkey(&self.provider.price_update)?;
        let receiver = provider_program(self.provider, "pyth-receiver")?;
        let router = provider_program(self.provider, "pyth-router")?;
        let local = local_validator_release_v1()
            .map_err(|error| Error::new(format!("local provider release: {error:?}")))?;
        let release = local.release();
        let material = record(self.plan, "source_material")?;
        let product = record(self.plan, "product_instance")?;
        let pyth = record(self.plan, "pyth_release")?;
        let accounts = vec![
            AccountMeta::new(pubkey(&self.plan.primary.state)?, false),
            AccountMeta::new(case_key(&self.plan.primary, "success", true)?, false),
            AccountMeta::new_readonly(pubkey(&self.plan.primary.market)?, false),
            AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
            AccountMeta::new_readonly(self.resolution, false),
            AccountMeta::new_readonly(self.resolution_programdata, false),
            AccountMeta::new_readonly(pubkey(&material.raw)?, false),
            AccountMeta::new_readonly(pubkey(&material.staging)?, false),
            AccountMeta::new_readonly(pubkey(&product.raw)?, false),
            AccountMeta::new_readonly(pubkey(&product.staging)?, false),
            AccountMeta::new_readonly(pubkey(&pyth.raw)?, false),
            AccountMeta::new_readonly(pubkey(&pyth.staging)?, false),
            AccountMeta::new_readonly(update, false),
            AccountMeta::new_readonly(receiver.program_id()?, false),
            AccountMeta::new_readonly(receiver.programdata_id()?, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(release.receiver_config()), false),
            AccountMeta::new_readonly(router.program_id()?, false),
            AccountMeta::new_readonly(router.programdata_id()?, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        if accounts.len() != 21 {
            return Err(Error::new("Resolution primary account count is not 21"));
        }
        Ok(Instruction {
            program_id: self.resolution,
            accounts,
            data: request.to_vec(),
        })
    }

    fn prove_primary_replay(&mut self) -> Result<ReplayEvidence> {
        let before = self.primary_snapshot()?;
        let transaction = self.rpc.send_expected_failure(
            "resolution_accept_real_pyth_primary_replay_refusal",
            &[self.primary_instruction()?],
            &self.payer,
        )?;
        let after = self.primary_snapshot()?;
        Ok(ReplayEvidence {
            transaction,
            state_unchanged: same(&before, &after, "state"),
            certificate_unchanged: same(&before, &after, "certificate"),
            before,
            after,
        })
    }

    fn primary_snapshot(&mut self) -> Result<BTreeMap<String, AccountEvidence>> {
        let entries = [
            ("state", pubkey(&self.plan.primary.state)?),
            (
                "certificate",
                case_key(&self.plan.primary, "success", true)?,
            ),
        ];
        let mut output = BTreeMap::new();
        for (label, key) in entries {
            let account = self.account(key, label)?;
            output.insert(label.into(), account_evidence(key, &account));
        }
        Ok(output)
    }

    fn run_funded_lifecycle(&mut self, case: &SourceCase, prefix: &str) -> Result<()> {
        self.submit_funded(
            case,
            prefix,
            "recovery",
            FundedTransitionActionV3::FailNext,
            false,
        )?;
        self.submit_funded(
            case,
            prefix,
            "exhaustion",
            FundedTransitionActionV3::Exhaust,
            false,
        )?;
        self.submit_funded(
            case,
            prefix,
            "failure",
            FundedTransitionActionV3::CommitFailure,
            false,
        )?;
        Ok(())
    }

    fn run_funded_prefix(&mut self, case: &SourceCase, prefix: &str) -> Result<()> {
        self.submit_funded(
            case,
            prefix,
            "recovery",
            FundedTransitionActionV3::FailNext,
            false,
        )?;
        self.submit_funded(
            case,
            prefix,
            "exhaustion",
            FundedTransitionActionV3::Exhaust,
            false,
        )?;
        Ok(())
    }

    fn prove_rollback(&mut self) -> Result<RollbackEvidence> {
        let case = &self.plan.rollback;
        let before = self.rollback_snapshot(case)?;
        let transaction = self.submit_funded(
            case,
            "rollback",
            "failure",
            FundedTransitionActionV3::CommitFailure,
            true,
        )?;
        let after = self.rollback_snapshot(case)?;
        Ok(RollbackEvidence {
            transaction,
            state_unchanged: same(&before, &after, "state"),
            certificate_unchanged: same(&before, &after, "certificate"),
            funding_unchanged: same(&before, &after, "funding"),
            worker_unchanged: same(&before, &after, "worker"),
            before,
            after,
        })
    }

    fn submit_funded(
        &mut self,
        case: &SourceCase,
        prefix: &str,
        step: &str,
        action: FundedTransitionActionV3,
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        let recovery_index = match action {
            FundedTransitionActionV3::FailNext => 0,
            FundedTransitionActionV3::Exhaust | FundedTransitionActionV3::CommitFailure => 1,
        };
        let allocation = match action {
            FundedTransitionActionV3::FailNext => hex32(&self.plan.recovery_allocation_id)?,
            FundedTransitionActionV3::Exhaust => hex32(&self.plan.exhaustion_allocation_id)?,
            FundedTransitionActionV3::CommitFailure => hex32(&self.plan.funded_source_material_id)?,
        };
        let request = FundedTransitionRequestV3 {
            action,
            expected_generation: GENERATION,
            expected_recovery_index: recovery_index,
            expected_result_domain_id: hex32(&self.plan.result_domain_id)?,
            expected_funding_allocation_id: allocation,
        }
        .to_bytes()
        .map_err(|error| Error::new(format!("funded request: {error:?}")))?;
        let material = record(self.plan, "funded_source_material")?;
        let product = record(self.plan, "product_instance")?;
        let manifest = record(self.plan, "capability_manifest")?;
        let accounts = vec![
            AccountMeta::new(pubkey(&case.state)?, false),
            AccountMeta::new(case_key(case, step, true)?, false),
            AccountMeta::new(case_key(case, step, false)?, false),
            AccountMeta::new(self.worker.pubkey(), false),
            AccountMeta::new_readonly(pubkey(&case.market)?, false),
            AccountMeta::new_readonly(pubkey(&self.plan.activation)?, false),
            AccountMeta::new_readonly(self.resolution, false),
            AccountMeta::new_readonly(self.resolution_programdata, false),
            AccountMeta::new_readonly(pubkey(&material.raw)?, false),
            AccountMeta::new_readonly(pubkey(&material.staging)?, false),
            AccountMeta::new_readonly(pubkey(&product.raw)?, false),
            AccountMeta::new_readonly(pubkey(&product.staging)?, false),
            AccountMeta::new_readonly(pubkey(&manifest.raw)?, false),
            AccountMeta::new_readonly(pubkey(&manifest.staging)?, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        if accounts.len() != 17 {
            return Err(Error::new("Resolution funded account count is not 17"));
        }
        let instruction = Instruction {
            program_id: self.resolution,
            accounts,
            data: request.to_vec(),
        };
        let label = format!("resolution_{prefix}_{step}");
        let transaction = if expect_failure {
            self.rpc
                .send_expected_failure(&label, &[instruction], &self.payer)?
        } else {
            self.rpc.send(&label, &[instruction], &self.payer)?
        };
        if !expect_failure {
            self.transactions.push(transaction.clone());
            let certificate = self.account(case_key(case, step, true)?, "funded certificate")?;
            authenticate_certificate(&certificate, self.resolution)?;
        }
        Ok(transaction)
    }

    fn rollback_snapshot(
        &mut self,
        case: &SourceCase,
    ) -> Result<BTreeMap<String, AccountEvidence>> {
        let entries = [
            ("state", pubkey(&case.state)?),
            ("certificate", case_key(case, "failure", true)?),
            ("funding", case_key(case, "failure", false)?),
            ("worker", self.worker.pubkey()),
        ];
        let mut output = BTreeMap::new();
        for (label, key) in entries {
            let account = self.account(key, label)?;
            output.insert(label.into(), account_evidence(key, &account));
        }
        Ok(output)
    }

    fn account(&mut self, address: Pubkey, label: &str) -> Result<RpcAccount> {
        self.rpc.required_account(address, label)
    }
}

fn validate_inputs(
    args: &RunArgs,
    plan: &SuccessorPlan,
    provider: &ProviderEvidenceInput,
) -> Result<()> {
    if plan.schema != "dclutch-local-successor-genesis-plan-v1"
        || plan.generation != GENERATION
        || plan.genesis_boundary.len() != 4
    {
        return Err(Error::new(
            "successor plan has unsupported schema or hidden boundary",
        ));
    }
    if validate_loopback_url(&provider.rpc_url)? != validate_loopback_url(&args.rpc_url)?
        || !provider.provider_state_initialized
        || provider.captured_release_identity_claimed
        || provider.price_update_reclaimed
    {
        return Err(Error::new(
            "provider evidence is not the live, initialized, non-reclaimed localhost profile",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let programs = [registry, core, claims, trading, resolution, custody];
    if programs.iter().enumerate().any(|(index, program)| {
        programs
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == program)
    }) {
        return Err(Error::new(
            "Registry and all five role Program IDs must be pairwise distinct",
        ));
    }
    validate_material_partition(plan)?;
    Ok(())
}

fn validate_material_partition(plan: &SuccessorPlan) -> Result<()> {
    let primary = record(plan, "source_material")?;
    let funded = record(plan, "funded_source_material")?;
    if plan.source_material_id == plan.funded_source_material_id
        || primary.content_sha256 != plan.source_material_id
        || funded.content_sha256 != plan.funded_source_material_id
        || plan.configured_max_age_seconds <= plan.funded_max_age_seconds
    {
        return Err(Error::new(
            "primary-observation and funded-expiry Source materials are not distinct exact profiles",
        ));
    }
    Ok(())
}

fn authenticate_genesis(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<()> {
    for (label, pin) in &plan.genesis_accounts {
        let key = pubkey(&pin.address)?;
        let account = rpc.required_account(key, label)?;
        let evidence = account_evidence(key, &account);
        if evidence.owner != pin.owner
            || evidence.lamports != pin.lamports
            || evidence.data_len != pin.data_len
            || evidence.data_sha256 != pin.data_sha256
            || evidence.account_sha256 != pin.account_sha256
        {
            return Err(Error::new(format!(
                "genesis account {label} differs from its exact plan pin"
            )));
        }
    }
    if rpc.account(pubkey(&plan.activation)?)?.is_some() {
        return Err(Error::new(
            "Registry activation already exists; fresh run required",
        ));
    }
    Ok(())
}

fn authenticate_local_program(
    rpc: &mut Rpc,
    label: &str,
    pin: &crate::model::ProgramPin,
) -> Result<LoaderProgramEvidence> {
    let program = pubkey(&pin.program_id)?;
    let programdata = pubkey(&pin.programdata_id)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata != expected_programdata {
        return Err(Error::new(format!(
            "{label} ProgramData PDA is not canonical"
        )));
    }
    let program_account = rpc.required_account(program, label)?;
    let data_account = rpc.required_account(programdata, &format!("{label} ProgramData"))?;
    if program_account.owner != bpf_loader_upgradeable::ID
        || !program_account.executable
        || program_account.data.len() != 36
        || program_account.data.get(..4) != Some(&2_u32.to_le_bytes())
        || program_account.data.get(4..36) != Some(programdata.as_ref())
        || data_account.owner != bpf_loader_upgradeable::ID
        || data_account.executable
        || data_account.data.get(..4) != Some(&3_u32.to_le_bytes())
        || data_account.data.get(4..12) != Some(&0_u64.to_le_bytes())
    {
        return Err(Error::new(format!(
            "{label} Loader V3 facts are not a canonical slot-0 deployment"
        )));
    }
    if data_account.data.get(12) != Some(&0)
        || data_account.data.get(13..45) != Some(&[0_u8; 32])
        || pin.upgrade_authority.is_some()
    {
        return Err(Error::new(format!(
            "{label} Loader V3 deployment is not canonically immutable"
        )));
    }
    let elf = data_account
        .data
        .get(45..)
        .ok_or_else(|| Error::new(format!("{label} ProgramData omitted ELF")))?;
    if hex(&Sha256::digest(elf)) != pin.elf_sha256 {
        return Err(Error::new(format!(
            "{label} on-chain ELF hash differs from plan"
        )));
    }
    let header = data_account
        .data
        .get(..45)
        .ok_or_else(|| Error::new(format!("{label} ProgramData omitted Loader header")))?;
    Ok(LoaderProgramEvidence {
        program_id: program.to_string(),
        programdata_id: programdata.to_string(),
        deployment_slot: 0,
        upgrade_authority: None,
        upgrade_authority_effectively_disabled: true,
        elf_sha256: pin.elf_sha256.clone(),
        loader_header_sha256: hex(&Sha256::digest(header)),
        program: account_evidence(program, &program_account),
        programdata: account_evidence(programdata, &data_account),
    })
}

fn authenticate_provider(rpc: &mut Rpc, provider: &ProviderEvidenceInput) -> Result<()> {
    let local = local_validator_release_v1()
        .map_err(|error| Error::new(format!("local provider release: {error:?}")))?;
    let release = local.release();
    for (name, expected_program, expected_programdata, expected_hash) in [
        (
            "pyth-receiver",
            Pubkey::new_from_array(release.receiver_program()),
            Pubkey::new_from_array(release.receiver_programdata()),
            "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        ),
        (
            "pyth-router",
            Pubkey::new_from_array(release.router_program()),
            Pubkey::new_from_array(release.router_programdata()),
            "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        ),
    ] {
        let evidence = provider_program(provider, name)?;
        if evidence.program_id()? != expected_program
            || evidence.programdata_id()? != expected_programdata
            || evidence.observed_deployment_slot != 0
            || !evidence.observed_upgrade_authority_effectively_disabled
            || evidence.elf_tail_sha256 != expected_hash
        {
            return Err(Error::new(format!(
                "{name} provider evidence differs from local release"
            )));
        }
        let account = rpc.required_account(expected_programdata, &format!("{name} ProgramData"))?;
        let elf = account
            .data
            .get(45..)
            .ok_or_else(|| Error::new(format!("{name} ProgramData omitted ELF")))?;
        if hex(&Sha256::digest(elf)) != expected_hash {
            return Err(Error::new(format!("{name} live ELF differs from evidence")));
        }
    }
    let update_key = pubkey(&provider.price_update)?;
    let update_account = rpc.required_account(update_key, "real posted Pyth PriceUpdate")?;
    let update = FullPriceUpdateV2::parse(&update_account.data)
        .map_err(|error| Error::new(format!("PriceUpdate: {error:?}")))?;
    if update_account.owner != Pubkey::new_from_array(release.receiver_program())
        || update_account.data.len() != 134
        || update.publish_time() != plan_publish_time()
    {
        return Err(Error::new(
            "posted Pyth PriceUpdate does not match captured fixture semantics",
        ));
    }
    Ok(())
}

fn plan_publish_time() -> i64 {
    crate::plan::FIXTURE_PUBLISH_TIME
}

fn authenticate_certificate(account: &RpcAccount, resolution: Pubkey) -> Result<()> {
    if account.owner != resolution || account.data.len() != RESOLUTION_CERTIFICATE_BYTES {
        return Err(Error::new(
            "certificate was not allocated and assigned by Resolution",
        ));
    }
    ResolutionCertificateV1::decode(&account.data)
        .map_err(|error| Error::new(format!("certificate bytes: {error:?}")))?;
    Ok(())
}

fn append_role(
    accounts: &mut Vec<AccountMeta>,
    record: &RecordPair,
    program: Pubkey,
    programdata: Pubkey,
) -> Result<()> {
    accounts.extend([
        AccountMeta::new_readonly(pubkey(&record.raw)?, false),
        AccountMeta::new_readonly(pubkey(&record.staging)?, false),
        AccountMeta::new_readonly(program, false),
        AccountMeta::new_readonly(programdata, false),
    ]);
    Ok(())
}

fn record<'a>(plan: &'a SuccessorPlan, name: &str) -> Result<&'a RecordPair> {
    plan.records
        .get(name)
        .ok_or_else(|| Error::new(format!("plan omitted {name} record")))
}

fn case_key(case: &SourceCase, step: &str, certificate: bool) -> Result<Pubkey> {
    let map = if certificate {
        &case.certificates
    } else {
        &case.funding
    };
    pubkey(
        map.get(step)
            .ok_or_else(|| Error::new(format!("case omitted {step}")))?,
    )
}

fn provider_program<'a>(
    provider: &'a ProviderEvidenceInput,
    name: &str,
) -> Result<&'a ProviderProgramInput> {
    provider
        .programs
        .iter()
        .find(|program| program.name == name)
        .ok_or_else(|| Error::new(format!("provider evidence omitted {name}")))
}

impl ProviderProgramInput {
    fn program_id(&self) -> Result<Pubkey> {
        pubkey(&self.program_id)
    }

    fn programdata_id(&self) -> Result<Pubkey> {
        pubkey(&self.programdata_id)
    }
}

fn same(
    before: &BTreeMap<String, AccountEvidence>,
    after: &BTreeMap<String, AccountEvidence>,
    name: &str,
) -> bool {
    before.get(name).map(|value| &value.account_sha256)
        == after.get(name).map(|value| &value.account_sha256)
}

fn validate_path(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::new(format!(
            "{label} must be an absolute regular file"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_or_hidden_plan_boundary_is_refused() {
        let plan = dummy_plan();
        let provider = ProviderEvidenceInput {
            rpc_url: "http://127.0.0.1:20890/".into(),
            provider_state_initialized: true,
            captured_release_identity_claimed: false,
            price_update: Pubkey::new_unique().to_string(),
            price_update_reclaimed: false,
            programs: vec![],
        };
        let args = RunArgs {
            rpc_url: provider.rpc_url.clone(),
            plan_path: "/tmp/a".into(),
            provider_evidence_path: "/tmp/b".into(),
            output: "/tmp/c".into(),
        };
        assert!(validate_inputs(&args, &plan, &provider).is_err());
    }

    #[test]
    fn conflated_primary_and_funded_materials_are_refused() {
        let mut plan = dummy_plan();
        let pair = RecordPair {
            raw: Pubkey::new_unique().to_string(),
            staging: Pubkey::new_unique().to_string(),
            schema_id: "33".repeat(32),
            content_sha256: plan.source_material_id.clone(),
        };
        plan.records.insert("source_material".into(), pair.clone());
        plan.records.insert("funded_source_material".into(), pair);
        assert!(validate_material_partition(&plan).is_err());
    }

    fn dummy_plan() -> SuccessorPlan {
        SuccessorPlan {
            schema: "wrong".into(),
            genesis_boundary: vec![],
            account_dir: "/tmp/unused".into(),
            registry: dummy_pin([1; 32]),
            core: dummy_pin([2; 32]),
            claims: dummy_pin([3; 32]),
            trading: dummy_pin([4; 32]),
            resolution: dummy_pin([5; 32]),
            custody: dummy_pin([6; 32]),
            activation: Pubkey::new_unique().to_string(),
            release_set_id: "11".repeat(32),
            records: BTreeMap::new(),
            result_domain_id: "11".repeat(32),
            source_material_id: "11".repeat(32),
            funded_source_material_id: "11".repeat(32),
            capability_manifest_id: "11".repeat(32),
            provider_release_id: "11".repeat(32),
            recovery_allocation_id: "11".repeat(32),
            exhaustion_allocation_id: "11".repeat(32),
            fixture_publish_time: plan_publish_time(),
            configured_max_age_seconds: 1,
            funded_max_age_seconds: 1,
            generation: GENERATION,
            primary: dummy_case(),
            lifecycle: dummy_case(),
            rollback: dummy_case(),
            genesis_accounts: BTreeMap::new(),
        }
    }

    fn dummy_pin(bytes: [u8; 32]) -> crate::model::ProgramPin {
        crate::model::ProgramPin {
            program_id: Pubkey::new_from_array(bytes).to_string(),
            programdata_id: Pubkey::new_unique().to_string(),
            elf_path: "/tmp/unused".into(),
            elf_sha256: "11".repeat(32),
            semantic_release_id: "22".repeat(32),
            upgrade_authority: None,
        }
    }

    fn dummy_case() -> SourceCase {
        SourceCase {
            market: Pubkey::new_unique().to_string(),
            state: Pubkey::new_unique().to_string(),
            certificates: BTreeMap::new(),
            funding: BTreeMap::new(),
            hostile_certificate_preoccupied: false,
        }
    }
}
