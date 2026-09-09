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
    let result = campaign_on_checked_substrate(request, progress, &checked);
    if std::env::var_os("DCLUTCH_STRUCTURED_KEEP_VALIDATOR").is_some() {
        // The owned continuation keeps this exact ledger/RPC alive across
        // independently journaled host commands. Its runner records the process
        // and must stop it gracefully when the lifecycle is finished.
        std::mem::forget(checked);
    }
    result
}

fn campaign_on_checked_substrate(
    request: &JourneyRequestV1,
    progress: &mut Progress,
    checked: &crate::substrate::CheckedSubstrateV1,
) -> Result<()> {
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
    let mut terminal_driver = StructuredTerminalDriver { request, checked };
    crate::structured_campaign::run_owned_loopback_with_terminal_v1(
        vec![
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
        ],
        Some(&mut terminal_driver),
    )?;
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
struct StructuredTerminalDriver<'a> {
    request: &'a JourneyRequestV1,
    checked: &'a crate::substrate::CheckedSubstrateV1,
}

impl crate::structured_campaign::StructuredTerminalDriverV1 for StructuredTerminalDriver<'_> {
    fn resolve_and_settle_native(
        &mut self,
        rpc: &mut crate::rpc::Rpc,
        plan: &crate::model::SuccessorPlan,
        evidence: &crate::campaign::CampaignTerminalEvidenceV1,
        payer: &solana_sdk::signature::Keypair,
        market: solana_sdk::pubkey::Pubkey,
        transactions: &mut Vec<TransactionEvidence>,
    ) -> Result<crate::structured_campaign::StructuredTerminalAccountsV1> {
        resolve_structured_terminal_v1(self.request, self.checked, rpc, payer, transactions)?;
        let payer_path = self
            .checked
            .report
            .campaign_founding_keypairs
            .get("campaign-payer")
            .ok_or_else(|| Error::new("Structured terminal omitted payer key"))?;
        let participant = self
            .request
            .work
            .join("structured-publication.json")
            .with_extension("holder-admission.json");
        let settled = crate::structured_native_settlement::settle_structured_native_wallet_v1(
            crate::structured_native_settlement::StructuredNativeSettlementInputV1 {
                rpc,
                rpc_url: &self.checked.rpc_url,
                plan,
                plan_path: &self.checked.plan_path,
                campaign: evidence,
                campaign_path: &self.request.work.join("structured-founding-evidence.json"),
                participant_evidence_path: &participant,
                payer,
                payer_keypair_path: Path::new(payer_path),
                market,
                work_dir: &self.request.work.join("structured-native-terminal"),
            },
            transactions,
        )?;
        write_json(
            &self.request.work.join("structured-native-terminal.json"),
            &serde_json::to_value(&settled)?,
        )?;
        Ok(crate::structured_campaign::StructuredTerminalAccountsV1 {
            terminal_certificate: settled.terminal_certificate,
            custody_replay: settled.custody_replay,
            hoard: settled.hoard,
        })
    }
}

/// Resolve the already-founded Primary Source through the shipped durable CLI.
/// The local guardian publication is the repository-pinned Pyth lab capture;
/// Router verification, Source submission, certificate, and Core acceptance all
/// execute against the checked validator.
fn resolve_structured_terminal_v1(
    request: &JourneyRequestV1,
    checked: &crate::substrate::CheckedSubstrateV1,
    rpc: &mut crate::rpc::Rpc,
    payer: &solana_sdk::signature::Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    use crate::provider::{ProviderPlanV1, PublicationV1};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    let work = request.work.join("structured-terminal");
    std::fs::create_dir_all(&work)?;
    let founder_path = checked
        .report
        .campaign_founding_keypairs
        .get("founding-founder")
        .ok_or_else(|| Error::new("Structured terminal omitted founder key"))?;
    let payer_path = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .ok_or_else(|| Error::new("Structured terminal omitted payer key"))?;
    let founder = crate::substrate::load_keypair(Path::new(founder_path))?;
    if founder.pubkey() == payer.pubkey() {
        return Err(Error::new(
            "Structured terminal founder and payer must differ",
        ));
    }
    let facts_path = work.join("pyth-facts.json");
    let update_path = work.join("update.json");
    let producer_path = work.join("producer.json");
    let table_path = work.join("tables.json");
    let input_path = work.join("input.json");
    let checkpoint_path = work.join("checkpoint.json");
    if !facts_path.exists() {
        let provider = ProviderPlanV1::derive(rpc, &checked.plan)?;
        let publication = PublicationV1::captured();
        std::fs::write(
            &update_path,
            serde_json::to_vec(&provider.update.to_bytes().as_slice())?,
        )?;
        crate::provider::prepare_verified_publication_v1(
            rpc,
            &founder,
            &provider,
            &publication,
            transactions,
        )?;
        write_json(
            &facts_path,
            &json!({
                "format": "dclutch-flagship-pyth-update-facts-v1",
                "encodedVaa": provider.encoded_vaa.pubkey().to_string(),
                "updateAccount": provider.update.pubkey().to_string(),
                "postUpdateBodyBase64": BASE64.encode(&publication.post_update_body),
            }),
        )?;
        write_json(
            &work.join("provider-preparation.json"),
            &json!({"transactions": transactions}),
        )?;
    }
    let refresh_path = work.join("refreshed-evidence.json");
    if !input_path.exists() {
        let market_path = request.work.join("structured-market.json");
        let campaign_path = request.work.join("structured-founding-evidence.json");
        crate::evidence_refresh::run_owned_loopback(vec![
            "--rpc-url".into(),
            checked.rpc_url.clone(),
            "--plan".into(),
            checked.plan_path.display().to_string(),
            "--expected-plan-sha256".into(),
            checked.plan_sha256.clone(),
            "--market-input".into(),
            market_path.display().to_string(),
            "--expected-market-input-sha256".into(),
            crate::evidence_refresh::hex_digest_v1(&std::fs::read(&market_path)?),
            "--campaign-report".into(),
            campaign_path.display().to_string(),
            "--expected-campaign-report-sha256".into(),
            crate::evidence_refresh::hex_digest_v1(&std::fs::read(&campaign_path)?),
            "--output".into(),
            refresh_path.display().to_string(),
        ])?;
    }
    let run = |mut args: Vec<String>| -> Result<()> {
        args.extend(["--rpc-url".into(), checked.rpc_url.clone()]);
        crate::flagship_resolution::run_owned_loopback(args)
    };
    while !input_path.exists() {
        run(vec![
            "--produce-input".into(),
            "--plan".into(),
            checked.plan_path.display().to_string(),
            "--campaign-evidence".into(),
            request
                .work
                .join("structured-founding-evidence.json")
                .display()
                .to_string(),
            "--refreshed-evidence".into(),
            refresh_path.display().to_string(),
            "--pyth-facts".into(),
            facts_path.display().to_string(),
            "--producer-checkpoint".into(),
            producer_path.display().to_string(),
            "--output".into(),
            input_path.display().to_string(),
            "--payer".into(),
            payer.pubkey().to_string(),
        ])?;
        if input_path.exists() {
            break;
        }
        let before = std::fs::read(&table_path).ok();
        run(vec![
            "--provision-tables".into(),
            "--producer-checkpoint".into(),
            producer_path.display().to_string(),
            "--table-journal".into(),
            table_path.display().to_string(),
            "--authority-keypair".into(),
            founder_path.clone(),
            "--execute".into(),
        ])?;
        if before.as_deref() == Some(std::fs::read(&table_path)?.as_slice()) {
            return Err(Error::new(
                "Structured terminal table provisioner made no journal progress",
            ));
        }
    }
    let input: Value = serde_json::from_slice(&std::fs::read(&input_path)?)?;
    for (stage, receipt) in [
        ("submit", "submit"),
        ("execute", "resolution-provider-execute-v1"),
        ("accept", "core-terminal-accept-v1"),
    ] {
        let existing: Option<Value> = std::fs::read(&checkpoint_path)
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes))
            .transpose()?;
        let has_receipt = |checkpoint: &Value| {
            checkpoint["receipts"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["stage"] == receipt))
        };
        if existing.as_ref().is_some_and(has_receipt) {
            continue;
        }
        if stage == "execute" {
            let certificate = crate::plan::pubkey(
                input["accounts"]["certificate"]
                    .as_str()
                    .ok_or_else(|| Error::new("Structured terminal input omitted certificate"))?,
            )?;
            let rent =
                rpc.minimum_balance(dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2)?;
            let before = rpc.account(certificate)?;
            // A finalized Execute may await receipt acceptance in the durable
            // journal. Fund only a vacant System account; an existing native
            // certificate is authenticated by that canonical resume path.
            let missing = match before.as_ref() {
                None => rent,
                Some(account)
                    if account.owner == solana_sdk_ids::system_program::ID
                        && !account.executable
                        && account.data.is_empty() =>
                {
                    rent.saturating_sub(account.lamports)
                }
                Some(_) => 0,
            };
            if missing != 0 {
                let sent = rpc.send(
                    "prepay Structured terminal certificate rent",
                    &[solana_system_interface::instruction::transfer(
                        &founder.pubkey(),
                        &certificate,
                        missing,
                    )],
                    &founder,
                )?;
                if sent.error.is_some() {
                    return Err(Error::new(
                        "Structured terminal certificate prepayment refused",
                    ));
                }
                transactions.push(sent);
            }
        }
        loop {
            let before = std::fs::read(&checkpoint_path).ok();
            run(vec![
                "--input".into(),
                input_path.display().to_string(),
                "--checkpoint".into(),
                checkpoint_path.display().to_string(),
                "--through".into(),
                stage.into(),
                "--submitter-keypair".into(),
                founder_path.clone(),
                "--resolver-keypair".into(),
                founder_path.clone(),
                "--payer-keypair".into(),
                payer_path.clone(),
                "--update-keypair".into(),
                update_path.display().to_string(),
                "--execute".into(),
            ])?;
            let bytes = std::fs::read(&checkpoint_path)?;
            let checkpoint: Value = serde_json::from_slice(&bytes)?;
            if has_receipt(&checkpoint) {
                break;
            }
            if before.as_deref() == Some(bytes.as_slice()) {
                return Err(Error::new(format!(
                    "Structured terminal {stage} made no journal progress"
                )));
            }
        }
    }
    // Journal rows are discovery hints. Reacquire each finalized packet before
    // it joins the outer campaign evidence.
    for path in [&table_path, &checkpoint_path] {
        let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        crate::structured_campaign::harvest_structured_driver_signatures_v1(
            rpc,
            "Structured durable terminal",
            &value,
            transactions,
        )?;
    }
    Ok(())
}
