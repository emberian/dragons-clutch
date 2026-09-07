//! Owned-loopback publication producer for the Structured receipt lifecycle.
//!
//! The command deliberately stops before a caller can submit a lifecycle
//! packet unless it has first rebuilt the per-Market representation closure
//! from finalized Product and selected-release records.  It is the mutation
//! boundary for the seven immutable Structured records; the Journey owns the
//! surrounding checked substrate and founding sequence.

use std::path::PathBuf;

use dclutch_operator::representation_composition::native_categorical_v1::{
    NativeBasisCompositionInputV1, compile_native_basis_composition_v1,
};
use serde_json::json;
use solana_sdk::signature::{Keypair, Signer};

use crate::{
    Error, Result,
    campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::pubkey,
    rpc::Rpc,
    structured_activation,
    structured_claims_producer::{
        compile_structured_publication_closure_v1,
        // Publication is followed by a fresh, independent full composition
        // admission; the compiler's in-memory bytes are not evidence.
        hydrate_structured_publication_input_same_slot_v1,
        publish_structured_publication_closure_v1,
    },
    structured_composition_admission::hydrate_structured_composition_admission_v1,
};

/// Public owned-loopback command for the Structured publication predecessor.
pub(crate) const COMMAND_V1: &str = "local-private-validator-structured-claims-v1";

#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    market_input: PathBuf,
    campaign_report: PathBuf,
    payer_keypair: PathBuf,
    output: PathBuf,
    execute: bool,
}

/// Run the authenticated Structured publication predecessor on an owned
/// validator.  The output is a durable input to the receipt-activation step,
/// never a claim that a receipt mint was created.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    if arguments.output.exists() {
        return Err(Error::new(format!(
            "Structured campaign refuses to overwrite {}",
            arguments.output.display()
        )));
    }
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, None)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
    let plan: SuccessorPlan = serde_json::from_slice(&std::fs::read(&arguments.plan)?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let market_input = structured_activation::read_pinned_structured_document_v1(
        &arguments.market_input,
        &sha256_hex(&std::fs::read(&arguments.market_input)?),
        "market input",
    )?;
    let campaign_report = structured_activation::read_pinned_structured_document_v1(
        &arguments.campaign_report,
        &sha256_hex(&std::fs::read(&arguments.campaign_report)?),
        "campaign report",
    )?;
    let evidence = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_report,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let mut rpc = Rpc::connect(&arguments.rpc_url)?;
    let activation = structured_activation::hydrate_activate_receipt_v1(
        &mut rpc,
        &market_input,
        &campaign_report,
        0,
    )?;
    let artifacts = structured_activation::hydrate_selected_activate_receipt_artifacts_v1(
        &mut rpc,
        registry,
        &market_input,
        activation.slot,
    )?;
    let provisional = hydrate_structured_publication_input_same_slot_v1(
        &mut rpc,
        registry,
        &market_input,
        &evidence,
        activation.slot,
        activation.market,
        claims,
        Vec::new(),
        2,
        Vec::new(),
    )?;
    // The shape is authored from the finalized Product width, never from a
    // request flag.  The selected release's RequestProfile bounds K at three;
    // this producer uses the first K canonical Product coordinates and unit
    // coefficients over a denominator of two.  The compiled closure remains
    // the authority for every derived graph, exposure, and receipt identity.
    let native = compile_native_basis_composition_v1(NativeBasisCompositionInputV1 {
        market: activation.market.to_bytes(),
        release_set: artifacts.token_behavior_selection.release_set(),
        product_record_bytes: &provisional.product_record_body,
        result_domain_bytes: &provisional.result_domain_body,
        portfolio_bytes: &provisional.portfolio_body,
        product_basis_bytes: &provisional.product_basis_body,
        price_gate_bytes: provisional.price_gate_body.as_deref(),
    })
    .map_err(|error| {
        Error::new(format!(
            "Structured campaign Product composition: {error:?}"
        ))
    })?;
    let width = native.width().min(3);
    if width == 0 {
        return Err(Error::new("Structured campaign Product width is zero"));
    }
    let coordinates = (0..width).collect::<Vec<_>>();
    let coefficients = vec![
        1_u64;
        usize::try_from(width).map_err(|_| Error::new(
            "Structured campaign width overflows host"
        ))?
    ];
    // Redo the same-slot join with the explicit canonical recipe after the
    // width is authenticated.  This is intentionally not a mutation of the
    // provisional value above: it proves recipe input cannot sneak through a
    // stale serial read.
    let input = hydrate_structured_publication_input_same_slot_v1(
        &mut rpc,
        registry,
        &market_input,
        &evidence,
        activation.slot,
        activation.market,
        claims,
        coordinates.clone(),
        2,
        coefficients.clone(),
    )?;
    let closure = compile_structured_publication_closure_v1(&input)?;
    let mut transactions = Vec::new();
    let payer = if arguments.execute {
        Some(Keypair::new_from_array(crate::campaign::read_keypair_file(
            &arguments.payer_keypair,
            "payer",
        )?))
    } else {
        None
    };
    let published = if arguments.execute {
        Some(publish_structured_publication_closure_v1(
            &mut rpc,
            registry,
            payer
                .as_ref()
                .ok_or_else(|| Error::new("Structured campaign omitted payer"))?,
            &closure,
            input_release_slot(&artifacts, activation.slot),
            &mut transactions,
        )?)
    } else {
        None
    };
    let admitted = if let Some(published) = published.as_ref() {
        let snapshot = hydrate_structured_composition_admission_v1(
            &mut rpc,
            registry,
            claims,
            &evidence,
            &published.records,
            published.slot,
        )?;
        let plan = snapshot.plan()?;
        Some(json!({
            "slot": snapshot.slot(),
            "representationWidth": plan.representation_width(),
            "productWidth": plan.product_width(),
            "executionDescriptorRaw": plan.admitted().execution_descriptor_record().raw_account.to_string(),
            "exposureRaw": plan.admitted().exposure_record().raw_account.to_string(),
        }))
    } else {
        None
    };
    write_json(
        &arguments.output,
        &json!({
            "schema": "dclutch-owned-loopback-structured-publication-v1",
            "cluster": "owned-loopback",
            "executed": arguments.execute,
            "market": activation.market.to_string(),
            "claims": claims.to_string(),
            "registry": registry.to_string(),
            "payer": payer.as_ref().map(|value| value.pubkey().to_string()),
            "finalizedSlot": published.as_ref().map(|value| value.slot).unwrap_or(activation.slot),
            "productCoordinates": coordinates,
            "denominator": 2_u64,
            "coefficients": coefficients,
            "publication": published.as_ref().map(|value| value.records.iter().map(|record| json!({
                "raw": record.raw.to_string(), "staging": record.staging.to_string(), "schema": crate::plan::hex(&record.schema), "digest": crate::plan::hex(&record.digest)
            })).collect::<Vec<_>>()),
            "compositionAdmission": admitted,
            "transactions": transactions,
        }),
    )
}

fn input_release_slot(
    artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
    activation_slot: u64,
) -> u64 {
    artifacts.slot.max(activation_slot)
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut values = std::collections::BTreeMap::new();
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if flag == "--execute" {
            if execute {
                return Err(Error::new("--execute was given twice"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} needs a value")))?;
        if values.insert(flag.clone(), value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let required_path = |flag: &str| -> Result<PathBuf> {
        let path = PathBuf::from(
            values
                .get(flag)
                .ok_or_else(|| Error::new(format!("{flag} is required")))?,
        );
        if !path.is_absolute() {
            return Err(Error::new(format!("{flag} must be absolute")));
        }
        Ok(path)
    };
    let rpc_url = values
        .get("--rpc-url")
        .cloned()
        .ok_or_else(|| Error::new("--rpc-url is required"))?;
    for key in values.keys() {
        if !matches!(
            key.as_str(),
            "--rpc-url"
                | "--plan"
                | "--market-input"
                | "--campaign-report"
                | "--payer-keypair"
                | "--output"
        ) {
            return Err(Error::new(format!(
                "unknown Structured campaign argument: {key}"
            )));
        }
    }
    Ok(ArgumentsV1 {
        rpc_url,
        plan: required_path("--plan")?,
        market_input: required_path("--market-input")?,
        campaign_report: required_path("--campaign-report")?,
        payer_keypair: required_path("--payer-keypair")?,
        output: required_path("--output")?,
        execute,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicate_execute_refuses() {
        let error = super::parse_arguments(vec!["--execute".into(), "--execute".into()])
            .expect_err("duplicate execution flag must refuse");
        assert_eq!(error.to_string(), "--execute was given twice");
    }
}
