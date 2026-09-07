//! Owned-loopback publication producer for the Structured receipt lifecycle.
//!
//! The command deliberately stops before a caller can submit a lifecycle
//! packet unless it has first rebuilt the per-Market representation closure
//! from finalized Product and selected-release records.  It is the mutation
//! boundary for the seven immutable Structured records; the Journey owns the
//! surrounding checked substrate and founding sequence.

use std::path::PathBuf;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims::{
    rational_kernel::RepresentationDescriptorV2,
    rational_lifecycle::{
        LIFECYCLE_HEADER_BYTES_V2, LifecycleRequestV2,
        hot_v6::STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1,
    },
    structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
};
use dclutch_market::{
    CoreState,
    capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    capability_program::hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_STRATEGY_RAW_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
    },
    realm::REALM_SCHEMA_RELEASE_ID_V1,
};
use dclutch_operator::structured_activation_bundle_v1::{
    STRUCTURED_CAPABILITY_ROOT_TAIL_V1, structured_activation_request_v1,
};
use dclutch_operator::{
    capability_seal_v1::{CapabilitySealInstructionInputV1, capability_seal_instruction_v1},
    observation::decode_rent,
    rational_lifecycle_hot::CheckedRationalLifecycleHotOuterV3,
    representation_composition::native_categorical_v1::{
        NativeBasisCompositionInputV1, compile_native_basis_composition_v1,
    },
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use serde_json::json;
use sha2::Digest as _;
use solana_sdk::{
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use crate::{
    Error, Result,
    campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::pubkey,
    rational_lifecycle_hot_frame::AuthenticatedRationalLifecycleHotFrameV1,
    rpc::Rpc,
    selected_capability_activation::{
        SelectedActivationRecordPairV1, SelectedCapabilityActivationInputV1,
        build_selected_capability_activation_plan_v1, execute_selected_capability_activation_v1,
    },
    structured_activation,
    structured_claims_producer::{
        compile_structured_publication_closure_v1,
        // Publication is followed by a fresh, independent full composition
        // admission; the compiler's in-memory bytes are not evidence.
        hydrate_structured_publication_input_same_slot_v1,
        publish_structured_publication_closure_v1,
    },
    structured_composition_admission::hydrate_structured_composition_admission_v1,
    structured_physical_frame::{
        ActivateReceiptPhysicalInputsV1, activate_receipt_claims_instruction_v1,
    },
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
    // Root creation is deliberately after the family closure is visible at
    // Registry and before any lifecycle child can be constructed.  The
    // generic executor owns the Core/Trading frame and verifies the ledger
    // transition; this family only authenticates its selector-255 artifacts.
    let root_activation = if let Some(published) = published.as_ref() {
        let payer = payer
            .as_ref()
            .ok_or_else(|| Error::new("Structured campaign omitted payer"))?;
        Some(activate_structured_root_v1(
            &mut rpc,
            payer,
            &plan,
            &market_input,
            &evidence,
            activation.market,
            published.slot,
            &mut transactions,
        )?)
    } else {
        None
    };
    let receipt_activation = if let (Some(_published), Some(root_activation), Some(payer)) =
        (published.as_ref(), root_activation.as_ref(), payer.as_ref())
    {
        Some(activate_structured_receipt_v1(
            &mut rpc,
            payer,
            &plan,
            &market_input,
            &evidence,
            &artifacts,
            activation.market,
            root_activation,
            &mut transactions,
        )?)
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
            "rootActivation": root_activation,
            "receiptActivation": receipt_activation,
            "transactions": transactions,
        }),
    )
}

fn selected_pair_v1(
    registry: solana_sdk::pubkey::Pubkey,
    schema: [u8; 32],
    body: &[u8],
) -> Result<SelectedActivationRecordPairV1> {
    let content: [u8; 32] = sha2::Sha256::digest(body).into();
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Structured activation schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("Structured activation content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
    };
    let (raw, raw_bump) = derive(key.raw_record_pda_seeds());
    let (staging, staging_bump) = derive(key.staging_cursor_pda_seeds());
    Ok(SelectedActivationRecordPairV1 {
        raw,
        staging,
        schema,
        content,
        bumps: [raw_bump, staging_bump],
    })
}

/// Execute selector-255 from the founded Market and report-named ledgers.
///
/// The report supplies routing coordinates only.  Record content and all root
/// semantics remain selected from the immutable Structured closure and live
/// Core Market state.
fn activate_structured_root_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    market_input: &[u8],
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    market: Pubkey,
    minimum_slot: u64,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    let input: crate::model::MarketRunInput = serde_json::from_slice(market_input)?;
    let selected = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Structured market omitted selected capability"))?;
    if selected.family != "structured" || selected.records.len() <= 53 {
        return Err(Error::new(
            "Structured root activation omitted selector-255 artifact bank",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let program_set_body = crate::runtime::decode_hex(&selected.program_set_hex)?;
    let config_body = crate::runtime::decode_hex(&selected.config_hex)?;
    let token_behavior = dclutch_custody::token_svm::TokenBehaviorSelectionV2::decode(&config_body)
        .map_err(|error| Error::new(format!("Structured root Token behavior: {error:?}")))?;
    let realm = pair_from_digest_v1(registry, REALM_SCHEMA_RELEASE_ID_V1, token_behavior.realm())?;
    let manifest_body = crate::runtime::decode_hex(&input.capability_manifest_hex)?;
    let manifest = selected_pair_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_body,
    )?;
    let program_set = selected_pair_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        &program_set_body,
    )?;
    let config = selected_pair_v1(
        registry,
        dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        &config_body,
    )?;
    let record = |index: usize, label: &str| -> Result<SelectedActivationRecordPairV1> {
        let row = selected
            .records
            .get(index)
            .ok_or_else(|| Error::new(format!("Structured root activation omitted {label}")))?;
        let schema = crate::plan::hex32(&row.schema_hex)?;
        selected_pair_v1(
            registry,
            schema,
            &crate::runtime::decode_hex(&row.body_hex)?,
        )
    };
    let account_profile = record(51, "root activation account profile")?;
    let effect = record(52, "root activation effect")?;
    let descriptor = record(53, "root activation descriptor")?;
    let selected_ledger = evidence
        .accounts
        .get("direct_trading_funding_ledger")
        .ok_or_else(|| Error::new("Structured founding report omitted Trading funding ledger"))?;
    let resolution_ledger = evidence
        .accounts
        .get("resolution_funding_ledger")
        .or_else(|| evidence.accounts.get("founding_funding_ledger_v2_0"))
        .or_else(|| evidence.accounts.get("founding_funding_ledger_v2_1"))
        .ok_or_else(|| {
            Error::new("Structured founding report omitted Resolution funding ledger")
        })?;
    let selected_funding_ledger = pubkey(&selected_ledger.address)?;
    let mut resolution_funding_ledger = pubkey(&resolution_ledger.address)?;
    if resolution_funding_ledger == selected_funding_ledger {
        resolution_funding_ledger = evidence
            .accounts
            .get("founding_funding_ledger_v2_0")
            .into_iter()
            .chain(evidence.accounts.get("founding_funding_ledger_v2_1"))
            .filter_map(|row| pubkey(&row.address).ok())
            .find(|coordinate| *coordinate != selected_funding_ledger)
            .ok_or_else(|| {
                Error::new("Structured founding report lacks the dependency funding ledger")
            })?;
    }
    // This is the one mutable/immutable observation boundary for activation.
    // Every key is derived above from the selected closure or founded report;
    // no pre-publication Market image or later singleton read enters the plan.
    let addresses = vec![
        market,
        realm.raw,
        realm.staging,
        manifest.raw,
        manifest.staging,
        program_set.raw,
        program_set.staging,
        config.raw,
        config.staging,
        account_profile.raw,
        account_profile.staging,
        effect.raw,
        effect.staging,
        descriptor.raw,
        descriptor.staging,
        selected_funding_ledger,
        resolution_funding_ledger,
    ];
    let (observed_slot, accounts) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    let facts_slot = activation_snapshot_slot_v1(observed_slot, minimum_slot)?;
    let at =
        |index: usize, label: &str| -> Result<crate::rpc::RpcAccount> {
            accounts.get(index).cloned().flatten().ok_or_else(|| {
                Error::new(format!("Structured activation snapshot omitted {label}"))
            })
        };
    let market_account = at(0, "Core Market")?;
    let realm_account = at(1, "Realm record")?;
    let manifest_account = at(3, "capability manifest")?;
    let program_set_account = at(5, "ProgramSet record")?;
    let config_account = at(7, "config record")?;
    let profile_account = at(9, "root activation account profile")?;
    let effect_account = at(11, "root activation effect")?;
    let descriptor_account = at(13, "root activation descriptor")?;
    let ledger_account = at(15, "selected funding ledger")?;
    let state = dclutch_market::CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Structured root Core Market: {error:?}")))?;
    if state.identity.realm_id.to_bytes() != token_behavior.realm()
        || state.identity.capability_manifest.to_bytes() != manifest.content
        || manifest_account.data != manifest_body
    {
        return Err(Error::new(
            "Structured activation same-slot Market/Realm/manifest join differs",
        ));
    }
    let record_matches = |account: &crate::rpc::RpcAccount, expected: &[u8]| {
        account.owner == registry && account.data == expected
    };
    if realm_account.owner != registry
        || sha2::Sha256::digest(&realm_account.data).as_slice() != realm.content
        || !record_matches(&program_set_account, &program_set_body)
        || !record_matches(&config_account, &config_body)
        || !record_matches(
            &profile_account,
            &crate::runtime::decode_hex(&selected.records[51].body_hex)?,
        )
        || !record_matches(
            &effect_account,
            &crate::runtime::decode_hex(&selected.records[52].body_hex)?,
        )
        || !record_matches(
            &descriptor_account,
            &crate::runtime::decode_hex(&selected.records[53].body_hex)?,
        )
    {
        return Err(Error::new(
            "Structured activation finalized selected-record batch differs",
        ));
    }
    let context: [u8; 32] = sha2::Sha256::digest(b"dclutch/structured-root-activation/v1").into();
    let activation_request = structured_activation_request_v1();
    let plan = build_selected_capability_activation_plan_v1(SelectedCapabilityActivationInputV1 {
        market,
        market_account: &market_account,
        current_slot: facts_slot,
        core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
        resolution,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        registry,
        activation_cache: pubkey(&plan.activation)?,
        realm,
        manifest,
        manifest_body: &manifest_account.data,
        entry_index: selected.selected_manifest_entry_index,
        program_set,
        program_set_id: sha2::Sha256::digest(&program_set_body).into(),
        config,
        config_id: sha2::Sha256::digest(&config_body).into(),
        capability_kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        account_profile,
        effect,
        descriptor,
        selected_funding_ledger,
        selected_funding_ledger_account: &ledger_account,
        resolution_funding_ledger,
        family_activation_request: &activation_request,
        context,
    })?;
    let outcome = execute_selected_capability_activation_v1(
        rpc,
        payer,
        &plan,
        "activate Structured selector-255 root",
    )?;
    transactions.extend(outcome.routing_transactions);
    transactions.push(outcome.activation.clone());
    let tail = outcome
        .root_account
        .data
        .get(dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| Error::new("Structured activated root omitted tail"))?;
    if tail != STRUCTURED_CAPABILITY_ROOT_TAIL_V1 {
        return Err(Error::new(
            "Structured activated root tail differs from selector-255 artifact",
        ));
    }
    Ok(
        json!({"root": plan.root.to_string(), "slot": plan.facts_slot, "activation": outcome.activation}),
    )
}

/// Create the permissionless seal (when vacant) and execute the first real
/// Structured lifecycle action in one routed, atomic transaction.  Every
/// coordinate is recovered from finalized records or the root just accepted;
/// the only fresh writable PDA is the receipt Mint the Claims instruction
/// itself creates.
fn activate_structured_receipt_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    market_input: &[u8],
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
    market: Pubkey,
    root_activation: &serde_json::Value,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    use dclutch_claims::rational_kernel::{
        DescriptorAdmissionV2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    };
    use dclutch_registry::{ActivatedExecutionReleaseSetV1, release_set::ExecutionRoleV1};
    use dclutch_release_tool::CheckedExecutionReleaseSetV1;
    use solana_sdk_ids::sysvar;

    let registry = pubkey(&plan.registry.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let input: crate::model::MarketRunInput = serde_json::from_slice(market_input)?;
    let selected = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Structured receipt omitted selected capability"))?;
    let root = root_activation
        .get("root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("Structured root activation omitted its root identity"))
        .and_then(pubkey)?;
    let manifest = selected_pair_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &crate::runtime::decode_hex(&input.capability_manifest_hex)?,
    )?;
    let program_set = selected_pair_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        &crate::runtime::decode_hex(&selected.program_set_hex)?,
    )?;
    let receipt_descriptor = &artifacts.bundle_records[0];
    let descriptor = selected_pair_v1(
        registry,
        receipt_descriptor.schema,
        &receipt_descriptor.body,
    )?;
    let config = selected_pair_v1(registry, artifacts.config.schema, &artifacts.config.body)?;
    let pair = |index: usize| -> Result<SelectedActivationRecordPairV1> {
        let value = &artifacts.bundle_records[index];
        selected_pair_v1(registry, value.schema, &value.body)
    };
    let profile = pair(1)?;
    let request = pair(2)?;
    let lifecycle = pair(3)?;
    let strategy = pair(4)?;
    let transition = pair(5)?;
    let effect = pair(6)?;
    let report_pair = |label: &str, schema: [u8; 32]| -> Result<SelectedActivationRecordPairV1> {
        let row = evidence
            .accounts
            .get(label)
            .ok_or_else(|| Error::new(format!("Structured receipt report omitted {label}")))?;
        pair_from_digest_v1(registry, schema, crate::plan::hex32(&row.data_sha256)?)
    };
    let product = report_pair(
        "product_record",
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let domain = report_pair(
        "result_domain_record",
        dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = report_pair(
        "portfolio_record",
        dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
    )?;
    let basis = report_pair(
        "linked_liability_basis_record",
        dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    )?;

    // The selected descriptor is the content address and Claims authority
    // source.  Its authority is derived before observation, never accepted
    // from a report or CLI.
    let descriptor_id: [u8; 32] = sha2::Sha256::digest(&receipt_descriptor.body).into();
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &claims,
    )
    .0;
    let descriptor_value = RepresentationDescriptorV2::decode(
        &receipt_descriptor.body,
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured receipt descriptor: {error:?}")))?;

    let mut fixed = vec![Pubkey::default(); HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut place = |index: usize, key: Pubkey| -> Result<()> {
        if index >= fixed.len() || key == Pubkey::default() {
            return Err(Error::new(
                "Structured receipt fixed frame has an invalid coordinate",
            ));
        }
        fixed[index] = key;
        Ok(())
    };
    place(HOT_MARKET_ACCOUNT_V3, market)?;
    place(HOT_ROOT_ACCOUNT_V3, root)?;
    for (index, record) in [
        (HOT_MANIFEST_RAW_ACCOUNT_V3, manifest),
        (HOT_PROGRAM_SET_RAW_ACCOUNT_V3, program_set),
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, descriptor),
        (HOT_CONFIG_RAW_ACCOUNT_V3, config),
        (HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, profile),
        (HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, request),
        (HOT_TRANSITION_RAW_ACCOUNT_V3, transition),
        (HOT_EFFECT_RAW_ACCOUNT_V3, effect),
        (HOT_LIFECYCLE_RAW_ACCOUNT_V3, lifecycle),
        (HOT_STRATEGY_RAW_ACCOUNT_V3, strategy),
        (HOT_PRODUCT_RAW_ACCOUNT_V3, product),
        (HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, domain),
        (HOT_PORTFOLIO_RAW_ACCOUNT_V3, portfolio),
        (HOT_LINKED_BASIS_RAW_ACCOUNT_V3, basis),
    ] {
        place(index, record.raw)?;
        place(index + 1, record.staging)?;
    }
    place(HOT_ACTIVATION_CACHE_ACCOUNT_V3, pubkey(&plan.activation)?)?;
    place(HOT_CORE_PROGRAM_ACCOUNT_V3, core)?;
    place(
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        pubkey(&plan.core.programdata_id)?,
    )?;
    place(HOT_TRADING_PROGRAM_ACCOUNT_V3, trading)?;
    place(
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        pubkey(&plan.trading.programdata_id)?,
    )?;
    place(HOT_REGISTRY_PROGRAM_ACCOUNT_V3, registry)?;
    place(HOT_RENT_SYSVAR_ACCOUNT_V3, sysvar::rent::ID)?;
    place(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, sysvar::instructions::ID)?;

    let core_state = CoreState::decode(
        &rpc.account(market)?
            .ok_or_else(|| Error::new("Structured receipt Market vanished after root activation"))?
            .data,
    )
    .map_err(|error| Error::new(format!("Structured receipt Market after root: {error:?}")))?;
    let rent_credit = Pubkey::new_from_array(core_state.rent_beneficiary.to_bytes());
    let aggregate = pubkey(
        &evidence
            .accounts
            .get("claims_aggregate")
            .ok_or_else(|| Error::new("Structured receipt report omitted claims aggregate"))?
            .address,
    )?;
    let receipt_mint = Pubkey::new_from_array(descriptor_value.receipt_mint());
    let addresses = [
        market,
        aggregate,
        rent_credit,
        receipt_mint,
        sysvar::rent::ID,
        root,
        pubkey(&plan.activation)?,
    ];
    let (slot, accounts) = rpc.finalized_accounts(&addresses, artifacts.slot)?;
    let account = |index: usize, label: &str| -> Result<crate::rpc::RpcAccount> {
        accounts.get(index).cloned().flatten().ok_or_else(|| {
            Error::new(format!(
                "Structured receipt finalized snapshot omitted {label}"
            ))
        })
    };
    let market_account = account(0, "Market")?;
    let aggregate_account = account(1, "Claims aggregate")?;
    let rent_credit_account = account(2, "RentCredit")?;
    let rent_account = account(4, "Rent sysvar")?;
    let rent_observed = dclutch_operator::ObservedAccount {
        observation: dclutch_operator::Observation {
            slot,
            unix_timestamp: rpc.block_time(slot)?,
            finality: dclutch_operator::Finality::Finalized,
        },
        key: sysvar::rent::ID,
        owner: rent_account.owner,
        lamports: rent_account.lamports,
        executable: rent_account.executable,
        data: rent_account.data.clone(),
    };
    let rent = decode_rent(&rent_observed)
        .map_err(|error| Error::new(format!("Structured receipt finalized Rent: {error:?}")))?;
    let header = structured_activation::build_activate_receipt_header_v1(
        structured_activation::ActivateReceiptHeaderObservationV1 {
            market,
            market_account: &market_account,
            claims,
            aggregate,
            aggregate_account: &aggregate_account,
            rent_credit,
            rent_credit_account: &rent_credit_account,
            rent_program,
            receipt_mint_account: accounts.get(3).and_then(Option::as_ref),
            rent: &rent,
            descriptor: descriptor_value,
        },
    )?;
    let mut lifecycle_bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2];
    LifecycleRequestV2::new(header, &[])
        .map_err(|error| Error::new(format!("Structured receipt lifecycle header: {error:?}")))?
        .encode_into(&mut lifecycle_bytes)
        .map_err(|error| Error::new(format!("Structured receipt lifecycle encode: {error:?}")))?;
    let claims_child = activate_receipt_claims_instruction_v1(
        ActivateReceiptPhysicalInputsV1 {
            trading,
            trading_programdata: pubkey(&plan.trading.programdata_id)?,
            claims,
            claims_programdata: pubkey(&plan.claims.programdata_id)?,
            registry,
            activation_cache: pubkey(&plan.activation)?,
            descriptor_raw: descriptor.raw,
            descriptor_staging: descriptor.staging,
            representation_authority,
            receipt_mint,
            rent_credit,
            rent_program,
            claims_market: aggregate,
            core_market: market,
            core,
            core_programdata: pubkey(&plan.core.programdata_id)?,
        },
        &lifecycle_bytes,
    )?;
    let local = plan.checked_local_mutable_set.as_ref().ok_or_else(|| {
        Error::new("Structured receipt requires checked local execution evidence")
    })?;
    let checked_bytes = BASE64
        .decode(
            &local
                .execution_release_set
                .checked_execution_release_set_base64,
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured receipt checked release base64: {error}"
            ))
        })?;
    let checked = CheckedExecutionReleaseSetV1::decode(&checked_bytes)
        .map_err(|error| Error::new(format!("Structured receipt checked release: {error:?}")))?;
    let activation_account = account(6, "activation cache")?;
    let activated = ActivatedExecutionReleaseSetV1::decode(&activation_account.data)
        .map_err(|error| Error::new(format!("Structured receipt activation cache: {error:?}")))?;
    if activated.execution_release_set_id().as_bytes()
        != &core_state.identity.selected_release_set.to_bytes()
        || checked
            .execution_release_set_id()
            .map_err(|error| {
                Error::new(format!(
                    "Structured receipt checked release identity: {error:?}"
                ))
            })?
            .as_bytes()
            != activated.execution_release_set_id().as_bytes()
    {
        return Err(Error::new(
            "Structured receipt checked release and activation cache select different release sets",
        ));
    }
    let hot_outer = CheckedRationalLifecycleHotOuterV3 {
        trading_program: trading,
        artifact_release: sha2::Sha256::digest(
            &activated
                .role(ExecutionRoleV1::Trading)
                .release()
                .to_bytes(),
        )
        .into(),
        checked_manifest_digest: checked
            .checked_execution_release_set_id()
            .map_err(|error| {
                Error::new(format!(
                    "Structured receipt checked manifest identity: {error:?}"
                ))
            })?
            .to_bytes(),
    };
    let seal_key = dclutch_vm::capability_seal::CapabilitySealKeyV1::new(
        dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
        descriptor_id,
        STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1,
        activated
            .role(ExecutionRoleV1::Trading)
            .release()
            .semantic_release_id()
            .to_bytes(),
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured receipt seal key: {error:?}")))?;
    place(
        HOT_CAPABILITY_SEAL_ACCOUNT_V3,
        Pubkey::find_program_address(&seal_key.seeds().as_slices(), &trading).0,
    )?;
    let metas = fixed
        .iter()
        .enumerate()
        .map(|(index, key)| {
            if index == HOT_ROOT_ACCOUNT_V3 {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            }
        })
        .collect::<Vec<_>>();
    let frame = AuthenticatedRationalLifecycleHotFrameV1 {
        fixed_accounts: metas,
        strategy_accounts: Vec::new(),
        root_data: account(5, "capability root")?.data,
        market_data: market_account.data.clone(),
        release_set: core_state.identity.selected_release_set.to_bytes(),
        market,
        generation: core_state.identity.generation,
        finalized_slot: slot,
        hot_outer,
    };
    let hot = structured_activation::build_selected_activate_receipt_instruction_v1(
        artifacts,
        &frame.state()?,
        &claims_child,
        descriptor_value,
        core_state.identity.realm_id.to_bytes(),
    )?;
    let seal_before = rpc.account(fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3])?;
    let mut instructions = Vec::new();
    if seal_before.is_none() {
        instructions.push(
            capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                trading_program: trading,
                registry_program: registry,
                trading_semantic_release: activated
                    .role(ExecutionRoleV1::Trading)
                    .release()
                    .semantic_release_id()
                    .to_bytes(),
                descriptor_digest: descriptor_id,
                action: STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1,
                fixed_frame: &fixed,
                payer: payer.pubkey(),
            })
            .map_err(|error| Error::new(format!("Structured receipt seal builder: {error:?}")))?
            .instruction,
        );
    }
    instructions.push(hot.instruction);
    let mut routing_addresses = std::collections::BTreeSet::new();
    for instruction in &instructions {
        routing_addresses.insert(instruction.program_id);
        routing_addresses.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    }
    let routing_addresses = routing_addresses.into_iter().collect::<Vec<_>>();
    let (observation, tables) = crate::market::publish_routing_table_over_v1(
        rpc,
        payer,
        "STRUCTURED-RECEIPT",
        &routing_addresses,
        transactions,
    )?;
    let sent = rpc.send_v0_on_heap(
        "activate Structured receipt",
        &instructions,
        payer,
        observation,
        &tables,
        DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured receipt activation refused on chain: {error}"
        )));
    }
    transactions.push(sent.clone());
    let mint = rpc.account(receipt_mint)?.ok_or_else(|| {
        Error::new("Structured receipt transaction landed without a receipt Mint")
    })?;
    let root_after = rpc
        .account(root)?
        .ok_or_else(|| Error::new("Structured receipt transaction removed its root"))?;
    let root_tail = root_after
        .data
        .get(dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| Error::new("Structured receipt poststate omitted root state"))?;
    dclutch_market::capability_program::CapabilityRootHeaderV1::decode(
        root_after
            .data
            .get(..dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| Error::new("Structured receipt poststate omitted root header"))?,
    )
    .map_err(|error| {
        Error::new(format!(
            "Structured receipt poststate root header: {error:?}"
        ))
    })?;
    if mint.owner != Pubkey::new_from_array(descriptor_value.token_program())
        || mint.data.len() != dclutch_custody::token_svm::TOKEN_2022_CLOSEABLE_MINT_BYTES_V2
        || root_tail.is_empty()
    {
        return Err(Error::new(
            "Structured receipt poststate differs from canonical Mint or root state",
        ));
    }
    Ok(
        json!({"slot": sent.slot, "receiptMint": receipt_mint.to_string(), "receiptMintLamports": mint.lamports, "receiptMintBytes": mint.data.len(), "root": root.to_string(), "rootBytes": root_after.data.len(), "rootStateSha256": sha256_hex(root_tail), "seal": fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3].to_string(), "routingTables": tables.iter().map(|table| table.key.to_string()).collect::<Vec<_>>() }),
    )
}

fn activation_snapshot_slot_v1(observed: u64, minimum: u64) -> Result<u64> {
    if observed == 0 || observed < minimum {
        return Err(Error::new(
            "Structured activation finalized snapshot predates its required publication slot",
        ));
    }
    Ok(observed)
}

fn pair_from_digest_v1(
    registry: Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<SelectedActivationRecordPairV1> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Structured root schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("Structured root content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
    };
    let (raw, raw_bump) = derive(key.raw_record_pda_seeds());
    let (staging, staging_bump) = derive(key.staging_cursor_pda_seeds());
    Ok(SelectedActivationRecordPairV1 {
        raw,
        staging,
        schema,
        content,
        bumps: [raw_bump, staging_bump],
    })
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

    #[test]
    fn root_activation_refuses_a_snapshot_before_the_published_closure() {
        let error = super::activation_snapshot_slot_v1(41, 42)
            .expect_err("a stale finalized snapshot must not build selector-255 activation");
        assert_eq!(
            error.to_string(),
            "Structured activation finalized snapshot predates its required publication slot"
        );
    }
}
