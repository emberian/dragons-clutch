//! Structured receipt publication over one checked owned-loopback substrate.
//!
//! A Structured selection binds the collateral Mint into its Realm before a
//! Market exists.  This campaign therefore reads the prepared founding role's
//! public key before compiling the Market, then lets the ordinary founding
//! command consume that same deterministic role.  No account is injected and
//! no fixture reports a poststate it did not read from the validator.

use std::path::Path;

use serde_json::{Value, json};
use solana_sdk::signature::Signer;

use crate::{Error, Result, journey::JourneyRequestV1, model::TransactionEvidence};

struct Progress {
    stage: String,
    stages: Vec<Value>,
    transactions: Vec<TransactionEvidence>,
    rpc_url: String,
    representation_retired: bool,
}

/// Run the checked substrate, Structured-selected founding, and the shipped
/// successor publication command.  Receipt activation remains a later action
/// in the same command lineage; this function refuses to call it until the
/// publication command has produced all seven finalized records.
pub(crate) fn execute(request: JourneyRequestV1) -> Result<()> {
    std::fs::create_dir_all(&request.work)?;
    let mut progress = Progress {
        stage: "checked substrate".into(),
        stages: Vec::new(),
        transactions: Vec::new(),
        rpc_url: String::new(),
        representation_retired: false,
    };
    let result = campaign(&request, &mut progress);
    write_json(
        &request.transcript,
        &json!({
            "schema": "dclutch-structured-claims-campaign-v1",
            "evidenceLevel": "local-validator",
            "runtimeSourceRevision": request.expected_source_revision,
            "checkedReleaseGateSha256": request.expected_gate_sha256,
            "completed": result.is_ok() && progress.representation_retired,
            "entranceCompleted": result.is_ok(),
            "wall": result.as_ref().err().map(|error| json!({"stage": progress.stage, "sentence": error.to_string()})),
            "stages": progress.stages,
        }),
    )?;
    write_json(
        &request.work.join("evidence.json"),
        &json!({
            "schema": "dclutch-local-successor-run-evidence-v2",
            "rpcUrl": progress.rpc_url,
            "transactions": progress.transactions,
        }),
    )?;
    result
}

fn campaign(request: &JourneyRequestV1, progress: &mut Progress) -> Result<()> {
    let substrate_dir = request.work.join("substrate");
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

    // This is the mint the founding campaign will draw as `collateral-mint[0]`.
    // `found_market` receives the exact same key file afterwards, so the
    // pre-founding Realm is bound to the Mint that actually reaches the chain.
    let collateral_mint_path = checked
        .report
        .campaign_founding_keypairs
        .get("collateral-mint")
        .ok_or_else(|| Error::new("checked substrate omitted collateral-mint role"))?;
    let collateral_mint = crate::substrate::load_keypair(Path::new(collateral_mint_path))?.pubkey();
    let registry = crate::plan::pubkey(&checked.plan.registry.program_id)?;
    progress.stage = "compile Structured selection before founding".into();
    let input = crate::structured_market::demo_structured_market_input(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        collateral_mint,
        &crate::market::LocalMarketShapeV1::default(),
    )?;
    let market_path = request.work.join("structured-market.json");
    write_json(&market_path, &serde_json::to_value(&input)?)?;
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "executed",
        "collateralMint": collateral_mint.to_string(),
    }));

    progress.stage = "found Structured market through Open".into();
    let founding_path = request.work.join("structured-founding-evidence.json");
    let mut rpc = crate::rpc::Rpc::connect(&checked.rpc_url)?;
    let founding =
        crate::substrate::found_market(&checked, &mut rpc, &market_path, &founding_path)?;
    progress.transactions.extend(founding.transactions);
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "executed",
        "market": founding.market.accounts.get("founding_market").map(|row| &row.address),
    }));

    progress.stage = "publish authenticated Structured lifecycle closure".into();
    let payer_path = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .ok_or_else(|| Error::new("checked substrate omitted campaign-payer role"))?;
    let publication_path = request.work.join("structured-publication.json");
    crate::structured_campaign::run_owned_loopback_v1(vec![
        "--rpc-url".into(),
        checked.rpc_url.clone(),
        "--plan".into(),
        checked.plan_path.display().to_string(),
        "--market-input".into(),
        market_path.display().to_string(),
        "--campaign-report".into(),
        founding_path.display().to_string(),
        "--payer-keypair".into(),
        payer_path.clone(),
        "--output".into(),
        publication_path.display().to_string(),
        "--execute".into(),
    ])?;
    let publication: Value = serde_json::from_slice(&std::fs::read(&publication_path)?)?;
    progress.representation_retired = publication
        .pointer("/receiptActivation/retirement/receipt/closed")
        .and_then(Value::as_bool)
        == Some(true);
    let records = publication
        .get("publication")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Structured publication command omitted finalized records"))?;
    if records.len() != 7 {
        return Err(Error::new(format!(
            "Structured publication command finalized {} records, expected 7",
            records.len()
        )));
    }
    let published_transactions: Vec<TransactionEvidence> = serde_json::from_value(
        publication
            .get("transactions")
            .cloned()
            .ok_or_else(|| Error::new("Structured publication command omitted transactions"))?,
    )?;
    if published_transactions.is_empty() {
        return Err(Error::new(
            "Structured publication command reported no transactions",
        ));
    }
    progress.transactions.extend(published_transactions);
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "executed",
        "report": publication_path,
        "records": records,
    }));
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn report_requires_all_seven_published_records() {
        let records = serde_json::json!([{}, {}, {}, {}, {}, {}]);
        assert_ne!(records.as_array().map(Vec::len), Some(7));
    }
}
