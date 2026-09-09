//! Owned-loopback publication producer for the Structured receipt lifecycle.
//!
//! The command deliberately stops before a caller can submit a lifecycle
//! packet unless it has first rebuilt the per-Market representation closure
//! from finalized Product and selected-release records.  It is the mutation
//! boundary for the seven immutable Structured records; the Journey owns the
//! surrounding checked substrate and founding sequence.

use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims::{
    composition::{CompositionExposureBundleV3, RecordAdmissionV3},
    liability_basis_state_v2::{
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketViewV2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2,
    },
    rational::{RATIONAL_SHARD_MINT_SEED_V2, RATIONAL_STRUCTURED_CUSTODY_SEED_V2},
    rational_kernel::RepresentationDescriptorV2,
    rational_lifecycle::{
        LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LifecycleActionV2,
        LifecycleCoordinateV2, LifecycleHeaderV2, LifecycleRequestV2,
        hot_v6::{
            RationalLifecycleHotRequestV6, STRUCTURED_ACTIVATE_COORDINATE_SELECTOR_V1,
            STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1,
        },
    },
    structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
};
use dclutch_core_contract::ContentId;
use dclutch_market::{
    CoreState, Phase,
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
    capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1},
    realm::REALM_SCHEMA_RELEASE_ID_V1,
};
use dclutch_operator::structured_activation_bundle_v1::{
    STRUCTURED_CAPABILITY_ROOT_TAIL_V2, structured_activation_request_v1,
};
use dclutch_operator::structured_selected_release_v1::{
    STRUCTURED_SELECTED_PUBLICATION_BYTES_V1, STRUCTURED_SELECTED_PUBLICATION_MAGIC_V1,
    STRUCTURED_SELECTED_PUBLICATION_VERSION_V1, StructuredSelectedReleaseInputV1,
    structured_selected_release_v1,
};
use dclutch_operator::{
    capability_seal_v1::{CapabilitySealInstructionInputV1, capability_seal_instruction_v1},
    observation::decode_rent,
    rational_lifecycle_hot::CheckedRationalLifecycleHotOuterV3,
    representation_composition::native_categorical_v1::{
        NativeBasisCompositionInputV1, compile_native_basis_composition_v1,
    },
};
use dclutch_product::admission::ProductRecordV2;
use dclutch_product::{PortfolioV2, ResultDomainV2};
use dclutch_registry::{
    record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId},
    release_set::CapabilityExecutionSelectionV1,
};
use dclutch_versioned_message_operator::{Observation, ObservedAccount};
use dclutch_vm::account_profile::{
    AccountObservationV1,
    v2::{
        AccountPrestateV2, AccountProfileV2, ProjectionRegistersV2,
        project_dynamic_fixed_spans_atomic,
    },
};
use serde_json::json;
use sha2::Digest as _;
use solana_sdk::{
    hash::hash,
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
        record_coordinates_v1,
    },
    structured_composition_admission::hydrate_structured_composition_admission_v1,
    structured_physical_frame::{
        ActivateReceiptPhysicalInputsV1, CoordinatePhysicalInputsV1,
        activate_receipt_claims_instruction_v1, coordinate_claims_instruction_v1,
    },
};

/// Public owned-loopback command for the Structured publication predecessor.
pub(crate) const COMMAND_V1: &str = "local-private-validator-structured-claims-v1";
pub(crate) const PROFILE_REPLAY_COMMAND_V1: &str = "structured-profile-capture-replay-v1";

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

/// Replay both the captured immutable receipt profile and the current source
/// candidate against one saved finalized logical frame. This command is
/// deliberately key-free and read-only.
pub(crate) fn run_profile_capture_replay_v1(arguments: Vec<String>) -> Result<()> {
    let mut capture = None;
    let mut market_input = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let destination = match argument.as_str() {
            "--capture" => &mut capture,
            "--market-input" => &mut market_input,
            "--output" => &mut output,
            _ => return Err(Error::new(format!("unknown argument: {argument}"))),
        };
        if destination.replace(PathBuf::from(value)).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let capture = capture.ok_or_else(|| Error::new("--capture is required"))?;
    let market_input = market_input.ok_or_else(|| Error::new("--market-input is required"))?;
    let output = output.ok_or_else(|| Error::new("--output is required"))?;
    if output.exists() {
        return Err(Error::new(format!(
            "Structured profile replay refuses to overwrite {}",
            output.display()
        )));
    }
    let capture_bytes = std::fs::read(&capture)?;
    let captured: serde_json::Value = serde_json::from_slice(&capture_bytes)?;
    let instructions = captured
        .get("instructions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Error::new("Structured profile capture omitted instructions"))?;
    let hot = instructions
        .last()
        .ok_or_else(|| Error::new("Structured profile capture omitted Hot instruction"))?;
    let profile_address = hot
        .get("accounts")
        .and_then(serde_json::Value::as_array)
        .and_then(|accounts| accounts.get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("Structured profile capture omitted profile address"))?;
    let original_profile = captured
        .get("state")
        .and_then(serde_json::Value::as_object)
        .and_then(|state| state.get(profile_address))
        .and_then(|account| account.get("dataBase64"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("Structured profile capture omitted original profile bytes"))?;
    let original_profile = BASE64.decode(original_profile).map_err(|error| {
        Error::new(format!(
            "Structured profile capture original profile base64: {error}"
        ))
    })?;
    let market_input_bytes = std::fs::read(&market_input)?;
    let candidate_profile = current_source_receipt_profile_v1(&market_input_bytes)?;
    let original = project_saved_structured_receipt_snapshot_v1(&captured, &original_profile)?;
    let candidate = project_saved_structured_receipt_snapshot_v1(&captured, &candidate_profile)?;
    let evidence = json!({
        "schema": "dclutch-structured-profile-capture-replay-v1",
        "capture": capture.display().to_string(),
        "captureSha256": sha256_hex(&capture_bytes),
        "marketInput": market_input.display().to_string(),
        "marketInputSha256": sha256_hex(&market_input_bytes),
        "original": original,
        "candidate": candidate,
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string(&evidence)?);
    Ok(())
}

fn current_source_receipt_profile_v1(market_input_bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(current_source_selected_release_v1(market_input_bytes)?
        .activation
        .activate_receipt
        .account_profile)
}

fn current_source_selected_release_v1(
    market_input_bytes: &[u8],
) -> Result<dclutch_operator::structured_selected_release_v1::StructuredSelectedReleaseV1> {
    let input: crate::model::MarketRunInput = serde_json::from_value(
        crate::rpc::parse_json_without_duplicate_keys_v1(market_input_bytes)
            .map_err(|error| Error::new(format!("Structured market input {error}")))?,
    )
    .map_err(|error| Error::new(format!("Structured market input shape: {error}")))?;
    crate::market::validate_market_input(&input)?;
    let selected = input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Structured market input omitted selected capability"))?;
    if selected.family != "structured" {
        return Err(Error::new(
            "Structured profile replay input selected another family",
        ));
    }
    let config_bytes = crate::runtime::decode_hex(&selected.config_hex)?;
    let config = dclutch_custody::token_svm::TokenBehaviorSelectionV2::decode(&config_bytes)
        .map_err(|error| Error::new(format!("Structured profile replay config: {error:?}")))?;
    let descriptor_bytes = crate::runtime::decode_hex(&selected.selected_descriptor_hex)?;
    let descriptor =
        dclutch_market::capability_program::v4::CapabilityProgramV4::decode(&descriptor_bytes)
            .map_err(|error| {
                Error::new(format!("Structured profile replay descriptor: {error:?}"))
            })?;
    let publication = crate::runtime::decode_hex(&selected.publication_hex)?;
    if publication.len() != STRUCTURED_SELECTED_PUBLICATION_BYTES_V1
        || publication.get(..8) != Some(STRUCTURED_SELECTED_PUBLICATION_MAGIC_V1.as_slice())
        || publication.get(8..10)
            != Some(
                STRUCTURED_SELECTED_PUBLICATION_VERSION_V1
                    .to_le_bytes()
                    .as_slice(),
            )
    {
        return Err(Error::new(
            "Structured profile replay publication header or width differs",
        ));
    }
    // The final 20-byte scalar block is public canonical wire: root width,
    // representation K, Product N, selector offset, action count and roles.
    let scalar = publication
        .len()
        .checked_sub(20)
        .ok_or_else(|| Error::new("Structured profile replay publication is truncated"))?;
    let outcome_count = u32::from_le_bytes(
        publication
            .get(scalar + 4..scalar + 8)
            .ok_or_else(|| Error::new("Structured profile replay outcome count is truncated"))?
            .try_into()
            .map_err(|_| Error::new("Structured profile replay outcome count width differs"))?,
    );
    let product_basis = crate::runtime::decode_hex(&input.linked_basis_hex)?;
    let release = structured_selected_release_v1(StructuredSelectedReleaseInputV1 {
        realm: config.realm(),
        release_set: config.release_set(),
        root_schema: descriptor.root_schema().to_bytes(),
        root_state_bytes: descriptor.root_state_bytes(),
        representation_outcome_count: outcome_count,
        // This is the selected demo producer's measured per-coordinate row
        // width. It does not enter either lifecycle AccountProfile, but the
        // complete compiler validates the representation closure alongside it.
        item_state_bytes: 64,
        product_basis: &product_basis,
    })
    .map_err(|error| Error::new(format!("Structured current-source release: {error:?}")))?;
    Ok(release)
}

/// Run the authenticated Structured publication predecessor on an owned
/// validator.  The output is a durable input to the receipt-activation step,
/// never a claim that a receipt mint was created.
pub(crate) struct StructuredTerminalAccountsV1 {
    pub(crate) terminal_certificate: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) hoard: Pubkey,
}

/// Host orchestration supplied by the owned Journey. Resolution and wallet
/// semantics remain in their shipped exteriors; this callback returns only
/// coordinates which the Rational terminal builder authenticates again.
pub(crate) trait StructuredTerminalDriverV1 {
    fn resolve_and_settle_native(
        &mut self,
        rpc: &mut Rpc,
        plan: &SuccessorPlan,
        evidence: &crate::campaign::CampaignTerminalEvidenceV1,
        payer: &Keypair,
        market: Pubkey,
        transactions: &mut Vec<crate::model::TransactionEvidence>,
    ) -> Result<StructuredTerminalAccountsV1>;

    /// Finish the authenticated provider lifecycle while Core is still Terminal.
    /// Its reclaimed accounts cannot be recovered after moving Core to Retiring.
    fn finish_source_before_retirement(
        &mut self,
        rpc: &mut Rpc,
        payer: &Keypair,
        market: Pubkey,
        transactions: &mut Vec<crate::model::TransactionEvidence>,
    ) -> Result<serde_json::Value>;
}

pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run_owned_loopback_with_terminal_v1(arguments, None)
}

pub(crate) fn run_owned_loopback_with_terminal_v1(
    arguments: Vec<String>,
    terminal_driver: Option<&mut dyn StructuredTerminalDriverV1>,
) -> Result<()> {
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
    let (activation_slot, activation_market) =
        match structured_activation::hydrate_activate_receipt_v1(
            &mut rpc,
            &market_input,
            &campaign_report,
            0,
        ) {
            Ok(observed) => (observed.slot, observed.market),
            Err(error)
                if error.to_string()
                    == "Structured activation finalized Market bytes differ from the sealed founding report" =>
            {
                // A selector-255 root changes Core's mutable Market bytes.  This
                // narrow continuation authenticates the canonical live aggregate and
                // later requires the exact derived active root before any receipt
                // instruction can be constructed.
                resume_after_structured_root_observation_v1(&mut rpc, &evidence, &plan, claims)?
            }
            Err(error) => return Err(error),
        };
    let artifacts = structured_activation::hydrate_selected_activate_receipt_artifacts_v1(
        &mut rpc,
        registry,
        &market_input,
        activation_slot,
    )?;
    let coordinate_artifacts =
        structured_activation::hydrate_selected_activate_coordinate_artifacts_v1(
            &mut rpc,
            registry,
            &market_input,
            activation_slot,
        )?;
    let provisional = hydrate_structured_publication_input_same_slot_v1(
        &mut rpc,
        registry,
        pubkey(&plan.core.program_id)?,
        &market_input,
        &evidence,
        activation_slot,
        activation_market,
        claims,
        Vec::new(),
        2,
        1,
        Vec::new(),
        Vec::new(),
    )?;
    // The Structured recipe is the finalized Portfolio with its zero-weight
    // Product coordinates removed. Product N remains the native composition
    // width; only the Structured child K is sparse and bounded by the selected
    // RequestProfile. No request flag may choose either coordinate or weight.
    let native = compile_native_basis_composition_v1(NativeBasisCompositionInputV1 {
        market: activation_market.to_bytes(),
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
    let portfolio = PortfolioV2::decode(&provisional.portfolio_body)
        .map_err(|error| Error::new(format!("Structured campaign Portfolio: {error:?}")))?;
    if portfolio.coefficient_count() != native.width() {
        return Err(Error::new(
            "Structured campaign Portfolio width differs from native Product width",
        ));
    }
    let portfolio_denominator = portfolio.denominator();
    let denominator = if portfolio_denominator >= 2 {
        portfolio_denominator
    } else {
        portfolio_denominator
            .checked_mul(2)
            .ok_or_else(|| Error::new("Structured campaign shard denominator overflows"))?
    };
    // The graph keeps the Portfolio's exact payoff in lowest terms. Receipt
    // atoms use the selected fractional unit; scale both sides together so
    // raising that unit never changes the economic recipe.
    let composition_denominator = portfolio_denominator;
    let scale = denominator / portfolio_denominator;
    let mut coordinates = Vec::new();
    let mut composition_coefficients = Vec::new();
    let mut coefficients = Vec::new();
    for (coordinate, coefficient) in portfolio.coefficients().enumerate() {
        if coefficient == 0 {
            continue;
        }
        coordinates.push(
            u32::try_from(coordinate)
                .map_err(|_| Error::new("Structured campaign Product coordinate overflows"))?,
        );
        composition_coefficients.push(coefficient);
        coefficients.push(
            coefficient
                .checked_mul(scale)
                .ok_or_else(|| Error::new("Structured campaign coefficient overflows"))?,
        );
    }
    if coordinates.is_empty() || coordinates.len() > 3 {
        return Err(Error::new(
            "Structured campaign Portfolio nonzero coordinate count exceeds selected K=3",
        ));
    }
    // Redo the same-slot join with the explicit canonical recipe after the
    // width is authenticated.  This is intentionally not a mutation of the
    // provisional value above: it proves recipe input cannot sneak through a
    // stale serial read.
    let input = hydrate_structured_publication_input_same_slot_v1(
        &mut rpc,
        registry,
        pubkey(&plan.core.program_id)?,
        &market_input,
        &evidence,
        activation_slot,
        activation_market,
        claims,
        coordinates.clone(),
        denominator,
        composition_denominator,
        composition_coefficients,
        coefficients.clone(),
    )?;
    let closure = compile_structured_publication_closure_v1(&input)?;
    // Authenticate the already-founded Product graph before publishing any
    // continuation records.  The report is allowed to name coordinates, but
    // it is not allowed to smuggle a stale graph past the same width join
    // Trading will perform in its Hot prelude.  In particular, a Product
    // domain with three outcomes and a four-coefficient Portfolio otherwise
    // becomes the coarse Trading `Content` refusal after another publication
    // batch has already been committed.
    preflight_structured_product_graph_v1(&mut rpc, registry, &evidence)?;
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
        let payer = payer
            .as_ref()
            .ok_or_else(|| Error::new("Structured campaign omitted payer"))?;
        if std::env::var_os("DCLUTCH_STRUCTURED_REUSE_CLOSURE").is_some() {
            Some(reuse_structured_publication_closure_v1(
                &mut rpc,
                registry,
                &closure,
                activation_slot,
            )?)
        } else {
            Some(publish_structured_publication_closure_v1(
                &mut rpc,
                registry,
                payer,
                &closure,
                input_release_slot(&artifacts, activation_slot),
                &mut transactions,
            )?)
        }
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
        write_structured_progress_v1(&arguments, "publication-finalized", &transactions)?;
        let payer = payer
            .as_ref()
            .ok_or_else(|| Error::new("Structured campaign omitted payer"))?;
        Some(activate_structured_root_v1(
            &mut rpc,
            payer,
            &plan,
            &market_input,
            &evidence,
            activation_market,
            published.slot,
            &mut transactions,
        )?)
    } else {
        None
    };
    if root_activation.is_some() {
        write_structured_progress_v1(&arguments, "root-activated", &transactions)?;
    }
    let receipt_activation = if let (Some(published), Some(root_activation), Some(payer)) =
        (published.as_ref(), root_activation.as_ref(), payer.as_ref())
    {
        Some(activate_structured_receipt_v1(
            &arguments,
            &mut rpc,
            payer,
            &plan,
            &market_input,
            &evidence,
            &artifacts,
            &coordinate_artifacts,
            &closure,
            published,
            activation_market,
            root_activation,
            &mut transactions,
            terminal_driver,
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
            "market": activation_market.to_string(),
            "claims": claims.to_string(),
            "registry": registry.to_string(),
            "payer": payer.as_ref().map(|value| value.pubkey().to_string()),
            "finalizedSlot": published.as_ref().map(|value| value.slot).unwrap_or(activation_slot),
            "productCoordinates": coordinates,
            "denominator": denominator,
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

/// Reacquire the only mutable founding fact after selector-255 activation.
/// The caller reaches this path only after the strict sealed-Market digest
/// refusal. Claims balances and revision may have advanced through accepted
/// actions; canonical PDA and immutable Core joins authenticate that aggregate.
/// Its persisted Custody namespace is authoritative; the report's projected
/// founding namespace is a different context and cannot authenticate this field.
/// The root verifier below then proves the exact expected active root.
fn resume_after_structured_root_observation_v1(
    rpc: &mut Rpc,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    plan: &SuccessorPlan,
    claims: Pubkey,
) -> Result<(u64, Pubkey)> {
    let market = pubkey(
        &evidence
            .accounts
            .get("founding_market")
            .ok_or_else(|| Error::new("Structured resume report omitted founding Market"))?
            .address,
    )?;
    let aggregate_row = evidence
        .accounts
        .get("claims_aggregate")
        .ok_or_else(|| Error::new("Structured resume report omitted Claims aggregate"))?;
    let aggregate = pubkey(&aggregate_row.address)?;
    let (slot, accounts) = rpc.finalized_accounts(&[market, aggregate], 0)?;
    let market_account = accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("Structured resume Market is absent"))?;
    let aggregate_account = accounts
        .get(1)
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("Structured resume Claims aggregate is absent"))?;
    let state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Structured resume Core Market: {error:?}")))?;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured resumed Claims aggregate: {error:?}")))?;
    let aggregate_seeds =
        dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketSeedsV2::new(
            market.to_bytes(),
        )
        .map_err(|error| Error::new(format!("Structured resumed aggregate seeds: {error:?}")))?;
    if market_account.owner != pubkey(&plan.core.program_id)?
        || state.phase != Phase::Open
        || state.identity.market_id.to_bytes() != market.to_bytes()
        || aggregate_account.owner != claims
        || aggregate != Pubkey::find_program_address(&aggregate_seeds.as_slices(), &claims).0
        || aggregate_view.logical_market != market.to_bytes()
        || aggregate_view.release_set != state.identity.selected_release_set.to_bytes()
        || aggregate_view.registry_program != state.identity.registry_program.to_bytes()
        || aggregate_view.realm_id != state.identity.realm_id.to_bytes()
        || aggregate_view.generation != state.identity.generation
    {
        return Err(Error::new(
            "Structured resume mutable Market or canonical Claims aggregate differs",
        ));
    }
    Ok((slot, market))
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
    if selected.family != "structured" {
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
    let release = current_source_selected_release_v1(market_input)?;
    if release.program_set != program_set_body || release.config != config_body {
        return Err(Error::new(
            "Structured root current-source publication differs",
        ));
    }
    let records = release
        .publication_records()
        .map_err(|error| Error::new(format!("Structured root publication records: {error:?}")))?;
    let record = |label: &str| -> Result<SelectedActivationRecordPairV1> {
        let row = records
            .iter()
            .find(|row| row.label == label)
            .ok_or_else(|| Error::new(format!("Structured root omitted {label}")))?;
        selected_pair_v1(registry, row.schema, row.body)
    };
    let account_profile = record("root-activation-account-profile")?;
    let effect = record("root-activation-effect")?;
    let descriptor = record("root-activation-descriptor")?;
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
        || !record_matches(&profile_account, &release.root_activation.account_profile)
        || !record_matches(&effect_account, &release.root_activation.effect)
        || !record_matches(&descriptor_account, &release.root_activation.descriptor)
    {
        return Err(Error::new(
            "Structured activation finalized selected-record batch differs",
        ));
    }
    let context: [u8; 32] = sha2::Sha256::digest(b"dclutch/structured-root-activation/v1").into();
    let expected_header = CapabilityRootHeaderV1::new(
        ContentId::new(state.identity.selected_release_set.to_bytes())
            .map_err(|_| Error::new("Structured root activation release-set identity"))?,
        market.to_bytes(),
        state.identity.generation,
        CapabilityExecutionSelectionV1::new(
            selected.selected_manifest_entry_index,
            ContentId::new(manifest.content)
                .map_err(|_| Error::new("Structured root activation manifest identity"))?,
            ContentId::new(STRUCTURED_CAPABILITY_KIND_ID_V2)
                .map_err(|_| Error::new("Structured root activation kind identity"))?,
            ContentId::new(sha2::Sha256::digest(&program_set_body).into())
                .map_err(|_| Error::new("Structured root activation ProgramSet identity"))?,
            ContentId::new(sha2::Sha256::digest(&config_body).into())
                .map_err(|_| Error::new("Structured root activation config identity"))?,
        )
        .map_err(|error| Error::new(format!("Structured root activation selection: {error:?}")))?
        .with_capability_release_record_bumps(program_set.bumps[0], program_set.bumps[1]),
        SelectedRecordBumpsV1::new(
            manifest.bumps[0],
            manifest.bumps[1],
            config.bumps[0],
            config.bumps[1],
        ),
    )
    .map_err(|error| Error::new(format!("Structured root activation header: {error:?}")))?;
    let expected_root =
        Pubkey::find_program_address(&expected_header.seeds().as_slices(), &trading).0;
    if let Some(existing_root) = rpc.account(expected_root)? {
        let header = CapabilityRootHeaderV1::decode(
            existing_root
                .data
                .get(..dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1)
                .ok_or_else(|| Error::new("Structured resumed root is truncated"))?,
        )
        .map_err(|error| Error::new(format!("Structured resumed root header: {error:?}")))?;
        let tail = existing_root
            .data
            .get(dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or_else(|| Error::new("Structured resumed root omitted tail"))?;
        let structured_root =
            dclutch_trading::structured_root_v2::StructuredCapabilityRootV2::decode(tail)
                .map_err(|error| Error::new(format!("Structured resumed root state: {error:?}")))?;
        if existing_root.owner != trading || header != expected_header {
            return Err(Error::new(
                "Structured resumed root differs from canonical selector-255 activation",
            ));
        }
        return Ok(json!({
            "root": expected_root.to_string(),
            "slot": facts_slot,
            "activation": serde_json::Value::Null,
            "resumed": true,
            "outstandingResourceGroups": structured_root.outstanding(),
        }));
    }
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
    if tail != STRUCTURED_CAPABILITY_ROOT_TAIL_V2 {
        return Err(Error::new(
            "Structured activated root tail differs from selector-255 artifact",
        ));
    }
    Ok(
        json!({"root": plan.root.to_string(), "slot": plan.facts_slot, "activation": outcome.activation}),
    )
}

/// Authenticate that closure target 6 is precisely the derived representation
/// descriptor before a Claims frame can use it.  The target position is only a
/// closure-order assertion; schema, digest, and both Registry PDAs independently
/// bind it to the semantic descriptor bytes.
fn authenticate_receipt_descriptor_publication_v1(
    registry: Pubkey,
    descriptor_bytes: &[u8],
    published: &crate::runtime::PublishedRecord,
) -> Result<SelectedActivationRecordPairV1> {
    let schema = dclutch_claims::rational_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3;
    let expected = selected_pair_v1(registry, schema, descriptor_bytes)?;
    if published.schema != schema || published.digest != expected.content {
        return Err(Error::new(
            "Structured receipt publication descriptor schema or digest differs",
        ));
    }
    if published.raw != expected.raw || published.staging != expected.staging {
        return Err(Error::new(
            "Structured receipt publication descriptor PDA differs",
        ));
    }
    Ok(expected)
}

/// Reacquire an already-published Structured closure for a diagnostic
/// continuation.  The bytes still come from the compiler's authenticated
/// closure, while this snapshot proves every raw record and vacant cursor is
/// the exact finalized target before a Hot frame is assembled.
fn reuse_structured_publication_closure_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    closure: &crate::structured_claims_producer::StructuredPublicationClosureV1,
    minimum_slot: u64,
) -> Result<crate::structured_claims_producer::PublishedStructuredClosureV1> {
    let targets = closure.publication_targets();
    let addresses = targets
        .iter()
        .flat_map(|target| {
            record_coordinates_v1(
                registry,
                target.schema_id,
                sha2::Sha256::digest(target.bytes).into(),
            )
            .ok()
            .into_iter()
            .flat_map(|(raw, staging)| [raw, staging])
        })
        .collect::<Vec<_>>();
    if addresses.len() != targets.len().saturating_mul(2) {
        return Err(Error::new(
            "Structured diagnostic closure coordinate count differed from seven targets",
        ));
    }
    let (slot_observation, observed) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, minimum_slot)?;
    let mut records = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        let raw = observed
            .get(index.saturating_mul(2))
            .ok_or_else(|| Error::new("Structured diagnostic closure raw record is absent"))?;
        let staging = observed
            .get(index.saturating_mul(2).saturating_add(1))
            .ok_or_else(|| Error::new("Structured diagnostic closure staging cursor is absent"))?;
        let digest: [u8; 32] = sha2::Sha256::digest(target.bytes).into();
        let (expected_raw, expected_staging) =
            record_coordinates_v1(registry, target.schema_id, digest)?;
        if raw.key != expected_raw
            || staging.key != expected_staging
            || raw.owner != registry
            || raw.data != target.bytes
            || staging.owner != solana_sdk_ids::system_program::ID
            || !staging.data.is_empty()
        {
            return Err(Error::new(format!(
                "Structured diagnostic closure target {index} differs from finalized Registry state"
            )));
        }
        dclutch_operator::observation::authenticate_finalized_record(
            registry,
            raw,
            &dclutch_operator::observation::FinalizedRecordProof {
                schema_release_id: target.schema_id,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured diagnostic closure target {index} refused: {error:?}"
            ))
        })?;
        records.push(crate::runtime::PublishedRecord {
            schema: target.schema_id,
            digest,
            raw: expected_raw,
            staging: expected_staging,
        });
    }
    let records: [crate::runtime::PublishedRecord; 7] = records.try_into().map_err(|_| {
        Error::new("Structured diagnostic closure target count differed from seven")
    })?;
    Ok(
        crate::structured_claims_producer::PublishedStructuredClosureV1 {
            slot: slot_observation.slot,
            records,
        },
    )
}

/// Save the exact routed Hot packet and its finalized account image before a
/// diagnostic run submits it.  The replay tool can replace only Trading's
/// ELF tail, preserving this frame's release-pinned Loader header.
fn write_structured_frame_capture_v1(
    rpc: &mut Rpc,
    path: &Path,
    instructions: &[solana_sdk::instruction::Instruction],
    fee_payer: Pubkey,
    observation: Observation,
    tables: &[ObservedAccount],
    profile_projection: serde_json::Value,
) -> Result<()> {
    let bounded =
        crate::rpc::bounded_instructions(instructions, Some(DIRECT_HOT_HEAP_FRAME_BYTES_V1))?;
    let (blockhash, _) = rpc.recent_blockhash_with_height_v1()?;
    let plan = dclutch_versioned_message_operator::compile_v0_message_with_optional_tables(
        fee_payer,
        &bounded,
        solana_hash::Hash::new_from_array(blockhash.to_bytes()),
        observation,
        tables,
    )
    .map_err(|error| Error::new(format!("Structured receipt capture message: {error:?}")))?;
    let transaction = solana_sdk::transaction::VersionedTransaction {
        signatures: vec![
            solana_sdk::signature::Signature::default();
            usize::from(plan.required_signatures.max(1))
        ],
        message: plan.message,
    };
    let packet = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("Structured receipt capture packet: {error}")))?;
    let mut addresses = std::collections::BTreeSet::new();
    addresses.insert(fee_payer);
    for instruction in &bounded {
        addresses.insert(instruction.program_id);
        addresses.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    }
    for table in tables {
        addresses.insert(table.key);
    }
    let mut state = serde_json::Map::new();
    for address in addresses {
        if let Some(account) = rpc.account(address)? {
            state.insert(
                address.to_string(),
                json!({
                    "lamports": account.lamports,
                    "owner": account.owner.to_string(),
                    "executable": account.executable,
                    "rentEpoch": account.rent_epoch,
                    "dataBase64": BASE64.encode(&account.data),
                }),
            );
        }
    }
    let frames = bounded
        .iter()
        .map(|instruction| {
            json!({
                "programId": instruction.program_id.to_string(),
                "accounts": instruction.accounts.iter().map(|meta| meta.pubkey.to_string()).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "schema": "dclutch-devnet-frame-capture-v1",
            "label": "structured-receipt-diagnostic",
            "warpSlot": rpc.finalized_slot()?,
            "transactionBase64": BASE64.encode(&packet),
            "instructions": frames,
            "state": state,
            "profileProjection": profile_projection,
        }))?,
    )?;
    eprintln!(
        "structured receipt diagnostic capture written to {}",
        path.display()
    );
    Ok(())
}

/// Replay the exact receipt AccountProfile against one finalized observation
/// of the logical frame before capture. This is a diagnostic control: the
/// native interpreter result and every decoded rule travel beside the exact
/// wire packet instead of being inferred from its coarse Trading refusal.
fn structured_receipt_profile_projection_v1(
    rpc: &mut Rpc,
    frame: &AuthenticatedRationalLifecycleHotFrameV1,
    claims_child: &solana_sdk::instruction::Instruction,
    hot_instruction: &solana_sdk::instruction::Instruction,
    artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
) -> Result<serde_json::Value> {
    const INJECTED: usize = 5;
    const TAIL_COUNT: u32 = 0;
    let profile = AccountProfileV2::decode(&artifacts.bundle.account_profile)
        .map_err(|error| Error::new(format!("Structured receipt diagnostic profile: {error:?}")))?;
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(|error| {
            Error::new(format!("Structured receipt diagnostic geometry: {error:?}"))
        })?;
    if logical_count != INJECTED + claims_child.accounts.len() {
        return Err(Error::new(
            "Structured receipt diagnostic logical width differs from its Claims frame",
        ));
    }
    let injected_fixed = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ];
    let mut logical_metas = Vec::with_capacity(logical_count);
    let mut representatives = Vec::with_capacity(logical_count);
    let mut ordinals = Vec::with_capacity(logical_count);
    for coordinate in 0..logical_count {
        let representative = profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(|error| {
                Error::new(format!(
                    "Structured receipt diagnostic representative {coordinate}: {error:?}"
                ))
            })?;
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(|error| {
                Error::new(format!(
                    "Structured receipt diagnostic ordinal {coordinate}: {error:?}"
                ))
            })?;
        let meta = if ordinal < INJECTED {
            let fixed_coordinate = *injected_fixed.get(ordinal).ok_or_else(|| {
                Error::new("Structured receipt diagnostic injected ordinal overflow")
            })?;
            hot_instruction
                .accounts
                .get(fixed_coordinate)
                .cloned()
                .ok_or_else(|| Error::new("Structured receipt diagnostic fixed frame overflow"))?
        } else {
            let outer_coordinate = frame
                .fixed_accounts
                .len()
                .checked_add(frame.strategy_accounts.len())
                .and_then(|base| base.checked_add(ordinal - INJECTED))
                .ok_or_else(|| Error::new("Structured receipt diagnostic outer frame overflow"))?;
            hot_instruction
                .accounts
                .get(outer_coordinate)
                .cloned()
                .ok_or_else(|| Error::new("Structured receipt diagnostic packed frame overflow"))?
        };
        logical_metas.push(meta);
        representatives.push(representative);
        ordinals.push(ordinal);
    }
    let addresses = logical_metas
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    let (slot, observed) = rpc.finalized_accounts(&addresses, frame.finalized_slot)?;
    let mut keys = Vec::with_capacity(logical_count);
    let mut owners = Vec::with_capacity(logical_count);
    let mut data = Vec::with_capacity(logical_count);
    let mut lamports = Vec::with_capacity(logical_count);
    let mut executable = Vec::with_capacity(logical_count);
    for (coordinate, account) in observed.into_iter().enumerate() {
        let meta = logical_metas
            .get(coordinate)
            .ok_or_else(|| Error::new("Structured receipt diagnostic meta overflow"))?;
        keys.push(meta.pubkey.to_bytes());
        match account {
            Some(account) => {
                owners.push(account.owner.to_bytes());
                data.push(account.data);
                lamports.push(account.lamports);
                executable.push(account.executable);
            }
            None => {
                owners.push(solana_sdk_ids::system_program::ID.to_bytes());
                data.push(Vec::new());
                lamports.push(0);
                executable.push(false);
            }
        }
    }
    // These are the same four independently authenticated content identities
    // `logical_projection_keys_boxed_v3` substitutes on chain.
    for coordinate in 1..=4 {
        keys[coordinate] = hash(&data[coordinate]).to_bytes();
    }
    let product_digest = hash(&data[2]).to_bytes();
    let mut observations = Vec::with_capacity(logical_count);
    for coordinate in 0..logical_count {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| {
                    Error::new("Structured receipt diagnostic rule coordinate overflow")
                })?,
            )
            .map_err(|error| {
                Error::new(format!(
                    "Structured receipt diagnostic rule {coordinate}: {error:?}"
                ))
            })?;
        // Mirror the runtime adapter's authenticated shared-prefix boundary.
        // A profile declaration alone does not authenticate a child-owned body.
        let observation = if matches!(coordinate, 1 | 4)
            && matches!(
                rule.prestate(),
                AccountPrestateV2::AdapterAuthenticatedVariableData
            ) {
            AccountObservationV1::new_adapter_authenticated_variable_data(
                &keys[coordinate],
                &owners[coordinate],
                lamports[coordinate],
                &data[coordinate],
                logical_metas[coordinate].is_signer,
                logical_metas[coordinate].is_writable,
                executable[coordinate],
            )
        } else {
            let observation = AccountObservationV1::new(
                &keys[coordinate],
                &owners[coordinate],
                lamports[coordinate],
                &data[coordinate],
                logical_metas[coordinate].is_signer,
                logical_metas[coordinate].is_writable,
                executable[coordinate],
            );
            if coordinate == 2 {
                observation.with_adapter_data_digest(&product_digest)
            } else {
                observation
            }
        };
        observations.push(observation);
    }
    let scalar_count = usize::from(profile.common_scalar_count());
    let identity_count = usize::from(profile.common_identity_count());
    let mut input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    if let Some(destination) = profile.trusted_current_slot_scalar() {
        input_scalars[usize::from(destination)] = slot;
    }
    if let Some(destination) = profile.trusted_current_executing_program_identity() {
        input_identities[usize::from(destination)] = hot_instruction.program_id.to_bytes();
    }
    if let Some(destination) = profile.trusted_system_program_identity() {
        input_identities[usize::from(destination)] = solana_sdk_ids::system_program::ID.to_bytes();
    }
    let mut scratch_scalars = vec![0_u64; scalar_count];
    let mut scratch_identities = vec![[0_u8; 32]; identity_count];
    let mut output_scalars = vec![0_u64; scalar_count];
    let mut output_identities = vec![[0_u8; 32]; identity_count];
    let projection = project_dynamic_fixed_spans_atomic(
        profile,
        TAIL_COUNT,
        &[],
        &observations,
        ProjectionRegistersV2 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
        None,
    );
    let mut expected_physical_flags = vec![0_u8; logical_count];
    for coordinate in 0..logical_count {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| {
                    Error::new("Structured receipt diagnostic rule coordinate overflow")
                })?,
            )
            .map_err(|error| {
                Error::new(format!("Structured receipt diagnostic rule: {error:?}"))
            })?;
        expected_physical_flags[representatives[coordinate]] |= rule.privileges();
    }
    let mut rows = Vec::with_capacity(logical_count);
    let mut mismatches = Vec::new();
    for coordinate in 0..logical_count {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| {
                    Error::new("Structured receipt diagnostic rule coordinate overflow")
                })?,
            )
            .map_err(|error| {
                Error::new(format!("Structured receipt diagnostic rule: {error:?}"))
            })?;
        let expected_width = u64::from(rule.data_length())
            .checked_add(u64::from(rule.data_item_stride()) * u64::from(TAIL_COUNT))
            .ok_or_else(|| Error::new("Structured receipt diagnostic width overflow"))?;
        let observed_width = u64::try_from(data[coordinate].len())
            .map_err(|_| Error::new("Structured receipt diagnostic observed width overflow"))?;
        let width_mismatch = match rule.prestate() {
            AccountPrestateV2::Exact => observed_width != expected_width,
            AccountPrestateV2::LifecycleBound => {
                observed_width != 0 && observed_width != expected_width
            }
            _ => false,
        };
        let rule_flags = rule.privileges();
        let observed_flags = u8::from(logical_metas[coordinate].is_signer)
            | (u8::from(logical_metas[coordinate].is_writable) << 1)
            | (u8::from(executable[coordinate]) << 2);
        let expected_flags = expected_physical_flags[representatives[coordinate]];
        let row = json!({
            "logicalCoordinate": coordinate,
            "representative": representatives[coordinate],
            "physicalOrdinal": ordinals[coordinate],
            "key": logical_metas[coordinate].pubkey.to_string(),
            "observedOwner": Pubkey::new_from_array(owners[coordinate]).to_string(),
            "observedDataLength": observed_width,
            "observedFlags": observed_flags,
            "rule": {
                "privileges": rule_flags,
                "expectedPhysicalPrivileges": expected_flags,
                "effectPermissions": rule.effect_permissions(),
                "aliasKind": format!("{:?}", rule.alias_kind()),
                "aliasIndex": rule.alias_index(),
                "prestate": format!("{:?}", rule.prestate()),
                "dataLength": rule.data_length(),
                "dataItemStride": rule.data_item_stride(),
                "expectedDataLength": expected_width,
            },
            "widthMismatch": width_mismatch,
            "privilegeMismatch": observed_flags != expected_flags,
        });
        if width_mismatch || observed_flags != expected_flags {
            mismatches.push(row.clone());
        }
        rows.push(row);
    }
    let result = json!({
        "schema": "dclutch-structured-account-profile-projection-v1",
        "finalizedSlot": slot,
        "profileSha256": crate::plan::hex(&hash(&artifacts.bundle.account_profile).to_bytes()),
        "nativeProjectionResult": format!("{projection:?}"),
        "logicalAccountCount": logical_count,
        "mismatches": mismatches,
        "coordinates": rows,
    });
    eprintln!(
        "structured receipt native AccountProfile projection: {}",
        serde_json::to_string(&result)?
    );
    Ok(result)
}

fn project_saved_structured_receipt_snapshot_v1(
    capture: &serde_json::Value,
    profile_bytes: &[u8],
) -> Result<serde_json::Value> {
    const TAIL_COUNT: u32 = 0;
    let profile = AccountProfileV2::decode(profile_bytes)
        .map_err(|error| Error::new(format!("Structured saved profile: {error:?}")))?;
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(|error| Error::new(format!("Structured saved profile geometry: {error:?}")))?;
    let captured_projection = capture
        .get("profileProjection")
        .ok_or_else(|| Error::new("Structured profile capture omitted its projection"))?;
    let captured_rows = captured_projection
        .get("coordinates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Error::new("Structured profile capture omitted coordinate rows"))?;
    if captured_rows.len() != logical_count {
        return Err(Error::new(
            "Structured saved profile logical width differs from captured frame",
        ));
    }
    let state = capture
        .get("state")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| Error::new("Structured profile capture omitted state"))?;
    let slot = captured_projection
        .get("finalizedSlot")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Error::new("Structured profile capture omitted finalized slot"))?;
    let executing_program = capture
        .get("instructions")
        .and_then(serde_json::Value::as_array)
        .and_then(|instructions| instructions.last())
        .and_then(|instruction| instruction.get("programId"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("Structured profile capture omitted Hot program"))?;
    let executing_program = pubkey(executing_program)?;

    let mut physical_keys = Vec::with_capacity(logical_count);
    let mut owners = Vec::with_capacity(logical_count);
    let mut data = Vec::with_capacity(logical_count);
    let mut lamports = Vec::with_capacity(logical_count);
    let mut executable = Vec::with_capacity(logical_count);
    let mut flags = Vec::with_capacity(logical_count);
    for (coordinate, row) in captured_rows.iter().enumerate() {
        if row
            .get("logicalCoordinate")
            .and_then(serde_json::Value::as_u64)
            != Some(
                u64::try_from(coordinate)
                    .map_err(|_| Error::new("Structured profile capture coordinate overflows"))?,
            )
        {
            return Err(Error::new(
                "Structured profile capture coordinates are not canonical",
            ));
        }
        let key_text = row
            .get("key")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::new("Structured profile capture row omitted key"))?;
        let key = pubkey(key_text)?;
        physical_keys.push(key.to_bytes());
        flags.push(
            u8::try_from(
                row.get("observedFlags")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| Error::new("Structured profile capture row omitted flags"))?,
            )
            .map_err(|_| Error::new("Structured profile capture flags overflow"))?,
        );
        if let Some(account) = state.get(key_text) {
            owners.push(
                pubkey(
                    account
                        .get("owner")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            Error::new("Structured profile capture account omitted owner")
                        })?,
                )?
                .to_bytes(),
            );
            data.push(
                BASE64
                    .decode(
                        account
                            .get("dataBase64")
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| {
                                Error::new("Structured profile capture account omitted data")
                            })?,
                    )
                    .map_err(|error| {
                        Error::new(format!(
                            "Structured profile capture account base64: {error}"
                        ))
                    })?,
            );
            lamports.push(
                account
                    .get("lamports")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| {
                        Error::new("Structured profile capture account omitted lamports")
                    })?,
            );
            executable.push(
                account
                    .get("executable")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or_else(|| {
                        Error::new("Structured profile capture account omitted executable")
                    })?,
            );
        } else {
            owners.push(solana_sdk_ids::system_program::ID.to_bytes());
            data.push(Vec::new());
            lamports.push(0);
            executable.push(false);
        }
    }
    let mut keys = physical_keys.clone();
    for coordinate in 1..=4 {
        keys[coordinate] = hash(&data[coordinate]).to_bytes();
    }
    let product_digest = hash(&data[2]).to_bytes();
    let mut observations = Vec::with_capacity(logical_count);
    for coordinate in 0..logical_count {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate)
                    .map_err(|_| Error::new("Structured saved rule coordinate overflows"))?,
            )
            .map_err(|error| Error::new(format!("Structured saved rule: {error:?}")))?;
        let signer = flags[coordinate] & 1 != 0;
        let writable = flags[coordinate] & 2 != 0;
        // Mirror the runtime adapter's authenticated shared-prefix boundary.
        // A profile declaration alone does not authenticate a child-owned body.
        let observation = if matches!(coordinate, 1 | 4)
            && matches!(
                rule.prestate(),
                AccountPrestateV2::AdapterAuthenticatedVariableData
            ) {
            AccountObservationV1::new_adapter_authenticated_variable_data(
                &keys[coordinate],
                &owners[coordinate],
                lamports[coordinate],
                &data[coordinate],
                signer,
                writable,
                executable[coordinate],
            )
        } else {
            let observation = AccountObservationV1::new(
                &keys[coordinate],
                &owners[coordinate],
                lamports[coordinate],
                &data[coordinate],
                signer,
                writable,
                executable[coordinate],
            );
            if coordinate == 2 {
                observation.with_adapter_data_digest(&product_digest)
            } else {
                observation
            }
        };
        observations.push(observation);
    }
    let scalar_count = usize::from(profile.common_scalar_count());
    let identity_count = usize::from(profile.common_identity_count());
    let mut input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    if let Some(destination) = profile.trusted_current_slot_scalar() {
        input_scalars[usize::from(destination)] = slot;
    }
    if let Some(destination) = profile.trusted_current_executing_program_identity() {
        input_identities[usize::from(destination)] = executing_program.to_bytes();
    }
    if let Some(destination) = profile.trusted_system_program_identity() {
        input_identities[usize::from(destination)] = solana_sdk_ids::system_program::ID.to_bytes();
    }
    let mut scratch_scalars = vec![0_u64; scalar_count];
    let mut scratch_identities = vec![[0_u8; 32]; identity_count];
    let mut output_scalars = vec![0_u64; scalar_count];
    let mut output_identities = vec![[0_u8; 32]; identity_count];
    let projection = project_dynamic_fixed_spans_atomic(
        profile,
        TAIL_COUNT,
        &[],
        &observations,
        ProjectionRegistersV2 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
        None,
    );
    let mut mismatches = Vec::new();
    for coordinate in 0..logical_count {
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate)
                    .map_err(|_| Error::new("Structured saved mismatch coordinate overflows"))?,
            )
            .map_err(|error| Error::new(format!("Structured saved mismatch rule: {error:?}")))?;
        let expected = u64::from(rule.data_length())
            .checked_add(u64::from(rule.data_item_stride()) * u64::from(TAIL_COUNT))
            .ok_or_else(|| Error::new("Structured saved expected width overflows"))?;
        let observed = u64::try_from(data[coordinate].len())
            .map_err(|_| Error::new("Structured saved observed width overflows"))?;
        if (matches!(rule.prestate(), AccountPrestateV2::Exact) && observed != expected)
            || (matches!(rule.prestate(), AccountPrestateV2::LifecycleBound)
                && observed != 0
                && observed != expected)
        {
            mismatches.push(json!({
                "logicalCoordinate": coordinate,
                "key": Pubkey::new_from_array(physical_keys[coordinate]).to_string(),
                "observedOwner": Pubkey::new_from_array(owners[coordinate]).to_string(),
                "observedDataLength": observed,
                "expectedDataLength": expected,
                "prestate": format!("{:?}", rule.prestate()),
            }));
        }
    }
    Ok(json!({
        "profileSha256": sha256_hex(profile_bytes),
        "finalizedSlot": slot,
        "nativeProjectionResult": format!("{projection:?}"),
        "mismatches": mismatches,
    }))
}

/// Create the permissionless seal (when vacant) and execute the first real
/// Structured lifecycle action in one routed, atomic transaction.  Every
/// coordinate is recovered from finalized records or the root just accepted;
/// the only fresh writable PDA is the receipt Mint the Claims instruction
/// itself creates.
fn activate_structured_receipt_v1(
    arguments: &ArgumentsV1,
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    market_input: &[u8],
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
    coordinate_artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
    closure: &crate::structured_claims_producer::StructuredPublicationClosureV1,
    published: &crate::structured_claims_producer::PublishedStructuredClosureV1,
    market: Pubkey,
    root_activation: &serde_json::Value,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
    terminal_driver: Option<&mut dyn StructuredTerminalDriverV1>,
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
    let selected_release = current_source_selected_release_v1(market_input)?;
    if selected_release.program_set != crate::runtime::decode_hex(&selected.program_set_hex)?
        || selected_release.config != crate::runtime::decode_hex(&selected.config_hex)?
        || selected_release.publication.to_bytes().as_slice()
            != crate::runtime::decode_hex(&selected.publication_hex)?.as_slice()
    {
        return Err(Error::new(
            "Structured current-source bundles differ from immutable selected publication",
        ));
    }
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
    let receipt_descriptor = &closure.representation_descriptor.preimage;
    let published_descriptor = published.records.get(6).ok_or_else(|| {
        Error::new("Structured receipt publication omitted representation descriptor")
    })?;
    let expected_descriptor = authenticate_receipt_descriptor_publication_v1(
        registry,
        receipt_descriptor,
        published_descriptor,
    )?;
    let expected_descriptor_digest = expected_descriptor.content;
    let observed_descriptor = rpc
        .account(published_descriptor.raw)?
        .ok_or_else(|| Error::new("Structured receipt publication descriptor vanished"))?;
    if observed_descriptor.owner != registry || observed_descriptor.data != *receipt_descriptor {
        return Err(Error::new(
            "Structured receipt publication descriptor finalized bytes differ",
        ));
    }
    let representation_descriptor_record = selected_pair_v1(
        registry,
        dclutch_claims::rational_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        receipt_descriptor,
    )?;
    let config = selected_pair_v1(registry, artifacts.config.schema, &artifacts.config.body)?;
    let pair = |index: usize| -> Result<SelectedActivationRecordPairV1> {
        let value = &artifacts.bundle_records[index];
        selected_pair_v1(registry, value.schema, &value.body)
    };
    let capability_descriptor = pair(0)?;
    let profile = pair(1)?;
    let request = pair(2)?;
    let lifecycle = pair(3)?;
    let strategy = pair(4)?;
    let transition = pair(5)?;
    let effect = pair(6)?;
    let mut report_pair = |label: &str,
                           schema: [u8; 32]|
     -> Result<SelectedActivationRecordPairV1> {
        let row = evidence
            .accounts
            .get(label)
            .ok_or_else(|| Error::new(format!("Structured receipt report omitted {label}")))?;
        let pair = pair_from_digest_v1(registry, schema, crate::plan::hex32(&row.data_sha256)?)?;
        require_live_structured_record_v1(rpc, registry, pair, label)?;
        Ok(pair)
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
    let descriptor_id = expected_descriptor_digest;
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &claims,
    )
    .0;
    let descriptor_value = RepresentationDescriptorV2::decode(
        receipt_descriptor,
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

    let exposure_id: [u8; 32] = sha2::Sha256::digest(&closure.exposure).into();
    let exposure = CompositionExposureBundleV3::decode(
        &closure.exposure,
        RecordAdmissionV3 {
            selected_id: exposure_id,
            finalized_id: exposure_id,
            recomputed_digest: exposure_id,
            finalized_digest: exposure_id,
            record_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured receipt exposure: {error:?}")))?;
    descriptor_value
        .authenticate_exposure(exposure)
        .map_err(|error| {
            Error::new(format!("Structured receipt descriptor/exposure: {error:?}"))
        })?;

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
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, capability_descriptor),
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
        capability_descriptor.content,
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
    drop(place);
    // Resume from finalized resources, never from a saved success flag. The
    // canonical token profile below authenticates an already existing receipt.
    let (receipt_slot, receipt_tables) = if accounts[3].is_none() {
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
                exposure_product_width: exposure.product_width(),
            },
        )?;
        let header = derive_selected_lifecycle_parent_v6(header)?;
        let mut lifecycle_bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2];
        LifecycleRequestV2::new(header, &[])
            .map_err(|error| Error::new(format!("Structured receipt lifecycle header: {error:?}")))?
            .encode_into(&mut lifecycle_bytes)
            .map_err(|error| {
                Error::new(format!("Structured receipt lifecycle encode: {error:?}"))
            })?;
        let mut claims_child = activate_receipt_claims_instruction_v1(
            ActivateReceiptPhysicalInputsV1 {
                trading,
                trading_programdata: pubkey(&plan.trading.programdata_id)?,
                claims,
                claims_programdata: pubkey(&plan.claims.programdata_id)?,
                registry,
                activation_cache: pubkey(&plan.activation)?,
                descriptor_raw: representation_descriptor_record.raw,
                descriptor_staging: representation_descriptor_record.staging,
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
        // The child frame records the caller-authority PDA as a signer for the
        // Claims CPI. Hot's selected operator removes that signer bit from the
        // outer transaction account, so the producer must preserve the child
        // semantic fact here rather than emitting a transaction-shaped child.
        mark_claims_child_caller_signer_v1(&mut claims_child)?;
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
        let seal_instruction = if seal_before.is_none() {
            Some(
                capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                    trading_program: trading,
                    registry_program: registry,
                    trading_semantic_release: activated
                        .role(ExecutionRoleV1::Trading)
                        .release()
                        .semantic_release_id()
                        .to_bytes(),
                    descriptor_digest: capability_descriptor.content,
                    action: STRUCTURED_ACTIVATE_RECEIPT_SELECTOR_V1,
                    fixed_frame: &fixed,
                    payer: payer.pubkey(),
                })
                .map_err(|error| Error::new(format!("Structured receipt seal builder: {error:?}")))?
                .instruction,
            )
        } else {
            None
        };
        let hot_instruction = hot.instruction;
        let receipt_funding = solana_system_interface::instruction::transfer(
            &payer.pubkey(),
            &receipt_mint,
            header.observed_receipt_lamports,
        );
        let funded_hot = [receipt_funding, hot_instruction.clone()];
        // The root is read-only to the permissionless seal outer, while Hot owns
        // its writable root frame. Solana merges account privileges across one
        // message, so putting both instructions in one transaction upgrades the
        // seal's root meta and makes Trading refuse its exact frame conjunction.
        // Publish one table for both packets, then submit the seal in its own
        // transaction before the Hot action.
        let mut instructions = Vec::new();
        if let Some(seal) = seal_instruction.as_ref() {
            instructions.push(seal.clone());
        }
        instructions.extend_from_slice(&funded_hot);
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
        if let Some(path) = std::env::var_os("DCLUTCH_STRUCTURED_FRAME_CAPTURE") {
            let profile_projection = structured_receipt_profile_projection_v1(
                rpc,
                &frame,
                &claims_child,
                &hot_instruction,
                artifacts,
            )?;
            let capture_instructions =
                if std::env::var_os("DCLUTCH_STRUCTURED_CAPTURE_SEAL_ONLY").is_some() {
                    instructions.get(..1).ok_or_else(|| {
                        Error::new("Structured receipt capture omitted seal instruction")
                    })?
                } else if std::env::var_os("DCLUTCH_STRUCTURED_CAPTURE_HOT_ONLY").is_some() {
                    if seal_instruction.is_some() {
                        instructions.get(1..).ok_or_else(|| {
                            Error::new("Structured receipt capture omitted Hot instruction")
                        })?
                    } else {
                        instructions.as_slice()
                    }
                } else {
                    instructions.as_slice()
                };
            write_structured_frame_capture_v1(
                rpc,
                Path::new(&path),
                capture_instructions,
                payer.pubkey(),
                observation,
                &tables,
                profile_projection,
            )?;
            return Err(Error::new(
                "Structured receipt diagnostic capture written; submission skipped",
            ));
        }
        if let Some(seal) = seal_instruction.as_ref() {
            let seal_sent = rpc.send_v0_on_heap(
                "materialize Structured receipt capability seal",
                std::slice::from_ref(seal),
                payer,
                observation,
                &tables,
                DIRECT_HOT_HEAP_FRAME_BYTES_V1,
            )?;
            if let Some(error) = seal_sent.error.as_ref() {
                return Err(Error::new(format!(
                    "Structured receipt seal refused on chain: {error}"
                )));
            }
            transactions.push(seal_sent);
            write_structured_progress_v1(arguments, "receipt-sealed", transactions)?;
        }
        let sent = rpc.send_v0_on_heap(
            "activate Structured receipt",
            &funded_hot,
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
        write_structured_progress_v1(arguments, "receipt-activated", transactions)?;
        (
            sent.slot,
            tables
                .iter()
                .map(|table| table.key.to_string())
                .collect::<Vec<_>>(),
        )
    } else {
        (slot, Vec::new())
    };
    let mint = rpc.account(receipt_mint)?.ok_or_else(|| {
        Error::new("Structured receipt transaction landed without a receipt Mint")
    })?;
    let receipt_supply = dclutch_custody::token_svm::Mint::parse(
        mint.data
            .get(..dclutch_custody::token_svm::MINT_BYTES)
            .ok_or_else(|| Error::new("Structured receipt Mint is truncated"))?,
    )
    .map_err(|error| Error::new(format!("Structured receipt Mint: {error:?}")))?
    .supply;
    dclutch_custody::token_svm::Token2022CloseableMintProfileV2::check_mint(
        mint.owner.to_bytes(),
        &mint.data,
        representation_authority.to_bytes(),
        representation_authority.to_bytes(),
        representation_authority.to_bytes(),
        receipt_supply,
        0,
    )
    .map_err(|error| Error::new(format!("Structured resumed receipt profile: {error:?}")))?;
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
    let coordinates = activate_structured_coordinates_v1(
        rpc,
        payer,
        plan,
        evidence,
        coordinate_artifacts,
        LifecycleActionV2::ActivateCoordinate,
        market,
        root,
        representation_descriptor_record,
        descriptor_value,
        exposure,
        hot_outer,
        core_state.identity.realm_id.to_bytes(),
        &mut fixed,
        transactions,
    )?;
    write_json(
        &arguments.output.with_extension("activation-progress.json"),
        &json!({
            "schema": "dclutch-structured-activation-progress-v1", "market": market.to_string(),
            "receiptMint": receipt_mint.to_string(), "coordinates": coordinates,
            "transactions": transactions,
        }),
    )?;
    let native_claims = prepare_structured_native_position_v1(
        arguments,
        rpc,
        payer,
        evidence,
        market,
        transactions
            .last()
            .map_or(receipt_slot, |transaction| transaction.slot),
        transactions,
    )?;
    let representation_journal = arguments
        .output
        .with_extension("representation-actions.json");
    let mut checkpoints: Vec<
        crate::structured_representation_campaign::StructuredRepresentationActionCheckpointV1,
    > = if representation_journal.exists() {
        serde_json::from_slice(&std::fs::read(&representation_journal)?)?
    } else {
        Vec::new()
    };
    let resume_checkpoints = checkpoints.clone();
    let mut persist_checkpoint = |checkpoint: &crate::structured_representation_campaign::StructuredRepresentationActionCheckpointV1| -> Result<()> {
        checkpoints.push(checkpoint.clone());
        write_json(&representation_journal, &serde_json::to_value(&checkpoints)?)
    };
    let representation =
        crate::structured_representation_campaign::run_structured_representation_campaign_v1(
            crate::structured_representation_campaign::StructuredRepresentationCampaignInputV1 {
                resume_checkpoints: &resume_checkpoints,
                rpc,
                fee_payer: payer,
                actor: payer,
                plan,
                campaign: evidence,
                selected_release: &selected_release,
                market,
                root,
                representation_descriptor: representation_descriptor_record,
                representation_descriptor_body: receipt_descriptor,
                composition_exposure: selected_pair_v1(
                    registry,
                    dclutch_claims::composition::COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                    &closure.exposure,
                )?,
                composition_exposure_body: &closure.exposure,
                hot_fixed: &fixed,
                lifecycle_hot_outer: hot_outer,
                minimum_finalized_slot: transactions
                    .last()
                    .map_or(receipt_slot, |transaction| transaction.slot),
            },
            transactions,
            &mut persist_checkpoint,
        )?;
    let retirement = if let Some(driver) = terminal_driver {
        let terminal =
            driver.resolve_and_settle_native(rpc, plan, evidence, payer, market, transactions)?;
        Some(complete_structured_terminal_v1(
            arguments,
            rpc,
            payer,
            plan,
            evidence,
            market_input,
            market,
            root,
            &selected_release,
            representation_descriptor_record,
            receipt_descriptor,
            descriptor_value,
            &closure.exposure,
            exposure,
            hot_outer,
            &mut fixed,
            terminal,
            driver,
            transactions,
        )?)
    } else {
        None
    };
    Ok(
        json!({"slot": receipt_slot, "receiptMint": receipt_mint.to_string(), "receiptMintLamports": mint.lamports, "receiptMintBytes": mint.data.len(), "root": root.to_string(), "rootBytes": root_after.data.len(), "rootStateSha256": sha256_hex(root_tail), "seal": fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3].to_string(), "routingTables": receipt_tables, "coordinates": coordinates, "nativeClaims": native_claims, "representation": representation, "retirement": retirement }),
    )
}

/// Materialize every descriptor coordinate through the selected coordinate
/// creation route and read back all four resources before representation may
/// begin. Each request is rebuilt from a fresh finalized state snapshot.
#[allow(clippy::too_many_arguments)]
fn activate_structured_coordinates_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    artifacts: &structured_activation::StructuredActivateReceiptArtifactsV1,
    action: LifecycleActionV2,
    market: Pubkey,
    root: Pubkey,
    representation_descriptor_record: SelectedActivationRecordPairV1,
    descriptor: RepresentationDescriptorV2<'_>,
    exposure: CompositionExposureBundleV3<'_>,
    hot_outer: CheckedRationalLifecycleHotOuterV3,
    market_realm: [u8; 32],
    fixed: &mut [Pubkey],
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<Vec<serde_json::Value>> {
    use dclutch_custody::token_svm::{
        ACCOUNT_BYTES, MINT_BYTES, Mint, TOKEN_2022_CLOSEABLE_MINT_BYTES_V2,
    };
    use dclutch_registry::{ActivatedExecutionReleaseSetV1, release_set::ExecutionRoleV1};

    let retiring = action == LifecycleActionV2::RetireCoordinate;
    if !matches!(
        action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) {
        return Err(Error::new(
            "Structured coordinate action differs from coordinate lifecycle",
        ));
    }
    let selector =
        dclutch_claims::rational_lifecycle::hot_v6::structured_lifecycle_action_selector_v1(
            STRUCTURED_CAPABILITY_KIND_ID_V2,
            action,
        )
        .ok_or_else(|| Error::new("Structured coordinate selector is absent"))?;
    let registry = pubkey(&plan.registry.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let aggregate = pubkey(
        &evidence
            .accounts
            .get("claims_aggregate")
            .ok_or_else(|| Error::new("Structured coordinate report omitted claims aggregate"))?
            .address,
    )?;
    let mut report_pair =
        |label: &str, schema: [u8; 32]| -> Result<SelectedActivationRecordPairV1> {
            let row = evidence.accounts.get(label).ok_or_else(|| {
                Error::new(format!("Structured coordinate report omitted {label}"))
            })?;
            let pair =
                pair_from_digest_v1(registry, schema, crate::plan::hex32(&row.data_sha256)?)?;
            require_live_structured_record_v1(rpc, registry, pair, label)?;
            Ok(pair)
        };
    let product = report_pair(
        "product_record",
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let result = report_pair(
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
    let pair = |index: usize| -> Result<SelectedActivationRecordPairV1> {
        let value = &artifacts.bundle_records[index];
        selected_pair_v1(registry, value.schema, &value.body)
    };
    let capability_descriptor = pair(0)?;
    for (index, record) in [
        (HOT_DESCRIPTOR_RAW_ACCOUNT_V3, capability_descriptor),
        (HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, pair(1)?),
        (HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, pair(2)?),
        (HOT_LIFECYCLE_RAW_ACCOUNT_V3, pair(3)?),
        (HOT_STRATEGY_RAW_ACCOUNT_V3, pair(4)?),
        (HOT_TRANSITION_RAW_ACCOUNT_V3, pair(5)?),
        (HOT_EFFECT_RAW_ACCOUNT_V3, pair(6)?),
    ] {
        fixed[index] = record.raw;
        fixed[index + 1] = record.staging;
    }
    let representation_authority = Pubkey::new_from_array(descriptor.representation_authority());
    let receipt_mint = Pubkey::new_from_array(descriptor.receipt_mint());
    let core_state = CoreState::decode(
        &rpc.account(market)?
            .ok_or_else(|| Error::new("Structured coordinate Market vanished"))?
            .data,
    )
    .map_err(|error| Error::new(format!("Structured coordinate Market: {error:?}")))?;
    let rent_credit = Pubkey::new_from_array(core_state.rent_beneficiary.to_bytes());
    let activation = rpc
        .account(pubkey(&plan.activation)?)?
        .ok_or_else(|| Error::new("Structured coordinate activation cache vanished"))?;
    let activated = ActivatedExecutionReleaseSetV1::decode(&activation.data).map_err(|error| {
        Error::new(format!("Structured coordinate activation cache: {error:?}"))
    })?;
    let trading_release = activated.role(ExecutionRoleV1::Trading).release();
    let seal_key = dclutch_vm::capability_seal::CapabilitySealKeyV1::new(
        dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
        capability_descriptor.content,
        selector,
        trading_release.semantic_release_id().to_bytes(),
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured coordinate seal key: {error:?}")))?;
    fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3] =
        Pubkey::find_program_address(&seal_key.seeds().as_slices(), &trading).0;

    let mut accepted = Vec::new();
    for outcome in 0..descriptor.outcome_count() {
        if descriptor
            .coefficient(outcome)
            .map_err(|error| Error::new(format!("Structured coordinate coefficient: {error:?}")))?
            == 0
        {
            continue;
        }
        let outcome_bytes = outcome.to_le_bytes();
        let shard_mint = Pubkey::find_program_address(
            &[
                RATIONAL_SHARD_MINT_SEED_V2,
                &descriptor.descriptor_id(),
                &outcome_bytes,
            ],
            &claims,
        )
        .0;
        let structured_custody = Pubkey::find_program_address(
            &[
                RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                &descriptor.descriptor_id(),
                &outcome_bytes,
            ],
            &claims,
        )
        .0;
        let owner_seeds =
            ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor.descriptor_id(), outcome)
                .map_err(|error| {
                    Error::new(format!("Structured coordinate owner seeds: {error:?}"))
                })?;
        let owner = Pubkey::find_program_address(&owner_seeds.as_slices(), &claims).0;
        let position_seeds = ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
            .map_err(|error| {
            Error::new(format!("Structured coordinate Position seeds: {error:?}"))
        })?;
        let admission_seeds =
            ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes()).map_err(
                |error| Error::new(format!("Structured coordinate admission seeds: {error:?}")),
            )?;
        let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0;
        let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &claims).0;
        let addresses = [
            market,
            aggregate,
            rent_credit,
            receipt_mint,
            solana_sdk_ids::sysvar::rent::ID,
            root,
            shard_mint,
            structured_custody,
            position,
            admission,
        ];
        let (slot, accounts) = rpc.finalized_accounts(&addresses, artifacts.slot)?;
        let required = |index: usize, label: &str| -> Result<crate::rpc::RpcAccount> {
            accounts.get(index).cloned().flatten().ok_or_else(|| {
                Error::new(format!("Structured coordinate snapshot omitted {label}"))
            })
        };
        let market_account = required(0, "Market")?;
        let aggregate_account = required(1, "Claims aggregate")?;
        let rent_credit_account = required(2, "RentCredit")?;
        let receipt_account = required(3, "receipt Mint")?;
        let rent_account = required(4, "Rent sysvar")?;
        let root_account = required(5, "capability root")?;
        let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
            .map_err(|error| Error::new(format!("Structured coordinate aggregate: {error:?}")))?;
        let vacant_resource = |account: &Option<crate::rpc::RpcAccount>| {
            account.as_ref().is_none_or(|account| {
                account.owner == solana_sdk_ids::system_program::ID
                    && !account.executable
                    && account.data.is_empty()
            })
        };
        if !retiring
            && accounts[6..]
                .iter()
                .any(|account| !vacant_resource(account))
        {
            let shard = required(6, "resumed shard Mint")?;
            let custody = required(7, "resumed custody")?;
            let position_account = required(8, "resumed Position")?;
            let admission_account = required(9, "resumed admission")?;
            let supply = Mint::parse(
                shard
                    .data
                    .get(..MINT_BYTES)
                    .ok_or_else(|| Error::new("Structured resumed shard is truncated"))?,
            )
            .map_err(|error| Error::new(format!("Structured resumed shard: {error:?}")))?
            .supply;
            dclutch_custody::token_svm::Token2022CloseableMintProfileV2::check_mint(
                shard.owner.to_bytes(),
                &shard.data,
                representation_authority.to_bytes(),
                representation_authority.to_bytes(),
                representation_authority.to_bytes(),
                supply,
                0,
            )
            .map_err(|error| Error::new(format!("Structured resumed shard profile: {error:?}")))?;
            let token = dclutch_custody::token_svm::TokenAccount::parse(&custody.data)
                .map_err(|error| Error::new(format!("Structured resumed custody: {error:?}")))?;
            let position_view =
                dclutch_claims::liability_basis_state_v2::LiabilityBasisPositionViewV2::decode(
                    &position_account.data,
                )
                .map_err(|error| Error::new(format!("Structured resumed Position: {error:?}")))?;
            let admitted =
                dclutch_claims::protocol_position_v2::ProtocolPositionAdmissionV2::decode(
                    &admission_account.data,
                )
                .map_err(|error| Error::new(format!("Structured resumed admission: {error:?}")))?
                .request();
            if custody.owner.to_bytes() != descriptor.token_program()
                || token.mint != shard_mint.to_bytes()
                || token.owner != representation_authority.to_bytes()
                || token.state != dclutch_custody::token_svm::AccountState::Initialized
                || !token.delegate.is_none()
                || token.delegated_amount != 0
                || !token.native_reserve.is_none()
                || !token.close_authority.is_none()
                || position_account.owner != claims
                || admission_account.owner != claims
                || position_view.market_account != aggregate.to_bytes()
                || position_view.owner != owner.to_bytes()
                || position_view.basis_id != aggregate_view.basis_id
                || position_view.claim_count != aggregate_view.claim_count
                || admitted.market != market.to_bytes()
                || admitted.release_set != core_state.identity.selected_release_set.to_bytes()
                || admitted.generation != core_state.identity.generation
                || admitted.position_owner != owner.to_bytes()
                || admitted.capability_descriptor != descriptor.descriptor_id()
                || admitted.capability_outcome != outcome
                || admitted.rent_credit != rent_credit.to_bytes()
                || admitted.rent_program != rent_program.to_bytes()
            {
                return Err(Error::new(
                    "Structured resumed coordinate identities differ",
                ));
            }
            accepted.push(json!({"outcome": outcome, "slot": slot, "resumedFromFinalizedAccounts": true,
                "shardMint": shard_mint.to_string(), "structuredCustody": structured_custody.to_string(),
                "claimsCustodyPosition": position.to_string(), "positionAdmission": admission.to_string()}));
            continue;
        }
        let rent_observed = dclutch_operator::ObservedAccount {
            observation: dclutch_operator::Observation {
                slot,
                unix_timestamp: rpc.block_time(slot)?,
                finality: dclutch_operator::Finality::Finalized,
            },
            key: solana_sdk_ids::sysvar::rent::ID,
            owner: rent_account.owner,
            lamports: rent_account.lamports,
            executable: rent_account.executable,
            data: rent_account.data.clone(),
        };
        let rent = decode_rent(&rent_observed)
            .map_err(|error| Error::new(format!("Structured coordinate Rent: {error:?}")))?;
        let receipt_supply = Mint::parse(
            receipt_account
                .data
                .get(..MINT_BYTES)
                .ok_or_else(|| Error::new("Structured coordinate receipt Mint is truncated"))?,
        )
        .map_err(|error| Error::new(format!("Structured coordinate receipt Mint: {error:?}")))?
        .supply;
        let position_width = liability_basis_vector_width_v2(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            exposure.product_width(),
        )
        .map_err(|error| Error::new(format!("Structured coordinate Position width: {error:?}")))?;
        let mut row = LifecycleCoordinateV2 {
            outcome,
            coefficient: descriptor.coefficient(outcome).map_err(|error| {
                Error::new(format!("Structured coordinate coefficient: {error:?}"))
            })?,
            shard_mint: shard_mint.to_bytes(),
            structured_custody_account: structured_custody.to_bytes(),
            claims_custody_owner: owner.to_bytes(),
            claims_custody_position: position.to_bytes(),
            position_admission: admission.to_bytes(),
            observed_shard_lamports: accounts[6]
                .as_ref()
                .map_or(0, |account| account.lamports)
                .max(rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2)),
            observed_structured_lamports: accounts[7]
                .as_ref()
                .map_or(0, |account| account.lamports)
                .max(rent.minimum_balance(ACCOUNT_BYTES)),
            observed_position_lamports: accounts[8]
                .as_ref()
                .map_or(0, |account| account.lamports)
                .max(rent.minimum_balance(position_width)),
            observed_admission_lamports: accounts[9]
                .as_ref()
                .map_or(0, |account| account.lamports)
                .max(rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)),
            shard_rent_principal: rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2),
            structured_rent_principal: rent.minimum_balance(ACCOUNT_BYTES),
            position_rent_principal: rent.minimum_balance(position_width),
            admission_rent_principal: rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
            expected_shard_supply: 0,
            expected_structured_amount: 0,
            expected_position_revision: 0,
        };
        if retiring {
            let shard = required(6, "shard Mint")?;
            let custody = required(7, "Structured custody")?;
            let position_account = required(8, "Claims custody Position")?;
            let admission_account = required(9, "Position admission")?;
            row.observed_shard_lamports = shard.lamports;
            row.observed_structured_lamports = custody.lamports;
            row.observed_position_lamports = position_account.lamports;
            row.observed_admission_lamports = admission_account.lamports;
            row.expected_shard_supply = Mint::parse(
                shard
                    .data
                    .get(..MINT_BYTES)
                    .ok_or_else(|| Error::new("Structured retiring shard truncated"))?,
            )
            .map_err(|error| Error::new(format!("Structured retiring shard: {error:?}")))?
            .supply;
            row.expected_structured_amount =
                dclutch_custody::token_svm::TokenAccount::parse(&custody.data)
                    .map_err(|error| Error::new(format!("Structured retiring custody: {error:?}")))?
                    .amount;
            let position_view =
                dclutch_claims::liability_basis_state_v2::LiabilityBasisPositionViewV2::decode(
                    &position_account.data,
                )
                .map_err(|error| Error::new(format!("Structured retiring Position: {error:?}")))?;
            row.expected_position_revision = position_view.revision;
        }
        let credit_after = if retiring {
            [
                row.observed_shard_lamports,
                row.observed_structured_lamports,
                row.observed_position_lamports,
                row.observed_admission_lamports,
            ]
            .into_iter()
            .try_fold(rent_credit_account.lamports, |credit, value| {
                credit
                    .checked_add(value)
                    .ok_or_else(|| Error::new("Structured retirement rent credit overflows"))
            })?
        } else {
            rent_credit_account.lamports
        };
        let header = LifecycleHeaderV2 {
            action,
            release_set: core_state.identity.selected_release_set.to_bytes(),
            market: market.to_bytes(),
            graph_id: descriptor.graph_id(),
            descriptor_id: descriptor.descriptor_id(),
            parent_context: [0; 32],
            representation_authority: representation_authority.to_bytes(),
            receipt_mint: receipt_mint.to_bytes(),
            token_program: descriptor.token_program(),
            rent_credit: rent_credit.to_bytes(),
            rent_program: rent_program.to_bytes(),
            generation: core_state.identity.generation,
            expected_claims_market_revision: aggregate_view.revision,
            observed_receipt_lamports: receipt_account.lamports,
            receipt_rent_principal: rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2),
            expected_receipt_supply: receipt_supply,
            outcome_count: descriptor.outcome_count(),
            coordinate_count: 1,
            rent_credit_before: rent_credit_account.lamports,
            rent_credit_after: credit_after,
        };
        let mut coordinate_bytes = vec![0; LIFECYCLE_COORDINATE_BYTES_V2];
        row.encode_into(&mut coordinate_bytes)
            .map_err(|error| Error::new(format!("Structured coordinate row encode: {error:?}")))?;
        let header =
            derive_selected_lifecycle_parent_with_coordinates_v6(header, &coordinate_bytes)?;
        let mut lifecycle_bytes =
            vec![0; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        LifecycleRequestV2::new(header, &coordinate_bytes)
            .map_err(|error| Error::new(format!("Structured coordinate lifecycle: {error:?}")))?
            .encode_into(&mut lifecycle_bytes)
            .map_err(|error| {
                Error::new(format!("Structured coordinate lifecycle encode: {error:?}"))
            })?;
        let lifecycle_digest = hash(&lifecycle_bytes).to_bytes();
        let protocol = LifecycleRequestV2::decode(&lifecycle_bytes)
            .map_err(|error| {
                Error::new(format!("Structured coordinate lifecycle decode: {error:?}"))
            })?
            .protocol_position_request(lifecycle_digest)
            .map_err(|error| {
                Error::new(format!("Structured coordinate Position request: {error:?}"))
            })?;
        let protocol_bytes = protocol.to_bytes().map_err(|error| {
            Error::new(format!("Structured coordinate Position encode: {error:?}"))
        })?;
        let child_authority_seeds =
            dclutch_registry::release_set::CallerAuthoritySeedsV1::from_bytes(
                header.release_set,
                header.market,
                dclutch_registry::release_set::ExecutionRoleV1::Trading,
                owner.to_bytes(),
                hash(&protocol_bytes).to_bytes(),
            )
            .map_err(|error| {
                Error::new(format!("Structured coordinate child authority: {error:?}"))
            })?;
        let child_authority =
            Pubkey::find_program_address(&child_authority_seeds.as_slices(), &trading).0;
        let child = coordinate_claims_instruction_v1(
            CoordinatePhysicalInputsV1 {
                common: ActivateReceiptPhysicalInputsV1 {
                    trading,
                    trading_programdata: pubkey(&plan.trading.programdata_id)?,
                    claims,
                    claims_programdata: pubkey(&plan.claims.programdata_id)?,
                    registry,
                    activation_cache: pubkey(&plan.activation)?,
                    descriptor_raw: representation_descriptor_record.raw,
                    descriptor_staging: representation_descriptor_record.staging,
                    representation_authority,
                    receipt_mint,
                    rent_credit,
                    rent_program,
                    claims_market: aggregate,
                    core_market: market,
                    core,
                    core_programdata: pubkey(&plan.core.programdata_id)?,
                },
                child_authority,
                position,
                admission,
                shard_mint,
                structured_custody,
                owner,
                basis_record: basis.raw,
                basis_staging: basis.staging,
                product_record: product.raw,
                product_staging: product.staging,
                result_record: result.raw,
                result_staging: result.staging,
                portfolio_record: portfolio.raw,
                portfolio_staging: portfolio.staging,
            },
            &lifecycle_bytes,
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
            root_data: root_account.data,
            market_data: market_account.data,
            release_set: core_state.identity.selected_release_set.to_bytes(),
            market,
            generation: core_state.identity.generation,
            finalized_slot: slot,
            hot_outer,
        };
        let hot = if retiring {
            structured_activation::build_selected_retirement_instruction_v1(
                artifacts,
                &frame.state()?,
                &child,
                descriptor,
                market_realm,
            )?
        } else {
            structured_activation::build_selected_activate_coordinate_instruction_v1(
                artifacts,
                &frame.state()?,
                &child,
                descriptor,
                market_realm,
            )?
        };
        let seal = if rpc
            .account(fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3])?
            .is_none()
        {
            Some(
                capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                    trading_program: trading,
                    registry_program: registry,
                    trading_semantic_release: trading_release.semantic_release_id().to_bytes(),
                    descriptor_digest: capability_descriptor.content,
                    action: selector,
                    fixed_frame: fixed,
                    payer: payer.pubkey(),
                })
                .map_err(|error| {
                    Error::new(format!("Structured coordinate seal builder: {error:?}"))
                })?
                .instruction,
            )
        } else {
            None
        };
        let mut prefunding = [
            (shard_mint, row.observed_shard_lamports),
            (structured_custody, row.observed_structured_lamports),
            (position, row.observed_position_lamports),
            (admission, row.observed_admission_lamports),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(index, (account, lamports))| {
            let observed = accounts[6 + index]
                .as_ref()
                .map_or(0, |account| account.lamports);
            let deficit = lamports.saturating_sub(observed);
            (deficit != 0).then(|| {
                solana_system_interface::instruction::transfer(&payer.pubkey(), &account, deficit)
            })
        })
        .collect::<Vec<_>>();
        if retiring {
            prefunding.clear();
        }
        let funded_hot = [hot.instruction];
        let mut routing = std::collections::BTreeSet::new();
        for instruction in seal.iter().chain(funded_hot.iter()) {
            routing.insert(instruction.program_id);
            routing.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
        }
        let routing = routing.into_iter().collect::<Vec<_>>();
        let (observation, tables) = crate::market::publish_routing_table_over_v1(
            rpc,
            payer,
            &format!("STRUCTURED-COORDINATE-{outcome}"),
            &routing,
            transactions,
        )?;
        if let Some(seal) = seal.as_ref() {
            let sent = rpc.send_v0_on_heap(
                "materialize Structured coordinate capability seal",
                std::slice::from_ref(seal),
                payer,
                observation,
                &tables,
                DIRECT_HOT_HEAP_FRAME_BYTES_V1,
            )?;
            if let Some(error) = sent.error.as_ref() {
                return Err(Error::new(format!(
                    "Structured coordinate seal refused on chain: {error}"
                )));
            }
            transactions.push(sent);
        }
        // Rent is prepaid before the native entrance. Keeping these transfers
        // outside the 672-byte lifecycle request avoids exceeding Solana's
        // packet limit; system-owned empty prefunding remains resumable above.
        if !prefunding.is_empty() {
            let funded = rpc.send_v0_on_heap(
                &format!("prepay Structured coordinate {outcome} rent"),
                &prefunding,
                payer,
                observation,
                &tables,
                DIRECT_HOT_HEAP_FRAME_BYTES_V1,
            )?;
            if let Some(error) = funded.error.as_ref() {
                return Err(Error::new(format!(
                    "Structured coordinate rent funding refused: {error}"
                )));
            }
            transactions.push(funded);
        }
        let sent = rpc.send_v0_on_heap(
            &format!("{action:?} Structured coordinate {outcome}"),
            &funded_hot,
            payer,
            observation,
            &tables,
            DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        )?;
        if let Some(error) = sent.error.as_ref() {
            return Err(Error::new(format!(
                "Structured coordinate {outcome} activation refused on chain: {error}"
            )));
        }
        transactions.push(sent.clone());
        let (_, after) = rpc.finalized_accounts(
            &[shard_mint, structured_custody, position, admission],
            sent.slot,
        )?;
        if retiring {
            if after.iter().any(Option::is_some) {
                return Err(Error::new(
                    "Structured coordinate retirement left a live resource",
                ));
            }
            let credit = rpc
                .account(rent_credit)?
                .ok_or_else(|| Error::new("Structured retirement RentCredit vanished"))?;
            if credit.lamports != credit_after {
                return Err(Error::new(
                    "Structured coordinate retirement rent credit differs",
                ));
            }
            accepted.push(json!({"action": "RetireCoordinate", "outcome": outcome, "slot": sent.slot, "rentCreditBefore": rent_credit_account.lamports, "rentCreditAfter": credit.lamports, "closed": ([shard_mint, structured_custody, position, admission].map(|key| key.to_string()))}));
            continue;
        }
        let live = after
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Error::new("Structured coordinate activation omitted a poststate"))?;
        if live[0].owner != Pubkey::new_from_array(descriptor.token_program())
            || live[0].data.len() != TOKEN_2022_CLOSEABLE_MINT_BYTES_V2
            || live[1].owner != Pubkey::new_from_array(descriptor.token_program())
            || live[1].data.len() != ACCOUNT_BYTES
            || live[2].owner != claims
            || live[2].data.len() != position_width
            || live[3].owner != claims
            || live[3].data.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2
        {
            return Err(Error::new(
                "Structured coordinate activation poststate differs from canonical resources",
            ));
        }
        accepted.push(json!({
            "outcome": outcome,
            "coefficient": row.coefficient,
            "slot": sent.slot,
            "shardMint": shard_mint.to_string(),
            "structuredCustody": structured_custody.to_string(),
            "claimsCustodyPosition": position.to_string(),
            "positionAdmission": admission.to_string(),
            "seal": fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3].to_string(),
            "routingTables": tables.iter().map(|table| table.key.to_string()).collect::<Vec<_>>(),
        }));
    }
    Ok(accepted)
}

/// Admit the actual collateral owner and issue two backed complete sets through
/// the shipped wallet exteriors. Two is this campaign's trade quantity, not a
/// protocol width or supply limit; every atom is debited by the canonical Split.
fn prepare_structured_native_position_v1(
    arguments: &ArgumentsV1,
    rpc: &mut Rpc,
    payer: &Keypair,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    market: Pubkey,
    minimum_slot: u64,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    let admission_path = arguments.output.with_extension("holder-admission.json");
    let split_path = arguments.output.with_extension("holder-split.json");
    let mut seen = std::collections::BTreeSet::new();
    let admission = loop {
        if admission_path.exists() {
            let bytes = std::fs::read(&admission_path)?;
            let document: serde_json::Value = serde_json::from_slice(&bytes)?;
            if document.get("phase").and_then(serde_json::Value::as_str) == Some("finalized") {
                break document;
            }
            if !seen.insert(sha256_hex(&bytes)) {
                return Err(Error::new(
                    "Structured holder admission journal did not advance",
                ));
            }
        }
        crate::user_position_admission::run_owned_loopback(vec![
            "--rpc-url".into(),
            arguments.rpc_url.clone(),
            "--plan".into(),
            arguments.plan.display().to_string(),
            "--campaign-evidence".into(),
            arguments.campaign_report.display().to_string(),
            "--position-owner".into(),
            payer.pubkey().to_string(),
            "--position-owner-keypair".into(),
            arguments.payer_keypair.display().to_string(),
            "--fee-payer".into(),
            payer.pubkey().to_string(),
            "--fee-payer-keypair".into(),
            arguments.payer_keypair.display().to_string(),
            "--minimum-finalized-slot".into(),
            minimum_slot.to_string(),
            "--output".into(),
            admission_path.display().to_string(),
            "--execute".into(),
        ])?;
    };
    let report_key = |label: &str| {
        evidence
            .accounts
            .get(label)
            .map(|account| account.address.clone())
            .ok_or_else(|| Error::new(format!("Structured holder report omitted {label}")))
    };
    if !split_path.exists() {
        crate::claims_conservation::run_split_owned_loopback_v1(vec![
            "--rpc-url".into(),
            arguments.rpc_url.clone(),
            "--plan".into(),
            arguments.plan.display().to_string(),
            "--market".into(),
            market.to_string(),
            "--owner".into(),
            payer.pubkey().to_string(),
            "--collateral-account".into(),
            report_key("collateral_wallet")?,
            "--linked-basis-record".into(),
            report_key("linked_liability_basis_record")?,
            "--quantity".into(),
            "2".into(),
            "--evidence".into(),
            split_path.display().to_string(),
            "--owner-keypair".into(),
            arguments.payer_keypair.display().to_string(),
            "--execute".into(),
        ])?;
    }
    let split: serde_json::Value = serde_json::from_slice(&std::fs::read(&split_path)?)?;
    if split.get("market").and_then(serde_json::Value::as_str) != Some(market.to_string().as_str())
        || split.get("owner").and_then(serde_json::Value::as_str)
            != Some(payer.pubkey().to_string().as_str())
        || split.get("landed").is_none_or(serde_json::Value::is_null)
    {
        return Err(Error::new(
            "Structured holder Split evidence does not name its real owner and Market",
        ));
    }
    for (label, document) in [
        ("Structured holder admission", &admission),
        ("Structured holder complete-set issuance", &split),
    ] {
        harvest_structured_driver_signatures_v1(rpc, label, document, transactions)?;
    }
    Ok(json!({"admission": admission, "split": split}))
}

/// Driver JSON only suggests signatures; finalized signed packets author every
/// transaction added to the campaign evidence.
pub(crate) fn harvest_structured_driver_signatures_v1(
    rpc: &mut Rpc,
    label: &str,
    document: &serde_json::Value,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    match document {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if (key == "signature" || key.ends_with("Signature"))
                    && let Some(signature) = value.as_str()
                {
                    let signature: solana_sdk::signature::Signature =
                        signature.parse().map_err(|error| {
                            Error::new(format!("{label} reported an invalid signature: {error}"))
                        })?;
                    if !transactions
                        .iter()
                        .any(|row| row.signature == signature.to_string())
                    {
                        let finalized = rpc
                            .finalized_signed_packet(label, signature, false)?
                            .ok_or_else(|| {
                                Error::new(format!(
                                    "{label} reported a signature absent from finalized history"
                                ))
                            })?;
                        transactions.push(finalized.evidence);
                    }
                }
                harvest_structured_driver_signatures_v1(rpc, label, value, transactions)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                harvest_structured_driver_signatures_v1(rpc, label, value, transactions)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Retire a zero-supply representation after genuine Core/Claims terminal work.
/// All resources and rent credits are read from finalized accounts; this
/// continuation never manufactures a terminal Market or a zero token supply.
#[allow(clippy::too_many_arguments)]
fn complete_structured_terminal_v1(
    arguments: &ArgumentsV1,
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    market_input: &[u8],
    market: Pubkey,
    root: Pubkey,
    selected_release: &dclutch_operator::structured_selected_release_v1::StructuredSelectedReleaseV1,
    descriptor_pair: SelectedActivationRecordPairV1,
    descriptor_body: &[u8],
    descriptor: RepresentationDescriptorV2<'_>,
    exposure_body: &[u8],
    exposure: CompositionExposureBundleV3<'_>,
    hot_outer: CheckedRationalLifecycleHotOuterV3,
    fixed: &mut [Pubkey],
    terminal: StructuredTerminalAccountsV1,
    terminal_driver: &mut dyn StructuredTerminalDriverV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    use crate::structured_representation_campaign::{
        StructuredRepresentationCampaignInputV1, StructuredRepresentationTerminalInputV1,
        run_structured_representation_terminal_v1,
    };
    use dclutch_operator::wallet_terminal_input::associated_token_account_v1;
    let registry = pubkey(&plan.registry.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let core = CoreState::decode(
        &rpc.account(market)?
            .ok_or_else(|| Error::new("Structured terminal Market absent"))?
            .data,
    )
    .map_err(|error| Error::new(format!("Structured terminal Market: {error:?}")))?;
    if core.phase != Phase::Terminal {
        return Err(Error::new(
            "Structured terminal driver did not produce Core Terminal",
        ));
    }
    let realm = pair_from_digest_v1(
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
        core.identity.realm_id.to_bytes(),
    )?;
    require_live_structured_record_v1(rpc, registry, realm, "terminal Realm")?;
    let realm_body = rpc
        .account(realm.raw)?
        .ok_or_else(|| Error::new("Structured terminal Realm absent"))?
        .data;
    let exposure_pair = selected_pair_v1(
        registry,
        dclutch_claims::composition::COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        exposure_body,
    )?;
    let mut terminal_actions = Vec::new();
    for outcome in 0..descriptor.outcome_count() {
        if descriptor
            .coefficient(outcome)
            .map_err(|error| Error::new(format!("Structured terminal coefficient: {error:?}")))?
            == 0
        {
            continue;
        }
        let shard = Pubkey::find_program_address(
            &[
                RATIONAL_SHARD_MINT_SEED_V2,
                &descriptor.descriptor_id(),
                &outcome.to_le_bytes(),
            ],
            &claims,
        )
        .0;
        let actor_shard = associated_token_account_v1(
            payer.pubkey(),
            shard,
            Pubkey::new_from_array(descriptor.token_program()),
        );
        let minimum = transactions
            .last()
            .map_or(0, |transaction| transaction.slot);
        let (slot, accounts) = rpc.finalized_accounts(&[actor_shard], minimum)?;
        let token_account = accounts
            .first()
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new("Structured terminal actor shard absent"))?;
        let token = dclutch_custody::token_svm::TokenAccount::parse(&token_account.data)
            .map_err(|error| Error::new(format!("Structured terminal actor shard: {error:?}")))?;
        if token_account.owner.to_bytes() != descriptor.token_program()
            || token.mint != shard.to_bytes()
            || token.owner != payer.pubkey().to_bytes()
        {
            return Err(Error::new(
                "Structured terminal actor shard identity differs",
            ));
        }
        if token.amount == 0 {
            continue;
        }
        let action = run_structured_representation_terminal_v1(
            StructuredRepresentationTerminalInputV1 {
                common: StructuredRepresentationCampaignInputV1 {
                    rpc,
                    fee_payer: payer,
                    actor: payer,
                    plan,
                    campaign: evidence,
                    selected_release,
                    market,
                    root,
                    representation_descriptor: descriptor_pair,
                    representation_descriptor_body: descriptor_body,
                    composition_exposure: exposure_pair,
                    composition_exposure_body: exposure_body,
                    hot_fixed: fixed,
                    lifecycle_hot_outer: hot_outer,
                    minimum_finalized_slot: slot,
                    resume_checkpoints: &[],
                },
                outcome,
                quantity: token.amount,
                realm,
                realm_body: &realm_body,
                terminal_certificate: terminal.terminal_certificate,
                custody_replay: terminal.custody_replay,
                hoard: terminal.hoard,
            },
            transactions,
        )?;
        terminal_actions.push(action);
        write_json(
            &arguments.output.with_extension("terminal-actions.json"),
            &json!({
                "schema": "dclutch-structured-terminal-actions-v1", "market": market.to_string(),
                "actions": terminal_actions, "transactions": transactions,
            }),
        )?;
    }
    // Reclaim and Complete authenticate Core in Terminal phase. Finish that
    // canonical provider lifecycle before BeginRetiring changes the phase.
    let source_completion =
        terminal_driver.finish_source_before_retirement(rpc, payer, market, transactions)?;
    write_structured_progress_v1(arguments, "provider-complete", transactions)?;
    // The existing semantic owner authenticates the entire aggregate as zero
    // before producing Core's transition. No fixture stage or account write.
    let retiring = crate::terminal_sequence::plan_core_begin_retiring_from_chain_v1(
        rpc,
        plan,
        evidence,
        market,
        &[],
    )?;
    let sent = rpc.send(
        "Structured Core BeginRetiring",
        &[retiring.mutation.instruction],
        payer,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured Core BeginRetiring refused: {error}"
        )));
    }
    transactions.push(sent.clone());
    for expected in &retiring.mutation.expected_accounts {
        let (_, accounts) = rpc.finalized_accounts(&[expected.key], sent.slot)?;
        let actual = accounts
            .first()
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new("Structured Core retiring poststate absent"))?;
        if actual.owner != expected.owner
            || actual.lamports != expected.lamports
            || actual.executable != expected.executable
            || actual.data != expected.data
        {
            return Err(Error::new(
                "Structured Core retiring exact poststate differs",
            ));
        }
    }
    let mut retired = retire_structured_representation_v1(
        rpc,
        payer,
        plan,
        evidence,
        market_input,
        market,
        root,
        descriptor_pair,
        descriptor,
        exposure,
        hot_outer,
        fixed,
        transactions,
    )?;
    retired["actorRentReclamation"] =
        reclaim_structured_actor_resources_v1(rpc, payer, claims, descriptor, transactions)?;
    retired["rootClose"] = crate::structured_root_close::close_structured_capability_root_v1(
        rpc,
        payer,
        plan,
        evidence,
        selected_release,
        market,
        root,
        transactions,
    )?;
    retired["sourceCompletion"] = source_completion;
    retired["terminalActions"] = serde_json::to_value(terminal_actions)?;
    write_structured_progress_v1(arguments, "Structured retirement finalized", transactions)?;
    Ok(retired)
}

fn reclaim_structured_actor_resources_v1(
    rpc: &mut Rpc,
    actor: &Keypair,
    claims: Pubkey,
    descriptor: RepresentationDescriptorV2<'_>,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    use dclutch_claims::rational::{
        RATIONAL_REPLAY_SEED_V2, RationalReplayCloseRequestV1, RationalReplayV2,
    };
    use dclutch_operator::wallet_terminal_input::associated_token_account_v1;
    let token_program = Pubkey::new_from_array(descriptor.token_program());
    let mut mints = vec![Pubkey::new_from_array(descriptor.receipt_mint())];
    for outcome in 0..descriptor.outcome_count() {
        if descriptor
            .coefficient(outcome)
            .map_err(|error| Error::new(format!("Structured actor coefficient: {error:?}")))?
            != 0
        {
            mints.push(
                Pubkey::find_program_address(
                    &[
                        RATIONAL_SHARD_MINT_SEED_V2,
                        &descriptor.descriptor_id(),
                        &outcome.to_le_bytes(),
                    ],
                    &claims,
                )
                .0,
            );
        }
    }
    let replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            &descriptor.descriptor_id(),
            actor.pubkey().as_ref(),
        ],
        &claims,
    )
    .0;
    let token_accounts = mints
        .iter()
        .map(|mint| associated_token_account_v1(actor.pubkey(), *mint, token_program))
        .collect::<Vec<_>>();
    let mut keys = mints.clone();
    keys.extend_from_slice(&token_accounts);
    keys.extend([replay, actor.pubkey()]);
    let minimum = transactions
        .last()
        .map_or(0, |transaction| transaction.slot);
    let (slot, accounts) = rpc.finalized_accounts(&keys, minimum)?;
    if accounts[..mints.len()].iter().any(Option::is_some) {
        return Err(Error::new(
            "Structured actor rent reclamation requires retired Mints",
        ));
    }
    let mut reclaimed = 0_u64;
    let mut instructions = Vec::new();
    let mut closed = Vec::new();
    for (index, key) in token_accounts.iter().enumerate() {
        let Some(account) = accounts[mints.len() + index].as_ref() else {
            continue;
        };
        let token = dclutch_custody::token_svm::TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("Structured empty actor ATA: {error:?}")))?;
        if account.owner != token_program
            || token.owner != actor.pubkey().to_bytes()
            || token.mint != mints[index].to_bytes()
            || token.amount != 0
        {
            return Err(Error::new("Structured actor ATA is not owned and empty"));
        }
        let spec = dclutch_custody::token_svm::close_account(
            token_program.to_bytes(),
            key.to_bytes(),
            actor.pubkey().to_bytes(),
            actor.pubkey().to_bytes(),
        )
        .map_err(|error| Error::new(format!("Structured actor token close: {error:?}")))?;
        instructions.push(solana_sdk::instruction::Instruction {
            program_id: Pubkey::new_from_array(*spec.program_id()),
            accounts: spec
                .accounts()
                .iter()
                .map(|meta| solana_sdk::instruction::AccountMeta {
                    pubkey: Pubkey::new_from_array(*meta.address()),
                    is_signer: meta.is_signer(),
                    is_writable: meta.is_writable(),
                })
                .collect(),
            data: spec.data().to_vec(),
        });
        reclaimed = reclaimed
            .checked_add(account.lamports)
            .ok_or_else(|| Error::new("Structured actor rent sum overflow"))?;
        closed.push(*key);
    }
    if let Some(account) = accounts[keys.len() - 2].as_ref() {
        if account.owner != claims {
            return Err(Error::new("Structured replay owner differs"));
        }
        RationalReplayV2::decode(&account.data)
            .and_then(|replay| {
                replay.authenticate(descriptor.descriptor_id(), actor.pubkey().to_bytes())
            })
            .map_err(|error| {
                Error::new(format!("Structured replay close authentication: {error:?}"))
            })?;
        instructions.push(solana_sdk::instruction::Instruction {
            program_id: claims,
            accounts: vec![
                AccountMeta::new(actor.pubkey(), true),
                AccountMeta::new(replay, false),
            ],
            data: RationalReplayCloseRequestV1::new(
                descriptor.descriptor_id(),
                actor.pubkey().to_bytes(),
            )
            .map_err(|error| Error::new(format!("Structured replay close: {error:?}")))?
            .to_bytes()
            .to_vec(),
        });
        reclaimed = reclaimed
            .checked_add(account.lamports)
            .ok_or_else(|| Error::new("Structured replay rent sum overflow"))?;
        closed.push(replay);
    }
    if instructions.is_empty() {
        return Ok(json!({"slot": slot, "alreadyAbsent": true}));
    }
    let actor_before = accounts
        .last()
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("Structured actor absent"))?
        .lamports;
    let sent = rpc.send(
        "reclaim Structured actor empty ATAs and replay",
        &instructions,
        actor,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured actor close refused: {error}"
        )));
    }
    transactions.push(sent.clone());
    let mut after_keys = closed.clone();
    after_keys.push(actor.pubkey());
    let (_, after) = rpc.finalized_accounts(&after_keys, sent.slot)?;
    let fee = sent
        .fee_lamports
        .ok_or_else(|| Error::new("Structured actor close fee absent"))?;
    let expected = actor_before
        .checked_add(reclaimed)
        .and_then(|value| value.checked_sub(fee))
        .ok_or_else(|| Error::new("Structured actor rent balance overflow"))?;
    if after[..closed.len()].iter().any(Option::is_some)
        || after
            .last()
            .and_then(Option::as_ref)
            .map(|account| account.lamports)
            != Some(expected)
    {
        return Err(Error::new(
            "Structured actor closure or exact net rent differs",
        ));
    }
    Ok(
        json!({"slot": sent.slot, "closed": closed.iter().map(ToString::to_string).collect::<Vec<_>>(), "rentReclaimed": reclaimed, "fee": fee, "actorLamportsAfter": expected}),
    )
}

fn retire_structured_representation_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
    market_input: &[u8],
    market: Pubkey,
    root: Pubkey,
    descriptor_record: SelectedActivationRecordPairV1,
    descriptor: RepresentationDescriptorV2<'_>,
    exposure: CompositionExposureBundleV3<'_>,
    hot_outer: CheckedRationalLifecycleHotOuterV3,
    fixed: &mut [Pubkey],
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<serde_json::Value> {
    use dclutch_claims::rational_lifecycle::{
        compact_hot_v4::RationalLifecycleCompactHotRequestV4,
        hot_v6::STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1,
    };
    use dclutch_custody::token_svm::{MINT_BYTES, Mint, TOKEN_2022_CLOSEABLE_MINT_BYTES_V2};
    use dclutch_registry::{ActivatedExecutionReleaseSetV1, release_set::ExecutionRoleV1};
    let registry = pubkey(&plan.registry.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let aggregate = pubkey(
        &evidence
            .accounts
            .get("claims_aggregate")
            .ok_or_else(|| Error::new("Structured retirement omitted aggregate"))?
            .address,
    )?;
    let market_before = rpc
        .account(market)?
        .ok_or_else(|| Error::new("Structured retirement Market vanished"))?;
    let core_state = CoreState::decode(&market_before.data)
        .map_err(|error| Error::new(format!("Structured retirement Market: {error:?}")))?;
    if core_state.phase != Phase::Retiring {
        return Err(Error::new(
            "Structured retirement requires genuine Core Retiring state",
        ));
    }
    let coordinate_artifacts = structured_activation::hydrate_selected_retirement_artifacts_v1(
        rpc,
        registry,
        market_input,
        0,
        LifecycleActionV2::RetireCoordinate,
    )?;
    let coordinates = activate_structured_coordinates_v1(
        rpc,
        payer,
        plan,
        evidence,
        &coordinate_artifacts,
        LifecycleActionV2::RetireCoordinate,
        market,
        root,
        descriptor_record,
        descriptor,
        exposure,
        hot_outer,
        core_state.identity.realm_id.to_bytes(),
        fixed,
        transactions,
    )?;
    let artifacts = structured_activation::hydrate_selected_retirement_artifacts_v1(
        rpc,
        registry,
        market_input,
        coordinate_artifacts.slot,
        LifecycleActionV2::RetireReceipt,
    )?;
    let rent_credit = Pubkey::new_from_array(core_state.rent_beneficiary.to_bytes());
    let receipt_mint = Pubkey::new_from_array(descriptor.receipt_mint());
    let mut vacancy_keys = Vec::new();
    for outcome in 0..descriptor.outcome_count() {
        if descriptor
            .coefficient(outcome)
            .map_err(|error| Error::new(format!("Structured retirement coefficient: {error:?}")))?
            == 0
        {
            continue;
        }
        let outcome_bytes = outcome.to_le_bytes();
        let shard = Pubkey::find_program_address(
            &[
                RATIONAL_SHARD_MINT_SEED_V2,
                &descriptor.descriptor_id(),
                &outcome_bytes,
            ],
            &claims,
        )
        .0;
        let custody = Pubkey::find_program_address(
            &[
                RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                &descriptor.descriptor_id(),
                &outcome_bytes,
            ],
            &claims,
        )
        .0;
        let owner_seeds =
            ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor.descriptor_id(), outcome)
                .map_err(|error| Error::new(format!("Structured retirement owner: {error:?}")))?;
        let owner = Pubkey::find_program_address(&owner_seeds.as_slices(), &claims).0;
        let position_seeds = ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
            .map_err(|error| {
            Error::new(format!("Structured retirement Position: {error:?}"))
        })?;
        let admission_seeds =
            ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes()).map_err(
                |error| Error::new(format!("Structured retirement admission: {error:?}")),
            )?;
        vacancy_keys.extend(
            [
                shard,
                custody,
                owner,
                Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0,
                Pubkey::find_program_address(&admission_seeds.as_slices(), &claims).0,
            ]
            .map(|key| key.to_bytes()),
        );
    }
    let mut addresses = vec![
        market,
        aggregate,
        rent_credit,
        receipt_mint,
        root,
        solana_sdk_ids::sysvar::rent::ID,
        pubkey(&plan.activation)?,
    ];
    addresses.extend(vacancy_keys.iter().copied().map(Pubkey::new_from_array));
    let (slot, accounts) = rpc.finalized_accounts(&addresses, artifacts.slot)?;
    let required = |index: usize| {
        accounts.get(index).cloned().flatten().ok_or_else(|| {
            Error::new(format!(
                "Structured retirement snapshot omitted account {index}"
            ))
        })
    };
    for (index, resource) in accounts.iter().skip(7).enumerate() {
        // Claims custody owner is an inert authority name; the other four
        // slots are resources whose entire account must be absent.
        if index % 5 != 2 && resource.is_some() {
            return Err(Error::new(
                "Structured retirement support still owns a resource",
            ));
        }
    }
    let market_account = required(0)?;
    let aggregate_account = required(1)?;
    let credit_account = required(2)?;
    let mint_account = required(3)?;
    let root_account = required(4)?;
    let rent_account = required(5)?;
    let activation = required(6)?;
    let rent_observed = dclutch_operator::ObservedAccount {
        observation: dclutch_operator::Observation {
            slot,
            unix_timestamp: rpc.block_time(slot)?,
            finality: dclutch_operator::Finality::Finalized,
        },
        key: solana_sdk_ids::sysvar::rent::ID,
        owner: rent_account.owner,
        lamports: rent_account.lamports,
        executable: rent_account.executable,
        data: rent_account.data,
    };
    let rent = decode_rent(&rent_observed)
        .map_err(|error| Error::new(format!("Structured retirement Rent: {error:?}")))?;
    let supply = Mint::parse(
        mint_account
            .data
            .get(..MINT_BYTES)
            .ok_or_else(|| Error::new("Structured retiring receipt truncated"))?,
    )
    .map_err(|error| Error::new(format!("Structured retiring receipt: {error:?}")))?
    .supply;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured retirement aggregate: {error:?}")))?;
    let credit_after = credit_account
        .lamports
        .checked_add(mint_account.lamports)
        .ok_or_else(|| Error::new("Structured receipt rent credit overflows"))?;
    let header = LifecycleHeaderV2 {
        action: LifecycleActionV2::RetireReceipt,
        release_set: core_state.identity.selected_release_set.to_bytes(),
        market: market.to_bytes(),
        graph_id: descriptor.graph_id(),
        descriptor_id: descriptor.descriptor_id(),
        parent_context: [0; 32],
        representation_authority: descriptor.representation_authority(),
        receipt_mint: descriptor.receipt_mint(),
        token_program: descriptor.token_program(),
        rent_credit: rent_credit.to_bytes(),
        rent_program: rent_program.to_bytes(),
        generation: core_state.identity.generation,
        expected_claims_market_revision: aggregate_view.revision,
        observed_receipt_lamports: mint_account.lamports,
        receipt_rent_principal: rent.minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2),
        expected_receipt_supply: supply,
        outcome_count: descriptor.outcome_count(),
        coordinate_count: 0,
        rent_credit_before: credit_account.lamports,
        rent_credit_after: credit_after,
    };
    let mut compact_bytes = [0; LIFECYCLE_HEADER_BYTES_V2];
    let compact =
        RationalLifecycleCompactHotRequestV4::from_header_into(header, &mut compact_bytes)
            .map_err(|error| {
                Error::new(format!("Structured retirement compact header: {error:?}"))
            })?;
    let width =
        LIFECYCLE_HEADER_BYTES_V2 + (vacancy_keys.len() / 5) * LIFECYCLE_COORDINATE_BYTES_V2;
    let mut scratch = vec![0; width];
    let mut child_bytes = vec![0; width];
    compact
        .specialize_child_into(
            hash(compact.as_bytes()).to_bytes(),
            descriptor,
            &vacancy_keys,
            &mut scratch,
            &mut child_bytes,
        )
        .map_err(|error| Error::new(format!("Structured retirement support: {error:?}")))?;
    let child = crate::structured_physical_frame::retire_receipt_claims_instruction_v1(
        ActivateReceiptPhysicalInputsV1 {
            trading,
            trading_programdata: pubkey(&plan.trading.programdata_id)?,
            claims,
            claims_programdata: pubkey(&plan.claims.programdata_id)?,
            registry,
            activation_cache: pubkey(&plan.activation)?,
            descriptor_raw: descriptor_record.raw,
            descriptor_staging: descriptor_record.staging,
            representation_authority: Pubkey::new_from_array(descriptor.representation_authority()),
            receipt_mint,
            rent_credit,
            rent_program,
            claims_market: aggregate,
            core_market: market,
            core,
            core_programdata: pubkey(&plan.core.programdata_id)?,
        },
        &child_bytes,
    )?;
    for (index, record) in [
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3,
    ]
    .into_iter()
    .zip(&artifacts.bundle_records)
    {
        fixed[index] = record.raw;
        fixed[index + 1] = record.staging;
    }
    let activated = ActivatedExecutionReleaseSetV1::decode(&activation.data)
        .map_err(|error| Error::new(format!("Structured retirement activation: {error:?}")))?;
    let trading_release = activated.role(ExecutionRoleV1::Trading).release();
    let capability_descriptor = artifacts.bundle_records[0].raw;
    let descriptor_digest = hash(&artifacts.bundle.descriptor).to_bytes();
    if fixed[HOT_DESCRIPTOR_RAW_ACCOUNT_V3] != capability_descriptor {
        return Err(Error::new(
            "Structured retirement selected descriptor differs",
        ));
    }
    let seal_key = dclutch_vm::capability_seal::CapabilitySealKeyV1::new(
        dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
        descriptor_digest,
        STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1,
        trading_release.semantic_release_id().to_bytes(),
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured retirement seal: {error:?}")))?;
    fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3] =
        Pubkey::find_program_address(&seal_key.seeds().as_slices(), &trading).0;
    let frame = AuthenticatedRationalLifecycleHotFrameV1 {
        fixed_accounts: fixed
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index == HOT_ROOT_ACCOUNT_V3 {
                    AccountMeta::new(*key, false)
                } else {
                    AccountMeta::new_readonly(*key, false)
                }
            })
            .collect(),
        strategy_accounts: Vec::new(),
        root_data: root_account.data,
        market_data: market_account.data,
        release_set: core_state.identity.selected_release_set.to_bytes(),
        market,
        generation: core_state.identity.generation,
        finalized_slot: slot,
        hot_outer,
    };
    let hot = structured_activation::build_selected_retirement_instruction_v1(
        &artifacts,
        &frame.state()?,
        &child,
        descriptor,
        core_state.identity.realm_id.to_bytes(),
    )?;
    let seal = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
        trading_program: trading,
        registry_program: registry,
        trading_semantic_release: trading_release.semantic_release_id().to_bytes(),
        descriptor_digest,
        action: STRUCTURED_RETIRE_RECEIPT_SELECTOR_V1,
        fixed_frame: fixed,
        payer: payer.pubkey(),
    })
    .map_err(|error| Error::new(format!("Structured retirement seal instruction: {error:?}")))?
    .instruction;
    let mut routing = std::collections::BTreeSet::new();
    for instruction in [&seal, &hot.instruction] {
        routing.insert(instruction.program_id);
        routing.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    }
    let (observation, tables) = crate::market::publish_routing_table_over_v1(
        rpc,
        payer,
        "STRUCTURED-RETIRE-RECEIPT",
        &routing.into_iter().collect::<Vec<_>>(),
        transactions,
    )?;
    for (label, instruction) in [
        ("seal Structured receipt retirement", seal),
        ("retire Structured receipt", hot.instruction),
    ] {
        let sent = rpc.send_v0_on_heap(
            label,
            &[instruction],
            payer,
            observation,
            &tables,
            DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        )?;
        if let Some(error) = sent.error.as_ref() {
            return Err(Error::new(format!("{label} refused: {error}")));
        }
        transactions.push(sent);
    }
    let (after_slot, after) = rpc.finalized_accounts(&[receipt_mint, rent_credit], slot)?;
    let after_credit = after[1]
        .as_ref()
        .ok_or_else(|| Error::new("Structured receipt retirement removed RentCredit"))?;
    if after[0].is_some() || after_credit.lamports != credit_after {
        return Err(Error::new(
            "Structured receipt retirement poststates differ",
        ));
    }
    Ok(
        json!({"coordinates": coordinates, "receipt": {"slot": after_slot, "mint": receipt_mint.to_string(), "supplyBefore": supply, "closed": true, "rentCreditBefore": credit_account.lamports, "rentCreditAfter": after_credit.lamports}}),
    )
}

fn derive_selected_lifecycle_parent_with_coordinates_v6(
    mut header: LifecycleHeaderV2,
    coordinate_bytes: &[u8],
) -> Result<LifecycleHeaderV2> {
    header.parent_context = [1; 32];
    let provisional = LifecycleRequestV2::new(header, coordinate_bytes).map_err(|error| {
        Error::new(format!(
            "Structured coordinate provisional lifecycle: {error:?}"
        ))
    })?;
    let mut family_bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2 + coordinate_bytes.len()];
    let family = RationalLifecycleHotRequestV6::from_child_into(provisional, &mut family_bytes)
        .map_err(|error| Error::new(format!("Structured coordinate V6 family: {error:?}")))?;
    header.parent_context = hash(family.as_bytes()).to_bytes();
    Ok(header)
}

/// Bind the Claims child to the exact V6 family bytes that Hot will carry.
///
/// The family form has a zero parent context by design; the child carries the
/// digest of that form.  Start from a nonzero provisional context because the
/// Claims lifecycle decoder refuses an unbound child before Hot can derive it.
fn derive_selected_lifecycle_parent_v6(mut header: LifecycleHeaderV2) -> Result<LifecycleHeaderV2> {
    header.parent_context = [1; 32];
    let provisional = LifecycleRequestV2::new(header, &[]).map_err(|error| {
        Error::new(format!(
            "Structured receipt provisional lifecycle: {error:?}"
        ))
    })?;
    let mut family_bytes = vec![0; LIFECYCLE_HEADER_BYTES_V2];
    let family = RationalLifecycleHotRequestV6::from_child_into(provisional, &mut family_bytes)
        .map_err(|error| Error::new(format!("Structured receipt V6 family: {error:?}")))?;
    header.parent_context = hash(family.as_bytes()).to_bytes();
    Ok(header)
}

/// Preserve the Claims child caller-authority signer fact for Hot's CPI.
///
/// The physical child builder emits transaction-shaped metadata because the
/// PDA is not a wallet signer. The selected Hot operator consumes the child
/// frame's CPI semantics first and strips this bit while packing the outer
/// transaction account list.
fn mark_claims_child_caller_signer_v1(
    claims_child: &mut solana_sdk::instruction::Instruction,
) -> Result<()> {
    claims_child
        .accounts
        .first_mut()
        .ok_or_else(|| Error::new("Structured receipt Claims child omitted caller authority"))?
        .is_signer = true;
    Ok(())
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

/// Authenticate the live raw owner of a report-backed Product graph record.
///
/// The report is evidence from the founding run, not a permission to hand a
/// Hot instruction a PDA whose account has since vanished. Trading's common
/// reader maps a missing, wrong-owner, or wrong-body raw record to its coarse
/// `Content` refusal. Resolve that accusation here while the producer still
/// knows the semantic row and canonical Registry coordinate.
fn require_live_structured_record_v1(
    rpc: &mut crate::rpc::Rpc,
    registry: Pubkey,
    pair: SelectedActivationRecordPairV1,
    label: &str,
) -> Result<()> {
    let observed = rpc.account(pair.raw)?;
    validate_live_structured_record_v1(registry, pair, label, observed.as_ref())
}

fn validate_live_structured_record_v1(
    registry: Pubkey,
    pair: SelectedActivationRecordPairV1,
    label: &str,
    observed: Option<&crate::rpc::RpcAccount>,
) -> Result<()> {
    let observed = observed.ok_or_else(|| {
        Error::new(format!(
            "Structured receipt {label} raw record is absent at canonical PDA {}",
            pair.raw
        ))
    })?;
    if observed.owner != registry {
        return Err(Error::new(format!(
            "Structured receipt {label} raw record owner {} differs from Registry {}",
            observed.owner, registry
        )));
    }
    let digest: [u8; 32] = sha2::Sha256::digest(&observed.data).into();
    if digest != pair.content {
        return Err(Error::new(format!(
            "Structured receipt {label} raw record body digest differs at canonical PDA {}",
            pair.raw
        )));
    }
    Ok(())
}

fn preflight_structured_product_graph_v1(
    rpc: &mut crate::rpc::Rpc,
    registry: Pubkey,
    evidence: &crate::campaign::CampaignTerminalEvidenceV1,
) -> Result<()> {
    let mut live_body = |label: &str, schema: [u8; 32]| -> Result<Vec<u8>> {
        let row = evidence
            .accounts
            .get(label)
            .ok_or_else(|| Error::new(format!("Structured receipt report omitted {label}")))?;
        let pair = pair_from_digest_v1(registry, schema, crate::plan::hex32(&row.data_sha256)?)?;
        require_live_structured_record_v1(rpc, registry, pair, label)?;
        rpc.account(pair.raw)?
            .map(|account| account.data)
            .ok_or_else(|| {
                Error::new(format!(
                    "Structured receipt {label} vanished during preflight"
                ))
            })
    };
    let product = live_body(
        "product_record",
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let domain = live_body(
        "result_domain_record",
        dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = live_body(
        "portfolio_record",
        dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
    )?;
    ProductRecordV2::decode(&product)
        .map_err(|error| Error::new(format!("Structured receipt Product record: {error:?}")))?;
    let domain = ResultDomainV2::decode(&domain)
        .map_err(|error| Error::new(format!("Structured receipt result domain: {error:?}")))?;
    let portfolio = PortfolioV2::decode(&portfolio)
        .map_err(|error| Error::new(format!("Structured receipt Portfolio: {error:?}")))?;
    let outcome_count = domain.outcome_count().map_err(|error| {
        Error::new(format!("Structured receipt result domain width: {error:?}"))
    })?;
    if outcome_count != portfolio.coefficient_count() {
        return Err(Error::new(format!(
            "Structured receipt Product graph outcome width {} differs from Portfolio coefficient count {}",
            outcome_count,
            portfolio.coefficient_count(),
        )));
    }
    Ok(())
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

fn write_structured_progress_v1(
    arguments: &ArgumentsV1,
    stage: &str,
    transactions: &[crate::model::TransactionEvidence],
) -> Result<()> {
    write_json(
        &arguments.output.with_extension("progress.json"),
        &json!({
            "schema": "dclutch-structured-finalized-progress-v1", "stage": stage,
            "transactions": transactions,
        }),
    )
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    use std::io::Write as _;
    let temporary = path.with_extension("json.pending");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(format!("{}\n", serde_json::to_string_pretty(value)?).as_bytes())?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_v6_parent_derives_from_the_zeroed_family_form() {
        use dclutch_claims::{
            rational_lifecycle::hot_v6::RationalLifecycleHotRequestV6,
            rational_lifecycle::{LifecycleActionV2, LifecycleHeaderV2, LifecycleRequestV2},
        };
        use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
        use solana_sdk::hash::hash;

        let header = LifecycleHeaderV2 {
            action: LifecycleActionV2::ActivateReceipt,
            release_set: [1; 32],
            market: [2; 32],
            graph_id: [3; 32],
            descriptor_id: [4; 32],
            parent_context: [0; 32],
            representation_authority: [5; 32],
            receipt_mint: [6; 32],
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: [7; 32],
            rent_program: [8; 32],
            generation: 1,
            expected_claims_market_revision: 1,
            observed_receipt_lamports: 1,
            receipt_rent_principal: 1,
            expected_receipt_supply: 0,
            outcome_count: 2,
            coordinate_count: 0,
            rent_credit_before: 1,
            rent_credit_after: 1,
        };
        assert_eq!(
            LifecycleRequestV2::new(header, &[]),
            Err(dclutch_claims::rational_lifecycle::Error::InvalidIdentity),
            "a child cannot use Hot's zeroed family context"
        );
        let derived =
            super::derive_selected_lifecycle_parent_v6(header).expect("canonical V6 parent digest");
        let child = LifecycleRequestV2::new(derived, &[]).expect("bound Claims child");
        let mut family_bytes =
            vec![0; dclutch_claims::rational_lifecycle::LIFECYCLE_HEADER_BYTES_V2];
        let family = RationalLifecycleHotRequestV6::from_child_into(child, &mut family_bytes)
            .expect("family projection");
        assert_eq!(derived.parent_context, hash(family.as_bytes()).to_bytes());
    }

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

    #[test]
    fn receipt_child_marks_the_cpi_caller_authority_as_signer() {
        use solana_sdk::{
            instruction::{AccountMeta, Instruction},
            pubkey::Pubkey,
        };

        let caller = Pubkey::new_unique();
        let mut child = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![AccountMeta::new_readonly(caller, false)],
            data: Vec::new(),
        };
        super::mark_claims_child_caller_signer_v1(&mut child).expect("caller authority coordinate");
        assert!(child.accounts[0].is_signer);
    }

    #[test]
    fn receipt_record_preflight_names_a_missing_live_raw_account() {
        use sha2::{Digest as _, Sha256};
        use solana_sdk::pubkey::Pubkey;

        let registry = Pubkey::new_unique();
        let body = b"live linked basis body";
        let pair = super::SelectedActivationRecordPairV1 {
            raw: Pubkey::new_unique(),
            staging: Pubkey::new_unique(),
            schema: [1; 32],
            content: Sha256::digest(body).into(),
            bumps: [254, 255],
        };
        let error = super::validate_live_structured_record_v1(registry, pair, "linked_basis", None)
            .expect_err("missing raw record must refuse before Hot construction");
        assert_eq!(
            error.to_string(),
            format!(
                "Structured receipt linked_basis raw record is absent at canonical PDA {}",
                pair.raw
            )
        );
    }

    #[test]
    fn receipt_record_preflight_accepts_registry_owned_matching_raw_account() {
        use sha2::{Digest as _, Sha256};
        use solana_sdk::pubkey::Pubkey;

        let registry = Pubkey::new_unique();
        let body = b"live linked basis body".to_vec();
        let pair = super::SelectedActivationRecordPairV1 {
            raw: Pubkey::new_unique(),
            staging: Pubkey::new_unique(),
            schema: [1; 32],
            content: Sha256::digest(&body).into(),
            bumps: [254, 255],
        };
        let observed = crate::rpc::RpcAccount {
            lamports: 1,
            owner: registry,
            executable: false,
            rent_epoch: 0,
            data: body,
        };
        super::validate_live_structured_record_v1(registry, pair, "linked_basis", Some(&observed))
            .expect("matching Registry-owned raw record is accepted");
    }

    #[test]
    fn receipt_descriptor_publication_refuses_a_same_digest_wrong_schema() {
        let registry = solana_sdk::pubkey::Pubkey::new_unique();
        let bytes = b"canonical derived representation descriptor";
        let expected = super::selected_pair_v1(
            registry,
            dclutch_claims::rational_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            bytes,
        )
        .expect("canonical descriptor pair");
        let observed = crate::runtime::PublishedRecord {
            schema: [7; 32],
            digest: expected.content,
            raw: expected.raw,
            staging: expected.staging,
        };
        let error =
            super::authenticate_receipt_descriptor_publication_v1(registry, bytes, &observed)
                .expect_err(
                    "same digest under another schema must not become a receipt descriptor",
                );
        assert_eq!(
            error.to_string(),
            "Structured receipt publication descriptor schema or digest differs"
        );
    }
}
