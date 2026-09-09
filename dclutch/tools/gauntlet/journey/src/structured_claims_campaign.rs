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

/// Run the checked substrate and one selected Structured lifecycle through
/// representation settlement, resource retirement, and capability-root close.
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

/// Resume the original founded Market on its still-running checked validator.
/// No validator handle is acquired or restarted; native exteriors reauthenticate
/// live state and continue their existing journals.
pub(crate) fn continue_existing(request: JourneyRequestV1) -> Result<()> {
    let plan_path = request.work.join("substrate/plan.json");
    let plan_bytes = std::fs::read(&plan_path)?;
    let plan: crate::model::SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let pins = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("Structured continuation omitted checked local pins"))?;
    if pins.source_revision != request.expected_source_revision
        || pins.source_tree_sha256 != request.expected_source_tree_sha256
        || pins.checked_release_gate_sha256 != request.expected_gate_sha256
    {
        return Err(Error::new(
            "Structured continuation source differs from its retained cohort",
        ));
    }
    let rpc_url = format!("http://127.0.0.1:{}", request.rpc_port);
    let mut context = StructuredTerminalContextV1 {
        rpc_url: rpc_url.clone(),
        plan_path: plan_path.clone(),
        plan,
        plan_sha256: crate::evidence_refresh::hex_digest_v1(&plan_bytes),
        campaign_founding_keypairs: ["campaign-payer", "founding-founder"]
            .into_iter()
            .map(|role| {
                (
                    role.to_owned(),
                    request
                        .work
                        .join("substrate/prepare/keys")
                        .join(format!("{role}.json"))
                        .display()
                        .to_string(),
                )
            })
            .collect(),
    };
    if !structured_founding_complete_v1(&request.work.join("structured-founding-evidence.json"))? {
        let administration: Value = serde_json::from_slice(&std::fs::read(
            request.work.join("substrate/administration-evidence.json"),
        )?)?;
        let mut rpc = crate::rpc::Rpc::connect(&rpc_url)?;
        let genesis = rpc.call("getGenesisHash", &json!([]))?;
        authenticate_retained_setup_v1(&administration, &rpc_url, &context.plan_sha256, &genesis)?;
        let key_dir = request.work.join("substrate/prepare/keys");
        context.campaign_founding_keypairs = crate::campaign::FOUNDING_REQUIRED_ROLES
            .iter()
            .copied()
            .chain([
                crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
                crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
                "founding-founder",
            ])
            .map(|role| {
                (
                    role.to_owned(),
                    key_dir.join(format!("{role}.json")).display().to_string(),
                )
            })
            .collect();
        let public = crate::local_mutable::local_campaign_public_identities_v1(
            crate::plan::hex32(&request.seed)?,
        )?;
        let retained_founder =
            crate::substrate::load_keypair(&key_dir.join("founding-founder.json"))?
                .pubkey()
                .to_string();
        if public.get(crate::seed::role::FOUNDING_FOUNDER) != Some(&retained_founder) {
            return Err(Error::new(
                "Structured continuation seed differs from retained founding identity",
            ));
        }
        let mut progress = Progress {
            stage: "retained checked substrate".into(),
            stages: Vec::new(),
            transactions: Vec::new(),
            rpc_url: rpc_url.clone(),
            representation_retired: false,
        };
        let result = ensure_structured_founding_v1(&request, &mut progress, &context, &public);
        write_json(
            &request.work.join("structured-founding-continuation.json"),
            &json!({
                "stages": progress.stages, "transactions": progress.transactions,
                "completed": result.is_ok(), "wall": result.as_ref().err().map(ToString::to_string),
            }),
        )?;
        result?;
    }
    let mut driver = StructuredTerminalDriver {
        request: &request,
        checked: &context,
    };
    let output = request.work.join("structured-publication.json");
    crate::structured_campaign::run_owned_loopback_with_terminal_v1(
        vec![
            "--rpc-url".into(),
            rpc_url,
            "--plan".into(),
            plan_path.display().to_string(),
            "--market-input".into(),
            request
                .work
                .join("structured-market.json")
                .display()
                .to_string(),
            "--campaign-report".into(),
            request
                .work
                .join("structured-founding-evidence.json")
                .display()
                .to_string(),
            "--payer-keypair".into(),
            context.campaign_founding_keypairs["campaign-payer"].clone(),
            "--output".into(),
            output.display().to_string(),
            "--execute".into(),
        ],
        Some(&mut driver),
    )?;
    let publication: Value = serde_json::from_slice(&std::fs::read(&output)?)?;
    if publication
        .pointer("/receiptActivation/retirement/rootClose/rootClosed")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(Error::new(
            "Structured continuation did not close its capability root",
        ));
    }
    write_json(
        &request.transcript,
        &json!({
            "schema": "dclutch-structured-claims-continuation-v1",
            "evidenceLevel": "local-validator", "completed": true,
            "runtimeSourceRevision": request.expected_source_revision,
            "checkedReleaseGateSha256": request.expected_gate_sha256,
            "publication": output,
        }),
    )
}

/// Found a fresh immutable selection on the retained checked bank. The old
/// Market, its reports and all substrate accounts remain independently named.
pub(crate) fn fresh_selection(arguments: Vec<String>) -> Result<()> {
    let mut retained_work = None;
    let mut common = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| Error::new(format!("{flag} needs a value")))?;
        if flag == "--retained-work" {
            if retained_work
                .replace(std::path::PathBuf::from(value))
                .is_some()
            {
                return Err(Error::new("--retained-work was given twice"));
            }
        } else {
            common.extend([flag, value]);
        }
    }
    let retained_work = retained_work
        .filter(|path| path.is_absolute())
        .ok_or_else(|| Error::new("--retained-work must be an absolute path"))?;
    let request = crate::parse_journey_request(common)?;
    if request.work.exists() || request.transcript.exists() {
        return Err(Error::new(
            "Structured fresh selection requires unused work and transcript paths",
        ));
    }
    let plan_bytes = std::fs::read(retained_work.join("substrate/plan.json"))?;
    let plan: crate::model::SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let pins = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("Structured fresh selection omitted checked local pins"))?;
    if pins.source_revision != request.expected_source_revision
        || pins.source_tree_sha256 != request.expected_source_tree_sha256
        || pins.checked_release_gate_sha256 != request.expected_gate_sha256
    {
        return Err(Error::new(
            "Structured fresh selection differs from retained cohort",
        ));
    }
    let administration_bytes =
        std::fs::read(retained_work.join("substrate/administration-evidence.json"))?;
    let administration: Value = serde_json::from_slice(&administration_bytes)?;
    let rpc_url = format!("http://127.0.0.1:{}", request.rpc_port);
    let mut rpc = crate::rpc::Rpc::connect(&rpc_url)?;
    let genesis = rpc.call("getGenesisHash", &json!([]))?;
    authenticate_retained_setup_v1(
        &administration,
        &rpc_url,
        &crate::evidence_refresh::hex_digest_v1(&plan_bytes),
        &genesis,
    )?;
    let seed = crate::plan::hex32(&request.seed)?;
    let public = crate::local_mutable::local_campaign_public_identities_v1(seed)?;
    let old_founder = crate::substrate::load_keypair(
        &retained_work.join("substrate/prepare/keys/founding-founder.json"),
    )?
    .pubkey()
    .to_string();
    if public.get(crate::seed::role::FOUNDING_FOUNDER) == Some(&old_founder) {
        return Err(Error::new(
            "Structured fresh selection must use a fresh founding seed",
        ));
    }
    std::fs::create_dir(&request.work)?;
    let substrate = request.work.join("substrate");
    std::fs::create_dir(&substrate)?;
    std::fs::create_dir(substrate.join("prepare"))?;
    crate::local_mutable::prepare_local_founding_keys_v1(&substrate.join("prepare/keys"), seed)?;
    std::fs::write(substrate.join("plan.json"), &plan_bytes)?;
    std::fs::write(
        substrate.join("administration-evidence.json"),
        &administration_bytes,
    )?;
    write_json(
        &request.work.join("retained-substrate.json"),
        &json!({
            "schema": "dclutch-structured-fresh-selection-on-retained-substrate-v1",
            "retainedWork": retained_work, "rpcUrl": rpc_url, "genesisHash": genesis,
            "planSha256": crate::evidence_refresh::hex_digest_v1(&plan_bytes),
            "runtimeSourceRevision": request.expected_source_revision,
            "checkedReleaseGateSha256": request.expected_gate_sha256,
        }),
    )?;
    continue_existing(request)
}

fn campaign(request: &JourneyRequestV1, progress: &mut Progress) -> Result<()> {
    crate::substrate::require_token_2022_fixture_v1()?;
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

    let context = StructuredTerminalContextV1 {
        rpc_url: checked.rpc_url.clone(),
        plan_path: checked.plan_path.clone(),
        plan: checked.plan.clone(),
        plan_sha256: checked.plan_sha256.clone(),
        campaign_founding_keypairs: checked.report.campaign_founding_keypairs.clone(),
    };
    ensure_structured_founding_v1(
        request,
        progress,
        &context,
        &checked.report.campaign_public_identities,
    )?;
    let market_path = request.work.join("structured-market.json");
    let founding_path = request.work.join("structured-founding-evidence.json");
    progress.stage = "publish authenticated Structured lifecycle closure".into();
    let payer_path = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .ok_or_else(|| Error::new("checked substrate omitted campaign-payer role"))?;
    let publication_path = request.work.join("structured-publication.json");
    let mut terminal_driver = StructuredTerminalDriver {
        request,
        checked: &context,
    };
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
        .pointer("/receiptActivation/retirement/rootClose/rootClosed")
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

/// Continue compilation and founding through the original stage owners, reusing
/// saved input and completed reports; the shipped commands authenticate live state.
fn ensure_structured_founding_v1(
    request: &JourneyRequestV1,
    progress: &mut Progress,
    checked: &StructuredTerminalContextV1,
    public_identities: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    if structured_founding_complete_v1(&request.work.join("structured-founding-evidence.json"))? {
        return Ok(());
    }
    // This is the mint the founding campaign will draw as `collateral-mint[0]`.
    // `found_market` receives the exact same key file afterwards, so the
    // pre-founding Realm is bound to the Mint that actually reaches the chain.
    let collateral_mint_path = checked
        .campaign_founding_keypairs
        .get("collateral-mint")
        .ok_or_else(|| Error::new("checked substrate omitted collateral-mint role"))?;
    let collateral_mint = crate::substrate::load_keypair(Path::new(collateral_mint_path))?.pubkey();
    let registry = crate::plan::pubkey(&checked.plan.registry.program_id)?;
    progress.stage = "compile Structured selection before founding".into();
    let market_path = request.work.join("structured-market.json");
    if !market_path.exists() {
        let input = crate::structured_market::demo_structured_market_input(
            &checked.plan_path,
            &checked.rpc_url,
            registry,
            collateral_mint,
            &crate::market::LocalMarketShapeV1::default(),
        )?;
        write_json(&market_path, &serde_json::to_value(&input)?)?;
    }
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "executed",
        "collateralMint": collateral_mint.to_string(),
    }));

    progress.stage = "found Structured market through Open".into();
    let founding_path = request.work.join("structured-founding-evidence.json");
    let mut rpc = crate::rpc::Rpc::connect(&checked.rpc_url)?;
    let founding = crate::substrate::found_market_from_roles_v1(
        &checked.rpc_url,
        &checked.plan_path,
        &checked.campaign_founding_keypairs,
        public_identities,
        &mut rpc,
        &market_path,
        &founding_path,
    )?;
    progress.transactions.extend(founding.transactions);
    progress.stages.push(json!({
        "stage": progress.stage,
        "outcome": "executed",
        "market": founding.market.accounts.get("founding_market").map(|row| &row.address),
    }));

    Ok(())
}

fn structured_founding_complete_v1(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let evidence: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    Ok(evidence
        .pointer("/execution/completed")
        .and_then(Value::as_bool)
        == Some(true))
}

fn authenticate_retained_setup_v1(
    report: &Value,
    rpc_url: &str,
    plan_sha256: &str,
    genesis: &Value,
) -> Result<()> {
    let endpoint = crate::rpc::validate_loopback_url(rpc_url)?;
    if genesis.as_str().is_none()
        || report.get("rpc_url").and_then(Value::as_str) != Some(endpoint.as_str())
        || report.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256)
        || report.get("through_stage").and_then(Value::as_str) != Some("activation")
        || report
            .pointer("/execution/completed")
            .and_then(Value::as_bool)
            != Some(true)
        || report.get("genesis_hash") != Some(genesis)
    {
        return Err(Error::new(
            "Structured retained setup differs from its completed administration evidence",
        ));
    }
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
    fn retained_setup_pins_exact_plan_chain_endpoint_and_completed_activation() {
        use super::authenticate_retained_setup_v1;
        use serde_json::json;
        let report = json!({"rpc_url":"http://127.0.0.1:30400/", "plan_sha256":"plan-digest",
            "through_stage":"activation", "genesis_hash":"original-bank", "execution":{"completed":true}});
        assert!(
            authenticate_retained_setup_v1(
                &report,
                "http://127.0.0.1:30400",
                "plan-digest",
                &json!("original-bank")
            )
            .is_ok()
        );
        for field in [
            "rpc_url",
            "plan_sha256",
            "through_stage",
            "genesis_hash",
            "execution",
        ] {
            let mut hostile = report.clone();
            hostile[field] = json!("substituted");
            assert_eq!(
                authenticate_retained_setup_v1(
                    &hostile,
                    "http://127.0.0.1:30400",
                    "plan-digest",
                    &json!("original-bank")
                )
                .expect_err("substituted retained setup must refuse")
                .to_string(),
                "Structured retained setup differs from its completed administration evidence"
            );
        }
        let mut unproven = report;
        unproven["genesis_hash"] = serde_json::Value::Null;
        assert_eq!(
            authenticate_retained_setup_v1(
                &unproven,
                "http://127.0.0.1:30400",
                "plan-digest",
                &serde_json::Value::Null
            )
            .expect_err("missing genesis must refuse")
            .to_string(),
            "Structured retained setup differs from its completed administration evidence"
        );
    }

    #[test]
    fn report_requires_all_seven_published_records() {
        let records = serde_json::json!([{}, {}, {}, {}, {}, {}]);
        assert_ne!(records.as_array().map(Vec::len), Some(7));
    }
}
struct StructuredTerminalContextV1 {
    rpc_url: String,
    plan_path: std::path::PathBuf,
    plan: crate::model::SuccessorPlan,
    plan_sha256: String,
    campaign_founding_keypairs: std::collections::BTreeMap<String, String>,
}

struct StructuredTerminalDriver<'a> {
    request: &'a JourneyRequestV1,
    checked: &'a StructuredTerminalContextV1,
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
        resolve_structured_terminal_v1(
            self.request,
            self.checked,
            rpc,
            payer,
            false,
            transactions,
        )?;
        let payer_path = self
            .checked
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

    fn finish_source_before_retirement(
        &mut self,
        rpc: &mut crate::rpc::Rpc,
        payer: &solana_sdk::signature::Keypair,
        market: solana_sdk::pubkey::Pubkey,
        transactions: &mut Vec<TransactionEvidence>,
    ) -> Result<Value> {
        resolve_structured_terminal_v1(self.request, self.checked, rpc, payer, true, transactions)?;
        let work = self.request.work.join("structured-terminal");
        let terminal = crate::flagship_resolution::authenticate_direct_resolution_terminal_v1(
            rpc,
            &work.join("input.json"),
            &work.join("checkpoint.json"),
        )?;
        if terminal.market != market.to_string() {
            return Err(Error::new(
                "Structured completed provider names another Market",
            ));
        }
        let value = serde_json::to_value(terminal)?;
        write_json(&work.join("complete.json"), &value)?;
        Ok(value)
    }
}

/// Resolve the already-founded Primary Source through the shipped durable CLI.
/// The local guardian publication is the repository-pinned Pyth lab capture;
/// Router verification, Source submission, certificate, and Core acceptance all
/// execute against the checked validator.
fn resolve_structured_terminal_v1(
    request: &JourneyRequestV1,
    checked: &StructuredTerminalContextV1,
    rpc: &mut crate::rpc::Rpc,
    payer: &solana_sdk::signature::Keypair,
    finish_provider: bool,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    use crate::provider::{ProviderPlanV1, PublicationV1};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    let work = request.work.join("structured-terminal");
    std::fs::create_dir_all(&work)?;
    let founder_path = checked
        .campaign_founding_keypairs
        .get("founding-founder")
        .ok_or_else(|| Error::new("Structured terminal omitted founder key"))?;
    let payer_path = checked
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
    let stages = [
        ("submit", "submit"),
        ("execute", "resolution-provider-execute-v1"),
        ("accept", "core-terminal-accept-v1"),
        ("reclaim", "reclaim"),
        ("complete", ""),
    ];
    for &(stage, receipt) in stages.iter().take(if finish_provider { 5 } else { 3 }) {
        let existing: Option<Value> = std::fs::read(&checkpoint_path)
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes))
            .transpose()?;
        let has_receipt = |checkpoint: &Value| {
            if stage == "complete" {
                return checkpoint["verifiedTerminal"].as_bool() == Some(true);
            }
            checkpoint["receipts"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["stage"] == receipt))
        };
        if existing.as_ref().is_some_and(has_receipt) {
            continue;
        }
        if stage == "reclaim" {
            let reclaim_after = input["reclaimAfterUnixSeconds"].as_i64().ok_or_else(|| {
                Error::new("Structured provider input omitted its immutable reclaim floor")
            })?;
            wait_for_structured_reclaim_floor_v1(rpc, reclaim_after)?;
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

/// Scheduling hint only: the canonical Reclaim builder and native handler
/// independently enforce the immutable floor against their own fresh Clock.
fn wait_for_structured_reclaim_floor_v1(
    rpc: &mut crate::rpc::Rpc,
    reclaim_after: i64,
) -> Result<()> {
    loop {
        let key = solana_sdk_ids::sysvar::clock::ID;
        let snapshot = crate::terminal_lifecycle::finalized_snapshot(rpc, &[key])?;
        let account = snapshot
            .account(key)
            .map_err(|error| Error::new(format!("Structured reclaim Clock: {error}")))?;
        let clock = dclutch_operator::observation::decode_clock(account)
            .map_err(|error| Error::new(format!("Structured reclaim Clock: {error:?}")))?;
        if clock.unix_timestamp >= reclaim_after {
            return Ok(());
        }
        let remaining = reclaim_after
            .checked_sub(clock.unix_timestamp)
            .ok_or_else(|| Error::new("Structured reclaim wait interval overflows"))?;
        eprintln!(
            "Structured provider reclaim remains time-locked for {remaining} seconds; Core stays Terminal"
        );
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
