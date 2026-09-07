//! Custody economics execution over the checked mutable validator substrate.
//!
//! The two accepted acts are executed by the successor's canonical commands;
//! this module owns only their ordering, retained-authority join, finalized
//! transaction capture, and refusal controls.

use std::{collections::BTreeMap, path::Path};

use dclutch_custody::upkeep_vault_v1::UpkeepVaultV1;
use dclutch_market::protocol_parameters::ProtocolParametersRecordV1;
use dclutch_operator::{
    protocol_parameters_v1::{
        found_protocol_parameters_instruction_v1, protocol_parameters_record_address_v1,
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
    let checked = crate::substrate::bring_up(&crate::substrate::SubstrateRequestV1 {
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
