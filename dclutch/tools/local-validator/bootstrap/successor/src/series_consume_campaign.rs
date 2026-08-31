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
};

use crate::{Error, Result, campaign::read_keypair_file, plan::hex, rpc::Rpc};

/// Command that submits one emitted `series_consume` campaign.
pub(crate) const SERIES_CONSUME_COMMAND_V1: &str = "local-private-validator-series-consume-v1";

const CAMPAIGN_SCHEMA_V1: &str = "dclutch-series-consume-validator-campaign-v1";
const JOURNAL_SCHEMA_V1: &str = "dclutch-series-consume-journal-v1";

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
}

struct ArgumentsV1 {
    campaign: PathBuf,
    rpc_url: String,
    payer: PathBuf,
    journal: PathBuf,
    execute: bool,
    expect_refusal: Option<u32>,
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

    let mut rpc = Rpc::connect(&parsed.rpc_url)?;
    let market = parse_key(&manifest.expect.market, "expect.market")?;
    let permit = parse_key(&manifest.expect.permit, "expect.permit")?;

    let mut journal = SeriesConsumeJournalV1 {
        schema: JOURNAL_SCHEMA_V1.to_owned(),
        phase: SeriesConsumePhaseV1::Planned,
        rpc_url: parsed.rpc_url.clone(),
        program_id: manifest.program_id.clone(),
        lookup_table: manifest.lookup_table.clone(),
        account_count: instruction.accounts.len(),
        instruction_data_sha256: hex(&Sha256::digest(&instruction.data)),
        outcome_count: manifest.expect.outcome_count,
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
    };

    // A finished campaign rerun is a no-op, not an error. This is read before
    // the preflight because the preflight's vacant-Market check is precisely
    // what a SUCCEEDED Found invalidates: once the Market is written, "the
    // Market is not vacant" stops being a misconfiguration and becomes the
    // proof that this campaign already did its work.
    if let Some(previous) = read_journal_v1(&parsed.journal)?
        && previous.phase == SeriesConsumePhaseV1::Finalized
        && parsed.expect_refusal.is_none()
    {
        if previous.instruction_data_sha256 != journal.instruction_data_sha256 {
            return Err(Error::new(
                "the journal in this path describes a different series_consume request; use a \
                 new journal path rather than resuming across intents",
            ));
        }
        let signature = previous
            .expected_signature
            .clone()
            .ok_or_else(|| Error::new("a finalized journal carries no signature"))?;
        println!(
            "series_consume already FOUND: signature {signature} finalized in slot {}, {} CU. \
             Nothing resubmitted.",
            previous.finalized_slot.unwrap_or_default(),
            previous.compute_units_consumed.unwrap_or_default()
        );
        return Ok(());
    }

    // Before anything is signed: prove the genesis actually carries the frame.
    // A validator started without the emitted --account-dir answers every one
    // of these with a system-owned empty account, and the transaction would
    // refuse deep inside Core with an opaque code. Failing here instead names
    // the cause.
    let probe = [market, permit, lookup_table, program_id];
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
    let market_before = observed
        .first()
        .ok_or_else(|| Error::new("preflight lost the Market"))?;
    // A hostile deliberately runs against a Market that is ALREADY written --
    // that is the whole point of the double-consume -- so the vacant-Market
    // precondition is a happy-path check, not a universal one.
    if parsed.expect_refusal.is_none() && !market_before.data.iter().all(|byte| *byte == 0) {
        return Err(Error::new(format!(
            "the Market {market} is already written on this cluster; series_consume IS the Found \
             and needs a vacant Market, so start a fresh ledger with --reset"
        )));
    }

    write_json_atomic_v1(&parsed.journal, &journal)?;
    if !parsed.execute {
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

    let payer = Keypair::new_from_array(read_keypair_file(&parsed.payer, "payer")?);
    let label = format!(
        "series_consume Found at {} outcomes",
        manifest.expect.outcome_count
    );

    // The compute budget is NOT set here: `bounded_instructions` owns the
    // ComputeBudget declarations and refuses a duplicate. It already asks for
    // 1,400,000 units, comfortably above the 722,142 this route measures, and a
    // real validator's 200,000 default would refuse without it.
    let _ = manifest.compute_unit_limit;

    let packet = rpc.prepare_signed_v0_packet(
        &label,
        std::slice::from_ref(&instruction),
        &payer,
        &table_observed,
    )?;
    journal.phase = SeriesConsumePhaseV1::Prepared;
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

    if let Some(code) = parsed.expect_refusal {
        rpc.submit_signed_v0_packet_expecting_failure(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?;
        journal.phase = SeriesConsumePhaseV1::Submitted;
        write_json_atomic_v1(&parsed.journal, &journal)?;
        let refused = rpc.confirm_signed_v0_packet_expecting_failure(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?;
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
        journal.phase = SeriesConsumePhaseV1::Finalized;
        journal.finalized_slot = Some(refused.slot);
        journal.compute_units_consumed = refused.compute_units_consumed;
        write_json_atomic_v1(&parsed.journal, &journal)?;
        println!(
            "series_consume HOSTILE refused as required: signature {} committed in slot {} and \
             failed with {token}. The refusal is in finalized history, not a simulation.",
            packet.signature, refused.slot
        );
        return Ok(());
    }

    rpc.submit_signed_v0_packet(
        &label,
        std::slice::from_ref(&instruction),
        payer.pubkey(),
        &table_observed,
        &packet,
    )?;
    journal.phase = SeriesConsumePhaseV1::Submitted;
    write_json_atomic_v1(&parsed.journal, &journal)?;

    let finalized = rpc.confirm_signed_v0_packet(
        &label,
        std::slice::from_ref(&instruction),
        payer.pubkey(),
        &table_observed,
        &packet,
    )?;
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

    journal.phase = SeriesConsumePhaseV1::Finalized;
    journal.finalized_slot = Some(finalized.slot);
    journal.compute_units_consumed = finalized.compute_units_consumed;
    journal.market_owner_after = Some(market_after.owner.to_string());
    journal.permit_owner_after = Some(permit_after.owner.to_string());
    journal.permit_lamports_after = Some(permit_after.lamports);
    write_json_atomic_v1(&parsed.journal, &journal)?;

    println!(
        "series_consume FOUND: signature {} finalized in slot {}, {} CU, {} routed wire bytes. \
         Market {market} is Core-owned and written; permit {permit} holds {} lamports.",
        packet.signature,
        finalized.slot,
        finalized.compute_units_consumed.unwrap_or_default(),
        journal.routed_wire_bytes.unwrap_or_default(),
        permit_after.lamports
    );
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut campaign = None;
    let mut rpc_url = None;
    let mut payer = None;
    let mut journal = None;
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

fn write_json_atomic_v1<T: Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.partial");
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(&temporary, &bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
