//! Durable exterior for the three-transaction staged-founding abort suffix.
//!
//! The founding campaign checkpoint is immutable input, never the abort's
//! progress owner. This file persists one exact signed packet before its first
//! send, derives every next action from finalized ControllerFundingCheckpoint
//! state, and emits completion only after all three distinct receipts and the
//! exact conservation poststate are present.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::signature::{Keypair, Signature, Signer as _};

use crate::{
    Error, Result,
    campaign::{
        FOUNDING_REQUIRED_ROLES, authenticate_checked_campaign_plan,
        authenticate_checked_live_substrate, load_market_input, read_keypair_file,
    },
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    market::{
        FoundingActorsV1, MarketExecutionCheckpointV1, SourceAbortRecoveryBaselineV1,
        SourceAbortRecoveryOperationV1, SourceAbortRecoveryPhaseV1,
        capture_source_abort_recovery_baseline_v1, plan_source_abort_recovery_v1,
    },
    model::{MarketRunInput, SuccessorPlan, TransactionEvidence},
    rpc::{FinalizedSignedPacketV1, Rpc, SignedVersionedPacketV1, WritePolicyV1},
    seed::{KeyForge, role},
};

pub(crate) const COMMAND_V1: &str = "source-abort-v1";
pub(crate) const INTERRUPTION_AUDIT_COMMAND_V1: &str = "source-abort-interruption-audit-v1";
const EVIDENCE_SCHEMA_V1: &str = "dclutch-source-abort-exterior-evidence-v1";
const COMPLETION_SCHEMA_V1: &str = "dclutch-source-abort-completion-v1";
const INTERRUPTION_CONTRACT_SCHEMA_V1: &str = "dclutch-source-abort-interruption-contract-v1";
const INTERRUPTION_AUDIT_SCHEMA_V1: &str = "dclutch-source-abort-interruption-audit-v1";
const FROZEN_UNION_ALT_CAPTURE_SCHEMA_V1: &str = "dclutch-source-abort-frozen-union-alt-capture-v1";
const MAX_INPUT_BYTES_V1: u64 = 16 * 1024 * 1024;

pub(crate) fn usage() -> &'static str {
    concat!(
        "\n  dclutch-local-successor-bootstrap source-abort-v1 \\\n",
        "     --rpc-url RPC [--acknowledge-devnet-genesis DEVNET_GENESIS] \\\n",
        "     --plan ABSOLUTE_JSON --market ABSOLUTE_JSON --founding-evidence ABSOLUTE_JSON \\\n",
        "     --evidence ABSOLUTE_NEW_OR_EXISTING_JSON --completion ABSOLUTE_JSON \\\n",
        "     --lookup-table FROZEN_PUBKEY --fee-payer PUBKEY --beneficiary PUBKEY \\\n",
        "     --founding-founder PUBKEY --substituted-founder PUBKEY \\\n",
        "     --keypair-campaign-payer ABSOLUTE_KEYPAIR \\\n",
        "     --keypair-collateral-mint ABSOLUTE_KEYPAIR \\\n",
        "     --keypair-collateral-wallet ABSOLUTE_KEYPAIR \\\n",
        "     --keypair-founding-beneficiary ABSOLUTE_KEYPAIR \\\n",
        "     --keypair-founding-projection-witness ABSOLUTE_KEYPAIR \\\n",
        "     --keypair-founding-source-funder ABSOLUTE_KEYPAIR \\\n",
        "     [--interruption-stop-after planned|dispatching|submitted] [--execute]\n\n",
        "Without --execute, this command authenticates the checked deployment set, exact expired DCLTPCB2 checkpoint, and frozen routing table, then fsyncs a key-free intent. Execute reads the six role files only after that boundary and advances at most the exact crash-resumable DCLTPCA1 -> DCLTCF1A -> DCLTCF2A suffix. Rerun until finalized. It refuses an Open success report, pre-expiry abort, mainnet, changed packet, changed phase, missing receipt, or incomplete conservation."
    )
}

pub(crate) fn interruption_audit_usage() -> &'static str {
    concat!(
        "\n  dclutch-local-successor-bootstrap source-abort-interruption-audit-v1 \\\n",
        "     --contract ABSOLUTE_JSON --planned-evidence ABSOLUTE_JSON \\\n",
        "     --dispatching-evidence ABSOLUTE_JSON --submitted-evidence ABSOLUTE_JSON \\\n",
        "     --output ABSOLUTE_NEW_OR_EXISTING_JSON\n\n",
        "This command is offline and key-free. It authenticates three copied SourceAbort evidence snapshots at the Planned, Dispatching, and Submitted kill boundaries for the terminal cleanup operation. The finalized two-operation prefix, immutable invocation, active instruction, exact signed packet, and embedded finalized frozen-union-ALT capture must remain byte-identical. It writes a static audit only; partial evidence never authorizes an Open Market, terminal state, payout, or external mutation."
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    market: PathBuf,
    founding_evidence: PathBuf,
    evidence: PathBuf,
    completion: PathBuf,
    lookup_table: Pubkey,
    fee_payer: Pubkey,
    beneficiary: Pubkey,
    founding_founder: Pubkey,
    substituted_founder: Pubkey,
    keypairs: BTreeMap<String, PathBuf>,
    interruption_stop_after: Option<JournalPhaseV1>,
    execute: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableInstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableInstructionV1 {
    program_id: String,
    accounts: Vec<DurableInstructionAccountV1>,
    data_base64: String,
    data_sha256: String,
}

impl DurableInstructionV1 {
    fn from_instruction(instruction: &Instruction) -> Self {
        Self {
            program_id: instruction.program_id.to_string(),
            accounts: instruction
                .accounts
                .iter()
                .map(|account| DurableInstructionAccountV1 {
                    address: account.pubkey.to_string(),
                    signer: account.is_signer,
                    writable: account.is_writable,
                })
                .collect(),
            data_base64: BASE64.encode(&instruction.data),
            data_sha256: sha256_hex(&instruction.data),
        }
    }

    fn instruction(&self) -> Result<Instruction> {
        let data = BASE64
            .decode(&self.data_base64)
            .map_err(|error| refusal(format!("abort instruction base64: {error}")))?;
        if BASE64.encode(&data) != self.data_base64 || sha256_hex(&data) != self.data_sha256 {
            return Err(refusal("abort instruction data changed"));
        }
        Ok(Instruction {
            program_id: parse_key(&self.program_id, "abort instruction program")?,
            accounts: self
                .accounts
                .iter()
                .map(|account| {
                    let key = parse_key(&account.address, "abort instruction account")?;
                    Ok(if account.writable {
                        solana_program::instruction::AccountMeta::new(key, account.signer)
                    } else {
                        solana_program::instruction::AccountMeta::new_readonly(key, account.signer)
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            data,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum JournalPhaseV1 {
    Planned,
    Dispatching,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SourceAbortJournalV1 {
    operation: SourceAbortRecoveryOperationV1,
    predecessor: SourceAbortRecoveryPhaseV1,
    successor: SourceAbortRecoveryPhaseV1,
    phase: JournalPhaseV1,
    instruction: DurableInstructionV1,
    complete_keys: usize,
    message_bytes: usize,
    packet_bytes: usize,
    packet: Option<SignedVersionedPacketV1>,
    finalized: Option<TransactionEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SourceAbortEvidenceV1 {
    schema: String,
    cluster: String,
    genesis_hash: String,
    rpc_url: String,
    plan_sha256: String,
    market_sha256: String,
    founding_evidence_sha256: String,
    evidence_path: String,
    completion_path: String,
    lookup_table: String,
    lookup_table_sha256: String,
    lookup_table_account: FrozenUnionAltCaptureV1,
    fee_payer: String,
    beneficiary: String,
    founding_founder: String,
    substituted_founder: String,
    founding_checkpoint: MarketExecutionCheckpointV1,
    baseline: Option<SourceAbortRecoveryBaselineV1>,
    journals: Vec<SourceAbortJournalV1>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceAbortCompletionV1<'a> {
    schema: &'static str,
    evidence_sha256: String,
    market: &'a str,
    lifecycle_rent_credit: &'a str,
    controller_funding_source: &'a str,
    principal_atoms: u64,
    controller_native_refund_lamports: u64,
    controller_rent_refund_lamports: u64,
    transactions: Vec<&'a TransactionEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InterruptionBoundaryContractV1 {
    name: String,
    phase: JournalPhaseV1,
    driver_argv: [String; 2],
    recovery: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InterruptionContractV1 {
    schema: String,
    target_operation: SourceAbortRecoveryOperationV1,
    operation_labels: [String; 3],
    boundaries: [InterruptionBoundaryContractV1; 3],
    exact_frozen_union_alt: bool,
    wallet_keys_required: bool,
    rpc_required: bool,
    external_mutation_authorized: bool,
    partial_success_authorized: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FrozenUnionAltCaptureV1 {
    schema: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    key: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
}

impl FrozenUnionAltCaptureV1 {
    fn from_observed(value: &dclutch_versioned_message_operator::ObservedAccount) -> Self {
        Self {
            schema: FROZEN_UNION_ALT_CAPTURE_SCHEMA_V1.into(),
            observation_slot: value.observation.slot,
            observation_unix_timestamp: value.observation.unix_timestamp,
            key: value.key.to_string(),
            owner: value.owner.to_string(),
            lamports: value.lamports,
            executable: value.executable,
            data_base64: BASE64.encode(&value.data),
            data_sha256: sha256_hex(&value.data),
        }
    }

    fn observed_account(&self) -> Result<dclutch_versioned_message_operator::ObservedAccount> {
        if self.schema != FROZEN_UNION_ALT_CAPTURE_SCHEMA_V1 {
            return Err(refusal("interruption audit ALT capture schema changed"));
        }
        let data = BASE64
            .decode(&self.data_base64)
            .map_err(|error| refusal(format!("interruption audit ALT base64: {error}")))?;
        if BASE64.encode(&data) != self.data_base64 || sha256_hex(&data) != self.data_sha256 {
            return Err(refusal("interruption audit ALT bytes or digest changed"));
        }
        Ok(dclutch_versioned_message_operator::ObservedAccount {
            observation: dclutch_versioned_message_operator::Observation {
                slot: self.observation_slot,
                unix_timestamp: self.observation_unix_timestamp,
                finality: dclutch_versioned_message_operator::Finality::Finalized,
            },
            key: parse_key(&self.key, "interruption audit ALT key")?,
            owner: parse_key(&self.owner, "interruption audit ALT owner")?,
            lamports: self.lamports,
            executable: self.executable,
            data,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InterruptionAuditArgumentsV1 {
    contract: PathBuf,
    planned_evidence: PathBuf,
    dispatching_evidence: PathBuf,
    submitted_evidence: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InterruptionAuditReportV1 {
    schema: &'static str,
    contract_sha256: String,
    planned_evidence_sha256: String,
    dispatching_evidence_sha256: String,
    submitted_evidence_sha256: String,
    immutable_invocation_sha256: String,
    lookup_table: String,
    lookup_table_capture_sha256: String,
    lookup_table_data_sha256: String,
    lookup_table_address_sha256: String,
    lookup_table_address_count: usize,
    finalized_prefix_signatures: [String; 2],
    terminal_packet_signature: String,
    terminal_packet_sha256: String,
    boundaries: [InterruptionBoundaryContractV1; 3],
    wallet_key_read_count: u8,
    rpc_call_count: u8,
    external_mutation_count: u8,
    partial_success_authorized: bool,
}

pub(crate) fn run_interruption_audit(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_interruption_audit_arguments_v1(arguments)?;
    let (contract_source, contract) = read_canonical_json_v1::<InterruptionContractV1>(
        &arguments.contract,
        "SourceAbort interruption contract",
    )?;
    let (planned_source, planned) = read_canonical_json_v1::<SourceAbortEvidenceV1>(
        &arguments.planned_evidence,
        "SourceAbort Planned evidence",
    )?;
    let (dispatching_source, dispatching) = read_canonical_json_v1::<SourceAbortEvidenceV1>(
        &arguments.dispatching_evidence,
        "SourceAbort Dispatching evidence",
    )?;
    let (submitted_source, submitted) = read_canonical_json_v1::<SourceAbortEvidenceV1>(
        &arguments.submitted_evidence,
        "SourceAbort Submitted evidence",
    )?;
    let table = planned.lookup_table_account.observed_account()?;
    let report = authenticate_interruption_bundle_v1(
        &contract,
        &planned,
        &dispatching,
        &submitted,
        &table,
        sha256_hex(&contract_source),
        sha256_hex(&planned_source),
        sha256_hex(&dispatching_source),
        sha256_hex(&submitted_source),
        sha256_hex(&canonical_json_v1(&planned.lookup_table_account)?),
    )?;
    write_or_authenticate_json_v1(&arguments.output, &report, "SourceAbort interruption audit")?;
    stdout_v1(serde_json::to_value(report)?)
}

fn expected_interruption_contract_v1() -> InterruptionContractV1 {
    InterruptionContractV1 {
        schema: INTERRUPTION_CONTRACT_SCHEMA_V1.into(),
        target_operation: SourceAbortRecoveryOperationV1::ControllerTerminal,
        operation_labels: SourceAbortRecoveryOperationV1::ORDERED
            .map(|operation| operation.label().to_owned()),
        boundaries: [
            InterruptionBoundaryContractV1 {
                name: "after-planned-fsync".into(),
                phase: JournalPhaseV1::Planned,
                driver_argv: ["--interruption-stop-after".into(), "planned".into()],
                recovery: "reauthenticate-predecessor-then-sign-once-and-fsync-dispatching".into(),
            },
            InterruptionBoundaryContractV1 {
                name: "after-dispatching-fsync-before-send".into(),
                phase: JournalPhaseV1::Dispatching,
                driver_argv: ["--interruption-stop-after".into(), "dispatching".into()],
                recovery: "poll-finalized-then-identical-persisted-packet-resend-only".into(),
            },
            InterruptionBoundaryContractV1 {
                name: "after-submitted-fsync".into(),
                phase: JournalPhaseV1::Submitted,
                driver_argv: ["--interruption-stop-after".into(), "submitted".into()],
                recovery: "poll-exact-persisted-signature-only".into(),
            },
        ],
        exact_frozen_union_alt: true,
        wallet_keys_required: false,
        rpc_required: false,
        external_mutation_authorized: false,
        partial_success_authorized: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_interruption_bundle_v1(
    contract: &InterruptionContractV1,
    planned: &SourceAbortEvidenceV1,
    dispatching: &SourceAbortEvidenceV1,
    submitted: &SourceAbortEvidenceV1,
    table: &dclutch_versioned_message_operator::ObservedAccount,
    contract_sha256: String,
    planned_evidence_sha256: String,
    dispatching_evidence_sha256: String,
    submitted_evidence_sha256: String,
    lookup_table_capture_sha256: String,
) -> Result<InterruptionAuditReportV1> {
    let expected_contract = expected_interruption_contract_v1();
    if contract != &expected_contract {
        return Err(refusal(
            "interruption contract changed its boundaries or authority exclusions",
        ));
    }
    let snapshots = [planned, dispatching, submitted];
    for (snapshot, phase) in snapshots.iter().zip([
        JournalPhaseV1::Planned,
        JournalPhaseV1::Dispatching,
        JournalPhaseV1::Submitted,
    ]) {
        if snapshot.schema != EVIDENCE_SCHEMA_V1 || snapshot.baseline.is_none() {
            return Err(refusal(
                "interruption snapshot omitted the armed SourceAbort schema or baseline",
            ));
        }
        authenticate_journal_prefix_v1(&snapshot.journals)?;
        if snapshot.journals.len() != SourceAbortRecoveryOperationV1::ORDERED.len()
            || snapshot.journals[2].operation != contract.target_operation
            || snapshot.journals[2].phase != phase
            || snapshot.journals[..2]
                .iter()
                .any(|journal| journal.phase != JournalPhaseV1::Finalized)
        {
            return Err(refusal(
                "interruption snapshots were not the exact two-finalized plus terminal-tail prefix",
            ));
        }
    }

    let immutable_invocation_sha256 = interruption_immutable_sha256_v1(planned)?;
    if snapshots.iter().skip(1).any(|snapshot| {
        interruption_immutable_sha256_v1(snapshot).ok().as_deref()
            != Some(immutable_invocation_sha256.as_str())
    }) {
        return Err(refusal(
            "interruption snapshots changed the immutable SourceAbort invocation",
        ));
    }
    for index in 0..2 {
        let expected = canonical_json_v1(&planned.journals[index])?;
        if canonical_json_v1(&dispatching.journals[index])? != expected
            || canonical_json_v1(&submitted.journals[index])? != expected
        {
            return Err(refusal(
                "interruption snapshots changed the exact finalized journal prefix",
            ));
        }
    }
    let active_identity = interruption_active_identity_v1(&planned.journals[2])?;
    if interruption_active_identity_v1(&dispatching.journals[2])? != active_identity
        || interruption_active_identity_v1(&submitted.journals[2])? != active_identity
    {
        return Err(refusal(
            "interruption snapshots changed the active operation or instruction",
        ));
    }

    let table_addresses = authenticate_frozen_table_account_v1(table)?;
    if snapshots.iter().any(|snapshot| {
        snapshot.lookup_table != table.key.to_string()
            || snapshot.lookup_table_sha256 != sha256_hex(&table.data)
    }) {
        return Err(refusal(
            "interruption snapshots changed the exact frozen union ALT",
        ));
    }
    let payer = parse_key(&planned.fee_payer, "interruption audit fee payer")?;
    let expected_union = canonical_abort_union_addresses_v1(payer, &planned.journals)?;
    if table_addresses != expected_union {
        return Err(refusal(
            "interruption ALT was not the exact canonical union of all three SourceAbort instructions",
        ));
    }
    for snapshot in snapshots {
        authenticate_snapshot_packets_v1(snapshot, payer, table)?;
    }

    let dispatching_packet = dispatching.journals[2]
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Dispatching interruption snapshot omitted terminal packet"))?;
    let submitted_packet = submitted.journals[2]
        .packet
        .as_ref()
        .ok_or_else(|| refusal("Submitted interruption snapshot omitted terminal packet"))?;
    if dispatching_packet != submitted_packet {
        return Err(refusal(
            "Submitted interruption snapshot changed the Dispatching packet or signature",
        ));
    }
    let finalized_prefix_signatures = [0_usize, 1_usize].map(|index| {
        planned.journals[index]
            .finalized
            .as_ref()
            .expect("journal prefix authenticated above")
            .signature
            .clone()
    });
    Ok(InterruptionAuditReportV1 {
        schema: INTERRUPTION_AUDIT_SCHEMA_V1,
        contract_sha256,
        planned_evidence_sha256,
        dispatching_evidence_sha256,
        submitted_evidence_sha256,
        immutable_invocation_sha256,
        lookup_table: table.key.to_string(),
        lookup_table_capture_sha256,
        lookup_table_data_sha256: sha256_hex(&table.data),
        lookup_table_address_sha256: address_list_sha256_v1(&table_addresses),
        lookup_table_address_count: table_addresses.len(),
        finalized_prefix_signatures,
        terminal_packet_signature: dispatching_packet.signature.clone(),
        terminal_packet_sha256: dispatching_packet.packet_sha256.clone(),
        boundaries: contract.boundaries.clone(),
        wallet_key_read_count: 0,
        rpc_call_count: 0,
        external_mutation_count: 0,
        partial_success_authorized: false,
    })
}

fn interruption_immutable_sha256_v1(evidence: &SourceAbortEvidenceV1) -> Result<String> {
    let mut immutable = evidence.clone();
    immutable.journals.clear();
    Ok(sha256_hex(&canonical_json_v1(&immutable)?))
}

fn interruption_active_identity_v1(journal: &SourceAbortJournalV1) -> Result<Vec<u8>> {
    let mut identity = journal.clone();
    identity.phase = JournalPhaseV1::Planned;
    identity.packet = None;
    identity.finalized = None;
    canonical_json_v1(&identity)
}

fn authenticate_snapshot_packets_v1(
    evidence: &SourceAbortEvidenceV1,
    payer: Pubkey,
    table: &dclutch_versioned_message_operator::ObservedAccount,
) -> Result<()> {
    for journal in &evidence.journals {
        if let Some(packet) = &journal.packet {
            Rpc::authenticate_signed_v0_packet(
                journal.operation.label(),
                std::slice::from_ref(&journal.instruction.instruction()?),
                payer,
                table,
                packet,
            )?;
        }
    }
    Ok(())
}

fn canonical_abort_union_addresses_v1(
    payer: Pubkey,
    journals: &[SourceAbortJournalV1],
) -> Result<Vec<Pubkey>> {
    if journals.len() != SourceAbortRecoveryOperationV1::ORDERED.len() {
        return Err(refusal(
            "SourceAbort union ALT requires all three exact journal instructions",
        ));
    }
    let mut addresses = Vec::new();
    for journal in journals {
        let instruction = journal.instruction.instruction()?;
        if instruction.program_id != payer {
            addresses.push(instruction.program_id);
        }
        addresses.extend(
            instruction
                .accounts
                .iter()
                .filter(|account| !account.is_signer && account.pubkey != payer)
                .map(|account| account.pubkey),
        );
    }
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses.is_empty() {
        return Err(refusal("SourceAbort union ALT was empty"));
    }
    Ok(addresses)
}

fn address_list_sha256_v1(addresses: &[Pubkey]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/source-abort/frozen-union-alt-addresses/v1");
    hasher.update(
        u64::try_from(addresses.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for address in addresses {
        hasher.update(address.as_ref());
    }
    crate::plan::hex(&hasher.finalize())
}

fn parse_interruption_audit_arguments_v1(
    arguments: Vec<String>,
) -> Result<InterruptionAuditArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if !matches!(
            argument.as_str(),
            "--contract"
                | "--planned-evidence"
                | "--dispatching-evidence"
                | "--submitted-evidence"
                | "--output"
        ) {
            return Err(refusal(format!(
                "unknown {INTERRUPTION_AUDIT_COMMAND_V1} argument: {argument}"
            )));
        }
        let value = iterator
            .next()
            .ok_or_else(|| refusal(format!("{argument} requires a value")))?;
        if values.insert(argument.clone(), value).is_some() {
            return Err(refusal(format!("{argument} may be supplied only once")));
        }
    }
    let take = |values: &mut BTreeMap<String, String>, flag: &str| {
        values
            .remove(flag)
            .ok_or_else(|| refusal(format!("{flag} is required")))
    };
    let parsed = InterruptionAuditArgumentsV1 {
        contract: absolute(&take(&mut values, "--contract")?, "--contract")?,
        planned_evidence: absolute(
            &take(&mut values, "--planned-evidence")?,
            "--planned-evidence",
        )?,
        dispatching_evidence: absolute(
            &take(&mut values, "--dispatching-evidence")?,
            "--dispatching-evidence",
        )?,
        submitted_evidence: absolute(
            &take(&mut values, "--submitted-evidence")?,
            "--submitted-evidence",
        )?,
        output: absolute(&take(&mut values, "--output")?, "--output")?,
    };
    let paths = [
        &parsed.contract,
        &parsed.planned_evidence,
        &parsed.dispatching_evidence,
        &parsed.submitted_evidence,
        &parsed.output,
    ];
    for (index, left) in paths.iter().enumerate() {
        if paths.iter().skip(index + 1).any(|right| left == right) {
            return Err(refusal(
                "interruption audit input and output paths must be pairwise distinct",
            ));
        }
    }
    Ok(parsed)
}

fn read_canonical_json_v1<T: serde::de::DeserializeOwned + Serialize>(
    path: &Path,
    label: &str,
) -> Result<(Vec<u8>, T)> {
    let source = read_bounded(path, label)?;
    let value: T = serde_json::from_slice(&source)?;
    if canonical_json_v1(&value)? != source {
        return Err(refusal(format!("{label} was not exact canonical JSON")));
    }
    Ok((source, value))
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments_v1(arguments)?;
    let plan_source = read_bounded(&arguments.plan, "successor plan")?;
    let market_source = read_bounded(&arguments.market, "Market input")?;
    let founding_source = read_bounded(&arguments.founding_evidence, "founding evidence")?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let market: MarketRunInput = load_market_input(&market_source)?;
    authenticate_checked_campaign_plan(&plan, &arguments.origin)?;
    let checkpoint = parse_founding_checkpoint_v1(
        &founding_source,
        &arguments,
        sha256_hex(&plan_source),
        sha256_hex(&market_source),
    )?;

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
    authenticate_checked_live_substrate(&mut rpc, &plan)?;
    let table = observe_frozen_table_v1(&mut rpc, arguments.lookup_table)?;

    let mut evidence = load_or_create_evidence_v1(
        &arguments,
        checkpoint,
        &plan_source,
        &market_source,
        &founding_source,
        &genesis_hash,
        &table,
    )?;
    authenticate_evidence_v1(
        &evidence,
        &arguments,
        &plan_source,
        &market_source,
        &founding_source,
        &genesis_hash,
        &table,
    )?;
    authenticate_journal_prefix_v1(&evidence.journals)?;
    if !arguments.execute {
        return stdout_v1(json!({
            "schema": EVIDENCE_SCHEMA_V1,
            "status": "key-free-preflight-complete",
            "evidence": arguments.evidence.display().to_string(),
            "message": "The checked deployment set, expired DCLTPCB2 checkpoint, frozen routing table, and exact public intent are durable. No key was read and no transaction was sent."
        }));
    }

    // This is the key boundary: the immutable invocation above is on disk and
    // fsynced before any secret-bearing path is opened.
    let secrets = load_keypairs_v1(&arguments.keypairs)?;
    let payer = Keypair::new_from_array(
        *secrets
            .get(role::CAMPAIGN_PAYER)
            .ok_or_else(|| refusal("SourceAbort payer secret disappeared"))?,
    );
    let beneficiary = Keypair::new_from_array(
        *secrets
            .get(role::FOUNDING_BENEFICIARY)
            .ok_or_else(|| refusal("SourceAbort beneficiary secret disappeared"))?,
    );
    if payer.pubkey() != arguments.fee_payer || beneficiary.pubkey() != arguments.beneficiary {
        return Err(refusal(
            "SourceAbort key files did not name the durable payer and beneficiary",
        ));
    }
    let forge = KeyForge::persisted(secrets, FOUNDING_REQUIRED_ROLES)?;
    let actors = FoundingActorsV1::new(arguments.founding_founder, arguments.substituted_founder)?;
    if evidence.baseline.is_none() {
        let baseline = capture_source_abort_recovery_baseline_v1(
            &mut rpc,
            &plan,
            &market,
            &payer,
            &forge,
            actors,
            &evidence.founding_checkpoint,
        )?;
        let expected = canonical_json_v1(&evidence)?;
        evidence.baseline = Some(baseline);
        replace_json_from_bytes_v1(&arguments.evidence, &expected, &evidence)?;
    }
    operate_v1(
        &mut rpc,
        &arguments,
        &plan,
        &market,
        &payer,
        &beneficiary,
        &forge,
        actors,
        &table,
        evidence,
    )
}

#[allow(clippy::too_many_arguments)]
fn operate_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    market: &MarketRunInput,
    payer: &Keypair,
    beneficiary: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    table: &dclutch_versioned_message_operator::ObservedAccount,
    mut evidence: SourceAbortEvidenceV1,
) -> Result<()> {
    let baseline = evidence
        .baseline
        .as_ref()
        .ok_or_else(|| refusal("SourceAbort armed evidence omitted its baseline"))?;
    if let Some(active) = evidence.journals.last()
        && active.phase != JournalPhaseV1::Finalized
    {
        return recover_active_v1(
            rpc,
            arguments,
            plan,
            market,
            payer,
            beneficiary,
            forge,
            actors,
            table,
            evidence,
        );
    }
    let planned = plan_source_abort_recovery_v1(
        rpc,
        plan,
        market,
        payer,
        forge,
        actors,
        &evidence.founding_checkpoint,
        baseline,
    )?;
    if planned.phase == SourceAbortRecoveryPhaseV1::Complete {
        if evidence.journals.len() != SourceAbortRecoveryOperationV1::ORDERED.len() {
            return Err(refusal(
                "chain is terminal but the exterior lacks the exact three finalized receipts",
            ));
        }
        return write_completion_v1(arguments, &evidence);
    }
    let operation = planned
        .operation
        .ok_or_else(|| refusal("nonterminal SourceAbort plan omitted operation"))?;
    if evidence.journals.len() != operation.ordinal() {
        return Err(refusal(
            "live SourceAbort phase disagreed with its finalized journal prefix",
        ));
    }
    let instruction = planned
        .instruction
        .as_ref()
        .ok_or_else(|| refusal("nonterminal SourceAbort plan omitted instruction"))?;
    let successor = operation.successor();
    let journal = SourceAbortJournalV1 {
        operation,
        predecessor: planned.phase,
        successor,
        phase: JournalPhaseV1::Planned,
        instruction: DurableInstructionV1::from_instruction(instruction),
        complete_keys: planned
            .complete_keys
            .ok_or_else(|| refusal("SourceAbort plan omitted key census"))?,
        message_bytes: planned
            .message_bytes
            .ok_or_else(|| refusal("SourceAbort plan omitted message census"))?,
        packet_bytes: planned
            .packet_bytes
            .ok_or_else(|| refusal("SourceAbort plan omitted packet census"))?,
        packet: None,
        finalized: None,
    };
    let expected = canonical_json_v1(&evidence)?;
    evidence.journals.push(journal);
    authenticate_journal_prefix_v1(&evidence.journals)?;
    replace_json_from_bytes_v1(&arguments.evidence, &expected, &evidence)?;
    recover_active_v1(
        rpc,
        arguments,
        plan,
        market,
        payer,
        beneficiary,
        forge,
        actors,
        table,
        evidence,
    )
}

#[allow(clippy::too_many_arguments)]
fn recover_active_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    market: &MarketRunInput,
    payer: &Keypair,
    beneficiary: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    table: &dclutch_versioned_message_operator::ObservedAccount,
    mut evidence: SourceAbortEvidenceV1,
) -> Result<()> {
    let index = evidence
        .journals
        .len()
        .checked_sub(1)
        .ok_or_else(|| refusal("SourceAbort recovery omitted active journal"))?;
    let active = evidence.journals[index].clone();
    if should_stop_interruption_v1(
        active.operation,
        active.phase,
        arguments.interruption_stop_after,
    ) {
        return stdout_progress_v1(
            arguments,
            active.operation,
            match active.phase {
                JournalPhaseV1::Planned => "interrupted-after-planned-fsync",
                JournalPhaseV1::Dispatching => "interrupted-after-dispatching-fsync-before-send",
                JournalPhaseV1::Submitted => "interrupted-after-submitted-fsync",
                JournalPhaseV1::Finalized => "finalized",
            },
            "Owned-loopback interruption seam reached. The journal is durable; copy this exact evidence before restarting the same immutable invocation without the stop flag.",
        );
    }
    let instruction = active.instruction.instruction()?;
    match active.phase {
        JournalPhaseV1::Planned => {
            let plan_now = plan_source_abort_recovery_v1(
                rpc,
                plan,
                market,
                payer,
                forge,
                actors,
                &evidence.founding_checkpoint,
                evidence
                    .baseline
                    .as_ref()
                    .ok_or_else(|| refusal("SourceAbort baseline disappeared"))?,
            )?;
            if plan_now.phase != active.predecessor
                || plan_now.operation != Some(active.operation)
                || plan_now
                    .instruction
                    .as_ref()
                    .map(DurableInstructionV1::from_instruction)
                    != Some(active.instruction.clone())
            {
                return Err(refusal(
                    "Planned SourceAbort journal no longer matched finalized predecessor state",
                ));
            }
            let additional = if active.operation == SourceAbortRecoveryOperationV1::Custody {
                vec![beneficiary]
            } else {
                Vec::new()
            };
            let packet = rpc.prepare_signed_v0_packet_with_signers(
                active.operation.label(),
                std::slice::from_ref(&instruction),
                payer,
                &additional,
                table,
            )?;
            replace_evidence_v1(&arguments.evidence, &mut evidence, |mut value| {
                value.journals[index].phase = JournalPhaseV1::Dispatching;
                value.journals[index].packet = Some(packet);
                Ok(value)
            })?;
            recover_active_v1(
                rpc,
                arguments,
                plan,
                market,
                payer,
                beneficiary,
                forge,
                actors,
                table,
                evidence,
            )
        }
        JournalPhaseV1::Dispatching => {
            let packet = active
                .packet
                .as_ref()
                .ok_or_else(|| refusal("Dispatching SourceAbort journal omitted packet"))?;
            let signature = Signature::from_str(&packet.signature)
                .map_err(|error| Error::new(format!("SourceAbort signature: {error}")))?;
            if let Some(finalized) =
                rpc.finalized_signed_packet(active.operation.label(), signature, false)?
            {
                replace_evidence_v1(&arguments.evidence, &mut evidence, |mut value| {
                    value.journals[index].phase = JournalPhaseV1::Submitted;
                    Ok(value)
                })?;
                return finalize_active_v1(
                    rpc, arguments, plan, market, payer, forge, actors, table, evidence, finalized,
                );
            }
            let plan_now = plan_source_abort_recovery_v1(
                rpc,
                plan,
                market,
                payer,
                forge,
                actors,
                &evidence.founding_checkpoint,
                evidence
                    .baseline
                    .as_ref()
                    .ok_or_else(|| refusal("SourceAbort baseline disappeared"))?,
            )?;
            if plan_now.phase != active.predecessor {
                return Err(refusal(
                    "Dispatching packet was absent but finalized predecessor state changed",
                ));
            }
            rpc.submit_signed_v0_packet(
                active.operation.label(),
                std::slice::from_ref(&instruction),
                payer.pubkey(),
                table,
                packet,
            )?;
            replace_evidence_v1(&arguments.evidence, &mut evidence, |mut value| {
                value.journals[index].phase = JournalPhaseV1::Submitted;
                Ok(value)
            })?;
            stdout_progress_v1(
                arguments,
                active.operation,
                "submitted",
                "The exact durable signature was sent once; rerun is poll-only.",
            )
        }
        JournalPhaseV1::Submitted => {
            let packet = active
                .packet
                .as_ref()
                .ok_or_else(|| refusal("Submitted SourceAbort journal omitted packet"))?;
            let signature = Signature::from_str(&packet.signature)
                .map_err(|error| Error::new(format!("SourceAbort signature: {error}")))?;
            let Some(finalized) =
                rpc.finalized_signed_packet(active.operation.label(), signature, false)?
            else {
                return stdout_progress_v1(
                    arguments,
                    active.operation,
                    "submitted",
                    "The exact signature is pending; rerun polls only and never re-signs.",
                );
            };
            finalize_active_v1(
                rpc, arguments, plan, market, payer, forge, actors, table, evidence, finalized,
            )
        }
        JournalPhaseV1::Finalized => Err(refusal(
            "SourceAbort selected a finalized journal as its active tail",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_active_v1(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    market: &MarketRunInput,
    payer: &Keypair,
    forge: &KeyForge,
    actors: FoundingActorsV1,
    table: &dclutch_versioned_message_operator::ObservedAccount,
    mut evidence: SourceAbortEvidenceV1,
    finalized: FinalizedSignedPacketV1,
) -> Result<()> {
    let index = evidence.journals.len() - 1;
    let active = evidence.journals[index].clone();
    let packet = active
        .packet
        .as_ref()
        .ok_or_else(|| refusal("finalized SourceAbort journal omitted packet"))?;
    if sha256_hex(&finalized.packet) != packet.packet_sha256 {
        return Err(refusal(
            "finalized SourceAbort transaction bytes differed from its durable packet",
        ));
    }
    let instruction = active.instruction.instruction()?;
    Rpc::authenticate_signed_v0_packet(
        active.operation.label(),
        std::slice::from_ref(&instruction),
        payer.pubkey(),
        table,
        packet,
    )?;
    let next = plan_source_abort_recovery_v1(
        rpc,
        plan,
        market,
        payer,
        forge,
        actors,
        &evidence.founding_checkpoint,
        evidence
            .baseline
            .as_ref()
            .ok_or_else(|| refusal("SourceAbort baseline disappeared"))?,
    )?;
    if next.phase != active.successor {
        return Err(refusal(
            "finalized SourceAbort packet did not produce its exact successor phase",
        ));
    }
    let receipt = finalized.evidence;
    if receipt.label != active.operation.label()
        || receipt.signature != packet.signature
        || receipt.error.is_some()
        || !receipt.transaction_metadata_available
        || receipt.fee_lamports.is_none()
        || receipt.compute_units_consumed.is_none()
    {
        return Err(refusal(
            "finalized SourceAbort transaction omitted exact fee/CU metadata or changed identity",
        ));
    }
    replace_evidence_v1(&arguments.evidence, &mut evidence, |mut value| {
        value.journals[index].phase = JournalPhaseV1::Finalized;
        value.journals[index].finalized = Some(receipt);
        Ok(value)
    })?;
    if next.phase == SourceAbortRecoveryPhaseV1::Complete {
        write_completion_v1(arguments, &evidence)
    } else {
        stdout_progress_v1(
            arguments,
            active.operation,
            "finalized",
            "One exact SourceAbort mutation finalized; rerun derives the adjacent suffix from onchain phase.",
        )
    }
}

fn parse_founding_checkpoint_v1(
    source: &[u8],
    arguments: &ArgumentsV1,
    plan_sha256: String,
    market_sha256: String,
) -> Result<MarketExecutionCheckpointV1> {
    let value: Value = serde_json::from_slice(source)?;
    if value.get("schema").and_then(Value::as_str) != Some("dclutch-successor-campaign-report-v1")
        || value.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256.as_str())
        || value.get("market_sha256").and_then(Value::as_str) != Some(market_sha256.as_str())
        || value.get("cluster").and_then(Value::as_str) != Some(arguments.origin.label())
        || value.get("rpc_url").and_then(Value::as_str)
            != Some(arguments.origin.redacted_url().as_str())
        || value.get("payer").and_then(Value::as_str)
            != Some(arguments.fee_payer.to_string().as_str())
        || value
            .pointer("/execution/market")
            .is_some_and(|market| !market.is_null())
    {
        return Err(refusal(
            "founding evidence did not bind the exact partial campaign, cluster, plan, Market, and payer",
        ));
    }
    serde_json::from_value(
        value
            .get("foundingCheckpoint")
            .cloned()
            .ok_or_else(|| refusal("founding evidence omitted its DCLTPCB2 checkpoint"))?,
    )
    .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn load_or_create_evidence_v1(
    arguments: &ArgumentsV1,
    checkpoint: MarketExecutionCheckpointV1,
    plan_source: &[u8],
    market_source: &[u8],
    founding_source: &[u8],
    genesis_hash: &str,
    table: &dclutch_versioned_message_operator::ObservedAccount,
) -> Result<SourceAbortEvidenceV1> {
    if arguments.evidence.exists() {
        return read_json_v1(&arguments.evidence, "SourceAbort evidence");
    }
    let evidence = SourceAbortEvidenceV1 {
        schema: EVIDENCE_SCHEMA_V1.into(),
        cluster: arguments.origin.label().into(),
        genesis_hash: genesis_hash.into(),
        rpc_url: arguments.origin.url().into(),
        plan_sha256: sha256_hex(plan_source),
        market_sha256: sha256_hex(market_source),
        founding_evidence_sha256: sha256_hex(founding_source),
        evidence_path: arguments.evidence.display().to_string(),
        completion_path: arguments.completion.display().to_string(),
        lookup_table: arguments.lookup_table.to_string(),
        lookup_table_sha256: sha256_hex(&table.data),
        lookup_table_account: FrozenUnionAltCaptureV1::from_observed(table),
        fee_payer: arguments.fee_payer.to_string(),
        beneficiary: arguments.beneficiary.to_string(),
        founding_founder: arguments.founding_founder.to_string(),
        substituted_founder: arguments.substituted_founder.to_string(),
        founding_checkpoint: checkpoint,
        baseline: None,
        journals: Vec::new(),
    };
    create_json_v1(&arguments.evidence, &evidence, "SourceAbort evidence")?;
    Ok(evidence)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_evidence_v1(
    evidence: &SourceAbortEvidenceV1,
    arguments: &ArgumentsV1,
    plan_source: &[u8],
    market_source: &[u8],
    founding_source: &[u8],
    genesis_hash: &str,
    table: &dclutch_versioned_message_operator::ObservedAccount,
) -> Result<()> {
    if evidence.schema != EVIDENCE_SCHEMA_V1
        || evidence.cluster != arguments.origin.label()
        || evidence.genesis_hash != genesis_hash
        || evidence.rpc_url != arguments.origin.url()
        || evidence.plan_sha256 != sha256_hex(plan_source)
        || evidence.market_sha256 != sha256_hex(market_source)
        || evidence.founding_evidence_sha256 != sha256_hex(founding_source)
        || evidence.evidence_path != arguments.evidence.display().to_string()
        || evidence.completion_path != arguments.completion.display().to_string()
        || evidence.lookup_table != arguments.lookup_table.to_string()
        || evidence.lookup_table_sha256 != sha256_hex(&table.data)
        || evidence.lookup_table_account != FrozenUnionAltCaptureV1::from_observed(table)
        || evidence.fee_payer != arguments.fee_payer.to_string()
        || evidence.beneficiary != arguments.beneficiary.to_string()
        || evidence.founding_founder != arguments.founding_founder.to_string()
        || evidence.substituted_founder != arguments.substituted_founder.to_string()
    {
        return Err(refusal(
            "SourceAbort evidence did not bind the exact invocation or frozen routing table",
        ));
    }
    Ok(())
}

fn authenticate_journal_prefix_v1(journals: &[SourceAbortJournalV1]) -> Result<()> {
    if journals.len() > SourceAbortRecoveryOperationV1::ORDERED.len() {
        return Err(refusal("SourceAbort journal exceeded three mutations"));
    }
    for (index, journal) in journals.iter().enumerate() {
        let operation = SourceAbortRecoveryOperationV1::ORDERED[index];
        let finalized_receipt_changed = journal.phase == JournalPhaseV1::Finalized
            && journal
                .packet
                .as_ref()
                .zip(journal.finalized.as_ref())
                .is_none_or(|(packet, receipt)| {
                    receipt.label != operation.label()
                        || receipt.signature != packet.signature
                        || receipt.error.is_some()
                        || !receipt.transaction_metadata_available
                        || receipt.fee_lamports.is_none()
                        || receipt.compute_units_consumed.is_none()
                });
        if journal.operation != operation
            || journal.predecessor != operation.predecessor()
            || journal.successor != operation.successor()
            || journal.complete_keys != operation.expected_complete_keys()
            || journal.instruction.instruction()?.program_id == Pubkey::default()
            || (index + 1 != journals.len() && journal.phase != JournalPhaseV1::Finalized)
            || (journal.phase == JournalPhaseV1::Planned
                && (journal.packet.is_some() || journal.finalized.is_some()))
            || (matches!(
                journal.phase,
                JournalPhaseV1::Dispatching | JournalPhaseV1::Submitted
            ) && (journal.packet.is_none() || journal.finalized.is_some()))
            || (journal.phase == JournalPhaseV1::Finalized
                && (journal.packet.is_none() || journal.finalized.is_none()))
            || finalized_receipt_changed
        {
            return Err(refusal(
                "SourceAbort journals were not one exact canonical adjacent prefix",
            ));
        }
    }
    Ok(())
}

fn observe_frozen_table_v1(
    rpc: &mut Rpc,
    key: Pubkey,
) -> Result<dclutch_versioned_message_operator::ObservedAccount> {
    let (_, mut values) = rpc.finalized_observed_accounts(&[key], 0)?;
    let value = values
        .pop()
        .ok_or_else(|| refusal("SourceAbort lookup observation omitted its account"))?;
    authenticate_frozen_table_account_v1(&value)?;
    Ok(value)
}

fn authenticate_frozen_table_account_v1(
    value: &dclutch_versioned_message_operator::ObservedAccount,
) -> Result<Vec<Pubkey>> {
    let decoded = AddressLookupTable::deserialize(&value.data)
        .map_err(|_| refusal("SourceAbort lookup table did not decode"))?;
    if value.owner != lookup_table_program::id()
        || value.executable
        || decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= value.observation.slot
        || decoded.addresses.is_empty()
    {
        return Err(refusal(
            "SourceAbort lookup table was not exact, frozen, active, and activated",
        ));
    }
    let mut addresses = decoded.addresses.to_vec();
    let observed_order = addresses.clone();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses != observed_order {
        return Err(refusal(
            "SourceAbort lookup table addresses were not canonical and duplicate-free",
        ));
    }
    Ok(addresses)
}

fn write_completion_v1(arguments: &ArgumentsV1, evidence: &SourceAbortEvidenceV1) -> Result<()> {
    authenticate_journal_prefix_v1(&evidence.journals)?;
    if evidence.journals.len() != 3
        || evidence
            .journals
            .iter()
            .any(|journal| journal.phase != JournalPhaseV1::Finalized)
    {
        return Err(refusal(
            "SourceAbort completion requires three distinct finalized receipts",
        ));
    }
    let baseline = evidence
        .baseline
        .as_ref()
        .ok_or_else(|| refusal("SourceAbort completion omitted conservation baseline"))?;
    let receipt = SourceAbortCompletionV1 {
        schema: COMPLETION_SCHEMA_V1,
        evidence_sha256: sha256_hex(&canonical_json_v1(evidence)?),
        market: &baseline.market,
        lifecycle_rent_credit: &baseline.lifecycle_rent_credit,
        controller_funding_source: &baseline.controller_funding_source,
        principal_atoms: baseline.principal_atoms,
        controller_native_refund_lamports: baseline.controller_native_refund_lamports,
        controller_rent_refund_lamports: baseline.controller_rent_refund_lamports,
        transactions: evidence
            .journals
            .iter()
            .map(|journal| journal.finalized.as_ref().expect("authenticated above"))
            .collect(),
    };
    write_or_authenticate_json_v1(&arguments.completion, &receipt, "SourceAbort completion")?;
    stdout_v1(json!({
        "schema": COMPLETION_SCHEMA_V1,
        "status": "finalized",
        "evidence": arguments.evidence.display().to_string(),
        "completion": arguments.completion.display().to_string(),
        "transactions": evidence.journals.iter().map(|journal| journal.operation.label()).collect::<Vec<_>>(),
        "message": "The real Custody source abort and both canonical controller-ledger cleanup transactions finalized; exact principal and rent conservation were reauthenticated."
    }))
}

fn replace_evidence_v1(
    path: &Path,
    evidence: &mut SourceAbortEvidenceV1,
    transition: impl FnOnce(SourceAbortEvidenceV1) -> Result<SourceAbortEvidenceV1>,
) -> Result<()> {
    let expected = canonical_json_v1(evidence)?;
    let next = transition(evidence.clone())?;
    authenticate_journal_prefix_v1(&next.journals)?;
    replace_json_from_bytes_v1(path, &expected, &next)?;
    *evidence = next;
    Ok(())
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(refusal("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| refusal(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | "--acknowledge-devnet-genesis"
                | "--plan"
                | "--market"
                | "--founding-evidence"
                | "--evidence"
                | "--completion"
                | "--lookup-table"
                | "--fee-payer"
                | "--beneficiary"
                | "--founding-founder"
                | "--substituted-founder"
                | "--interruption-stop-after"
        ) && !argument.starts_with("--keypair-")
        {
            return Err(refusal(format!(
                "unknown {COMMAND_V1} argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(refusal(format!("{argument} may be supplied only once")));
        }
    }
    let take = |values: &mut BTreeMap<String, String>, flag: &str| {
        values
            .remove(flag)
            .ok_or_else(|| refusal(format!("{flag} is required")))
    };
    let rpc_url = take(&mut values, "--rpc-url")?;
    let acknowledgment = values.remove("--acknowledge-devnet-genesis");
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    let interruption_stop_after = values
        .remove("--interruption-stop-after")
        .map(|value| match value.as_str() {
            "planned" => Ok(JournalPhaseV1::Planned),
            "dispatching" => Ok(JournalPhaseV1::Dispatching),
            "submitted" => Ok(JournalPhaseV1::Submitted),
            _ => Err(refusal(
                "--interruption-stop-after must be planned, dispatching, or submitted",
            )),
        })
        .transpose()?;
    if interruption_stop_after.is_some() {
        ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
        if !execute {
            return Err(refusal(
                "--interruption-stop-after requires --execute against owned loopback",
            ));
        }
    }
    let mut keypairs = BTreeMap::new();
    for role in FOUNDING_REQUIRED_ROLES {
        let flag = format!("--keypair-{role}");
        keypairs.insert(
            (*role).to_owned(),
            absolute(&take(&mut values, &flag)?, &flag)?,
        );
    }
    for role in [
        crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1,
        crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1,
    ] {
        let flag = format!("--keypair-{role}");
        if let Some(value) = values.remove(&flag) {
            keypairs.insert(role.to_owned(), absolute(&value, &flag)?);
        }
    }
    if keypairs.contains_key(crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1)
        != keypairs.contains_key(crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1)
    {
        return Err(refusal(
            "local participant fixture owner and source keypairs must be supplied together",
        ));
    }
    let fee_payer = parse_key(&take(&mut values, "--fee-payer")?, "--fee-payer")?;
    let beneficiary = parse_key(&take(&mut values, "--beneficiary")?, "--beneficiary")?;
    let founding_founder = parse_key(
        &take(&mut values, "--founding-founder")?,
        "--founding-founder",
    )?;
    let substituted_founder = parse_key(
        &take(&mut values, "--substituted-founder")?,
        "--substituted-founder",
    )?;
    FoundingActorsV1::new(founding_founder, substituted_founder)?;
    let parsed = ArgumentsV1 {
        origin,
        plan: absolute(&take(&mut values, "--plan")?, "--plan")?,
        market: absolute(&take(&mut values, "--market")?, "--market")?,
        founding_evidence: absolute(
            &take(&mut values, "--founding-evidence")?,
            "--founding-evidence",
        )?,
        evidence: absolute(&take(&mut values, "--evidence")?, "--evidence")?,
        completion: absolute(&take(&mut values, "--completion")?, "--completion")?,
        lookup_table: parse_key(&take(&mut values, "--lookup-table")?, "--lookup-table")?,
        fee_payer,
        beneficiary,
        founding_founder,
        substituted_founder,
        keypairs,
        interruption_stop_after,
        execute,
    };
    if let Some((flag, _)) = values.first_key_value() {
        return Err(refusal(format!("unsupported SourceAbort argument {flag}")));
    }
    Ok(parsed)
}

impl SourceAbortRecoveryOperationV1 {
    const fn ordinal(self) -> usize {
        match self {
            Self::Custody => 0,
            Self::ControllerFirst => 1,
            Self::ControllerTerminal => 2,
        }
    }

    const fn predecessor(self) -> SourceAbortRecoveryPhaseV1 {
        match self {
            Self::Custody => SourceAbortRecoveryPhaseV1::CustodyStaged,
            Self::ControllerFirst => SourceAbortRecoveryPhaseV1::CustodyAborted,
            Self::ControllerTerminal => SourceAbortRecoveryPhaseV1::CustodyFirstLedgerClosed,
        }
    }

    const fn successor(self) -> SourceAbortRecoveryPhaseV1 {
        match self {
            Self::Custody => SourceAbortRecoveryPhaseV1::CustodyAborted,
            Self::ControllerFirst => SourceAbortRecoveryPhaseV1::CustodyFirstLedgerClosed,
            Self::ControllerTerminal => SourceAbortRecoveryPhaseV1::Complete,
        }
    }
}

fn load_keypairs_v1(paths: &BTreeMap<String, PathBuf>) -> Result<BTreeMap<String, [u8; 32]>> {
    let mut secrets = BTreeMap::new();
    let mut public = Vec::new();
    for (role, path) in paths {
        let secret = read_keypair_file(path, &format!("SourceAbort {role}"))?;
        let key = Keypair::new_from_array(secret).pubkey();
        if public.contains(&key) {
            return Err(refusal("SourceAbort keypair roles reused one public key"));
        }
        public.push(key);
        secrets.insert(role.clone(), secret);
    }
    for role in FOUNDING_REQUIRED_ROLES {
        if !secrets.contains_key(*role) {
            return Err(refusal(format!("SourceAbort omitted --keypair-{role}")));
        }
    }
    Ok(secrets)
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|error| Error::new(format!("{label}: {error}")))
}

fn absolute(value: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(refusal(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn read_bounded(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .map_err(|error| Error::new(format!("read {label} {}: {error}", path.display())))?;
    if metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES_V1 {
        return Err(refusal(format!(
            "{label} is outside the 1..={MAX_INPUT_BYTES_V1} byte bound"
        )));
    }
    fs::read(path).map_err(Into::into)
}

fn read_json_v1<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    serde_json::from_slice(&read_bounded(path, label)?).map_err(Into::into)
}

fn canonical_json_v1<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
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

fn replace_json_from_bytes_v1<T: Serialize>(path: &Path, expected: &[u8], value: &T) -> Result<()> {
    if fs::read(path)? != expected {
        return Err(refusal(
            "SourceAbort evidence changed between authentication and transition",
        ));
    }
    let bytes = canonical_json_v1(value)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
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

fn sync_parent_v1(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("durable SourceAbort output omitted parent directory"))?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::plan::hex(&Sha256::digest(bytes))
}

fn stdout_progress_v1(
    arguments: &ArgumentsV1,
    operation: SourceAbortRecoveryOperationV1,
    status: &str,
    message: &str,
) -> Result<()> {
    stdout_v1(json!({
        "schema": EVIDENCE_SCHEMA_V1,
        "status": status,
        "operation": operation.label(),
        "evidence": arguments.evidence.display().to_string(),
        "message": message,
    }))
}

fn should_stop_interruption_v1(
    operation: SourceAbortRecoveryOperationV1,
    phase: JournalPhaseV1,
    requested: Option<JournalPhaseV1>,
) -> bool {
    operation == SourceAbortRecoveryOperationV1::ControllerTerminal
        && requested == Some(phase)
        && phase != JournalPhaseV1::Finalized
}

fn stdout_v1(value: Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&serde_json::to_vec_pretty(&value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("SourceAbort REFUSED: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    use solana_address_lookup_table_interface::state::{AddressLookupTable, LookupTableMeta};
    use solana_program::instruction::AccountMeta;
    use solana_sdk::{hash::Hash, transaction::VersionedTransaction};

    fn instruction() -> DurableInstructionV1 {
        DurableInstructionV1::from_instruction(&Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![solana_program::instruction::AccountMeta::new(
                Pubkey::new_unique(),
                false,
            )],
            data: b"DCLTPCA1".to_vec(),
        })
    }

    fn journal(
        operation: SourceAbortRecoveryOperationV1,
        phase: JournalPhaseV1,
    ) -> SourceAbortJournalV1 {
        SourceAbortJournalV1 {
            operation,
            predecessor: operation.predecessor(),
            successor: operation.successor(),
            phase,
            instruction: instruction(),
            complete_keys: operation.expected_complete_keys(),
            message_bytes: 1,
            packet_bytes: 1,
            packet: if phase == JournalPhaseV1::Planned {
                None
            } else {
                Some(SignedVersionedPacketV1 {
                    signature: Signature::default().to_string(),
                    packet_base64: BASE64.encode([]),
                    packet_sha256: sha256_hex(&[]),
                    last_valid_block_height: 1,
                })
            },
            finalized: None,
        }
    }

    #[test]
    fn source_abort_journal_admits_only_one_adjacent_prefix() {
        let mut first = journal(
            SourceAbortRecoveryOperationV1::Custody,
            JournalPhaseV1::Finalized,
        );
        first.finalized = Some(TransactionEvidence {
            label: SourceAbortRecoveryOperationV1::Custody.label().into(),
            signature: Signature::default().to_string(),
            slot: 1,
            transaction_metadata_available: true,
            fee_lamports: Some(0),
            fee_only_balance_change: Some(false),
            compute_units_consumed: Some(1),
            error: None,
            logs: Vec::new(),
        });
        let second = journal(
            SourceAbortRecoveryOperationV1::ControllerFirst,
            JournalPhaseV1::Planned,
        );
        assert!(authenticate_journal_prefix_v1(&[first.clone(), second]).is_ok());
        assert!(authenticate_journal_prefix_v1(&[first.clone(), first]).is_err());
        assert!(
            authenticate_journal_prefix_v1(&[
                journal(
                    SourceAbortRecoveryOperationV1::Custody,
                    JournalPhaseV1::Dispatching,
                ),
                journal(
                    SourceAbortRecoveryOperationV1::ControllerFirst,
                    JournalPhaseV1::Planned,
                ),
            ])
            .is_err()
        );
    }

    #[test]
    fn source_abort_operation_labels_and_phases_are_frozen() {
        assert_eq!(
            SourceAbortRecoveryOperationV1::ORDERED.map(SourceAbortRecoveryOperationV1::label),
            [
                "source-abort-custody-v1",
                "source-abort-controller-first-v1",
                "source-abort-controller-terminal-v1",
            ]
        );
        assert_eq!(
            SourceAbortRecoveryOperationV1::ORDERED
                .map(SourceAbortRecoveryOperationV1::expected_complete_keys),
            [33, 19, 19]
        );
        assert_eq!(
            SourceAbortRecoveryOperationV1::ControllerTerminal.successor(),
            SourceAbortRecoveryPhaseV1::Complete
        );
    }

    #[test]
    fn source_abort_help_has_no_literal_patch_markers() {
        for text in [usage(), interruption_audit_usage()] {
            assert!(!text.contains("\n+"));
        }
        assert!(usage().contains("     --rpc-url RPC"));
        assert!(interruption_audit_usage().contains("     --dispatching-evidence ABSOLUTE_JSON"));
    }

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn abort_instruction(
        operation: SourceAbortRecoveryOperationV1,
        beneficiary: Pubkey,
    ) -> Instruction {
        let byte = u8::try_from(operation.ordinal()).expect("ordinal") + 10;
        let mut accounts = vec![AccountMeta::new(key(byte + 20), false)];
        if operation == SourceAbortRecoveryOperationV1::Custody {
            accounts.push(AccountMeta::new_readonly(beneficiary, true));
        }
        Instruction {
            program_id: key(byte),
            accounts,
            data: operation.label().as_bytes().to_vec(),
        }
    }

    fn frozen_union_table(
        payer: Pubkey,
        journals: &[SourceAbortJournalV1],
    ) -> dclutch_versioned_message_operator::ObservedAccount {
        let addresses = canonical_abort_union_addresses_v1(payer, journals).expect("union");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                deactivation_slot: u64::MAX,
                last_extended_slot: 40,
                last_extended_slot_start_index: 0,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        dclutch_versioned_message_operator::ObservedAccount {
            observation: dclutch_versioned_message_operator::Observation {
                slot: 41,
                unix_timestamp: 1_800_000_000,
                finality: dclutch_versioned_message_operator::Finality::Finalized,
            },
            key: key(90),
            owner: lookup_table_program::id(),
            lamports: 9_999,
            executable: false,
            data: table.serialize_for_tests().expect("ALT bytes"),
        }
    }

    fn signed_packet(
        operation: SourceAbortRecoveryOperationV1,
        instruction: &Instruction,
        payer: &Keypair,
        beneficiary: &Keypair,
        table: &dclutch_versioned_message_operator::ObservedAccount,
    ) -> SignedVersionedPacketV1 {
        let bounded = crate::rpc::bounded_instructions(std::slice::from_ref(instruction), None)
            .expect("bounded instruction");
        let routed = dclutch_versioned_message_operator::compile_v0_message(
            payer.pubkey(),
            &bounded,
            Hash::new_from_array([operation.ordinal() as u8 + 50; 32]),
            table.observation,
            std::slice::from_ref(table),
        )
        .expect("compiled packet");
        let signers = if operation == SourceAbortRecoveryOperationV1::Custody {
            vec![payer, beneficiary]
        } else {
            vec![payer]
        };
        let transaction =
            VersionedTransaction::try_new(routed.message, &signers).expect("signed packet");
        let packet = bincode::serialize(&transaction).expect("packet bytes");
        SignedVersionedPacketV1 {
            signature: transaction.signatures[0].to_string(),
            packet_base64: BASE64.encode(&packet),
            packet_sha256: sha256_hex(&packet),
            last_valid_block_height: 500,
        }
    }

    fn finalized_receipt(
        operation: SourceAbortRecoveryOperationV1,
        packet: &SignedVersionedPacketV1,
    ) -> TransactionEvidence {
        TransactionEvidence {
            label: operation.label().into(),
            signature: packet.signature.clone(),
            slot: 100 + operation.ordinal() as u64,
            transaction_metadata_available: true,
            fee_lamports: Some(5_000),
            fee_only_balance_change: Some(false),
            compute_units_consumed: Some(100_000),
            error: None,
            logs: Vec::new(),
        }
    }

    fn interruption_fixture() -> (
        InterruptionContractV1,
        SourceAbortEvidenceV1,
        SourceAbortEvidenceV1,
        SourceAbortEvidenceV1,
        dclutch_versioned_message_operator::ObservedAccount,
    ) {
        let payer = Keypair::new_from_array([1; 32]);
        let beneficiary = Keypair::new_from_array([2; 32]);
        let instructions = SourceAbortRecoveryOperationV1::ORDERED
            .map(|operation| abort_instruction(operation, beneficiary.pubkey()));
        let mut journals = SourceAbortRecoveryOperationV1::ORDERED
            .into_iter()
            .zip(instructions.iter())
            .map(|(operation, instruction)| SourceAbortJournalV1 {
                operation,
                predecessor: operation.predecessor(),
                successor: operation.successor(),
                phase: JournalPhaseV1::Planned,
                instruction: DurableInstructionV1::from_instruction(instruction),
                complete_keys: operation.expected_complete_keys(),
                message_bytes: 200,
                packet_bytes: 400,
                packet: None,
                finalized: None,
            })
            .collect::<Vec<_>>();
        let table = frozen_union_table(payer.pubkey(), &journals);
        for (index, journal) in journals.iter_mut().enumerate().take(2) {
            let operation = journal.operation;
            let packet = signed_packet(
                operation,
                &instructions[index],
                &payer,
                &beneficiary,
                &table,
            );
            journal.phase = JournalPhaseV1::Finalized;
            journal.finalized = Some(finalized_receipt(operation, &packet));
            journal.packet = Some(packet);
        }
        let checkpoint = MarketExecutionCheckpointV1 {
            schema: crate::market::DCLTPCB2_CHECKPOINT_SCHEMA_V1.into(),
            market: key(70).to_string(),
            founding_custody_context: key(71).to_string(),
            direct_selected_manifest_entry_index: 1,
            direct_capability_root: key(72).to_string(),
            direct_trading_funding_ledger: key(73).to_string(),
            expiry_slot: 80,
            found_record: key(74).to_string(),
            lock_record: key(75).to_string(),
            local_participant_fixture_liquidity: None,
            accounts: BTreeMap::new(),
            completed: vec!["DCLTPCB2 staged".into()],
        };
        let baseline = SourceAbortRecoveryBaselineV1 {
            market: checkpoint.market.clone(),
            controller_funding_checkpoint: key(76).to_string(),
            funding_ledgers: vec![key(77).to_string(), key(78).to_string()],
            destination: key(79).to_string(),
            destination_before_atoms: 1,
            principal_atoms: 1_000_000_000,
            lifecycle_rent_credit: key(80).to_string(),
            lifecycle_rent_credit_before_lamports: 2,
            controller_funding_source: key(81).to_string(),
            controller_funding_source_before_lamports: 3,
            controller_rent_refund_lamports: 4,
            controller_native_refund_lamports: 5,
            beneficiary: beneficiary.pubkey().to_string(),
            expiry_slot: checkpoint.expiry_slot,
        };
        let planned = SourceAbortEvidenceV1 {
            schema: EVIDENCE_SCHEMA_V1.into(),
            cluster: "owned-loopback".into(),
            genesis_hash: "fixture-genesis".into(),
            rpc_url: "http://127.0.0.1:8899".into(),
            plan_sha256: "11".repeat(32),
            market_sha256: "22".repeat(32),
            founding_evidence_sha256: "33".repeat(32),
            evidence_path: "/tmp/source-abort.json".into(),
            completion_path: "/tmp/source-abort-completion.json".into(),
            lookup_table: table.key.to_string(),
            lookup_table_sha256: sha256_hex(&table.data),
            lookup_table_account: FrozenUnionAltCaptureV1::from_observed(&table),
            fee_payer: payer.pubkey().to_string(),
            beneficiary: beneficiary.pubkey().to_string(),
            founding_founder: key(82).to_string(),
            substituted_founder: key(83).to_string(),
            founding_checkpoint: checkpoint,
            baseline: Some(baseline),
            journals,
        };
        let terminal_packet = signed_packet(
            SourceAbortRecoveryOperationV1::ControllerTerminal,
            &instructions[2],
            &payer,
            &beneficiary,
            &table,
        );
        let mut dispatching = planned.clone();
        dispatching.journals[2].phase = JournalPhaseV1::Dispatching;
        dispatching.journals[2].packet = Some(terminal_packet.clone());
        let mut submitted = dispatching.clone();
        submitted.journals[2].phase = JournalPhaseV1::Submitted;
        (
            expected_interruption_contract_v1(),
            planned,
            dispatching,
            submitted,
            table,
        )
    }

    #[test]
    fn source_abort_interruption_contract_fixture_is_canonical_and_frozen() {
        let source =
            include_bytes!("../../../../../fixtures/source-abort/interruption-contract-v1.json");
        let contract: InterruptionContractV1 =
            serde_json::from_slice(source).expect("contract fixture");
        assert_eq!(contract, expected_interruption_contract_v1());
        assert_eq!(source.as_slice(), canonical_json_v1(&contract).unwrap());
    }

    #[test]
    fn source_abort_interruption_audit_binds_three_boundaries_packet_and_union_alt() {
        let (contract, planned, dispatching, submitted, table) = interruption_fixture();
        let report = authenticate_interruption_bundle_v1(
            &contract,
            &planned,
            &dispatching,
            &submitted,
            &table,
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "e".into(),
        )
        .expect("static interruption audit");
        assert_eq!(report.wallet_key_read_count, 0);
        assert_eq!(report.rpc_call_count, 0);
        assert_eq!(report.external_mutation_count, 0);
        assert!(!report.partial_success_authorized);
        assert_eq!(report.lookup_table_address_count, 6);
        assert_eq!(report.boundaries, contract.boundaries);
    }

    #[test]
    fn source_abort_interruption_audit_refuses_changed_boundary_receipt_packet_or_alt() {
        let (contract, planned, dispatching, submitted, table) = interruption_fixture();
        let audit =
            |contract: &InterruptionContractV1,
             planned: &SourceAbortEvidenceV1,
             dispatching: &SourceAbortEvidenceV1,
             submitted: &SourceAbortEvidenceV1,
             table: &dclutch_versioned_message_operator::ObservedAccount| {
                authenticate_interruption_bundle_v1(
                    contract,
                    planned,
                    dispatching,
                    submitted,
                    table,
                    "a".into(),
                    "b".into(),
                    "c".into(),
                    "d".into(),
                    "e".into(),
                )
            };

        let mut changed_contract = contract.clone();
        changed_contract.partial_success_authorized = true;
        assert!(
            audit(
                &changed_contract,
                &planned,
                &dispatching,
                &submitted,
                &table
            )
            .is_err()
        );

        let mut changed_receipt = planned.clone();
        changed_receipt.journals[0]
            .finalized
            .as_mut()
            .unwrap()
            .signature = Signature::default().to_string();
        assert!(
            audit(
                &contract,
                &changed_receipt,
                &dispatching,
                &submitted,
                &table
            )
            .is_err()
        );

        let mut changed_submitted = submitted.clone();
        changed_submitted.journals[2]
            .packet
            .as_mut()
            .unwrap()
            .last_valid_block_height += 1;
        assert!(
            audit(
                &contract,
                &planned,
                &dispatching,
                &changed_submitted,
                &table
            )
            .is_err()
        );

        let decoded = AddressLookupTable::deserialize(&table.data).expect("ALT");
        let mut changed_table = table.clone();
        changed_table.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(decoded.addresses[..decoded.addresses.len() - 1].to_vec()),
        }
        .serialize_for_tests()
        .expect("changed ALT bytes");
        assert!(
            audit(
                &contract,
                &planned,
                &dispatching,
                &submitted,
                &changed_table
            )
            .is_err()
        );
    }

    #[test]
    fn source_abort_interruption_stops_only_at_the_terminal_durable_boundary() {
        for phase in [
            JournalPhaseV1::Planned,
            JournalPhaseV1::Dispatching,
            JournalPhaseV1::Submitted,
        ] {
            assert!(should_stop_interruption_v1(
                SourceAbortRecoveryOperationV1::ControllerTerminal,
                phase,
                Some(phase),
            ));
            assert!(!should_stop_interruption_v1(
                SourceAbortRecoveryOperationV1::ControllerFirst,
                phase,
                Some(phase),
            ));
        }
        assert!(!should_stop_interruption_v1(
            SourceAbortRecoveryOperationV1::ControllerTerminal,
            JournalPhaseV1::Finalized,
            Some(JournalPhaseV1::Finalized),
        ));
    }
}
