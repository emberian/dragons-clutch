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
    chaos_fault::{self, BoundaryV1},
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
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
/// The same retirement, against a public devnet cohort.
///
/// Retirement was reachable on exactly one cluster because two lines said so:
/// `ExpectedClusterV1::OwnedLoopback` was a literal in `run`, and the
/// acknowledgment handed to `ClusterOriginV1::parse` was a literal `None`. The
/// retirement itself never had a loopback assumption in it -- the packets, the
/// journal, the vault close and the rent arithmetic are cluster-blind -- so the
/// devnet arm is a second entry point and a threaded expectation, not a second
/// implementation. The idiom is `claims_custody_replay.rs`'s.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-aggregate-retirement-v1";

const fn command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
    }
}
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
    /// The finalized-slot refresh that carries the Direct EXECUTION capability
    /// root. Optional, and a market that never activated needs none.
    refreshed_evidence: Option<PathBuf>,
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

/// Family-neutral transport inputs for one already-authenticated aggregate
/// Market retirement campaign. Family exteriors own how the live snapshot and
/// campaign were authorized; this engine owns only the exact four-operation
/// journal, signed-packet, resend/poll, and conservation boundaries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AggregateRetirementTransportV1<'a> {
    pub(crate) campaign_path: &'a Path,
    pub(crate) journal_dir: &'a Path,
    pub(crate) completion: &'a Path,
    pub(crate) payer: Pubkey,
    pub(crate) payer_keypair: &'a Path,
    pub(crate) lookup_table: Pubkey,
    pub(crate) execute: bool,
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::Devnet)
}

fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments_v1(arguments, expected)?;
    expected.authenticate(&arguments.origin)?;
    if !arguments.journal_dir.is_dir() {
        return Err(refusal(
            "--journal-dir must be an existing absolute directory",
        ));
    }
    let plan_source = read_bounded(&arguments.plan, "successor plan")?;
    let evidence_source = read_bounded(&arguments.evidence, "terminal evidence")?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let founding_evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_source, expected)?;
    authenticate_plan_source(&plan_source, &founding_evidence.plan_sha256)?;
    let refreshed_source = arguments
        .refreshed_evidence
        .as_ref()
        .map(|path| read_bounded(path, "refreshed terminal evidence"))
        .transpose()?;

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
    // `direct_capability_root` names two different addresses, and the founding
    // campaign carries the wrong one for this stage: the founding checkpoint's
    // scalar is the founding-PERMIT root, at which no account can ever exist,
    // while the terminal sequence means the EXECUTION root that activation
    // creates. Only an evidence refresh emits the second under that label
    // (`docs/design/EVIDENCE_REFRESH_V1.md` §3), which is why
    // `terminal_sequence.rs` already takes one and why retirement -- the last
    // stage of the same lifecycle -- could not run without it.
    //
    // The refresh widens WHICH DOCUMENT MAY CARRY A ROW and nothing else: the
    // two checks below run unchanged against the effective map, and with no
    // refresh supplied the sequence is byte-for-byte what it was. They moved
    // below the cluster connect because a refresh has to be admitted against a
    // finalized slot before the evidence it produces can be judged.
    let evidence = match refreshed_source.as_deref() {
        None => founding_evidence,
        Some(bytes) => {
            let refresh = crate::evidence_refresh::parse_refresh_v1(bytes)?;
            let effective = crate::evidence_refresh::effective_accounts_v1(
                &refresh,
                &evidence_source,
                &crate::terminal_sequence::terminal_rows_as_model_v1(&founding_evidence.accounts),
                &founding_evidence.plan_sha256,
                expected,
                rpc.finalized_slot()?,
            )?;
            let founding_custody_context = crate::evidence_refresh::effective_custody_context_v1(
                Some(&refresh),
                &founding_evidence.founding_custody_context,
            )?;
            CampaignTerminalEvidenceV1 {
                accounts: crate::terminal_sequence::model_rows_as_terminal_v1(effective),
                founding_custody_context,
                ..founding_evidence
            }
        }
    };
    require_direct_retirement_evidence(&evidence)?;
    authenticate_campaign_market_v1(&evidence, arguments.market)?;

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

    run_authenticated_aggregate_retirement_v1(
        &mut rpc,
        &campaign,
        AggregateRetirementTransportV1 {
            campaign_path: &arguments.campaign,
            journal_dir: &arguments.journal_dir,
            completion: &arguments.completion,
            payer: arguments.payer,
            payer_keypair: &arguments.payer_keypair,
            lookup_table: arguments.lookup_table,
            execute: arguments.execute,
        },
    )
}

/// Advance one exact generic retirement operation. The caller cannot bypass
/// authentication by handing this function an in-memory projection: the
/// canonical campaign file, live RPC URL/genesis, payer, frozen lookup table,
/// and semantic campaign digest are all reauthenticated before any journal or
/// key is opened.
pub(crate) fn run_authenticated_aggregate_retirement_v1(
    rpc: &mut Rpc,
    campaign: &AggregateRetirementCampaignV1,
    transport: AggregateRetirementTransportV1<'_>,
) -> Result<()> {
    authenticate_aggregate_retirement_transport_v1(rpc, campaign, transport)?;
    operate_authenticated_v1(rpc, transport, campaign)
}

fn authenticate_aggregate_retirement_transport_v1(
    rpc: &mut Rpc,
    campaign: &AggregateRetirementCampaignV1,
    transport: AggregateRetirementTransportV1<'_>,
) -> Result<()> {
    authenticate_aggregate_retirement_transport_durable_v1(campaign, transport, rpc.url())?;
    let genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("getGenesisHash returned a non-string"))?
        .to_owned();
    if genesis_hash != campaign.genesis_hash {
        return Err(refusal(
            "generic retirement transport changed validator genesis",
        ));
    }
    Ok(())
}

fn authenticate_aggregate_retirement_transport_durable_v1(
    campaign: &AggregateRetirementCampaignV1,
    transport: AggregateRetirementTransportV1<'_>,
    rpc_url: &str,
) -> Result<()> {
    crate::aggregate_retirement_journal::authenticate_aggregate_retirement_campaign_v1(campaign)?;
    if [
        transport.campaign_path,
        transport.journal_dir,
        transport.completion,
        transport.payer_keypair,
    ]
    .into_iter()
    .any(|path| !path.is_absolute())
    {
        return Err(refusal(
            "generic retirement transport requires absolute durable paths",
        ));
    }
    let journal_metadata = fs::symlink_metadata(transport.journal_dir).map_err(|error| {
        Error::new(format!(
            "retirement journal directory {}: {error}",
            transport.journal_dir.display()
        ))
    })?;
    if !journal_metadata.file_type().is_dir() {
        return Err(refusal(
            "retirement journal directory was not a real directory",
        ));
    }
    let completion_parent = transport
        .completion
        .parent()
        .ok_or_else(|| refusal("retirement completion omitted a parent directory"))?;
    let completion_parent_metadata = fs::symlink_metadata(completion_parent).map_err(|error| {
        Error::new(format!(
            "retirement completion parent {}: {error}",
            completion_parent.display()
        ))
    })?;
    let durable_paths_alias = transport.campaign_path == transport.completion
        || AggregateRetirementOperationV1::ORDERED
            .into_iter()
            .map(|operation| journal_path_v1(transport.journal_dir, operation))
            .any(|journal| journal == transport.campaign_path || journal == transport.completion);
    if !completion_parent_metadata.file_type().is_dir() || durable_paths_alias {
        return Err(refusal(
            "retirement campaign, journals, and completion did not have disjoint durable paths",
        ));
    }
    let durable: AggregateRetirementCampaignV1 =
        read_json_v1(transport.campaign_path, "retirement campaign")?;
    if &durable != campaign
        || campaign.payer != transport.payer.to_string()
        || campaign.lookup_table != transport.lookup_table.to_string()
        || campaign.rpc_url != rpc_url
    {
        return Err(refusal(
            "generic retirement transport changed its durable campaign, payer, lookup table, or RPC",
        ));
    }
    Ok(())
}

fn operate_authenticated_v1(
    rpc: &mut Rpc,
    transport: AggregateRetirementTransportV1<'_>,
    campaign: &AggregateRetirementCampaignV1,
) -> Result<()> {
    let journals = load_journals_v1(transport.journal_dir, campaign)?;
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
            write_completion_v1(transport, campaign, &journals, &projection)
        }
        AggregateRetirementRouteV1::Plan(operation) => {
            let journal = plan_aggregate_retirement_journal_v1(campaign, operation, &projection)?;
            let path = journal_path_v1(transport.journal_dir, operation);
            create_json_v1(&path, &journal, "retirement journal")?;
            if !transport.execute {
                return progress_v1(
                    transport,
                    campaign,
                    operation,
                    "planned",
                    Some(&path),
                    "The next mutation is planned from finalized chain state; no key was read.",
                );
            }
            advance_active_v1(rpc, transport, campaign, journal, projection)
        }
        AggregateRetirementRouteV1::Recover(operation, recovery) => {
            let journal = journals
                .last()
                .cloned()
                .ok_or_else(|| refusal("recovery route omitted its active journal"))?;
            if journal.operation != operation {
                return Err(refusal("recovery route changed the active operation"));
            }
            if !transport.execute {
                return progress_v1(
                    transport,
                    campaign,
                    operation,
                    journal_phase_text(journal.phase),
                    Some(&journal_path_v1(transport.journal_dir, operation)),
                    recovery_message(recovery),
                );
            }
            advance_active_v1(rpc, transport, campaign, journal, projection)
        }
    }
}

fn advance_active_v1(
    rpc: &mut Rpc,
    transport: AggregateRetirementTransportV1<'_>,
    campaign: &AggregateRetirementCampaignV1,
    mut journal: AggregateRetirementJournalV1,
    mut projection: AggregateRetirementChainProjectionV1,
) -> Result<()> {
    let path = journal_path_v1(transport.journal_dir, journal.operation);
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
            transport.payer_keypair,
            "AggregateRetirement fee payer",
        )?);
        if keypair.pubkey() != transport.payer {
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
            transport.payer,
            &table,
            &signed,
        )?;
        let resolved = resolve_packet_keys_v1(&signed, transport.lookup_table, &table)?;
        let binding = build_aggregate_retirement_packet_binding_v1(
            campaign,
            journal.operation,
            signed,
            resolved,
        )?;
        let next = dispatch_aggregate_retirement_journal_v1(campaign, &journal, binding)?;
        replace_json_v1(&path, &journal, &next, "retirement journal")?;
        journal = next;
        park_finish_chaos_boundary_v1(
            campaign,
            &journal,
            &path,
            BoundaryV1::DispatchingBeforeSend,
        )?;
    }
    if journal.phase == AggregateRetirementJournalPhaseV1::Dispatching {
        if let Some(finalized) = poll_finalized_v1(rpc, &journal)? {
            let next = submit_aggregate_retirement_journal_v1(
                campaign,
                &journal,
                &finalized.evidence.signature,
            )?;
            replace_json_v1(&path, &journal, &next, "retirement journal")?;
            return finalize_active_v1(rpc, transport, campaign, next, finalized, &path);
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
            transport.payer,
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
            return finalize_active_v1(rpc, transport, campaign, journal, finalized, &path);
        }
        return progress_v1(
            transport,
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
            finalized_fee_total_v1(&load_journals_v1(transport.journal_dir, campaign)?)?,
            false,
        )?;
        return progress_v1(
            transport,
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
    transport: AggregateRetirementTransportV1<'_>,
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
    let previous = load_journals_v1(transport.journal_dir, campaign)?;
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
    park_finish_chaos_boundary_v1(
        campaign,
        &journal,
        path,
        BoundaryV1::LandedBeforeFinalizationFsync,
    )?;
    replace_json_v1(path, &journal, &next, "retirement journal")?;
    let journals = load_journals_v1(transport.journal_dir, campaign)?;
    if route_aggregate_retirement_v1(campaign, &journals, &projection)?
        == AggregateRetirementRouteV1::Complete
    {
        return write_completion_v1(transport, campaign, &journals, &projection);
    }
    progress_v1(
        transport,
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
    if journal.operation == AggregateRetirementOperationV1::Finish
        && chaos_fault::is_armed_for_v1(
            journal.operation.label(),
            BoundaryV1::LandedBeforeFinalizationFsync,
        )?
    {
        for _ in 0..300 {
            if let Some(finalized) =
                rpc.finalized_signed_packet(journal.operation.label(), signature, false)?
            {
                return Ok(Some(finalized));
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        return Err(refusal(
            "aggregate-retirement-finish chaos target did not reach finalized history before its fault boundary",
        ));
    }
    rpc.finalized_signed_packet(journal.operation.label(), signature, false)
}

fn park_finish_chaos_boundary_v1(
    campaign: &AggregateRetirementCampaignV1,
    journal: &AggregateRetirementJournalV1,
    path: &Path,
    boundary: BoundaryV1,
) -> Result<()> {
    if journal.operation != AggregateRetirementOperationV1::Finish {
        return Ok(());
    }
    let packet = journal
        .packet
        .as_ref()
        .ok_or_else(|| refusal("aggregate-retirement-finish chaos boundary omitted packet"))?;
    chaos_fault::park_if_armed_v1(
        &campaign.cluster,
        journal.operation.label(),
        boundary,
        path,
        &journal.intent_sha256,
        &packet.signed.packet_sha256,
        &packet.signed.signature,
    )
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
    transport: AggregateRetirementTransportV1<'_>,
    campaign: &AggregateRetirementCampaignV1,
    journals: &[AggregateRetirementJournalV1],
    projection: &AggregateRetirementChainProjectionV1,
) -> Result<()> {
    let receipt =
        build_aggregate_retirement_conservation_receipt_v1(campaign, journals, projection)?;
    write_or_authenticate_json_v1(transport.completion, &receipt, "retirement completion")?;
    stdout_v1(json!({
        "schema": PROGRESS_SCHEMA_V1,
        "status": "finalized",
        "campaign": transport.campaign_path.display().to_string(),
        "campaignSha256": campaign.campaign_sha256,
        "journalDirectory": transport.journal_dir.display().to_string(),
        "completion": transport.completion.display().to_string(),
        "completionSha256": sha256_hex(&fs::read(transport.completion)?),
        "message": "Aggregate retirement finalized through prepare, close-vault, close-replay, and finish; exact rent/refund conservation reverified."
    }))
}

fn progress_v1(
    transport: AggregateRetirementTransportV1<'_>,
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
        "campaign": transport.campaign_path.display().to_string(),
        "campaignSha256": campaign.campaign_sha256,
        "journal": journal.map(|path| path.display().to_string()),
        "completion": transport.completion.display().to_string(),
        "message": message
    }))
}

fn parse_arguments_v1(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut acknowledgment = None;
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
        // Accepted on the devnet arm only. On the loopback arm it falls through
        // to the unknown-argument refusal below, which names the command that
        // does take it.
        if argument == DEVNET_ACKNOWLEDGMENT_FLAG && expected == ExpectedClusterV1::Devnet {
            if acknowledgment.replace(value).is_some() {
                return Err(Error::new(format!("{argument} may be supplied only once")));
            }
            continue;
        }
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | "--plan"
                | "--evidence"
                | "--refreshed-evidence"
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
                "unknown {} argument: {argument}",
                command(expected)
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
    if expected == ExpectedClusterV1::Devnet && acknowledgment.is_none() {
        return Err(Error::new(format!(
            "{DEVNET_ACKNOWLEDGMENT_FLAG} is required by {COMMAND_DEVNET_V1}"
        )));
    }
    let parse_key = |value: String, flag: &str| {
        Pubkey::from_str(&value).map_err(|error| Error::new(format!("{flag}: {error}")))
    };
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(take(&mut values, "--plan")?, "--plan")?,
        evidence: absolute(take(&mut values, "--evidence")?, "--evidence")?,
        refreshed_evidence: values
            .remove("--refreshed-evidence")
            .map(|value| absolute(value, "--refreshed-evidence"))
            .transpose()?,
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

pub(crate) fn devnet_usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap devnet-aggregate-retirement-v1 \\\n     \
     --rpc-url https://api.devnet.solana.com --i-mean-devnet DEVNET_GENESIS \\\n     \
     --plan ABSOLUTE_JSON \\\n     \
     --evidence ABSOLUTE_JSON [--refreshed-evidence ABSOLUTE_JSON] --market PUBKEY --source-receipt PUBKEY \\\n     \
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \\\n     \
     --lookup-table PUBKEY --campaign ABSOLUTE_JSON \\\n     \
     --journal-dir ABSOLUTE_DIRECTORY --completion ABSOLUTE_JSON [--execute]\n\nThe same \
     retirement as the loopback command against a public devnet cohort. The four packets, the \
     journal, the vault close and the rent arithmetic are identical; only the expected cluster \
     is threaded rather than fixed."
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-private-validator-aggregate-retirement-v1 \\\n     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \\\n     --evidence ABSOLUTE_JSON [--refreshed-evidence ABSOLUTE_JSON] --market PUBKEY --source-receipt PUBKEY \\\n     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \\\n     --lookup-table PUBKEY --campaign ABSOLUTE_JSON \\\n     --journal-dir ABSOLUTE_DIRECTORY --completion ABSOLUTE_JSON [--execute]\n\nWithout \\
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
    use crate::aggregate_retirement_journal::AggregateRetirementInitialAccountV1;
    use dclutch_market::{
        AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1, AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
        AGGREGATE_RETIREMENT_FINISH_MAGIC_V1, AggregateRetirementSuffixBindingV1,
        AggregateRetirementSuffixRequestV1,
    };
    use dclutch_market_retirement_v1_operator::{
        CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, CHECKPOINT_RETIREMENT_FINISH_BYTES_V1,
        CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1, CheckpointMarketRetirementReportV1, Finality,
        Observation,
    };
    use solana_program::instruction::{AccountMeta, Instruction};
    use solana_sdk_ids::system_program;

    use super::*;

    /// The command line every cluster arm shares, minus the two coordinates
    /// that separate them.
    fn retirement_arguments(extra: &[&str]) -> Vec<String> {
        let mut argv: Vec<String> = [
            "--rpc-url",
            "http://127.0.0.1:8899",
            "--plan",
            "/tmp/plan.json",
            "--evidence",
            "/tmp/evidence.json",
            "--market",
            "11111111111111111111111111111112",
            "--source-receipt",
            "11111111111111111111111111111113",
            "--fee-payer",
            "11111111111111111111111111111114",
            "--fee-payer-keypair",
            "/tmp/payer.json",
            "--lookup-table",
            "11111111111111111111111111111115",
            "--campaign",
            "/tmp/campaign.json",
            "--journal-dir",
            "/tmp/journal",
            "--completion",
            "/tmp/completion.json",
        ]
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
        argv.extend(extra.iter().map(|value| (*value).to_owned()));
        argv
    }

    /// A refresh is optional and absolute, and both arms take it.
    ///
    /// Retirement is the last stage of the same lifecycle `terminal_sequence`
    /// runs, and it consumes the same document for the same reason: the
    /// founding campaign carries the founding-PERMIT capability root, at which
    /// no account can ever exist, and only a refresh emits the EXECUTION root
    /// under that label.
    #[test]
    fn retirement_takes_an_optional_absolute_refreshed_evidence_document() {
        let parsed = parse_arguments_v1(
            retirement_arguments(&["--refreshed-evidence", "/tmp/refresh.json"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect("a refresh is an accepted argument");
        assert_eq!(
            parsed.refreshed_evidence.as_deref(),
            Some(std::path::Path::new("/tmp/refresh.json")),
        );
        let parsed =
            parse_arguments_v1(retirement_arguments(&[]), ExpectedClusterV1::OwnedLoopback)
                .expect("a refresh is optional");
        assert_eq!(parsed.refreshed_evidence, None);
    }

    #[test]
    fn a_relative_refresh_is_refused_by_name() {
        let Err(refusal) = parse_arguments_v1(
            retirement_arguments(&["--refreshed-evidence", "refresh.json"]),
            ExpectedClusterV1::OwnedLoopback,
        ) else {
            panic!("a relative refresh path must be refused");
        };
        assert!(
            refusal.to_string().contains("--refreshed-evidence"),
            "the refusal must name the flag: {refusal}"
        );
    }

    #[test]
    fn a_repeated_refresh_is_refused() {
        let Err(refusal) = parse_arguments_v1(
            retirement_arguments(&[
                "--refreshed-evidence",
                "/tmp/a.json",
                "--refreshed-evidence",
                "/tmp/b.json",
            ]),
            ExpectedClusterV1::OwnedLoopback,
        ) else {
            panic!("two refreshes must be refused");
        };
        assert!(
            refusal.to_string().contains("only once"),
            "unexpected refusal: {refusal}"
        );
    }

    /// The devnet acknowledgment belongs to the public arm alone, and the
    /// loopback arm names the command that does take it rather than refusing
    /// it as an anonymous unknown argument.
    #[test]
    fn the_devnet_acknowledgment_belongs_to_the_public_retirement_arm_alone() {
        let Err(refusal) = parse_arguments_v1(
            retirement_arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "SomeGenesisHash"]),
            ExpectedClusterV1::OwnedLoopback,
        ) else {
            panic!("the loopback arm must not take a devnet acknowledgment");
        };
        let text = refusal.to_string();
        assert!(
            text.contains(COMMAND_V1) && text.contains(DEVNET_ACKNOWLEDGMENT_FLAG),
            "expected the loopback arm to name itself and the flag, got: {text}"
        );
        // The same command line without the flag parses, so the refusal above
        // is about the flag and not about the rest of it.
        assert!(
            parse_arguments_v1(retirement_arguments(&[]), ExpectedClusterV1::OwnedLoopback).is_ok(),
            "the loopback arm must parse its own command line"
        );
    }

    /// The public arm REQUIRES the acknowledgment, and says which command
    /// requires it.
    #[test]
    fn the_public_retirement_arm_requires_the_devnet_acknowledgment() {
        let Err(refusal) = parse_arguments_v1(retirement_arguments(&[]), ExpectedClusterV1::Devnet)
        else {
            panic!("the devnet arm must not run unacknowledged");
        };
        let text = refusal.to_string();
        assert!(
            text.contains(DEVNET_ACKNOWLEDGMENT_FLAG) && text.contains(COMMAND_DEVNET_V1),
            "expected the devnet arm to name the flag and itself, got: {text}"
        );
    }

    /// A loopback URL is refused by the public arm at ORIGIN PARSING -- before
    /// the cluster check, before any key, before any read. An acknowledgment
    /// given for a loopback socket means one of the two is a typo and nothing
    /// here can tell which.
    #[test]
    fn the_public_retirement_arm_refuses_a_loopback_origin_before_any_key_or_read() {
        let Err(refusal) = parse_arguments_v1(
            retirement_arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "SomeGenesisHash"]),
            ExpectedClusterV1::Devnet,
        ) else {
            panic!("a loopback socket must not be acknowledged as devnet");
        };
        assert!(
            refusal
                .to_string()
                .contains("was given for the loopback origin"),
            "expected the origin parser's loopback refusal, got: {refusal}"
        );
    }

    /// The two arms are two names for one implementation, and the selector is
    /// total.
    #[test]
    fn each_cluster_names_its_own_retirement_command() {
        assert_eq!(command(ExpectedClusterV1::OwnedLoopback), COMMAND_V1);
        assert_eq!(command(ExpectedClusterV1::Devnet), COMMAND_DEVNET_V1);
        assert_ne!(COMMAND_V1, COMMAND_DEVNET_V1);
        assert!(devnet_usage().contains(COMMAND_DEVNET_V1));
        assert!(devnet_usage().contains(DEVNET_ACKNOWLEDGMENT_FLAG));
        assert!(usage().contains(COMMAND_V1));
        assert!(!usage().contains(DEVNET_ACKNOWLEDGMENT_FLAG));
    }

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn initial_account_v1(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    ) -> AggregateRetirementInitialAccountV1 {
        AggregateRetirementInitialAccountV1 {
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn campaign_fixture_v1() -> AggregateRetirementCampaignV1 {
        let core = key(5);
        let claims = key(6);
        let market = key(40);
        let checkpoint = key(41);
        let mut accounts = (1..=35)
            .map(|byte| AccountMeta::new_readonly(key(byte), false))
            .collect::<Vec<_>>();
        accounts[0] = AccountMeta::new(market, false);
        accounts[4] = AccountMeta::new_readonly(core, false);
        accounts[14] = AccountMeta::new(checkpoint, false);
        let binding = AggregateRetirementSuffixBindingV1 {
            market: market.to_bytes(),
            checkpoint: checkpoint.to_bytes(),
            bundle_digest: [7; 32],
            source_receipt_digest: [8; 32],
        };
        let suffix = |magic, phase, custody| {
            AggregateRetirementSuffixRequestV1::new(
                magic,
                binding,
                if magic == AGGREGATE_RETIREMENT_FINISH_MAGIC_V1 {
                    [0; 32]
                } else {
                    [9; 32]
                },
                phase,
                custody,
            )
            .expect("retirement suffix")
            .to_bytes()
        };
        let instruction = |data: Vec<u8>| Instruction {
            program_id: core,
            accounts: accounts.clone(),
            data,
        };
        let mut close_vault = suffix(AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1, 1, 2).to_vec();
        close_vault.resize(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, 0x41);
        let mut close_replay = suffix(AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1, 2, 3).to_vec();
        close_replay.resize(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, 0x42);
        let mut finish = suffix(AGGREGATE_RETIREMENT_FINISH_MAGIC_V1, 3, 4).to_vec();
        finish.resize(CHECKPOINT_RETIREMENT_FINISH_BYTES_V1, 0x43);
        let report = CheckpointMarketRetirementReportV1 {
            prepare: instruction(vec![0x40; CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1]),
            close_vault: instruction(close_vault),
            close_replay: instruction(close_replay),
            finish: instruction(finish),
            observation: Observation {
                slot: 9,
                unix_timestamp: 10,
                finality: Finality::Finalized,
            },
            expected_refund_delta: 150,
        };
        build_aggregate_retirement_campaign_v1(
            AggregateRetirementCampaignInputV1 {
                genesis_hash: key(90).to_string(),
                rpc_url: "http://127.0.0.1:43210/".into(),
                plan_sha256: "11".repeat(32),
                evidence_sha256: "22".repeat(32),
                payer: key(80),
                lookup_table: key(81),
                lookup_table_sha256: "33".repeat(32),
                core_program: core,
                claims_program: claims,
                market: initial_account_v1(market, core, 10, vec![1]),
                rent_credit: initial_account_v1(key(42), key(7), 20, vec![2]),
                checkpoint: initial_account_v1(checkpoint, claims, 30, vec![3]),
                custody_replay: initial_account_v1(key(43), key(8), 40, vec![4]),
                hoard_vault: initial_account_v1(key(44), key(8), 50, vec![5]),
                source_receipt: initial_account_v1(key(45), key(9), 1, vec![6]),
                refund_wallet: initial_account_v1(key(46), system_program::ID, 1_000, Vec::new()),
            },
            &report,
        )
        .expect("generic retirement campaign")
    }

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
        let parsed = parse_arguments_v1(
            argv("/private/tmp/aggregate-retirement"),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect("exact argv");
        assert!(!parsed.execute);
        let mut execute = argv("/private/tmp/aggregate-retirement");
        execute.push("--execute".into());
        assert!(
            parse_arguments_v1(execute, ExpectedClusterV1::OwnedLoopback)
                .expect("execute argv")
                .execute
        );
    }

    #[test]
    fn argv_refuses_external_origins_duplicates_and_relative_outputs() {
        let mut external = argv("/private/tmp/aggregate-retirement");
        external[1] = "https://api.devnet.solana.com".into();
        assert!(parse_arguments_v1(external, ExpectedClusterV1::OwnedLoopback).is_err());
        let mut duplicate = argv("/private/tmp/aggregate-retirement");
        duplicate.extend(["--market".into(), Pubkey::new_unique().to_string()]);
        assert!(parse_arguments_v1(duplicate, ExpectedClusterV1::OwnedLoopback).is_err());
        let mut relative = argv("/private/tmp/aggregate-retirement");
        let index = relative
            .iter()
            .position(|value| value == "--campaign")
            .expect("campaign flag")
            + 1;
        relative[index] = "campaign.json".into();
        assert!(parse_arguments_v1(relative, ExpectedClusterV1::OwnedLoopback).is_err());
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

    #[test]
    fn generic_transport_refuses_direct_schema_or_untrusted_durable_projection() {
        let campaign = campaign_fixture_v1();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dclutch-generic-retirement-{}-{nonce}",
            std::process::id()
        ));
        let journals = root.join("journals");
        fs::create_dir(&root).expect("test root");
        fs::create_dir(&journals).expect("test journals");
        let campaign_path = root.join("campaign.json");
        let completion = root.join("completion.json");
        let payer_keypair = root.join("payer.json");
        create_json_v1(&campaign_path, &campaign, "test retirement campaign")
            .expect("durable campaign");
        let transport = AggregateRetirementTransportV1 {
            campaign_path: &campaign_path,
            journal_dir: &journals,
            completion: &completion,
            payer: key(80),
            payer_keypair: &payer_keypair,
            lookup_table: key(81),
            execute: false,
        };
        authenticate_aggregate_retirement_transport_durable_v1(
            &campaign,
            transport,
            &campaign.rpc_url,
        )
        .expect("authenticated generic transport");

        let mut direct_projection = serde_json::to_value(&campaign).expect("campaign value");
        direct_projection["schema"] = serde_json::Value::String(
            "dclutch-owned-loopback-terminal-sequence-completion-v1".into(),
        );
        fs::write(
            &campaign_path,
            serde_json::to_vec_pretty(&direct_projection).expect("direct projection bytes"),
        )
        .expect("write hostile direct projection");
        assert!(
            authenticate_aggregate_retirement_transport_durable_v1(
                &campaign,
                transport,
                &campaign.rpc_url,
            )
            .is_err()
        );

        direct_projection["schema"] = serde_json::Value::String(campaign.schema.clone());
        direct_projection["rpcUrl"] = serde_json::Value::String("http://127.0.0.1:9999/".into());
        fs::write(
            &campaign_path,
            serde_json::to_vec_pretty(&direct_projection).expect("untrusted projection bytes"),
        )
        .expect("write hostile untrusted projection");
        assert!(
            authenticate_aggregate_retirement_transport_durable_v1(
                &campaign,
                transport,
                &campaign.rpc_url,
            )
            .is_err()
        );
        fs::remove_file(&campaign_path).expect("remove test campaign");
        fs::remove_dir(&journals).expect("remove test journals");
        fs::remove_dir(&root).expect("remove test root");
    }
}
