//! Exterior, restart-safe execution of one wallet User Position admission.
//!
//! The transaction body is owned exclusively by
//! `dclutch_operator::user_position_admission_v1`. This module supplies the
//! exterior guarantees that cannot live in an unsigned operator: one bounded
//! finalized devnet observation, durable intent before key access, exact fee
//! accounting, finalized-only submission evidence, and hostile poststate.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionInputV2, LiabilityBasisPositionViewV2,
        encode_liability_basis_position_into_v2, liability_basis_vector_width_v2,
    },
    protocol_position_v2::ProtocolPositionAdmissionV2,
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    user_position_admission_v1::{
        UserPositionAdmissionPlanV1, UserPositionAdmissionSnapshotV1,
        plan_user_position_admission_v1,
    },
};
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_versioned_message_operator::compile_v0_message_with_optional_tables;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    hash::Hash,
    message::VersionedMessage,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::VersionedTransaction,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH},
    model::SuccessorPlan,
    plan::{hex, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
};

const REPORT_SCHEMA_V1: &str = "dclutch-devnet-user-position-admission-execution-v1";
const FINALITY_WAIT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    campaign_evidence: PathBuf,
    position_owner: Pubkey,
    position_owner_keypair: PathBuf,
    fee_payer: Pubkey,
    fee_payer_keypair: PathBuf,
    minimum_finalized_slot: u64,
    output: PathBuf,
    execute: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
    SignedNotSubmitted,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountStateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    rent_epoch: u64,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionEvidenceV1 {
    program_id: String,
    accounts: Vec<InstructionAccountV1>,
    data_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentV1 {
    plan_sha256: String,
    campaign_evidence_sha256: String,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    minimum_finalized_slot: u64,
    position_owner: String,
    fee_payer: String,
    claims_market: String,
    position: String,
    admission: String,
    founding_trading_custody_replay: String,
    position_rent_principal_lamports: u64,
    admission_rent_principal_lamports: u64,
    position_top_up_lamports: u64,
    admission_top_up_lamports: u64,
    transaction_fee_lamports: u64,
    total_owner_debit_lamports: u64,
    total_fee_payer_debit_lamports: u64,
    recent_blockhash: String,
    last_valid_block_height: u64,
    wire_bytes: usize,
    message_base64: String,
    message_sha256: String,
    claims_request_sha256: String,
    expected_receipt_producer: String,
    expected_receipt_base64: String,
    expected_position_base64: String,
    expected_admission_base64: String,
    instructions: Vec<InstructionEvidenceV1>,
    prestate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    return_data_producer: String,
    return_data_sha256: String,
    poststate: BTreeMap<String, AccountStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    authorized_mutation: bool,
    phase: PhaseV1,
    intent_sha256: String,
    intent: IntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<FinalizedEvidenceV1>,
}

#[derive(Clone, Copy)]
struct CoordinatesV1 {
    claims_market: Pubkey,
    core_market: Pubkey,
    rent_credit: Pubkey,
    trading_replay: Pubkey,
    product: Pubkey,
    result_domain: Pubkey,
    portfolio: Pubkey,
    linked_basis: Pubkey,
}

#[derive(Clone, Copy)]
struct RecordCoordinatesV1 {
    raw: Pubkey,
    staging: Pubkey,
}

#[derive(Clone)]
struct SnapshotBundleV1 {
    operator: UserPositionAdmissionSnapshotV1,
    replay: ObservedAccount,
    fee_payer: ObservedAccount,
    states: BTreeMap<String, AccountStateV1>,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    if arguments.origin.label() != "devnet" {
        return Err(Error::new(
            "User Position admission is devnet-only; loopback rehearsal uses the ProgramTest campaign",
        ));
    }
    let plan_source = fs::read(&arguments.plan)?;
    let plan_sha256 = sha256_hex(&plan_source);
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence_source = fs::read(&arguments.campaign_evidence)?;
    let evidence_sha256 = sha256_hex(&evidence_source);
    let evidence: Value = serde_json::from_slice(&evidence_source)?;
    authenticate_plan_evidence(&plan_sha256, &evidence)?;
    let coordinates = evidence_coordinates(&evidence)?;

    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    authenticate_genesis_again(&mut rpc, &arguments.origin)?;

    if arguments.output.exists() {
        let mut report: ReportV1 = serde_json::from_slice(&fs::read(&arguments.output)?)?;
        authenticate_existing_report(&report, &arguments, &plan_sha256, &evidence_sha256)?;
        if arguments.execute && !report.authorized_mutation {
            // A read-only planning invocation may be resumed later under a
            // fresh explicit --execute authorization. Record that expansion
            // durably before the first key-file read.
            report.authorized_mutation = true;
            write_report_atomically(&arguments.output, &report, false)?;
        }
        resume(&mut rpc, &arguments, &mut report)?;
        return print_report(&report);
    }

    let snapshot = acquire_snapshot(&mut rpc, &arguments, &plan, coordinates, &evidence)?;
    let unsigned = plan_user_position_admission_v1(&snapshot.operator)
        .map_err(|error| Error::new(format!("User Position admission plan refused: {error:?}")))?;
    let mut report = build_report(
        &mut rpc,
        &arguments,
        &plan_sha256,
        &evidence_sha256,
        &snapshot,
        &unsigned,
    )?;
    write_report_atomically(&arguments.output, &report, true)?;
    if arguments.execute {
        resume(&mut rpc, &arguments, &mut report)?;
    }
    print_report(&report)
}

fn build_report(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan_sha256: &str,
    evidence_sha256: &str,
    snapshot: &SnapshotBundleV1,
    unsigned: &UserPositionAdmissionPlanV1,
) -> Result<ReportV1> {
    if unsigned.required_signer != arguments.position_owner {
        return Err(Error::new("operator selected a substituted Position owner"));
    }
    if snapshot.fee_payer.owner != system_program::ID
        || snapshot.fee_payer.executable
        || !snapshot.fee_payer.data.is_empty()
    {
        return Err(Error::new(
            "fee payer must be one existing System-owned, nonexecutable, data-empty wallet",
        ));
    }
    let (recent_blockhash, last_valid_block_height) = latest_blockhash(rpc)?;
    let compiled = compile_v0_message_with_optional_tables(
        arguments.fee_payer,
        &unsigned.instructions,
        recent_blockhash,
        unsigned.observation,
        &[],
    )
    .map_err(|error| Error::new(format!("admission message compilation: {error:?}")))?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let transaction_fee_lamports = fee_for_message(rpc, &message_base64)?;
    let top_ups = unsigned
        .position_top_up_lamports
        .checked_add(unsigned.admission_top_up_lamports)
        .ok_or_else(|| Error::new("admission rent top-up overflow"))?;
    let (total_owner_debit_lamports, total_fee_payer_debit_lamports) =
        if arguments.position_owner == arguments.fee_payer {
            let total = top_ups
                .checked_add(transaction_fee_lamports)
                .ok_or_else(|| Error::new("combined admission debit overflow"))?;
            (total, total)
        } else {
            (top_ups, transaction_fee_lamports)
        };
    if snapshot.operator.owner.lamports < total_owner_debit_lamports {
        return Err(Error::new(format!(
            "Position owner has {} lamports but exact admission debit is {} (rent top-ups {}, fee {})",
            snapshot.operator.owner.lamports,
            total_owner_debit_lamports,
            top_ups,
            if arguments.position_owner == arguments.fee_payer {
                transaction_fee_lamports
            } else {
                0
            }
        )));
    }
    if arguments.position_owner != arguments.fee_payer
        && snapshot.fee_payer.lamports < total_fee_payer_debit_lamports
    {
        return Err(Error::new(format!(
            "fee payer has {} lamports but exact transaction fee is {}",
            snapshot.fee_payer.lamports, total_fee_payer_debit_lamports
        )));
    }

    let claims_market = LiabilityBasisMarketViewV2::decode(&snapshot.operator.claims_market.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position_width = liability_basis_vector_width_v2(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        claims_market.claim_count,
    )
    .map_err(|error| Error::new(format!("Claims Position width: {error:?}")))?;
    let mut expected_position = vec![0; position_width];
    let zero_balances = vec![
        0;
        usize::try_from(claims_market.claim_count).map_err(|_| Error::new(
            "Claims outcome width does not fit host usize"
        ))?
    ];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 0,
            market_account: unsigned.claims_request.market,
            owner: arguments.position_owner.to_bytes(),
            basis_id: claims_market.basis_id,
        },
        &zero_balances,
        &mut expected_position,
    )
    .map_err(|error| Error::new(format!("predict Claims Position: {error:?}")))?;
    let expected_admission =
        ProtocolPositionAdmissionV2::decode_receipt(&unsigned.expected_receipt_body)
            .and_then(ProtocolPositionAdmissionV2::to_state_bytes)
            .map_err(|error| Error::new(format!("predict Claims admission state: {error:?}")))?;

    let instructions = unsigned
        .instructions
        .iter()
        .map(|instruction| InstructionEvidenceV1 {
            program_id: instruction.program_id.to_string(),
            accounts: instruction
                .accounts
                .iter()
                .map(|account| InstructionAccountV1 {
                    address: account.pubkey.to_string(),
                    signer: account.is_signer,
                    writable: account.is_writable,
                })
                .collect(),
            data_base64: BASE64.encode(&instruction.data),
        })
        .collect();
    let intent = IntentV1 {
        plan_sha256: plan_sha256.into(),
        campaign_evidence_sha256: evidence_sha256.into(),
        observation_slot: unsigned.observation.slot,
        observation_unix_timestamp: unsigned.observation.unix_timestamp,
        minimum_finalized_slot: arguments.minimum_finalized_slot,
        position_owner: arguments.position_owner.to_string(),
        fee_payer: arguments.fee_payer.to_string(),
        claims_market: snapshot.operator.claims_market.key.to_string(),
        position: unsigned.position.to_string(),
        admission: unsigned.admission.to_string(),
        founding_trading_custody_replay: snapshot.replay.key.to_string(),
        position_rent_principal_lamports: unsigned.position_rent_principal,
        admission_rent_principal_lamports: unsigned.admission_rent_principal,
        position_top_up_lamports: unsigned.position_top_up_lamports,
        admission_top_up_lamports: unsigned.admission_top_up_lamports,
        transaction_fee_lamports,
        total_owner_debit_lamports,
        total_fee_payer_debit_lamports,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        wire_bytes: compiled.wire_bytes,
        message_sha256: sha256_hex(&message_bytes),
        message_base64,
        claims_request_sha256: hex(&unsigned.claims_request_digest),
        expected_receipt_producer: unsigned.expected_receipt_producer.to_string(),
        expected_receipt_base64: BASE64.encode(unsigned.expected_receipt_body),
        expected_position_base64: BASE64.encode(expected_position),
        expected_admission_base64: BASE64.encode(expected_admission),
        instructions,
        prestate: snapshot.states.clone(),
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    Ok(ReportV1 {
        schema: REPORT_SCHEMA_V1.into(),
        cluster: "devnet".into(),
        rpc_url: arguments.origin.redacted_url(),
        authorized_mutation: arguments.execute,
        phase: PhaseV1::Planned,
        intent_sha256,
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
    })
}

fn resume(rpc: &mut Rpc, arguments: &ArgumentsV1, report: &mut ReportV1) -> Result<()> {
    authenticate_intent_digest(report)?;
    authenticate_genesis_again(rpc, &arguments.origin)?;
    if report.phase == PhaseV1::Finalized {
        verify_persisted_finalized(rpc, report)?;
        return Ok(());
    }

    let completion = probe_expected_poststate(rpc, report, 0)?;
    if completion {
        let signature = report.expected_signature.clone().ok_or_else(|| {
            Error::new(
                "REFUSED: exact admission state exists but this durable plan has no transaction signature; signing would replay and the receipt producer cannot be authenticated",
            )
        })?;
        finalize_known_signature(rpc, report, &signature)?;
        write_report_atomically(&arguments.output, report, false)?;
        return Ok(());
    }
    if !arguments.execute {
        return Ok(());
    }

    match report.phase {
        PhaseV1::Planned => sign_and_submit(rpc, arguments, report),
        PhaseV1::SignedNotSubmitted | PhaseV1::Submitted => {
            let signature = report.expected_signature.clone().ok_or_else(|| {
                Error::new("signed/submitted evidence omitted its expected signature")
            })?;
            match finalized_transaction(rpc, &signature)? {
                Some(_) => {
                    finalize_known_signature(rpc, report, &signature)?;
                    write_report_atomically(&arguments.output, report, false)
                }
                None => Err(Error::new(format!(
                    "REFUSED: transaction {signature} is not finalized and the expected poststate is absent. Its durable phase is {:?}; this is an ambiguous submitted state, so the executor will neither sign again nor replay the packet",
                    report.phase
                ))),
            }
        }
        PhaseV1::Finalized => Ok(()),
    }
}

fn sign_and_submit(rpc: &mut Rpc, arguments: &ArgumentsV1, report: &mut ReportV1) -> Result<()> {
    require_prestate_unchanged(rpc, report)?;
    let height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if height > report.intent.last_valid_block_height {
        return Err(Error::new(
            "planned transaction blockhash expired before key load; archive this unsigned plan and construct a fresh finalized observation",
        ));
    }
    authenticate_genesis_again(rpc, &arguments.origin)?;

    // This is deliberately the first key-file access in the command. The
    // complete message, fee, rent arithmetic, prestate, and output destination
    // are already durable above.
    let owner = Keypair::new_from_array(campaign::read_keypair_file(
        &arguments.position_owner_keypair,
        "position-owner",
    )?);
    if owner.pubkey() != arguments.position_owner {
        return Err(Error::new(
            "position-owner keypair does not expand to --position-owner",
        ));
    }
    let fee_payer = if arguments.position_owner == arguments.fee_payer
        && arguments.position_owner_keypair == arguments.fee_payer_keypair
    {
        None
    } else {
        let payer = Keypair::new_from_array(campaign::read_keypair_file(
            &arguments.fee_payer_keypair,
            "fee-payer",
        )?);
        if payer.pubkey() != arguments.fee_payer {
            return Err(Error::new(
                "fee-payer keypair does not expand to --fee-payer",
            ));
        }
        Some(payer)
    };
    if arguments.position_owner == arguments.fee_payer && fee_payer.is_some() {
        return Err(Error::new(
            "one public key was supplied through two different keypair paths; use the same absolute path for both roles",
        ));
    }
    let message_bytes = BASE64
        .decode(&report.intent.message_base64)
        .map_err(|error| Error::new(format!("persisted message base64: {error}")))?;
    if sha256_hex(&message_bytes) != report.intent.message_sha256 {
        return Err(Error::new("persisted message digest changed"));
    }
    let message: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("persisted versioned message: {error}")))?;
    let mut signers: Vec<&dyn Signer> = Vec::new();
    if let Some(payer) = fee_payer.as_ref() {
        signers.push(payer);
        signers.push(&owner);
    } else {
        signers.push(&owner);
    }
    let transaction = VersionedTransaction::try_new(message, &signers)
        .map_err(|error| Error::new(format!("sign admission transaction: {error}")))?;
    let signature = transaction
        .signatures
        .first()
        .copied()
        .ok_or_else(|| Error::new("signed transaction omitted payer signature"))?;
    let wire = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("serialize signed admission: {error}")))?;
    if wire.len() != report.intent.wire_bytes {
        return Err(Error::new(format!(
            "signed admission wire is {} bytes but the durable plan committed {}",
            wire.len(),
            report.intent.wire_bytes
        )));
    }
    report.signed_packet_base64 = Some(BASE64.encode(&wire));
    report.expected_signature = Some(signature.to_string());
    report.phase = PhaseV1::SignedNotSubmitted;
    write_report_atomically(&arguments.output, report, false)?;

    authenticate_genesis_again(rpc, &arguments.origin)?;
    require_prestate_unchanged(rpc, report)?;
    let returned = rpc
        .call(
            "sendTransaction",
            &json!([BASE64.encode(&wire), {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"finalized",
                "maxRetries":0
            }]),
        )?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("sendTransaction signature: {error}")))?;
    if returned != signature {
        return Err(Error::new(
            "RPC returned a signature different from the locally signed packet",
        ));
    }
    report.phase = PhaseV1::Submitted;
    write_report_atomically(&arguments.output, report, false)?;
    wait_finalized(rpc, &signature.to_string())?;
    finalize_known_signature(rpc, report, &signature.to_string())?;
    write_report_atomically(&arguments.output, report, false)
}

fn finalize_known_signature(rpc: &mut Rpc, report: &mut ReportV1, signature: &str) -> Result<()> {
    let transaction = finalized_transaction(rpc, signature)?.ok_or_else(|| {
        Error::new(format!(
            "signature {signature} has not reached finalized history; confirmed is never accepted"
        ))
    })?;
    let meta = transaction
        .get("meta")
        .ok_or_else(|| Error::new("finalized admission transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "finalized admission transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized admission transaction omitted slot"))?;
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("finalized admission transaction omitted fee"))?;
    if fee != report.intent.transaction_fee_lamports {
        return Err(Error::new(format!(
            "finalized fee {fee} differs from planned exact fee {}",
            report.intent.transaction_fee_lamports
        )));
    }
    let return_data = meta
        .get("returnData")
        .ok_or_else(|| Error::new("finalized admission transaction omitted returnData"))?;
    let producer = return_data
        .get("programId")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("finalized returnData omitted producer"))?;
    if producer != report.intent.expected_receipt_producer {
        return Err(Error::new(format!(
            "finalized return data producer {producer} was not Claims {}",
            report.intent.expected_receipt_producer
        )));
    }
    let encoded = return_data
        .get("data")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("finalized returnData omitted base64 body"))?;
    let receipt = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("finalized returnData base64: {error}")))?;
    let expected = BASE64
        .decode(&report.intent.expected_receipt_base64)
        .map_err(|error| Error::new(format!("persisted receipt base64: {error}")))?;
    if receipt != expected {
        return Err(Error::new(
            "finalized Claims receipt differed from the semantic owner's prediction",
        ));
    }
    let poststate = verify_poststate(rpc, report, slot)?;
    report.finalized = Some(FinalizedEvidenceV1 {
        signature: signature.into(),
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        return_data_producer: producer.into(),
        return_data_sha256: sha256_hex(&receipt),
        poststate,
    });
    report.phase = PhaseV1::Finalized;
    Ok(())
}

fn verify_poststate(
    rpc: &mut Rpc,
    report: &ReportV1,
    minimum_slot: u64,
) -> Result<BTreeMap<String, AccountStateV1>> {
    let mut labels = report.intent.prestate.keys().cloned().collect::<Vec<_>>();
    labels.sort();
    let addresses = labels
        .iter()
        .map(|label| parse_state_address(&report.intent.prestate[label]))
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    if slot < minimum_slot {
        return Err(Error::new(
            "poststate snapshot preceded finalized transaction",
        ));
    }
    let mut post = BTreeMap::new();
    for ((label, address), value) in labels.iter().zip(addresses).zip(values) {
        let state = account_state(address, value.as_ref());
        post.insert(label.clone(), state);
    }
    let expected_position = decode_expected(&report.intent.expected_position_base64, "Position")?;
    let expected_admission =
        decode_expected(&report.intent.expected_admission_base64, "admission")?;
    let position = post
        .get("claims_position")
        .ok_or_else(|| Error::new("poststate omitted Claims Position"))?;
    let admission = post
        .get("claims_admission")
        .ok_or_else(|| Error::new("poststate omitted Claims admission"))?;
    require_account_bytes(
        position,
        &expected_position,
        &report.intent.expected_receipt_producer,
        report.intent.position_rent_principal_lamports,
        "Claims Position",
    )?;
    LiabilityBasisPositionViewV2::decode(&expected_position)
        .map_err(|error| Error::new(format!("expected Claims Position: {error:?}")))?;
    require_account_bytes(
        admission,
        &expected_admission,
        &report.intent.expected_receipt_producer,
        report.intent.admission_rent_principal_lamports,
        "Claims admission",
    )?;
    ProtocolPositionAdmissionV2::decode(&expected_admission)
        .map_err(|error| Error::new(format!("expected Claims admission: {error:?}")))?;

    for label in [
        "claims_market",
        "core_market",
        "lifecycle_rent_credit",
        "founding_trading_custody_replay",
    ] {
        if post.get(label) != report.intent.prestate.get(label) {
            return Err(Error::new(format!(
                "{label} changed during User Position admission; this route owns no such mutation"
            )));
        }
    }
    // Every authenticated release/record/sysvar input is immutable across this
    // route. Owner and payer lamports plus the two newly allocated accounts are
    // the only permitted changes.
    for (label, before) in &report.intent.prestate {
        if matches!(
            label.as_str(),
            "claims_position" | "claims_admission" | "position_owner" | "fee_payer"
        ) {
            continue;
        }
        if post.get(label) != Some(before) {
            return Err(Error::new(format!(
                "authenticated admission input {label} changed unexpectedly"
            )));
        }
    }
    verify_wallet_debits(report, &post)?;
    Ok(post)
}

fn verify_wallet_debits(report: &ReportV1, post: &BTreeMap<String, AccountStateV1>) -> Result<()> {
    let owner_before = &report.intent.prestate["position_owner"];
    let payer_before = &report.intent.prestate["fee_payer"];
    let owner_after = &post["position_owner"];
    let payer_after = &post["fee_payer"];
    if !same_wallet_except_lamports(owner_before, owner_after)
        || !same_wallet_except_lamports(payer_before, payer_after)
    {
        return Err(Error::new(
            "Position owner or fee payer changed owner, privilege, rent epoch, width, or data during admission",
        ));
    }
    if report.intent.position_owner == report.intent.fee_payer {
        let expected = owner_before
            .lamports
            .checked_sub(report.intent.total_owner_debit_lamports)
            .ok_or_else(|| Error::new("owner post-balance underflow"))?;
        if owner_after.lamports != expected || payer_after != owner_after {
            return Err(Error::new(
                "combined owner/fee-payer debit differed from exact rent+fee",
            ));
        }
    } else {
        let expected_owner = owner_before
            .lamports
            .checked_sub(report.intent.total_owner_debit_lamports)
            .ok_or_else(|| Error::new("owner post-balance underflow"))?;
        let expected_payer = payer_before
            .lamports
            .checked_sub(report.intent.total_fee_payer_debit_lamports)
            .ok_or_else(|| Error::new("fee-payer post-balance underflow"))?;
        if owner_after.lamports != expected_owner || payer_after.lamports != expected_payer {
            return Err(Error::new(
                "owner or fee-payer debit differed from exact rent/fee split",
            ));
        }
    }
    Ok(())
}

fn same_wallet_except_lamports(before: &AccountStateV1, after: &AccountStateV1) -> bool {
    before.address == after.address
        && before.owner == after.owner
        && before.executable == after.executable
        && before.rent_epoch == after.rent_epoch
        && before.data_len == after.data_len
        && before.data_sha256 == after.data_sha256
}

fn probe_expected_poststate(rpc: &mut Rpc, report: &ReportV1, floor: u64) -> Result<bool> {
    let position = Pubkey::from_str(&report.intent.position)
        .map_err(|error| Error::new(format!("persisted Position: {error}")))?;
    let admission = Pubkey::from_str(&report.intent.admission)
        .map_err(|error| Error::new(format!("persisted admission: {error}")))?;
    let (_, values) = rpc.finalized_accounts(&[position, admission], floor)?;
    let expected_position = decode_expected(&report.intent.expected_position_base64, "Position")?;
    let expected_admission =
        decode_expected(&report.intent.expected_admission_base64, "admission")?;
    match (&values[0], &values[1]) {
        (None, None) => Ok(false),
        (Some(position), Some(admission)) => {
            let exact = position.owner.to_string() == report.intent.expected_receipt_producer
                && position.lamports == report.intent.position_rent_principal_lamports
                && position.data == expected_position
                && admission.owner.to_string() == report.intent.expected_receipt_producer
                && admission.lamports == report.intent.admission_rent_principal_lamports
                && admission.data == expected_admission;
            if exact {
                Ok(true)
            } else {
                Err(Error::new(
                    "REFUSED: canonical Position/admission coordinates are occupied by another state; never replay or overwrite",
                ))
            }
        }
        _ => Err(Error::new(
            "REFUSED: only one of Position/admission exists; the admission transaction is atomic, so this is a conflicting state",
        )),
    }
}

fn require_prestate_unchanged(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let labels = report.intent.prestate.keys().cloned().collect::<Vec<_>>();
    let addresses = labels
        .iter()
        .map(|label| parse_state_address(&report.intent.prestate[label]))
        .collect::<Result<Vec<_>>>()?;
    let (_, values) = rpc.finalized_accounts(&addresses, report.intent.observation_slot)?;
    for ((label, address), value) in labels.iter().zip(addresses).zip(values) {
        let now = account_state(address, value.as_ref());
        if now != report.intent.prestate[label] {
            return Err(Error::new(format!(
                "REFUSED: {label} changed after the durable finalized plan; this may be a stale deployment, release graph, wallet balance, or replay"
            )));
        }
    }
    Ok(())
}

fn acquire_snapshot(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    plan: &SuccessorPlan,
    coordinates: CoordinatesV1,
    evidence: &Value,
) -> Result<SnapshotBundleV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let raw_keys = [
        coordinates.linked_basis,
        coordinates.product,
        coordinates.result_domain,
        coordinates.portfolio,
    ];
    let (raw_slot, raw_values) =
        rpc.finalized_accounts(&raw_keys, arguments.minimum_finalized_slot)?;
    let schemas = [
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        PORTFOLIO_SCHEMA_ID_V2,
    ];
    let mut records = Vec::new();
    for ((raw, schema), value) in raw_keys.into_iter().zip(schemas).zip(raw_values) {
        let account = value.ok_or_else(|| Error::new(format!("missing finalized record {raw}")))?;
        let digest: [u8; 32] = Sha256::digest(&account.data).into();
        let expected_raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &registry,
        )
        .0;
        if raw != expected_raw {
            return Err(Error::new(format!(
                "campaign evidence record {raw} is not the PDA of its finalized bytes"
            )));
        }
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &registry,
        )
        .0;
        records.push(RecordCoordinatesV1 { raw, staging });
    }

    let claims = pubkey(&plan.claims.program_id)?;
    let position_seeds = dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2::new(
        coordinates.claims_market.to_bytes(),
        arguments.position_owner.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Position seeds: {error:?}")))?;
    let admission_seeds =
        dclutch_claims_svm::protocol_position_v2::ProtocolPositionAdmissionSeedsV2::new(
            coordinates.claims_market.to_bytes(),
            arguments.position_owner.to_bytes(),
        )
        .map_err(|error| Error::new(format!("admission seeds: {error:?}")))?;
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims).0;
    let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &claims).0;
    let keys = vec![
        ("claims_market", coordinates.claims_market),
        ("claims_position", position),
        ("claims_admission", admission),
        ("linked_basis_raw", records[0].raw),
        ("linked_basis_staging", records[0].staging),
        ("product_raw", records[1].raw),
        ("product_staging", records[1].staging),
        ("result_domain_raw", records[2].raw),
        ("result_domain_staging", records[2].staging),
        ("portfolio_raw", records[3].raw),
        ("portfolio_staging", records[3].staging),
        ("rent_sysvar", sysvar::rent::ID),
        ("system_program", system_program::ID),
        ("core_market", coordinates.core_market),
        ("activation_cache", pubkey(&plan.activation)?),
        ("registry_program", registry),
        ("trading_program", pubkey(&plan.trading.program_id)?),
        ("trading_programdata", pubkey(&plan.trading.programdata_id)?),
        ("claims_program", claims),
        ("claims_programdata", pubkey(&plan.claims.programdata_id)?),
        ("core_program", pubkey(&plan.core.program_id)?),
        ("core_programdata", pubkey(&plan.core.programdata_id)?),
        ("position_owner", arguments.position_owner),
        ("lifecycle_rent_credit", coordinates.rent_credit),
        ("rent_program", pubkey(&plan.rent_credit.program_id)?),
        (
            "founding_trading_custody_replay",
            coordinates.trading_replay,
        ),
        ("fee_payer", arguments.fee_payer),
    ];
    let mut unique = Vec::new();
    for (_, key) in &keys {
        if !unique.contains(key) {
            unique.push(*key);
        }
    }
    let (slot, values) = rpc.finalized_accounts(&unique, raw_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let by_key = unique.into_iter().zip(values).collect::<BTreeMap<_, _>>();
    let observed = |key: Pubkey, allow_vacant: bool| -> Result<ObservedAccount> {
        match by_key
            .get(&key)
            .ok_or_else(|| Error::new(format!("snapshot omitted requested account {key}")))?
        {
            Some(account) => Ok(to_observed(observation, key, account)),
            None if allow_vacant => Ok(ObservedAccount {
                observation,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            }),
            None => Err(Error::new(format!(
                "snapshot missing required account {key}"
            ))),
        }
    };
    let position_account = observed(position, true)?;
    let admission_account = observed(admission, true)?;
    let operator = UserPositionAdmissionSnapshotV1 {
        genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
        claims_market: observed(coordinates.claims_market, false)?,
        position: position_account,
        admission: admission_account,
        linked_basis_raw: observed(records[0].raw, false)?,
        linked_basis_staging: observed(records[0].staging, true)?,
        product_raw: observed(records[1].raw, false)?,
        product_staging: observed(records[1].staging, true)?,
        result_domain_raw: observed(records[2].raw, false)?,
        result_domain_staging: observed(records[2].staging, true)?,
        portfolio_raw: observed(records[3].raw, false)?,
        portfolio_staging: observed(records[3].staging, true)?,
        rent_sysvar: observed(sysvar::rent::ID, false)?,
        system_program: observed(system_program::ID, false)?,
        core_market: observed(coordinates.core_market, false)?,
        activation_cache: observed(pubkey(&plan.activation)?, false)?,
        registry_program: observed(registry, false)?,
        trading_program: observed(pubkey(&plan.trading.program_id)?, false)?,
        trading_programdata: observed(pubkey(&plan.trading.programdata_id)?, false)?,
        claims_program: observed(claims, false)?,
        claims_programdata: observed(pubkey(&plan.claims.programdata_id)?, false)?,
        core_program: observed(pubkey(&plan.core.program_id)?, false)?,
        core_programdata: observed(pubkey(&plan.core.programdata_id)?, false)?,
        owner: observed(arguments.position_owner, false)?,
        rent_credit: observed(coordinates.rent_credit, false)?,
        rent_program: observed(pubkey(&plan.rent_credit.program_id)?, false)?,
    };
    let replay = observed(coordinates.trading_replay, false)?;
    let fee_payer = observed(arguments.fee_payer, false)?;
    authenticate_evidence_hints(evidence, &operator, &replay)?;
    let mut states = BTreeMap::new();
    for (label, key) in keys {
        let value = by_key.get(&key).and_then(Option::as_ref);
        states.insert(label.into(), account_state(key, value));
    }
    Ok(SnapshotBundleV1 {
        operator,
        replay,
        fee_payer,
        states,
    })
}

fn evidence_coordinates(evidence: &Value) -> Result<CoordinatesV1> {
    Ok(CoordinatesV1 {
        claims_market: evidence_address(evidence, "claims_aggregate")?,
        core_market: evidence_address(evidence, "founding_market")?,
        rent_credit: evidence_address(evidence, "lifecycle_rent_credit")?,
        trading_replay: evidence_address(evidence, "founding_normal_custody_replay")?,
        product: evidence_address(evidence, "product_record")?,
        result_domain: evidence_address(evidence, "result_domain_record")?,
        portfolio: evidence_address(evidence, "portfolio_record")?,
        linked_basis: evidence_address(evidence, "linked_liability_basis_record")?,
    })
}

fn evidence_accounts(evidence: &Value) -> Result<&serde_json::Map<String, Value>> {
    evidence
        .pointer("/execution/market/accounts")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("campaign evidence omitted execution.market.accounts"))
}

fn evidence_address(evidence: &Value, label: &str) -> Result<Pubkey> {
    let value = evidence_accounts(evidence)?
        .get(label)
        .and_then(|value| value.get("address"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("campaign evidence omitted {label}.address")))?;
    pubkey(value)
}

fn authenticate_plan_evidence(plan_sha256: &str, evidence: &Value) -> Result<()> {
    if evidence.get("schema").and_then(Value::as_str)
        != Some("dclutch-successor-campaign-report-v1")
        || evidence.get("plan_sha256").and_then(Value::as_str) != Some(plan_sha256)
        || evidence
            .pointer("/execution/completed")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::new(
            "campaign evidence schema, plan digest, or completed execution refused",
        ));
    }
    Ok(())
}

fn authenticate_evidence_hints(
    evidence: &Value,
    snapshot: &UserPositionAdmissionSnapshotV1,
    replay: &ObservedAccount,
) -> Result<()> {
    for (label, account) in [
        ("claims_aggregate", &snapshot.claims_market),
        ("founding_market", &snapshot.core_market),
        ("lifecycle_rent_credit", &snapshot.rent_credit),
        ("founding_normal_custody_replay", replay),
        ("product_record", &snapshot.product_raw),
        ("result_domain_record", &snapshot.result_domain_raw),
        ("portfolio_record", &snapshot.portfolio_raw),
        ("linked_liability_basis_record", &snapshot.linked_basis_raw),
    ] {
        let row = evidence_accounts(evidence)?
            .get(label)
            .ok_or_else(|| Error::new(format!("campaign evidence omitted {label}")))?;
        if row.get("address").and_then(Value::as_str) != Some(&account.key.to_string()) {
            return Err(Error::new(format!("campaign evidence substituted {label}")));
        }
        // Immutable finalized records must still be the exact founding bytes.
        if label.ends_with("_record")
            && row.get("data_sha256").and_then(Value::as_str) != Some(&sha256_hex(&account.data))
        {
            return Err(Error::new(format!("finalized record {label} changed")));
        }
    }
    Ok(())
}

fn authenticate_existing_report(
    report: &ReportV1,
    arguments: &ArgumentsV1,
    plan_sha256: &str,
    evidence_sha256: &str,
) -> Result<()> {
    if report.schema != REPORT_SCHEMA_V1
        || report.cluster != "devnet"
        || report.intent.plan_sha256 != plan_sha256
        || report.intent.campaign_evidence_sha256 != evidence_sha256
        || report.intent.position_owner != arguments.position_owner.to_string()
        || report.intent.fee_payer != arguments.fee_payer.to_string()
        || report.intent.minimum_finalized_slot != arguments.minimum_finalized_slot
    {
        return Err(Error::new(
            "existing output belongs to another plan, evidence file, owner, payer, or finalized floor",
        ));
    }
    authenticate_intent_digest(report)
}

fn authenticate_intent_digest(report: &ReportV1) -> Result<()> {
    if sha256_hex(&serde_json::to_vec(&report.intent)?) != report.intent_sha256 {
        return Err(Error::new("durable admission intent digest changed"));
    }
    Ok(())
}

fn verify_persisted_finalized(rpc: &mut Rpc, report: &ReportV1) -> Result<()> {
    let evidence = report
        .finalized
        .as_ref()
        .ok_or_else(|| Error::new("finalized phase omitted finalized evidence"))?;
    let transaction = finalized_transaction(rpc, &evidence.signature)?
        .ok_or_else(|| Error::new("persisted finalized signature disappeared from history"))?;
    if transaction.get("slot").and_then(Value::as_u64) != Some(evidence.slot) {
        return Err(Error::new("persisted finalized slot changed"));
    }
    let post = verify_poststate(rpc, report, evidence.slot)?;
    if post != evidence.poststate {
        return Err(Error::new(
            "current poststate differs from finalized admission evidence",
        ));
    }
    Ok(())
}

fn finalized_transaction(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .filter(|value| !value.is_null());
    let Some(status) = status else {
        return Ok(None);
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(None);
    }
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"json",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(Error::new(
            "finalized signature omitted finalized transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn wait_finalized(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + FINALITY_WAIT;
    while Instant::now() < deadline {
        if finalized_transaction(rpc, signature)?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "transaction {signature} did not reach finalized history within 300 seconds; durable signature retained for resume"
    )))
}

fn latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let hash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("latest blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((hash, last_valid))
}

fn fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact fee"))
}

fn authenticate_genesis_again(rpc: &mut Rpc, origin: &ClusterOriginV1) -> Result<()> {
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    if genesis != DEVNET_GENESIS_HASH {
        return Err(Error::new(format!(
            "User Position executor expected devnet genesis {DEVNET_GENESIS_HASH}, observed {genesis}"
        )));
    }
    origin.authenticate_genesis(&genesis)
}

fn to_observed(observation: Observation, key: Pubkey, account: &RpcAccount) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data.clone(),
    }
}

fn account_state(address: Pubkey, account: Option<&RpcAccount>) -> AccountStateV1 {
    let (owner, lamports, executable, rent_epoch, data): (Pubkey, u64, bool, u64, &[u8]) =
        match account {
            Some(account) => (
                account.owner,
                account.lamports,
                account.executable,
                account.rent_epoch,
                &account.data,
            ),
            None => (system_program::ID, 0, false, 0, &[]),
        };
    let mut exact = Sha256::new();
    exact.update(owner.as_ref());
    exact.update(lamports.to_le_bytes());
    exact.update([u8::from(executable)]);
    exact.update(rent_epoch.to_le_bytes());
    exact.update(u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes());
    exact.update(data);
    AccountStateV1 {
        address: address.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        rent_epoch,
        data_len: data.len(),
        data_sha256: sha256_hex(data),
        account_sha256: hex(&exact.finalize()),
    }
}

fn require_account_bytes(
    state: &AccountStateV1,
    bytes: &[u8],
    owner: &str,
    lamports: u64,
    label: &str,
) -> Result<()> {
    if state.owner != owner
        || state.executable
        || state.lamports != lamports
        || state.data_len != bytes.len()
        || state.data_sha256 != sha256_hex(bytes)
    {
        return Err(Error::new(format!(
            "{label} owner, privilege, exact rent, width, or bytes differed from prediction"
        )));
    }
    Ok(())
}

fn parse_state_address(state: &AccountStateV1) -> Result<Pubkey> {
    Pubkey::from_str(&state.address)
        .map_err(|error| Error::new(format!("persisted account address: {error}")))
}

fn decode_expected(value: &str, label: &str) -> Result<Vec<u8>> {
    BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("persisted {label} base64: {error}")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn write_report_atomically(path: &Path, report: &ReportV1, new: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("admission output path must be absolute"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("admission output needs a UTF-8 file name"))?;
    let temporary =
        path.with_file_name(format!(".{name}.user-position-{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(&temporary)
        .map_err(|error| Error::new(format!("create {}: {error}", temporary.display())))?;
    let bytes = serde_json::to_vec_pretty(report)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if new && path.exists() {
        let _ = fs::remove_file(&temporary);
        return Err(Error::new(format!(
            "admission output {} already exists; resume it or choose a new path",
            path.display()
        )));
    }
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically replace {} from {}: {error}",
            path.display(),
            temporary.display()
        ))
    })?;
    if let Some(parent) = path.parent() {
        let directory = OpenOptions::new()
            .read(true)
            .open(parent)
            .map_err(|error| Error::new(format!("open output directory: {error}")))?;
        directory.sync_all()?;
    }
    Ok(())
}

fn print_report(report: &ReportV1) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(report)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut owner = None;
    let mut owner_keypair = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut minimum_slot = None;
    let mut output = None;
    let mut execute = false;
    let mut seen = BTreeSet::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        if !seen.insert(argument.clone()) {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        match argument.as_str() {
            "--rpc-url" => rpc_url = Some(value),
            DEVNET_ACKNOWLEDGMENT_FLAG => acknowledgment = Some(value),
            "--plan" => plan = Some(value),
            "--campaign-evidence" => evidence = Some(value),
            "--position-owner" => owner = Some(value),
            "--position-owner-keypair" => owner_keypair = Some(value),
            "--fee-payer" => payer = Some(value),
            "--fee-payer-keypair" => payer_keypair = Some(value),
            "--minimum-finalized-slot" => minimum_slot = Some(value),
            "--output" => output = Some(value),
            _ => {
                return Err(Error::new(format!(
                    "unknown devnet-user-position-admission-v1 argument: {argument}"
                )));
            }
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let absolute = |value: Option<String>, label: &str| -> Result<PathBuf> {
        let path = PathBuf::from(required(value, label)?);
        if !path.is_absolute() {
            return Err(Error::new(format!("{label} must be absolute")));
        }
        Ok(path)
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    let minimum_finalized_slot = required(minimum_slot, "--minimum-finalized-slot")?
        .parse::<u64>()
        .map_err(|_| Error::new("--minimum-finalized-slot must be a decimal u64"))?;
    if minimum_finalized_slot == 0 {
        return Err(Error::new("--minimum-finalized-slot must be nonzero"));
    }
    Ok(ArgumentsV1 {
        origin,
        plan: absolute(plan, "--plan")?,
        campaign_evidence: absolute(evidence, "--campaign-evidence")?,
        position_owner: pubkey(&required(owner, "--position-owner")?)?,
        position_owner_keypair: absolute(owner_keypair, "--position-owner-keypair")?,
        fee_payer: pubkey(&required(payer, "--fee-payer")?)?,
        fee_payer_keypair: absolute(payer_keypair, "--fee-payer-keypair")?,
        minimum_finalized_slot,
        output: absolute(output, "--output")?,
        execute,
    })
}

pub(crate) fn usage() -> &'static str {
    "Usage:\n  dclutch-local-successor-bootstrap devnet-user-position-admission-v1 \\\n+     --rpc-url URL --i-mean-devnet DEVNET_GENESIS_HASH --plan ABSOLUTE_JSON \\\n+     --campaign-evidence ABSOLUTE_JSON --position-owner PUBKEY \\\n+     --position-owner-keypair ABSOLUTE_JSON --fee-payer PUBKEY \\\n+     --fee-payer-keypair ABSOLUTE_JSON --minimum-finalized-slot U64 \\\n+     --output ABSOLUTE_JSON [--execute]\n\nDefault is finalized read-only planning. The complete canonical message, exact rent top-ups, exact getFeeForMessage fee, input fingerprints, and output path are fsynced before --execute reads either key file. The command only admits Solana devnet, never accepts confirmed state, never retries an ambiguous submission, and resumes by authenticating the exact Position/admission and finalized signature before it can sign."
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "https://api.devnet.solana.com".into(),
            "--i-mean-devnet".into(),
            DEVNET_GENESIS_HASH.into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
            "--campaign-evidence".into(),
            "/tmp/evidence.json".into(),
            "--position-owner".into(),
            Pubkey::new_unique().to_string(),
            "--position-owner-keypair".into(),
            "/tmp/owner.json".into(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--fee-payer-keypair".into(),
            "/tmp/payer.json".into(),
            "--minimum-finalized-slot".into(),
            "91".into(),
            "--output".into(),
            "/tmp/output.json".into(),
        ]
    }

    #[test]
    fn cli_is_explicit_and_defaults_to_no_mutation() {
        let parsed = parse_arguments(base_args()).expect("explicit CLI");
        assert!(!parsed.execute);
        assert_eq!(parsed.minimum_finalized_slot, 91);
    }

    #[test]
    fn cli_refuses_relative_key_path_and_confirmed_escape_hatch() {
        let mut relative = base_args();
        let index = relative
            .iter()
            .position(|value| value == "--position-owner-keypair")
            .expect("flag");
        relative[index + 1] = "owner.json".into();
        assert!(
            parse_arguments(relative)
                .expect_err("relative")
                .to_string()
                .contains("must be absolute")
        );

        let mut confirmed = base_args();
        confirmed.extend(["--commitment".into(), "confirmed".into()]);
        assert!(
            parse_arguments(confirmed)
                .expect_err("confirmed")
                .to_string()
                .contains("unknown")
        );
    }

    #[test]
    fn cli_refuses_loopback_even_with_devnet_acknowledgment() {
        let mut args = base_args();
        args[1] = "http://127.0.0.1:20990".into();
        assert!(parse_arguments(args).is_err());
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeSignatureState {
        Missing,
        Confirmed,
        FinalizedOk,
        FinalizedError,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ResumeDecision {
        Sign,
        VerifyFinalized,
        RefuseAmbiguous,
        RefuseFailed,
        RefuseReplayWithoutSignature,
    }

    fn fake_resume_decision(
        phase: PhaseV1,
        signature: Option<FakeSignatureState>,
        exact_poststate: bool,
    ) -> ResumeDecision {
        if exact_poststate && signature.is_none() {
            return ResumeDecision::RefuseReplayWithoutSignature;
        }
        match signature {
            Some(FakeSignatureState::FinalizedOk) => ResumeDecision::VerifyFinalized,
            Some(FakeSignatureState::FinalizedError) => ResumeDecision::RefuseFailed,
            Some(FakeSignatureState::Missing | FakeSignatureState::Confirmed) => {
                ResumeDecision::RefuseAmbiguous
            }
            None if phase == PhaseV1::Planned => ResumeDecision::Sign,
            None => ResumeDecision::RefuseAmbiguous,
        }
    }

    #[test]
    fn fake_rpc_phase_machine_never_accepts_confirmed_or_replays_ambiguity() {
        assert_eq!(
            fake_resume_decision(PhaseV1::Planned, None, false),
            ResumeDecision::Sign
        );
        assert_eq!(
            fake_resume_decision(
                PhaseV1::Submitted,
                Some(FakeSignatureState::Confirmed),
                false
            ),
            ResumeDecision::RefuseAmbiguous
        );
        assert_eq!(
            fake_resume_decision(
                PhaseV1::SignedNotSubmitted,
                Some(FakeSignatureState::Missing),
                false
            ),
            ResumeDecision::RefuseAmbiguous
        );
        assert_eq!(
            fake_resume_decision(PhaseV1::Planned, None, true),
            ResumeDecision::RefuseReplayWithoutSignature
        );
        assert_eq!(
            fake_resume_decision(
                PhaseV1::Submitted,
                Some(FakeSignatureState::FinalizedError),
                false
            ),
            ResumeDecision::RefuseFailed
        );
        assert_eq!(
            fake_resume_decision(
                PhaseV1::Submitted,
                Some(FakeSignatureState::FinalizedOk),
                true
            ),
            ResumeDecision::VerifyFinalized
        );
    }
}
