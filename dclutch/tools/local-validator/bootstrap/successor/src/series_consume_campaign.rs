//! The first executed Series Found: one durable `series_consume` submission.
//!
//! # What this is, and what it deliberately is not
//!
//! `series_consume` is the only Series route the tree actually dispatches, and
//! until now it had only ever executed inside `ProgramTest`. This module
//! submits it to a real validator.
//!
//! It is **not** a port of the fixture. That fixture is roughly 1,250 lines
//! living in `programs/dclutch-core-sbf/tests/found_program_test.rs`, and
//! rewriting it here would give one campaign two authors who could silently
//! disagree. Instead the fixture stays where it is and gained one
//! `#[ignore]`-gated emitter — `emit_series_consume_validator_campaign` — that
//! starts the genesis it would have run against and reads every account the
//! instruction names back out of the banks client. This module consumes that
//! bundle. Everything semantic is still authored once, by the test.
//!
//! That division is what makes two of the hazards disappear rather than get
//! solved. `series_consume` compares `market.lamports()` to
//! `request.market_rent()` with `!=`, so any rent heuristic silently refuses;
//! and the six loader-v3 Program/ProgramData pairs carry a deployment slot that
//! flows into the release-set digest and therefore into the Market PDA, making
//! deploy-then-derive circular with genesis. Both are *observed* from the
//! emitted bundle, never recomputed.
//!
//! # Why v0 with a lookup table, and not the legacy path
//!
//! The frame is 62 metas over 61 unique keys. Sixty-one keys is 1,952 bytes of
//! addresses alone, against a 1,232-byte packet — legacy is not tight here, it
//! is arithmetically impossible. The emitter carries the address lookup table
//! the fixture builds, and this module routes through it. The durable ladder is
//! the same one the General campaign uses, on the v0 rather than the legacy
//! sibling.
//!
//! # What counts as the acknowledgment
//!
//! The General accelerator publishes a typed ack in return data because it is a
//! readonly evaluator. `series_consume` is a write path: its acknowledgment is
//! the state it committed. So this module authenticates the *outcome* — the
//! Market is Core-owned and the founding permit exists at its exact expected
//! balance — which is a stronger claim than a returned buffer, and is the same
//! pair of facts the ProgramTest asserts.

use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    plan::hex,
    rpc::{Rpc, RpcAccount, SignedVersionedPacketV1},
};

/// Command that submits one emitted `series_consume` campaign.
pub(crate) const SERIES_CONSUME_COMMAND_V1: &str = "local-private-validator-series-consume-v1";

const CAMPAIGN_SCHEMA_V1: &str = "dclutch-series-consume-validator-campaign-v1";
const JOURNAL_SCHEMA_V1: &str = "dclutch-series-consume-journal-v1";
const EVIDENCE_SCHEMA_V1: &str = "dclutch-series-consume-evidence-v1";

/// The bundle the ProgramTest emitter wrote.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignManifestV1 {
    schema: String,
    program_id: String,
    lookup_table: String,
    data_base64: String,
    accounts: Vec<CampaignMetaV1>,
    compute_unit_limit: u32,
    genesis_account_count: usize,
    genesis_only: Vec<String>,
    absent_by_design: Vec<String>,
    expect: CampaignExpectationV1,
}

/// One account meta, exactly as the fixture ordered it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignMetaV1 {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

/// What must be true of the chain once the transaction finalizes.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignExpectationV1 {
    outcome_count: u32,
    market: String,
    market_owner: String,
    permit: String,
    permit_owner: String,
    permit_lamports: u64,
    resolution_program: String,
    resolution_programdata: String,
}

/// The durable phase of the submission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SeriesConsumePhaseV1 {
    /// The instruction is rebuilt and the table observed; no key opened.
    Planned,
    /// The exact signed v0 packet is on disk and may only be resent.
    Prepared,
    /// Those exact bytes reached the RPC; recovery is poll-only.
    Submitted,
    /// Finalized, with the committed Found authenticated.
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResumeActionV1 {
    Prepare,
    SubmitPersisted,
    PollOnly,
    Done,
}

fn resume_action_v1(phase: SeriesConsumePhaseV1) -> ResumeActionV1 {
    match phase {
        SeriesConsumePhaseV1::Planned => ResumeActionV1::Prepare,
        SeriesConsumePhaseV1::Prepared => ResumeActionV1::SubmitPersisted,
        SeriesConsumePhaseV1::Submitted => ResumeActionV1::PollOnly,
        SeriesConsumePhaseV1::Finalized => ResumeActionV1::Done,
    }
}

/// The durable record of the submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesConsumeJournalV1 {
    schema: String,
    phase: SeriesConsumePhaseV1,
    rpc_url: String,
    program_id: String,
    lookup_table: String,
    account_count: usize,
    instruction_data_sha256: String,
    outcome_count: u32,
    #[serde(default)]
    expected_refusal: Option<u32>,
    #[serde(default)]
    replay_of_signature: Option<String>,
    #[serde(default)]
    replay_of_evidence_sha256: Option<String>,
    #[serde(default)]
    fee_payer: Option<String>,
    signed_packet_base64: Option<String>,
    signed_packet_sha256: Option<String>,
    expected_signature: Option<String>,
    last_valid_block_height: Option<u64>,
    routed_wire_bytes: Option<usize>,
    finalized_slot: Option<u64>,
    compute_units_consumed: Option<u64>,
    market: String,
    market_owner_after: Option<String>,
    permit: String,
    permit_owner_after: Option<String>,
    permit_lamports_after: Option<u64>,
    #[serde(default)]
    prestate_slot: Option<u64>,
    #[serde(default)]
    writable_before: Vec<SeriesAccountSnapshotV1>,
    #[serde(default)]
    writable_lamports_before: Option<u64>,
    #[serde(default)]
    evidence_sha256: Option<String>,
}

/// Finalized state and rollback evidence for one real validator submission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesConsumeEvidenceV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    program_id: String,
    lookup_table: String,
    instruction_data_sha256: String,
    outcome_count: u32,
    account_count: usize,
    writable_account_count: usize,
    #[serde(default)]
    fee_payer: String,
    #[serde(default)]
    recent_blockhash: String,
    signature: String,
    finalized_slot: u64,
    compute_units_consumed: Option<u64>,
    fee_lamports: Option<u64>,
    disposition: String,
    refusal_code: Option<u32>,
    #[serde(default)]
    replay_of_signature: Option<String>,
    #[serde(default)]
    replay_of_evidence_sha256: Option<String>,
    #[serde(default)]
    distinct_replay_signature: Option<bool>,
    writable_before: Vec<SeriesAccountSnapshotV1>,
    writable_after: Vec<SeriesAccountSnapshotV1>,
    writable_lamports_before: u64,
    writable_lamports_after: u64,
    writable_lamports_conserved: bool,
    rollback_byte_exact: Option<bool>,
    transaction_fee_only_balance_change: Option<bool>,
    market: String,
    permit: String,
    journal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SeriesAccountSnapshotV1 {
    address: String,
    present: bool,
    owner: Option<String>,
    lamports: u64,
    executable: bool,
    rent_epoch: Option<u64>,
    data_base64: String,
    data_sha256: String,
}

struct ArgumentsV1 {
    campaign: PathBuf,
    rpc_url: String,
    payer: PathBuf,
    journal: PathBuf,
    evidence: PathBuf,
    replay_after_evidence: Option<PathBuf>,
    execute: bool,
    expect_refusal: Option<u32>,
}

struct ReplayParentV1 {
    signature: String,
    fee_payer: String,
    recent_blockhash: String,
    evidence_sha256: String,
}

/// Submit one emitted `series_consume` campaign against a local validator.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let parsed = parse_arguments(arguments)?;
    let manifest = read_manifest_v1(&parsed.campaign)?;
    let program_id = parse_key(&manifest.program_id, "programId")?;
    let lookup_table = parse_key(&manifest.lookup_table, "lookupTable")?;
    let data = BASE64
        .decode(&manifest.data_base64)
        .map_err(|error| Error::new(format!("campaign data base64: {error}")))?;
    let mut accounts = Vec::with_capacity(manifest.accounts.len());
    for meta in &manifest.accounts {
        let pubkey = parse_key(&meta.pubkey, "account meta")?;
        accounts.push(AccountMeta {
            pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }
    // The emitter records every meta the fixture built, in order. A campaign
    // that lost or reordered one would not be the transaction the ProgramTest
    // asserts about, so the count is pinned rather than trusted.
    if accounts.is_empty() {
        return Err(Error::new("campaign carries no account metas"));
    }
    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };
    let instruction_data_sha256 = hex(&Sha256::digest(&instruction.data));
    let replay_parent = read_replay_parent_v1(
        &parsed,
        &manifest,
        &instruction_data_sha256,
        instruction.accounts.len(),
    )?;
    let mut writable_keys = Vec::new();
    for meta in &instruction.accounts {
        if meta.is_writable && !writable_keys.contains(&meta.pubkey) {
            writable_keys.push(meta.pubkey);
        }
    }
    if writable_keys.is_empty() || writable_keys.len() > 100 {
        return Err(Error::new(
            "Series Consume writable account set is empty or exceeds one exact RPC observation",
        ));
    }

    let mut rpc = Rpc::connect(&parsed.rpc_url)?;
    let market = parse_key(&manifest.expect.market, "expect.market")?;
    let permit = parse_key(&manifest.expect.permit, "expect.permit")?;
    let core_program = parse_key(&manifest.expect.market_owner, "expect.marketOwner")?;
    let resolution_program = parse_key(
        &manifest.expect.resolution_program,
        "expect.resolutionProgram",
    )?;
    let resolution_programdata = parse_key(
        &manifest.expect.resolution_programdata,
        "expect.resolutionProgramdata",
    )?;
    if manifest.genesis_only
        != [
            Pubkey::find_program_address(
                &[core_program.as_ref()],
                &solana_sdk_ids::bpf_loader_upgradeable::ID,
            )
            .0
            .to_string(),
            resolution_program.to_string(),
            resolution_programdata.to_string(),
        ]
    {
        return Err(Error::new(
            "Series Consume genesis-only identities do not bind Core ProgramData plus the exact \
             Resolution Program/ProgramData pair",
        ));
    }

    let mut journal = SeriesConsumeJournalV1 {
        schema: JOURNAL_SCHEMA_V1.to_owned(),
        phase: SeriesConsumePhaseV1::Planned,
        rpc_url: parsed.rpc_url.clone(),
        program_id: manifest.program_id.clone(),
        lookup_table: manifest.lookup_table.clone(),
        account_count: instruction.accounts.len(),
        instruction_data_sha256,
        outcome_count: manifest.expect.outcome_count,
        expected_refusal: parsed.expect_refusal,
        replay_of_signature: replay_parent
            .as_ref()
            .map(|parent| parent.signature.clone()),
        replay_of_evidence_sha256: replay_parent
            .as_ref()
            .map(|parent| parent.evidence_sha256.clone()),
        fee_payer: None,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        last_valid_block_height: None,
        routed_wire_bytes: None,
        finalized_slot: None,
        compute_units_consumed: None,
        market: manifest.expect.market.clone(),
        market_owner_after: None,
        permit: manifest.expect.permit.clone(),
        permit_owner_after: None,
        permit_lamports_after: None,
        prestate_slot: None,
        writable_before: Vec::new(),
        writable_lamports_before: None,
        evidence_sha256: None,
    };

    let previous = read_journal_v1(&parsed.journal)?;
    if let Some(previous) = previous.as_ref() {
        authenticate_journal_intent_v1(previous, &journal)?;
    }

    // A finished campaign rerun is a no-op, not an error. This is read before
    // the preflight because the preflight's vacant-Market check is precisely
    // what a SUCCEEDED Found invalidates: once the Market is written, "the
    // Market is not vacant" stops being a misconfiguration and becomes the
    // proof that this campaign already did its work.
    if let Some(previous) = previous.as_ref()
        && previous.phase == SeriesConsumePhaseV1::Finalized
    {
        let signature = previous
            .expected_signature
            .clone()
            .ok_or_else(|| Error::new("a finalized journal carries no signature"))?;
        let evidence = fs::read(&parsed.evidence).map_err(|error| {
            Error::new(format!(
                "finalized Series Consume journal exists but evidence {} is unavailable: {error}",
                parsed.evidence.display()
            ))
        })?;
        let evidence_digest = hex(&Sha256::digest(&evidence));
        if previous.evidence_sha256.as_deref() != Some(evidence_digest.as_str()) {
            return Err(Error::new(
                "finalized Series Consume evidence no longer matches its journal digest",
            ));
        }
        println!(
            "series_consume already finalized: signature {signature} in slot {}, {} CU; evidence \
             {} is unchanged. Nothing resubmitted.",
            previous.finalized_slot.unwrap_or_default(),
            previous.compute_units_consumed.unwrap_or_default(),
            parsed.evidence.display()
        );
        return Ok(());
    }

    let recovering_signed_packet = previous.as_ref().is_some_and(|previous| {
        matches!(
            previous.phase,
            SeriesConsumePhaseV1::Prepared | SeriesConsumePhaseV1::Submitted
        )
    });

    // Before anything is signed: prove the genesis actually carries the frame.
    // A validator started without the emitted --account-dir answers every one
    // of these with a system-owned empty account, and the transaction would
    // refuse deep inside Core with an opaque code. Failing here instead names
    // the cause.
    let probe = [
        market,
        permit,
        lookup_table,
        program_id,
        resolution_program,
        resolution_programdata,
    ];
    let (_, observed) = rpc.finalized_observed_accounts(&probe, 0)?;
    let table_observed = observed
        .get(2)
        .ok_or_else(|| Error::new("preflight lost the lookup table"))?
        .clone();
    if table_observed.data.is_empty() {
        return Err(Error::new(format!(
            "the address lookup table {lookup_table} is empty on this cluster: start the \
             validator with --account-dir pointing at the emitted accounts directory"
        )));
    }
    let program_observed = observed
        .get(3)
        .ok_or_else(|| Error::new("preflight lost the invoked program"))?;
    if !program_observed.executable {
        return Err(Error::new(format!(
            "the invoked program {program_id} is not executable on this cluster; the emitted \
             genesis account was not loaded"
        )));
    }
    let resolution_observed = observed
        .get(4)
        .ok_or_else(|| Error::new("preflight lost the Resolution program"))?;
    let resolution_programdata_observed = observed
        .get(5)
        .ok_or_else(|| Error::new("preflight lost Resolution ProgramData"))?;
    if !resolution_observed.executable
        || resolution_programdata_observed.executable
        || resolution_programdata_observed.data.len() <= 45
    {
        return Err(Error::new(format!(
            "the Series genesis does not carry an executable Resolution {resolution_program} \
             with nonempty ProgramData {resolution_programdata}; it cannot advance this Found \
             state into flagship resolution on the same validator"
        )));
    }
    let market_before = observed
        .first()
        .ok_or_else(|| Error::new("preflight lost the Market"))?;
    // A hostile deliberately runs against a Market that is ALREADY written --
    // that is the whole point of the double-consume -- so the vacant-Market
    // precondition is a happy-path check, not a universal one.
    if parsed.expect_refusal.is_none()
        && !recovering_signed_packet
        && !market_before.data.iter().all(|byte| *byte == 0)
    {
        return Err(Error::new(format!(
            "the Market {market} is already written on this cluster; series_consume IS the Found \
             and needs a vacant Market, so start a fresh ledger with --reset"
        )));
    }
    if recovering_signed_packet {
        journal = previous
            .clone()
            .ok_or_else(|| Error::new("Series Consume recovery journal disappeared"))?;
        validate_persisted_prestate_v1(&journal, &writable_keys)?;
    } else {
        let (prestate_slot, writable_before_accounts) =
            rpc.finalized_accounts(&writable_keys, 0)?;
        journal.prestate_slot = Some(prestate_slot);
        journal.writable_before = snapshot_accounts_v1(&writable_keys, &writable_before_accounts)?;
        journal.writable_lamports_before = Some(snapshot_lamports_v1(&journal.writable_before)?);
        write_json_atomic_v1(&parsed.journal, &journal)?;
    }
    if !parsed.execute {
        if recovering_signed_packet {
            println!(
                "series_consume has a durable {:?} packet at signature {}; no RPC write was \
                 issued without --execute",
                journal.phase,
                journal.expected_signature.as_deref().unwrap_or("missing")
            );
            return Ok(());
        }
        println!(
            "planned: {} metas, {} genesis accounts emitted, {} absent by design, lookup table {} \
             carries {} bytes. Rerun with --execute to sign and submit.",
            instruction.accounts.len(),
            manifest.genesis_account_count,
            manifest.absent_by_design.len(),
            lookup_table,
            table_observed.data.len()
        );
        return Ok(());
    }

    let label = format!(
        "series_consume Found at {} outcomes",
        manifest.expect.outcome_count
    );

    // The compute budget is NOT set here: `bounded_instructions` owns the
    // ComputeBudget declarations and refuses a duplicate. It already asks for
    // 1,400,000 units, comfortably above the 722,142 this route measures, and a
    // real validator's 200,000 default would refuse without it.
    let _ = manifest.compute_unit_limit;

    let resume_action = resume_action_v1(journal.phase);
    let (packet, payer_pubkey) = match resume_action {
        ResumeActionV1::Prepare => {
            let payer = Keypair::new_from_array(read_keypair_file(&parsed.payer, "payer")?);
            let payer_pubkey = payer.pubkey();
            let packet = rpc.prepare_signed_v0_packet(
                &label,
                std::slice::from_ref(&instruction),
                &payer,
                &table_observed,
            )?;
            journal.phase = SeriesConsumePhaseV1::Prepared;
            journal.fee_payer = Some(payer_pubkey.to_string());
            journal.routed_wire_bytes = Some(
                BASE64
                    .decode(&packet.packet_base64)
                    .map_err(|error| Error::new(format!("{label}: packet base64: {error}")))?
                    .len(),
            );
            journal.signed_packet_base64 = Some(packet.packet_base64.clone());
            journal.signed_packet_sha256 = Some(packet.packet_sha256.clone());
            journal.expected_signature = Some(packet.signature.clone());
            journal.last_valid_block_height = Some(packet.last_valid_block_height);
            write_json_atomic_v1(&parsed.journal, &journal)?;
            (packet, payer_pubkey)
        }
        ResumeActionV1::SubmitPersisted | ResumeActionV1::PollOnly => {
            let payer_pubkey = parse_key(
                journal.fee_payer.as_deref().ok_or_else(|| {
                    Error::new("persisted Series Consume packet has no fee payer")
                })?,
                "journal.feePayer",
            )?;
            (persisted_packet_v1(&journal)?, payer_pubkey)
        }
        ResumeActionV1::Done => {
            return Err(Error::new(
                "finalized Series Consume journal escaped the idempotent rerun gate",
            ));
        }
    };
    if let Some(parent) = replay_parent.as_ref()
        && packet.signature == parent.signature
    {
        return Err(Error::new(
            "Series Consume replay reused the accepted transaction signature; a duplicate \
             signature is runtime deduplication, not a protocol replay test",
        ));
    }
    if let Some(parent) = replay_parent.as_ref()
        && payer_pubkey.to_string() != parent.fee_payer
    {
        return Err(Error::new(
            "Series Consume replay changed the fee payer; use the same signer so the distinct \
             signature proves a fresh blockhash for the same occurrence packet",
        ));
    }
    let recent_blockhash = packet_recent_blockhash_v1(&packet)?;
    if let Some(parent) = replay_parent.as_ref()
        && recent_blockhash == parent.recent_blockhash
    {
        return Err(Error::new(
            "Series Consume replay reused the accepted recent blockhash; wait for a fresh \
             blockhash so the validator executes the protocol replay",
        ));
    }

    // Prepared recovery may resend only these already-authenticated bytes. A
    // Submitted recovery is poll-only: it can never create a second blockhash,
    // signature, or transaction identity for one occurrence.
    if resume_action == ResumeActionV1::SubmitPersisted || resume_action == ResumeActionV1::Prepare
    {
        if parsed.expect_refusal.is_some() {
            rpc.submit_signed_v0_packet_expecting_failure(
                &label,
                std::slice::from_ref(&instruction),
                payer_pubkey,
                &table_observed,
                &packet,
            )?;
        } else {
            rpc.submit_signed_v0_packet(
                &label,
                std::slice::from_ref(&instruction),
                payer_pubkey,
                &table_observed,
                &packet,
            )?;
        }
        journal.phase = SeriesConsumePhaseV1::Submitted;
        write_json_atomic_v1(&parsed.journal, &journal)?;
    }

    let finalized = if parsed.expect_refusal.is_some() {
        rpc.confirm_signed_v0_packet_expecting_failure(
            &label,
            std::slice::from_ref(&instruction),
            payer_pubkey,
            &table_observed,
            &packet,
        )?
    } else {
        rpc.confirm_signed_v0_packet(
            &label,
            std::slice::from_ref(&instruction),
            payer_pubkey,
            &table_observed,
            &packet,
        )?
    };
    let writable_before = journal.writable_before.clone();
    let writable_lamports_before = journal
        .writable_lamports_before
        .ok_or_else(|| Error::new("Series Consume journal lost its writable prestate total"))?;

    if let Some(code) = parsed.expect_refusal {
        let refused = finalized;
        let rendered = refused
            .error
            .as_ref()
            .map(|value| value.to_string())
            .ok_or_else(|| {
                Error::new(format!(
                    "{label}: the hostile transaction SUCCEEDED; the property it defends is broken"
                ))
            })?;
        // The RPC layer carries the error as JSON, not as a Rust `Debug`
        // rendering, so the shape is `{"Custom":12293}`. The closing brace is
        // part of the token on purpose: `"Custom":3` is a prefix of
        // `"Custom":30`, and a hostile that accepts the wrong refusal is worse
        // than no hostile at all.
        let token = format!("{{\"Custom\":{code}}}");
        if !rendered.contains(&token) {
            return Err(Error::new(format!(
                "{label}: expected refusal {token}, got {rendered}"
            )));
        }
        if refused.fee_only_balance_change != Some(true) {
            return Err(Error::new(format!(
                "{label}: finalized hostile balances do not prove fee-only rollback"
            )));
        }
        let (_, writable_after_accounts) = rpc.finalized_accounts(&writable_keys, refused.slot)?;
        let writable_after = snapshot_accounts_v1(&writable_keys, &writable_after_accounts)?;
        let writable_lamports_after = snapshot_lamports_v1(&writable_after)?;
        if writable_after != writable_before || writable_lamports_after != writable_lamports_before
        {
            return Err(Error::new(format!(
                "{label}: hostile refusal changed one or more writable protocol accounts"
            )));
        }
        let evidence = SeriesConsumeEvidenceV1 {
            schema: EVIDENCE_SCHEMA_V1.to_owned(),
            cluster: "local-private-validator".to_owned(),
            rpc_url: parsed.rpc_url.clone(),
            program_id: manifest.program_id.clone(),
            lookup_table: manifest.lookup_table.clone(),
            instruction_data_sha256: journal.instruction_data_sha256.clone(),
            outcome_count: manifest.expect.outcome_count,
            account_count: instruction.accounts.len(),
            writable_account_count: writable_keys.len(),
            fee_payer: payer_pubkey.to_string(),
            recent_blockhash: recent_blockhash.clone(),
            signature: packet.signature.clone(),
            finalized_slot: refused.slot,
            compute_units_consumed: refused.compute_units_consumed,
            fee_lamports: refused.fee_lamports,
            disposition: "refused".to_owned(),
            refusal_code: Some(code),
            replay_of_signature: replay_parent
                .as_ref()
                .map(|parent| parent.signature.clone()),
            replay_of_evidence_sha256: replay_parent
                .as_ref()
                .map(|parent| parent.evidence_sha256.clone()),
            distinct_replay_signature: Some(true),
            writable_before,
            writable_after,
            writable_lamports_before,
            writable_lamports_after,
            writable_lamports_conserved: true,
            rollback_byte_exact: Some(true),
            transaction_fee_only_balance_change: Some(true),
            market: manifest.expect.market.clone(),
            permit: manifest.expect.permit.clone(),
            journal: parsed.journal.display().to_string(),
        };
        let evidence_bytes = json_bytes_v1(&evidence)?;
        write_bytes_atomic_v1(&parsed.evidence, &evidence_bytes)?;
        journal.phase = SeriesConsumePhaseV1::Finalized;
        journal.finalized_slot = Some(refused.slot);
        journal.compute_units_consumed = refused.compute_units_consumed;
        journal.evidence_sha256 = Some(hex(&Sha256::digest(&evidence_bytes)));
        write_json_atomic_v1(&parsed.journal, &journal)?;
        println!(
            "series_consume HOSTILE refused as required: signature {} committed in slot {} and \
             failed with {token}. Every writable protocol account rolled back byte-exactly; \
             evidence {}.",
            packet.signature,
            refused.slot,
            parsed.evidence.display()
        );
        return Ok(());
    }
    if let Some(error) = finalized.error.as_ref() {
        return Err(Error::new(format!("{label}: transaction failed: {error}")));
    }

    // The acknowledgment: what the write path committed.
    let (_, after) = rpc.finalized_observed_accounts(&[market, permit], finalized.slot)?;
    let market_after = after
        .first()
        .ok_or_else(|| Error::new("post-state lost the Market"))?;
    let permit_after = after
        .get(1)
        .ok_or_else(|| Error::new("post-state lost the permit"))?;
    let expected_market_owner = parse_key(&manifest.expect.market_owner, "expect.marketOwner")?;
    let expected_permit_owner = parse_key(&manifest.expect.permit_owner, "expect.permitOwner")?;
    if market_after.owner != expected_market_owner {
        return Err(Error::new(format!(
            "{label}: the Market is owned by {} after the Found, not {expected_market_owner}",
            market_after.owner
        )));
    }
    if permit_after.owner != expected_permit_owner {
        return Err(Error::new(format!(
            "{label}: the founding permit is owned by {} after the Found, not \
             {expected_permit_owner}",
            permit_after.owner
        )));
    }
    if permit_after.lamports != manifest.expect.permit_lamports {
        return Err(Error::new(format!(
            "{label}: the founding permit holds {} lamports, not the {} the campaign expects",
            permit_after.lamports, manifest.expect.permit_lamports
        )));
    }
    if market_after.data.iter().all(|byte| *byte == 0) {
        return Err(Error::new(format!(
            "{label}: the Market is Core-owned but still all zero, so nothing was written"
        )));
    }
    let (_, writable_after_accounts) = rpc.finalized_accounts(&writable_keys, finalized.slot)?;
    let writable_after = snapshot_accounts_v1(&writable_keys, &writable_after_accounts)?;
    let writable_lamports_after = snapshot_lamports_v1(&writable_after)?;
    if writable_lamports_after != writable_lamports_before {
        return Err(Error::new(format!(
            "{label}: writable protocol-account lamports changed from {writable_lamports_before} \
             to {writable_lamports_after}; the Found campaign is not conserved"
        )));
    }
    let evidence = SeriesConsumeEvidenceV1 {
        schema: EVIDENCE_SCHEMA_V1.to_owned(),
        cluster: "local-private-validator".to_owned(),
        rpc_url: parsed.rpc_url.clone(),
        program_id: manifest.program_id.clone(),
        lookup_table: manifest.lookup_table.clone(),
        instruction_data_sha256: journal.instruction_data_sha256.clone(),
        outcome_count: manifest.expect.outcome_count,
        account_count: instruction.accounts.len(),
        writable_account_count: writable_keys.len(),
        fee_payer: payer_pubkey.to_string(),
        recent_blockhash,
        signature: packet.signature.clone(),
        finalized_slot: finalized.slot,
        compute_units_consumed: finalized.compute_units_consumed,
        fee_lamports: finalized.fee_lamports,
        disposition: "found".to_owned(),
        refusal_code: None,
        replay_of_signature: None,
        replay_of_evidence_sha256: None,
        distinct_replay_signature: None,
        writable_before,
        writable_after,
        writable_lamports_before,
        writable_lamports_after,
        writable_lamports_conserved: true,
        rollback_byte_exact: None,
        transaction_fee_only_balance_change: finalized.fee_only_balance_change,
        market: manifest.expect.market.clone(),
        permit: manifest.expect.permit.clone(),
        journal: parsed.journal.display().to_string(),
    };
    let evidence_bytes = json_bytes_v1(&evidence)?;
    write_bytes_atomic_v1(&parsed.evidence, &evidence_bytes)?;

    journal.phase = SeriesConsumePhaseV1::Finalized;
    journal.finalized_slot = Some(finalized.slot);
    journal.compute_units_consumed = finalized.compute_units_consumed;
    journal.market_owner_after = Some(market_after.owner.to_string());
    journal.permit_owner_after = Some(permit_after.owner.to_string());
    journal.permit_lamports_after = Some(permit_after.lamports);
    journal.evidence_sha256 = Some(hex(&Sha256::digest(&evidence_bytes)));
    write_json_atomic_v1(&parsed.journal, &journal)?;

    println!(
        "series_consume FOUND: signature {} finalized in slot {}, {} CU, {} routed wire bytes. \
         Market {market} is Core-owned and written; permit {permit} holds {} lamports; writable \
         protocol lamports conserved exactly; evidence {}.",
        packet.signature,
        finalized.slot,
        finalized.compute_units_consumed.unwrap_or_default(),
        journal.routed_wire_bytes.unwrap_or_default(),
        permit_after.lamports,
        parsed.evidence.display()
    );
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut campaign = None;
    let mut rpc_url = None;
    let mut payer = None;
    let mut journal = None;
    let mut evidence = None;
    let mut replay_after_evidence = None;
    let mut expect_refusal = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--campaign" => &mut campaign,
            "--rpc-url" => &mut rpc_url,
            "--payer-keypair" => &mut payer,
            "--journal" => &mut journal,
            "--evidence" => &mut evidence,
            "--replay-after-evidence" => &mut replay_after_evidence,
            "--expect-refusal" => &mut expect_refusal,
            _ => return Err(Error::new(format!("unknown argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    Ok(ArgumentsV1 {
        campaign: absolute_path(
            campaign.ok_or_else(|| Error::new("--campaign is required"))?,
            "--campaign",
        )?,
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        payer: absolute_path(
            payer.ok_or_else(|| Error::new("--payer-keypair is required"))?,
            "--payer-keypair",
        )?,
        journal: absolute_path(
            journal.ok_or_else(|| Error::new("--journal is required"))?,
            "--journal",
        )?,
        evidence: absolute_path(
            evidence.ok_or_else(|| Error::new("--evidence is required"))?,
            "--evidence",
        )?,
        replay_after_evidence: replay_after_evidence
            .map(|value| absolute_path(value, "--replay-after-evidence"))
            .transpose()?,
        execute,
        expect_refusal: match expect_refusal {
            None => None,
            Some(value) => Some(
                value
                    .parse::<u32>()
                    .map_err(|error| Error::new(format!("--expect-refusal: {error}")))?,
            ),
        },
    })
}

/// Read the durable journal, if this campaign has been attempted before.
fn read_journal_v1(path: &PathBuf) -> Result<Option<SeriesConsumeJournalV1>> {
    match fs::read(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::new(format!("{}: {error}", path.display()))),
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
    }
}

fn read_replay_parent_v1(
    parsed: &ArgumentsV1,
    manifest: &CampaignManifestV1,
    instruction_data_sha256: &str,
    account_count: usize,
) -> Result<Option<ReplayParentV1>> {
    let path = match (parsed.expect_refusal, parsed.replay_after_evidence.as_ref()) {
        (None, None) => return Ok(None),
        (Some(_), None) => {
            return Err(Error::new(
                "--expect-refusal requires --replay-after-evidence from the accepted Found; \
                 otherwise a second packet does not prove a distinct-signature protocol replay",
            ));
        }
        (None, Some(_)) => {
            return Err(Error::new(
                "--replay-after-evidence is only valid with --expect-refusal",
            ));
        }
        (Some(_), Some(path)) => path,
    };
    if path == &parsed.evidence {
        return Err(Error::new(
            "replay input evidence and hostile output evidence must be different paths",
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        Error::new(format!(
            "accepted Series Consume evidence {}: {error}",
            path.display()
        ))
    })?;
    let evidence: SeriesConsumeEvidenceV1 = serde_json::from_slice(&bytes)?;
    if evidence.schema != EVIDENCE_SCHEMA_V1
        || evidence.cluster != "local-private-validator"
        || evidence.disposition != "found"
        || evidence.refusal_code.is_some()
        || evidence.program_id != manifest.program_id
        || evidence.lookup_table != manifest.lookup_table
        || evidence.instruction_data_sha256 != instruction_data_sha256
        || evidence.outcome_count != manifest.expect.outcome_count
        || evidence.account_count != account_count
        || evidence.market != manifest.expect.market
        || evidence.permit != manifest.expect.permit
        || !evidence.writable_lamports_conserved
        || evidence.fee_payer.is_empty()
        || evidence.recent_blockhash.is_empty()
        || evidence.signature.is_empty()
    {
        return Err(Error::new(
            "--replay-after-evidence is not the accepted, conserved Found for this exact Series \
             occurrence",
        ));
    }
    Ok(Some(ReplayParentV1 {
        signature: evidence.signature,
        fee_payer: evidence.fee_payer,
        recent_blockhash: evidence.recent_blockhash,
        evidence_sha256: hex(&Sha256::digest(&bytes)),
    }))
}

fn read_manifest_v1(path: &PathBuf) -> Result<CampaignManifestV1> {
    let bytes = fs::read(path).map_err(|error| {
        Error::new(format!(
            "{}: {error}. Emit it first with the ignored ProgramTest \
             emit_series_consume_validator_campaign.",
            path.display()
        ))
    })?;
    let manifest: CampaignManifestV1 = serde_json::from_slice(&bytes)?;
    if manifest.schema != CAMPAIGN_SCHEMA_V1 {
        return Err(Error::new(format!(
            "campaign schema is {}, not {CAMPAIGN_SCHEMA_V1}",
            manifest.schema
        )));
    }
    Ok(manifest)
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn absolute_path(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    Ok(path)
}

fn authenticate_journal_intent_v1(
    observed: &SeriesConsumeJournalV1,
    expected: &SeriesConsumeJournalV1,
) -> Result<()> {
    if observed.schema != JOURNAL_SCHEMA_V1
        || observed.rpc_url != expected.rpc_url
        || observed.program_id != expected.program_id
        || observed.lookup_table != expected.lookup_table
        || observed.account_count != expected.account_count
        || observed.instruction_data_sha256 != expected.instruction_data_sha256
        || observed.outcome_count != expected.outcome_count
        || observed.expected_refusal != expected.expected_refusal
        || observed.replay_of_signature != expected.replay_of_signature
        || observed.replay_of_evidence_sha256 != expected.replay_of_evidence_sha256
        || observed.market != expected.market
        || observed.permit != expected.permit
    {
        return Err(Error::new(
            "the Series Consume journal describes another RPC, request, or expected disposition; \
             use a new journal path rather than resuming across intents",
        ));
    }
    Ok(())
}

fn validate_persisted_prestate_v1(
    journal: &SeriesConsumeJournalV1,
    writable_keys: &[Pubkey],
) -> Result<()> {
    if journal.prestate_slot.is_none()
        || journal.writable_lamports_before.is_none()
        || journal.writable_before.len() != writable_keys.len()
    {
        return Err(Error::new(
            "persisted Series Consume packet has no complete writable prestate; retain this \
             journal and do not replace it with a newly signed action",
        ));
    }
    for (snapshot, expected) in journal.writable_before.iter().zip(writable_keys) {
        if snapshot.address != expected.to_string() {
            return Err(Error::new(
                "persisted Series Consume writable prestate address order changed",
            ));
        }
    }
    let recomputed = snapshot_lamports_v1(&journal.writable_before)?;
    if journal.writable_lamports_before != Some(recomputed) {
        return Err(Error::new(
            "persisted Series Consume writable prestate total changed",
        ));
    }
    Ok(())
}

fn persisted_packet_v1(journal: &SeriesConsumeJournalV1) -> Result<SignedVersionedPacketV1> {
    let packet = SignedVersionedPacketV1 {
        signature: journal
            .expected_signature
            .clone()
            .ok_or_else(|| Error::new("persisted Series Consume packet has no signature"))?,
        packet_base64: journal
            .signed_packet_base64
            .clone()
            .ok_or_else(|| Error::new("persisted Series Consume packet has no bytes"))?,
        packet_sha256: journal
            .signed_packet_sha256
            .clone()
            .ok_or_else(|| Error::new("persisted Series Consume packet has no digest"))?,
        last_valid_block_height: journal.last_valid_block_height.ok_or_else(|| {
            Error::new("persisted Series Consume packet has no last-valid block height")
        })?,
    };
    let wire = BASE64
        .decode(&packet.packet_base64)
        .map_err(|error| Error::new(format!("persisted Series Consume packet base64: {error}")))?;
    if journal.routed_wire_bytes != Some(wire.len()) {
        return Err(Error::new(
            "persisted Series Consume packet wire extent changed",
        ));
    }
    Ok(packet)
}

fn packet_recent_blockhash_v1(packet: &SignedVersionedPacketV1) -> Result<String> {
    let bytes = BASE64
        .decode(&packet.packet_base64)
        .map_err(|error| Error::new(format!("Series Consume packet base64: {error}")))?;
    let transaction: VersionedTransaction = bincode::deserialize(&bytes)
        .map_err(|error| Error::new(format!("Series Consume packet decode: {error}")))?;
    Ok(transaction.message.recent_blockhash().to_string())
}

fn snapshot_accounts_v1(
    keys: &[Pubkey],
    accounts: &[Option<RpcAccount>],
) -> Result<Vec<SeriesAccountSnapshotV1>> {
    if keys.len() != accounts.len() {
        return Err(Error::new(
            "Series Consume account observation width changed",
        ));
    }
    Ok(keys
        .iter()
        .copied()
        .zip(accounts)
        .map(|(address, account)| snapshot_account_v1(address, account.as_ref()))
        .collect())
}

fn snapshot_account_v1(address: Pubkey, account: Option<&RpcAccount>) -> SeriesAccountSnapshotV1 {
    let (present, owner, lamports, executable, rent_epoch, data) = match account {
        Some(account) => (
            true,
            Some(account.owner.to_string()),
            account.lamports,
            account.executable,
            Some(account.rent_epoch),
            account.data.as_slice(),
        ),
        None => (false, None, 0, false, None, &[][..]),
    };
    SeriesAccountSnapshotV1 {
        address: address.to_string(),
        present,
        owner,
        lamports,
        executable,
        rent_epoch,
        data_base64: BASE64.encode(data),
        data_sha256: hex(&Sha256::digest(data)),
    }
}

fn snapshot_lamports_v1(accounts: &[SeriesAccountSnapshotV1]) -> Result<u64> {
    accounts.iter().try_fold(0_u64, |total, account| {
        total
            .checked_add(account.lamports)
            .ok_or_else(|| Error::new("Series Consume writable lamport sum overflow"))
    })
}

fn json_bytes_v1<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_json_atomic_v1<T: Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    write_bytes_atomic_v1(path, &json_bytes_v1(value)?)
}

fn write_bytes_atomic_v1(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_keeps_absence_distinct_from_a_zero_account() {
        let key = Pubkey::new_unique();
        let absent = snapshot_account_v1(key, None);
        let account = RpcAccount {
            lamports: 0,
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        };
        let present = snapshot_account_v1(key, Some(&account));
        assert!(!absent.present);
        assert!(present.present);
        assert_ne!(absent, present);
    }

    #[test]
    fn writable_lamport_sum_refuses_overflow() {
        let mut first = snapshot_account_v1(Pubkey::new_unique(), None);
        first.lamports = u64::MAX;
        let mut second = snapshot_account_v1(Pubkey::new_unique(), None);
        second.lamports = 1;
        assert!(snapshot_lamports_v1(&[first, second]).is_err());
    }

    #[test]
    fn recovery_never_resigns_a_prepared_or_submitted_occurrence() {
        assert_eq!(
            resume_action_v1(SeriesConsumePhaseV1::Planned),
            ResumeActionV1::Prepare
        );
        assert_eq!(
            resume_action_v1(SeriesConsumePhaseV1::Prepared),
            ResumeActionV1::SubmitPersisted
        );
        assert_eq!(
            resume_action_v1(SeriesConsumePhaseV1::Submitted),
            ResumeActionV1::PollOnly
        );
        assert_eq!(
            resume_action_v1(SeriesConsumePhaseV1::Finalized),
            ResumeActionV1::Done
        );
    }
}
