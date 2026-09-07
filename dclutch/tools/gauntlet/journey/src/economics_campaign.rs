//! Custody economics execution over the checked mutable validator substrate.
//!
//! The two accepted acts are executed by the successor's canonical commands;
//! this module owns only their ordering, retained-authority join, finalized
//! transaction capture, and refusal controls.

use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

use dclutch_custody::upkeep_vault_v1::UpkeepVaultV1;
use dclutch_market::protocol_parameters::{
    ProtocolParametersChangeReceiptV1, ProtocolParametersRecordV1,
};
use dclutch_operator::{
    protocol_parameters_v1::{
        apply_protocol_parameters_instruction_v1, found_protocol_parameters_instruction_v1,
        propose_protocol_parameters_instruction_v1, protocol_parameters_receipt_address_v1,
        protocol_parameters_record_address_v1,
    },
    upkeep_vault_v1::{found_upkeep_vault_instruction_v1, upkeep_vault_address_v1},
};
use serde_json::{Value, json};
use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{
    Error, Result,
    economics_successor::{self, ClusterV1, RouteV1},
    journey::JourneyRequestV1,
    model::TransactionEvidence,
    rpc::{Rpc, RpcAccount},
};

const DEPOSIT_LAMPORTS_V1: u64 = 17_001;
const PARAMETERS_FOUNDING_AUTHORITY_REFUSAL_V1: u64 = 0x6104;
const PARAMETERS_DUPLICATE_RECORD_REFUSAL_V1: u64 = 0x6102;
const UPKEEP_DUPLICATE_VAULT_REFUSAL_V1: u64 = 0x6202;
const PARAMETERS_UNAUTHORIZED_GOVERNANCE_REFUSAL_V1: u64 = 0x6106;
const PARAMETERS_NO_PENDING_PROPOSAL_REFUSAL_V1: u64 = 0x6109;
const PARAMETERS_NOT_MATURED_REFUSAL_V1: u64 = 0x610A;
/// Provisional local-validator persistence ceiling. Lift only after measuring a
/// slower checked-mutable validator root; it names a refusal rather than
/// treating an unrooted transaction as restart-safe.
const PERSISTED_FINALIZATION_WAIT_V1: Duration = Duration::from_secs(90);

struct Progress {
    stage: String,
    stages: Vec<Value>,
    transactions: Vec<TransactionEvidence>,
    accounts: BTreeMap<String, crate::model::AccountEvidence>,
    rpc_url: String,
}

pub(crate) fn execute(request: JourneyRequestV1) -> Result<()> {
    std::fs::create_dir_all(&request.work)?;
    let mut progress = Progress {
        stage: "checked substrate".into(),
        stages: Vec::new(),
        transactions: Vec::new(),
        accounts: BTreeMap::new(),
        rpc_url: String::new(),
    };
    let result = campaign(&request, &mut progress);
    write_json(
        &request.transcript,
        &json!({
            "schema": "dclutch-economics-custody-campaign-v1",
            "evidenceLevel": "local-validator",
            "hostSourceRevision": std::env::var("DCLUTCH_HOST_SOURCE_REVISION").ok(),
            "runtimeSourceRevision": request.expected_source_revision,
            "checkedReleaseGateSha256": request.expected_gate_sha256,
            "completed": result.is_ok(),
            "wall": result.as_ref().err().map(|error| json!({"stage": progress.stage, "sentence": error.to_string()})),
            "stages": progress.stages,
        }),
    )?;
    write_json(
        &request.work.join("evidence.json"),
        &json!({
            "schema": "dclutch-local-successor-run-evidence-v2",
            "rpc_url": progress.rpc_url,
            "transactions": progress.transactions,
            "accounts": progress.accounts,
        }),
    )?;
    result
}

fn campaign(request: &JourneyRequestV1, progress: &mut Progress) -> Result<()> {
    let substrate_dir = request.work.join("substrate");
    std::fs::create_dir_all(&substrate_dir)?;
    let mut checked = crate::substrate::bring_up(&crate::substrate::SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    progress.rpc_url = checked.rpc_url.clone();
    progress
        .stages
        .push(json!({"stage": progress.stage, "outcome": "executed"}));

    let custody = crate::plan::pubkey(&checked.plan.custody.program_id)?;
    let authority = crate::substrate::authority_keypair(&checked)?;
    let authority_path = checked
        .report
        .keypairs
        .get("core-upgrade-authority")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| Error::new("checked substrate omitted core-upgrade-authority key path"))?;
    let planned_authority = checked
        .plan
        .custody
        .upgrade_authority
        .as_deref()
        .ok_or_else(|| Error::new("checked mutable Custody pin omitted its retained authority"))?
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("Custody retained authority: {error}")))?;
    if planned_authority != authority.pubkey() {
        return Err(Error::new(
            "Custody plan authority and retained local authority key differ",
        ));
    }
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let hostile = solana_sdk::signature::Keypair::new();
    progress.transactions.push(rpc.airdrop(
        "economics campaign: fund hostile governance signer",
        hostile.pubkey(),
        10_000_000,
    )?);

    progress.stage = "parameters Found hostile authority".into();
    let parameters = protocol_parameters_record_address_v1(custody);
    let hostile_found = found_protocol_parameters_instruction_v1(
        custody,
        hostile.pubkey(),
        hostile.pubkey().to_bytes(),
    )
    .map_err(|error| Error::new(format!("build hostile parameters Found: {error:?}")))?;
    let hostile_tx = rpc.send_expected_failure(
        "parameters Found hostile authority",
        &[hostile_found],
        &hostile,
    )?;
    require_custom_refusal(
        &hostile_tx,
        PARAMETERS_FOUNDING_AUTHORITY_REFUSAL_V1,
        "ProtocolParametersSbfErrorV1::FoundingAuthority",
    )?;
    progress.transactions.push(hostile_tx);
    if rpc.account(parameters)?.is_some() {
        return Err(Error::new("hostile parameters Found left a record behind"));
    }
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "refused",
        "exactRefusal": "ProtocolParametersSbfErrorV1::FoundingAuthority",
        "custom": PARAMETERS_FOUNDING_AUTHORITY_REFUSAL_V1,
    }));

    progress.stage = "parameters Found accepted".into();
    let parameters_report = economics_successor::run_value(
        RouteV1::Parameters,
        ClusterV1::OwnedLoopback,
        command_arguments(&checked.rpc_url, custody, &authority, &authority_path, None),
    )?;
    capture_report_transaction(
        &mut rpc,
        progress,
        "parameters Found",
        &parameters_report,
        "/signature",
    )?;
    let parameter_account = rpc.required_account(parameters, "parameters poststate")?;
    ProtocolParametersRecordV1::decode(&parameter_account.data)
        .map_err(|error| Error::new(format!("parameters poststate decode: {error:?}")))?;
    progress
        .stages
        .push(json!({"stage": progress.stage, "outcome": "executed", "report": parameters_report}));

    progress.stage = "parameters Found duplicate".into();
    let before_parameters = snapshot(&parameter_account);
    let duplicate_parameters = found_protocol_parameters_instruction_v1(
        custody,
        authority.pubkey(),
        authority.pubkey().to_bytes(),
    )
    .map_err(|error| Error::new(format!("build duplicate parameters Found: {error:?}")))?;
    let duplicate_tx = rpc.send_expected_failure(
        "parameters Found duplicate",
        &[duplicate_parameters],
        &authority,
    )?;
    require_custom_refusal(
        &duplicate_tx,
        PARAMETERS_DUPLICATE_RECORD_REFUSAL_V1,
        "ProtocolParametersSbfErrorV1::Record",
    )?;
    progress.transactions.push(duplicate_tx);
    require_unchanged(
        &before_parameters,
        &rpc.required_account(parameters, "parameters after duplicate")?,
        "parameters duplicate",
    )?;
    progress.stages.push(json!({"stage": progress.stage, "outcome": "refused", "exactRefusal": "ProtocolParametersSbfErrorV1::Record", "custom": PARAMETERS_DUPLICATE_RECORD_REFUSAL_V1}));

    let payer = crate::substrate::campaign_payer_keypair(&checked)?;
    let baseline_parameters = ProtocolParametersRecordV1::decode(&parameter_account.data)
        .map_err(|error| Error::new(format!("parameters proposal prestate decode: {error:?}")))?
        .parameters;
    let proposed_cap = baseline_parameters
        .closer_reward_cap_lamports
        .checked_add(1)
        .ok_or_else(|| Error::new("parameters proposal cap overflow"))?;
    let proposed_body = dclutch_market::protocol_parameters::ProtocolParametersV1 {
        closer_reward_cap_lamports: proposed_cap,
        ..baseline_parameters
    };
    let hostile_body = proposed_body;
    progress.stage = "parameters Propose hostile authority rollback".into();
    let before_propose = snapshot(&parameter_account);
    let hostile_propose =
        propose_protocol_parameters_instruction_v1(custody, hostile.pubkey(), hostile_body)
            .map_err(|error| Error::new(format!("build hostile parameters Propose: {error:?}")))?;
    let hostile_propose_tx = rpc.send_expected_failure(
        "parameters Propose hostile authority",
        &[hostile_propose],
        &hostile,
    )?;
    require_custom_refusal(
        &hostile_propose_tx,
        PARAMETERS_UNAUTHORIZED_GOVERNANCE_REFUSAL_V1,
        "ProtocolParametersSbfErrorV1::UnauthorizedGovernance",
    )?;
    progress.transactions.push(hostile_propose_tx);
    require_unchanged(
        &before_propose,
        &rpc.required_account(parameters, "parameters after hostile Propose")?,
        "parameters hostile Propose",
    )?;
    progress.stages.push(json!({"stage":progress.stage,"outcome":"refused","exactRefusal":"ProtocolParametersSbfErrorV1::UnauthorizedGovernance","custom":PARAMETERS_UNAUTHORIZED_GOVERNANCE_REFUSAL_V1}));

    progress.stage = "parameters Propose accepted".into();
    let propose_report = economics_successor::run_value(
        RouteV1::Propose,
        ClusterV1::OwnedLoopback,
        parameters_command_arguments(
            &checked.rpc_url,
            custody,
            &authority,
            &authority_path,
            proposed_cap,
        ),
    )?;
    capture_report_transaction(
        &mut rpc,
        progress,
        "parameters Propose",
        &propose_report,
        "/signature",
    )?;
    let after_propose = rpc.required_account(parameters, "parameters after Propose")?;
    let proposed_record = ProtocolParametersRecordV1::decode(&after_propose.data)
        .map_err(|error| Error::new(format!("parameters proposed poststate decode: {error:?}")))?;
    if !proposed_record.pending.is_standing()
        || proposed_record.pending.digest != proposed_body.body_digest()
        || proposed_record.parameters != baseline_parameters
    {
        return Err(Error::new(
            "parameters Propose did not leave the active body unchanged with one exact standing change",
        ));
    }
    progress
        .stages
        .push(json!({"stage":progress.stage,"outcome":"executed","report":propose_report}));

    progress.stage = "parameters Apply before maturity rollback".into();
    let before_early_apply = snapshot(&after_propose);
    let early_body = proposed_body;
    let early_apply = apply_protocol_parameters_instruction_v1(
        custody,
        payer.pubkey(),
        early_body,
        proposed_record.parameters.generation + 1,
    )
    .map_err(|error| Error::new(format!("build early parameters Apply: {error:?}")))?;
    let early_apply_tx =
        rpc.send_expected_failure("parameters Apply before maturity", &[early_apply], &payer)?;
    require_custom_refusal(
        &early_apply_tx,
        PARAMETERS_NOT_MATURED_REFUSAL_V1,
        "ProtocolParametersSbfErrorV1::ProposalNotMatured",
    )?;
    let early_apply_slot = early_apply_tx.slot;
    progress.transactions.push(early_apply_tx);
    require_unchanged(
        &before_early_apply,
        &rpc.required_account(parameters, "parameters after early Apply")?,
        "parameters early Apply",
    )?;
    progress.stages.push(json!({"stage":progress.stage,"outcome":"refused","exactRefusal":"ProtocolParametersSbfErrorV1::ProposalNotMatured","custom":PARAMETERS_NOT_MATURED_REFUSAL_V1}));

    progress.stage = "parameters persist finalized proposal before warp".into();
    let finalized_before_warp = wait_for_finalized_slot(&mut rpc, early_apply_slot)?;
    let durable_proposal =
        rpc.required_account(parameters, "parameters after durable finalization")?;
    require_unchanged(
        &before_early_apply,
        &durable_proposal,
        "parameters durable finalization",
    )?;
    progress.stages.push(json!({
        "stage":progress.stage,
        "outcome":"executed",
        "transactionSlot":early_apply_slot,
        "finalizedSlot":finalized_before_warp
    }));

    progress.stage = "parameters Apply accepted after governed notice".into();
    let earliest_apply_slot = proposed_record.pending.earliest_apply_slot;
    let finalized_after_warp = checked.restart_at_finalized_slot(earliest_apply_slot)?;
    if finalized_after_warp < earliest_apply_slot {
        return Err(Error::new(
            "validator warp did not reach the proposal's earliest Apply slot",
        ));
    }
    let mut resumed_rpc = Rpc::connect(&checked.rpc_url)?;
    let resumed =
        resumed_rpc.required_account(parameters, "parameters after governed notice warp")?;
    require_unchanged(
        &snapshot(&durable_proposal),
        &resumed,
        "parameters governed notice warp",
    )?;
    let apply_report = economics_successor::run_value(
        RouteV1::Apply,
        ClusterV1::OwnedLoopback,
        parameters_command_arguments(
            &checked.rpc_url,
            custody,
            &authority,
            &authority_path,
            proposed_cap,
        ),
    )?;
    capture_report_transaction(
        &mut resumed_rpc,
        progress,
        "parameters Apply",
        &apply_report,
        "/signature",
    )?;
    let applied = ProtocolParametersRecordV1::decode(
        &resumed_rpc
            .required_account(parameters, "parameters after Apply")?
            .data,
    )
    .map_err(|error| Error::new(format!("parameters Apply poststate decode: {error:?}")))?;
    if applied.pending.is_standing()
        || applied.parameters.generation != proposed_record.parameters.generation + 1
        || applied.parameters.closer_reward_cap_lamports != proposed_cap
    {
        return Err(Error::new(
            "parameters Apply did not activate exactly the standing proposed body",
        ));
    }
    let receipt_address =
        protocol_parameters_receipt_address_v1(custody, applied.parameters.generation);
    let receipt_account =
        resumed_rpc.required_account(receipt_address, "parameters Apply receipt")?;
    if receipt_account.owner != custody {
        return Err(Error::new("parameters Apply receipt has the wrong owner"));
    }
    let receipt = ProtocolParametersChangeReceiptV1::decode(&receipt_account.data)
        .map_err(|error| Error::new(format!("parameters Apply receipt decode: {error:?}")))?;
    if receipt.previous_digest != proposed_record.parameters.body_digest()
        || receipt.new_digest != applied.parameters.body_digest()
        || receipt.generation != applied.parameters.generation
        || receipt.activation_slot != applied.parameters.activation_slot
        || receipt.delay_slots != proposed_record.parameters.change_delay_slots
        || receipt.proposed_at_slot.checked_add(receipt.delay_slots) != Some(earliest_apply_slot)
    {
        return Err(Error::new(
            "parameters Apply receipt does not bind the prior body, notice, and activated body",
        ));
    }
    progress.stages.push(json!({
        "stage":progress.stage,
        "outcome":"executed",
        "earliestApplySlot":earliest_apply_slot,
        "finalizedSlotAfterWarp":finalized_after_warp,
        "report":apply_report
    }));

    progress.stage = "parameters Withdraw after Apply refusal rollback".into();
    let after_apply =
        resumed_rpc.required_account(parameters, "parameters before post-Apply Withdraw")?;
    let post_apply_withdraw =
        dclutch_operator::protocol_parameters_v1::withdraw_protocol_parameters_instruction_v1(
            custody,
            authority.pubkey(),
        )
        .map_err(|error| Error::new(format!("build post-Apply parameters Withdraw: {error:?}")))?;
    let post_apply_withdraw_tx = resumed_rpc.send_expected_failure(
        "parameters Withdraw after Apply",
        &[post_apply_withdraw],
        &authority,
    )?;
    require_custom_refusal(
        &post_apply_withdraw_tx,
        PARAMETERS_NO_PENDING_PROPOSAL_REFUSAL_V1,
        "ProtocolParametersSbfErrorV1::NoPendingProposal",
    )?;
    progress.transactions.push(post_apply_withdraw_tx);
    require_unchanged(
        &snapshot(&after_apply),
        &resumed_rpc.required_account(parameters, "parameters after post-Apply Withdraw")?,
        "parameters post-Apply Withdraw",
    )?;
    progress.stages.push(json!({"stage":progress.stage,"outcome":"refused","exactRefusal":"ProtocolParametersSbfErrorV1::NoPendingProposal","custom":PARAMETERS_NO_PENDING_PROPOSAL_REFUSAL_V1}));

    progress.stage = "parameters Propose for Withdraw accepted".into();
    let withdraw_cap = proposed_cap
        .checked_add(1)
        .ok_or_else(|| Error::new("parameters withdrawal proposal cap overflow"))?;
    let withdraw_propose_report = economics_successor::run_value(
        RouteV1::Propose,
        ClusterV1::OwnedLoopback,
        parameters_command_arguments(
            &checked.rpc_url,
            custody,
            &authority,
            &authority_path,
            withdraw_cap,
        ),
    )?;
    capture_report_transaction(
        &mut resumed_rpc,
        progress,
        "parameters Propose for Withdraw",
        &withdraw_propose_report,
        "/signature",
    )?;
    let before_withdraw = resumed_rpc.required_account(parameters, "parameters before Withdraw")?;
    let withdraw_proposed =
        ProtocolParametersRecordV1::decode(&before_withdraw.data).map_err(|error| {
            Error::new(format!(
                "parameters withdraw proposal poststate decode: {error:?}"
            ))
        })?;
    let withdraw_body = dclutch_market::protocol_parameters::ProtocolParametersV1 {
        closer_reward_cap_lamports: withdraw_cap,
        ..applied.parameters
    };
    if !withdraw_proposed.pending.is_standing()
        || withdraw_proposed.pending.digest != withdraw_body.body_digest()
        || withdraw_proposed.parameters != applied.parameters
    {
        return Err(Error::new(
            "parameters second Propose did not preserve the applied body and pin its exact withdrawal body",
        ));
    }
    progress.stages.push(
        json!({"stage":progress.stage,"outcome":"executed","report":withdraw_propose_report}),
    );

    progress.stage = "parameters Withdraw accepted".into();
    let withdraw_report = economics_successor::run_value(
        RouteV1::Withdraw,
        ClusterV1::OwnedLoopback,
        command_arguments(&checked.rpc_url, custody, &authority, &authority_path, None),
    )?;
    capture_report_transaction(
        &mut resumed_rpc,
        progress,
        "parameters Withdraw",
        &withdraw_report,
        "/signature",
    )?;
    let withdrawn = ProtocolParametersRecordV1::decode(
        &resumed_rpc
            .required_account(parameters, "parameters after Withdraw")?
            .data,
    )
    .map_err(|error| Error::new(format!("parameters withdraw poststate decode: {error:?}")))?;
    if withdrawn.pending.is_standing() || withdrawn.parameters != applied.parameters {
        return Err(Error::new(
            "parameters Withdraw did not clear only the standing proposal",
        ));
    }
    progress
        .stages
        .push(json!({"stage":progress.stage,"outcome":"executed","report":withdraw_report}));

    let payer_path = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| Error::new("checked substrate omitted campaign-payer key path"))?;
    progress.transactions.push(rpc.airdrop(
        "economics campaign: fund voluntary upkeep payer",
        payer.pubkey(),
        10_000_000,
    )?);
    progress.stage = "upkeep Found and nonzero Deposit Credit".into();
    let upkeep_report = economics_successor::run_value(
        RouteV1::Upkeep,
        ClusterV1::OwnedLoopback,
        command_arguments(
            &checked.rpc_url,
            custody,
            &payer,
            &payer_path,
            Some(DEPOSIT_LAMPORTS_V1),
        ),
    )?;
    capture_report_transaction(
        &mut rpc,
        progress,
        "upkeep Found",
        &upkeep_report,
        "/foundSignature",
    )?;
    capture_report_transaction(
        &mut rpc,
        progress,
        "upkeep Deposit Credit",
        &upkeep_report,
        "/creditSignature",
    )?;
    let upkeep = upkeep_vault_address_v1(custody);
    let upkeep_account = rpc.required_account(upkeep, "upkeep poststate")?;
    let vault = UpkeepVaultV1::decode(&upkeep_account.data)
        .map_err(|error| Error::new(format!("upkeep poststate decode: {error:?}")))?;
    if vault.inflow_deposit != DEPOSIT_LAMPORTS_V1
        || vault.inflow_donation != 0
        || vault.credit_count != 1
    {
        return Err(Error::new(
            "upkeep command report and decoded nonzero Credit state disagree",
        ));
    }
    progress
        .stages
        .push(json!({"stage": progress.stage, "outcome": "executed", "report": upkeep_report}));

    progress.stage = "upkeep Found duplicate".into();
    let before_upkeep = snapshot(&upkeep_account);
    let duplicate_upkeep = found_upkeep_vault_instruction_v1(custody, payer.pubkey())
        .map_err(|error| Error::new(format!("build duplicate upkeep Found: {error:?}")))?;
    let duplicate_tx =
        rpc.send_expected_failure("upkeep Found duplicate", &[duplicate_upkeep], &payer)?;
    require_custom_refusal(
        &duplicate_tx,
        UPKEEP_DUPLICATE_VAULT_REFUSAL_V1,
        "UpkeepVaultSbfErrorV1::Vault",
    )?;
    progress.transactions.push(duplicate_tx);
    require_unchanged(
        &before_upkeep,
        &rpc.required_account(upkeep, "upkeep after duplicate")?,
        "upkeep duplicate",
    )?;
    progress.stages.push(json!({"stage": progress.stage, "outcome": "refused", "exactRefusal": "UpkeepVaultSbfErrorV1::Vault", "custom": UPKEEP_DUPLICATE_VAULT_REFUSAL_V1}));
    Ok(())
}

fn parameters_command_arguments(
    rpc_url: &str,
    custody: Pubkey,
    payer: &solana_sdk::signature::Keypair,
    payer_path: &Path,
    cap: u64,
) -> Vec<String> {
    let mut values = command_arguments(rpc_url, custody, payer, payer_path, None);
    let keypair = values.pop().expect("command args carry keypair value");
    let flag = values.pop().expect("command args carry keypair flag");
    values.extend([
        "--closer-reward-cap-lamports".into(),
        cap.to_string(),
        flag,
        keypair,
    ]);
    values
}

fn command_arguments(
    rpc_url: &str,
    custody: Pubkey,
    payer: &solana_sdk::signature::Keypair,
    payer_path: &Path,
    amount: Option<u64>,
) -> Vec<String> {
    let mut values = vec![
        "--rpc-url".into(),
        rpc_url.into(),
        "--custody".into(),
        custody.to_string(),
        "--payer".into(),
        payer.pubkey().to_string(),
        "--execute".into(),
    ];
    if let Some(amount) = amount {
        values.extend(["--amount".into(), amount.to_string()]);
    }
    values.extend(["--payer-keypair".into(), payer_path.display().to_string()]);
    values
}

fn capture_report_transaction(
    rpc: &mut Rpc,
    progress: &mut Progress,
    label: &str,
    report: &Value,
    pointer: &str,
) -> Result<()> {
    let signature = report
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} report omitted {pointer}")))?
        .parse()
        .map_err(|error| Error::new(format!("{label} signature: {error}")))?;
    let landed = rpc
        .finalized_signed_packet(label, signature, false)?
        .ok_or_else(|| Error::new(format!("{label} finalized transaction missing")))?;
    progress.transactions.push(landed.evidence);
    Ok(())
}

fn wait_for_finalized_slot(rpc: &mut Rpc, transaction_slot: u64) -> Result<u64> {
    let deadline = Instant::now() + PERSISTED_FINALIZATION_WAIT_V1;
    loop {
        let finalized = rpc.finalized_slot()?;
        if finalized >= transaction_slot {
            return Ok(finalized);
        }
        if Instant::now() >= deadline {
            return Err(Error::new(format!(
                "transaction at slot {transaction_slot} did not reach finalized slot within {} seconds; refuse to restart an unpersisted ledger",
                PERSISTED_FINALIZATION_WAIT_V1.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn require_custom_refusal(
    transaction: &TransactionEvidence,
    expected: u64,
    label: &str,
) -> Result<()> {
    let actual = transaction
        .error
        .as_ref()
        .and_then(|value| value.get("InstructionError"))
        .and_then(Value::as_array)
        .and_then(|values| values.get(1))
        .and_then(|value| value.get("Custom"))
        .and_then(Value::as_u64);
    if actual != Some(expected) {
        return Err(Error::new(format!(
            "expected {label} Custom({expected}), got {:?}",
            transaction.error
        )));
    }
    Ok(())
}

fn snapshot(account: &RpcAccount) -> (u64, Pubkey, Vec<u8>) {
    (account.lamports, account.owner, account.data.clone())
}
fn require_unchanged(
    before: &(u64, Pubkey, Vec<u8>),
    after: &RpcAccount,
    label: &str,
) -> Result<()> {
    if before.0 != after.lamports || before.1 != after.owner || before.2 != after.data {
        return Err(Error::new(format!("{label} refusal mutated Custody state")));
    }
    Ok(())
}

fn write_json(path: &Path, document: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(document)?),
    )?;
    Ok(())
}
