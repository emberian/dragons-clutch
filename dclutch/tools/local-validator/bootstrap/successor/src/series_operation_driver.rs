//! Production Series operation inputs and durable local/devnet transport.
//!
//! A source is emitted by the typed native constructor, then reauthenticated
//! against finalized selected accounts on every acquisition. It grants no
//! authority by itself. Journal execution reuses the Series terminal engine.

use super::*;
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::rpc::WritePolicyV1;
use dclutch_operator::series_intent_v1::SeriesOperationIntentV1;

pub(crate) const LOCAL_COMMAND: &str = "local-private-validator-series-act-v1";
pub(crate) const DEVNET_COMMAND: &str = "devnet-series-act-v1";
const SOURCE_SCHEMA: &str =
    dclutch_operator::series_operation_corpus_v1::SERIES_NATIVE_OPERATION_SOURCE_SCHEMA_V1;

/// Emit the first production continuation from the same typed source that
/// constructed its accepted native Prepare instruction. No raw corpus entry
/// or physical privilege is accepted from a launcher.
pub(crate) fn write_prepare_source(
    path: &Path,
    input: &SeriesPrepareAddressFrameV1<'_>,
    selected: &SeriesSelectedHotReportV5,
    genesis_hash: String,
    lookup: &ObservedAccount,
) -> Result<String> {
    let (acquisition, current) = prepare_source_from_addresses_v1(input)?;
    let request = SeriesActionRequestV3::decode(&selected.selected.request_bytes)
        .map_err(|error| refusal(format!("Series accepted request decode: {error:?}")))?;
    let _intent = SeriesOperationIntentV1::from_request(selected.roles.root.to_bytes(), request)
        .map_err(|error| refusal(format!("Series accepted intent: {error:?}")))?;
    let source = SeriesNativeOperationSourceV1 {
        schema: SOURCE_SCHEMA.into(),
        genesis_hash,
        payer: input.payer.to_string(),
        lookup_table: lookup.key.to_string(),
        lookup_table_sha256: sha256_hex(&lookup.data),
        accepted_root: selected.roles.root.to_string(),
        accepted_request_base64: BASE64.encode(&selected.selected.request_bytes),
        current_source: current.document()?,
        acquisition,
    };
    create_series_canonical_json_v1(path, &source, "Series operation source")?;
    Ok(sha256_hex(&fs::read(path)?))
}

/// Emit any of the five actions from the same typed observations accepted by
/// the native acquisition operator. All bodies, aliases, privileges, replay,
/// and scheduling remain owned by that operator. This is the source-producer
/// seam for a CLI authoring driver or a wallet-backed Workbench adapter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_current_source(
    path: &Path,
    input: SeriesCurrentAcquisitionInputV5<'_>,
    current_source: SeriesCurrentReleaseInputV5<'_>,
    intent: SeriesOperationIntentV1,
    payer: Pubkey,
    genesis_hash: String,
    lookup: &ObservedAccount,
) -> Result<(String, SeriesSelectedHotReportV5)> {
    let lifecycle = inspect_series_lifecycle_v3(input.lifecycle)
        .map_err(|error| refusal(format!("Series source lifecycle: {error:?}")))?;
    let planned = match lifecycle.next() {
        SeriesNextActV3::Ready(planned) => planned,
        other => return Err(refusal(format!("Series source is not ready: {other:?}"))),
    };
    let owned_release = emit_current_series_release_source_v5(current_source)
        .map_err(|error| refusal(format!("Series source emission: {error:?}")))?;
    let release = compile_series_release_v5(owned_release.as_source())
        .map_err(|error| refusal(format!("Series source compilation: {error:?}")))?;
    let preselected = authenticate_series_selected_action_v5(
        &release,
        owned_release.as_source(),
        planned.request().as_bytes(),
    )
    .map_err(|error| refusal(format!("Series source selection: {error:?}")))?;
    let occurrence = input.records.occurrence;
    let ticket = input.records.ticket;
    let rent_credit = input.records.rent_credit;
    let expire_permit = input.records.expire_permit;
    let current = input.lifecycle.current;
    let shadow = input.shadow;
    let logical = input
        .runtime_logical_accounts
        .iter()
        .map(|account| account.key.to_string())
        .collect();
    let acquired = acquire_current_series_hot_v5(
        &preselected,
        owned_release.action_artifacts(preselected.action),
        input,
    )
    .map_err(|error| refusal(format!("Series source native acquisition: {error:?}")))?;
    let selected = match inspect_current_series_hot_v5(&acquired.state, current_source)
        .map_err(|error| refusal(format!("Series source native inspection: {error:?}")))?
    {
        SeriesCurrentHotPlanV5::Ready(selected) => selected,
        other => {
            return Err(refusal(format!(
                "Series source native inspection is not ready: {other:?}"
            )));
        }
    };
    let request = SeriesActionRequestV3::decode(&selected.selected.request_bytes)
        .map_err(|error| refusal(format!("Series source request: {error:?}")))?;
    intent
        .require_matches(selected.roles.root.to_bytes(), request)
        .map_err(|error| refusal(format!("Series source intent changed: {error:?}")))?;
    authenticate_permissionless_series_signers_v1(&selected.instruction, payer)?;
    let fixed: [Pubkey; dclutch_market::capability_program::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3] =
        acquired
            .state
            .fixed_accounts
            .iter()
            .map(|meta| meta.account.key)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| refusal("Series source fixed frame cardinality changed"))?;
    let pair = |record: &dclutch_operator::direct_inline_route_v3::FinalizedRecordRouteV3| {
        SeriesFinalizedRecordAddressesV2 {
            raw: record.raw.key.to_string(),
            staging: record.staging.key.to_string(),
        }
    };
    let current_occurrence = if selected.selected.action.occurrence_bound() {
        let occurrence =
            occurrence.ok_or_else(|| refusal("Series source omitted occurrence record"))?;
        let ticket = ticket.ok_or_else(|| refusal("Series source omitted Ticket record"))?;
        let current =
            current.ok_or_else(|| refusal("Series source omitted native occurrence proof"))?;
        Some(SeriesCurrentOccurrenceRouteV1 {
            occurrence_record: occurrence.raw.key.to_string(),
            occurrence_staging: occurrence.staging.key.to_string(),
            ticket_record: ticket.raw.key.to_string(),
            ticket_staging: ticket.staging.key.to_string(),
            ticket_replay: current
                .ticket_state
                .map(|_| {
                    selected
                        .roles
                        .ticket
                        .ok_or_else(|| refusal("Series source omitted Ticket replay role"))
                        .map(|key| key.to_string())
                })
                .transpose()?,
            siblings: current.siblings.iter().map(|value| hex32(*value)).collect(),
        })
    } else {
        None
    };
    let terminal_ticket = if selected.selected.action == SeriesActionV3::Retire {
        let ticket = ticket.ok_or_else(|| refusal("Series Retire source omitted Ticket record"))?;
        Some(SeriesTerminalTicketRouteV1 {
            ticket_record: ticket.raw.key.to_string(),
            ticket_staging: ticket.staging.key.to_string(),
            ticket_replay: selected
                .roles
                .ticket
                .ok_or_else(|| refusal("Series Retire source omitted replay"))?
                .to_string(),
        })
    } else {
        None
    };
    let consume_shadow = shadow
        .map(|shadow| -> Result<_> {
            let mut request = vec![
                0;
                shadow.request.encoded_len().map_err(|error| refusal(
                    format!("Series shadow source length: {error:?}")
                ))?
            ];
            shadow
                .request
                .encode_into(&mut request)
                .map_err(|error| refusal(format!("Series shadow source encode: {error:?}")))?;
            Ok(SeriesConsumeShadowAcquisitionV2 {
                certificate: pair(shadow.certificate),
                artifact: pair(shadow.artifact),
                accelerator_program: shadow.accelerator_program.key.to_string(),
                accelerator_programdata: shadow.accelerator_programdata.key.to_string(),
                caller_authority: shadow.caller_authority.key.to_string(),
                checked_manifest_sha256: hex32(shadow.checked.checked_manifest_digest),
                request_base64: BASE64.encode(request),
            })
        })
        .transpose()?;
    let source = SeriesNativeOperationSourceV1 {
        schema: SOURCE_SCHEMA.into(),
        genesis_hash,
        payer: payer.to_string(),
        lookup_table: lookup.key.to_string(),
        lookup_table_sha256: sha256_hex(&lookup.data),
        accepted_root: selected.roles.root.to_string(),
        accepted_request_base64: BASE64.encode(&selected.selected.request_bytes),
        current_source: DecodedSeriesCurrentSourceV1::from_release_v1(current_source)?
            .document()?,
        acquisition: SeriesHotAcquisitionRecipeV2 {
            sequence: 0,
            fixed: hot_fixed_source_from_addresses_v1(&fixed)?,
            runtime_logical_accounts: logical,
            consume_shadow,
            current_occurrence,
            terminal_ticket,
            lifecycle_rent_credit: rent_credit.map(|account| account.key.to_string()),
            expire_permit: expire_permit.map(|account| account.key.to_string()),
        },
    };
    authenticate_series_lookup_table_v1(lookup, lookup.key, &source.lookup_table_sha256)?;
    create_series_canonical_json_v1(path, &source, "Series operation source")?;
    Ok((sha256_hex(&fs::read(path)?), selected))
}

/// One operation, using the same native acquisition and durable packet engine
/// on either admitted cluster. A public origin is bound to the actual devnet
/// genesis; no local validator path is invented for it.
pub(crate) fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let mut values = BTreeMap::new();
    let mut execute = false;
    let mut args = arguments.into_iter();
    while let Some(flag) = args.next() {
        if flag == "--execute" {
            if execute {
                return Err(refusal("Series act repeats --execute"));
            }
            execute = true;
            continue;
        }
        if !matches!(
            flag.as_str(),
            "--source"
                | "--expected-source-sha256"
                | "--rpc-url"
                | "--journal"
                | "--fee-payer-keypair"
                | "--ledger"
                | DEVNET_ACKNOWLEDGMENT_FLAG
        ) {
            return Err(refusal(format!("Series act rejects {flag}")));
        }
        let value = args
            .next()
            .ok_or_else(|| refusal(format!("Series act {flag} needs a value")))?;
        if values.insert(flag.clone(), value).is_some() {
            return Err(refusal(format!("Series act repeats {flag}")));
        }
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| refusal(format!("Series act requires {flag}")))
    };
    let origin = ClusterOriginV1::parse(
        &required("--rpc-url")?,
        values.get(DEVNET_ACKNOWLEDGMENT_FLAG).map(String::as_str),
    )?;
    expected.authenticate(&origin)?;
    let source_path = PathBuf::from(required("--source")?);
    let journal_path = PathBuf::from(required("--journal")?);
    if !source_path.is_absolute() || !journal_path.is_absolute() || journal_path == source_path {
        return Err(refusal(
            "Series source and journal require distinct absolute paths",
        ));
    }
    let bytes = fs::read(&source_path)?;
    let source_sha256 = sha256_hex(&bytes);
    if source_sha256 != required("--expected-source-sha256")? {
        return Err(refusal("Series operation source digest changed"));
    }
    let source: SeriesNativeOperationSourceV1 = serde_json::from_slice(&bytes)?;
    if source.schema != SOURCE_SCHEMA || source.acquisition.sequence != 0 {
        return Err(refusal(
            "Series operation source schema or sequence changed",
        ));
    }
    let origin_identity = match expected {
        ExpectedClusterV1::OwnedLoopback => {
            let path = PathBuf::from(required("--ledger")?);
            if !path.is_absolute() || !path.is_dir() || fs::canonicalize(&path)? != path {
                return Err(refusal(
                    "Series local operation needs its canonical existing ledger",
                ));
            }
            SeriesLedgerIdentityV1::admit(path.display().to_string(), source.genesis_hash.clone())?
        }
        ExpectedClusterV1::Devnet => {
            if values.contains_key("--ledger") {
                return Err(refusal(
                    "Series devnet operation does not accept a local ledger",
                ));
            }
            let mut identity = SeriesLedgerIdentityV1 {
                canonical_ledger_path: String::new(),
                cluster_origin: Some(origin.url().to_owned()),
                genesis_hash: source.genesis_hash.clone(),
                identity_sha256: String::new(),
            };
            identity.identity_sha256 = ledger_identity_digest_v1(&identity)?;
            authenticate_ledger_identity_v1(&identity)?;
            identity
        }
    };
    let mut rpc = Rpc::connect_cluster(
        &origin,
        if execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let genesis = rpc.call("getGenesisHash", &serde_json::json!([]))?;
    if genesis.as_str() != Some(&source.genesis_hash) {
        return Err(refusal("Series operation genesis changed"));
    }
    let payer = parse_pubkey(&source.payer, "Series operation payer")?;
    let lookup = parse_pubkey(&source.lookup_table, "Series operation lookup table")?;
    let decoded = DecodedSeriesCurrentSourceV1::decode(&source.current_source)?;
    let accepted = decode_base64(&source.accepted_request_base64, "Series accepted request")?;
    let accepted_request = SeriesActionRequestV3::decode(&accepted)
        .map_err(|error| refusal(format!("Series accepted request: {error:?}")))?;
    let intent = SeriesOperationIntentV1::from_request(
        parse_pubkey(&source.accepted_root, "Series accepted root")?.to_bytes(),
        accepted_request,
    )
    .map_err(|error| refusal(format!("Series intent: {error:?}")))?;
    let current = if journal_path.exists() {
        Some(read_series_terminal_journal_file_v1(&journal_path)?)
    } else {
        None
    };
    if let Some(current) = &current {
        if current.campaign_sha256 != source_sha256 || current.ledger != origin_identity {
            return Err(refusal(
                "Series operation journal changed source or cluster",
            ));
        }
        if current.phase == SeriesTerminalJournalPhaseV1::Finalized {
            print_series_terminal_progress_v1(
                &journal_path,
                current,
                "finalized",
                "Accepted packet and native poststate remain durable.",
            );
            return Ok(());
        }
        if matches!(
            current.phase,
            SeriesTerminalJournalPhaseV1::Dispatching | SeriesTerminalJournalPhaseV1::Submitted
        ) {
            if let Some((journal, _)) = try_finalize_landed_series_action_v1(
                &mut rpc,
                &journal_path,
                current,
                &decoded,
                lookup,
                &source.lookup_table_sha256,
            )? {
                print_series_terminal_progress_v1(
                    &journal_path,
                    &journal,
                    "finalized",
                    "Recovered exact accepted packet and native poststate.",
                );
                return Ok(());
            }
        }
    }
    let acquired = acquire_current_series_selected_v1(
        &mut rpc,
        &source.acquisition,
        &decoded,
        payer,
        lookup,
        SeriesCampaignPolicyV1::for_act(intent.action),
    )?;
    let fresh_request = SeriesActionRequestV3::decode(&acquired.selected.selected.request_bytes)
        .map_err(|error| refusal(format!("Series fresh request: {error:?}")))?;
    intent
        .require_matches(acquired.selected.roles.root.to_bytes(), fresh_request)
        .map_err(|error| refusal(format!("Series fresh plan changed intent: {error:?}")))?;
    let lookup_observed = operator_account_v1(
        required_series_account_v1(&acquired.accounts, lookup, "Series lookup table")?,
        acquired.observation,
    )?;
    authenticate_series_lookup_table_v1(&lookup_observed, lookup, &source.lookup_table_sha256)?;
    authenticate_permissionless_series_signers_v1(&acquired.selected.instruction, payer)?;
    let prepared = if let Some(current) = current {
        reauthenticate_series_selected_action_v1(&current, &acquired.selected)?;
        current
    } else {
        let planned = plan_series_terminal_journal_v1(
            SeriesPlannerObservationV1 {
                campaign_sha256: source_sha256,
                ledger: origin_identity.clone(),
                finalized_slot: acquired.observation.slot,
                snapshot_sha256: acquired_series_snapshot_digest_v1(
                    acquired.observation,
                    &acquired.accounts,
                ),
            },
            0,
            &acquired.lifecycle,
        )?;
        let prestate = selected_projection_from_acquisition_v1(
            &origin_identity,
            &acquired.selected,
            payer,
            acquired.observation.slot,
            &acquired.accounts,
        )?;
        let prepared =
            prepare_series_terminal_journal_v1(&planned, &acquired.selected, prestate, payer)?;
        create_series_terminal_journal_file_v1(&journal_path, &prepared)?;
        prepared
    };
    if !execute {
        print_series_terminal_progress_v1(
            &journal_path,
            &prepared,
            "prepared",
            "Native frame and observed prestate persisted; no signing key read.",
        );
        return Ok(());
    }
    let active = if prepared.phase == SeriesTerminalJournalPhaseV1::Prepared {
        let key = Keypair::new_from_array(read_keypair_file(
            Path::new(&required("--fee-payer-keypair")?),
            "Series fee payer",
        )?);
        if key.pubkey() != payer {
            return Err(refusal("Series signing wallet changed"));
        }
        dispatch_series_terminal_from_rpc_v1(
            &mut rpc,
            &journal_path,
            &prepared,
            &acquired.selected,
            &key,
            &[],
            lookup,
            &source.lookup_table_sha256,
            &lookup_observed,
        )?
    } else {
        prepared
    };
    if active.phase == SeriesTerminalJournalPhaseV1::Dispatching {
        let packet = active
            .packet
            .as_ref()
            .ok_or_else(|| refusal("Series dispatch omitted packet"))?;
        let bytes = decode_series_packet_v1(&packet.signed)?;
        let simulation_path = journal_path.with_extension("simulation.json");
        if !simulation_path.exists() {
            let simulation = rpc.simulate_versioned_v1("Series exact durable packet", &bytes)?;
            let evidence = serde_json::json!({
                "schema": "dclutch-series-exact-packet-simulation-v1",
                "packetSha256": packet.signed.packet_sha256,
                "packetBytes": bytes.len(),
                "accountKeyCount": packet.resolved_account_keys.len(),
                "simulation": simulation,
            });
            create_series_canonical_json_v1(
                &simulation_path,
                &evidence,
                "Series packet simulation",
            )?;
            eprintln!("Series durable packet simulation: {evidence}");
            if simulation
                .get("value")
                .unwrap_or(&simulation)
                .get("err")
                .is_none_or(|value| !value.is_null())
            {
                return Err(refusal(format!(
                    "Series exact durable packet simulation refused: {simulation}"
                )));
            }
        } else {
            let evidence: serde_json::Value =
                read_series_canonical_json_v1(&simulation_path, "Series packet simulation")?;
            if evidence
                .get("packetSha256")
                .and_then(serde_json::Value::as_str)
                != Some(&packet.signed.packet_sha256)
                || evidence
                    .pointer("/simulation/value/err")
                    .or_else(|| evidence.pointer("/simulation/err"))
                    .is_none_or(|value| !value.is_null())
            {
                return Err(refusal("Series persisted simulation changed or refused"));
            }
        }
    }
    let advanced = advance_series_terminal_from_rpc_v1(
        &mut rpc,
        &journal_path,
        &active,
        &acquired.selected,
        lookup,
        &source.lookup_table_sha256,
        &lookup_observed,
    )?;
    match advanced {
        SeriesTerminalRpcAdvanceV1::Finalized { journal, .. } => print_series_terminal_progress_v1(
            &journal_path,
            &journal,
            "finalized",
            "Exact packet and accepted native poststate are durable.",
        ),
        SeriesTerminalRpcAdvanceV1::Dispatching(journal)
        | SeriesTerminalRpcAdvanceV1::Pending(journal) => print_series_terminal_progress_v1(
            &journal_path,
            &journal,
            "pending",
            "Rerun this command to recover the same signature; no fresh signature is authorized.",
        ),
    }
    Ok(())
}

/// The Found entrance drives the same durable public operation command. A
/// bounded local finality wait never creates another transaction identity.
pub(crate) fn execute_local_prepare_source(
    rpc: &mut Rpc,
    source_path: &Path,
    source_sha256: &str,
    rpc_url: &str,
    ledger: &Path,
    payer_keypair: &Path,
    journal_path: &Path,
) -> Result<(crate::model::TransactionEvidence, serde_json::Value)> {
    let arguments = vec![
        "--source".into(),
        source_path.display().to_string(),
        "--expected-source-sha256".into(),
        source_sha256.into(),
        "--rpc-url".into(),
        rpc_url.into(),
        "--ledger".into(),
        ledger.display().to_string(),
        "--journal".into(),
        journal_path.display().to_string(),
        "--fee-payer-keypair".into(),
        payer_keypair.display().to_string(),
        "--execute".into(),
    ];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        run(arguments.clone(), ExpectedClusterV1::OwnedLoopback)?;
        let journal = read_series_terminal_journal_file_v1(journal_path)?;
        if let Some(finalization) = &journal.finalization {
            let signature = Signature::from_str(&finalization.signature)
                .map_err(|error| refusal(format!("Series finalized signature: {error}")))?;
            let finalized = rpc
                .finalized_signed_packet("durable first Series Prepare", signature, false)?
                .ok_or_else(|| refusal("Series finalized journal omitted transaction history"))?;
            let simulation = read_series_canonical_json_v1(
                &journal_path.with_extension("simulation.json"),
                "Series packet simulation",
            )?;
            return Ok((finalized.evidence, simulation));
        }
        if std::time::Instant::now() >= deadline {
            return Err(refusal(format!(
                "Series Prepare remains pending; resume the same source and journal {}",
                journal_path.display()
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_operation_source_round_trips_the_native_child_bank() {
        use crate::series_found_prepare_campaign::{
            compile_series_found_prepare_selection_v1,
            tests::{compiler_input, prepared_founder},
        };
        let founder = prepared_founder();
        let input = compiler_input(
            &founder,
            Pubkey::new_unique(),
            &founder.admitted.tickets()[0],
        );
        let compiled = compile_series_found_prepare_selection_v1(
            input,
            ContentId::new([61; 32]).expect("certificate"),
        )
        .expect("native complete compiler");
        let template = dclutch_trading::series::template_content_id(founder.admitted.template())
            .expect("native Template");
        let release = crate::series_found_prepare_driver::series_current_release_v1(
            &compiled,
            founder.admitted.template(),
            ContentId::new([61; 32]).expect("certificate"),
            2,
            1,
        )
        .expect("native release input");
        let decoded = DecodedSeriesCurrentSourceV1::from_release_v1(release).expect("typed source");
        let document = decoded.document().expect("native document encoders");
        let bytes = serde_json::to_vec(&document).expect("source document");
        let reread: SeriesCurrentSourceCorpusV1 =
            serde_json::from_slice(&bytes).expect("hostile document read");
        let round_trip =
            DecodedSeriesCurrentSourceV1::decode(&reread).expect("native child decoders");
        let original = emit_current_series_release_source_v5(decoded.input(template))
            .expect("original native source");
        let recovered = emit_current_series_release_source_v5(round_trip.input(template))
            .expect("recovered native source");
        assert_eq!(original, recovered);
        let mut hostile = reread;
        hostile.prepare_fixed_data_lengths.pop();
        assert_eq!(
            DecodedSeriesCurrentSourceV1::decode(&hostile)
                .err()
                .expect("truncated native geometry")
                .to_string(),
            "REFUSED Series terminal: Series Prepare fixed-width corpus changed cardinality"
        );
    }

    #[test]
    fn series_operation_cluster_guard_refuses_cross_cluster_before_files() {
        let args = vec!["--rpc-url".into(), "http://127.0.0.1:31100".into()];
        assert_eq!(
            run(args, ExpectedClusterV1::Devnet)
                .expect_err("devnet verb cannot use local bank")
                .to_string(),
            "public executor requires acknowledged Solana devnet and refuses loopback"
        );
    }
}
