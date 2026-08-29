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
    cluster::ClusterOriginV1,
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
const EVIDENCE_SCHEMA_V1: &str = "dclutch-source-abort-exterior-evidence-v1";
const COMPLETION_SCHEMA_V1: &str = "dclutch-source-abort-completion-v1";
const MAX_INPUT_BYTES_V1: u64 = 16 * 1024 * 1024;

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap source-abort-v1 \\\n+     --rpc-url RPC [--acknowledge-devnet-genesis DEVNET_GENESIS] \\\n+     --plan ABSOLUTE_JSON --market ABSOLUTE_JSON --founding-evidence ABSOLUTE_JSON \\\n+     --evidence ABSOLUTE_NEW_OR_EXISTING_JSON --completion ABSOLUTE_JSON \\\n+     --lookup-table FROZEN_PUBKEY --fee-payer PUBKEY --beneficiary PUBKEY \\\n+     --founding-founder PUBKEY --substituted-founder PUBKEY \\\n+     --keypair-campaign-payer ABSOLUTE_KEYPAIR \\\n+     --keypair-collateral-mint ABSOLUTE_KEYPAIR \\\n+     --keypair-collateral-wallet ABSOLUTE_KEYPAIR \\\n+     --keypair-founding-beneficiary ABSOLUTE_KEYPAIR \\\n+     --keypair-founding-projection-witness ABSOLUTE_KEYPAIR \\\n+     --keypair-founding-source-funder ABSOLUTE_KEYPAIR [--execute]\n\nWithout --execute, this command authenticates the checked deployment set, exact expired DCLTPCB2 checkpoint, and frozen routing table, then fsyncs a key-free intent. Execute reads the six role files only after that boundary and advances at most the exact crash-resumable DCLTPCA1 -> DCLTCF1A -> DCLTCF2A suffix. Rerun until finalized. It refuses an Open success report, pre-expiry abort, mainnet, changed packet, changed phase, missing receipt, or incomplete conservation."
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
    Ok(value)
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
}
