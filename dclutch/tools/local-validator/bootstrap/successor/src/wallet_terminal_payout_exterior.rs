//! Durable cluster-authenticated execution of one wallet terminal payout.
//!
//! The public payout input remains owned by `wallet_terminal`; this module only
//! supplies the private-validator transaction exterior.  Preflight is
//! read-only and never opens a key.  Execute advances one canonical ALT or
//! payout action, persisting the exact packet and signature before transport.
//! `Dispatching` recovery polls first and may resend only the exact persisted
//! packet when its signature is absent; `Submitted` is permanently poll-only.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_operator::{
    ObservedAccount,
    wallet_terminal_payout_v3::{
        WalletTerminalPayoutExpectedPoststateV3, WalletTerminalPayoutPoststateV3,
        WalletTerminalPayoutReportV3, canonical_wallet_terminal_payout_lookup_addresses_v3,
        compile_wallet_terminal_payout_v0, project_wallet_terminal_payout_postcondition_v3,
        verify_wallet_terminal_payout_postcondition_v3,
    },
};
use dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    instruction::{create_lookup_table, extend_lookup_table, freeze_lookup_table},
    program as lookup_table_program,
    state::AddressLookupTable,
};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::{
    hash::Hash,
    message::{Message, VersionedMessage},
    signature::{Keypair, Signature, Signer as _},
    transaction::VersionedTransaction,
};

use crate::wallet_terminal::snapshot_from_rpc;
use crate::{
    Error, Result, campaign, chaos_fault,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    rpc::{Rpc, WritePolicyV1},
    wallet_terminal::{
        FinalizedSnapshotV1, LookupTableRequirementV1, PlanInputV1, SelectedInputV1, build_report,
    },
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-wallet-terminal-payout-v1";
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-wallet-terminal-payout-v1";
const JOURNAL_SCHEMA_V1: &str = "dclutch-local-private-validator-wallet-terminal-payout-journal-v1";
const JOURNAL_SCHEMA_DEVNET_V1: &str = "dclutch-devnet-wallet-terminal-payout-journal-v1";
const EVIDENCE_SCHEMA_V1: &str =
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1";
const EVIDENCE_SCHEMA_DEVNET_V1: &str = "dclutch-devnet-wallet-terminal-payout-evidence-v1";
const PREFLIGHT_SCHEMA_V1: &str =
    "dclutch-local-private-validator-wallet-terminal-payout-preflight-v1";
const PREFLIGHT_SCHEMA_DEVNET_V1: &str = "dclutch-devnet-wallet-terminal-payout-preflight-v1";

const fn journal_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => JOURNAL_SCHEMA_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => JOURNAL_SCHEMA_V1,
    }
}

const fn evidence_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => EVIDENCE_SCHEMA_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => EVIDENCE_SCHEMA_V1,
    }
}

const fn preflight_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => PREFLIGHT_SCHEMA_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => PREFLIGHT_SCHEMA_V1,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StageV1 {
    LookupCreate,
    LookupExtend,
    LookupFreeze,
    LookupActivation,
    Payout,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
    Dispatching,
    SignedNotSubmitted,
    Submitted,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryActionV1 {
    SignOnceAndPersistDispatching,
    PollThenResendIdentical,
    PollOnly,
    AuthenticateFinalized,
}

const fn recovery_action(phase: PhaseV1) -> RecoveryActionV1 {
    match phase {
        PhaseV1::Planned => RecoveryActionV1::SignOnceAndPersistDispatching,
        PhaseV1::Dispatching => RecoveryActionV1::PollThenResendIdentical,
        PhaseV1::SignedNotSubmitted | PhaseV1::Submitted => RecoveryActionV1::PollOnly,
        PhaseV1::Finalized => RecoveryActionV1::AuthenticateFinalized,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignaturePresenceV1 {
    Absent,
    Pending,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchRouteV1 {
    ResendIdentical,
    PollOnly,
    AuthenticateFinalized,
}

fn dispatch_route(phase: PhaseV1, presence: SignaturePresenceV1) -> Result<DispatchRouteV1> {
    match (phase, presence) {
        (PhaseV1::Dispatching, SignaturePresenceV1::Absent) => Ok(DispatchRouteV1::ResendIdentical),
        (
            PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted,
            SignaturePresenceV1::Pending,
        )
        | (PhaseV1::SignedNotSubmitted | PhaseV1::Submitted, SignaturePresenceV1::Absent) => {
            Ok(DispatchRouteV1::PollOnly)
        }
        (
            PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted,
            SignaturePresenceV1::Finalized,
        ) => Ok(DispatchRouteV1::AuthenticateFinalized),
        (PhaseV1::Planned | PhaseV1::Finalized, _) => Err(refusal(
            "wallet payout dispatch routing requires one ambiguous signed phase",
        )),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedAccountV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedAccountV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JournalV1 {
    schema: String,
    input_sha256: String,
    payout_intent_sha256: String,
    fee_payer: String,
    owner: String,
    stage: StageV1,
    action_index: u16,
    phase: PhaseV1,
    observation_slot: u64,
    lookup_creation_slot: u64,
    lookup_table: String,
    lookup_addresses: Vec<String>,
    lookup_addresses_sha256: String,
    payout_instruction_base64: String,
    payout_instruction_sha256: String,
    custody_request_base64: Option<String>,
    custody_request_sha256: Option<String>,
    message_base64: Option<String>,
    message_sha256: Option<String>,
    last_valid_block_height: Option<u64>,
    exact_fee_lamports: Option<u64>,
    expected_wire_bytes: Option<usize>,
    signed_packet_base64: Option<String>,
    expected_signature: Option<String>,
    expected_return_data_producer: Option<String>,
    expected_return_data_base64: Option<String>,
    expected_poststates: Vec<ExpectedAccountV1>,
    finalized_slot: Option<u64>,
    transaction_sha256: Option<String>,
    fee_lamports: Option<u64>,
    compute_units_consumed: Option<u64>,
    return_data_producer: Option<String>,
    return_data_base64: Option<String>,
    finalized_poststates: Vec<ObservedAccountV1>,
    state_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceV1 {
    schema: String,
    cluster: String,
    input_sha256: String,
    payout_intent_sha256: String,
    journal_state_sha256: String,
    signature: String,
    finalized_slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
    fee_payer: String,
    owner: String,
    market: String,
    recipient: String,
    payout: String,
    lookup_table: String,
    lookup_addresses_sha256: String,
    payout_instruction_sha256: String,
    custody_request_sha256: Option<String>,
    return_data_producer: String,
    return_data_base64: String,
    poststates: Vec<ObservedAccountV1>,
    evidence_sha256: String,
}

#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    input: PathBuf,
    fee_payer: Pubkey,
    fee_payer_keypair: PathBuf,
    owner_keypair: PathBuf,
    journal_dir: PathBuf,
    evidence: PathBuf,
    execute: bool,
}

struct PlanningV1 {
    input_sha256: String,
    selected: SelectedInputV1,
    report: WalletTerminalPayoutReportV3,
    payout_intent_sha256: String,
    payout_instruction_base64: String,
    payout_instruction_sha256: String,
    custody_request_base64: Option<String>,
    custody_request_sha256: Option<String>,
    lookup_creation_slot: u64,
    lookup_table: Pubkey,
    lookup_addresses: Vec<Pubkey>,
    lookup_addresses_sha256: String,
    next_stage: StageV1,
    action_index: usize,
    snapshot: FinalizedSnapshotV1,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

fn run_for_cluster_v1(arguments: Vec<String>, expected_cluster: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments_for_cluster_v1(arguments, expected_cluster)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    if arguments.evidence.exists() {
        return stdout_json(&authenticate_evidence(&mut rpc, &arguments)?);
    }
    let journals = load_journals(&arguments)?;
    let planning = plan(&mut rpc, &arguments, &journals)?;
    if let Some(mut journal) = journals
        .last()
        .cloned()
        .filter(|journal| journal.phase != PhaseV1::Finalized)
    {
        if !arguments.execute {
            return stdout_json(&preflight(&arguments, &planning, Some(&journal)));
        }
        let path = journal_path(&arguments.journal_dir, journal.action_index, journal.stage);
        resume_transaction(&mut rpc, &arguments, &planning, &path, &mut journal)?;
        return finish_or_output(&mut rpc, &arguments, &planning, journal);
    }
    if !arguments.execute {
        return stdout_json(&preflight(&arguments, &planning, None));
    }
    if planning.next_stage == StageV1::LookupActivation {
        let journal = finalize_activation(&arguments, &planning)?;
        return stdout_json(&journal);
    }
    let mut journal = planned_journal(&mut rpc, &arguments, &planning)?;
    let path = journal_path(&arguments.journal_dir, journal.action_index, journal.stage);
    write_journal(&path, &journal, true, None)?;
    resume_transaction(&mut rpc, &arguments, &planning, &path, &mut journal)?;
    finish_or_output(&mut rpc, &arguments, &planning, journal)
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap local-private-validator-wallet-terminal-payout-v1 \\\n     --rpc-url http://127.0.0.1:PORT --input ABSOLUTE_JSON \\\n     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_JSON \\\n     --owner-keypair ABSOLUTE_JSON --journal-dir ABSOLUTE_DIRECTORY \\\n     --evidence ABSOLUTE_JSON [--execute]\n\nThis command only admits an owned loopback validator. Preflight performs finalized reads and opens no key. Execute advances one canonical ALT or payout action; every signed packet and expected signature are fsynced before its sole send, and ambiguous recovery only polls that signature."
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    parse_arguments_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn devnet_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-wallet-terminal-payout-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --input ABSOLUTE_JSON --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_JSON --owner-keypair ABSOLUTE_JSON --journal-dir ABSOLUTE_DIRECTORY --evidence ABSOLUTE_JSON [--execute]\n\nThe public arm has distinct devnet journal, evidence, and preflight domains. Preflight opens no key. Execute advances exactly one durable ALT or payout action and recovery never re-signs."
}

fn parse_arguments_for_cluster_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut input = None;
    let mut fee_payer = None;
    let mut fee_payer_keypair = None;
    let mut owner_keypair = None;
    let mut journal_dir = None;
    let mut evidence = None;
    let mut execute = false;
    let mut seen = BTreeSet::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if !seen.insert(argument.clone()) {
            return Err(refusal(format!("{argument} may be supplied only once")));
        }
        if argument == "--execute" {
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        match argument.as_str() {
            "--rpc-url" => rpc_url = Some(value),
            "--i-mean-devnet" => acknowledgment = Some(value),
            "--input" => input = Some(value),
            "--fee-payer" => fee_payer = Some(value),
            "--fee-payer-keypair" => fee_payer_keypair = Some(value),
            "--owner-keypair" => owner_keypair = Some(value),
            "--journal-dir" => journal_dir = Some(value),
            "--evidence" => evidence = Some(value),
            _ => {
                return Err(refusal(format!(
                    "unknown {} argument: {argument}",
                    match expected_cluster {
                        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
                        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
                    }
                )));
            }
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    expected_cluster.authenticate(&origin)?;
    let fee_payer = required(fee_payer, "--fee-payer")?
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("--fee-payer: {error}")))?;
    if fee_payer == Pubkey::default() {
        return Err(refusal("--fee-payer must be nonzero"));
    }
    let input = absolute_existing(required(input, "--input")?, "--input", false)?;
    let fee_payer_keypair = absolute_existing(
        required(fee_payer_keypair, "--fee-payer-keypair")?,
        "--fee-payer-keypair",
        false,
    )?;
    let owner_keypair = absolute_existing(
        required(owner_keypair, "--owner-keypair")?,
        "--owner-keypair",
        false,
    )?;
    let journal_dir = absolute_existing(
        required(journal_dir, "--journal-dir")?,
        "--journal-dir",
        true,
    )?;
    let evidence = absolute_output(required(evidence, "--evidence")?, "--evidence")?;
    Ok(ArgumentsV1 {
        origin,
        expected_cluster,
        input,
        fee_payer,
        fee_payer_keypair,
        owner_keypair,
        journal_dir,
        evidence,
        execute,
    })
}

fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1, journals: &[JournalV1]) -> Result<PlanningV1> {
    let input_bytes = fs::read(&arguments.input)?;
    let input_sha256 = sha256_hex(&input_bytes);
    let mut input: PlanInputV1 = serde_json::from_slice(&input_bytes)
        .map_err(|error| Error::new(format!("wallet payout input: {error}")))?;
    if input.lookup_table.is_some() {
        return Err(refusal(
            "wallet payout input must omit lookupTable; this exterior derives and freezes its sole table",
        ));
    }
    let selected_without_table = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    let initial_snapshot = snapshot(rpc, &selected_without_table)?;
    let initial_report = build_report(&selected_without_table, &initial_snapshot)?;
    if initial_report.owner != selected_without_table.owner {
        return Err(refusal("wallet payout report changed its owner"));
    }
    let (first_create, first_table) = create_lookup_table(
        arguments.fee_payer,
        arguments.fee_payer,
        initial_report.observation.slot,
    );
    let _ = first_create;
    let (lookup_creation_slot, lookup_table) = journals
        .first()
        .map(|journal| -> Result<(u64, Pubkey)> {
            Ok((
                journal.lookup_creation_slot,
                parse_pubkey(&journal.lookup_table, "journal lookup table")?,
            ))
        })
        .transpose()?
        .unwrap_or((initial_report.observation.slot, first_table));
    input.lookup_table = Some(lookup_table.to_string());
    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Present)?;
    let snapshot = snapshot(rpc, &selected)?;
    let report = build_report(&selected, &snapshot)?;
    let instruction_bytes = bincode::serialize(&report.instruction)
        .map_err(|error| Error::new(format!("wallet payout instruction: {error}")))?;
    let payout_instruction_base64 = BASE64.encode(&instruction_bytes);
    let payout_instruction_sha256 = sha256_hex(&instruction_bytes);
    let custody_request_bytes = report
        .custody_request
        .map(|request| {
            request
                .to_bytes()
                .map(|bytes| bytes.to_vec())
                .map_err(|error| Error::new(format!("wallet payout Custody request: {error:?}")))
        })
        .transpose()?;
    let custody_request_base64 = custody_request_bytes
        .as_ref()
        .map(|bytes| BASE64.encode(bytes));
    let custody_request_sha256 = custody_request_bytes
        .as_ref()
        .map(|bytes| sha256_hex(bytes));
    let lookup_addresses =
        canonical_wallet_terminal_payout_lookup_addresses_v3(&report, arguments.fee_payer)
            .map_err(|error| Error::new(format!("wallet payout lookup addresses: {error:?}")))?;
    let lookup_addresses_sha256 = pubkey_list_sha256(&lookup_addresses);
    let extension_count = lookup_addresses
        .len()
        .div_ceil(EXTEND_ADDRESSES_PER_TRANSACTION_V1);
    let payout_intent_sha256 = payout_intent_sha256(
        arguments.expected_cluster,
        &input_sha256,
        arguments.fee_payer,
        report.owner,
        &instruction_bytes,
        custody_request_bytes.as_deref(),
        lookup_creation_slot,
        lookup_table,
        &lookup_addresses,
    );
    for (index, journal) in journals.iter().enumerate() {
        if journal.stage != stage_for_action(index, extension_count)?
            || usize::from(journal.action_index) != index
        {
            return Err(refusal(
                "wallet payout journal stage was outside its canonical action sequence",
            ));
        }
        authenticate_identity(
            journal,
            arguments.expected_cluster,
            &input_sha256,
            &payout_intent_sha256,
            arguments.fee_payer,
            report.owner,
            lookup_creation_slot,
            lookup_table,
            &lookup_addresses,
            &payout_instruction_base64,
            &payout_instruction_sha256,
            custody_request_base64.as_deref(),
            custody_request_sha256.as_deref(),
        )?;
    }
    let completed = journals
        .iter()
        .filter(|journal| journal.phase == PhaseV1::Finalized)
        .count();
    if completed != journals.len() {
        let journal = journals
            .last()
            .ok_or_else(|| refusal("journal sequence vanished"))?;
        return Ok(PlanningV1 {
            input_sha256,
            selected,
            report,
            payout_intent_sha256,
            payout_instruction_base64,
            payout_instruction_sha256,
            custody_request_base64,
            custody_request_sha256,
            lookup_creation_slot,
            lookup_table,
            lookup_addresses,
            lookup_addresses_sha256,
            next_stage: journal.stage,
            action_index: usize::from(journal.action_index),
            snapshot,
        });
    }
    let (next_stage, action_index) = if completed == 0 {
        (StageV1::LookupCreate, 0)
    } else if completed <= extension_count {
        (StageV1::LookupExtend, completed)
    } else if completed == extension_count + 1 {
        (StageV1::LookupFreeze, completed)
    } else if completed == extension_count + 2 {
        (StageV1::LookupActivation, completed)
    } else if completed == extension_count + 3 {
        (StageV1::Payout, completed)
    } else {
        return Err(refusal(
            "wallet payout journal sequence continued after payout",
        ));
    };
    Ok(PlanningV1 {
        input_sha256,
        selected,
        report,
        payout_intent_sha256,
        payout_instruction_base64,
        payout_instruction_sha256,
        custody_request_base64,
        custody_request_sha256,
        lookup_creation_slot,
        lookup_table,
        lookup_addresses,
        lookup_addresses_sha256,
        next_stage,
        action_index,
        snapshot,
    })
}

fn stage_for_action(index: usize, extension_count: usize) -> Result<StageV1> {
    if index == 0 {
        Ok(StageV1::LookupCreate)
    } else if index <= extension_count {
        Ok(StageV1::LookupExtend)
    } else if index == extension_count + 1 {
        Ok(StageV1::LookupFreeze)
    } else if index == extension_count + 2 {
        Ok(StageV1::LookupActivation)
    } else if index == extension_count + 3 {
        Ok(StageV1::Payout)
    } else {
        Err(refusal(
            "wallet payout action index continued past canonical payout",
        ))
    }
}

fn planned_journal(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    planning: &PlanningV1,
) -> Result<JournalV1> {
    let (blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    let (message, expected_return_data, expected_poststates) = match planning.next_stage {
        StageV1::LookupCreate | StageV1::LookupExtend | StageV1::LookupFreeze => {
            let instruction = infrastructure_instruction(planning, arguments.fee_payer)?;
            (
                VersionedMessage::Legacy(Message::new_with_blockhash(
                    core::slice::from_ref(&instruction),
                    Some(&arguments.fee_payer),
                    &blockhash,
                )),
                None,
                Vec::new(),
            )
        }
        StageV1::Payout => {
            let table = planning
                .snapshot
                .required(planning.lookup_table, "wallet payout lookup table")?;
            authenticate_lookup_table(planning, table, None, true)?;
            let transaction = compile_wallet_terminal_payout_v0(
                planning.report.clone(),
                arguments.fee_payer,
                blockhash,
                table,
            )
            .map_err(|error| Error::new(format!("wallet payout v0 compilation: {error:?}")))?;
            let expected = project_wallet_terminal_payout_postcondition_v3(&transaction.payout)
                .map_err(|error| Error::new(format!("wallet payout postcondition: {error:?}")))?;
            let accounts = expected_payout_accounts(planning, &expected)?;
            (
                transaction.message.message,
                Some((
                    planning.report.instruction.program_id.to_string(),
                    BASE64.encode(&expected.receipt_bytes),
                )),
                accounts,
            )
        }
        StageV1::LookupActivation => {
            return Err(refusal("lookup activation has no transaction journal"));
        }
    };
    let message_bytes = bincode::serialize(&message)
        .map_err(|error| Error::new(format!("wallet payout message: {error}")))?;
    let message_base64 = BASE64.encode(&message_bytes);
    let exact_fee_lamports = rpc
        .call(
            "getFeeForMessage",
            &json!([message_base64, {"commitment":"finalized"}]),
        )?
        .get("value")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("wallet payout getFeeForMessage omitted exact fee"))?;
    let signature_count = message.header().num_required_signatures;
    let expected_wire_bytes = bincode::serialize(&VersionedTransaction {
        signatures: vec![Signature::default(); usize::from(signature_count)],
        message: message.clone(),
    })
    .map_err(|error| Error::new(format!("wallet payout geometry: {error}")))?
    .len();
    if expected_wire_bytes > 1_232 {
        return Err(refusal(
            "wallet payout packet exceeds Solana's 1,232-byte limit",
        ));
    }
    let mut journal = JournalV1 {
        schema: journal_schema_v1(arguments.expected_cluster).into(),
        input_sha256: planning.input_sha256.clone(),
        payout_intent_sha256: planning.payout_intent_sha256.clone(),
        fee_payer: arguments.fee_payer.to_string(),
        owner: planning.report.owner.to_string(),
        stage: planning.next_stage,
        action_index: u16::try_from(planning.action_index)
            .map_err(|_| refusal("wallet payout action index overflow"))?,
        phase: PhaseV1::Planned,
        observation_slot: planning.report.observation.slot,
        lookup_creation_slot: planning.lookup_creation_slot,
        lookup_table: planning.lookup_table.to_string(),
        lookup_addresses: planning
            .lookup_addresses
            .iter()
            .map(ToString::to_string)
            .collect(),
        lookup_addresses_sha256: planning.lookup_addresses_sha256.clone(),
        payout_instruction_base64: planning.payout_instruction_base64.clone(),
        payout_instruction_sha256: planning.payout_instruction_sha256.clone(),
        custody_request_base64: planning.custody_request_base64.clone(),
        custody_request_sha256: planning.custody_request_sha256.clone(),
        message_base64: Some(message_base64),
        message_sha256: Some(sha256_hex(&message_bytes)),
        last_valid_block_height: Some(last_valid_block_height),
        exact_fee_lamports: Some(exact_fee_lamports),
        expected_wire_bytes: Some(expected_wire_bytes),
        signed_packet_base64: None,
        expected_signature: None,
        expected_return_data_producer: expected_return_data.as_ref().map(|value| value.0.clone()),
        expected_return_data_base64: expected_return_data.map(|value| value.1),
        expected_poststates,
        finalized_slot: None,
        transaction_sha256: None,
        fee_lamports: None,
        compute_units_consumed: None,
        return_data_producer: None,
        return_data_base64: None,
        finalized_poststates: Vec::new(),
        state_sha256: String::new(),
    };
    refresh_state_sha256(&mut journal)?;
    Ok(journal)
}

fn resume_transaction(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    planning: &PlanningV1,
    path: &Path,
    journal: &mut JournalV1,
) -> Result<()> {
    authenticate_journal(planning, arguments, journal)?;
    match recovery_action(journal.phase) {
        RecoveryActionV1::AuthenticateFinalized => authenticate_finalized_history(rpc, journal),
        RecoveryActionV1::PollOnly => {
            authenticate_signed_packet_envelope(journal)?;
            let signature = journal
                .expected_signature
                .as_deref()
                .ok_or_else(|| refusal("ambiguous payout journal omitted signature"))?;
            let observation = observe_signature(rpc, signature)?;
            let route = dispatch_route(journal.phase, observation.presence())?;
            let SignatureObservationV1::Finalized(history) = observation else {
                debug_assert_eq!(route, DispatchRouteV1::PollOnly);
                return Err(refusal(format!(
                    "wallet payout transaction {signature} is not finalized; {:?} recovery is exact-signature poll-only and never re-signs or resubmits",
                    journal.phase
                )));
            };
            debug_assert_eq!(route, DispatchRouteV1::AuthenticateFinalized);
            finalize_transaction(rpc, planning, journal, &history)?;
            persist_update(path, journal)
        }
        RecoveryActionV1::SignOnceAndPersistDispatching => {
            authenticate_planned_message(planning, arguments, journal)?;
            let current_height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| refusal("wallet payout getBlockHeight was not u64"))?;
            if current_height
                > journal
                    .last_valid_block_height
                    .ok_or_else(|| refusal("planned payout omitted block-height validity"))?
            {
                return Err(refusal(
                    "planned payout blockhash expired before key access",
                ));
            }
            let payer = read_keypair(
                &arguments.fee_payer_keypair,
                arguments.fee_payer,
                "wallet-terminal-fee-payer",
            )?;
            let owner = if journal.stage != StageV1::Payout
                || planning.report.owner == arguments.fee_payer
            {
                None
            } else {
                Some(read_keypair(
                    &arguments.owner_keypair,
                    planning.report.owner,
                    "wallet-terminal-owner",
                )?)
            };
            let message_bytes = BASE64
                .decode(
                    journal
                        .message_base64
                        .as_deref()
                        .ok_or_else(|| refusal("planned payout omitted message"))?,
                )
                .map_err(|error| Error::new(format!("wallet payout message base64: {error}")))?;
            let message: VersionedMessage = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("wallet payout durable message: {error}")))?;
            let signers: Vec<&dyn solana_sdk::signer::Signer> = if let Some(owner) = owner.as_ref()
            {
                vec![&payer, owner]
            } else {
                vec![&payer]
            };
            let transaction = VersionedTransaction::try_new(message, &signers)
                .map_err(|error| Error::new(format!("sign wallet payout transaction: {error}")))?;
            let packet = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize wallet payout packet: {error}")))?;
            if Some(packet.len()) != journal.expected_wire_bytes || packet.len() > 1_232 {
                return Err(refusal("signed payout packet changed its durable geometry"));
            }
            let signature = transaction
                .signatures
                .first()
                .copied()
                .ok_or_else(|| refusal("signed payout packet omitted payer signature"))?;
            journal.signed_packet_base64 = Some(BASE64.encode(&packet));
            journal.expected_signature = Some(signature.to_string());
            journal.phase = PhaseV1::Dispatching;
            persist_update(path, journal)?;
            resume_transaction(rpc, arguments, planning, path, journal)
        }
        RecoveryActionV1::PollThenResendIdentical => {
            authenticate_planned_message(planning, arguments, journal)?;
            let (packet, signature) = authenticate_signed_packet_envelope(journal)?;
            let observation = observe_signature(rpc, &signature.to_string())?;
            match dispatch_route(journal.phase, observation.presence())? {
                DispatchRouteV1::AuthenticateFinalized => {
                    let SignatureObservationV1::Finalized(history) = observation else {
                        unreachable!("finalized route requires finalized history")
                    };
                    park_payout_chaos_boundary(
                        arguments.expected_cluster,
                        path,
                        journal,
                        &packet,
                        chaos_fault::BoundaryV1::LandedBeforeFinalizationFsync,
                    )?;
                    finalize_transaction(rpc, planning, journal, &history)?;
                    persist_update(path, journal)
                }
                DispatchRouteV1::PollOnly => Err(refusal(format!(
                    "wallet payout transaction {signature} is present but not finalized; Dispatching recovery is now poll-only and cannot resend or re-sign"
                ))),
                DispatchRouteV1::ResendIdentical => {
                    let current_height = rpc
                        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                        .as_u64()
                        .ok_or_else(|| refusal("wallet payout getBlockHeight was not u64"))?;
                    if current_height
                        > journal.last_valid_block_height.ok_or_else(|| {
                            refusal("Dispatching payout omitted block-height validity")
                        })?
                    {
                        return Err(refusal(
                            "Dispatching payout packet expired while absent; preserve the journal rather than re-signing",
                        ));
                    }
                    park_payout_chaos_boundary(
                        arguments.expected_cluster,
                        path,
                        journal,
                        &packet,
                        chaos_fault::BoundaryV1::DispatchingBeforeSend,
                    )?;
                    let returned = rpc
                        .call_once(
                            "sendTransaction",
                            &json!([BASE64.encode(&packet), {
                                "encoding":"base64",
                                "skipPreflight":false,
                                "preflightCommitment":"finalized",
                                "maxRetries":0
                            }]),
                        )?
                        .as_str()
                        .ok_or_else(|| refusal("wallet payout sendTransaction omitted signature"))?
                        .parse::<Signature>()
                        .map_err(|error| {
                            Error::new(format!("wallet payout returned signature: {error}"))
                        })?;
                    if returned != signature {
                        return Err(refusal(
                            "wallet payout RPC substituted its returned signature",
                        ));
                    }
                    let landed_armed = arguments.expected_cluster
                        == ExpectedClusterV1::OwnedLoopback
                        && journal.stage == StageV1::Payout
                        && chaos_fault::is_armed_for_v1(
                            "wallet-terminal-payout",
                            chaos_fault::BoundaryV1::LandedBeforeFinalizationFsync,
                        )?;
                    let history = if landed_armed {
                        let history = wait_finalized(
                            rpc,
                            &signature.to_string(),
                            arguments.origin.pacing().confirm_timeout,
                        )?;
                        park_payout_chaos_boundary(
                            arguments.expected_cluster,
                            path,
                            journal,
                            &packet,
                            chaos_fault::BoundaryV1::LandedBeforeFinalizationFsync,
                        )?;
                        history
                    } else {
                        journal.phase = PhaseV1::Submitted;
                        persist_update(path, journal)?;
                        wait_finalized(
                            rpc,
                            &signature.to_string(),
                            arguments.origin.pacing().confirm_timeout,
                        )?
                    };
                    finalize_transaction(rpc, planning, journal, &history)?;
                    persist_update(path, journal)
                }
            }
        }
    }
}

fn finish_or_output(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    planning: &PlanningV1,
    journal: JournalV1,
) -> Result<()> {
    if journal.stage == StageV1::Payout && journal.phase == PhaseV1::Finalized {
        let evidence = publish_evidence(arguments, planning, &journal)?;
        authenticate_evidence_value(rpc, arguments, &evidence)?;
        return stdout_json(&evidence);
    }
    stdout_json(&journal)
}

fn finalize_activation(arguments: &ArgumentsV1, planning: &PlanningV1) -> Result<JournalV1> {
    let table = planning
        .snapshot
        .required(planning.lookup_table, "wallet payout lookup table")?;
    authenticate_lookup_table(planning, table, None, true)?;
    let mut journal = identity_journal(arguments, planning, StageV1::LookupActivation)?;
    journal.phase = PhaseV1::Finalized;
    journal.finalized_slot = Some(planning.snapshot.observation.slot);
    refresh_state_sha256(&mut journal)?;
    let path = journal_path(&arguments.journal_dir, journal.action_index, journal.stage);
    write_journal(&path, &journal, true, None)?;
    Ok(journal)
}

fn identity_journal(
    arguments: &ArgumentsV1,
    planning: &PlanningV1,
    stage: StageV1,
) -> Result<JournalV1> {
    let mut journal = JournalV1 {
        schema: journal_schema_v1(arguments.expected_cluster).into(),
        input_sha256: planning.input_sha256.clone(),
        payout_intent_sha256: planning.payout_intent_sha256.clone(),
        fee_payer: arguments.fee_payer.to_string(),
        owner: planning.report.owner.to_string(),
        stage,
        action_index: u16::try_from(planning.action_index)
            .map_err(|_| refusal("action index overflow"))?,
        phase: PhaseV1::Planned,
        observation_slot: planning.report.observation.slot,
        lookup_creation_slot: planning.lookup_creation_slot,
        lookup_table: planning.lookup_table.to_string(),
        lookup_addresses: planning
            .lookup_addresses
            .iter()
            .map(ToString::to_string)
            .collect(),
        lookup_addresses_sha256: planning.lookup_addresses_sha256.clone(),
        payout_instruction_base64: planning.payout_instruction_base64.clone(),
        payout_instruction_sha256: planning.payout_instruction_sha256.clone(),
        custody_request_base64: planning.custody_request_base64.clone(),
        custody_request_sha256: planning.custody_request_sha256.clone(),
        message_base64: None,
        message_sha256: None,
        last_valid_block_height: None,
        exact_fee_lamports: None,
        expected_wire_bytes: None,
        signed_packet_base64: None,
        expected_signature: None,
        expected_return_data_producer: None,
        expected_return_data_base64: None,
        expected_poststates: Vec::new(),
        finalized_slot: None,
        transaction_sha256: None,
        fee_lamports: None,
        compute_units_consumed: None,
        return_data_producer: None,
        return_data_base64: None,
        finalized_poststates: Vec::new(),
        state_sha256: String::new(),
    };
    refresh_state_sha256(&mut journal)?;
    Ok(journal)
}

fn infrastructure_instruction(planning: &PlanningV1, payer: Pubkey) -> Result<Instruction> {
    match planning.next_stage {
        StageV1::LookupCreate => {
            let (instruction, table) =
                create_lookup_table(payer, payer, planning.lookup_creation_slot);
            if table != planning.lookup_table {
                return Err(refusal("wallet payout lookup create identity changed"));
            }
            Ok(instruction)
        }
        StageV1::LookupExtend => {
            let page = planning
                .lookup_addresses
                .chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .nth(planning.action_index.saturating_sub(1))
                .ok_or_else(|| refusal("wallet payout extension page changed"))?;
            Ok(extend_lookup_table(
                planning.lookup_table,
                payer,
                Some(payer),
                page.to_vec(),
            ))
        }
        StageV1::LookupFreeze => Ok(freeze_lookup_table(planning.lookup_table, payer)),
        StageV1::LookupActivation | StageV1::Payout => Err(refusal(
            "non-infrastructure stage has no infrastructure instruction",
        )),
    }
}

fn expected_payout_accounts(
    planning: &PlanningV1,
    expected: &WalletTerminalPayoutExpectedPoststateV3,
) -> Result<Vec<ExpectedAccountV1>> {
    let route = planning.report.route;
    let coordinates = [
        (route.aggregate, expected.aggregate_bytes.as_slice()),
        (route.position, expected.position_bytes.as_slice()),
        (
            route.custody_replay,
            expected.custody_replay_bytes.as_slice(),
        ),
        (route.hoard, expected.hoard_token_bytes.as_slice()),
        (route.recipient, expected.recipient_token_bytes.as_slice()),
    ];
    coordinates
        .into_iter()
        .map(|(address, data)| {
            let before = planning
                .snapshot
                .required(address, "wallet payout poststate coordinate")?;
            Ok(ExpectedAccountV1 {
                address: address.to_string(),
                owner: before.owner.to_string(),
                lamports: before.lamports,
                executable: before.executable,
                data_base64: BASE64.encode(data),
                data_sha256: sha256_hex(data),
            })
        })
        .collect()
}

fn authenticate_planned_message(
    planning: &PlanningV1,
    arguments: &ArgumentsV1,
    journal: &JournalV1,
) -> Result<()> {
    authenticate_journal(planning, arguments, journal)?;
    let bytes = BASE64
        .decode(
            journal
                .message_base64
                .as_deref()
                .ok_or_else(|| refusal("journal omitted message"))?,
        )
        .map_err(|error| Error::new(format!("wallet payout message base64: {error}")))?;
    let message: VersionedMessage = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("wallet payout message: {error}")))?;
    let blockhash = match &message {
        VersionedMessage::Legacy(message) => message.recent_blockhash,
        VersionedMessage::V0(message) => message.recent_blockhash,
    };
    let expected = match journal.stage {
        StageV1::LookupCreate => {
            let (instruction, table) = create_lookup_table(
                arguments.fee_payer,
                arguments.fee_payer,
                planning.lookup_creation_slot,
            );
            if table != planning.lookup_table {
                return Err(refusal("wallet payout lookup identity changed"));
            }
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[instruction],
                Some(&arguments.fee_payer),
                &blockhash,
            ))
        }
        StageV1::LookupExtend => {
            let page = planning
                .lookup_addresses
                .chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .nth(usize::from(journal.action_index).saturating_sub(1))
                .ok_or_else(|| refusal("wallet payout extension page changed"))?;
            let instruction = extend_lookup_table(
                planning.lookup_table,
                arguments.fee_payer,
                Some(arguments.fee_payer),
                page.to_vec(),
            );
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[instruction],
                Some(&arguments.fee_payer),
                &blockhash,
            ))
        }
        StageV1::LookupFreeze => {
            let instruction = freeze_lookup_table(planning.lookup_table, arguments.fee_payer);
            VersionedMessage::Legacy(Message::new_with_blockhash(
                &[instruction],
                Some(&arguments.fee_payer),
                &blockhash,
            ))
        }
        StageV1::Payout => {
            let table = planning
                .snapshot
                .required(planning.lookup_table, "wallet payout lookup table")?;
            authenticate_lookup_table(planning, table, None, true)?;
            compile_wallet_terminal_payout_v0(
                planning.report.clone(),
                arguments.fee_payer,
                blockhash,
                table,
            )
            .map_err(|error| Error::new(format!("wallet payout message rederive: {error:?}")))?
            .message
            .message
        }
        StageV1::LookupActivation => return Err(refusal("activation journal has no message")),
    };
    if message != expected || journal.message_sha256.as_deref() != Some(sha256_hex(&bytes).as_str())
    {
        return Err(refusal(
            "durable wallet payout message differed from its semantic owner",
        ));
    }
    Ok(())
}

fn finalize_transaction(
    rpc: &mut Rpc,
    planning: &PlanningV1,
    journal: &mut JournalV1,
    history: &Value,
) -> Result<()> {
    let authenticated = authenticate_history(journal, history)?;
    journal.finalized_slot = Some(authenticated.slot);
    journal.transaction_sha256 = Some(sha256_hex(&authenticated.packet));
    journal.fee_lamports = Some(authenticated.fee);
    journal.compute_units_consumed = Some(authenticated.compute);
    journal.return_data_producer = authenticated
        .return_data
        .as_ref()
        .map(|value| value.0.clone());
    journal.return_data_base64 = authenticated
        .return_data
        .as_ref()
        .map(|value| value.1.clone());
    match journal.stage {
        StageV1::LookupCreate => {
            let account = finalized_account(rpc, planning.lookup_table, authenticated.slot)?;
            authenticate_lookup_table(
                planning,
                &account,
                Some(parse_pubkey(&journal.fee_payer, "lookup authority")?),
                false,
            )?;
            journal.finalized_poststates = vec![observed_account(&account)];
        }
        StageV1::LookupExtend => {
            let account = finalized_account(rpc, planning.lookup_table, authenticated.slot)?;
            authenticate_lookup_prefix(planning, journal, &account)?;
            journal.finalized_poststates = vec![observed_account(&account)];
        }
        StageV1::LookupFreeze => {
            let account = finalized_account(rpc, planning.lookup_table, authenticated.slot)?;
            authenticate_lookup_table(planning, &account, None, false)?;
            journal.finalized_poststates = vec![observed_account(&account)];
        }
        StageV1::Payout => {
            let accounts = finalized_accounts(
                rpc,
                &[
                    planning.report.route.aggregate,
                    planning.report.route.position,
                    planning.report.route.custody_replay,
                    planning.report.route.hoard,
                    planning.report.route.recipient,
                ],
                authenticated.slot,
            )?;
            verify_payout_poststates(planning, journal, &accounts)?;
            journal.finalized_poststates = accounts.iter().map(observed_account).collect();
        }
        StageV1::LookupActivation => return Err(refusal("activation is not a transaction")),
    }
    journal.phase = PhaseV1::Finalized;
    Ok(())
}

struct AuthenticatedHistoryV1 {
    slot: u64,
    packet: Vec<u8>,
    fee: u64,
    compute: u64,
    return_data: Option<(String, String)>,
}

fn authenticate_history(journal: &JournalV1, history: &Value) -> Result<AuthenticatedHistoryV1> {
    let meta = history
        .get("meta")
        .ok_or_else(|| refusal("finalized payout history omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(refusal(format!(
            "wallet payout transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let tuple = history
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("finalized payout history omitted base64 transaction tuple"))?;
    if tuple.len() != 2 || tuple.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(refusal(
            "wallet payout finalized history was not exact base64",
        ));
    }
    let packet = BASE64
        .decode(
            tuple[0]
                .as_str()
                .ok_or_else(|| refusal("wallet payout history packet was not text"))?,
        )
        .map_err(|error| Error::new(format!("wallet payout history base64: {error}")))?;
    let expected_packet = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("wallet payout journal omitted signed packet"))?,
        )
        .map_err(|error| Error::new(format!("wallet payout journal packet base64: {error}")))?;
    if packet != expected_packet {
        return Err(refusal(
            "finalized history substituted another wallet payout packet",
        ));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("wallet payout history transaction: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("wallet payout history omitted signature"))?;
    if journal.expected_signature.as_deref() != Some(signature.to_string().as_str()) {
        return Err(refusal("wallet payout history signature changed"));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("wallet payout history omitted fee"))?;
    if Some(fee) != journal.exact_fee_lamports {
        return Err(refusal(
            "wallet payout finalized fee differed from getFeeForMessage",
        ));
    }
    let compute = meta
        .get("computeUnitsConsumed")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("wallet payout history omitted computeUnitsConsumed"))?;
    let return_data = parse_return_data(meta.get("returnData"))?;
    if return_data.as_ref().map(|value| value.0.as_str())
        != journal.expected_return_data_producer.as_deref()
        || return_data.as_ref().map(|value| value.1.as_str())
            != journal.expected_return_data_base64.as_deref()
    {
        return Err(refusal("wallet payout finalized return data changed"));
    }
    Ok(AuthenticatedHistoryV1 {
        slot: history
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| refusal("wallet payout history omitted slot"))?,
        packet,
        fee,
        compute,
        return_data,
    })
}

fn parse_return_data(value: Option<&Value>) -> Result<Option<(String, String)>> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let producer = value
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("returnData omitted programId"))?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("returnData omitted tuple"))?;
    if data.len() != 2 || data.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(refusal("returnData was not canonical base64"));
    }
    Ok(Some((
        producer.into(),
        data[0]
            .as_str()
            .ok_or_else(|| refusal("returnData bytes were not text"))?
            .into(),
    )))
}

fn authenticate_finalized_history(rpc: &mut Rpc, journal: &JournalV1) -> Result<()> {
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("finalized payout journal omitted signature"))?;
    let history = finalized_transaction(rpc, signature)?
        .ok_or_else(|| refusal("persisted finalized payout disappeared from history"))?;
    let authenticated = authenticate_history(journal, &history)?;
    if Some(authenticated.slot) != journal.finalized_slot
        || Some(authenticated.fee) != journal.fee_lamports
        || Some(authenticated.compute) != journal.compute_units_consumed
        || Some(sha256_hex(&authenticated.packet)) != journal.transaction_sha256
    {
        return Err(refusal("persisted finalized payout history facts changed"));
    }
    Ok(())
}

enum SignatureObservationV1 {
    Absent,
    Pending,
    Finalized(Value),
}

impl SignatureObservationV1 {
    const fn presence(&self) -> SignaturePresenceV1 {
        match self {
            Self::Absent => SignaturePresenceV1::Absent,
            Self::Pending => SignaturePresenceV1::Pending,
            Self::Finalized(_) => SignaturePresenceV1::Finalized,
        }
    }
}

fn observe_signature(rpc: &mut Rpc, signature: &str) -> Result<SignatureObservationV1> {
    let statuses = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let values = statuses
        .get("value")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("wallet payout signature status omitted value array"))?;
    if values.len() != 1 {
        return Err(refusal(
            "wallet payout signature status did not preserve request width",
        ));
    }
    let status = &values[0];
    if status.is_null() {
        return Ok(SignatureObservationV1::Absent);
    }
    if status.get("err").is_some_and(|value| !value.is_null()) {
        return Err(refusal(format!(
            "wallet payout signature {signature} failed: {}",
            status.get("err").unwrap_or(&Value::Null)
        )));
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(SignatureObservationV1::Pending);
    }
    let history = rpc.call(
        "getTransaction",
        &json!([signature, {"encoding":"base64", "commitment":"finalized", "maxSupportedTransactionVersion":0}]),
    )?;
    if history.is_null() {
        return Err(refusal(
            "finalized wallet payout omitted transaction history",
        ));
    }
    Ok(SignatureObservationV1::Finalized(history))
}

fn finalized_transaction(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    match observe_signature(rpc, signature)? {
        SignatureObservationV1::Finalized(history) => Ok(Some(history)),
        SignatureObservationV1::Absent | SignatureObservationV1::Pending => Ok(None),
    }
}

fn wait_finalized(rpc: &mut Rpc, signature: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(history) = finalized_transaction(rpc, signature)? {
            return Ok(history);
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(refusal(format!(
        "wallet payout {signature} did not finalize before the bounded deadline; its durable signature remains poll-only"
    )))
}

fn snapshot(rpc: &mut Rpc, selected: &SelectedInputV1) -> Result<FinalizedSnapshotV1> {
    let addresses = selected.addresses();
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    snapshot_from_rpc(slot, rpc.block_time(slot)?, &addresses, values)
}

fn finalized_account(rpc: &mut Rpc, key: Pubkey, floor: u64) -> Result<ObservedAccount> {
    finalized_accounts(rpc, &[key], floor)?
        .into_iter()
        .next()
        .ok_or_else(|| refusal("finalized account result vanished"))
}

fn finalized_accounts(rpc: &mut Rpc, keys: &[Pubkey], floor: u64) -> Result<Vec<ObservedAccount>> {
    let (slot, values) = rpc.finalized_accounts(keys, floor)?;
    let observation = dclutch_operator::Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: dclutch_operator::Finality::Finalized,
    };
    keys.iter()
        .copied()
        .zip(values)
        .map(|(key, value)| {
            let value = value
                .ok_or_else(|| refusal(format!("finalized payout poststate {key} is absent")))?;
            Ok(ObservedAccount {
                observation,
                key,
                owner: value.owner,
                lamports: value.lamports,
                executable: value.executable,
                data: value.data,
            })
        })
        .collect()
}

fn authenticate_lookup_prefix(
    planning: &PlanningV1,
    journal: &JournalV1,
    account: &ObservedAccount,
) -> Result<()> {
    if account.owner != lookup_table_program::id() || account.executable {
        return Err(refusal(
            "wallet payout lookup account changed owner or executable bit",
        ));
    }
    let table = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("wallet payout lookup data refused"))?;
    let pages = usize::from(journal.action_index);
    let expected_len = pages
        .checked_mul(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
        .unwrap_or(usize::MAX)
        .min(planning.lookup_addresses.len());
    if table.meta.deactivation_slot != u64::MAX
        || table.meta.authority != Some(parse_pubkey(&journal.fee_payer, "lookup authority")?)
        || table.addresses.as_ref() != &planning.lookup_addresses[..expected_len]
    {
        return Err(refusal("wallet payout lookup extension prefix changed"));
    }
    Ok(())
}

fn authenticate_lookup_table(
    planning: &PlanningV1,
    account: &ObservedAccount,
    authority: Option<Pubkey>,
    activated: bool,
) -> Result<()> {
    if account.key != planning.lookup_table
        || account.owner != lookup_table_program::id()
        || account.executable
    {
        return Err(refusal("wallet payout lookup account identity changed"));
    }
    let table = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("wallet payout lookup data refused"))?;
    if table.meta.deactivation_slot != u64::MAX {
        return Err(refusal("wallet payout lookup was deactivated"));
    }
    if authority.is_none() {
        if table.meta.authority.is_some()
            || table.addresses.as_ref() != planning.lookup_addresses.as_slice()
        {
            return Err(refusal(
                "wallet payout lookup was not the exact frozen canonical sequence",
            ));
        }
    } else if table.meta.authority != authority || !table.addresses.is_empty() {
        return Err(refusal(
            "fresh wallet payout lookup had another authority or contents",
        ));
    }
    if activated && account.observation.slot <= table.meta.last_extended_slot {
        return Err(refusal(
            "wallet payout lookup has not crossed its activation slot",
        ));
    }
    Ok(())
}

fn verify_payout_poststates(
    planning: &PlanningV1,
    journal: &JournalV1,
    accounts: &[ObservedAccount],
) -> Result<()> {
    authenticate_expected_accounts(accounts, &journal.expected_poststates)?;
    if accounts.len() != 5 {
        return Err(refusal("wallet payout poststate width changed"));
    }
    let receipt = BASE64
        .decode(
            journal
                .return_data_base64
                .as_deref()
                .ok_or_else(|| refusal("wallet payout receipt missing"))?,
        )
        .map_err(|error| Error::new(format!("wallet payout receipt: {error}")))?;
    verify_wallet_terminal_payout_postcondition_v3(
        &planning.report,
        WalletTerminalPayoutPoststateV3 {
            receipt_bytes: &receipt,
            aggregate_bytes: &accounts[0].data,
            position_bytes: &accounts[1].data,
            custody_replay_bytes: &accounts[2].data,
            hoard_token_bytes: &accounts[3].data,
            recipient_token_bytes: &accounts[4].data,
        },
    )
    .map_err(|error| Error::new(format!("wallet payout semantic postcondition: {error:?}")))
}

fn authenticate_expected_accounts(
    accounts: &[ObservedAccount],
    expected: &[ExpectedAccountV1],
) -> Result<()> {
    if accounts.len() != expected.len() {
        return Err(refusal("wallet payout poststate width changed"));
    }
    for (account, expected) in accounts.iter().zip(expected) {
        let data = BASE64
            .decode(&expected.data_base64)
            .map_err(|error| Error::new(format!("wallet payout expected poststate: {error}")))?;
        if account.key.to_string() != expected.address
            || account.owner.to_string() != expected.owner
            || account.lamports != expected.lamports
            || account.executable != expected.executable
            || account.data != data
            || sha256_hex(&account.data) != expected.data_sha256
        {
            return Err(refusal(format!(
                "wallet payout poststate {} changed",
                expected.address
            )));
        }
    }
    Ok(())
}

fn publish_evidence(
    arguments: &ArgumentsV1,
    planning: &PlanningV1,
    journal: &JournalV1,
) -> Result<EvidenceV1> {
    let mut evidence = EvidenceV1 {
        schema: evidence_schema_v1(arguments.expected_cluster).into(),
        cluster: arguments.expected_cluster.evidence_label().into(),
        input_sha256: journal.input_sha256.clone(),
        payout_intent_sha256: journal.payout_intent_sha256.clone(),
        journal_state_sha256: journal.state_sha256.clone(),
        signature: journal
            .expected_signature
            .clone()
            .ok_or_else(|| refusal("final payout journal omitted signature"))?,
        finalized_slot: journal
            .finalized_slot
            .ok_or_else(|| refusal("final payout journal omitted slot"))?,
        fee_lamports: journal
            .fee_lamports
            .ok_or_else(|| refusal("final payout journal omitted fee"))?,
        compute_units_consumed: journal
            .compute_units_consumed
            .ok_or_else(|| refusal("final payout journal omitted compute"))?,
        fee_payer: journal.fee_payer.clone(),
        owner: journal.owner.clone(),
        market: planning.selected.market.to_string(),
        recipient: planning.selected.recipient.to_string(),
        payout: planning.report.payout.to_string(),
        lookup_table: journal.lookup_table.clone(),
        lookup_addresses_sha256: journal.lookup_addresses_sha256.clone(),
        payout_instruction_sha256: journal.payout_instruction_sha256.clone(),
        custody_request_sha256: journal.custody_request_sha256.clone(),
        return_data_producer: journal
            .return_data_producer
            .clone()
            .ok_or_else(|| refusal("final payout journal omitted return producer"))?,
        return_data_base64: journal
            .return_data_base64
            .clone()
            .ok_or_else(|| refusal("final payout journal omitted return bytes"))?,
        poststates: journal.finalized_poststates.clone(),
        evidence_sha256: String::new(),
    };
    evidence.evidence_sha256 = evidence_digest(&evidence)?;
    write_create_only_json(&arguments.evidence, &evidence)?;
    Ok(evidence)
}

fn authenticate_evidence(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<EvidenceV1> {
    let bytes = fs::read(&arguments.evidence)?;
    let evidence: EvidenceV1 = serde_json::from_slice(&bytes)?;
    authenticate_evidence_value(rpc, arguments, &evidence)?;
    Ok(evidence)
}

fn authenticate_evidence_value(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    evidence: &EvidenceV1,
) -> Result<()> {
    let input_sha256 = sha256_hex(&fs::read(&arguments.input)?);
    if evidence.schema != evidence_schema_v1(arguments.expected_cluster)
        || evidence.cluster != arguments.expected_cluster.evidence_label()
        || evidence.fee_payer != arguments.fee_payer.to_string()
        || evidence.input_sha256 != input_sha256
        || evidence.evidence_sha256 != evidence_digest(evidence)?
    {
        return Err(refusal("wallet payout evidence identity changed"));
    }
    let journals = load_journals(arguments)?;
    let journal = authenticate_evidence_journal(evidence, journals.last())?;
    authenticate_finalized_history(rpc, journal)
}

fn authenticate_evidence_journal<'a>(
    evidence: &EvidenceV1,
    journal: Option<&'a JournalV1>,
) -> Result<&'a JournalV1> {
    let journal = journal
        .filter(|journal| journal.stage == StageV1::Payout && journal.phase == PhaseV1::Finalized)
        .ok_or_else(|| refusal("wallet payout evidence omitted its finalized payout journal"))?;
    if journal.state_sha256 != evidence.journal_state_sha256
        || journal.input_sha256 != evidence.input_sha256
        || journal.payout_intent_sha256 != evidence.payout_intent_sha256
        || journal.expected_signature.as_deref() != Some(evidence.signature.as_str())
        || journal.finalized_slot != Some(evidence.finalized_slot)
        || journal.fee_lamports != Some(evidence.fee_lamports)
        || journal.compute_units_consumed != Some(evidence.compute_units_consumed)
        || journal.finalized_poststates != evidence.poststates
    {
        return Err(refusal(
            "wallet payout evidence and finalized journal diverged",
        ));
    }
    Ok(journal)
}

fn preflight(arguments: &ArgumentsV1, planning: &PlanningV1, journal: Option<&JournalV1>) -> Value {
    json!({
        "schema": preflight_schema_v1(arguments.expected_cluster),
        "cluster": arguments.expected_cluster.evidence_label(),
        "mutationPermitted": false,
        "keysOpened": false,
        "inputSha256": planning.input_sha256,
        "payoutIntentSha256": planning.payout_intent_sha256,
        "observationSlot": planning.report.observation.slot,
        "lookupTable": planning.lookup_table.to_string(),
        "lookupAddressCount": planning.lookup_addresses.len(),
        "lookupAddressesSha256": planning.lookup_addresses_sha256,
        "payout": planning.report.payout.to_string(),
        "nextStage": journal.map(|journal| journal.stage).unwrap_or(planning.next_stage),
        "phase": journal.map(|journal| journal.phase),
        "recovery": journal.filter(|journal| matches!(journal.phase, PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted)).map(|journal| json!({
            "signature": journal.expected_signature,
            "mode": match journal.phase {
                PhaseV1::Dispatching => "poll-then-identical-resend-only-if-absent",
                _ => "poll-only"
            }
        }))
    })
}

fn authenticate_identity(
    journal: &JournalV1,
    expected_cluster: ExpectedClusterV1,
    input_sha256: &str,
    payout_intent_sha256: &str,
    fee_payer: Pubkey,
    owner: Pubkey,
    lookup_creation_slot: u64,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    instruction_base64: &str,
    instruction_sha256: &str,
    custody_request_base64: Option<&str>,
    custody_request_sha256: Option<&str>,
) -> Result<()> {
    if journal.schema != journal_schema_v1(expected_cluster)
        || journal.input_sha256 != input_sha256
        || journal.payout_intent_sha256 != payout_intent_sha256
        || journal.fee_payer != fee_payer.to_string()
        || journal.owner != owner.to_string()
        || journal.lookup_creation_slot != lookup_creation_slot
        || journal.lookup_table != lookup_table.to_string()
        || journal.lookup_addresses
            != lookup_addresses
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        || journal.lookup_addresses_sha256 != pubkey_list_sha256(lookup_addresses)
        || journal.payout_instruction_base64 != instruction_base64
        || journal.payout_instruction_sha256 != instruction_sha256
        || journal.custody_request_base64.as_deref() != custody_request_base64
        || journal.custody_request_sha256.as_deref() != custody_request_sha256
        || journal.state_sha256 != journal_state_sha256(journal)?
    {
        return Err(refusal(
            "wallet payout journal identity, Custody request, or state digest changed",
        ));
    }
    authenticate_phase_envelope(journal)
}

fn authenticate_phase_envelope(journal: &JournalV1) -> Result<()> {
    let has_packet = journal.signed_packet_base64.is_some();
    let has_signature = journal.expected_signature.is_some();
    let has_any_finalization = journal.finalized_slot.is_some()
        || journal.transaction_sha256.is_some()
        || journal.fee_lamports.is_some()
        || journal.compute_units_consumed.is_some()
        || journal.return_data_producer.is_some()
        || journal.return_data_base64.is_some()
        || !journal.finalized_poststates.is_empty();
    match journal.phase {
        PhaseV1::Planned => {
            if has_packet || has_signature || has_any_finalization {
                return Err(refusal(
                    "Planned wallet payout journal carried signed or finalized facts",
                ));
            }
        }
        PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted => {
            if !has_packet || !has_signature || has_any_finalization {
                return Err(refusal(
                    "ambiguous wallet payout phase did not carry exactly its signed packet envelope",
                ));
            }
        }
        PhaseV1::Finalized if journal.stage == StageV1::LookupActivation => {
            if has_packet
                || has_signature
                || journal.finalized_slot.is_none()
                || journal.transaction_sha256.is_some()
                || journal.fee_lamports.is_some()
                || journal.compute_units_consumed.is_some()
                || journal.return_data_producer.is_some()
                || journal.return_data_base64.is_some()
                || !journal.finalized_poststates.is_empty()
            {
                return Err(refusal(
                    "lookup activation finalization carried transaction facts",
                ));
            }
        }
        PhaseV1::Finalized => {
            if !has_packet
                || !has_signature
                || journal.finalized_slot.is_none()
                || journal.transaction_sha256.is_none()
                || journal.fee_lamports.is_none()
                || journal.compute_units_consumed.is_none()
                || journal.finalized_poststates.is_empty()
            {
                return Err(refusal(
                    "Finalized wallet payout transaction omitted its authenticated envelope",
                ));
            }
        }
    }
    Ok(())
}

fn authenticate_signed_packet_envelope(journal: &JournalV1) -> Result<(Vec<u8>, Signature)> {
    if !matches!(
        journal.phase,
        PhaseV1::Dispatching
            | PhaseV1::SignedNotSubmitted
            | PhaseV1::Submitted
            | PhaseV1::Finalized
    ) || journal.stage == StageV1::LookupActivation
    {
        return Err(refusal(
            "wallet payout signed-packet authentication requires one transaction phase",
        ));
    }
    let encoded = journal
        .signed_packet_base64
        .as_deref()
        .ok_or_else(|| refusal("signed wallet payout journal omitted packet"))?;
    let packet = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("wallet payout packet base64: {error}")))?;
    if BASE64.encode(&packet) != encoded
        || Some(packet.len()) != journal.expected_wire_bytes
        || packet.len() > 1_232
    {
        return Err(refusal(
            "signed wallet payout packet encoding or wire width changed",
        ));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("wallet payout signed packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("wallet payout packet reencode: {error}")))?
        != packet
    {
        return Err(refusal("wallet payout signed packet was noncanonical"));
    }
    transaction
        .verify_and_hash_message()
        .map_err(|error| Error::new(format!("wallet payout packet signatures: {error}")))?;
    let message_bytes = bincode::serialize(&transaction.message)
        .map_err(|error| Error::new(format!("wallet payout packet message: {error}")))?;
    let encoded_message = journal
        .message_base64
        .as_deref()
        .ok_or_else(|| refusal("signed wallet payout journal omitted message"))?;
    if BASE64.encode(&message_bytes) != encoded_message
        || journal.message_sha256.as_deref() != Some(sha256_hex(&message_bytes).as_str())
    {
        return Err(refusal(
            "wallet payout signed packet substituted its durable action",
        ));
    }
    let (header, static_keys) = match &transaction.message {
        VersionedMessage::Legacy(message) => (&message.header, message.account_keys.as_slice()),
        VersionedMessage::V0(message) => (&message.header, message.account_keys.as_slice()),
    };
    let payer = parse_pubkey(&journal.fee_payer, "wallet payout fee payer")?;
    let owner = parse_pubkey(&journal.owner, "wallet payout owner")?;
    let expected_signers = if journal.stage == StageV1::Payout && owner != payer {
        vec![payer, owner]
    } else {
        vec![payer]
    };
    let required = usize::from(header.num_required_signatures);
    if required != expected_signers.len()
        || transaction.signatures.len() != required
        || static_keys.get(..required) != Some(expected_signers.as_slice())
    {
        return Err(refusal(
            "wallet payout signed packet action authority or signer width changed",
        ));
    }
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| refusal("wallet payout signed packet omitted payer signature"))?;
    if journal.expected_signature.as_deref() != Some(signature.to_string().as_str()) {
        return Err(refusal(
            "wallet payout signed packet expected signature changed",
        ));
    }
    Ok((packet, signature))
}

fn park_payout_chaos_boundary(
    expected_cluster: ExpectedClusterV1,
    journal_path: &Path,
    journal: &JournalV1,
    packet: &[u8],
    boundary: chaos_fault::BoundaryV1,
) -> Result<()> {
    if expected_cluster != ExpectedClusterV1::OwnedLoopback || journal.stage != StageV1::Payout {
        return Ok(());
    }
    if journal.phase != PhaseV1::Dispatching {
        return Err(refusal(
            "wallet-terminal-payout chaos seam requires durable Dispatching",
        ));
    }
    let signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("wallet-terminal-payout chaos seam omitted signature"))?;
    chaos_fault::park_if_armed_v1(
        "owned-loopback",
        "wallet-terminal-payout",
        boundary,
        journal_path,
        &journal.payout_intent_sha256,
        &sha256_hex(packet),
        signature,
    )
}

fn authenticate_journal(
    planning: &PlanningV1,
    arguments: &ArgumentsV1,
    journal: &JournalV1,
) -> Result<()> {
    authenticate_identity(
        journal,
        arguments.expected_cluster,
        &planning.input_sha256,
        &planning.payout_intent_sha256,
        arguments.fee_payer,
        planning.report.owner,
        planning.lookup_creation_slot,
        planning.lookup_table,
        &planning.lookup_addresses,
        &planning.payout_instruction_base64,
        &planning.payout_instruction_sha256,
        planning.custody_request_base64.as_deref(),
        planning.custody_request_sha256.as_deref(),
    )
}

fn load_journals(arguments: &ArgumentsV1) -> Result<Vec<JournalV1>> {
    let mut entries = fs::read_dir(&arguments.journal_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .ends_with(".wallet-payout.json")
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut journals = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let bytes = fs::read(entry.path())?;
        let journal: JournalV1 = serde_json::from_slice(&bytes)?;
        if usize::from(journal.action_index) != index
            || journal.state_sha256 != journal_state_sha256(&journal)?
            || journals
                .last()
                .is_some_and(|previous: &JournalV1| previous.phase != PhaseV1::Finalized)
        {
            return Err(refusal(
                "wallet payout journals were not one finalized ordered prefix",
            ));
        }
        journals.push(journal);
    }
    Ok(journals)
}

fn journal_path(root: &Path, index: u16, stage: StageV1) -> PathBuf {
    root.join(format!(
        "{index:02}-{}.wallet-payout.json",
        stage_name(stage)
    ))
}

fn stage_name(stage: StageV1) -> &'static str {
    match stage {
        StageV1::LookupCreate => "lookup-create",
        StageV1::LookupExtend => "lookup-extend",
        StageV1::LookupFreeze => "lookup-freeze",
        StageV1::LookupActivation => "lookup-activation",
        StageV1::Payout => "payout",
    }
}

fn write_journal(
    path: &Path,
    journal: &JournalV1,
    create: bool,
    previous: Option<&str>,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("wallet payout journal omitted parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("wallet payout journal omitted UTF-8 name"))?;
    let lock = parent.join(format!(".{name}.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            refusal(format!(
                "acquire exclusive wallet payout journal lock {}: {error}",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    let result = (|| {
        if journal.state_sha256 != journal_state_sha256(journal)? {
            return Err(refusal("wallet payout journal digest changed before write"));
        }
        if create && path.exists() {
            return Err(refusal("wallet payout journal already exists"));
        }
        if !create {
            let persisted: JournalV1 = serde_json::from_slice(&fs::read(path)?)?;
            if previous != Some(persisted.state_sha256.as_str()) {
                return Err(refusal("wallet payout journal update used stale state"));
            }
        }
        let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(journal)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        if create {
            fs::hard_link(&temporary, path)?;
            fs::remove_file(&temporary)?;
        } else {
            fs::rename(&temporary, path)?;
        }
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    drop(lock_file);
    let remove = fs::remove_file(&lock);
    match (result, remove) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(Error::new(format!(
            "release wallet payout journal lock {}: {error}",
            lock.display()
        ))),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn persist_update(path: &Path, journal: &mut JournalV1) -> Result<()> {
    let previous = journal.state_sha256.clone();
    refresh_state_sha256(journal)?;
    write_journal(path, journal, false, Some(&previous))
}

fn refresh_state_sha256(journal: &mut JournalV1) -> Result<()> {
    journal.state_sha256.clear();
    journal.state_sha256 = journal_state_sha256(journal)?;
    Ok(())
}

fn journal_state_sha256(journal: &JournalV1) -> Result<String> {
    let mut projected = journal.clone();
    projected.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&projected)?))
}

fn evidence_digest(evidence: &EvidenceV1) -> Result<String> {
    let mut projected = evidence.clone();
    projected.evidence_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&projected)?))
}

fn payout_intent_sha256(
    expected_cluster: ExpectedClusterV1,
    input_sha256: &str,
    payer: Pubkey,
    owner: Pubkey,
    instruction: &[u8],
    custody_request: Option<&[u8]>,
    creation_slot: u64,
    lookup_table: Pubkey,
    addresses: &[Pubkey],
) -> String {
    let mut digest = Sha256::new();
    digest.update(match expected_cluster {
        ExpectedClusterV1::Devnet => b"dclutch-devnet-wallet-terminal-payout-intent-v1".as_slice(),
        ExpectedClusterV1::OwnedLoopback => {
            b"dclutch-owned-loopback-wallet-terminal-payout-intent-v1".as_slice()
        }
    });
    digest.update(input_sha256.as_bytes());
    digest.update(payer.to_bytes());
    digest.update(owner.to_bytes());
    digest.update(instruction);
    digest.update(custody_request.unwrap_or_default());
    digest.update(creation_slot.to_le_bytes());
    digest.update(lookup_table.to_bytes());
    for address in addresses {
        digest.update(address.to_bytes());
    }
    hex(&digest.finalize())
}

fn pubkey_list_sha256(values: &[Pubkey]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"dclutch-wallet-terminal-payout-lookup-addresses-v1");
    for value in values {
        digest.update(value.to_bytes());
    }
    hex(&digest.finalize())
}

fn latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| refusal("wallet payout getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("wallet payout latest blockhash omitted hash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("wallet payout blockhash: {error}")))?;
    let height = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("wallet payout latest blockhash omitted last valid height"))?;
    Ok((blockhash, height))
}

fn read_keypair(path: &Path, expected: Pubkey, label: &str) -> Result<Keypair> {
    let keypair = Keypair::new_from_array(campaign::read_keypair_file(path, label)?);
    if keypair.pubkey() != expected {
        return Err(refusal(format!(
            "{label} keypair did not expand to {expected}"
        )));
    }
    Ok(keypair)
}

fn absolute_existing(value: String, label: &str, directory: bool) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || !path.exists() || path.is_dir() != directory {
        return Err(refusal(format!(
            "{label} must name an existing absolute {}",
            if directory { "directory" } else { "file" }
        )));
    }
    Ok(path)
}

fn absolute_output(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.parent().is_none_or(|parent| !parent.is_dir()) {
        return Err(refusal(format!(
            "{label} must be an absolute path under an existing directory"
        )));
    }
    Ok(path)
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn observed_account(account: &ObservedAccount) -> ObservedAccountV1 {
    ObservedAccountV1 {
        address: account.key.to_string(),
        owner: account.owner.to_string(),
        lamports: account.lamports,
        executable: account.executable,
        data_len: account.data.len(),
        data_sha256: sha256_hex(&account.data),
    }
}

fn write_create_only_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if path.exists() {
        return Err(refusal("wallet payout evidence already exists"));
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(
        path.parent()
            .ok_or_else(|| refusal("wallet payout evidence omitted parent"))?,
    )?
    .sync_all()?;
    Ok(())
}

fn stdout_json(value: &impl Serialize) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("REFUSED: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_operator::{Finality, Observation};
    use solana_sdk_ids::system_program;

    fn journal() -> JournalV1 {
        let mut journal = JournalV1 {
            schema: JOURNAL_SCHEMA_V1.into(),
            input_sha256: "11".repeat(32),
            payout_intent_sha256: "22".repeat(32),
            fee_payer: Pubkey::new_unique().to_string(),
            owner: Pubkey::new_unique().to_string(),
            stage: StageV1::Payout,
            action_index: 4,
            phase: PhaseV1::Planned,
            observation_slot: 9,
            lookup_creation_slot: 7,
            lookup_table: Pubkey::new_unique().to_string(),
            lookup_addresses: vec![Pubkey::new_unique().to_string()],
            lookup_addresses_sha256: "33".repeat(32),
            payout_instruction_base64: BASE64.encode([1, 2, 3]),
            payout_instruction_sha256: "44".repeat(32),
            custody_request_base64: Some(BASE64.encode([4, 5, 6])),
            custody_request_sha256: Some("55".repeat(32)),
            message_base64: Some(BASE64.encode([7, 8])),
            message_sha256: Some("66".repeat(32)),
            last_valid_block_height: Some(99),
            exact_fee_lamports: Some(5_000),
            expected_wire_bytes: Some(900),
            signed_packet_base64: None,
            expected_signature: None,
            expected_return_data_producer: Some(Pubkey::new_unique().to_string()),
            expected_return_data_base64: Some(BASE64.encode([9])),
            expected_poststates: Vec::new(),
            finalized_slot: None,
            transaction_sha256: None,
            fee_lamports: None,
            compute_units_consumed: None,
            return_data_producer: None,
            return_data_base64: None,
            finalized_poststates: Vec::new(),
            state_sha256: String::new(),
        };
        refresh_state_sha256(&mut journal).unwrap();
        journal
    }

    fn history_fixture() -> (JournalV1, Value) {
        let payer = Keypair::new();
        let message = VersionedMessage::Legacy(Message::new_with_blockhash(
            &[],
            Some(&payer.pubkey()),
            &Hash::new_unique(),
        ));
        let transaction = VersionedTransaction::try_new(message.clone(), &[&payer]).unwrap();
        let packet = bincode::serialize(&transaction).unwrap();
        let message_bytes = bincode::serialize(&message).unwrap();
        let mut journal = journal();
        journal.stage = StageV1::LookupCreate;
        journal.phase = PhaseV1::Submitted;
        journal.message_base64 = Some(BASE64.encode(message_bytes));
        journal.signed_packet_base64 = Some(BASE64.encode(&packet));
        journal.expected_signature = Some(transaction.signatures[0].to_string());
        journal.expected_return_data_producer = None;
        journal.expected_return_data_base64 = None;
        journal.exact_fee_lamports = Some(5_000);
        refresh_state_sha256(&mut journal).unwrap();
        let history = json!({
            "slot": 55,
            "meta": {
                "err": null,
                "fee": 5_000,
                "computeUnitsConsumed": 321,
                "returnData": null
            },
            "transaction": [BASE64.encode(packet), "base64"]
        });
        (journal, history)
    }

    fn signed_payout_fixture() -> JournalV1 {
        let payer = Keypair::new();
        let owner = Keypair::new();
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![solana_program::instruction::AccountMeta::new_readonly(
                owner.pubkey(),
                true,
            )],
            data: vec![7, 8, 9],
        };
        let message = VersionedMessage::Legacy(Message::new_with_blockhash(
            &[instruction],
            Some(&payer.pubkey()),
            &Hash::new_unique(),
        ));
        let transaction =
            VersionedTransaction::try_new(message.clone(), &[&payer, &owner]).unwrap();
        let message_bytes = bincode::serialize(&message).unwrap();
        let packet = bincode::serialize(&transaction).unwrap();
        let mut journal = journal();
        journal.fee_payer = payer.pubkey().to_string();
        journal.owner = owner.pubkey().to_string();
        journal.message_base64 = Some(BASE64.encode(&message_bytes));
        journal.message_sha256 = Some(sha256_hex(&message_bytes));
        journal.expected_wire_bytes = Some(packet.len());
        journal.signed_packet_base64 = Some(BASE64.encode(&packet));
        journal.expected_signature = Some(transaction.signatures[0].to_string());
        journal.phase = PhaseV1::Dispatching;
        refresh_state_sha256(&mut journal).unwrap();
        journal
    }

    fn evidence_for(journal: &JournalV1) -> EvidenceV1 {
        EvidenceV1 {
            schema: EVIDENCE_SCHEMA_V1.into(),
            cluster: "owned-loopback".into(),
            input_sha256: journal.input_sha256.clone(),
            payout_intent_sha256: journal.payout_intent_sha256.clone(),
            journal_state_sha256: journal.state_sha256.clone(),
            signature: journal.expected_signature.clone().unwrap(),
            finalized_slot: journal.finalized_slot.unwrap(),
            fee_lamports: journal.fee_lamports.unwrap(),
            compute_units_consumed: journal.compute_units_consumed.unwrap(),
            fee_payer: journal.fee_payer.clone(),
            owner: journal.owner.clone(),
            market: Pubkey::new_unique().to_string(),
            recipient: Pubkey::new_unique().to_string(),
            payout: "1".into(),
            lookup_table: journal.lookup_table.clone(),
            lookup_addresses_sha256: journal.lookup_addresses_sha256.clone(),
            payout_instruction_sha256: journal.payout_instruction_sha256.clone(),
            custody_request_sha256: journal.custody_request_sha256.clone(),
            return_data_producer: journal.return_data_producer.clone().unwrap(),
            return_data_base64: journal.return_data_base64.clone().unwrap(),
            poststates: journal.finalized_poststates.clone(),
            evidence_sha256: String::new(),
        }
    }

    #[test]
    fn journal_digest_refuses_substitution() {
        let journal = journal();
        let mut substituted = journal.clone();
        substituted.custody_request_base64 = Some(BASE64.encode([0xff]));
        assert_ne!(
            substituted.state_sha256,
            journal_state_sha256(&substituted).unwrap()
        );
    }

    #[test]
    fn ambiguous_phases_are_not_planned() {
        for phase in [
            PhaseV1::Dispatching,
            PhaseV1::SignedNotSubmitted,
            PhaseV1::Submitted,
        ] {
            let mut journal = journal();
            journal.phase = phase;
            journal.expected_signature = Some(Signature::new_unique().to_string());
            refresh_state_sha256(&mut journal).unwrap();
            assert!(matches!(
                journal.phase,
                PhaseV1::Dispatching | PhaseV1::SignedNotSubmitted | PhaseV1::Submitted
            ));
            assert!(journal.expected_signature.is_some());
            assert_eq!(
                recovery_action(journal.phase),
                if phase == PhaseV1::Dispatching {
                    RecoveryActionV1::PollThenResendIdentical
                } else {
                    RecoveryActionV1::PollOnly
                }
            );
        }
        assert_eq!(
            recovery_action(PhaseV1::Planned),
            RecoveryActionV1::SignOnceAndPersistDispatching
        );
        assert_eq!(
            recovery_action(PhaseV1::Finalized),
            RecoveryActionV1::AuthenticateFinalized
        );
    }

    #[test]
    fn only_dispatching_plus_absent_authorizes_identical_resend() {
        assert_eq!(
            dispatch_route(PhaseV1::Dispatching, SignaturePresenceV1::Absent).unwrap(),
            DispatchRouteV1::ResendIdentical
        );
        for presence in [SignaturePresenceV1::Absent, SignaturePresenceV1::Pending] {
            assert_eq!(
                dispatch_route(PhaseV1::Submitted, presence).unwrap(),
                DispatchRouteV1::PollOnly
            );
            assert_eq!(
                dispatch_route(PhaseV1::SignedNotSubmitted, presence).unwrap(),
                DispatchRouteV1::PollOnly
            );
        }
        assert_eq!(
            dispatch_route(PhaseV1::Dispatching, SignaturePresenceV1::Pending).unwrap(),
            DispatchRouteV1::PollOnly
        );
        for phase in [
            PhaseV1::Dispatching,
            PhaseV1::SignedNotSubmitted,
            PhaseV1::Submitted,
        ] {
            assert_eq!(
                dispatch_route(phase, SignaturePresenceV1::Finalized).unwrap(),
                DispatchRouteV1::AuthenticateFinalized
            );
        }
        assert!(dispatch_route(PhaseV1::Planned, SignaturePresenceV1::Absent).is_err());
        assert!(dispatch_route(PhaseV1::Finalized, SignaturePresenceV1::Finalized).is_err());
    }

    #[test]
    fn dispatching_packet_authenticates_action_authority_and_wire_width() {
        let exact = signed_payout_fixture();
        assert!(authenticate_phase_envelope(&exact).is_ok());
        assert!(authenticate_signed_packet_envelope(&exact).is_ok());

        let mut changed_action = exact.clone();
        let alternate = VersionedMessage::Legacy(Message::new_with_blockhash(
            &[],
            Some(&parse_pubkey(&exact.fee_payer, "payer").unwrap()),
            &Hash::new_unique(),
        ));
        let alternate_bytes = bincode::serialize(&alternate).unwrap();
        changed_action.message_base64 = Some(BASE64.encode(&alternate_bytes));
        changed_action.message_sha256 = Some(sha256_hex(&alternate_bytes));
        refresh_state_sha256(&mut changed_action).unwrap();
        assert!(authenticate_signed_packet_envelope(&changed_action).is_err());

        let mut changed_authority = exact.clone();
        changed_authority.owner = Pubkey::new_unique().to_string();
        refresh_state_sha256(&mut changed_authority).unwrap();
        assert!(authenticate_signed_packet_envelope(&changed_authority).is_err());

        let mut changed_width = exact;
        changed_width.expected_wire_bytes =
            changed_width.expected_wire_bytes.map(|value| value + 1);
        refresh_state_sha256(&mut changed_width).unwrap();
        assert!(authenticate_signed_packet_envelope(&changed_width).is_err());
    }

    #[test]
    fn dispatching_packet_refuses_signature_substitution_and_planned_packet() {
        let exact = signed_payout_fixture();
        let mut resigned = exact.clone();
        let mut transaction: VersionedTransaction = bincode::deserialize(
            &BASE64
                .decode(resigned.signed_packet_base64.as_deref().unwrap())
                .unwrap(),
        )
        .unwrap();
        transaction.signatures[0] = Signature::new_unique();
        let substituted = bincode::serialize(&transaction).unwrap();
        resigned.signed_packet_base64 = Some(BASE64.encode(&substituted));
        resigned.expected_signature = Some(transaction.signatures[0].to_string());
        refresh_state_sha256(&mut resigned).unwrap();
        assert!(authenticate_signed_packet_envelope(&resigned).is_err());

        let mut planned_with_packet = exact;
        planned_with_packet.phase = PhaseV1::Planned;
        refresh_state_sha256(&mut planned_with_packet).unwrap();
        assert!(authenticate_phase_envelope(&planned_with_packet).is_err());
        assert!(authenticate_signed_packet_envelope(&planned_with_packet).is_err());
    }

    #[test]
    fn finalized_evidence_requires_its_exact_payout_journal() {
        let (mut journal, _) = history_fixture();
        journal.stage = StageV1::Payout;
        journal.phase = PhaseV1::Finalized;
        journal.finalized_slot = Some(55);
        journal.fee_lamports = Some(5_000);
        journal.compute_units_consumed = Some(321);
        journal.return_data_producer = Some(Pubkey::new_unique().to_string());
        journal.return_data_base64 = Some(BASE64.encode([7]));
        refresh_state_sha256(&mut journal).unwrap();
        let evidence = evidence_for(&journal);
        assert!(authenticate_evidence_journal(&evidence, Some(&journal)).is_ok());
        assert!(authenticate_evidence_journal(&evidence, None).is_err());
        let mut substituted = journal;
        substituted.payout_intent_sha256 = "ff".repeat(32);
        refresh_state_sha256(&mut substituted).unwrap();
        assert!(authenticate_evidence_journal(&evidence, Some(&substituted)).is_err());
    }

    #[test]
    fn finalized_history_refuses_packet_fee_and_return_substitution() {
        let (journal, history) = history_fixture();
        let exact = authenticate_history(&journal, &history).unwrap();
        assert_eq!(exact.slot, 55);
        assert_eq!(exact.fee, 5_000);
        assert_eq!(exact.compute, 321);

        let mut wrong_packet = history.clone();
        wrong_packet["transaction"][0] = Value::String(BASE64.encode([0xff]));
        assert!(authenticate_history(&journal, &wrong_packet).is_err());

        let mut wrong_fee = history.clone();
        wrong_fee["meta"]["fee"] = Value::from(5_001_u64);
        assert!(authenticate_history(&journal, &wrong_fee).is_err());

        let mut wrong_return = history;
        wrong_return["meta"]["returnData"] = json!({
            "programId": Pubkey::new_unique().to_string(),
            "data": [BASE64.encode([1]), "base64"]
        });
        assert!(authenticate_history(&journal, &wrong_return).is_err());
    }

    #[test]
    fn external_cluster_is_refused_before_rpc() {
        let error = parse_arguments(vec![
            "--rpc-url".into(),
            "https://api.devnet.solana.com".into(),
            "--input".into(),
            "/missing/input".into(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer-keypair".into(),
            "/missing/payer".into(),
            "--owner-keypair".into(),
            "/missing/owner".into(),
            "--journal-dir".into(),
            "/missing/journals".into(),
            "--evidence".into(),
            "/missing/evidence".into(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("not loopback"));
    }

    #[test]
    fn public_devnet_parser_and_domains_are_distinct_from_loopback() {
        let root = std::env::temp_dir().join(format!(
            "dclutch-devnet-wallet-payout-{}-{}",
            std::process::id(),
            Pubkey::new_unique()
        ));
        fs::create_dir(&root).unwrap();
        for name in ["input.json", "payer.json", "owner.json"] {
            fs::write(root.join(name), b"fixture").unwrap();
        }
        let arguments = vec![
            "--rpc-url".into(),
            "https://api.devnet.solana.com:443/".into(),
            "--i-mean-devnet".into(),
            crate::cluster::DEVNET_GENESIS_HASH.into(),
            "--input".into(),
            root.join("input.json").display().to_string(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer-keypair".into(),
            root.join("payer.json").display().to_string(),
            "--owner-keypair".into(),
            root.join("owner.json").display().to_string(),
            "--journal-dir".into(),
            root.display().to_string(),
            "--evidence".into(),
            root.join("evidence.json").display().to_string(),
        ];
        let parsed = parse_arguments_for_cluster_v1(arguments.clone(), ExpectedClusterV1::Devnet)
            .expect("exact devnet payout arm");
        assert_eq!(parsed.expected_cluster, ExpectedClusterV1::Devnet);
        assert_ne!(
            journal_schema_v1(ExpectedClusterV1::Devnet),
            journal_schema_v1(ExpectedClusterV1::OwnedLoopback)
        );
        assert_ne!(
            evidence_schema_v1(ExpectedClusterV1::Devnet),
            evidence_schema_v1(ExpectedClusterV1::OwnedLoopback)
        );
        assert!(
            parse_arguments_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback).is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn payout_intent_digest_has_distinct_cluster_domains() {
        let payer = Pubkey::new_from_array([1; 32]);
        let owner = Pubkey::new_from_array([2; 32]);
        let lookup = Pubkey::new_from_array([3; 32]);
        let addresses = [Pubkey::new_from_array([4; 32])];
        let digest_for = |cluster| {
            payout_intent_sha256(
                cluster,
                &"5".repeat(64),
                payer,
                owner,
                b"instruction",
                None,
                17,
                lookup,
                &addresses,
            )
        };
        assert_ne!(
            digest_for(ExpectedClusterV1::Devnet),
            digest_for(ExpectedClusterV1::OwnedLoopback)
        );
    }

    #[test]
    fn owned_loopback_refuses_mainnet_genesis_even_through_a_tunnel() {
        let origin = ClusterOriginV1::parse("http://127.0.0.1:18899/", None).unwrap();
        assert!(
            origin
                .authenticate_genesis(crate::cluster::MAINNET_BETA_GENESIS_HASH)
                .is_err()
        );
    }

    #[test]
    fn execute_key_reader_refuses_owner_substitution() {
        let keypair = Keypair::new();
        let path = std::env::temp_dir().join(format!(
            "dclutch-wallet-payout-key-{}-{}.json",
            std::process::id(),
            keypair.pubkey()
        ));
        fs::write(
            &path,
            serde_json::to_vec(&keypair.to_bytes().to_vec()).unwrap(),
        )
        .unwrap();
        assert!(read_keypair(&path, keypair.pubkey(), "test-owner").is_ok());
        assert!(read_keypair(&path, Pubkey::new_unique(), "test-owner").is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn return_data_shape_refuses_encoding_substitution() {
        let value = json!({"programId": Pubkey::new_unique().to_string(), "data": [BASE64.encode([1]), "base58"]});
        assert!(parse_return_data(Some(&value)).is_err());
    }

    #[test]
    fn poststate_width_is_exact() {
        let observation = Observation {
            slot: 7,
            unix_timestamp: 9,
            finality: Finality::Finalized,
        };
        let key = Pubkey::new_unique();
        let account = ObservedAccount {
            observation,
            key,
            owner: system_program::ID,
            lamports: 42,
            executable: false,
            data: vec![1, 2, 3],
        };
        let expected = ExpectedAccountV1 {
            address: key.to_string(),
            owner: system_program::ID.to_string(),
            lamports: 42,
            executable: false,
            data_base64: BASE64.encode([1, 2, 3]),
            data_sha256: sha256_hex(&[1, 2, 3]),
        };
        assert!(
            authenticate_expected_accounts(
                core::slice::from_ref(&account),
                core::slice::from_ref(&expected)
            )
            .is_ok()
        );
        let mut substituted = account;
        substituted.data[0] ^= 0xff;
        assert!(authenticate_expected_accounts(&[substituted], &[expected]).is_err());
        assert!(authenticate_expected_accounts(&[], &[]).is_ok());
        assert!(
            authenticate_expected_accounts(
                &[],
                &[ExpectedAccountV1 {
                    address: key.to_string(),
                    owner: system_program::ID.to_string(),
                    lamports: 42,
                    executable: false,
                    data_base64: BASE64.encode([1, 2, 3]),
                    data_sha256: sha256_hex(&[1, 2, 3]),
                }]
            )
            .is_err()
        );
    }
}
