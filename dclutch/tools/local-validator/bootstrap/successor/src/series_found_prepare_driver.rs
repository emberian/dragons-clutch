//! Parent-market normalization facts for the real Series Found -> Prepare entrance.
//!
//! A Series Template commits *child* Markets.  The Market that selects the
//! Series capability is a separate parent: it receives the compiled Series
//! publication only after the child Template has been admitted.  This module
//! owns the boundary between those two identities.  In particular, it derives
//! the parent root from the parent Market's final selected manifest, rather
//! than accidentally using the child Market or a root assembled with default
//! Registry bumps.

use dclutch_core_contract::ContentId;
use dclutch_market::{
    capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1, v4::CapabilityProgramV4},
};
use dclutch_registry::{
    record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId},
    release_set::{CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1},
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as _},
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use std::{collections::BTreeMap, fs, path::PathBuf};

use crate::{
    Error, Result,
    market::{
        FutureMarketImmutablePublicationV1, derive_founding_targets, open_market_generation_v1,
        record_identity,
    },
    model::{MarketRunInput, SelectedCapabilityV1, SuccessorPlan},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount},
    runtime::decode_hex,
    selected_capability_activation::{
        SelectedActivationRecordPairV1, SelectedCapabilityActivationInputV1,
        build_selected_capability_activation_plan_v1, execute_selected_capability_activation_v1,
    },
    series_geometry::{
        SeriesPrepareRoleLayoutV1, SeriesPrepareRoleSourceV1, observe_series_prepare_geometry_v1,
    },
};

type SeriesPrepareFixedDataLengthsV1 = [u32;
    dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5
        as usize];

const SERIES_SHADOW_DIAGNOSTIC_SEMANTIC_PREIMAGE_V1: &[u8] =
    b"dclutch/series-shadow-selected-diagnostic/v1";

fn series_shadow_diagnostic_accelerator_semantic_release_v1() -> ContentId {
    ContentId::new(Sha256::digest(SERIES_SHADOW_DIAGNOSTIC_SEMANTIC_PREIMAGE_V1).into())
        .expect("domain-separated Series diagnostic semantic release is nonzero")
}

/// The one composite entrance for a local Series Found -> first Prepare run.
/// Its accelerator capacity is an already-built, hostile-decoded source
/// manifest; an arbitrary executable or textual certificate identity is
/// deliberately not an argument of this command.
pub(crate) const SERIES_FOUND_PREPARE_COMMAND_V1: &str =
    "local-private-validator-series-found-prepare-v1";

#[derive(Debug)]
struct SeriesFoundPrepareArgumentsV1 {
    plan: PathBuf,
    rpc_url: String,
    payer_keypair: PathBuf,
    direct_fee_basis_points: u16,
    direct_fee_recipient: Pubkey,
    shadow_source_manifest: Option<PathBuf>,
    diagnostic_shadow_output: Option<PathBuf>,
    output: Option<PathBuf>,
    execute: bool,
}

fn canonical_regular_v1(path: PathBuf, flag: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be an absolute path")));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!("{flag} must name a regular file")));
    }
    let canonical = fs::canonicalize(&path)?;
    if canonical != path {
        return Err(Error::new(format!("{flag} must be canonical")));
    }
    Ok(path)
}

fn absolute_new_v1(path: PathBuf, flag: &str) -> Result<PathBuf> {
    if !path.is_absolute() || path.exists() || fs::symlink_metadata(&path).is_ok() {
        return Err(Error::new(format!(
            "{flag} must name an absolute path that does not exist"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{flag} omitted its parent")))?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Error::new(format!(
            "{flag} parent must be a regular directory"
        )));
    }
    Ok(fs::canonicalize(parent)?.join(
        path.file_name()
            .ok_or_else(|| Error::new(format!("{flag} omitted its filename")))?,
    ))
}

fn parse_series_found_prepare_arguments_v1(
    arguments: Vec<String>,
) -> Result<SeriesFoundPrepareArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut execute = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--execute" => {
                if execute {
                    return Err(Error::new("Series Found/Prepare repeats --execute"));
                }
                execute = true;
                index += 1;
            }
            flag => {
                let value = arguments.get(index + 1).ok_or_else(|| {
                    Error::new(format!("Series Found/Prepare {flag} needs a value"))
                })?;
                if !matches!(
                    flag,
                    "--plan"
                        | "--rpc-url"
                        | "--payer-keypair"
                        | "--direct-fee-basis-points"
                        | "--direct-fee-recipient"
                        | "--series-shadow-source-manifest"
                        | "--emit-series-shadow-diagnostic"
                        | "--output"
                ) || values.insert(flag.to_owned(), value.to_owned()).is_some()
                {
                    return Err(Error::new(format!(
                        "Series Found/Prepare rejects argument {flag}"
                    )));
                }
                index += 2;
            }
        }
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| Error::new(format!("Series Found/Prepare requires {flag}")))
    };
    let direct_fee_recipient: Pubkey = required("--direct-fee-recipient")?
        .parse()
        .map_err(|_| Error::new("--direct-fee-recipient must be a base58 Pubkey"))?;
    if direct_fee_recipient == Pubkey::default() {
        return Err(Error::new(
            "--direct-fee-recipient must not be the default Pubkey",
        ));
    }
    let shadow_source_manifest = values
        .get("--series-shadow-source-manifest")
        .map(|value| canonical_regular_v1(PathBuf::from(value), "--series-shadow-source-manifest"))
        .transpose()?;
    let diagnostic_shadow_output = values
        .get("--emit-series-shadow-diagnostic")
        .map(|value| absolute_new_v1(PathBuf::from(value), "--emit-series-shadow-diagnostic"))
        .transpose()?;
    let output = values
        .get("--output")
        .map(|value| absolute_new_v1(PathBuf::from(value), "--output"))
        .transpose()?;
    let execute_mode = shadow_source_manifest.is_some()
        && diagnostic_shadow_output.is_none()
        && output.is_some();
    let diagnostic_mode = shadow_source_manifest.is_none()
        && diagnostic_shadow_output.is_some()
        && output.is_none();
    if !execute_mode && !diagnostic_mode {
        return Err(Error::new(
            "Series Found/Prepare requires either --series-shadow-source-manifest with --output, or --emit-series-shadow-diagnostic",
        ));
    }
    Ok(SeriesFoundPrepareArgumentsV1 {
        plan: canonical_regular_v1(PathBuf::from(required("--plan")?), "--plan")?,
        rpc_url: required("--rpc-url")?,
        payer_keypair: canonical_regular_v1(
            PathBuf::from(required("--payer-keypair")?),
            "--payer-keypair",
        )?,
        direct_fee_basis_points: required("--direct-fee-basis-points")?
            .parse()
            .map_err(|_| Error::new("--direct-fee-basis-points must be a decimal u16"))?,
        direct_fee_recipient,
        shadow_source_manifest,
        diagnostic_shadow_output,
        output,
        execute,
    })
}

/// Run the supported local Series entrance through one actual first Prepare.
///
/// The selected Shadow source manifest is decoded before Clock is observed:
/// it owns executor capacity, while the later finalized Template owns the
/// schedule and config bytes. The command never accepts account lists,
/// request banks, or an executable path from its caller.
pub(crate) fn run_series_found_prepare_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_series_found_prepare_arguments_v1(arguments)?;
    if !arguments.execute {
        return Err(Error::new(
            "Series Found/Prepare refuses a dry run: Found, activation, and Prepare are one execution",
        ));
    }
    let source_manifest_bytes = arguments
        .shadow_source_manifest
        .as_ref()
        .map(fs::read)
        .transpose()?;
    let source_manifest = source_manifest_bytes
        .as_deref()
        .map(dclutch_series_shadow_bundle_generator::SeriesShadowSourceManifestV1::decode)
        .transpose()
        .map_err(|error| Error::new(format!("Series Shadow source manifest: {error:?}")))?;
    if source_manifest.is_some_and(|manifest| manifest.occurrence_count() != 2) {
        return Err(Error::new(
            "Series Found/Prepare requires a two-occurrence executor capacity",
        ));
    }

    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&arguments.plan)?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let payer = Keypair::new_from_array(crate::campaign::read_keypair_file(
        &arguments.payer_keypair,
        "Series Found/Prepare payer",
    )?);
    let registry = pubkey(&plan.registry.program_id)?;
    let selected_release = ContentId::new(hex32(&plan.release_set_id)?)
        .map_err(|_| Error::new("Series selected release identity"))?;
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &arguments.plan,
        &arguments.rpc_url,
        registry,
        Some(arguments.direct_fee_basis_points),
        Some(arguments.direct_fee_recipient),
    )?;
    let direct_compiler = direct.compiler();
    let activation_deadline_slot = direct_compiler.activation_deadline_slot;
    let child_market = crate::market::demo_market_input(registry, direct_compiler)?;
    let parent_base = crate::market::demo_market_input_base(
        registry,
        crate::direct_market::authenticated_resolution_release_v1(&plan)?,
    )?;
    let selected_manifest_entry_index = preselection_series_manifest_entry_index_v1(&decode_hex(
        &parent_base.capability_manifest_hex,
    )?)?;
    let funding_ledger_slot_count =
        selected_series_funding_ledger_slot_count_v1(selected_manifest_entry_index)?;
    if source_manifest
        .is_some_and(|manifest| manifest.funding_count() != u32::from(funding_ledger_slot_count))
    {
        return Err(Error::new(
            "Series Shadow executor funding geometry differs from the selected ledger",
        ));
    }

    let mut rpc = Rpc::connect(&arguments.rpc_url)?;
    let mut transactions = Vec::new();
    let child_forge = crate::seed::KeyForge::random();
    let child_collateral = crate::market::create_real_collateral_for_market_v1(
        &mut rpc,
        &payer,
        &child_forge,
        &child_market,
        &mut transactions,
    )?;
    let m0 = crate::market::publish_future_market_immutable_records_v1(
        &mut rpc,
        &plan,
        &child_market,
        child_collateral.mint,
        &payer,
        &mut transactions,
    )?;
    let (founder, scenario) = prepare_local_series_founder_from_market_v1(
        &mut rpc,
        &plan,
        &child_market,
        child_collateral.mint,
        payer.pubkey(),
        payer.pubkey(),
        child_collateral.wallet,
    )?;
    let founder_records =
        publish_series_founder_records_v1(&mut rpc, registry, &payer, &founder, &mut transactions)?;
    let m0_records = series_prepare_records_from_m0_publication_v1(&m0);
    let m0_accounts = observe_series_prepare_m0_accounts_v1(
        &mut rpc,
        &m0,
        &m0_records,
        child_collateral.wallet,
        payer.pubkey(),
        scenario.finalized_slot,
    )?;
    let m0_frame = series_prepare_m0_frame_from_publication_v1(&m0, &m0_records, &m0_accounts);
    let hydration =
        series_prepare_hydration_records_v1(&m0, &m0_records, &founder, &founder_records)?;
    let certificate_program = source_manifest
        .map(|manifest| manifest.generated_bundle().certificate_program)
        .unwrap_or_else(|| {
            let mut hasher = Sha256::new();
            hasher.update(b"dclutch-series-shadow-diagnostic-provisional-certificate-v1");
            hasher.update(series_shadow_diagnostic_accelerator_semantic_release_v1().to_bytes());
            ContentId::new(hasher.finalize().into())
                .expect("domain-separated diagnostic certificate is nonzero")
        });

    let prediction = crate::series_found_prepare_input::predict_series_parent_root_v1(
        &plan,
        &founder,
        selected_release,
        selected_manifest_entry_index,
    )?;
    let provisional = compile_series_prepare_pass_v1(
        &mut rpc,
        &plan,
        &m0,
        &founder,
        &founder_records,
        &payer,
        child_collateral.mint,
        child_collateral.wallet,
        prediction,
        funding_ledger_slot_count,
        selected_release,
        activation_deadline_slot,
        certificate_program,
        m0_frame,
        hydration,
        scenario.finalized_slot,
        &mut transactions,
    )?;
    if let Some(output) = arguments.diagnostic_shadow_output.as_ref() {
        let built = build_series_shadow_diagnostic_preselection_v1(
            2,
            &provisional.geometry.consume_fixed_data_lengths,
            u32::from(funding_ledger_slot_count),
        )?;
        write_series_shadow_diagnostic_v1(output, &built)?;
        return Ok(());
    }
    let source_manifest = source_manifest.expect("argument modes were validated");
    require_series_shadow_capacity_v1(source_manifest, &provisional.geometry)?;

    let parent_market =
        build_series_parent_market_v1(&plan, registry, provisional.selected.clone())?;
    let parent_forge = crate::seed::KeyForge::random();
    let parent_collateral = crate::market::create_real_collateral_for_market_v1(
        &mut rpc,
        &payer,
        &parent_forge,
        &parent_market,
        &mut transactions,
    )?;
    let parent_collateral_mint = parent_collateral.mint;
    let parent_root = derive_series_parent_root_v1(&plan, &parent_market, parent_collateral_mint)?;
    let normalized = crate::series_found_prepare_input::SeriesPredictedParentRootV1 {
        root: parent_root.root,
        data_len:
            dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    };
    let normalized_compiled = compile_series_prepare_pass_v1(
        &mut rpc,
        &plan,
        &m0,
        &founder,
        &founder_records,
        &payer,
        child_collateral.mint,
        child_collateral.wallet,
        normalized,
        funding_ledger_slot_count,
        selected_release,
        activation_deadline_slot,
        certificate_program,
        m0_frame,
        hydration,
        scenario.finalized_slot,
        &mut transactions,
    )?;
    crate::series_found_prepare_campaign::require_series_selection_invariance_v1(
        &provisional,
        &normalized_compiled,
    )?;
    require_series_shadow_capacity_v1(source_manifest, &normalized_compiled.geometry)?;

    let founding = crate::market::execute_found_market_with_existing_collateral_v1(
        &mut rpc,
        &plan,
        &parent_market,
        &payer,
        &parent_forge,
        &mut transactions,
        parent_collateral,
    )?;
    let activated = activate_series_parent_root_v1(
        &mut rpc,
        &payer,
        &plan,
        &parent_market,
        &founding,
        parent_collateral_mint,
        &mut transactions,
    )?;
    if activated != parent_root {
        return Err(Error::new(
            "Series activated root differed from its Found prediction",
        ));
    }
    let root_account = rpc.required_account(parent_root.root, "activated Series root")?;
    let root_header = root_account
        .data
        .get(..dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or_else(|| Error::new("activated Series root omitted its canonical header"))?
        .to_vec();
    let finalized_fact = crate::series_found_prepare_input::SeriesParentRootFactV1::Finalized {
        root: parent_root.root,
        observed_data_len: root_account.data.len(),
        observed_lamports: root_account.lamports,
    };
    let final_input = build_series_found_prepare_input_v1(
        &mut rpc,
        &plan,
        &m0,
        &founder,
        &founder_records,
        &payer,
        child_collateral.mint,
        child_collateral.wallet,
        finalized_fact,
        funding_ledger_slot_count,
        selected_release,
        activation_deadline_slot,
        selected_manifest_entry_index,
        &mut transactions,
    )?;
    let final_minimum_slot = rpc.finalized_slot()?;
    let final_compiled = compile_series_prepare_from_hydrated_geometry_v1(
        &mut rpc,
        final_input,
        certificate_program,
        m0_frame,
        hydration,
        SeriesPrepareParentRootStateV1::Finalized,
        finalized_fact,
        crate::series_consume_geometry::SeriesConsumePrestateV1::PreparedPrediction,
        final_minimum_slot,
    )?;
    crate::series_found_prepare_campaign::require_series_selection_invariance_v1(
        &normalized_compiled,
        &final_compiled,
    )?;
    crate::series_geometry::require_series_prepare_geometry_invariance_v1(
        &normalized_compiled.geometry.prepare_fixed_data_lengths,
        &final_compiled.geometry.prepare_fixed_data_lengths,
    )?;
    require_series_shadow_capacity_v1(source_manifest, &final_compiled.geometry)?;

    let logical = series_prepare_logical_addresses_v1(
        &plan,
        parent_root,
        m0_frame,
        hydration,
        &final_compiled,
    )?;
    let fixed = series_prepare_hot_fixed_addresses_v1(
        &plan,
        &parent_market,
        parent_root,
        &m0,
        &founder_records,
        &final_compiled,
    )?;
    materialize_series_prepare_seal_v1(
        &mut rpc,
        &payer,
        &plan,
        &fixed,
        &final_compiled,
        &mut transactions,
    )?;
    let template = dclutch_trading::series::TemplateV3::decode(founder.admitted.template())
        .map_err(|_| Error::new("Series runtime Template refused decode"))?;
    let release = series_current_release_v1(
        &final_compiled,
        founder.admitted.template(),
        certificate_program,
        template.occurrence_count(),
        funding_ledger_slot_count,
    )?;
    let selected = crate::series_terminal_campaign::acquire_series_prepare_from_addresses_v1(
        &mut rpc,
        crate::series_terminal_campaign::SeriesPrepareAddressFrameV1 {
            fixed: &fixed,
            logical: &logical,
            occurrence_record: founder_records.occurrences[0].raw,
            occurrence_staging: founder_records.occurrences[0].staging,
            ticket_record: founder_records.tickets[0].raw,
            ticket_staging: founder_records.tickets[0].staging,
            siblings: &founder.admitted.siblings()[0],
            payer: payer.pubkey(),
            release,
        },
    )?;
    let (routing_observation, tables) = crate::market::publish_routing_table(
        &mut rpc,
        &payer,
        "SERIES-FIRST-PREPARE",
        std::slice::from_ref(&selected.instruction),
        &mut transactions,
    )?;
    let sent = rpc.send_v0_on_heap(
        "execute first selected Series Prepare",
        std::slice::from_ref(&selected.instruction),
        &payer,
        routing_observation,
        &tables,
        dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    )?;
    if let Some(error) = &sent.error {
        return Err(Error::new(format!(
            "first selected Series Prepare refused: {error}"
        )));
    }
    transactions.push(sent);
    let poststates = authenticate_series_prepare_poststates_v1(
        &mut rpc,
        parent_root,
        &founder,
        &final_compiled,
        logical[5],
        child_collateral.wallet,
        &root_header,
    )?;
    let output = serde_json::json!({
        "schema": "dclutch-series-found-prepare-execution-v1",
        "evidenceLevel": "local-validator",
        "parentMarket": parent_root.market.to_string(),
        "root": parent_root.root.to_string(),
        "childMarket": Pubkey::new_from_array(final_compiled.predicted_core.identity.market_id.to_bytes()).to_string(),
        "shadowCapacity": source_manifest.capacity_profile().as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "poststates": poststates,
        "transactions": transactions,
    });
    fs::write(
        arguments
            .output
            .as_ref()
            .expect("argument modes were validated"),
        format!("{}\n", serde_json::to_string_pretty(&output)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_series_found_prepare_input_v1<'a>(
    rpc: &mut Rpc,
    plan: &'a SuccessorPlan,
    m0: &'a FutureMarketImmutablePublicationV1,
    founder: &'a crate::series_founder::PreparedSeriesFounderV1,
    founder_records: &'a PublishedSeriesFounderRecordsV1,
    payer: &Keypair,
    collateral_mint: Pubkey,
    collateral_wallet: Pubkey,
    parent_root: crate::series_found_prepare_input::SeriesParentRootFactV1,
    funding_ledger_slot_count: u16,
    selected_release: ContentId,
    activation_deadline_slot: u64,
    selected_manifest_entry_index: u16,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'a>> {
    let mut input =
        crate::series_found_prepare_input::build_series_found_prepare_selection_input_v1(
            rpc,
            crate::series_found_prepare_input::SeriesFoundPrepareInputFactsV1 {
                plan,
                m0,
                founder,
                founder_records,
                payer: payer.pubkey(),
                founder_key: payer.pubkey(),
                refund_destination: collateral_wallet,
                founder_source: collateral_wallet,
                collateral_mint,
                parent_root,
                funding_ledger_slot_count,
                selected_release,
                activation_deadline_slot,
                selected_manifest_entry_index,
            },
        )?;
    let vacancy = input.material.claims_vacancy;
    let keys = [vacancy.aggregate, vacancy.position, vacancy.admission];
    let observed = [
        vacancy.aggregate_lamports,
        vacancy.position_lamports,
        vacancy.admission_lamports,
    ];
    let transfers = keys
        .iter()
        .zip(observed)
        .zip(input.claims_rent_principals)
        .filter_map(|((key, current), required)| {
            let deficit = required.saturating_sub(current);
            (deficit != 0).then(|| {
                solana_system_interface::instruction::transfer(&payer.pubkey(), key, deficit)
            })
        })
        .collect::<Vec<_>>();
    if !transfers.is_empty() {
        let sent = rpc.send(
            "prepay future Series M0 Claims rent from campaign payer",
            &transfers,
            payer,
        )?;
        if let Some(error) = sent.error.as_ref() {
            return Err(Error::new(format!(
                "Series Claims rent prepayment refused: {error}"
            )));
        }
        transactions.push(sent);
        input.material.claims_vacancy =
            crate::series_found_prepare_input::observe_series_claims_vacancy_v1(rpc, keys)?;
        input.lifecycle.now_slot = rpc.finalized_slot()?;
    }
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
fn compile_series_prepare_pass_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    m0: &FutureMarketImmutablePublicationV1,
    founder: &crate::series_founder::PreparedSeriesFounderV1,
    founder_records: &PublishedSeriesFounderRecordsV1,
    payer: &Keypair,
    collateral_mint: Pubkey,
    collateral_wallet: Pubkey,
    prediction: crate::series_found_prepare_input::SeriesPredictedParentRootV1,
    funding_ledger_slot_count: u16,
    selected_release: ContentId,
    activation_deadline_slot: u64,
    certificate_program: ContentId,
    m0_frame: SeriesPrepareM0FrameV1<'_>,
    hydration: SeriesPrepareHydrationRecordsV1<'_>,
    minimum_slot: u64,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1> {
    let root = crate::series_found_prepare_input::SeriesParentRootFactV1::Predicted(prediction);
    let input = build_series_found_prepare_input_v1(
        rpc,
        plan,
        m0,
        founder,
        founder_records,
        payer,
        collateral_mint,
        collateral_wallet,
        root,
        funding_ledger_slot_count,
        selected_release,
        activation_deadline_slot,
        preselection_series_manifest_entry_index_v1(&decode_hex(
            &crate::market::demo_market_input_base(
                pubkey(&plan.registry.program_id)?,
                crate::direct_market::authenticated_resolution_release_v1(plan)?,
            )?
            .capability_manifest_hex,
        )?)?,
        transactions,
    )?;
    compile_series_prepare_from_hydrated_geometry_v1(
        rpc,
        input,
        certificate_program,
        m0_frame,
        hydration,
        SeriesPrepareParentRootStateV1::PreActivationPredicted,
        root,
        crate::series_consume_geometry::SeriesConsumePrestateV1::PreparedPrediction,
        minimum_slot,
    )
}

fn series_prepare_hydration_records_v1<'a>(
    m0: &'a FutureMarketImmutablePublicationV1,
    m0_records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    founder: &'a crate::series_founder::PreparedSeriesFounderV1,
    published: &'a PublishedSeriesFounderRecordsV1,
) -> Result<SeriesPrepareHydrationRecordsV1<'a>> {
    use dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2;
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    };
    let portfolio_body = m0_body_from_publication_v1(m0, m0.portfolio.raw)?;
    Ok(SeriesPrepareHydrationRecordsV1 {
        template: series_prepare_founder_record_v1(
            published.template,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            founder.admitted.template(),
            "Template",
        )?,
        occurrence: series_prepare_founder_record_v1(
            published.occurrences[0],
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.occurrences()[0],
            "occurrence",
        )?,
        ticket: series_prepare_founder_record_v1(
            published.tickets[0],
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.tickets()[0],
            "Ticket",
        )?,
        portfolio: series_prepare_record_from_m0_publication_v1(
            m0_records,
            PORTFOLIO_SCHEMA_ID_V2,
            &portfolio_body,
            "Portfolio",
        )?,
    })
}

fn observe_series_prepare_m0_accounts_v1(
    rpc: &mut Rpc,
    publication: &FutureMarketImmutablePublicationV1,
    records: &[SeriesPrepareFinalizedRecordV1<'_>],
    collateral_wallet: Pubkey,
    payer: Pubkey,
    minimum_slot: u64,
) -> Result<Vec<SeriesPrepareFinalizedAccountV1>> {
    let mut addresses = Vec::new();
    for address in publication.project_found {
        let is_record = records
            .iter()
            .any(|record| address == record.raw || address == record.staging);
        if address != publication.project_found[1]
            && !publication.series_prepare_vacancies.contains(&address)
            && !is_record
            && !addresses.contains(&address)
        {
            addresses.push(address);
        }
    }
    // Custody consumes the Realm's Mint and Token program even though Core's
    // ProjectFound frame authenticates their identities through the Realm.
    let realm_record = records
        .iter()
        .find(|record| record.raw == publication.realm.raw)
        .ok_or_else(|| Error::new("Series M0 publication omitted Realm record"))?;
    let realm = dclutch_market::realm::RealmV1::decode(realm_record.body)
        .map_err(|error| Error::new(format!("Series M0 Realm record: {error:?}")))?;
    for key in [realm.collateral_mint(), realm.token_program()] {
        let address = Pubkey::new_from_array(*key);
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    // Expire returns collateral to the exact campaign wallet and native rent
    // to its payer. These are the same facts authenticated by the input builder.
    for address in [collateral_wallet, payer] {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    let (_, accounts) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    addresses
        .into_iter()
        .zip(accounts)
        .map(|(address, account)| {
            let account = account.ok_or_else(|| {
                Error::new(format!(
                    "Series M0 publisher-owned account {address} vanished before compilation"
                ))
            })?;
            Ok(SeriesPrepareFinalizedAccountV1 {
                address,
                expected_owner: account.owner,
            })
        })
        .collect()
}

fn require_series_shadow_capacity_v1(
    manifest: dclutch_series_shadow_bundle_generator::SeriesShadowSourceManifestV1<'_>,
    geometry: &crate::series_source::SeriesObservedGeometryV1,
) -> Result<()> {
    let capacity = dclutch_trading_sbf::series::release_v5::series_consume_capacity_profile_v1(
        &geometry.consume_fixed_data_lengths,
        geometry.consume_funding_count,
        2,
    )
    .map_err(|error| Error::new(format!("Series executor capacity: {error:?}")))?;
    if manifest.fixed_data_lengths() != &geometry.consume_fixed_data_lengths
        || manifest.funding_count() != geometry.consume_funding_count
        || manifest.capacity_profile() != capacity
    {
        return Err(Error::new(
            "Series selected compiler geometry differs from the prebuilt executor capacity",
        ));
    }
    Ok(())
}

fn series_prepare_logical_addresses_v1(
    plan: &SuccessorPlan,
    parent_root: SeriesParentRootV1,
    m0: SeriesPrepareM0FrameV1<'_>,
    records: SeriesPrepareHydrationRecordsV1<'_>,
    compiled: &crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
) -> Result<Vec<Pubkey>> {
    let input = SeriesPrepareHydratorInputV1 {
        registry: pubkey(&plan.registry.program_id)?,
        core: pubkey(&plan.core.program_id)?,
        trading: pubkey(&plan.trading.program_id)?,
        custody: pubkey(&plan.custody.program_id)?,
        rent_program: pubkey(&plan.rent_credit.program_id)?,
        parent_root: parent_root.root,
        parent_root_state: SeriesPrepareParentRootStateV1::Finalized,
        m0,
        template: records.template,
        occurrence: records.occurrence,
        ticket: records.ticket,
        portfolio: records.portfolio,
        children: &compiled.prepare_children,
    };
    let layout = hydrate_series_prepare_role_layout_v1(&input)?;
    Ok(crate::series_geometry::prepare_sources_v1(&layout)
        .iter()
        .map(SeriesPrepareRoleSourceV1::address)
        .collect())
}

fn series_current_release_v1<'a>(
    compiled: &'a crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
    template_bytes: &[u8],
    certificate_program: ContentId,
    occurrence_count: u32,
    funding_count: u16,
) -> Result<dclutch_trading_sbf::series::release_v5::SeriesCurrentReleaseInputV5<'a>> {
    use dclutch_trading_sbf::series::{
        expire_funding_artifacts_v5::SeriesExpireAccountProfileInputV5,
        prepare_funding_artifacts_v5::SeriesPrepareAccountProfileInputV5,
        release_v5::SeriesCurrentReleaseInputV5,
    };
    Ok(SeriesCurrentReleaseInputV5 {
        template: dclutch_trading::series::template_content_id(template_bytes)
            .map_err(|_| Error::new("Series current release Template identity"))?,
        template_occurrence_count: occurrence_count,
        consume_shadow_certificate_program: certificate_program,
        prepare_profile: SeriesPrepareAccountProfileInputV5 {
            fixed_data_lengths: &compiled.geometry.prepare_fixed_data_lengths,
        },
        prepare_requests: compiled.prepare_children.prepare_requests(),
        prepare_ticket_rent_lamports: compiled.geometry.prepare_ticket_rent_lamports,
        consume_observed_data_lengths: &compiled.geometry.consume_fixed_data_lengths,
        consume_requests: compiled.prepare_children.consume_requests(),
        consume_funding_count: u32::from(funding_count),
        expire_profile: SeriesExpireAccountProfileInputV5 {
            fixed_data_lengths: &compiled.geometry.expire_fixed_data_lengths,
        },
        expire_requests: compiled.prepare_children.expire_requests(),
    })
}

fn materialize_series_prepare_seal_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    fixed: &[Pubkey; dclutch_market::capability_program::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3],
    compiled: &crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    use dclutch_market::capability_program::hot_v3::HOT_CAPABILITY_SEAL_ACCOUNT_V3;
    use dclutch_operator::capability_seal_v1::{
        CapabilitySealInstructionInputV1, capability_seal_instruction_v1,
    };
    use dclutch_trading::series::request::SeriesActionV3;

    let seal = fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3];
    if rpc.account(seal)?.is_some() {
        return Ok(());
    }
    let descriptor = compiled
        .selected
        .records
        .get(6)
        .ok_or_else(|| Error::new("Series Prepare selected records omitted descriptor"))?;
    let descriptor_body = decode_hex(&descriptor.body_hex)?;
    let instruction = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
        trading_program: pubkey(&plan.trading.program_id)?,
        registry_program: pubkey(&plan.registry.program_id)?,
        trading_semantic_release: hex32(&plan.trading.semantic_release_id)?,
        descriptor_digest: Sha256::digest(&descriptor_body).into(),
        action: u32::from(SeriesActionV3::Prepare as u8),
        fixed_frame: fixed,
        payer: payer.pubkey(),
    })
    .map_err(|error| Error::new(format!("Series Prepare seal builder: {error:?}")))?
    .instruction;
    let (observation, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "SERIES-PREPARE-SEAL",
        std::slice::from_ref(&instruction),
        transactions,
    )?;
    let sent = rpc.send_v0_on_heap(
        "materialize Series Prepare capability seal",
        std::slice::from_ref(&instruction),
        payer,
        observation,
        &tables,
        dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    )?;
    if let Some(error) = &sent.error {
        return Err(Error::new(format!(
            "Series Prepare capability seal refused: {error}"
        )));
    }
    transactions.push(sent);
    let live = rpc.required_account(seal, "Series Prepare capability seal")?;
    if live.owner != pubkey(&plan.trading.program_id)? || live.executable || live.data.is_empty() {
        return Err(Error::new(
            "Series Prepare capability seal poststate differed",
        ));
    }
    Ok(())
}

fn authenticate_series_prepare_poststates_v1(
    rpc: &mut Rpc,
    parent: SeriesParentRootV1,
    founder: &crate::series_founder::PreparedSeriesFounderV1,
    compiled: &crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
    ticket_state: Pubkey,
    founder_source: Pubkey,
    expected_root_header: &[u8],
) -> Result<serde_json::Value> {
    use dclutch_custody::token_svm::TokenAccount;
    use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
    use dclutch_trading::series::{
        admit_ticket,
        replay::{SeriesStateV3, TicketStateV3},
    };

    let template = dclutch_trading::series::TemplateV3::decode(founder.admitted.template())
        .map_err(|_| Error::new("Series Prepare poststate Template refused decode"))?;
    let expected_root = SeriesStateV3::new(template.close_rent())
        .prepare_ticket(0)
        .map_err(|_| Error::new("Series Prepare expected root successor refused"))?
        .encode(template.occurrence_count())
        .map_err(|_| Error::new("Series Prepare expected root encoding refused"))?;
    let root = rpc.required_account(parent.root, "prepared Series root")?;
    if root.data.len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + expected_root.len()
        || root.data[..CAPABILITY_ROOT_HEADER_BYTES_V1] != *expected_root_header
        || root.data[CAPABILITY_ROOT_HEADER_BYTES_V1..] != expected_root
    {
        return Err(Error::new("Series Prepare root replay poststate differed"));
    }
    let ticket = admit_ticket(&founder.admitted.tickets()[0])
        .map_err(|_| Error::new("Series Prepare expected Ticket refused"))?;
    let expected_ticket = TicketStateV3::prepared(ticket.content_id()).encode();
    let ticket_account = rpc.required_account(ticket_state, "prepared Series Ticket state")?;
    if ticket_account.data != expected_ticket {
        return Err(Error::new("Series Prepare Ticket poststate differed"));
    }
    let projected_address = compiled.physical.realized_hoard_replay;
    let projected = rpc.required_account(projected_address, "prepared projected Custody state")?;
    let expected_projected = compiled
        .prepared_projected_state
        .encode()
        .map_err(|_| Error::new("Series Prepare projected poststate encoding refused"))?;
    if projected.data != expected_projected {
        return Err(Error::new(
            "Series Prepare projected Custody poststate differed",
        ));
    }
    let replay_address = compiled.physical.normal_replay;
    let replay = rpc.required_account(replay_address, "prepared normal Custody replay")?;
    let expected_replay = compiled
        .prepared_source_replay
        .to_bytes()
        .map_err(|_| Error::new("Series Prepare replay poststate encoding refused"))?;
    if replay.data != expected_replay {
        return Err(Error::new(
            "Series Prepare Custody replay poststate differed",
        ));
    }
    let source = rpc.required_account(founder_source, "Series founder source poststate")?;
    let vault = rpc.required_account(
        Pubkey::new_from_array(compiled.physical.prepare.escrow_vault),
        "Series escrow vault poststate",
    )?;
    let source = TokenAccount::parse(&source.data)
        .map_err(|_| Error::new("Series founder source poststate refused token decode"))?;
    let vault = TokenAccount::parse(&vault.data)
        .map_err(|_| Error::new("Series escrow vault poststate refused token decode"))?;
    let principal = founder.facts.occurrences[0].hoard_principal;
    let total_principal = principal
        .checked_add(founder.facts.occurrences[1].hoard_principal)
        .ok_or_else(|| Error::new("Series founder principal sum overflow"))?;
    if vault.amount != principal
        || source
            .amount
            .checked_add(vault.amount)
            .filter(|amount| *amount == total_principal)
            .is_none()
    {
        return Err(Error::new(
            "Series Prepare escrow vault did not receive first-occurrence principal",
        ));
    }
    Ok(serde_json::json!({
        "rootRevision": 1,
        "ticketPrepared": true,
        "projectedCustody": projected_address.to_string(),
        "normalCustodyReplay": replay_address.to_string(),
        "founderSourceAtoms": source.amount,
        "escrowVaultAtoms": vault.amount,
    }))
}

/// The selected Trading ledger carries exactly its manifest-selected row.
/// Derive its width from that one owned mask instead of retaining a fixture
/// count beside the Series activation compiler.
fn selected_series_funding_ledger_slot_count_v1(selected_manifest_entry_index: u16) -> Result<u16> {
    let mask = 1_u16
        .checked_shl(u32::from(selected_manifest_entry_index))
        .ok_or_else(|| Error::new("Series selected manifest entry exceeds the funding mask"))?;
    dclutch_market::capability_manifest::funding_ledger_slot_count_v2(mask)
        .map_err(|error| Error::new(format!("Series selected funding ledger: {error:?}")))
}

/// Derive the Series entry position before the selected descriptor and
/// Certificate exist. The selected-capability owner validates the canonical
/// Resolution base and shares its kind ordering with the later final merge.
fn preselection_series_manifest_entry_index_v1(parent_base_manifest: &[u8]) -> Result<u16> {
    let kind: [u8; 32] =
        Sha256::digest(dclutch_trading::series::SERIES_SUCCESSOR_KIND_PREIMAGE_V3).into();
    crate::selected_capability::preselection_manifest_entry_index_v1(parent_base_manifest, kind)
}

/// Finalized Registry coordinates for the immutable two-leaf Series founder
/// material.  The Template is also M1's selected config; the occurrence and
/// Ticket records remain M0 child facts and must never be folded into the
/// parent Market manifest.
#[derive(Clone, Debug)]
pub(crate) struct PublishedSeriesFounderRecordsV1 {
    pub(crate) template: crate::runtime::PublishedRecord,
    pub(crate) occurrences: [crate::runtime::PublishedRecord; 2],
    pub(crate) tickets: [crate::runtime::PublishedRecord; 2],
}

/// Publish the immutable child facts before the first Prepare can acquire
/// them.  `publish_record` is idempotent only after re-reading and checking
/// the derived raw record, so accepting an already-published Template still
/// proves that M1's config bytes equal the founder's M0 template bytes.
pub(crate) fn publish_series_founder_records_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    founder: &crate::series_founder::PreparedSeriesFounderV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<PublishedSeriesFounderRecordsV1> {
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    };

    let publish = |schema,
                   bytes: &[u8],
                   label: &str,
                   rpc: &mut Rpc,
                   transactions: &mut Vec<crate::model::TransactionEvidence>| {
        let record = crate::runtime::publish_record(
            rpc,
            registry,
            payer,
            schema,
            bytes,
            None,
            transactions,
        )?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if record.schema != schema || record.digest != digest {
            return Err(Error::new(format!(
                "Series founder {label} publication changed schema or content"
            )));
        }
        Ok(record)
    };
    let template = publish(
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        founder.admitted.template(),
        "Template",
        rpc,
        transactions,
    )?;
    let occurrences = [
        publish(
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.occurrences()[0],
            "first occurrence",
            rpc,
            transactions,
        )?,
        publish(
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.occurrences()[1],
            "second occurrence",
            rpc,
            transactions,
        )?,
    ];
    let tickets = [
        publish(
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.tickets()[0],
            "first Ticket",
            rpc,
            transactions,
        )?,
        publish(
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.tickets()[1],
            "second Ticket",
            rpc,
            transactions,
        )?,
    ];
    Ok(PublishedSeriesFounderRecordsV1 {
        template,
        occurrences,
        tickets,
    })
}

/// Activate the just-founded Series parent from its durable founding report.
///
/// The command is deliberately separate from the generic founding executable:
/// it accepts only the parent Market document already consumed by Found and
/// the report Found wrote.  It cannot substitute child M0 for parent M1, and
/// it cannot claim activation without the signed transaction evidence it
/// emits to `--output`.
pub(crate) const SERIES_ROOT_ACTIVATE_COMMAND_V1: &str =
    "local-private-validator-series-root-activate-v1";

struct RootActivateArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    parent_market: PathBuf,
    founding_report: PathBuf,
    collateral_mint: Pubkey,
    payer_keypair: PathBuf,
    output: PathBuf,
    execute: bool,
}

pub(crate) fn run_root_activate_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_root_activate_arguments_v1(arguments)?;
    if !arguments.execute {
        return Err(Error::new(
            "Series root activation refuses a dry run: this command exists to record an actual finalized activation",
        ));
    }
    if arguments.output.exists() {
        return Err(Error::new(format!(
            "Series root activation refuses to overwrite {}",
            arguments.output.display()
        )));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&arguments.plan)?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let parent_input: MarketRunInput =
        serde_json::from_slice(&fs::read(&arguments.parent_market)?)?;
    crate::market::validate_market_input(&parent_input)?;
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&arguments.founding_report)?)?;
    let evidence = report
        .pointer("/execution/market")
        .cloned()
        .unwrap_or(report);
    let founding: crate::market::MarketExecutionEvidence = serde_json::from_value(evidence)
        .map_err(|error| Error::new(format!("Series founding report: {error}")))?;
    let payer = Keypair::new_from_array(crate::campaign::read_keypair_file(
        &arguments.payer_keypair,
        "Series root activation payer",
    )?);
    let mut rpc = Rpc::connect(&arguments.rpc_url)?;
    let mut transactions = Vec::new();
    let activated = activate_series_parent_root_v1(
        &mut rpc,
        &payer,
        &plan,
        &parent_input,
        &founding,
        arguments.collateral_mint,
        &mut transactions,
    )?;
    if transactions.is_empty() {
        return Err(Error::new(
            "Series root activation submitted no transaction evidence",
        ));
    }
    let output = serde_json::json!({
        "schema": "dclutch-series-parent-root-activation-v1",
        "evidenceLevel": "local-validator",
        "parentMarket": activated.market.to_string(),
        "root": activated.root.to_string(),
        "transactions": transactions,
    });
    fs::write(
        &arguments.output,
        format!("{}\n", serde_json::to_string_pretty(&output)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn parse_root_activate_arguments_v1(arguments: Vec<String>) -> Result<RootActivateArgumentsV1> {
    let mut values = std::collections::BTreeMap::new();
    let mut position = 0_usize;
    let mut execute = false;
    while position < arguments.len() {
        let flag = arguments
            .get(position)
            .ok_or_else(|| Error::new("Series root activation arguments ended unexpectedly"))?;
        if flag == "--execute" {
            execute = true;
            position += 1;
            continue;
        }
        let value = arguments.get(position + 1).ok_or_else(|| {
            Error::new(format!("Series root activation omitted value for {flag}"))
        })?;
        if !matches!(
            flag.as_str(),
            "--rpc-url"
                | "--plan"
                | "--parent-market"
                | "--founding-report"
                | "--collateral-mint"
                | "--payer-keypair"
                | "--output"
        ) || values.insert(flag.clone(), value.clone()).is_some()
        {
            return Err(Error::new(format!(
                "Series root activation arguments rejected {flag}"
            )));
        }
        position += 2;
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| Error::new(format!("Series root activation requires {flag}")))
    };
    let collateral_mint = required("--collateral-mint")?
        .parse()
        .map_err(|error| Error::new(format!("Series root activation collateral mint: {error}")))?;
    if collateral_mint == Pubkey::default() {
        return Err(Error::new(
            "Series root activation collateral mint was default",
        ));
    }
    Ok(RootActivateArgumentsV1 {
        rpc_url: required("--rpc-url")?,
        plan: PathBuf::from(required("--plan")?),
        parent_market: PathBuf::from(required("--parent-market")?),
        founding_report: PathBuf::from(required("--founding-report")?),
        collateral_mint,
        payer_keypair: PathBuf::from(required("--payer-keypair")?),
        output: PathBuf::from(required("--output")?),
        execute,
    })
}

/// Final parent identity used to normalize a Series-selected payload before
/// the parent founding begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesParentRootV1 {
    /// The parent Market which will be founded through Open.  It is never the
    /// child Market precommitted by the Template.
    pub(crate) market: Pubkey,
    /// The Trading PDA that selector-255 will create for this parent.
    pub(crate) root: Pubkey,
    /// Registry-derived raw/staging bumps carried in the root header.
    pub(crate) record_bumps: SelectedRecordBumpsV1,
}

/// Produce the Series parent M1 from the same checked Resolution release as a
/// normal local Market, then attach the already-normalized Series closure.
///
/// The Template remains the M0 child configuration within `selected`; this
/// creates a separate M1 manifest whose selected entry names that Template.
/// Keeping this at the driver boundary prevents a caller from reusing the
/// Direct child input after the Series payload has been attached.
pub(crate) fn build_series_parent_market_v1(
    plan: &SuccessorPlan,
    registry: Pubkey,
    selected: SelectedCapabilityV1,
) -> Result<MarketRunInput> {
    require_series_parent_payload_v1(&selected)?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(plan)?;
    let mut parent = crate::market::demo_market_input_base(registry, resolution_release)?;
    crate::selected_capability::attach_selected_capability_v1(&mut parent, selected)?;
    crate::market::validate_market_input(&parent)?;
    Ok(parent)
}

/// Derive the parent Series root from the complete parent Market input.
///
/// The payload must already be attached to `parent_input`; deriving this from
/// the earlier capability-free child preview would make a root whose header
/// names a different manifest.  No chain state is asserted here: the caller
/// supplies the future parent collateral Mint and later observes this exact
/// vacant PDA before activation.
pub(crate) fn derive_series_parent_root_v1(
    plan: &SuccessorPlan,
    parent_input: &MarketRunInput,
    collateral_mint: Pubkey,
) -> Result<SeriesParentRootV1> {
    let selected = parent_input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Series parent Market omitted selected capability"))?;
    if selected.family != "series" {
        return Err(Error::new(
            "Series parent Market selected a capability from another family",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let targets = derive_founding_targets(plan, parent_input, collateral_mint)?;
    let manifest = decode_hex(&parent_input.capability_manifest_hex)?;
    let descriptor = decode_hex(&selected.selected_descriptor_hex)?;
    let descriptor = CapabilityProgramV4::decode(&descriptor)
        .map_err(|error| Error::new(format!("Series parent selected descriptor: {error:?}")))?;
    let program_set = decode_hex(&selected.program_set_hex)?;
    let config = decode_hex(&selected.config_hex)?;
    let manifest_id = record_identity(&manifest);
    let program_set_id: [u8; 32] = Sha256::digest(&program_set).into();
    let config_id: [u8; 32] = Sha256::digest(&config).into();
    let selection = CapabilityExecutionSelectionV1::new(
        selected.selected_manifest_entry_index,
        ContentId::new(manifest_id).map_err(|_| Error::new("Series parent manifest identity"))?,
        descriptor.kind(),
        ContentId::new(program_set_id)
            .map_err(|_| Error::new("Series parent program-set identity"))?,
        ContentId::new(config_id).map_err(|_| Error::new("Series parent config identity"))?,
    )
    .map_err(|error| Error::new(format!("Series parent execution selection: {error:?}")))?;
    let manifest_bumps = record_bumps_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_id,
        "Series parent manifest",
    )?;
    let program_set_bumps = record_bumps_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        program_set_id,
        "Series parent program set",
    )?;
    let config_bumps = record_bumps_v1(
        registry,
        dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        config_id,
        "Series parent Template",
    )?;
    let record_bumps = SelectedRecordBumpsV1::new(
        manifest_bumps[0],
        manifest_bumps[1],
        config_bumps[0],
        config_bumps[1],
    );
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(hex32(&plan.release_set_id)?)
            .map_err(|_| Error::new("Series parent release-set identity"))?,
        targets.open_market.to_bytes(),
        open_market_generation_v1(parent_input)?,
        selection.with_capability_release_record_bumps(program_set_bumps[0], program_set_bumps[1]),
        record_bumps,
    )
    .map_err(|error| Error::new(format!("Series parent root header: {error:?}")))?;
    Ok(SeriesParentRootV1 {
        market: targets.open_market,
        root: Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0,
        record_bumps,
    })
}

fn record_bumps_v1(
    registry: Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
    label: &str,
) -> Result<[u8; 2]> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("{label} schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("{label} content: {error:?}")))?,
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
    let (_, raw) = derive(key.raw_record_pda_seeds());
    let (_, staging) = derive(key.staging_cursor_pda_seeds());
    Ok([raw, staging])
}

/// Refuse a caller that accidentally normalizes against child bytes.  Keeping
/// the check beside the root derivation makes the distinction executable at
/// the one public driver boundary rather than a comment in a compiler test.
pub(crate) fn require_series_parent_payload_v1(payload: &SelectedCapabilityV1) -> Result<()> {
    if payload.family != "series" || payload.records.len() != 39 {
        return Err(Error::new(
            "Series parent activation requires the complete 39-record Series payload",
        ));
    }
    Ok(())
}

/// Publish selector-255 for an already-founded parent and verify the generic
/// root poststate.  The parent founding has already transaction-published all
/// 39 records; this function re-derives each Registry coordinate from those
/// bytes and refuses an evidence map that tries to substitute one.
pub(crate) fn activate_series_parent_root_v1(
    rpc: &mut Rpc,
    payer: &solana_sdk::signature::Keypair,
    plan: &SuccessorPlan,
    parent_input: &MarketRunInput,
    founding: &crate::market::MarketExecutionEvidence,
    collateral_mint: Pubkey,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<SeriesParentRootV1> {
    let selected = parent_input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Series root activation omitted selected payload"))?;
    require_series_parent_payload_v1(selected)?;
    let expected = derive_series_parent_root_v1(plan, parent_input, collateral_mint)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let template = dclutch_trading::series::TemplateV3::decode(&decode_hex(&selected.config_hex)?)
        .map_err(|_| Error::new("Series root activation Template refused decode"))?;
    let manifest_body = decode_hex(&parent_input.capability_manifest_hex)?;
    let program_set_body = decode_hex(&selected.program_set_hex)?;
    let config_body = decode_hex(&selected.config_hex)?;
    let manifest = pair_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_body,
    )?;
    let program_set = pair_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        &program_set_body,
    )?;
    let config = pair_v1(
        registry,
        dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        &config_body,
    )?;
    let realm = pair_from_id_v1(
        registry,
        dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1,
        template.realm().to_bytes(),
    )?;
    let record = |index: usize, label: &str| -> Result<SelectedActivationRecordPairV1> {
        let value = selected
            .records
            .get(index)
            .ok_or_else(|| Error::new(format!("Series root activation omitted {label}")))?;
        pair_v1(
            registry,
            hex32(&value.schema_hex)?,
            &decode_hex(&value.body_hex)?,
        )
    };
    // Five action banks occupy seven records each.  The activation bundle is
    // the final three records before the activation-capable ProgramSet.
    let account_profile = record(35, "activation account profile")?;
    let effect = record(36, "activation effect")?;
    let descriptor = record(37, "activation descriptor")?;
    let market = evidence_key_v1(founding, "founding_market")?;
    if market != expected.market {
        return Err(Error::new(
            "Series root activation founding report names a different parent Market",
        ));
    }
    let selected_funding_ledger = evidence_key_v1(founding, "direct_trading_funding_ledger")?;
    let resolution_funding_ledger = founding
        .accounts
        .iter()
        .filter(|(label, _)| label.starts_with("founding_funding_ledger_v2_"))
        .map(|(_, evidence)| pubkey(&evidence.address))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .find(|key| *key != selected_funding_ledger)
        .ok_or_else(|| Error::new("Series root activation omitted Resolution funding ledger"))?;
    let addresses = [
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
    let (slot, observed) = rpc.finalized_accounts(&addresses, 0)?;
    let account = |index: usize, label: &str| -> Result<RpcAccount> {
        observed.get(index).cloned().flatten().ok_or_else(|| {
            Error::new(format!(
                "Series root activation finalized snapshot omitted {label}"
            ))
        })
    };
    let market_account = account(0, "parent Market")?;
    let realm_account = account(1, "Series realm")?;
    let manifest_account = account(3, "parent manifest")?;
    let program_set_account = account(5, "Series ProgramSet")?;
    let config_account = account(7, "Series Template")?;
    let profile_account = account(9, "Series activation profile")?;
    let effect_account = account(11, "Series activation effect")?;
    let descriptor_account = account(13, "Series activation descriptor")?;
    let ledger_account = account(15, "Trading funding ledger")?;
    let market_state = dclutch_market::CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Series root activation Core Market: {error:?}")))?;
    let manifest_id: [u8; 32] = Sha256::digest(&manifest_body).into();
    let record_matches = |account: &RpcAccount, expected: &[u8]| {
        account.owner == registry && account.data == expected
    };
    if slot == 0
        || market_account.owner != core
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.realm_id.to_bytes() != template.realm().to_bytes()
        || market_state.identity.capability_manifest.to_bytes() != manifest_id
        || realm_account.owner != registry
        || Sha256::digest(&realm_account.data).as_slice() != template.realm().to_bytes()
        || !record_matches(&manifest_account, &manifest_body)
        || !record_matches(&program_set_account, &program_set_body)
        || !record_matches(&config_account, &config_body)
        || !record_matches(
            &profile_account,
            &decode_hex(&selected.records[35].body_hex)?,
        )
        || !record_matches(
            &effect_account,
            &decode_hex(&selected.records[36].body_hex)?,
        )
        || !record_matches(
            &descriptor_account,
            &decode_hex(&selected.records[37].body_hex)?,
        )
    {
        return Err(Error::new(
            "Series root activation finalized record bytes differ from the selected closure",
        ));
    }
    let selected_descriptor =
        CapabilityProgramV4::decode(&decode_hex(&selected.selected_descriptor_hex)?)
            .map_err(|error| Error::new(format!("Series root selected descriptor: {error:?}")))?;
    let activation_request =
        dclutch_trading_sbf::series::activation_bundle_v1::series_activation_request_v1()
            .map_err(|error| Error::new(format!("Series activation request: {error:?}")))?;
    let context: [u8; 32] = Sha256::digest(b"dclutch/series-root-activation/v1").into();
    let activation =
        build_selected_capability_activation_plan_v1(SelectedCapabilityActivationInputV1 {
            market,
            market_account: &market_account,
            current_slot: slot,
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
            manifest_body: &manifest_body,
            entry_index: selected.selected_manifest_entry_index,
            program_set,
            program_set_id: Sha256::digest(&program_set_body).into(),
            config,
            config_id: Sha256::digest(&config_body).into(),
            capability_kind: selected_descriptor.kind().to_bytes(),
            account_profile,
            effect,
            descriptor,
            selected_funding_ledger,
            selected_funding_ledger_account: &ledger_account,
            resolution_funding_ledger,
            family_activation_request: &activation_request,
            context,
        })?;
    if activation.root != expected.root {
        return Err(Error::new(
            "Series root activation plan changed the normalized parent root",
        ));
    }
    let outcome = execute_selected_capability_activation_v1(
        rpc,
        payer,
        &activation,
        "activate Series selector-255 parent root",
    )?;
    transactions.extend(outcome.routing_transactions);
    transactions.push(outcome.activation);
    Ok(expected)
}

fn series_prepare_hot_fixed_addresses_v1(
    plan: &SuccessorPlan,
    parent: &MarketRunInput,
    parent_root: SeriesParentRootV1,
    m0: &FutureMarketImmutablePublicationV1,
    founder_records: &PublishedSeriesFounderRecordsV1,
    compiled: &crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
) -> Result<[Pubkey; dclutch_market::capability_program::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3]> {
    use dclutch_market::capability_program::{
        hot_v3::*, v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
    };
    use dclutch_trading::series::request::SeriesActionV3;
    use dclutch_vm::capability_seal::CapabilitySealKeyV1;

    let registry = pubkey(&plan.registry.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let selected = &compiled.selected;
    let selected_record = |index: usize, label: &str| {
        let record = selected
            .records
            .get(index)
            .ok_or_else(|| Error::new(format!("Series Prepare omitted {label}")))?;
        pair_v1(
            registry,
            hex32(&record.schema_hex)?,
            &decode_hex(&record.body_hex)?,
        )
    };
    let manifest = pair_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &decode_hex(&parent.capability_manifest_hex)?,
    )?;
    let program_set = selected_record(38, "ProgramSet record")?;
    let account_profile = selected_record(0, "Prepare AccountProfile record")?;
    let request_profile = selected_record(1, "Prepare RequestProfile record")?;
    let lifecycle = selected_record(2, "Prepare lifecycle record")?;
    let strategy = selected_record(3, "Prepare strategy record")?;
    let transition = selected_record(4, "Prepare Transition record")?;
    let effect = selected_record(5, "Prepare Effect record")?;
    let descriptor = selected_record(6, "Prepare descriptor record")?;
    let product = pair_v1(
        registry,
        m0.product.schema,
        &m0_body_from_publication_v1(m0, m0.product.raw)?,
    )?;
    let result_domain = pair_v1(
        registry,
        m0.domain.schema,
        &m0_body_from_publication_v1(m0, m0.domain.raw)?,
    )?;
    let portfolio = pair_v1(
        registry,
        m0.portfolio.schema,
        &m0_body_from_publication_v1(m0, m0.portfolio.raw)?,
    )?;
    let basis = m0
        .series_prepare_records
        .iter()
        .find(|record| {
            record.published.schema
                == dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3
        })
        .ok_or_else(|| Error::new("Series Prepare M0 publication omitted linked Basis"))?;
    let linked_basis = pair_v1(registry, basis.published.schema, &basis.body)?;
    let config = pair_v1(
        registry,
        dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        &m0_body_from_founder_v1(founder_records.template, compiled)?,
    )?;

    let mut fixed = [Pubkey::default(); HOT_FIXED_ACCOUNT_COUNT_V3];
    let set_record = |fixed: &mut [Pubkey; HOT_FIXED_ACCOUNT_COUNT_V3],
                      raw: usize,
                      pair: SelectedActivationRecordPairV1| {
        fixed[raw] = pair.raw;
        fixed[raw + 1] = pair.staging;
    };
    fixed[HOT_MARKET_ACCOUNT_V3] = parent_root.market;
    fixed[HOT_ROOT_ACCOUNT_V3] = parent_root.root;
    set_record(&mut fixed, HOT_MANIFEST_RAW_ACCOUNT_V3, manifest);
    set_record(&mut fixed, HOT_PROGRAM_SET_RAW_ACCOUNT_V3, program_set);
    set_record(&mut fixed, HOT_DESCRIPTOR_RAW_ACCOUNT_V3, descriptor);
    set_record(&mut fixed, HOT_CONFIG_RAW_ACCOUNT_V3, config);
    set_record(
        &mut fixed,
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        account_profile,
    );
    set_record(
        &mut fixed,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        request_profile,
    );
    set_record(&mut fixed, HOT_TRANSITION_RAW_ACCOUNT_V3, transition);
    set_record(&mut fixed, HOT_EFFECT_RAW_ACCOUNT_V3, effect);
    set_record(&mut fixed, HOT_LIFECYCLE_RAW_ACCOUNT_V3, lifecycle);
    set_record(&mut fixed, HOT_STRATEGY_RAW_ACCOUNT_V3, strategy);
    fixed[HOT_ACTIVATION_CACHE_ACCOUNT_V3] = pubkey(&plan.activation)?;
    fixed[HOT_CORE_PROGRAM_ACCOUNT_V3] = pubkey(&plan.core.program_id)?;
    fixed[HOT_CORE_PROGRAMDATA_ACCOUNT_V3] = pubkey(&plan.core.programdata_id)?;
    fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = trading;
    fixed[HOT_TRADING_PROGRAMDATA_ACCOUNT_V3] = pubkey(&plan.trading.programdata_id)?;
    fixed[HOT_REGISTRY_PROGRAM_ACCOUNT_V3] = registry;
    fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = sysvar::rent::ID;
    fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = sysvar::instructions::ID;
    set_record(&mut fixed, HOT_PRODUCT_RAW_ACCOUNT_V3, product);
    set_record(&mut fixed, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, result_domain);
    set_record(&mut fixed, HOT_PORTFOLIO_RAW_ACCOUNT_V3, portfolio);
    set_record(&mut fixed, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, linked_basis);
    let semantic_release = hex32(&plan.trading.semantic_release_id)?;
    let seal_key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
        descriptor.content,
        u32::from(SeriesActionV3::Prepare as u8),
        semantic_release,
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Series Prepare capability seal key: {error:?}")))?;
    fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3] =
        Pubkey::find_program_address(&seal_key.seeds().as_slices(), &trading).0;
    if fixed.iter().any(|key| *key == Pubkey::default()) {
        return Err(Error::new(
            "Series Prepare fixed frame omitted a coordinate",
        ));
    }
    Ok(fixed)
}

fn m0_body_from_publication_v1(
    publication: &FutureMarketImmutablePublicationV1,
    raw: Pubkey,
) -> Result<Vec<u8>> {
    publication
        .series_prepare_records
        .iter()
        .find(|record| record.published.raw == raw)
        .map(|record| record.body.clone())
        .ok_or_else(|| Error::new("Series Prepare publication omitted finalized record body"))
}

fn m0_body_from_founder_v1(
    published: crate::runtime::PublishedRecord,
    compiled: &crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1,
) -> Result<Vec<u8>> {
    let bytes = decode_hex(&compiled.selected.config_hex)?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    if published.digest != digest {
        return Err(Error::new(
            "Series Prepare selected Template differed from founder publication",
        ));
    }
    Ok(bytes)
}

fn evidence_key_v1(
    founding: &crate::market::MarketExecutionEvidence,
    label: &str,
) -> Result<Pubkey> {
    founding
        .accounts
        .get(label)
        .ok_or_else(|| {
            Error::new(format!(
                "Series root activation founding report omitted {label}"
            ))
        })
        .and_then(|evidence| pubkey(&evidence.address))
}

fn pair_v1(
    registry: Pubkey,
    schema: [u8; 32],
    body: &[u8],
) -> Result<SelectedActivationRecordPairV1> {
    pair_from_id_v1(registry, schema, Sha256::digest(body).into())
}

fn pair_from_id_v1(
    registry: Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<SelectedActivationRecordPairV1> {
    let bumps = record_bumps_v1(registry, schema, content, "Series activation record")?;
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Series activation schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("Series activation content: {error:?}")))?,
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
        .0
    };
    Ok(SelectedActivationRecordPairV1 {
        raw: derive(key.raw_record_pda_seeds()),
        staging: derive(key.staging_cursor_pda_seeds()),
        schema,
        content,
        bumps,
    })
}

/// Persisted authoring facts for the immutable two-occurrence Series founder.
///
/// The source Market determines the M0 Product, Realm, Source and Direct
/// manifest.  These fields deliberately cover only the facts the Template
/// author owns: generator identities, schedule, and the two separately
/// committed funding partitions.  Keeping the boundary narrow prevents a
/// campaign document from substituting the child Market's canonical bodies.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesFounderAuthoringInputV1 {
    pub(crate) product_generator: String,
    pub(crate) occurrence_generator: String,
    pub(crate) capability_template: String,
    pub(crate) product_derivation: String,
    pub(crate) occurrence_derivation: String,
    pub(crate) capability_derivation: String,
    pub(crate) funding_derivation: String,
    pub(crate) first_slot: u64,
    pub(crate) period_slots: u64,
    pub(crate) retry_window: u64,
    pub(crate) close_rent: u64,
    pub(crate) occurrences: [SeriesOccurrenceAuthoringInputV1; 2],
}

/// One exact M0 occurrence funding partition.  These are principal and
/// pre-paid non-principal amounts, not a caller-selected future account.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesOccurrenceAuthoringInputV1 {
    pub(crate) funding_list: String,
    pub(crate) hoard_principal: u64,
    pub(crate) market_rent: u64,
    pub(crate) capability_native: u64,
    pub(crate) founding_work: u64,
}

/// Decode only canonical content identities and the Series owner's bounded
/// funding constructor.  A live driver calls this before
/// `prepare_series_founder_from_market_v1`; no test fixture identity crosses
/// that boundary.
pub(crate) fn decode_series_founder_authoring_v1(
    input: &SeriesFounderAuthoringInputV1,
) -> Result<(
    crate::series_founder::SeriesTemplatePolicyV1,
    [crate::series_founder::SeriesOccurrenceFundingV1; 2],
)> {
    let id = |value: &str, label: &str| {
        ContentId::new(hex32(value)?).map_err(|_| Error::new(format!("Series {label} identity")))
    };
    let policy = crate::series_founder::SeriesTemplatePolicyV1 {
        product_generator: id(&input.product_generator, "product generator")?,
        occurrence_generator: id(&input.occurrence_generator, "occurrence generator")?,
        capability_template: id(&input.capability_template, "capability template")?,
        product_derivation: id(&input.product_derivation, "product derivation")?,
        occurrence_derivation: id(&input.occurrence_derivation, "occurrence derivation")?,
        capability_derivation: id(&input.capability_derivation, "capability derivation")?,
        funding_derivation: id(&input.funding_derivation, "funding derivation")?,
        first_slot: input.first_slot,
        period_slots: input.period_slots,
        retry_window: input.retry_window,
        close_rent: input.close_rent,
    };
    let decode_occurrence = |occurrence: &SeriesOccurrenceAuthoringInputV1| -> Result<crate::series_founder::SeriesOccurrenceFundingV1> {
        Ok(crate::series_founder::SeriesOccurrenceFundingV1 {
            funding_list: id(&occurrence.funding_list, "funding list")?,
            funds: dclutch_trading::series::FoundingFundsV3::new(
                occurrence.hoard_principal,
                occurrence.market_rent,
                occurrence.capability_native,
                occurrence.founding_work,
            )
            .map_err(|error| Error::new(format!("Series occurrence funding: {error:?}")))?,
        })
    };
    let [first, second] = &input.occurrences;
    let funding = [decode_occurrence(first)?, decode_occurrence(second)?];
    Ok((policy, funding))
}

#[cfg(test)]
mod authoring_tests {
    use super::*;

    fn input() -> SeriesFounderAuthoringInputV1 {
        let identity = |byte| format!("{byte:02x}").repeat(32);
        SeriesFounderAuthoringInputV1 {
            product_generator: identity(1),
            occurrence_generator: identity(2),
            capability_template: identity(3),
            product_derivation: identity(4),
            occurrence_derivation: identity(5),
            capability_derivation: identity(6),
            funding_derivation: identity(7),
            first_slot: 100,
            period_slots: 10,
            retry_window: 2,
            close_rent: 1,
            occurrences: [
                SeriesOccurrenceAuthoringInputV1 {
                    funding_list: identity(8),
                    hoard_principal: 9,
                    market_rent: 2,
                    capability_native: 3,
                    founding_work: 4,
                },
                SeriesOccurrenceAuthoringInputV1 {
                    funding_list: identity(9),
                    hoard_principal: 18,
                    market_rent: 2,
                    capability_native: 3,
                    founding_work: 4,
                },
            ],
        }
    }

    #[test]
    fn authoring_decoder_keeps_two_funding_partitions_distinct() {
        let (_, funding) = decode_series_founder_authoring_v1(&input()).unwrap();
        assert_eq!(funding[0].funds.hoard_principal(), 9);
        assert_eq!(funding[1].funds.hoard_principal(), 18);
    }

    #[test]
    fn authoring_decoder_refuses_noncanonical_identity() {
        let mut hostile = input();
        hostile.funding_derivation = "aa".into();
        assert!(decode_series_founder_authoring_v1(&hostile).is_err());
    }
}

/// One finality-bound source observation used to author the local scenario.
/// The rent schedule is kept as its canonical sysvar bytes until the caller
/// has checked both its owner and its bincode round-trip.
#[derive(Clone, Debug)]
pub(crate) struct SeriesLocalScenarioObservationV1 {
    pub(crate) finalized_slot: u64,
    pub(crate) rent: solana_program::rent::Rent,
}

/// A finalized token source observation that the first SeriesEscrow Lock will
/// consume.  The scenario derives both occurrence principals from `amount`,
/// never from a descriptive Market-input scalar.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesFounderSourceObservationV1 {
    pub(crate) source: Pubkey,
    pub(crate) amount: u64,
}

/// Read and authenticate the concrete external Token-2022 source before it
/// becomes the first occurrence's SeriesEscrow Lock source.
pub(crate) fn observe_series_founder_source_v1(
    rpc: &mut Rpc,
    source: Pubkey,
    mint: Pubkey,
    founder: Pubkey,
) -> Result<SeriesFounderSourceObservationV1> {
    if source == Pubkey::default() || mint == Pubkey::default() || founder == Pubkey::default() {
        return Err(Error::new(
            "Series founder source observation named a default key",
        ));
    }
    let account = rpc.required_account(source, "Series founder token source")?;
    let amount = authenticate_series_founder_source_account_v1(&account, mint, founder)?;
    Ok(SeriesFounderSourceObservationV1 { source, amount })
}

fn authenticate_series_founder_source_account_v1(
    account: &RpcAccount,
    mint: Pubkey,
    founder: Pubkey,
) -> Result<u64> {
    use dclutch_custody::token_svm::{AccountState, TOKEN_2022_PROGRAM_ID, TokenAccount};

    let token = TokenAccount::parse(&account.data)
        .map_err(|error| Error::new(format!("Series founder token source: {error:?}")))?;
    if account.owner != Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
        || account.executable
        || token.mint != mint.to_bytes()
        || token.owner != founder.to_bytes()
        || token.state != AccountState::Initialized
        || token.amount == 0
    {
        return Err(Error::new(
            "Series founder source did not authenticate as an initialized founder Token-2022 account",
        ));
    }
    Ok(token.amount)
}

#[cfg(test)]
mod founder_source_tests {
    use dclutch_custody::token_svm::{TOKEN_2022_PROGRAM_ID, TokenAccount};

    use super::*;

    fn account(mint: Pubkey, founder: Pubkey) -> RpcAccount {
        let initial = TokenAccount::initialized_base_bytes(mint.to_bytes(), founder.to_bytes())
            .expect("canonical base token account");
        let data = TokenAccount::project_amount_poststate(&initial, 41)
            .expect("canonical token amount poststate");
        RpcAccount {
            lamports: 1,
            owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
            data: data.to_vec(),
        }
    }

    #[test]
    fn founder_source_requires_the_named_mint_owner_and_nonzero_balance() {
        let mint = Pubkey::new_unique();
        let founder = Pubkey::new_unique();
        assert_eq!(
            authenticate_series_founder_source_account_v1(&account(mint, founder), mint, founder)
                .expect("authenticated founder source"),
            41
        );

        let mut hostile = account(mint, founder);
        hostile.owner = Pubkey::new_unique();
        let error = authenticate_series_founder_source_account_v1(&hostile, mint, founder)
            .expect_err("non-Token-2022 owner must refuse");
        assert_eq!(
            error.to_string(),
            "Series founder source did not authenticate as an initialized founder Token-2022 account"
        );
    }
}

/// Read the exact finality point and Rent schedule that bound the first two
/// occurrence partitions.  This is intentionally separate from the scenario
/// constructor so the later M1 compiler can retain the slot it was based on.
pub(crate) fn observe_local_series_scenario_v1(
    rpc: &mut Rpc,
) -> Result<SeriesLocalScenarioObservationV1> {
    let finalized_slot = rpc.finalized_slot()?;
    if finalized_slot == 0 {
        return Err(Error::new("Series local scenario observed slot zero"));
    }
    let account = rpc.required_account(solana_sdk_ids::sysvar::rent::ID, "Rent sysvar")?;
    if account.owner != solana_sdk_ids::sysvar::ID || account.executable {
        return Err(Error::new(
            "Series local scenario Rent sysvar owner or executable bit changed",
        ));
    }
    let rent: solana_program::rent::Rent = bincode::deserialize(&account.data)
        .map_err(|error| Error::new(format!("Series local scenario Rent sysvar: {error}")))?;
    let encoded = bincode::serialize(&rent)
        .map_err(|error| Error::new(format!("Series local scenario Rent encoding: {error}")))?;
    if encoded != account.data {
        return Err(Error::new(
            "Series local scenario Rent sysvar was not canonical",
        ));
    }
    Ok(SeriesLocalScenarioObservationV1 {
        finalized_slot,
        rent,
    })
}

/// Author the one concrete local-validator Series scenario from the M0 Direct
/// manifest, current Rent quote, and a finalized slot.  Every generator and
/// derivation identity is domain-separated from that exact M0 manifest; no
/// repeated-byte fixture identity is admitted.  Both occurrence partitions
/// split the complete collateral-token source while naming native account rent
/// in separate funding compartments paid by the transaction fee payer.
pub(crate) fn local_series_founder_scenario_v1(
    plan: &SuccessorPlan,
    child_market: &MarketRunInput,
    collateral_mint: Pubkey,
    founder_source_amount: u64,
    rent: &solana_program::rent::Rent,
    finalized_slot: u64,
) -> Result<(
    crate::series_founder::SeriesTemplatePolicyV1,
    [crate::series_founder::SeriesOccurrenceFundingV1; 2],
)> {
    if finalized_slot == 0 {
        return Err(Error::new(
            "Series local scenario requires a finalized slot",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let preview = crate::market::compile_market_publication_preview_v1(
        registry,
        child_market,
        collateral_mint,
    )?;
    let manifest = record_identity(&preview.manifest);
    let identity = |label: &[u8]| -> Result<ContentId> {
        let mut digest = Sha256::new();
        digest.update(b"dclutch/local-validator/series-founder-scenario/v1\0");
        digest.update(label);
        digest.update([0]);
        digest.update(manifest);
        ContentId::new(digest.finalize().into())
            .map_err(|_| Error::new("Series local scenario identity was zero"))
    };
    let market_rent = rent.minimum_balance(dclutch_market::STATE_BYTES);
    let capability_native = rent.minimum_balance(
        dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    );
    let founding_work =
        rent.minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3);
    let [first_principal, second_principal] =
        split_series_hoard_principal_v1(founder_source_amount)?;
    let funding = |label: &[u8],
                   hoard_principal|
     -> Result<crate::series_founder::SeriesOccurrenceFundingV1> {
        Ok(crate::series_founder::SeriesOccurrenceFundingV1 {
            funding_list: identity(label)?,
            funds: dclutch_trading::series::FoundingFundsV3::new(
                hoard_principal,
                market_rent,
                capability_native,
                founding_work,
            )
            .map_err(|error| Error::new(format!("Series local scenario funding: {error:?}")))?,
        })
    };
    let first_slot = finalized_slot
        .checked_add(8)
        .ok_or_else(|| Error::new("Series local scenario first slot overflow"))?;
    Ok((
        crate::series_founder::SeriesTemplatePolicyV1 {
            product_generator: identity(b"product-generator")?,
            occurrence_generator: identity(b"occurrence-generator")?,
            capability_template: identity(b"capability-template")?,
            product_derivation: identity(b"product-derivation")?,
            occurrence_derivation: identity(b"occurrence-derivation")?,
            capability_derivation: identity(b"capability-derivation")?,
            funding_derivation: identity(b"funding-derivation")?,
            first_slot,
            period_slots: 32,
            retry_window: 16,
            close_rent: founding_work,
        },
        [
            funding(b"occurrence-0", first_principal)?,
            funding(b"occurrence-1", second_principal)?,
        ],
    ))
}

fn split_series_hoard_principal_v1(founder_source_amount: u64) -> Result<[u64; 2]> {
    let first = founder_source_amount / 2;
    let second = founder_source_amount
        .checked_sub(first)
        .ok_or_else(|| Error::new("Series local scenario principal partition overflow"))?;
    if first == 0 || second == 0 {
        return Err(Error::new(
            "Series local scenario leaves a zero child Hoard principal",
        ));
    }
    Ok([first, second])
}

/// Prepare the canonical two-leaf M0 founder from a live validator snapshot.
/// This is the first production source hydrator: it joins the child Direct
/// Market bytes, real collateral Mint, authenticated founder source balance,
/// on-chain Rent, and current slot before any Template or occurrence bytes
/// are generated.
pub(crate) fn prepare_local_series_founder_from_market_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    child_market: &MarketRunInput,
    collateral_mint: Pubkey,
    founder: Pubkey,
    refund_owner: Pubkey,
    founder_source: Pubkey,
) -> Result<(
    crate::series_founder::PreparedSeriesFounderV1,
    SeriesLocalScenarioObservationV1,
)> {
    if collateral_mint == Pubkey::default()
        || founder == Pubkey::default()
        || refund_owner == Pubkey::default()
        || founder_source == Pubkey::default()
    {
        return Err(Error::new(
            "Series local founder named a default Mint or actor",
        ));
    }
    let observation = observe_local_series_scenario_v1(rpc)?;
    let source = observe_series_founder_source_v1(rpc, founder_source, collateral_mint, founder)?;
    let (policy, funding) = local_series_founder_scenario_v1(
        plan,
        child_market,
        collateral_mint,
        source.amount,
        &observation.rent,
        observation.finalized_slot,
    )?;
    let prepared = crate::series_founder::prepare_series_founder_from_market_v1(
        plan,
        child_market,
        collateral_mint,
        founder,
        refund_owner,
        policy,
        funding,
    )?;
    Ok((prepared, observation))
}

/// Build the first Consume Shadow certificate before the selected parent has
/// published its ProgramSet and descriptor records.
///
/// The full observed source operator intentionally needs those finalized M1
/// records and therefore runs after Found as a cross-check.  First selection
/// instead derives the descriptor's semantic coordinates from the same
/// canonical Series owners used by `encode_series_action_descriptor_v5`, then
/// hands the opaque Consume child bank and live Profile13 widths to the
/// preselection producer.  The caller has already authenticated the checked
/// accelerator release and evidence bytes used to form `release_sources`;
/// this boundary accepts no text-form certificate identity.
pub(crate) fn build_series_shadow_preselection_v1(
    occurrence_count: u32,
    fixed_data_lengths: &[u32;
        dclutch_series_shadow_bundle_generator::SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    funding_count: u32,
    evidence: &crate::series_checked_evidence::CheckedSeriesShadowEvidenceV1,
) -> Result<dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1> {
    use dclutch_series_shadow_bundle_generator::SeriesShadowReleaseSourcesV4;

    let built = build_series_shadow_preselection_from_sources_v1(
        occurrence_count,
        fixed_data_lengths,
        funding_count,
        SeriesShadowReleaseSourcesV4 {
            semantic_source: include_bytes!(
                "../../../../../programs/dclutch-trading-sbf/src/series/consume_artifacts_v4.rs"
            ),
            compiler_source: &evidence.compiler_manifest,
            toolchain_manifest: &evidence.toolchain,
            accelerator_semantic_release: evidence.accelerator_semantic_release,
            translation_validation: evidence.translation_validation,
        },
    )?;
    // The generator owns the certificate wire, while the evidence owner owns
    // every checked source/build join.  Authenticate both before these bytes
    // can be written as an accelerator include or finalized in Registry.
    crate::series_checked_evidence::authenticate_series_shadow_generated_v1(evidence, &built)?;
    Ok(built)
}

fn build_series_shadow_preselection_from_sources_v1(
    occurrence_count: u32,
    fixed_data_lengths: &[u32;
        dclutch_series_shadow_bundle_generator::SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    funding_count: u32,
    release_sources: dclutch_series_shadow_bundle_generator::SeriesShadowReleaseSourcesV4<'_>,
) -> Result<dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1> {
    use dclutch_series_shadow_bundle_generator::{
        SeriesShadowBundleSourceV4, SeriesShadowDescriptorSemanticsV4,
        build_series_shadow_preselection_v1 as build,
    };
    use dclutch_trading::series::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
        SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_DERIVATION_PREIMAGE_V3, replay::SERIES_STATE_BYTES_V3,
    };

    let identity = |bytes: [u8; 32], label: &str| {
        ContentId::new(bytes).map_err(|_| Error::new(format!("Series Shadow {label} identity")))
    };
    let descriptor = SeriesShadowDescriptorSemanticsV4 {
        kind: identity(
            Sha256::digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).into(),
            "kind",
        )?,
        config_schema: identity(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, "Template schema")?,
        request_schema: identity(
            Sha256::digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).into(),
            "request schema",
        )?,
        root_schema: identity(
            Sha256::digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3).into(),
            "root schema",
        )?,
        derivation_policy: identity(
            Sha256::digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3).into(),
            "Ticket derivation",
        )?,
        capacity_profile:
            dclutch_trading_sbf::series::release_v5::series_consume_capacity_profile_v1(
                fixed_data_lengths,
                funding_count,
                occurrence_count,
            )
            .map_err(|error| Error::new(format!("Series Shadow capacity: {error:?}")))?,
        root_state_bytes: u32::try_from(SERIES_STATE_BYTES_V3)
            .map_err(|_| Error::new("Series Shadow root state width escaped u32"))?,
    };
    // Consume is the only first-accelerated action. Its release owner emits
    // this empty policy directly; it depends on no certificate or parent-root
    // identity, so it can be derived before first selected publication.
    let lifecycle = dclutch_trading_sbf::series::release_v5::series_consume_lifecycle_v5()
        .map_err(|error| Error::new(format!("Series Shadow Consume lifecycle: {error:?}")))?;
    let built = build(SeriesShadowBundleSourceV4 {
        descriptor,
        release_sources,
        lifecycle: &lifecycle,
        fixed_data_lengths,
        funding_count,
        occurrence_count,
    })
    .map_err(|error| Error::new(format!("Series Shadow preselection refused: {error:?}")))?;
    let certificate: [u8; 32] = Sha256::digest(built.certificate).into();
    if certificate != built.build_inputs.certificate.to_bytes() {
        return Err(Error::new(
            "Series Shadow preselection certificate hash changed before Registry finalization",
        ));
    }
    Ok(built)
}

/// Build one explicitly unpublishable selected-accelerator input from the
/// full live Consume geometry observed by the production founder path.
///
/// This is the bootstrap half of the schedule-independent capacity cycle:
/// the resulting executor is built before the later command observes and
/// authors its actual Template schedule. The normal execution mode decodes
/// this manifest again and requires every live width and funding span to
/// match before it founds the selected parent.
fn build_series_shadow_diagnostic_preselection_v1(
    occurrence_count: u32,
    fixed_data_lengths: &[u32;
        dclutch_series_shadow_bundle_generator::SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    funding_count: u32,
) -> Result<dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1> {
    use dclutch_series_shadow_bundle_generator::SeriesShadowReleaseSourcesV4;

    if occurrence_count != 2
        || funding_count == 0
        || fixed_data_lengths.iter().all(|width| *width == 0)
    {
        return Err(Error::new(
            "Series diagnostic executor requires funded two-occurrence live geometry",
        ));
    }
    const COMPILER_SOURCE: &[u8] = concat!(
        include_str!("../../../../../programs/dclutch-accelerator-sbf/generator/src/lib.rs"),
        include_str!("../../../../../programs/dclutch-accelerator-sbf/generator/src/manifest.rs"),
        include_str!(
            "../../../../../programs/dclutch-accelerator-sbf/generator/src/source_operator.rs"
        ),
    )
    .as_bytes();
    const TOOLCHAIN: &[u8] = b"unpublishable-series-runtime-diagnostic-toolchain-v1";
    const TRANSLATION: &[u8] = b"unpublishable-series-runtime-diagnostic-translation-v1";
    let translation_validation = ContentId::new(Sha256::digest(TRANSLATION).into())
        .map_err(|_| Error::new("Series diagnostic translation identity"))?;
    build_series_shadow_preselection_from_sources_v1(
        occurrence_count,
        fixed_data_lengths,
        funding_count,
        SeriesShadowReleaseSourcesV4 {
            semantic_source: include_bytes!(
                "../../../../../programs/dclutch-trading-sbf/src/series/consume_artifacts_v4.rs"
            ),
            compiler_source: COMPILER_SOURCE,
            toolchain_manifest: TOOLCHAIN,
            accelerator_semantic_release:
                series_shadow_diagnostic_accelerator_semantic_release_v1(),
            translation_validation,
        },
    )
}

fn write_series_shadow_diagnostic_v1(
    output: &std::path::Path,
    built: &dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1,
) -> Result<()> {
    use dclutch_series_shadow_bundle_generator::SeriesShadowSourceManifestV1;

    let decoded = SeriesShadowSourceManifestV1::decode(&built.manifest)
        .map_err(|error| Error::new(format!("Series diagnostic manifest: {error:?}")))?;
    if decoded.occurrence_count() != 2
        || decoded.funding_count() == 0
        || decoded.fixed_data_lengths().iter().all(|width| *width == 0)
        || decoded.generated_bundle().certificate_program != built.build_inputs.certificate
    {
        return Err(Error::new(
            "Series diagnostic output lost its funded recurring capacity",
        ));
    }
    fs::create_dir(output)?;
    let manifest_temporary = output.join(".series_shadow_source_manifest.bin.tmp");
    let include_temporary = output.join(".series_shadow_generated.rs.tmp");
    let semantic_temporary = output.join(".accelerator_semantic_preimage.bin.tmp");
    let marker_temporary = output.join(".UNPUBLISHABLE_DIAGNOSTIC_ONLY.tmp");
    fs::write(&manifest_temporary, &built.manifest)?;
    fs::write(&include_temporary, &built.generated_include)?;
    fs::write(
        &semantic_temporary,
        SERIES_SHADOW_DIAGNOSTIC_SEMANTIC_PREIMAGE_V1,
    )?;
    fs::write(
        &marker_temporary,
        b"Generated from live two-occurrence funded Series geometry. This directory is not checked release evidence.\n",
    )?;
    fs::rename(
        manifest_temporary,
        output.join("series_shadow_source_manifest.bin"),
    )?;
    fs::rename(include_temporary, output.join("series_shadow_generated.rs"))?;
    fs::rename(
        semantic_temporary,
        output.join("accelerator_semantic_preimage.bin"),
    )?;
    fs::rename(
        marker_temporary,
        output.join("UNPUBLISHABLE_DIAGNOSTIC_ONLY"),
    )?;
    let hex = |identity: ContentId| {
        identity
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    println!(
        "series_shadow_source_manifest {}",
        hex(built.build_inputs.source_manifest)
    );
    println!(
        "series_shadow_generated_include {}",
        hex(built.build_inputs.generated_include)
    );
    println!(
        "series_shadow_certificate {}",
        hex(built.build_inputs.certificate)
    );
    println!(
        "series_shadow_accelerator_semantic_release {}",
        hex(series_shadow_diagnostic_accelerator_semantic_release_v1())
    );
    println!("series_shadow_diagnostic_dir {}", output.display());
    Ok(())
}

/// One Registry raw/staging pair that has already reached finality.  The
/// hydrator retains the canonical body so geometry can prove both the raw
/// address and its width, rather than treating a record as an arbitrary
/// Registry-owned account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesPrepareFinalizedRecordV1<'a> {
    pub(crate) schema: [u8; 32],
    pub(crate) body: &'a [u8],
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

/// A non-record M0 ProjectFound coordinate read at finality.  M0's raw
/// records are represented by [`SeriesPrepareFinalizedRecordV1`] instead;
/// splitting them prevents a caller from claiming that a byte-identical
/// Registry account is the raw PDA for a different record identity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesPrepareFinalizedAccountV1 {
    pub(crate) address: Pubkey,
    pub(crate) expected_owner: Pubkey,
}

/// The finalized M0 frame forwarded by projected-Custody's Initialize child.
/// It is the ordinary Core `ProjectFound36` order, with the Rent sysvar
/// omitted by Core.  The M0 Market remains a vacant coordinate: Prepare never
/// creates Core state, and Consume's later Found is its only creator.
#[derive(Clone, Copy)]
pub(crate) struct SeriesPrepareM0FrameV1<'a> {
    pub(crate) project_found: [Pubkey; dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
    pub(crate) records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    /// ProjectFound record coordinates that the M0 publisher proved vacant.
    /// They stay zero-width vacancies through Prepare; this is distinct from
    /// M0 Core and every future Custody state, which have their own owners.
    pub(crate) vacancies: &'a [Pubkey],
    pub(crate) finalized_accounts: &'a [SeriesPrepareFinalizedAccountV1],
}

/// Borrow the exact M0 Registry records the canonical publisher finalized.
/// This is the only bridge from Market's typed publication owner into Series
/// Prepare: it copies no body and recomputes no record identity.
pub(crate) fn series_prepare_records_from_m0_publication_v1<'a>(
    publication: &'a FutureMarketImmutablePublicationV1,
) -> Vec<SeriesPrepareFinalizedRecordV1<'a>> {
    publication
        .series_prepare_records
        .iter()
        .map(|record| SeriesPrepareFinalizedRecordV1 {
            schema: record.published.schema,
            body: &record.body,
            raw: record.published.raw,
            staging: record.published.staging,
        })
        .collect()
}

/// Select one M0 record from the publisher's exact finalized closure.
///
/// The source compiler already owns the expected schema and body.  This
/// lookup refuses an absent, duplicated, or byte-substituted record instead
/// of deriving a fresh pair from a parallel Market preview.
pub(crate) fn series_prepare_record_from_m0_publication_v1<'a>(
    records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    schema: [u8; 32],
    body: &[u8],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let matches = records
        .iter()
        .copied()
        .filter(|record| record.schema == schema && record.body == body)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(*record),
        [] => Err(Error::new(format!(
            "Series Prepare M0 publisher omitted canonical {label} record"
        ))),
        _ => Err(Error::new(format!(
            "Series Prepare M0 publisher duplicated canonical {label} record"
        ))),
    }
}

/// Convert a finalized Series founder publication to the hydratable pair,
/// retaining the exact source-owner body for the later Registry readback.
pub(crate) fn series_prepare_founder_record_v1<'a>(
    published: crate::runtime::PublishedRecord,
    schema: [u8; 32],
    body: &'a [u8],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let digest: [u8; 32] = Sha256::digest(body).into();
    if published.schema != schema || published.digest != digest {
        return Err(Error::new(format!(
            "Series Prepare founder publication changed canonical {label} schema or body"
        )));
    }
    Ok(SeriesPrepareFinalizedRecordV1 {
        schema,
        body,
        raw: published.raw,
        staging: published.staging,
    })
}

/// Assemble the M0 half of the hydrator input directly from the one canonical
/// publisher result.  The caller supplies only the non-record accounts it
/// observed at the same finality floor; no future Core or Custody account can
/// enter this frame through that list.
pub(crate) fn series_prepare_m0_frame_from_publication_v1<'a>(
    publication: &'a FutureMarketImmutablePublicationV1,
    records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    finalized_accounts: &'a [SeriesPrepareFinalizedAccountV1],
) -> SeriesPrepareM0FrameV1<'a> {
    SeriesPrepareM0FrameV1 {
        project_found: publication.project_found,
        records,
        vacancies: &publication.series_prepare_vacancies,
        finalized_accounts,
    }
}

/// Inputs that turn the semantic-owner child bank into the complete physical
/// first-Prepare layout.  M1 is the active Series parent (`parent_root`),
/// while the ProjectFound frame and Template leaves are M0 child facts.
/// Whether M1's Series root is the pre-activation fixed-width projection or
/// the finalized selector-255 account.  The compiler needs the first form to
/// break the descriptor/root cycle; Prepare itself always uses `Finalized`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesPrepareParentRootStateV1 {
    PreActivationPredicted,
    Finalized,
}

pub(crate) struct SeriesPrepareHydratorInputV1<'a> {
    pub(crate) registry: Pubkey,
    pub(crate) core: Pubkey,
    pub(crate) trading: Pubkey,
    pub(crate) custody: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) parent_root: Pubkey,
    pub(crate) parent_root_state: SeriesPrepareParentRootStateV1,
    pub(crate) m0: SeriesPrepareM0FrameV1<'a>,
    pub(crate) template: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) occurrence: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) ticket: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) portfolio: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) children: &'a dclutch_operator::series_child_bank_v1::SeriesChildBankV1,
}

/// The four immutable records whose roles are outside M0's ordinary
/// `ProjectFound` subframe.  They are still carried as canonical Registry
/// pairs: Template, occurrence and Ticket are founder facts, while Portfolio
/// is the Product graph record which the M0 publisher finalized.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesPrepareHydrationRecordsV1<'a> {
    pub(crate) template: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) occurrence: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) ticket: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) portfolio: SeriesPrepareFinalizedRecordV1<'a>,
}

/// Materialize the current source compiler's opaque Prepare child bank into
/// the full 116-role geometry at one finalized slot.
///
/// This is the production connection between the semantic owners and the
/// hydrator.  The caller first invokes it with the release-owned predicted
/// M1 root, then activates that same parent and invokes it again with
/// [`SeriesPrepareParentRootStateV1::Finalized`].  The caller owns the
/// invariance comparison because it is the only layer that sees both
/// observations; this helper owns neither an alternate request bank nor a
/// second geometry representation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn observe_series_prepare_preprofile_geometry_v1(
    rpc: &mut Rpc,
    selection: &mut crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'_>,
    m0: SeriesPrepareM0FrameV1<'_>,
    records: SeriesPrepareHydrationRecordsV1<'_>,
    parent_root_state: SeriesPrepareParentRootStateV1,
    minimum_slot: u64,
) -> Result<SeriesPrepareFixedDataLengthsV1> {
    // The preprofile is the only producer of Prepare children.  Do not admit
    // a caller-authored child byte bank merely to obtain geometry.
    selection.geometry = None;
    let preprofile =
        crate::series_found_prepare_campaign::derive_series_found_prepare_preprofile_v1(selection)?;
    let hydrator = SeriesPrepareHydratorInputV1 {
        registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
        core: selection.material.core,
        trading: selection.material.trading,
        custody: selection.material.custody,
        rent_program: selection.material.rent_program,
        parent_root: selection.material.parent_root,
        parent_root_state,
        m0,
        template: records.template,
        occurrence: records.occurrence,
        ticket: records.ticket,
        portfolio: records.portfolio,
        children: &preprofile.prepare_children,
    };
    let layout = hydrate_series_prepare_role_layout_v1(&hydrator)?;
    observe_series_prepare_geometry_v1(rpc, &layout, minimum_slot)
}

/// Compile the selected Series closure only after the Prepare profile has been
/// replaced by a full finalized M0/M1 observation.
///
/// Expire and Consume are likewise derived from their semantic child bank and
/// the same finalized M0/M1 facts. Consume's phase states whether its
/// Prepare-created Custody accounts are canonical predictions for selection or
/// finalized facts after the actual Prepare transaction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_series_prepare_from_hydrated_geometry_v1(
    rpc: &mut Rpc,
    mut selection: crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'_>,
    consume_shadow_certificate_program: ContentId,
    m0: SeriesPrepareM0FrameV1<'_>,
    records: SeriesPrepareHydrationRecordsV1<'_>,
    parent_root_state: SeriesPrepareParentRootStateV1,
    parent_root_fact: crate::series_found_prepare_input::SeriesParentRootFactV1,
    consume_prestate: crate::series_consume_geometry::SeriesConsumePrestateV1,
    minimum_slot: u64,
) -> Result<crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1> {
    if selection.geometry.is_some() {
        return Err(Error::new(
            "Series selected compilation refuses caller-supplied action geometry",
        ));
    }
    let preprofile =
        crate::series_found_prepare_campaign::derive_series_found_prepare_preprofile_v1(
            &mut selection,
        )?;
    let expected_root = match parent_root_fact {
        crate::series_found_prepare_input::SeriesParentRootFactV1::Predicted(prediction) => (
            prediction.root,
            SeriesPrepareParentRootStateV1::PreActivationPredicted,
        ),
        crate::series_found_prepare_input::SeriesParentRootFactV1::Finalized { root, .. } => {
            (root, SeriesPrepareParentRootStateV1::Finalized)
        }
    };
    if selection.material.parent_root != expected_root.0 || parent_root_state != expected_root.1 {
        return Err(Error::new(
            "Series Prepare root state disagreed with its typed geometry fact",
        ));
    }
    let consume_fixed_data_lengths =
        crate::series_consume_geometry::derive_series_consume_fixed_data_lengths_v1(
            rpc,
            crate::series_consume_geometry::SeriesConsumeGeometryInputV1 {
                preprofile: &preprofile,
                m0,
                records,
                parent_root: parent_root_fact,
                registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
                core: selection.material.core,
                trading: selection.material.trading,
                custody: selection.material.custody,
                claims: selection.material.claims,
                rent_program: selection.material.rent_program,
                prestate: consume_prestate,
                minimum_slot,
            },
        )?;
    let expire_fixed_data_lengths =
        crate::series_expire_geometry::derive_series_expire_fixed_data_lengths_v1(
            rpc,
            crate::series_expire_geometry::SeriesExpireGeometryInputV1 {
                preprofile: &preprofile,
                m0,
                records,
                parent_root: parent_root_fact,
                registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
                trading: selection.material.trading,
                custody: selection.material.custody,
                minimum_slot,
            },
        )?;
    let geometry = crate::series_source::SeriesObservedGeometryV1 {
        prepare_fixed_data_lengths: observe_series_prepare_preprofile_geometry_v1(
            rpc,
            &mut selection,
            m0,
            records,
            parent_root_state,
            minimum_slot,
        )?,
        prepare_ticket_rent_lamports: selection
            .material
            .rent
            .minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3),
        consume_fixed_data_lengths,
        consume_funding_count: u32::from(selection.funding_ledger_slot_count),
        expire_fixed_data_lengths,
    };
    selection.geometry = Some(geometry);
    crate::series_found_prepare_campaign::compile_series_found_prepare_selection_v1(
        selection,
        consume_shadow_certificate_program,
    )
}

/// Hydrate all 116 Series Prepare roles from the decoded semantic-owner child
/// requests.  This is intentionally a layout constructor, not an RPC reader:
/// `observe_series_prepare_geometry_v1` performs the one finalized snapshot
/// after this function has made every address, owner, canonical record body,
/// and permitted vacancy explicit.
pub(crate) fn hydrate_series_prepare_role_layout_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
) -> Result<SeriesPrepareRoleLayoutV1<'a>> {
    use dclutch_custody::{CustodyRequestV1, ProjectedCustodyRequestV1};

    let children = input.children.prepare_requests();
    let projected_initialize = ProjectedCustodyRequestV1::decode(children.projected_initialize)
        .map_err(|error| {
            Error::new(format!(
                "Series Prepare projected Initialize decode: {error:?}"
            ))
        })?;
    let projected_open =
        ProjectedCustodyRequestV1::decode(children.projected_open).map_err(|error| {
            Error::new(format!(
                "Series Prepare projected OpenHoard decode: {error:?}"
            ))
        })?;
    let replay_initialize =
        CustodyRequestV1::decode(children.replay_initialize).map_err(|error| {
            Error::new(format!(
                "Series Prepare replay Initialize decode: {error:?}"
            ))
        })?;
    let escrow_open = CustodyRequestV1::decode(children.escrow_open)
        .map_err(|error| Error::new(format!("Series Prepare escrow Open decode: {error:?}")))?;
    let escrow_lock = CustodyRequestV1::decode(children.escrow_lock)
        .map_err(|error| Error::new(format!("Series Prepare escrow Lock decode: {error:?}")))?;

    require_prepare_projected_pair_v1(
        projected_initialize,
        projected_open,
        input.parent_root,
        input.trading,
        input.core,
        input.rent_program,
    )?;
    for request in [replay_initialize, escrow_open, escrow_lock] {
        require_prepare_custody_request_v1(request, input)?;
    }

    let root = parent_root_source_v1(input.parent_root, input.parent_root_state, input.trading)?;
    let product = m0_record_by_schema_v1(
        input,
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
        "Product",
    )?;
    let basis = m0_record_by_schema_v1(
        input,
        dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        "linked Basis",
    )?;
    let outer = [
        root,
        record_raw_v1("Series Template", input.registry, input.template)?,
        record_raw_v1("M0 Product", input.registry, product)?,
        record_raw_v1("M0 Portfolio", input.registry, input.portfolio)?,
        record_raw_v1("M0 linked Basis", input.registry, basis)?,
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "Series Ticket replay",
            address: ticket_state_address_v1(input)?,
            fixed_data_len: u32::try_from(
                dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3,
            )
            .map_err(|_| Error::new("Series Ticket replay width escaped u32"))?,
        },
    ];
    let projected_initialize =
        projected_frame_v1::<47>(input, projected_initialize, children.projected_initialize)?;
    let projected_open = projected_frame_v1::<15>(input, projected_open, children.projected_open)?;
    let replay_initialize =
        custody_frame_v1::<13>(input, replay_initialize, children.replay_initialize)?;
    let escrow_open = custody_frame_v1::<16>(input, escrow_open, children.escrow_open)?;
    let escrow_lock = custody_frame_v1::<14>(input, escrow_lock, children.escrow_lock)?;
    Ok(SeriesPrepareRoleLayoutV1 {
        outer,
        projected_initialize,
        projected_open,
        replay_initialize,
        escrow_open,
        escrow_lock,
        occurrence_evidence: [
            record_raw_v1("Series occurrence", input.registry, input.occurrence)?,
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Series occurrence staging",
                address: input.occurrence.staging,
                fixed_data_len: 0,
            },
            record_raw_v1("Series Ticket", input.registry, input.ticket)?,
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Series Ticket staging",
                address: input.ticket.staging,
                fixed_data_len: 0,
            },
        ],
        custody_program: finalized_v1(
            "Custody callee",
            input.custody,
            bpf_loader_upgradeable::ID,
            None,
        ),
    })
}

fn parent_root_source_v1<'a>(
    parent_root: Pubkey,
    state: SeriesPrepareParentRootStateV1,
    trading: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    match state {
        SeriesPrepareParentRootStateV1::PreActivationPredicted => Ok(
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "pre-activation M1 Series root",
                address: parent_root,
                fixed_data_len: u32::try_from(
                    dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
                )
                .map_err(|_| Error::new("Series root width escaped u32"))?,
            },
        ),
        SeriesPrepareParentRootStateV1::Finalized => Ok(finalized_v1(
            "M1 Series root",
            parent_root,
            trading,
            None,
        )),
    }
}

fn finalized_v1<'a>(
    role: &'static str,
    address: Pubkey,
    owner: Pubkey,
    body: Option<&'a [u8]>,
) -> SeriesPrepareRoleSourceV1<'a> {
    SeriesPrepareRoleSourceV1::Finalized {
        role,
        address,
        expected_owner: owner,
        canonical_body: body,
    }
}

fn record_raw_v1<'a>(
    role: &'static str,
    registry: Pubkey,
    record: SeriesPrepareFinalizedRecordV1<'a>,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|_| Error::new("Series Prepare record schema was zero"))?,
        ContentDigest::new(Sha256::digest(record.body).into())
            .map_err(|_| Error::new("Series Prepare record digest was zero"))?,
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
        .0
    };
    if record.raw != derive(key.raw_record_pda_seeds())
        || record.staging != derive(key.staging_cursor_pda_seeds())
    {
        return Err(Error::new(format!(
            "Series Prepare {role} record pair was noncanonical"
        )));
    }
    Ok(finalized_v1(role, record.raw, registry, Some(record.body)))
}

fn ticket_state_address_v1(input: &SeriesPrepareHydratorInputV1<'_>) -> Result<Pubkey> {
    let ticket = dclutch_trading::series::admit_ticket(input.ticket.body)
        .map_err(|_| Error::new("Series Prepare Ticket record refused"))?
        .content_id();
    Ok(Pubkey::find_program_address(
        &[
            dclutch_trading::series::replay::SERIES_TICKET_STATE_PDA_DOMAIN_V3,
            input.parent_root.as_ref(),
            ticket.as_bytes(),
        ],
        &input.trading,
    )
    .0)
}

fn require_prepare_projected_pair_v1(
    initialize: dclutch_custody::ProjectedCustodyRequestV1,
    open: dclutch_custody::ProjectedCustodyRequestV1,
    parent_root: Pubkey,
    trading: Pubkey,
    core: Pubkey,
    rent_program: Pubkey,
) -> Result<()> {
    use dclutch_custody::ProjectedCustodyOperationV1;
    if initialize.operation != ProjectedCustodyOperationV1::Initialize
        || open.operation != ProjectedCustodyOperationV1::OpenHoard
        || initialize.market != open.market
        || initialize.release_set != open.release_set
        || initialize.parent_capability_root != parent_root.to_bytes()
        || open.parent_capability_root != parent_root.to_bytes()
        || initialize.caller_program != trading.to_bytes()
        || open.caller_program != trading.to_bytes()
        || initialize.core_program != core.to_bytes()
        || open.core_program != core.to_bytes()
        || initialize.rent_program != rent_program.to_bytes()
        || open.rent_program != rent_program.to_bytes()
    {
        return Err(Error::new(
            "Series Prepare projected children did not join the active M1 root",
        ));
    }
    Ok(())
}

fn require_prepare_custody_request_v1(
    request: dclutch_custody::CustodyRequestV1,
    input: &SeriesPrepareHydratorInputV1<'_>,
) -> Result<()> {
    if request.caller_role != dclutch_custody::CallerRoleV1::Trading
        || request.caller_program != input.trading.to_bytes()
        || request.market != input.m0.project_found[1].to_bytes()
    {
        return Err(Error::new(
            "Series Prepare Custody child did not join M0 and Trading",
        ));
    }
    Ok(())
}

fn projected_frame_v1<'a, const N: usize>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::ProjectedCustodyRequestV1,
    bytes: &[u8],
) -> Result<[SeriesPrepareRoleSourceV1<'a>; N]> {
    use dclutch_custody::{
        ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1, ProjectedCustodyStateSeedsV2,
    };
    let digest = solana_program::hash::hash(bytes).to_bytes();
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(request, digest).as_slices(),
        &input.trading,
    )
    .0;
    let state = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &request.release_set,
        ],
        &input.registry,
    )
    .0;
    let common = [
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "projected caller authority",
            address: caller,
            fixed_data_len: 0,
        },
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "projected Custody state",
            address: state,
            fixed_data_len: u32::try_from(dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2)
                .map_err(|_| Error::new("projected state width escaped u32"))?,
        },
        finalized_v1("activation cache", cache, input.registry, None),
        finalized_v1(
            "Registry program",
            input.registry,
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "Trading program",
            input.trading,
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "Trading ProgramData",
            crate::upgrade::target_programdata(input.trading),
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "M0 RentCredit",
            Pubkey::new_from_array(request.rent_credit),
            input.rent_program,
            None,
        ),
    ];
    match request.operation {
        ProjectedCustodyOperationV1::Initialize => {
            let mut values = Vec::from(common);
            values.extend([
                finalized_v1("Core program", input.core, bpf_loader_upgradeable::ID, None),
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?,
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
                finalized_v1(
                    "System program",
                    system_program::ID,
                    native_loader::ID,
                    None,
                ),
            ]);
            for address in input.m0.project_found {
                values.push(m0_source_v1(input, address)?);
            }
            values
                .try_into()
                .map_err(|_| Error::new("Series Prepare projected Initialize frame drifted"))
        }
        ProjectedCustodyOperationV1::OpenHoard => {
            let mut values = Vec::from(common);
            let vault = Pubkey::new_from_array(request.hoard_vault);
            let expected_vault = Pubkey::find_program_address(
                &dclutch_custody::CustodyVaultSeedsV1::new(
                    request.market,
                    request.release_set,
                    request.context_digest,
                    dclutch_custody::CompartmentV1::HoardPrincipal,
                )
                .as_slices(),
                &input.custody,
            )
            .0;
            if vault != expected_vault {
                return Err(Error::new(
                    "Series Prepare Hoard vault PDA differed from Custody seeds",
                ));
            }
            let authority = Pubkey::find_program_address(
                &dclutch_custody::CustodyAuthoritySeedsV1::new(request.market, request.release_set)
                    .as_slices(),
                &input.custody,
            )
            .0;
            values.extend([
                SeriesPrepareRoleSourceV1::PredictedVacancy {
                    role: "projected Hoard vault",
                    address: vault,
                    fixed_data_len: u32::try_from(dclutch_custody::token_svm::ACCOUNT_BYTES)
                        .map_err(|_| Error::new("Hoard vault width escaped u32"))?,
                },
                SeriesPrepareRoleSourceV1::PredictedVacancy {
                    role: "Custody authority",
                    address: authority,
                    fixed_data_len: 0,
                },
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?,
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?,
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?,
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
                finalized_v1(
                    "System program",
                    system_program::ID,
                    native_loader::ID,
                    None,
                ),
                m0_source_v1(input, Pubkey::new_from_array(request.market))?,
            ]);
            values
                .try_into()
                .map_err(|_| Error::new("Series Prepare projected OpenHoard frame drifted"))
        }
        _ => Err(Error::new(
            "Series Prepare projected child selected an unsupported operation",
        )),
    }
}

fn custody_frame_v1<'a, const N: usize>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
    bytes: &[u8],
) -> Result<[SeriesPrepareRoleSourceV1<'a>; N]> {
    use dclutch_custody::{CustodyFrameRoleV1, CustodyFrameSpecV1};
    let spec = CustodyFrameSpecV1::new(request.operation);
    if usize::from(spec.account_count()) != N {
        return Err(Error::new(
            "Series Prepare Custody frame cardinality drifted",
        ));
    }
    let digest = solana_program::hash::hash(bytes).to_bytes();
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set)
                .map_err(|_| Error::new("Series Prepare Custody release was zero"))?,
            request.market,
            ExecutionRoleV1::Trading,
            request.context,
            digest,
        )
        .map_err(|_| Error::new("Series Prepare Custody caller seeds refused"))?
        .as_slices(),
        &input.trading,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &dclutch_custody::CustodyReplaySeedsV1::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &request.release_set,
        ],
        &input.registry,
    )
    .0;
    let mut values = Vec::with_capacity(N);
    for index in 0..N {
        let role = spec
            .account(u16::try_from(index).map_err(|_| Error::new("Custody index escaped u16"))?)
            .map_err(|_| Error::new("Series Prepare Custody frame coordinate refused"))?
            .role();
        let source = match role {
            CustodyFrameRoleV1::CallerAuthority => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Custody caller authority",
                address: authority,
                fixed_data_len: 0,
            },
            CustodyFrameRoleV1::CoreMarket => {
                m0_source_v1(input, Pubkey::new_from_array(request.market))?
            }
            CustodyFrameRoleV1::ActivationCache => {
                finalized_v1("activation cache", cache, input.registry, None)
            }
            CustodyFrameRoleV1::RegistryProgram => finalized_v1(
                "Registry program",
                input.registry,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgram => finalized_v1(
                "Trading program",
                input.trading,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgramData => finalized_v1(
                "Trading ProgramData",
                crate::upgrade::target_programdata(input.trading),
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::RealmRecord => m0_source_v1(input, input.m0.project_found[4])?,
            CustodyFrameRoleV1::RealmStaging => m0_source_v1(input, input.m0.project_found[5])?,
            CustodyFrameRoleV1::Replay => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "SeriesEscrow replay",
                address: replay,
                fixed_data_len: u32::try_from(dclutch_custody::CUSTODY_REPLAY_BYTES_V1)
                    .map_err(|_| Error::new("Custody replay width escaped u32"))?,
            },
            CustodyFrameRoleV1::Payer => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?
            }
            CustodyFrameRoleV1::SystemProgram => finalized_v1(
                "System program",
                system_program::ID,
                native_loader::ID,
                None,
            ),
            CustodyFrameRoleV1::RentSysvar => {
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None)
            }
            CustodyFrameRoleV1::Mint => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?
            }
            CustodyFrameRoleV1::Vault => custody_vault_source_v1(
                input,
                request,
                request.operation == dclutch_custody::OperationV1::OpenVault,
            )?,
            CustodyFrameRoleV1::CustodyAuthority => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Custody authority",
                address: Pubkey::find_program_address(
                    &dclutch_custody::CustodyAuthoritySeedsV1::new(
                        request.market,
                        request.release_set,
                    )
                    .as_slices(),
                    &input.custody,
                )
                .0,
                fixed_data_len: 0,
            },
            CustodyFrameRoleV1::TokenProgram => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?
            }
            CustodyFrameRoleV1::TransferSource => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.source))?
            }
            CustodyFrameRoleV1::TransferDestination => {
                custody_transfer_destination_v1(input, request)?
            }
            CustodyFrameRoleV1::RentRefund => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.rent_refund))?
            }
        };
        values.push(source);
    }
    values
        .try_into()
        .map_err(|_| Error::new("Series Prepare Custody frame cardinality changed"))
}

fn custody_vault_source_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
    creating: bool,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let context = if creating {
        request.destination_vault_context
    } else {
        request.source_vault_context
    };
    let vault = if creating {
        request.destination
    } else {
        request.source
    };
    let compartment = if creating {
        request.destination_compartment
    } else {
        request.source_compartment
    };
    let address = Pubkey::new_from_array(vault);
    if address
        != Pubkey::find_program_address(
            &dclutch_custody::CustodyVaultSeedsV1::new(
                request.market,
                request.release_set,
                context,
                compartment,
            )
            .as_slices(),
            &input.custody,
        )
        .0
    {
        return Err(Error::new(
            "Series Prepare Custody vault differed from canonical seeds",
        ));
    }
    Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
        role: "SeriesEscrow vault",
        address,
        fixed_data_len: u32::try_from(dclutch_custody::token_svm::ACCOUNT_BYTES)
            .map_err(|_| Error::new("Custody vault width escaped u32"))?,
    })
}

fn custody_transfer_destination_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    if request.destination_compartment == dclutch_custody::CompartmentV1::SeriesEscrow {
        custody_vault_source_v1(input, request, true)
    } else {
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.destination))
    }
}

fn m0_source_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    m0_frame_source_v1(input.registry, &input.m0, address)
}

/// Select a unique source-owned M0 record by the schema the universal Hot
/// prefix names.  The Prefix consumes Product and linked Basis bodies, never
/// occurrence/Ticket records which belong to the Series child proof.
fn m0_record_by_schema_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    schema: [u8; 32],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let matches = input
        .m0
        .records
        .iter()
        .copied()
        .filter(|record| record.schema == schema)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(*record),
        [] => Err(Error::new(format!(
            "Series Prepare M0 frame omitted {label} record"
        ))),
        _ => Err(Error::new(format!(
            "Series Prepare M0 frame duplicated {label} record"
        ))),
    }
}

fn m0_frame_source_v1<'a>(
    registry: Pubkey,
    m0: &'a SeriesPrepareM0FrameV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    if address == m0.project_found[1] {
        return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "vacant M0 Core Market",
            address,
            fixed_data_len: 0,
        });
    }
    if m0.vacancies.contains(&address) {
        return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "M0 vacant ProjectFound record",
            address,
            fixed_data_len: 0,
        });
    }
    for record in m0.records {
        if address == record.raw {
            return record_raw_v1("M0 finalized record", registry, *record);
        }
        if address == record.staging {
            record_raw_v1("M0 finalized record", registry, *record)?;
            return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "M0 vacant record staging",
                address,
                fixed_data_len: 0,
            });
        }
    }
    m0_frame_nonrecord_v1(m0, address)
}

fn m0_nonrecord_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    m0_frame_nonrecord_v1(&input.m0, address)
}

fn m0_frame_nonrecord_v1<'a>(
    m0: &'a SeriesPrepareM0FrameV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let account = m0
        .finalized_accounts
        .iter()
        .find(|account| account.address == address)
        .ok_or_else(|| {
            Error::new(format!(
                "Series Prepare omitted finalized M0 account {address}"
            ))
        })?;
    Ok(finalized_v1(
        "finalized M0 account",
        address,
        account.expected_owner,
        None,
    ))
}

#[cfg(test)]
mod prepare_hydrator_tests {
    use super::*;
    use dclutch_custody::{
        CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyOperationV1,
        ProjectedCustodyRequestV1,
    };

    #[test]
    fn found_prepare_command_rejects_repeated_execution_before_paths() {
        let error = parse_series_found_prepare_arguments_v1(vec![
            "--execute".to_owned(),
            "--execute".to_owned(),
        ])
        .expect_err("the command must not accept duplicate execution switches");
        assert_eq!(error.to_string(), "Series Found/Prepare repeats --execute");
    }

    #[test]
    fn found_prepare_command_rejects_unknown_flag_before_paths() {
        let error = parse_series_found_prepare_arguments_v1(vec![
            "--certificate-id".to_owned(),
            "00".to_owned(),
        ])
        .expect_err("certificate identity must be generated, never text input");
        assert_eq!(
            error.to_string(),
            "Series Found/Prepare rejects argument --certificate-id"
        );
    }

    #[test]
    fn selected_series_ledger_width_follows_its_manifest_bit() {
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(0).expect("first selected manifest bit"),
            1
        );
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(15).expect("last selected manifest bit"),
            1
        );
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(16)
                .expect_err("sixteenth index cannot fit the u16 funding mask")
                .to_string(),
            "Series selected manifest entry exceeds the funding mask"
        );
    }

    #[test]
    fn collateral_atoms_are_partitioned_without_subtracting_native_rent() {
        let source_atoms = 1_000_001;
        let [first, second] =
            split_series_hoard_principal_v1(source_atoms).expect("nonzero partition");
        assert_eq!(first.checked_add(second), Some(source_atoms));
        assert_eq!(first, 500_000);
        assert_eq!(second, 500_001);
    }

    fn projected(root: Pubkey) -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::Initialize,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: [1; 32],
            generation: 1,
            realm: [2; 32],
            product_record: [3; 32],
            product: [4; 32],
            source: [5; 32],
            release_set: [6; 32],
            projection_receipt_digest: [7; 32],
            parent_capability_root: root.to_bytes(),
            context_digest: [8; 32],
            caller_program: [9; 32],
            payer: [10; 32],
            core_program: [11; 32],
            rent_program: [12; 32],
            refund_owner: [13; 32],
            rent_credit: [14; 32],
            hoard_vault: [15; 32],
            funding_source_vault: [16; 32],
            funding_source_context: [17; 32],
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: [18; 32],
            token_program: [19; 32],
            collateral_release: [20; 32],
            expiry_slot: 2,
            expected_revision: 0,
            resulting_revision: 1,
            amount: 0,
            state_rent_lamports: 1,
            vault_rent_lamports: 1,
            funding_source_replay_revision: 3,
            funding_source_state_rent_lamports: 1,
            funding_source_vault_rent_lamports: 1,
        }
    }

    #[test]
    fn projected_prepare_children_cannot_substitute_the_m1_root() {
        let root = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let error = require_prepare_projected_pair_v1(
            projected(wrong),
            ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::OpenHoard,
                expected_revision: 1,
                resulting_revision: 2,
                ..projected(wrong)
            },
            root,
            Pubkey::new_from_array([9; 32]),
            Pubkey::new_from_array([11; 32]),
            Pubkey::new_from_array([12; 32]),
        )
        .expect_err("M1 root substitution must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare projected children did not join the active M1 root"
        );
    }

    #[test]
    fn preactivation_root_is_only_the_fixed_width_projection() {
        let root = Pubkey::new_unique();
        assert!(matches!(
            parent_root_source_v1(
                root,
                SeriesPrepareParentRootStateV1::PreActivationPredicted,
                Pubkey::new_unique(),
            )
            .expect("projected root"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "pre-activation M1 Series root",
                address,
                fixed_data_len,
            } if address == root && fixed_data_len == dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32
        ));
    }

    #[test]
    fn m0_core_coordinate_remains_a_vacancy_until_consume_found() {
        let market = Pubkey::new_unique();
        let absent_floor = Pubkey::new_unique();
        let mut project_found = [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2];
        project_found[1] = market;
        let vacancies = [absent_floor];
        let m0 = SeriesPrepareM0FrameV1 {
            project_found,
            records: &[],
            vacancies: &vacancies,
            finalized_accounts: &[],
        };
        assert!(matches!(
            m0_frame_source_v1(Pubkey::new_unique(), &m0, market)
                .expect("M0 Core coordinate"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "vacant M0 Core Market",
                address,
                fixed_data_len: 0,
            } if address == market
        ));
        assert!(matches!(
            m0_frame_source_v1(Pubkey::new_unique(), &m0, absent_floor)
                .expect("M0 absent floor coordinate"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "M0 vacant ProjectFound record",
                address,
                fixed_data_len: 0,
            } if address == absent_floor
        ));
    }

    #[test]
    fn m0_token_program_keeps_its_authenticated_upgradeable_loader_owner() {
        let token_program = Pubkey::new_unique();
        let frame = SeriesPrepareM0FrameV1 {
            project_found: [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
            records: &[],
            vacancies: &[],
            finalized_accounts: &[SeriesPrepareFinalizedAccountV1 {
                address: token_program,
                expected_owner: bpf_loader_upgradeable::ID,
            }],
        };
        assert!(matches!(
            m0_frame_nonrecord_v1(&frame, token_program).expect("published Token-2022 program"),
            SeriesPrepareRoleSourceV1::Finalized {
                expected_owner,
                ..
            } if expected_owner == bpf_loader_upgradeable::ID
        ));
    }

    #[test]
    fn m0_publisher_handoff_preserves_its_exact_pair_and_body() {
        let published = crate::runtime::PublishedRecord {
            schema: [7; 32],
            digest: Sha256::digest(b"published M0 body").into(),
            raw: Pubkey::new_unique(),
            staging: Pubkey::new_unique(),
        };
        let publication = FutureMarketImmutablePublicationV1 {
            realm: published,
            product: published,
            domain: published,
            portfolio: published,
            manifest: published,
            rent_credit: Pubkey::new_unique(),
            project_found: [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
            principal_cap_sets: 1,
            series_prepare_records: vec![crate::market::FutureMarketFinalizedRecordV1 {
                published,
                body: b"published M0 body".to_vec(),
            }],
            series_prepare_vacancies: vec![Pubkey::new_unique()],
        };
        let records = series_prepare_records_from_m0_publication_v1(&publication);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].schema, published.schema);
        assert_eq!(records[0].body, b"published M0 body");
        assert_eq!(records[0].raw, published.raw);
        assert_eq!(records[0].staging, published.staging);
        assert_eq!(
            series_prepare_record_from_m0_publication_v1(
                &records,
                published.schema,
                b"published M0 body",
                "published M0"
            )
            .expect("publisher body lookup"),
            records[0]
        );
        let error = series_prepare_record_from_m0_publication_v1(
            &records,
            published.schema,
            b"substituted M0 body",
            "published M0",
        )
        .expect_err("substituted publisher body must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare M0 publisher omitted canonical published M0 record"
        );
        assert_eq!(
            series_prepare_founder_record_v1(
                published,
                published.schema,
                b"published M0 body",
                "founder"
            )
            .expect("founder pair"),
            records[0]
        );
        let frame = series_prepare_m0_frame_from_publication_v1(&publication, &records, &[]);
        assert_eq!(frame.records[0].raw, published.raw);
        assert_eq!(frame.vacancies, publication.series_prepare_vacancies);
    }

    #[test]
    fn record_pair_requires_canonical_registry_pdas() {
        let error = record_raw_v1(
            "forged record",
            Pubkey::new_unique(),
            SeriesPrepareFinalizedRecordV1 {
                schema: [1; 32],
                body: b"canonical body",
                raw: Pubkey::new_unique(),
                staging: Pubkey::new_unique(),
            },
        )
        .expect_err("arbitrary Registry pair must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare forged record record pair was noncanonical"
        );
    }
}
