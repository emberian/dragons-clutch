//! Compose the physically executed Series prefix without pretending Direct
//! retirement evidence is family-neutral.
//!
//! The current tree can prove a real-ELF Series Found, a distinct-blockhash
//! replay refusal, a second permit's accepted expiry plus two infrastructure
//! hostiles, provider Resolution, and a wallet payout. The only shipped
//! terminal-sequence and aggregate-retirement producers are Direct-specific.
//! This adapter therefore emits an exact, machine-readable prefix ledger and
//! fails closed if a caller offers either Direct completion schema as if it
//! retired a Series root.

use std::{collections::BTreeSet, fs, path::PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    plan::hex,
    rpc::parse_json_without_duplicate_keys_v1,
    wallet_terminal::{LookupTableRequirementV1, PlanInputV1, SelectedInputV1},
};

pub(crate) const SERIES_LIFECYCLE_PREFIX_COMMAND_V1: &str =
    "local-private-validator-series-lifecycle-prefix-v1";

const LEDGER_SCHEMA_V2: &str = "dclutch-series-lifecycle-prefix-ledger-v2";
const CONSUME_CHECKPOINT_SCHEMA_V1: &str = "dclutch-series-consume-physical-checkpoint-v1";
const CONSUME_CHECKPOINT_SCHEMA_V2: &str = "dclutch-series-consume-physical-checkpoint-v2";
const EXPIRY_CHECKPOINT_SCHEMA_V1: &str = "dclutch-series-permit-expiry-physical-checkpoint-v1";
const RESOLUTION_INPUT_FORMAT_V1: &str = "dclutch-owned-loopback-flagship-resolution-input-v1";
const RESOLUTION_CHECKPOINT_FORMAT_V3: &str =
    "dclutch-owned-loopback-flagship-resolution-checkpoint-v3";
const WALLET_INPUT_FORMAT_V1: &str = "dclutch-wallet-terminal-payout-plan-input-v1";
const WALLET_PAYOUT_EVIDENCE_SCHEMA_V1: &str =
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1";
const DIRECT_TERMINAL_COMPLETION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-terminal-sequence-completion-v1";
const DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-aggregate-retirement-completion-v1";
const FUTURE_SERIES_TERMINAL_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-terminal-sequence-completion-v1";
const FUTURE_SERIES_AGGREGATE_SCHEMA_V1: &str =
    "dclutch-owned-loopback-series-aggregate-retirement-completion-v1";

const REPLAY_REFUSAL_MARKET: u64 = 0x3005;
const EXPIRY_REFUSAL_INFRASTRUCTURE: u64 = 0x300f;

#[derive(Clone, Debug)]
struct ArgumentsV1 {
    consume_checkpoint: PathBuf,
    expiry_checkpoint: PathBuf,
    resolution_input: PathBuf,
    resolution_checkpoint: PathBuf,
    wallet_input: PathBuf,
    payout_evidence: PathBuf,
    terminal_completion: Option<PathBuf>,
    aggregate_completion: Option<PathBuf>,
    output: PathBuf,
}

struct ArtifactV1 {
    path: PathBuf,
    sha256: String,
    value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactBindingV1 {
    role: &'static str,
    schema: String,
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OccurrenceReceiptV1 {
    world: &'static str,
    kind: &'static str,
    signature: String,
    finalized_slot: u64,
    compute_units_consumed: u64,
    fee_lamports: u64,
    disposition: &'static str,
    refusal_code: Option<u64>,
    evidence_sha256: String,
    writable_lamports_before: u64,
    writable_lamports_after: u64,
    conservation_exact: bool,
    rollback_byte_exact: Option<bool>,
    transaction_fee_only_balance_change: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolutionReceiptV1 {
    input_sha256: String,
    receipt_count: usize,
    signatures: Vec<String>,
    first_finalized_slot: u64,
    last_finalized_slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PayoutReceiptV1 {
    signature: String,
    finalized_slot: u64,
    fee_lamports: u64,
    compute_units_consumed: u64,
    recipient: String,
    payout: String,
    evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MissingAuthorityV1 {
    first_missing_transition: &'static str,
    missing_transitions: Vec<&'static str>,
    required_terminal_schema: &'static str,
    required_aggregate_schema: &'static str,
    direct_terminal_schema_refused: &'static str,
    direct_aggregate_schema_refused: &'static str,
    direct_handoff_allowed: bool,
}

/// Accepted transaction evidence which established the recurring Series root.
/// The terminal campaign derives its own campaign and validator-ledger
/// identities when it admits these immutable facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SeriesPrefixFoundBindingV2 {
    pub(crate) root: String,
    pub(crate) parent_market: String,
    pub(crate) parent_generation: u64,
    pub(crate) template: String,
    pub(crate) signature: String,
    pub(crate) finalized_slot: u64,
    pub(crate) packet_sha256: String,
    pub(crate) poststate_sha256: String,
    pub(crate) found_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SeriesLifecyclePrefixLedgerV2 {
    schema: &'static str,
    status: &'static str,
    complete: bool,
    same_ledger_recurring_sequence_proven: bool,
    cluster: &'static str,
    family: &'static str,
    market: String,
    permit: String,
    found: SeriesPrefixFoundBindingV2,
    source_artifacts: Vec<ArtifactBindingV1>,
    occurrence_path_evidence: Vec<OccurrenceReceiptV1>,
    resolution: ResolutionReceiptV1,
    payout: PayoutReceiptV1,
    evidence_ensemble_transaction_fees_lamports: u64,
    evidence_ensemble_compute_units_consumed: u64,
    temporary_protocol_state_closed: bool,
    missing_authority: MissingAuthorityV1,
    ledger_sha256: String,
}

struct ConsumeFactsV1 {
    market: String,
    permit: String,
    found: Option<SeriesPrefixFoundBindingV2>,
    receipts: Vec<OccurrenceReceiptV1>,
}

struct ExpiryFactsV1 {
    receipts: Vec<OccurrenceReceiptV1>,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments_v1(arguments)?;
    let consume = read_artifact_v1(&arguments.consume_checkpoint, "consume checkpoint")?;
    let expiry = read_artifact_v1(&arguments.expiry_checkpoint, "expiry checkpoint")?;
    let resolution_input = read_artifact_v1(&arguments.resolution_input, "Resolution input")?;
    let resolution_checkpoint =
        read_artifact_v1(&arguments.resolution_checkpoint, "Resolution checkpoint")?;
    let wallet_input = read_artifact_v1(&arguments.wallet_input, "wallet payout input")?;
    let payout = read_artifact_v1(&arguments.payout_evidence, "wallet payout evidence")?;

    if let Some(path) = arguments.terminal_completion.as_ref() {
        refuse_terminal_artifact_v1(&read_artifact_v1(path, "terminal completion")?, true)?;
    }
    if let Some(path) = arguments.aggregate_completion.as_ref() {
        refuse_terminal_artifact_v1(&read_artifact_v1(path, "aggregate completion")?, false)?;
    }

    let consume_facts = authenticate_consume_v1(&consume)?;
    let found = consume_facts.found.clone().ok_or_else(|| {
        Error::new(format!(
            "Series prefix requires {CONSUME_CHECKPOINT_SCHEMA_V2} with an exact authenticated Found binding; legacy checkpoint evidence cannot author terminal acquisition"
        ))
    })?;
    let expiry_facts = authenticate_expiry_v1(&expiry, &consume_facts.permit)?;
    let resolution_receipt = authenticate_resolution_v1(
        &resolution_input,
        &resolution_checkpoint,
        &consume_facts.market,
    )?;
    let payout_receipt = authenticate_payout_v1(
        &wallet_input,
        &payout,
        &consume_facts.market,
        &resolution_input,
    )?;
    if resolution_receipt
        .signatures
        .iter()
        .any(|signature| signature == &payout_receipt.signature)
    {
        return Err(Error::new(
            "wallet payout reused a Resolution transaction signature",
        ));
    }

    let mut occurrence_path_evidence = consume_facts.receipts;
    occurrence_path_evidence.extend(expiry_facts.receipts);
    let evidence_ensemble_transaction_fees_lamports = occurrence_path_evidence
        .iter()
        .try_fold(0_u64, |total, receipt| {
            total.checked_add(receipt.fee_lamports)
        })
        .and_then(|total| total.checked_add(resolution_receipt.fee_lamports))
        .and_then(|total| total.checked_add(payout_receipt.fee_lamports))
        .ok_or_else(|| Error::new("Series prefix fee ledger overflowed"))?;
    let evidence_ensemble_compute_units_consumed = occurrence_path_evidence
        .iter()
        .try_fold(0_u64, |total, receipt| {
            total.checked_add(receipt.compute_units_consumed)
        })
        .and_then(|total| total.checked_add(resolution_receipt.compute_units_consumed))
        .and_then(|total| total.checked_add(payout_receipt.compute_units_consumed))
        .ok_or_else(|| Error::new("Series prefix compute ledger overflowed"))?;

    let source_artifacts = vec![
        binding_v1("series-consume", &consume, CONSUME_CHECKPOINT_SCHEMA_V2)?,
        binding_v1("series-expiry", &expiry, EXPIRY_CHECKPOINT_SCHEMA_V1)?,
        binding_v1(
            "resolution-input",
            &resolution_input,
            RESOLUTION_INPUT_FORMAT_V1,
        )?,
        binding_v1(
            "resolution-checkpoint",
            &resolution_checkpoint,
            RESOLUTION_CHECKPOINT_FORMAT_V3,
        )?,
        binding_v1("wallet-payout-input", &wallet_input, WALLET_INPUT_FORMAT_V1)?,
        binding_v1(
            "wallet-payout-evidence",
            &payout,
            WALLET_PAYOUT_EVIDENCE_SCHEMA_V1,
        )?,
    ];
    let mut ledger = SeriesLifecyclePrefixLedgerV2 {
        schema: LEDGER_SCHEMA_V2,
        status: "artifact-bound-prefix-through-wallet-payout",
        complete: false,
        same_ledger_recurring_sequence_proven: false,
        cluster: "owned-loopback",
        family: "series",
        market: consume_facts.market,
        permit: consume_facts.permit,
        found,
        source_artifacts,
        occurrence_path_evidence,
        resolution: resolution_receipt,
        payout: payout_receipt,
        evidence_ensemble_transaction_fees_lamports,
        evidence_ensemble_compute_units_consumed,
        temporary_protocol_state_closed: false,
        missing_authority: MissingAuthorityV1 {
            first_missing_transition: "series-second-occurrence-commit",
            missing_transitions: vec![
                "series-second-occurrence-commit",
                "series-terminal-source-and-funding-close",
                "series-root-retirement",
            ],
            required_terminal_schema: FUTURE_SERIES_TERMINAL_SCHEMA_V1,
            required_aggregate_schema: FUTURE_SERIES_AGGREGATE_SCHEMA_V1,
            direct_terminal_schema_refused: DIRECT_TERMINAL_COMPLETION_SCHEMA_V1,
            direct_aggregate_schema_refused: DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1,
            direct_handoff_allowed: false,
        },
        ledger_sha256: String::new(),
    };
    ledger.ledger_sha256 = ledger_digest_v2(&ledger)?;
    let bytes = json_bytes_v1(&ledger)?;
    write_idempotent_v1(&arguments.output, &bytes)?;
    println!(
        "Series lifecycle prefix artifact-bound through wallet payout: {} isolated occurrence-path receipts, {} Resolution receipts, ensemble fees {} lamports, ensemble compute {} CU. A same-ledger second occurrence and Series terminal authority remain absent; Direct handoff was refused. Ledger {}",
        ledger.occurrence_path_evidence.len(),
        ledger.resolution.receipt_count,
        ledger.evidence_ensemble_transaction_fees_lamports,
        ledger.evidence_ensemble_compute_units_consumed,
        arguments.output.display()
    );
    Ok(())
}

fn authenticate_consume_v1(artifact: &ArtifactV1) -> Result<ConsumeFactsV1> {
    let schema = require_nonempty_string_v1(artifact.value.get("schema"), "consume schema")?;
    if schema != CONSUME_CHECKPOINT_SCHEMA_V1 && schema != CONSUME_CHECKPOINT_SCHEMA_V2 {
        return Err(Error::new(format!(
            "{} identity is neither exact {CONSUME_CHECKPOINT_SCHEMA_V1} nor {CONSUME_CHECKPOINT_SCHEMA_V2}",
            artifact.path.display()
        )));
    }
    require_bool_v1(
        &artifact.value,
        "/validator/realElf",
        true,
        "consume real ELF",
    )?;
    let market = require_pubkey_v1(&artifact.value, "/fixture/market", "consume Market")?;
    let permit = require_pubkey_v1(&artifact.value, "/fixture/permit", "consume permit")?;
    let accepted = require_object_v1(&artifact.value, "/accepted", "accepted Consume")?;
    let replay = require_object_v1(&artifact.value, "/replay", "Consume replay")?;
    let accepted_signature =
        require_nonempty_string_v1(accepted.get("signature"), "accepted Consume signature")?;
    let accepted_blockhash = require_nonempty_string_v1(
        accepted.get("recentBlockhash"),
        "accepted Consume blockhash",
    )?;
    let accepted_evidence_sha = require_sha256_v1(
        require_nonempty_string_v1(
            accepted.get("evidenceSha256"),
            "accepted Consume evidence digest",
        )?,
        "accepted Consume evidence digest",
    )?;
    require_bool_member_v1(
        accepted,
        "writableLamportsConserved",
        true,
        "accepted conservation",
    )?;
    let accepted_before =
        require_u64_member_v1(accepted, "writableLamportsBefore", "accepted prestate")?;
    let accepted_after =
        require_u64_member_v1(accepted, "writableLamportsAfter", "accepted poststate")?;
    if accepted_before != accepted_after {
        return Err(Error::new(
            "accepted Series Consume writable lamports were not conserved",
        ));
    }
    authenticate_writable_ledger_v1(accepted, &market, &permit, accepted_before)?;
    let fee_payer = require_pubkey_member_v1(accepted, "feePayer", "consume fee payer")?;
    let found = receipt_from_v1(accepted, "consume", "found", None, false)?;
    let found_binding = if schema == CONSUME_CHECKPOINT_SCHEMA_V2 {
        Some(authenticate_prefix_found_binding_v2(
            &artifact.value,
            &market,
            accepted_signature,
            require_positive_u64_member_v1(accepted, "finalizedSlot", "accepted slot")?,
        )?)
    } else {
        None
    };

    if require_nonempty_string_v1(replay.get("replayOfSignature"), "replay parent signature")?
        != accepted_signature
        || require_nonempty_string_v1(
            replay.get("replayOfEvidenceSha256"),
            "replay parent evidence digest",
        )? != accepted_evidence_sha
        || require_pubkey_member_v1(replay, "feePayer", "replay fee payer")? != fee_payer
        || require_nonempty_string_v1(replay.get("recentBlockhash"), "replay blockhash")?
            == accepted_blockhash
        || require_nonempty_string_v1(replay.get("signature"), "replay signature")?
            == accepted_signature
        || require_nonempty_string_v1(replay.get("evidenceSha256"), "replay evidence digest")?
            == accepted_evidence_sha
        || require_positive_u64_member_v1(replay, "finalizedSlot", "replay slot")?
            <= require_positive_u64_member_v1(accepted, "finalizedSlot", "accepted slot")?
        || require_u64_member_v1(replay, "refusalCode", "replay refusal")? != REPLAY_REFUSAL_MARKET
    {
        return Err(Error::new(
            "Series replay did not bind the accepted evidence under the same payer and a distinct blockhash/signature with exact Market refusal",
        ));
    }
    for (field, label) in [
        ("distinctRecentBlockhash", "distinct replay blockhash"),
        ("distinctSignature", "distinct replay signature"),
        ("writableLamportsConserved", "replay conservation"),
        ("rollbackByteExact", "replay rollback"),
        ("transactionFeeOnlyBalanceChange", "replay fee-only change"),
        ("poststateMatchesAccepted", "replay poststate binding"),
    ] {
        require_bool_member_v1(replay, field, true, label)?;
    }
    let replay_before = require_u64_member_v1(replay, "writableLamportsBefore", "replay prestate")?;
    let replay_after = require_u64_member_v1(replay, "writableLamportsAfter", "replay poststate")?;
    if replay_before != accepted_after || replay_after != accepted_after {
        return Err(Error::new(
            "Series replay writable ledger diverged from accepted poststate",
        ));
    }
    let replay_receipt = receipt_from_v1(
        replay,
        "consume-replay",
        "refused",
        Some(REPLAY_REFUSAL_MARKET),
        true,
    )?;
    Ok(ConsumeFactsV1 {
        market,
        permit,
        found: found_binding,
        receipts: vec![found, replay_receipt],
    })
}

fn authenticate_prefix_found_binding_v2(
    value: &Value,
    market: &str,
    accepted_signature: &str,
    accepted_slot: u64,
) -> Result<SeriesPrefixFoundBindingV2> {
    let binding = decode_prefix_found_binding_v2(value)?;
    if binding.parent_market != market
        || binding.signature != accepted_signature
        || binding.finalized_slot != accepted_slot
    {
        return Err(Error::new(
            "Series Found binding changed its accepted Market, signature, or slot",
        ));
    }
    Ok(binding)
}

fn decode_prefix_found_binding_v2(value: &Value) -> Result<SeriesPrefixFoundBindingV2> {
    let object = require_object_v1(value, "/found", "Series Found binding")?;
    let root = require_pubkey_member_v1(object, "root", "Series Found root")?;
    let parent_market =
        require_pubkey_member_v1(object, "parentMarket", "Series Found parent Market")?;
    let parent_generation =
        require_positive_u64_member_v1(object, "parentGeneration", "Series Found generation")?;
    let template = require_sha256_v1(
        require_nonempty_string_v1(object.get("template"), "Series Found Template")?,
        "Series Found Template",
    )?;
    let signature =
        require_nonempty_string_v1(object.get("signature"), "Series Found signature")?.to_owned();
    signature
        .parse::<solana_sdk::signature::Signature>()
        .map_err(|error| Error::new(format!("Series Found signature: {error}")))?;
    let finalized_slot =
        require_positive_u64_member_v1(object, "finalizedSlot", "Series Found finalized slot")?;
    let packet_sha256 = require_sha256_v1(
        require_nonempty_string_v1(object.get("packetSha256"), "Series Found packet")?,
        "Series Found packet",
    )?;
    let poststate_sha256 = require_sha256_v1(
        require_nonempty_string_v1(object.get("poststateSha256"), "Series Found poststate")?,
        "Series Found poststate",
    )?;
    let found_sha256 = require_sha256_v1(
        require_nonempty_string_v1(object.get("foundSha256"), "Series Found binding digest")?,
        "Series Found binding digest",
    )?;
    let binding = SeriesPrefixFoundBindingV2 {
        root,
        parent_market,
        parent_generation,
        template,
        signature,
        finalized_slot,
        packet_sha256,
        poststate_sha256,
        found_sha256,
    };
    if binding.root == binding.parent_market
        || binding.found_sha256 != prefix_found_digest_v2(&binding)?
    {
        return Err(Error::new(
            "Series Found binding changed its root separation or exact digest",
        ));
    }
    Ok(binding)
}

/// Read the prefix semantic owner's exact v2 output and return only its
/// authenticated Found transaction. The terminal producer supplies the raw
/// file digest from its immutable campaign recipe; it cannot project a Found
/// binding from unrelated evidence or from the historical v1 prefix.
pub(crate) fn read_authenticated_series_prefix_found_v2(
    path: &std::path::Path,
    expected_file_sha256: &str,
) -> Result<SeriesPrefixFoundBindingV2> {
    require_sha256_v1(expected_file_sha256, "Series prefix file")?;
    let path = path.to_path_buf();
    let artifact = read_artifact_v1(&path, "Series lifecycle prefix")?;
    if artifact.sha256 != expected_file_sha256 {
        return Err(Error::new(
            "Series lifecycle prefix bytes changed from the immutable acquisition recipe",
        ));
    }
    require_identity_v1(&artifact, "schema", LEDGER_SCHEMA_V2)?;
    require_bool_v1(
        &artifact.value,
        "/sameLedgerRecurringSequenceProven",
        false,
        "Series prefix completion state",
    )?;
    require_bool_v1(
        &artifact.value,
        "/complete",
        false,
        "Series prefix complete flag",
    )?;
    if artifact.value.get("family").and_then(Value::as_str) != Some("series")
        || artifact.value.get("cluster").and_then(Value::as_str) != Some("owned-loopback")
    {
        return Err(Error::new(
            "Series lifecycle prefix changed family or owned-loopback cluster",
        ));
    }
    let supplied_digest = require_sha256_v1(
        require_string_v1(
            &artifact.value,
            "/ledgerSha256",
            "Series prefix ledger digest",
        )?,
        "Series prefix ledger digest",
    )?;
    if supplied_digest != prefix_ledger_value_digest_v2(&artifact.value)? {
        return Err(Error::new(
            "Series lifecycle prefix changed outside its ledger digest",
        ));
    }
    let binding = decode_prefix_found_binding_v2(&artifact.value)?;
    let market = require_pubkey_v1(&artifact.value, "/market", "Series prefix Market")?;
    let consume_source = artifact
        .value
        .pointer("/sourceArtifacts")
        .and_then(Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .find(|value| value.get("role").and_then(Value::as_str) == Some("series-consume"))
        })
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("Series prefix omitted its v2 Consume source binding"))?;
    let consume_source_sha = require_sha256_v1(
        require_nonempty_string_v1(
            consume_source.get("sha256"),
            "Series prefix Consume source digest",
        )?,
        "Series prefix Consume source digest",
    )?;
    let consume_source_path = PathBuf::from(require_nonempty_string_v1(
        consume_source.get("path"),
        "Series prefix Consume source path",
    )?);
    if !consume_source_path.is_absolute() {
        return Err(Error::new(
            "Series prefix Consume source path was not absolute",
        ));
    }
    let consume_artifact = read_artifact_v1(&consume_source_path, "Series prefix Consume source")?;
    let consume_facts = authenticate_consume_v1(&consume_artifact)?;
    if binding.parent_market != market
        || consume_source.get("schema").and_then(Value::as_str)
            != Some(CONSUME_CHECKPOINT_SCHEMA_V2)
        || consume_source_sha.bytes().all(|byte| byte == b'0')
        || consume_artifact.sha256 != consume_source_sha
        || consume_facts.market != market
        || consume_facts.found.as_ref() != Some(&binding)
    {
        return Err(Error::new(
            "Series prefix Found Market or v2 Consume source authority changed",
        ));
    }
    Ok(binding)
}

fn authenticate_writable_ledger_v1(
    accepted: &serde_json::Map<String, Value>,
    market: &str,
    permit: &str,
    expected_total: u64,
) -> Result<()> {
    let rows = accepted
        .get("writableLedger")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("accepted Consume omitted writable ledger"))?;
    if rows.len() != 3 {
        return Err(Error::new(
            "accepted Consume writable ledger is not exactly three accounts",
        ));
    }
    let mut roles = BTreeSet::new();
    let mut addresses = BTreeSet::new();
    let mut before = 0_u64;
    let mut after = 0_u64;
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| Error::new("Consume writable ledger row was not an object"))?;
        let role = require_nonempty_string_v1(row.get("role"), "writable role")?;
        let address = require_pubkey_member_v1(row, "address", "writable address")?;
        roles.insert(role.to_owned());
        if !addresses.insert(address.clone()) {
            return Err(Error::new("Consume writable ledger duplicated an address"));
        }
        if (role == "market" && address != market)
            || (role == "foundingPermit" && address != permit)
        {
            return Err(Error::new(
                "Consume writable ledger role/address binding changed",
            ));
        }
        for field in ["dataSha256Before", "dataSha256After"] {
            require_sha256_v1(require_nonempty_string_v1(row.get(field), field)?, field)?;
        }
        before = before
            .checked_add(require_u64_member_v1(
                row,
                "lamportsBefore",
                "writable prestate",
            )?)
            .ok_or_else(|| Error::new("Consume writable prestate overflowed"))?;
        after = after
            .checked_add(require_u64_member_v1(
                row,
                "lamportsAfter",
                "writable poststate",
            )?)
            .ok_or_else(|| Error::new("Consume writable poststate overflowed"))?;
    }
    if roles
        != ["callerAuthority", "foundingPermit", "market"]
            .into_iter()
            .map(str::to_owned)
            .collect()
        || before != expected_total
        || after != expected_total
    {
        return Err(Error::new(
            "Consume writable ledger roles or exact totals changed",
        ));
    }
    Ok(())
}

fn authenticate_expiry_v1(artifact: &ArtifactV1, consume_permit: &str) -> Result<ExpiryFactsV1> {
    require_identity_v1(artifact, "schema", EXPIRY_CHECKPOINT_SCHEMA_V1)?;
    require_bool_v1(
        &artifact.value,
        "/validator/realElf",
        true,
        "expiry real ELF",
    )?;
    let permit = require_pubkey_v1(&artifact.value, "/identity/permit", "expiry permit")?;
    if permit != consume_permit {
        return Err(Error::new(
            "consume and expiry checkpoints name different permit coordinates",
        ));
    }
    let runs = artifact
        .value
        .pointer("/runs")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("expiry checkpoint omitted runs"))?;
    if runs.len() != 3 {
        return Err(Error::new(
            "expiry checkpoint is not the exact three-world ensemble",
        ));
    }
    let mut receipts = Vec::new();
    for (case, disposition, refusal, rollback) in [
        ("successor", "refunded", None, false),
        (
            "wrong-address",
            "refused",
            Some(EXPIRY_REFUSAL_INFRASTRUCTURE),
            true,
        ),
        (
            "sealed-predecessor",
            "refused",
            Some(EXPIRY_REFUSAL_INFRASTRUCTURE),
            true,
        ),
    ] {
        let run = runs
            .iter()
            .filter_map(Value::as_object)
            .find(|run| run.get("case").and_then(Value::as_str) == Some(case))
            .ok_or_else(|| Error::new(format!("expiry checkpoint omitted {case}")))?;
        let ledger = run
            .get("assetLedger")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new(format!("expiry {case} omitted asset ledger")))?;
        require_bool_member_v1(ledger, "conservationExact", true, "expiry conservation")?;
        let before = require_u64_member_v1(ledger, "twoAccountLamportsBefore", "expiry prestate")?;
        let after = require_u64_member_v1(ledger, "twoAccountLamportsAfter", "expiry poststate")?;
        if before != after {
            return Err(Error::new(format!(
                "expiry {case} did not conserve its two-account ledger"
            )));
        }
        if rollback {
            require_bool_member_v1(run, "rollbackByteExact", true, "expiry rollback")?;
            require_bool_member_v1(
                run,
                "transactionFeeOnlyBalanceChange",
                true,
                "expiry fee-only change",
            )?;
        }
        receipts.push(receipt_from_expiry_v1(
            run,
            case,
            disposition,
            refusal,
            before,
            after,
            rollback,
        )?);
    }
    Ok(ExpiryFactsV1 { receipts })
}

fn authenticate_resolution_v1(
    input: &ArtifactV1,
    checkpoint: &ArtifactV1,
    market: &str,
) -> Result<ResolutionReceiptV1> {
    require_identity_v1(input, "format", RESOLUTION_INPUT_FORMAT_V1)?;
    require_identity_v1(checkpoint, "format", RESOLUTION_CHECKPOINT_FORMAT_V3)?;
    if require_pubkey_v1(&input.value, "/accounts/market", "Resolution Market")? != market
        || require_sha256_v1(
            require_string_v1(&checkpoint.value, "/inputSha256", "Resolution input digest")?,
            "Resolution input digest",
        )? != input.sha256
        || checkpoint.value.pointer("/stagePlan") != Some(&Value::Null)
        || checkpoint
            .value
            .pointer("/verifiedTerminal")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::new(
            "Resolution checkpoint is not the verified terminal for this exact Series Market/input",
        ));
    }
    let receipts = checkpoint
        .value
        .pointer("/receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("Resolution checkpoint omitted receipts"))?;
    let expected = [
        "submit",
        "resolution-provider-execute-v1",
        "core-terminal-accept-v1",
        "reclaim",
    ];
    if receipts.len() != expected.len() {
        return Err(Error::new(
            "Resolution terminal prefix is not exactly four receipts",
        ));
    }
    let mut signatures = Vec::new();
    let mut seen = BTreeSet::new();
    let mut slots = Vec::new();
    let mut fees = 0_u64;
    let mut compute = 0_u64;
    for (receipt, stage) in receipts.iter().zip(expected) {
        let receipt = receipt
            .as_object()
            .ok_or_else(|| Error::new("Resolution receipt was not an object"))?;
        if receipt.get("stage").and_then(Value::as_str) != Some(stage) {
            return Err(Error::new(
                "Resolution receipt order or stage identity changed",
            ));
        }
        let signature =
            require_nonempty_string_v1(receipt.get("signature"), "Resolution signature")?;
        if !seen.insert(signature.to_owned()) {
            return Err(Error::new("Resolution receipts reused a signature"));
        }
        signatures.push(signature.to_owned());
        slots.push(require_positive_u64_member_v1(
            receipt,
            "slot",
            "Resolution slot",
        )?);
        fees = fees
            .checked_add(require_positive_u64_member_v1(
                receipt,
                "feeLamports",
                "Resolution fee",
            )?)
            .ok_or_else(|| Error::new("Resolution fee total overflowed"))?;
        compute = compute
            .checked_add(require_positive_u64_member_v1(
                receipt,
                "computeUnitsConsumed",
                "Resolution compute",
            )?)
            .ok_or_else(|| Error::new("Resolution compute total overflowed"))?;
        require_sha256_v1(
            require_nonempty_string_v1(
                receipt.get("signedTransactionSha256"),
                "Resolution packet digest",
            )?,
            "Resolution packet digest",
        )?;
    }
    if slots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::new(
            "Resolution receipt slots are not strictly increasing",
        ));
    }
    Ok(ResolutionReceiptV1 {
        input_sha256: input.sha256.clone(),
        receipt_count: receipts.len(),
        signatures,
        first_finalized_slot: slots[0],
        last_finalized_slot: *slots.last().unwrap_or(&slots[0]),
        fee_lamports: fees,
        compute_units_consumed: compute,
    })
}

fn authenticate_payout_v1(
    input: &ArtifactV1,
    evidence: &ArtifactV1,
    market: &str,
    resolution_input: &ArtifactV1,
) -> Result<PayoutReceiptV1> {
    require_identity_v1(input, "format", WALLET_INPUT_FORMAT_V1)?;
    require_identity_v1(evidence, "schema", WALLET_PAYOUT_EVIDENCE_SCHEMA_V1)?;
    let payout_input: PlanInputV1 = serde_json::from_value(input.value.clone())
        .map_err(|error| Error::new(format!("wallet payout input shape: {error}")))?;
    let selected = SelectedInputV1::parse(&payout_input, LookupTableRequirementV1::Present)?;
    let resolution_certificate = require_pubkey_v1(
        &resolution_input.value,
        "/accounts/certificate",
        "Resolution certificate",
    )?;
    if selected.market.to_string() != market
        || selected.terminal_certificate.to_string() != resolution_certificate
        || require_pubkey_v1(&evidence.value, "/market", "payout Market")? != market
        || require_pubkey_v1(&evidence.value, "/owner", "payout owner")?
            != selected.owner.to_string()
        || require_pubkey_v1(&evidence.value, "/recipient", "payout recipient")?
            != selected.recipient.to_string()
        || require_string_v1(&evidence.value, "/cluster", "payout cluster")? != "owned-loopback"
        || require_sha256_v1(
            require_string_v1(&evidence.value, "/inputSha256", "payout input digest")?,
            "payout input digest",
        )? != input.sha256
    {
        return Err(Error::new(
            "wallet payout input/evidence did not join the exact Series Market, Resolution certificate, and input digest",
        ));
    }
    require_pubkey_v1(&evidence.value, "/feePayer", "payout fee payer")?;
    let evidence_sha256 = require_sha256_v1(
        require_string_v1(&evidence.value, "/evidenceSha256", "payout evidence digest")?,
        "payout evidence digest",
    )?;
    let poststates = evidence
        .value
        .pointer("/poststates")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("payout evidence omitted finalized poststates"))?;
    if poststates.is_empty() {
        return Err(Error::new(
            "payout evidence carried no finalized poststates",
        ));
    }
    for state in poststates {
        let state = state
            .as_object()
            .ok_or_else(|| Error::new("payout poststate was not an object"))?;
        require_pubkey_member_v1(state, "address", "payout poststate address")?;
        require_pubkey_member_v1(state, "owner", "payout poststate owner")?;
        require_sha256_v1(
            require_nonempty_string_v1(state.get("dataSha256"), "payout poststate digest")?,
            "payout poststate digest",
        )?;
    }
    Ok(PayoutReceiptV1 {
        signature: require_nonempty_string_v1(
            evidence.value.pointer("/signature"),
            "payout signature",
        )?
        .to_owned(),
        finalized_slot: require_positive_u64_v1(&evidence.value, "/finalizedSlot", "payout slot")?,
        fee_lamports: require_positive_u64_v1(&evidence.value, "/feeLamports", "payout fee")?,
        compute_units_consumed: require_positive_u64_v1(
            &evidence.value,
            "/computeUnitsConsumed",
            "payout compute",
        )?,
        recipient: require_pubkey_v1(&evidence.value, "/recipient", "payout recipient")?,
        payout: require_string_v1(&evidence.value, "/payout", "payout atoms")?.to_owned(),
        evidence_sha256,
    })
}

fn refuse_terminal_artifact_v1(artifact: &ArtifactV1, terminal: bool) -> Result<()> {
    let identity = artifact
        .value
        .get("schema")
        .and_then(Value::as_str)
        .or_else(|| artifact.value.get("format").and_then(Value::as_str))
        .unwrap_or("missing");
    let direct = if terminal {
        DIRECT_TERMINAL_COMPLETION_SCHEMA_V1
    } else {
        DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1
    };
    let expected = if terminal {
        FUTURE_SERIES_TERMINAL_SCHEMA_V1
    } else {
        FUTURE_SERIES_AGGREGATE_SCHEMA_V1
    };
    if identity == direct {
        return Err(Error::new(format!(
            "REFUSED: {identity} contains Direct begin/close/handoff authority and cannot retire a Series root; an actual Series producer must own {expected}"
        )));
    }
    Err(Error::new(format!(
        "REFUSED: terminal artifact schema {identity} is not an admitted Series completion; expected a checked producer for {expected}"
    )))
}

fn receipt_from_v1(
    object: &serde_json::Map<String, Value>,
    kind: &'static str,
    disposition: &'static str,
    refusal: Option<u64>,
    rollback: bool,
) -> Result<OccurrenceReceiptV1> {
    let observed_refusal = object.get("refusalCode").and_then(Value::as_u64);
    if observed_refusal != refusal {
        return Err(Error::new(format!(
            "{kind} disposition or exact refusal changed"
        )));
    }
    Ok(OccurrenceReceiptV1 {
        world: "series-consume",
        kind,
        signature: require_nonempty_string_v1(object.get("signature"), "occurrence signature")?
            .to_owned(),
        finalized_slot: require_positive_u64_member_v1(object, "finalizedSlot", "occurrence slot")?,
        compute_units_consumed: require_positive_u64_member_v1(
            object,
            "computeUnitsConsumed",
            "occurrence compute",
        )?,
        fee_lamports: require_positive_u64_member_v1(object, "feeLamports", "occurrence fee")?,
        disposition,
        refusal_code: refusal,
        evidence_sha256: require_sha256_v1(
            require_nonempty_string_v1(object.get("evidenceSha256"), "occurrence evidence")?,
            "occurrence evidence",
        )?,
        writable_lamports_before: require_u64_member_v1(
            object,
            "writableLamportsBefore",
            "occurrence prestate",
        )?,
        writable_lamports_after: require_u64_member_v1(
            object,
            "writableLamportsAfter",
            "occurrence poststate",
        )?,
        conservation_exact: true,
        rollback_byte_exact: rollback.then_some(true),
        transaction_fee_only_balance_change: rollback
            .then(|| {
                require_bool_member_v1(
                    object,
                    "transactionFeeOnlyBalanceChange",
                    true,
                    "occurrence fee accounting",
                )
            })
            .transpose()?,
    })
}

fn receipt_from_expiry_v1(
    object: &serde_json::Map<String, Value>,
    kind: &'static str,
    disposition: &'static str,
    refusal: Option<u64>,
    before: u64,
    after: u64,
    rollback: bool,
) -> Result<OccurrenceReceiptV1> {
    if object.get("disposition").and_then(Value::as_str) != Some(disposition)
        || object.get("refusalCode").and_then(Value::as_u64) != refusal
    {
        return Err(Error::new(format!(
            "expiry {kind} disposition or refusal changed"
        )));
    }
    Ok(OccurrenceReceiptV1 {
        world: match kind {
            "successor" => "series-expiry-successor",
            "wrong-address" => "series-expiry-wrong-address",
            "sealed-predecessor" => "series-expiry-sealed-predecessor",
            _ => "series-expiry-unknown",
        },
        kind,
        signature: require_nonempty_string_v1(object.get("signature"), "expiry signature")?
            .to_owned(),
        finalized_slot: require_positive_u64_member_v1(object, "finalizedSlot", "expiry slot")?,
        compute_units_consumed: require_positive_u64_member_v1(
            object,
            "computeUnitsConsumed",
            "expiry compute",
        )?,
        fee_lamports: require_positive_u64_member_v1(object, "feeLamports", "expiry fee")?,
        disposition,
        refusal_code: refusal,
        evidence_sha256: require_sha256_v1(
            require_nonempty_string_v1(object.get("evidenceSha256"), "expiry evidence digest")?,
            "expiry evidence digest",
        )?,
        writable_lamports_before: before,
        writable_lamports_after: after,
        conservation_exact: true,
        rollback_byte_exact: rollback.then_some(true),
        transaction_fee_only_balance_change: Some(require_bool_member_v1(
            object,
            "transactionFeeOnlyBalanceChange",
            rollback,
            "expiry fee accounting",
        )?),
    })
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut consume = None;
    let mut expiry = None;
    let mut resolution_input = None;
    let mut resolution_checkpoint = None;
    let mut wallet_input = None;
    let mut payout = None;
    let mut terminal = None;
    let mut aggregate = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--consume-checkpoint" => &mut consume,
            "--expiry-checkpoint" => &mut expiry,
            "--resolution-input" => &mut resolution_input,
            "--resolution-checkpoint" => &mut resolution_checkpoint,
            "--wallet-input" => &mut wallet_input,
            "--payout-evidence" => &mut payout,
            "--terminal-completion" => &mut terminal,
            "--aggregate-completion" => &mut aggregate,
            "--output" => &mut output,
            _ => return Err(Error::new(format!("unknown argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    Ok(ArgumentsV1 {
        consume_checkpoint: absolute_required_v1(consume, "--consume-checkpoint")?,
        expiry_checkpoint: absolute_required_v1(expiry, "--expiry-checkpoint")?,
        resolution_input: absolute_required_v1(resolution_input, "--resolution-input")?,
        resolution_checkpoint: absolute_required_v1(
            resolution_checkpoint,
            "--resolution-checkpoint",
        )?,
        wallet_input: absolute_required_v1(wallet_input, "--wallet-input")?,
        payout_evidence: absolute_required_v1(payout, "--payout-evidence")?,
        terminal_completion: absolute_optional_v1(terminal, "--terminal-completion")?,
        aggregate_completion: absolute_optional_v1(aggregate, "--aggregate-completion")?,
        output: absolute_required_v1(output, "--output")?,
    })
}

fn absolute_required_v1(value: Option<String>, label: &str) -> Result<PathBuf> {
    absolute_optional_v1(
        Some(value.ok_or_else(|| Error::new(format!("{label} is required")))?),
        label,
    )?
    .ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute_optional_v1(value: Option<String>, label: &str) -> Result<Option<PathBuf>> {
    value
        .map(|value| {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(Error::new(format!("{label} must be an absolute path")));
            }
            Ok(path)
        })
        .transpose()
}

fn read_artifact_v1(path: &PathBuf, label: &str) -> Result<ArtifactV1> {
    let bytes = fs::read(path)
        .map_err(|error| Error::new(format!("read {label} {}: {error}", path.display())))?;
    let value = parse_json_without_duplicate_keys_v1(&bytes)?;
    if !value.is_object() {
        return Err(Error::new(format!("{label} was not a JSON object")));
    }
    Ok(ArtifactV1 {
        path: path.clone(),
        sha256: hex(&Sha256::digest(&bytes)),
        value,
    })
}

fn binding_v1(
    role: &'static str,
    artifact: &ArtifactV1,
    identity: &str,
) -> Result<ArtifactBindingV1> {
    let field = if artifact.value.get("schema").is_some() {
        "schema"
    } else {
        "format"
    };
    require_identity_v1(artifact, field, identity)?;
    Ok(ArtifactBindingV1 {
        role,
        schema: identity.to_owned(),
        path: artifact.path.display().to_string(),
        sha256: artifact.sha256.clone(),
    })
}

fn require_identity_v1(artifact: &ArtifactV1, field: &str, expected: &str) -> Result<()> {
    if artifact.value.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(Error::new(format!(
            "{} identity is not exact {expected}",
            artifact.path.display()
        )));
    }
    Ok(())
}

fn require_object_v1<'a>(
    value: &'a Value,
    pointer: &str,
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .pointer(pointer)
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new(format!("{label} was not an object")))
}

fn require_string_v1<'a>(value: &'a Value, pointer: &str, label: &str) -> Result<&'a str> {
    require_nonempty_string_v1(value.pointer(pointer), label)
}

fn require_nonempty_string_v1<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::new(format!("{label} was not a nonempty string")))
}

fn require_pubkey_v1(value: &Value, pointer: &str, label: &str) -> Result<String> {
    let text = require_string_v1(value, pointer, label)?;
    text.parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    Ok(text.to_owned())
}

fn require_pubkey_member_v1(
    object: &serde_json::Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<String> {
    let text = require_nonempty_string_v1(object.get(field), label)?;
    text.parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    Ok(text.to_owned())
}

fn require_bool_v1(value: &Value, pointer: &str, expected: bool, label: &str) -> Result<()> {
    if value.pointer(pointer).and_then(Value::as_bool) != Some(expected) {
        return Err(Error::new(format!("{label} was not {expected}")));
    }
    Ok(())
}

fn require_bool_member_v1(
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: bool,
    label: &str,
) -> Result<bool> {
    let observed = object.get(field).and_then(Value::as_bool);
    if observed != Some(expected) {
        return Err(Error::new(format!("{label} was not {expected}")));
    }
    Ok(expected)
}

fn require_u64_member_v1(
    object: &serde_json::Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("{label} was not a u64")))
}

fn require_positive_u64_member_v1(
    object: &serde_json::Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<u64> {
    let value = require_u64_member_v1(object, field, label)?;
    if value == 0 {
        return Err(Error::new(format!("{label} must be positive")));
    }
    Ok(value)
}

fn require_positive_u64_v1(value: &Value, pointer: &str, label: &str) -> Result<u64> {
    let value = value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("{label} was not a u64")))?;
    if value == 0 {
        return Err(Error::new(format!("{label} must be positive")));
    }
    Ok(value)
}

fn require_sha256_v1(value: &str, label: &str) -> Result<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(format!("{label} was not lowercase SHA-256")));
    }
    Ok(value.to_owned())
}

fn prefix_found_digest_v2(found: &SeriesPrefixFoundBindingV2) -> Result<String> {
    let mut projected = found.clone();
    projected.found_sha256.clear();
    Ok(hex(&Sha256::digest(serde_json::to_vec(&projected)?)))
}

fn ledger_digest_v2(ledger: &SeriesLifecyclePrefixLedgerV2) -> Result<String> {
    prefix_ledger_value_digest_v2(&serde_json::to_value(ledger)?)
}

fn prefix_ledger_value_digest_v2(value: &Value) -> Result<String> {
    let mut projected = value.clone();
    let object = projected
        .as_object_mut()
        .ok_or_else(|| Error::new("Series prefix ledger was not an object"))?;
    if object
        .insert("ledgerSha256".to_owned(), Value::String(String::new()))
        .is_none()
    {
        return Err(Error::new("Series prefix ledger omitted its digest"));
    }
    Ok(hex(&Sha256::digest(serde_json::to_vec(&projected)?)))
}

fn json_bytes_v1<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_idempotent_v1(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => {
            return Err(Error::new(format!(
                "Series prefix output {} already exists with different bytes",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::new(format!("read {}: {error}", path.display()))),
    }
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(value: Value) -> ArtifactV1 {
        ArtifactV1 {
            path: PathBuf::from("/tmp/series-test.json"),
            sha256: hex(&[7; 32]),
            value,
        }
    }

    /// The local-validator evidence directory, from ANY manifest that links
    /// this file.
    ///
    /// It used to be `CARGO_MANIFEST_DIR/../evidence`, which is one root when
    /// the compiling crate is the successor and a different, absent one when it
    /// is anything else. `tools/gauntlet/journey` began linking the whole
    /// successor module set on 2026-09-06 and these three tests went red there
    /// while staying green here -- a fixture path with two possible roots, and
    /// the second one only appeared when a second crate compiled the file. This
    /// walks up to the repository root and joins the tracked path, so there is
    /// one root however many crates link this module.
    fn local_validator_evidence_root_v1() -> PathBuf {
        let mut directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            if directory.join("AGENTS.md").is_file() && directory.join("Cargo.lock").is_file() {
                return directory.join("tools/local-validator/bootstrap/evidence");
            }
            if !directory.pop() {
                panic!(
                    "no repository root above {}: this test reads a tracked fixture and cannot \
                     invent one",
                    env!("CARGO_MANIFEST_DIR")
                );
            }
        }
    }

    #[test]
    fn direct_terminal_and_aggregate_schemas_are_never_series_evidence() {
        let terminal = artifact(serde_json::json!({
            "schema": DIRECT_TERMINAL_COMPLETION_SCHEMA_V1
        }));
        let aggregate = artifact(serde_json::json!({
            "schema": DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1
        }));
        assert!(
            refuse_terminal_artifact_v1(&terminal, true)
                .unwrap_err()
                .to_string()
                .contains("Direct begin/close/handoff")
        );
        assert!(
            refuse_terminal_artifact_v1(&aggregate, false)
                .unwrap_err()
                .to_string()
                .contains("Direct begin/close/handoff")
        );
    }

    #[test]
    fn unknown_terminal_schema_refuses_instead_of_becoming_a_projection() {
        let unknown = artifact(serde_json::json!({"schema": "third-schema"}));
        assert!(
            refuse_terminal_artifact_v1(&unknown, true)
                .unwrap_err()
                .to_string()
                .contains(FUTURE_SERIES_TERMINAL_SCHEMA_V1)
        );
    }

    #[test]
    fn lifecycle_ledger_digest_excludes_only_its_own_digest_field() {
        let mut found = SeriesPrefixFoundBindingV2 {
            root: Pubkey::new_unique().to_string(),
            parent_market: Pubkey::new_unique().to_string(),
            parent_generation: 1,
            template: hex(&[3; 32]),
            signature: solana_sdk::signature::Signature::new_unique().to_string(),
            finalized_slot: 1,
            packet_sha256: hex(&[4; 32]),
            poststate_sha256: hex(&[5; 32]),
            found_sha256: String::new(),
        };
        found.found_sha256 = prefix_found_digest_v2(&found).unwrap();
        let mut ledger = SeriesLifecyclePrefixLedgerV2 {
            schema: LEDGER_SCHEMA_V2,
            status: "artifact-bound-prefix-through-wallet-payout",
            complete: false,
            same_ledger_recurring_sequence_proven: false,
            cluster: "owned-loopback",
            family: "series",
            market: Pubkey::new_unique().to_string(),
            permit: Pubkey::new_unique().to_string(),
            found,
            source_artifacts: Vec::new(),
            occurrence_path_evidence: Vec::new(),
            resolution: ResolutionReceiptV1 {
                input_sha256: hex(&[1; 32]),
                receipt_count: 0,
                signatures: Vec::new(),
                first_finalized_slot: 0,
                last_finalized_slot: 0,
                fee_lamports: 0,
                compute_units_consumed: 0,
            },
            payout: PayoutReceiptV1 {
                signature: "signature".into(),
                finalized_slot: 1,
                fee_lamports: 1,
                compute_units_consumed: 1,
                recipient: Pubkey::new_unique().to_string(),
                payout: "1".into(),
                evidence_sha256: hex(&[2; 32]),
            },
            evidence_ensemble_transaction_fees_lamports: 1,
            evidence_ensemble_compute_units_consumed: 1,
            temporary_protocol_state_closed: false,
            missing_authority: MissingAuthorityV1 {
                first_missing_transition: "series-second-occurrence-commit",
                missing_transitions: vec![
                    "series-second-occurrence-commit",
                    "series-terminal-source-and-funding-close",
                    "series-root-retirement",
                ],
                required_terminal_schema: FUTURE_SERIES_TERMINAL_SCHEMA_V1,
                required_aggregate_schema: FUTURE_SERIES_AGGREGATE_SCHEMA_V1,
                direct_terminal_schema_refused: DIRECT_TERMINAL_COMPLETION_SCHEMA_V1,
                direct_aggregate_schema_refused: DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1,
                direct_handoff_allowed: false,
            },
            ledger_sha256: String::new(),
        };
        let first = ledger_digest_v2(&ledger).unwrap();
        ledger.ledger_sha256 = first.clone();
        assert_eq!(ledger_digest_v2(&ledger).unwrap(), first);
        ledger.evidence_ensemble_compute_units_consumed = 2;
        assert_ne!(ledger_digest_v2(&ledger).unwrap(), first);
    }

    #[test]
    fn committed_physical_occurrence_checkpoints_authenticate_exactly() {
        let evidence_root = local_validator_evidence_root_v1();
        let consume = read_artifact_v1(
            &evidence_root.join("series-consume-replay-2026-08-31.json"),
            "committed Series consume checkpoint",
        )
        .unwrap();
        let expiry = read_artifact_v1(
            &evidence_root.join("series-permit-expiry-2026-08-31.json"),
            "committed Series expiry checkpoint",
        )
        .unwrap();

        let consume = authenticate_consume_v1(&consume).unwrap();
        assert!(consume.found.is_none());
        let expiry = authenticate_expiry_v1(&expiry, &consume.permit).unwrap();
        assert_eq!(consume.receipts.len(), 2);
        assert_eq!(consume.receipts[0].disposition, "found");
        assert_eq!(
            consume.receipts[0].transaction_fee_only_balance_change,
            None
        );
        assert_eq!(
            consume.receipts[1].refusal_code,
            Some(REPLAY_REFUSAL_MARKET)
        );
        assert_eq!(expiry.receipts.len(), 3);
        assert_eq!(expiry.receipts[0].writable_lamports_before, 6_904_320);
        assert_eq!(expiry.receipts[0].writable_lamports_after, 6_904_320);
        assert!(expiry.receipts[1..].iter().all(|receipt| {
            receipt.refusal_code == Some(EXPIRY_REFUSAL_INFRASTRUCTURE)
                && receipt.rollback_byte_exact == Some(true)
                && receipt.transaction_fee_only_balance_change == Some(true)
        }));
    }

    #[test]
    fn v2_consume_carries_exact_found_transaction_and_digest() {
        let evidence_root = local_validator_evidence_root_v1();
        let mut consume = read_artifact_v1(
            &evidence_root.join("series-consume-replay-2026-08-31.json"),
            "committed Series consume checkpoint",
        )
        .unwrap();
        consume.value["schema"] = Value::String(CONSUME_CHECKPOINT_SCHEMA_V2.to_owned());
        let market = consume.value["fixture"]["market"]
            .as_str()
            .unwrap()
            .to_owned();
        let signature = consume.value["accepted"]["signature"]
            .as_str()
            .unwrap()
            .to_owned();
        let finalized_slot = consume.value["accepted"]["finalizedSlot"].as_u64().unwrap();
        let mut found = SeriesPrefixFoundBindingV2 {
            root: Pubkey::new_unique().to_string(),
            parent_market: market,
            parent_generation: 7,
            template: hex(&[3; 32]),
            signature,
            finalized_slot,
            packet_sha256: hex(&[4; 32]),
            poststate_sha256: hex(&[5; 32]),
            found_sha256: String::new(),
        };
        found.found_sha256 = prefix_found_digest_v2(&found).unwrap();
        consume.value["found"] = serde_json::to_value(&found).unwrap();

        let admitted = authenticate_consume_v1(&consume).unwrap();
        assert_eq!(
            admitted.found.as_ref().unwrap().found_sha256,
            found.found_sha256
        );

        consume.value["found"]["parentGeneration"] = Value::from(8_u64);
        assert!(
            authenticate_consume_v1(&consume)
                .err()
                .expect("mutated Found binding must refuse")
                .to_string()
                .contains("Found binding changed")
        );
    }

    #[test]
    fn terminal_consumer_reauthenticates_v2_prefix_bytes_and_found() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dclutch-series-prefix-consumer-{}-{nonce}.json",
            std::process::id()
        ));
        let consume_path = std::env::temp_dir().join(format!(
            "dclutch-series-prefix-consume-source-{}-{nonce}.json",
            std::process::id()
        ));
        let evidence_root = local_validator_evidence_root_v1();
        let mut consume = read_artifact_v1(
            &evidence_root.join("series-consume-replay-2026-08-31.json"),
            "committed Series consume checkpoint",
        )
        .unwrap();
        consume.value["schema"] = Value::String(CONSUME_CHECKPOINT_SCHEMA_V2.to_owned());
        let parent_market = consume.value["fixture"]["market"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut found = SeriesPrefixFoundBindingV2 {
            root: Pubkey::new_unique().to_string(),
            parent_market: parent_market.clone(),
            parent_generation: 7,
            template: hex(&[3; 32]),
            signature: consume.value["accepted"]["signature"]
                .as_str()
                .unwrap()
                .to_owned(),
            finalized_slot: consume.value["accepted"]["finalizedSlot"].as_u64().unwrap(),
            packet_sha256: hex(&[4; 32]),
            poststate_sha256: hex(&[5; 32]),
            found_sha256: String::new(),
        };
        found.found_sha256 = prefix_found_digest_v2(&found).unwrap();
        consume.value["found"] = serde_json::to_value(&found).unwrap();
        let consume_bytes = serde_json::to_vec_pretty(&consume.value).unwrap();
        fs::write(&consume_path, &consume_bytes).unwrap();
        let consume_sha = hex(&Sha256::digest(&consume_bytes));
        let mut ledger = SeriesLifecyclePrefixLedgerV2 {
            schema: LEDGER_SCHEMA_V2,
            status: "artifact-bound-prefix-through-wallet-payout",
            complete: false,
            same_ledger_recurring_sequence_proven: false,
            cluster: "owned-loopback",
            family: "series",
            market: parent_market,
            permit: Pubkey::new_unique().to_string(),
            found: found.clone(),
            source_artifacts: vec![ArtifactBindingV1 {
                role: "series-consume",
                schema: CONSUME_CHECKPOINT_SCHEMA_V2.to_owned(),
                path: consume_path.display().to_string(),
                sha256: consume_sha,
            }],
            occurrence_path_evidence: Vec::new(),
            resolution: ResolutionReceiptV1 {
                input_sha256: hex(&[1; 32]),
                receipt_count: 0,
                signatures: Vec::new(),
                first_finalized_slot: 0,
                last_finalized_slot: 0,
                fee_lamports: 0,
                compute_units_consumed: 0,
            },
            payout: PayoutReceiptV1 {
                signature: "signature".into(),
                finalized_slot: 1,
                fee_lamports: 1,
                compute_units_consumed: 1,
                recipient: Pubkey::new_unique().to_string(),
                payout: "1".into(),
                evidence_sha256: hex(&[2; 32]),
            },
            evidence_ensemble_transaction_fees_lamports: 1,
            evidence_ensemble_compute_units_consumed: 1,
            temporary_protocol_state_closed: false,
            missing_authority: MissingAuthorityV1 {
                first_missing_transition: "series-second-occurrence-commit",
                missing_transitions: vec!["series-second-occurrence-commit"],
                required_terminal_schema: FUTURE_SERIES_TERMINAL_SCHEMA_V1,
                required_aggregate_schema: FUTURE_SERIES_AGGREGATE_SCHEMA_V1,
                direct_terminal_schema_refused: DIRECT_TERMINAL_COMPLETION_SCHEMA_V1,
                direct_aggregate_schema_refused: DIRECT_AGGREGATE_COMPLETION_SCHEMA_V1,
                direct_handoff_allowed: false,
            },
            ledger_sha256: String::new(),
        };
        ledger.ledger_sha256 = ledger_digest_v2(&ledger).unwrap();
        let bytes = json_bytes_v1(&ledger).unwrap();
        fs::write(&path, &bytes).unwrap();
        let file_sha = hex(&Sha256::digest(&bytes));
        let admitted = read_authenticated_series_prefix_found_v2(&path, &file_sha).unwrap();
        assert_eq!(admitted.found_sha256, found.found_sha256);

        let mut hostile: Value = serde_json::from_slice(&bytes).unwrap();
        hostile["found"]["parentGeneration"] = Value::from(8_u64);
        let hostile = serde_json::to_vec_pretty(&hostile).unwrap();
        fs::write(&path, &hostile).unwrap();
        let hostile_sha = hex(&Sha256::digest(&hostile));
        assert!(read_authenticated_series_prefix_found_v2(&path, &hostile_sha).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_file(consume_path).unwrap();
    }
}
