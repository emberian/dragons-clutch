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
    release_set::CapabilityExecutionSelectionV1,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::{fs, path::PathBuf};

use crate::{
    Error, Result,
    market::{derive_founding_targets, open_market_generation_v1, record_identity},
    model::{MarketRunInput, SelectedCapabilityV1, SuccessorPlan},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount},
    runtime::decode_hex,
    selected_capability_activation::{
        SelectedActivationRecordPairV1, SelectedCapabilityActivationInputV1,
        build_selected_capability_activation_plan_v1, execute_selected_capability_activation_v1,
    },
};

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
