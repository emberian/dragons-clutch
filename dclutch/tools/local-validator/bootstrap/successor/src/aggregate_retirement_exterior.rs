//! Owned-loopback exterior for checkpointed AggregateRetirement.
//!
//! The onchain checkpoint is the route owner. Four separate journals retain
//! the exact packets needed to recover a crash without inventing another
//! transaction identity.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_market_retirement_v1_operator::{
    ObservedAccount, build_checkpoint_market_retirement_v1,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_program::pubkey::Pubkey;
use solana_sdk::{
    message::VersionedMessage,
    signature::{Keypair, Signature, Signer as _},
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result,
    aggregate_retirement_journal::{
        AggregateRetirementCampaignInputV1, AggregateRetirementCampaignV1,
        AggregateRetirementChainAccountV1, AggregateRetirementChainProjectionV1,
        AggregateRetirementFinalizationV1, AggregateRetirementJournalPhaseV1,
        AggregateRetirementJournalV1, AggregateRetirementOperationV1,
        AggregateRetirementRecoveryV1, AggregateRetirementRouteV1,
        build_aggregate_retirement_campaign_v1, build_aggregate_retirement_conservation_receipt_v1,
        build_aggregate_retirement_packet_binding_v1, classify_aggregate_retirement_chain_v1,
        dispatch_aggregate_retirement_journal_v1, finalize_aggregate_retirement_journal_v1,
        plan_aggregate_retirement_journal_v1, prepare_aggregate_retirement_journal_v1,
        route_aggregate_retirement_v1, submit_aggregate_retirement_journal_v1,
    },
    campaign::{
        CampaignTerminalEvidenceV1, parse_campaign_terminal_evidence_with_expected_cluster_v1,
        read_keypair_file,
    },
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::pubkey,
    rpc::{Rpc, SignedVersionedPacketV1, WritePolicyV1},
    terminal_lifecycle::{
        authenticate_campaign_market_v1, authenticate_plan_source,
        require_direct_retirement_evidence,
    },
    terminal_sequence::aggregate_retirement_snapshot_from_chain_v1,
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-aggregate-retirement-v1";
const PROGRESS_SCHEMA_V1: &str = "dclutch-owned-loopback-aggregate-retirement-progress-v1";
const JOURNAL_NAMES_V1: [&str; 4] = [
    "00-prepare.json",
    "01-close-vault.json",
    "02-close-replay.json",
    "03-finish.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    source_receipt: Pubkey,
    payer: Pubkey,
    payer_keypair: PathBuf,
    lookup_table: Pubkey,
    campaign: PathBuf,
    journal_dir: PathBuf,
    completion: PathBuf,
    execute: bool,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments_v1(arguments)?;
    if !arguments.journal_dir.is_dir() {
        return Err(refusal(
            "--journal-dir must be an existing absolute directory",
        ));
    }
    let plan_source = read_bounded(&arguments.plan, "successor plan")?;
    let evidence_source = read_bounded(&arguments.evidence, "terminal evidence")?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &evidence_source,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_direct_retirement_evidence(&evidence)?;
    authenticate_campaign_market_v1(&evidence, arguments.market)?;

    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("getGenesisHash returned a non-string"))?
        .to_owned();
    arguments.origin.authenticate_genesis(&genesis_hash)?;

    let campaign = load_or_create_campaign_v1(
        &mut rpc,
        &arguments,
        &plan,
        &evidence,
        &plan_source,
        &evidence_source,
        genesis_hash,
    )?;
    authenticate_invocation_v1(
        &campaign,
        &arguments,
        &plan,
        &evidence,
        &plan_source,
        &evidence_source,
        rpc.url(),
    )?;

    operate_v1(&mut rpc, &arguments, &campaign)
}

fn operate_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    campaign: &AggregateRetirementCampaignV1,
) -> Result<()> {
    let journals = load_journals_v1(&arguments.journal_dir, campaign)?;
    let known_fees = finalized_fee_total_v1(&journals)?;
    let allow_unreconciled_fee = journals.last().is_some_and(|journal| {
        matches!(
            journal.phase,
            AggregateRetirementJournalPhaseV1::Dispatching
                | AggregateRetirementJournalPhaseV1::Submitted
        )
    });
    let projection = observe_projection_v1(rpc, campaign, 0, known_fees, allow_unreconciled_fee)?;
    match route_aggregate_retirement_v1(campaign, &journals, &projection)? {
        AggregateRetirementRouteV1::Complete => {
            write_completion_v1(arguments, campaign, &journals, &projection)
        }
        AggregateRetirementRouteV1::Plan(operation) => {
            let journal = plan_aggregate_retirement_journal_v1(campaign, operation, &projection)?;
            let path = journal_path_v1(&arguments.journal_dir, operation);
            create_json_v1(&path, &journal, "retirement journal")?;
            if !arguments.execute {
                return progress_v1(
                    arguments,
                    campaign,
                    operation,
                    "planned",
                    Some(&path),
                    "The next mutation is planned from finalized chain state; no key was read.",
                );
            }
            advance_active_v1(rpc, arguments, campaign, journal, projection)
        }
        AggregateRetirementRouteV1::Recover(operation, recovery) => {
            let journal = journals
                .last()
                .cloned()
                .ok_or_else(|| refusal("recovery route omitted its active journal"))?;
            if journal.operation != operation {
                return Err(refusal("recovery route changed the active operation"));
            }
            if !arguments.execute {
                return progress_v1(
                    arguments,
                    campaign,
                    operation,
                    journal_phase_text(journal.phase),
                    Some(&journal_path_v1(&arguments.journal_dir, operation)),
                    recovery_message(recovery),
                );
            }
            advance_active_v1(rpc, arguments, campaign, journal, projection)
        }
    }
}

fn advance_active_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    campaign: &AggregateRetirementCampaignV1,
    mut journal: AggregateRetirementJournalV1,
    mut projection: AggregateRetirementChainProjectionV1,
) -> Result<()> {
    let path = journal_path_v1(&arguments.journal_dir, journal.operation);
    if journal.phase == AggregateRetirementJournalPhaseV1::Planned {
        let next = prepare_aggregate_retirement_journal_v1(campaign, &journal, &projection)?;
        replace_json_v1(&path, &journal, &next, "retirement journal")?;
        journal = next;
    }
    if journal.phase == AggregateRetirementJournalPhaseV1::Prepared {
        if projection.phase != journal.predecessor {
            return Err(refusal(
                "Prepared journal no longer matched its exact chain predecessor",
            ));
        }
        let keypair = Keypair::new_from_array(read_keypair_file(
            &arguments.payer_keypair,
            "AggregateRetirement fee payer",
        )?);
        if keypair.pubkey() != arguments.payer {
            return Err(refusal("fee-payer keypair did not name --fee-payer"));
        }
        let table = observe_lookup_table_v1(rpc, campaign, 0)?;
        let instruction = campaign.operations[journal.operation.ordinal()].instruction()?;
        let signed = rpc.prepare_signed_v0_packet(
            journal.operation.label(),
            std::slice::from_ref(&instruction),
            &keypair,
            &table,
        )?;
        Rpc::authenticate_signed_v0_packet(
            journal.operation.label(),
            std::slice::from_ref(&instruction),
            arguments.payer,
            &table,
            &signed,
        )?;
        let resolved = resolve_packet_keys_v1(&signed, arguments.lookup_table, &table)?;
        let binding = build_aggregate_retirement_packet_binding_v1(
            campaign,
            journal.operation,
            signed,
            resolved,
        )?;
        let next = dispatch_aggregate_retirement_journal_v1(campaign, &journal, binding)?;
        replace_json_v1(&path, &journal, &next, "retirement journal")?;
        journal = next;
    }
    if journal.phase == AggregateRetirementJournalPhaseV1::Dispatching {
        if let Some(finalized) = poll_finalized_v1(rpc, &journal)? {
            let next = submit_aggregate_retirement_journal_v1(
                campaign,
                &journal,
                &finalized.evidence.signature,
            )?;
            replace_json_v1(&path, &journal, &next, "retirement journal")?;
            return finalize_active_v1(rpc, arguments, campaign, next, finalized, &path);
        }
        let packet = journal
            .packet
            .as_ref()
            .ok_or_else(|| refusal("Dispatching journal omitted its signed packet"))?;
        let instruction = campaign.operations[journal.operation.ordinal()].instruction()?;
        let table = observe_lookup_table_v1(rpc, campaign, projection.finalized_slot)?;
        Rpc::authenticate_signed_v0_packet(
            journal.operation.label(),
            std::slice::from_ref(&instruction),
            arguments.payer,
            &table,
            &packet.signed,
        )?;
        let signature = Signature::from_str(&packet.signed.signature)
            .map_err(|error| Error::new(format!("retirement signature: {error}")))?;
        let bytes = decode_packet_v1(&packet.signed)?;
        rpc.submit_signed_packet_once(journal.operation.label(), &bytes, signature, false)?;
        let next =
            submit_aggregate_retirement_journal_v1(campaign, &journal, &packet.signed.signature)?;
        replace_json_v1(&path, &journal, &next, "retirement journal")?;
        journal = next;
    }
    if journal.phase == AggregateRetirementJournalPhaseV1::Submitted {
        if let Some(finalized) = poll_finalized_v1(rpc, &journal)? {
            return finalize_active_v1(rpc, arguments, campaign, journal, finalized, &path);
        }
        return progress_v1(
            arguments,
            campaign,
            journal.operation,
            "submitted",
            Some(&path),
            "The exact durable signature is pending; rerun polls only and never re-signs.",
        );
    }
    if journal.phase == AggregateRetirementJournalPhaseV1::Finalized {
        projection = observe_projection_v1(
            rpc,
            campaign,
            projection.finalized_slot,
            finalized_fee_total_v1(&load_journals_v1(&arguments.journal_dir, campaign)?)?,
            false,
        )?;
        return progress_v1(
            arguments,
            campaign,
            journal.operation,
            "finalized",
            Some(&path),
            &format!(
                "{} finalized; rerun derives the next action from chain phase {:?}.",
                journal.operation.label(),
                projection.phase
            ),
        );
    }
    Err(refusal("active retirement journal had no executable route"))
}

fn finalize_active_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    campaign: &AggregateRetirementCampaignV1,
    journal: AggregateRetirementJournalV1,
    finalized: crate::rpc::FinalizedSignedPacketV1,
    path: &Path,
) -> Result<()> {
    let packet = journal
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Submitted journal omitted packet"))?;
    if sha256_hex(&finalized.packet) != packet.signed.packet_sha256 {
        return Err(refusal(
            "finalized transaction bytes differed from the durable packet",
        ));
    }
    let fee = finalized
        .evidence
        .fee_lamports
        .ok_or_else(|| refusal("finalized retirement omitted exact fee"))?;
    let compute_units = finalized
        .evidence
        .compute_units_consumed
        .ok_or_else(|| refusal("finalized retirement omitted compute units"))?;
    let previous = load_journals_v1(&arguments.journal_dir, campaign)?;
    let prior_fees = finalized_fee_total_v1(&previous)?;
    let projection = observe_projection_v1(
        rpc,
        campaign,
        finalized.evidence.slot,
        prior_fees
            .checked_add(fee)
            .ok_or_else(|| refusal("retirement fee sum overflowed"))?,
        false,
    )?;
    let finalization = AggregateRetirementFinalizationV1 {
        signature: finalized.evidence.signature,
        finalized_slot: finalized.evidence.slot,
        packet_sha256: packet.signed.packet_sha256.clone(),
        fee_lamports: fee,
        compute_units_consumed: compute_units,
        poststate_sha256: projection.state_sha256.clone(),
        checkpoint_history_sha256: projection.checkpoint_history_sha256.clone(),
    };
    let next =
        finalize_aggregate_retirement_journal_v1(campaign, &journal, &projection, finalization)?;
    replace_json_v1(path, &journal, &next, "retirement journal")?;
    let journals = load_journals_v1(&arguments.journal_dir, campaign)?;
    if route_aggregate_retirement_v1(campaign, &journals, &projection)?
        == AggregateRetirementRouteV1::Complete
    {
        return write_completion_v1(arguments, campaign, &journals, &projection);
    }
    progress_v1(
        arguments,
        campaign,
        next.operation,
        "finalized",
        Some(path),
        "One exact retirement mutation finalized; rerun derives its successor from chain state.",
    )
}

fn load_or_create_campaign_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    plan_source: &[u8],
    evidence_source: &[u8],
    genesis_hash: String,
) -> Result<AggregateRetirementCampaignV1> {
    if arguments.campaign.exists() {
        return read_json_v1(&arguments.campaign, "retirement campaign");
    }
    let (snapshot, prestate) = aggregate_retirement_snapshot_from_chain_v1(
        rpc,
        plan,
        evidence,
        arguments.market,
        arguments.source_receipt,
        &[arguments.payer, arguments.lookup_table],
    )?;
    let report = build_checkpoint_market_retirement_v1(&snapshot)
        .map_err(|error| Error::new(format!("checkpoint AggregateRetirement: {error:?}")))?;
    let table = find_observed_v1(&prestate, arguments.lookup_table, "retirement lookup table")?;
    authenticate_lookup_table_v1(table, arguments.lookup_table)?;
    let campaign = build_aggregate_retirement_campaign_v1(
        AggregateRetirementCampaignInputV1 {
            genesis_hash,
            rpc_url: arguments.origin.url().into(),
            plan_sha256: sha256_hex(plan_source),
            evidence_sha256: sha256_hex(evidence_source),
            payer: arguments.payer,
            lookup_table: arguments.lookup_table,
            lookup_table_sha256: sha256_hex(&table.data),
            core_program: snapshot.core_program.key,
            claims_program: snapshot.claims_program.key,
            market: initial_account(&snapshot.market),
            rent_credit: initial_account(&snapshot.rent_credit),
            checkpoint: initial_account(&snapshot.claims_aggregate),
            custody_replay: initial_account(&snapshot.custody_replay),
            hoard_vault: initial_account(&snapshot.hoard_vault),
            source_receipt: initial_account(&snapshot.source_receipt),
            refund_wallet: initial_account(&snapshot.refund_wallet),
        },
        &report,
    )?;
    create_json_v1(&arguments.campaign, &campaign, "retirement campaign")?;
    Ok(campaign)
}

fn authenticate_invocation_v1(
    campaign: &AggregateRetirementCampaignV1,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    plan_source: &[u8],
    evidence_source: &[u8],
    rpc_url: &str,
) -> Result<()> {
    crate::aggregate_retirement_journal::authenticate_aggregate_retirement_campaign_v1(campaign)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    if campaign.plan_sha256 != sha256_hex(plan_source)
        || campaign.evidence_sha256 != sha256_hex(evidence_source)
        || campaign.rpc_url != rpc_url
        || campaign.payer != arguments.payer.to_string()
        || campaign.lookup_table != arguments.lookup_table.to_string()
        || campaign.market.address != arguments.market.to_string()
        || campaign.source_receipt.address != arguments.source_receipt.to_string()
        || campaign.core_program != core.to_string()
        || campaign.claims_program != claims.to_string()
        || evidence.plan_sha256 != campaign.plan_sha256
    {
        return Err(refusal(
            "campaign did not bind the exact invocation, plan, evidence, or program set",
        ));
    }
    Ok(())
}

fn observe_projection_v1(
    rpc: &mut Rpc,
    campaign: &AggregateRetirementCampaignV1,
    minimum_slot: u64,
    fees: u64,
    allow_unreconciled_fee: bool,
) -> Result<AggregateRetirementChainProjectionV1> {
    let keys = campaign_account_keys_v1(campaign)?;
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    let accounts = keys
        .into_iter()
        .zip(values)
        .map(|(key, value)| {
            (
                key,
                value.map(|account| AggregateRetirementChainAccountV1 {
                    key,
                    owner: account.owner,
                    lamports: account.lamports,
                    executable: account.executable,
                    data: account.data,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    classify_aggregate_retirement_chain_v1(campaign, slot, &accounts, fees, allow_unreconciled_fee)
}

fn observe_lookup_table_v1(
    rpc: &mut Rpc,
    campaign: &AggregateRetirementCampaignV1,
    minimum_slot: u64,
) -> Result<ObservedAccount> {
    let key = Pubkey::from_str(&campaign.lookup_table)
        .map_err(|error| Error::new(format!("campaign lookup table: {error}")))?;
    let (_, mut accounts) = rpc.finalized_observed_accounts(&[key], minimum_slot)?;
    let table = accounts
        .pop()
        .ok_or_else(|| refusal("lookup observation omitted its account"))?;
    authenticate_lookup_table_v1(&table, key)?;
    if sha256_hex(&table.data) != campaign.lookup_table_sha256 {
        return Err(refusal(
            "lookup table bytes changed after campaign planning",
        ));
    }
    Ok(table)
}

fn authenticate_lookup_table_v1(table: &ObservedAccount, expected: Pubkey) -> Result<()> {
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| refusal("retirement lookup table bytes did not decode"))?;
    if table.key != expected
        || table.owner != lookup_table_program::id()
        || table.executable
        || decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.is_empty()
    {
        return Err(refusal(
            "retirement lookup table was not exact, frozen, activated routing data",
        ));
    }
    Ok(())
}

fn resolve_packet_keys_v1(
    signed: &SignedVersionedPacketV1,
    table_key: Pubkey,
    table: &ObservedAccount,
) -> Result<Vec<Pubkey>> {
    let bytes = decode_packet_v1(signed)?;
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("retirement packet: {error}")))?;
    let VersionedMessage::V0(message) = transaction.message else {
        return Err(refusal("retirement transaction was not v0"));
    };
    if message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != table_key
    {
        return Err(refusal(
            "retirement packet changed its one exact lookup table",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| refusal("retirement lookup table bytes did not decode"))?;
    let address = |index: u8| -> Result<Pubkey> {
        decoded
            .addresses
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| refusal("retirement lookup index exceeded frozen table"))
    };
    let mut result = message.account_keys.clone();
    result.extend(
        lookup
            .writable_indexes
            .iter()
            .copied()
            .map(address)
            .collect::<Result<Vec<_>>>()?,
    );
    result.extend(
        lookup
            .readonly_indexes
            .iter()
            .copied()
            .map(address)
            .collect::<Result<Vec<_>>>()?,
    );
    Ok(result)
}

fn poll_finalized_v1(
    rpc: &mut Rpc,
    journal: &AggregateRetirementJournalV1,
) -> Result<Option<crate::rpc::FinalizedSignedPacketV1>> {
    let packet = journal
        .packet
        .as_ref()
        .ok_or_else(|| refusal("signed journal omitted packet"))?;
    let signature = Signature::from_str(&packet.signed.signature)
        .map_err(|error| Error::new(format!("retirement signature: {error}")))?;
    rpc.finalized_signed_packet(journal.operation.label(), signature, false)
}

fn load_journals_v1(
    directory: &Path,
    campaign: &AggregateRetirementCampaignV1,
) -> Result<Vec<AggregateRetirementJournalV1>> {
    let mut result = Vec::new();
    let mut gap = false;
    for operation in AggregateRetirementOperationV1::ORDERED {
        let path = journal_path_v1(directory, operation);
        if !path.exists() {
            gap = true;
            continue;
        }
        if gap {
            return Err(refusal("retirement journals contained a filename gap"));
        }
        let journal: AggregateRetirementJournalV1 = read_json_v1(&path, "retirement journal")?;
        crate::aggregate_retirement_journal::authenticate_aggregate_retirement_journal_v1(
            campaign, &journal,
        )?;
        if journal.operation != operation {
            return Err(refusal("retirement journal filename changed operation"));
        }
        result.push(journal);
    }
    Ok(result)
}

fn write_completion_v1(
    arguments: &ArgumentsV1,
    campaign: &AggregateRetirementCampaignV1,
    journals: &[AggregateRetirementJournalV1],
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<()> {
    let receipt =
        build_aggregate_retirement_conservation_receipt_v1(campaign, journals, projection)?;
    write_or_authenticate_json_v1(&arguments.completion, &receipt, "retirement completion")?;
    stdout_v1(json!({
        "schema": PROGRESS_SCHEMA_V1,
        "status": "finalized",
        "campaign": arguments.campaign.display().to_string(),
        "campaignSha256": campaign.campaign_sha256,
        "journalDirectory": arguments.journal_dir.display().to_string(),
        "completion": arguments.completion.display().to_string(),
        "completionSha256": sha256_hex(&fs::read(&arguments.completion)?),
        "message": "Aggregate retirement finalized through prepare, close-vault, close-replay, and finish; exact rent/refund conservation reverified."
    }))
}

fn progress_v1(
    arguments: &ArgumentsV1,
    campaign: &AggregateRetirementCampaignV1,
    operation: AggregateRetirementOperationV1,
    status: &str,
    journal: Option<&Path>,
    message: &str,
) -> Result<()> {
    stdout_v1(json!({
        "schema": PROGRESS_SCHEMA_V1,
        "status": status,
        "operation": operation,
        "campaign": arguments.campaign.display().to_string(),
        "campaignSha256": campaign.campaign_sha256,
        "journal": journal.map(|path| path.display().to_string()),
        "completion": arguments.completion.display().to_string(),
        "message": message
    }))
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | "--plan"
                | "--evidence"
                | "--market"
                | "--source-receipt"
                | "--fee-payer"
                | "--fee-payer-keypair"
                | "--lookup-table"
                | "--campaign"
                | "--journal-dir"
                | "--completion"
        ) {
            return Err(Error::new(format!(
                "unknown {COMMAND_V1} argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let take = |values: &mut BTreeMap<String, String>, flag: &str| {
        values
            .remove(flag)
            .ok_or_else(|| Error::new(format!("{flag} is required")))
    };
    let absolute = |value: String, flag: &str| -> Result<PathBuf> {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err(Error::new(format!("{flag} must be absolute")));
        }
        Ok(path)
    };
    let rpc_url = take(&mut values, "--rpc-url")?;
    let parse_key = |value: String, flag: &str| {
        Pubkey::from_str(&value).map_err(|error| Error::new(format!("{flag}: {error}")))
    };
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, None)?,
        plan: absolute(take(&mut values, "--plan")?, "--plan")?,
        evidence: absolute(take(&mut values, "--evidence")?, "--evidence")?,
        market: parse_key(take(&mut values, "--market")?, "--market")?,
        source_receipt: parse_key(take(&mut values, "--source-receipt")?, "--source-receipt")?,
        payer: parse_key(take(&mut values, "--fee-payer")?, "--fee-payer")?,
        payer_keypair: absolute(
            take(&mut values, "--fee-payer-keypair")?,
            "--fee-payer-keypair",
        )?,
        lookup_table: parse_key(take(&mut values, "--lookup-table")?, "--lookup-table")?,
        campaign: absolute(take(&mut values, "--campaign")?, "--campaign")?,
        journal_dir: absolute(take(&mut values, "--journal-dir")?, "--journal-dir")?,
        completion: absolute(take(&mut values, "--completion")?, "--completion")?,
        execute,
    })
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-private-validator-aggregate-retirement-v1 \\\n+     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \\\n+     --evidence ABSOLUTE_JSON --market PUBKEY --source-receipt PUBKEY \\\n+     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \\\n+     --lookup-table PUBKEY --campaign ABSOLUTE_JSON \\\n+     --journal-dir ABSOLUTE_DIRECTORY --completion ABSOLUTE_JSON [--execute]\n\nWithout \\
     --execute this command performs finalized owned-loopback reads, creates or authenticates the \\
     immutable four-packet campaign, and persists only the next unsigned Planned journal. Execute \\
     reads the named payer key only from Prepared, fsyncs Dispatching before the first send, polls \\
     before an identical resend, and makes Submitted poll-only. Rerun until /status=finalized."
}

fn initial_account(
    value: &ObservedAccount,
) -> crate::aggregate_retirement_journal::AggregateRetirementInitialAccountV1 {
    crate::aggregate_retirement_journal::AggregateRetirementInitialAccountV1 {
        key: value.key,
        owner: value.owner,
        lamports: value.lamports,
        executable: value.executable,
        data: value.data.clone(),
    }
}

fn find_observed_v1<'a>(
    values: &'a [ObservedAccount],
    key: Pubkey,
    label: &str,
) -> Result<&'a ObservedAccount> {
    values
        .iter()
        .find(|account| account.key == key)
        .ok_or_else(|| refusal(format!("initial snapshot omitted {label}")))
}

fn campaign_account_keys_v1(campaign: &AggregateRetirementCampaignV1) -> Result<Vec<Pubkey>> {
    [
        &campaign.market.address,
        &campaign.rent_credit.address,
        &campaign.checkpoint.address,
        &campaign.custody_replay.address,
        &campaign.hoard_vault.address,
        &campaign.source_receipt.address,
        &campaign.refund_wallet.address,
    ]
    .into_iter()
    .map(|value| {
        Pubkey::from_str(value)
            .map_err(|error| Error::new(format!("campaign account key: {error}")))
    })
    .collect()
}

fn finalized_fee_total_v1(journals: &[AggregateRetirementJournalV1]) -> Result<u64> {
    journals.iter().try_fold(0u64, |sum, journal| {
        let fee = journal
            .finalization
            .as_ref()
            .map(|value| value.fee_lamports)
            .unwrap_or(0);
        sum.checked_add(fee)
            .ok_or_else(|| refusal("retirement fee total overflowed"))
    })
}

fn journal_path_v1(directory: &Path, operation: AggregateRetirementOperationV1) -> PathBuf {
    directory.join(JOURNAL_NAMES_V1[operation.ordinal()])
}

fn journal_phase_text(phase: AggregateRetirementJournalPhaseV1) -> &'static str {
    match phase {
        AggregateRetirementJournalPhaseV1::Planned => "planned",
        AggregateRetirementJournalPhaseV1::Prepared => "prepared",
        AggregateRetirementJournalPhaseV1::Dispatching => "dispatching",
        AggregateRetirementJournalPhaseV1::Submitted => "submitted",
        AggregateRetirementJournalPhaseV1::Finalized => "finalized",
    }
}

fn recovery_message(recovery: AggregateRetirementRecoveryV1) -> &'static str {
    match recovery {
        AggregateRetirementRecoveryV1::PersistPrepared => {
            "The exact Planned intent is durable; execute reauthenticates its prestate before any key read."
        }
        AggregateRetirementRecoveryV1::SignOnceAndPersistDispatching => {
            "Prepared is durable; execute signs once and fsyncs Dispatching before send."
        }
        AggregateRetirementRecoveryV1::PollThenResendIdentical => {
            "Dispatching is durable; execute polls first and can only resend identical bytes."
        }
        AggregateRetirementRecoveryV1::PollOnly => {
            "Submitted is durable; execute is poll-only for the exact signature."
        }
        AggregateRetirementRecoveryV1::Complete => "The exact journal is finalized.",
    }
}

fn decode_packet_v1(packet: &SignedVersionedPacketV1) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(&packet.packet_base64)
        .map_err(|error| Error::new(format!("retirement packet base64: {error}")))?;
    if BASE64.encode(&bytes) != packet.packet_base64 || sha256_hex(&bytes) != packet.packet_sha256 {
        return Err(refusal("retirement packet bytes or digest changed"));
    }
    Ok(bytes)
}

fn read_bounded(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .map_err(|error| Error::new(format!("read {label} {}: {error}", path.display())))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 16 * 1024 * 1024 {
        return Err(refusal(format!(
            "{label} was outside the 1..16777216 byte bound"
        )));
    }
    fs::read(path).map_err(Into::into)
}

fn read_json_v1<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let source = read_bounded(path, label)?;
    serde_json::from_slice(&source).map_err(Into::into)
}

fn create_json_v1<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<()> {
    let bytes = canonical_json_v1(value)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| Error::new(format!("create {label} {}: {error}", path.display())))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    sync_parent_v1(path)
}

fn replace_json_v1<T: Serialize>(path: &Path, expected: &T, next: &T, label: &str) -> Result<()> {
    let expected_bytes = canonical_json_v1(expected)?;
    let current = fs::read(path)?;
    if current != expected_bytes {
        return Err(refusal(format!(
            "{label} changed between authentication and transition"
        )));
    }
    let next_bytes = canonical_json_v1(next)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(&next_bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path)?;
    sync_parent_v1(path)
}

fn write_or_authenticate_json_v1<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<()> {
    let bytes = canonical_json_v1(value)?;
    if path.exists() {
        if fs::read(path)? != bytes {
            return Err(refusal(format!("existing {label} changed")));
        }
        return Ok(());
    }
    create_json_v1(path, value, label)
}

fn canonical_json_v1<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sync_parent_v1(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("durable output omitted a parent directory"))?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn stdout_v1(value: Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &value)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!(
        "REFUSED aggregate retirement exterior: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(root: &str) -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "http://127.0.0.1:42321".into(),
            "--plan".into(),
            format!("{root}/plan.json"),
            "--evidence".into(),
            format!("{root}/evidence.json"),
            "--market".into(),
            Pubkey::new_unique().to_string(),
            "--source-receipt".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer-keypair".into(),
            format!("{root}/payer.json"),
            "--lookup-table".into(),
            Pubkey::new_unique().to_string(),
            "--campaign".into(),
            format!("{root}/campaign.json"),
            "--journal-dir".into(),
            root.into(),
            "--completion".into(),
            format!("{root}/completion.json"),
        ]
    }

    #[test]
    fn argv_freezes_exact_private_exterior_surface() {
        let parsed =
            parse_arguments_v1(argv("/private/tmp/aggregate-retirement")).expect("exact argv");
        assert!(!parsed.execute);
        let mut execute = argv("/private/tmp/aggregate-retirement");
        execute.push("--execute".into());
        assert!(parse_arguments_v1(execute).expect("execute argv").execute);
    }

    #[test]
    fn argv_refuses_external_origins_duplicates_and_relative_outputs() {
        let mut external = argv("/private/tmp/aggregate-retirement");
        external[1] = "https://api.devnet.solana.com".into();
        assert!(parse_arguments_v1(external).is_err());
        let mut duplicate = argv("/private/tmp/aggregate-retirement");
        duplicate.extend(["--market".into(), Pubkey::new_unique().to_string()]);
        assert!(parse_arguments_v1(duplicate).is_err());
        let mut relative = argv("/private/tmp/aggregate-retirement");
        let index = relative
            .iter()
            .position(|value| value == "--campaign")
            .expect("campaign flag")
            + 1;
        relative[index] = "campaign.json".into();
        assert!(parse_arguments_v1(relative).is_err());
    }

    #[test]
    fn journal_filenames_are_exact_predecessor_order() {
        assert_eq!(
            AggregateRetirementOperationV1::ORDERED
                .into_iter()
                .map(|operation| {
                    journal_path_v1(Path::new("/tmp/journals"), operation)
                        .file_name()
                        .expect("filename")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>(),
            JOURNAL_NAMES_V1
        );
    }
}
