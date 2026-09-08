//! Persistent owned-loopback General vertical.
//!
//! This is intentionally a composition of the checked-mutable substrate and
//! existing General semantic owners.  It neither launches a second validator
//! nor constructs a parallel General frame: the substrate owns the validator
//! and administration lifetime, the local compiler observes the installed
//! accelerator, and `general_session` emits the route consumed by the existing
//! successor executor.

use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::Engine as _;
use dclutch_chain_bundle_builder::{
    admitted::{create_admitted_output_page_v1, validate_admitted_output_page_v1},
    frame::BuiltAccountV1,
};
use dclutch_trading::general::{
    hot_candidate_v3::general_hot_candidate_bank_len_v3, release_v3::GENERAL_ACTIONS_V3,
};
use serde_json::Value;
use solana_account::Account;
use solana_sdk::{
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    sysvar,
};

use crate::{
    Error, Result, daemon, general_capability_activation, general_local_market, general_market,
    general_session, general_successor_plan,
    rpc::Rpc,
    substrate::{self, SubstrateRequestV1},
};

pub(crate) struct RequestV1 {
    pub(crate) work: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) checked_release_gate: PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    pub(crate) seed: String,
    pub(crate) policy: PathBuf,
    pub(crate) compiler_release: PathBuf,
    pub(crate) toolchain: PathBuf,
    pub(crate) translation_validation: PathBuf,
    pub(crate) selection_policy: PathBuf,
    pub(crate) quote_surplus_beneficiary: Pubkey,
    /// A non-aliasing provisional table coordinate used only to derive the
    /// exact table set.  It is replaced by the observed frozen table before
    /// the successor executor sees the final route.
    pub(crate) bootstrap_lookup_table: Pubkey,
}

fn address(
    accounts: &std::collections::BTreeMap<String, crate::model::AccountEvidence>,
    name: &str,
) -> Result<String> {
    accounts
        .get(name)
        .map(|entry| entry.address.clone())
        .ok_or_else(|| Error::new(format!("General founding evidence omitted {name}")))
}

fn write_checked_release(plan: &crate::model::SuccessorPlan, path: &Path) -> Result<()> {
    let checked = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("plan omitted checked local mutable set"))?;
    let encoded = &checked
        .execution_release_set
        .checked_execution_release_set_base64;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| Error::new(format!("checked execution release set base64: {error}")))?;
    fs::write(path, bytes)
        .map_err(|error| Error::new(format!("checked execution release set: {error}")))
}

fn lookup_table(evidence: &Path) -> Result<Pubkey> {
    let value: Value = serde_json::from_slice(&fs::read(evidence)?)?;
    value
        .get("lookupTable")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("General lookup-table evidence omitted lookupTable"))?
        .parse()
        .map_err(|error| Error::new(format!("General lookup-table address: {error}")))
}

/// Allocate one page that can serve every selected General action at this
/// market width. Reusing that one caller-funded page is the transport contract;
/// changing actions changes its contents, never its address or its funding.
fn output_page_width(outcome_count: u32) -> Result<usize> {
    GENERAL_ACTIONS_V3
        .into_iter()
        .map(|action| {
            general_hot_candidate_bank_len_v3(action, outcome_count).map_err(|error| {
                Error::new(format!("General {action:?} output-page width: {error:?}"))
            })
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .ok_or_else(|| Error::new("General release has no actions"))
}

fn create_output_page(
    work: &Path,
    rpc: &mut Rpc,
    payer: &Keypair,
    accelerator: Pubkey,
    outcome_count: u32,
) -> Result<Pubkey> {
    let page = Keypair::new();
    let key_path =
        daemon::write_keypair_file(&work.join("keys"), "general-output-page.json", &page)?;
    let rent_account = rpc.required_account(sysvar::rent::ID, "Rent sysvar")?;
    let rent: Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let bytes = output_page_width(outcome_count)?;
    let create =
        create_admitted_output_page_v1(&payer.pubkey(), &page.pubkey(), &accelerator, bytes, &rent)
            .map_err(|error| Error::new(format!("General output-page creation: {error:?}")))?;
    let transaction = rpc.send_with_signers(
        "General: create reusable accelerator output page",
        &[create],
        payer,
        &[&page],
    )?;
    let floor = rpc.finalized_slot()?;
    let (slot, accounts) = rpc.finalized_accounts(&[page.pubkey()], floor)?;
    let observed = accounts
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| Error::new("created General output page is absent at finality"))?;
    let built = BuiltAccountV1 {
        key: page.pubkey(),
        account: Account {
            lamports: observed.lamports,
            data: observed.data.clone(),
            owner: observed.owner,
            executable: observed.executable,
            rent_epoch: observed.rent_epoch,
        },
        observed: None,
    };
    validate_admitted_output_page_v1(&built, &accelerator, bytes, &rent)
        .map_err(|error| Error::new(format!("finalized General output page: {error:?}")))?;
    let evidence = serde_json::json!({
        "schema": "dclutch-local-general-output-page-evidence-v1",
        "page": page.pubkey().to_string(),
        "owner": observed.owner.to_string(),
        "bytes": observed.data.len(),
        "lamports": observed.lamports,
        "executable": observed.executable,
        "minimumBalance": rent.minimum_balance(observed.data.len()),
        "finalizedSlot": slot,
        "signerPath": key_path.display().to_string(),
        "expense": "caller-funded reusable scratch rent; no Hoard principal and no current close route",
        "creationTransaction": transaction,
    });
    let evidence_path = work.join("general-output-page-creation.json");
    if evidence_path.exists() {
        return Err(Error::new(format!(
            "refusing to overwrite {}",
            evidence_path.display()
        )));
    }
    fs::write(
        &evidence_path,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    Ok(page.pubkey())
}

/// Found, activate, and execute the canonical General OpenBatch route while
/// the checked substrate remains owned by this process.
pub(crate) fn execute(request: RequestV1) -> Result<()> {
    fs::create_dir_all(&request.work)?;
    let substrate_dir = request.work.join("substrate");
    let checked = substrate::bring_up(&SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    let rpc_url = checked.rpc_url.clone();
    let payer_path = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .ok_or_else(|| Error::new("checked substrate omitted campaign-payer key"))?
        .clone();
    let payer_keypair = substrate::campaign_payer_keypair(&checked)?;
    let payer = payer_keypair.pubkey().to_string();

    let market = request.work.join("general-market.json");
    general_local_market::run(vec![
        "--plan".into(),
        checked.plan_path.display().to_string(),
        "--rpc-url".into(),
        rpc_url.clone(),
        "--general-policy".into(),
        request.policy.display().to_string(),
        "--general-compiler-release".into(),
        request.compiler_release.display().to_string(),
        "--general-toolchain".into(),
        request.toolchain.display().to_string(),
        "--general-translation-validation".into(),
        request.translation_validation.display().to_string(),
        "--general-selection-policy".into(),
        request.selection_policy.display().to_string(),
        "--general-quote-surplus-beneficiary".into(),
        request.quote_surplus_beneficiary.to_string(),
        "--output".into(),
        market.display().to_string(),
    ])?;
    let market_input: crate::model::MarketRunInput = serde_json::from_slice(&fs::read(&market)?)?;
    let outcome_count = general_market::general_market_derivation_v1(&market_input)?.outcome_count;

    let mut rpc = Rpc::connect(&rpc_url)?;
    let founding = substrate::found_market(
        &checked,
        &mut rpc,
        &market,
        &request.work.join("general-founding-execution.json"),
    )?;
    let accounts = &founding.market.accounts;
    let market_address = address(accounts, "founding_market")?;
    let result_domain = address(accounts, "result_domain_record")?;
    let portfolio = address(accounts, "portfolio_record")?;
    let linked_basis = address(accounts, "linked_liability_basis_record")?;

    let activation = request.work.join("general-capability-activation.json");
    general_capability_activation::run_owned_loopback(vec![
        "--rpc-url".into(),
        rpc_url.clone(),
        "--plan".into(),
        checked.plan_path.display().to_string(),
        "--campaign-report".into(),
        request
            .work
            .join("general-founding-execution.json")
            .display()
            .to_string(),
        "--payer-keypair".into(),
        payer_path.clone(),
        "--output".into(),
        activation.display().to_string(),
        "--execute".into(),
    ])?;

    let accelerator = checked
        .plan
        .general_accelerator
        .as_ref()
        .ok_or_else(|| Error::new("checked plan omitted General accelerator"))?
        .program_id
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("General accelerator address: {error}")))?;
    let output_page = create_output_page(
        &request.work,
        &mut rpc,
        &payer_keypair,
        accelerator,
        outcome_count,
    )?;

    let release = request.work.join("checked-execution-release-set.bin");
    write_checked_release(&checked.plan, &release)?;
    let bootstrap_report = request.work.join("general-openbatch-bootstrap-frame.json");
    let bootstrap_route = request.work.join("general-openbatch-bootstrap-route.json");
    general_session::run_owned_loopback(vec![
        "--rpc-url".into(),
        rpc_url.clone(),
        "--plan".into(),
        checked.plan_path.display().to_string(),
        "--market".into(),
        market_address.clone(),
        "--result-domain-record".into(),
        result_domain.clone(),
        "--portfolio-record".into(),
        portfolio.clone(),
        "--linked-basis-record".into(),
        linked_basis.clone(),
        "--payer".into(),
        payer.clone(),
        "--output".into(),
        bootstrap_report.display().to_string(),
        "--action".into(),
        "open-batch".into(),
        "--output-page".into(),
        output_page.to_string(),
        "--emit-route".into(),
        bootstrap_route.display().to_string(),
        "--lookup-table".into(),
        request.bootstrap_lookup_table.to_string(),
        "--checked-release".into(),
        release.display().to_string(),
    ])?;

    let bootstrap_lookup = request.work.join("general-openbatch-bootstrap-lookup.json");
    general_successor_plan::run_lookup_table_owned_loopback(vec![
        "--rpc-url".into(),
        rpc_url.clone(),
        "--route".into(),
        bootstrap_route.display().to_string(),
        "--payer-keypair".into(),
        payer_path.clone(),
        "--evidence".into(),
        bootstrap_lookup.display().to_string(),
        "--execute".into(),
    ])?;
    let exact_table = lookup_table(&bootstrap_lookup)?;

    let frame = request.work.join("general-openbatch-frame.json");
    let route = request.work.join("general-openbatch-route.json");
    general_session::run_owned_loopback(vec![
        "--rpc-url".into(),
        rpc_url.clone(),
        "--plan".into(),
        checked.plan_path.display().to_string(),
        "--market".into(),
        market_address,
        "--result-domain-record".into(),
        result_domain,
        "--portfolio-record".into(),
        portfolio,
        "--linked-basis-record".into(),
        linked_basis,
        "--payer".into(),
        payer,
        "--output".into(),
        frame.display().to_string(),
        "--action".into(),
        "open-batch".into(),
        "--output-page".into(),
        output_page.to_string(),
        "--emit-route".into(),
        route.display().to_string(),
        "--lookup-table".into(),
        exact_table.to_string(),
        "--checked-release".into(),
        release.display().to_string(),
    ])?;
    general_successor_plan::run_execute_owned_loopback(vec![
        "--rpc-url".into(),
        rpc_url,
        "--route".into(),
        route.display().to_string(),
        "--plan-output".into(),
        request
            .work
            .join("general-openbatch-plan.json")
            .display()
            .to_string(),
        "--payer-keypair".into(),
        payer_path,
        "--evidence".into(),
        request
            .work
            .join("general-openbatch-execution.json")
            .display()
            .to_string(),
        "--execute".into(),
    ])
}
