//! Durable live-validator evidence for the compiled Series permit-expiry route.
//!
//! The Series fixture remains the semantic owner of the 25-account frame. Its
//! ignored emitter exports three exact genesis worlds: the authenticated V2
//! successor profile, byte-identical V2 bytes at the wrong PDA, and the sealed
//! V1 predecessor. This consumer does not reconstruct any protocol fact. It
//! authenticates that bundle, submits its exact instruction through a v0
//! lookup-table packet, and records either:
//!
//! - every unallocated permit lamport moving once into the creation-fixed
//!   `LifecycleRentCreditV2`; or
//! - the exact expected refusal plus byte-exact rollback of both writable
//!   protocol accounts and a transaction metadata proof that only the fee
//!   payer moved.
//!
//! A local validator is real-ELF evidence, not mainnet evidence. The emitted
//! `minimumExecutionSlot` must be supplied to `solana-test-validator
//! --warp-slot`; shortening the retry window in a special fixture would prove
//! a different request.

use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    plan::hex,
    rpc::{Rpc, RpcAccount},
};

/// Submit one fixture-emitted Series permit-expiry campaign.
pub(crate) const COMMAND_V1: &str = "local-private-validator-series-permit-expiry-v1";

const CAMPAIGN_SCHEMA_V1: &str = "dclutch-series-permit-expiry-validator-campaign-v1";
const JOURNAL_SCHEMA_V1: &str = "dclutch-series-permit-expiry-journal-v1";
const EVIDENCE_SCHEMA_V1: &str = "dclutch-series-permit-expiry-evidence-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignManifestV1 {
    schema: String,
    case: String,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignMetaV1 {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CampaignExpectationV1 {
    expiry_slot: u64,
    minimum_execution_slot: u64,
    permit: String,
    permit_owner_before: String,
    permit_lamports_before: u64,
    permit_data_base64_before: String,
    rent_credit: String,
    rent_credit_owner: String,
    rent_credit_lamports_before: u64,
    rent_credit_data_base64_before: String,
    refund_lamports: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum PhaseV1 {
    Planned,
    Prepared,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct JournalV1 {
    schema: String,
    phase: PhaseV1,
    case: String,
    rpc_url: String,
    program_id: String,
    lookup_table: String,
    instruction_data_sha256: String,
    account_count: usize,
    expected_refusal: Option<u32>,
    signed_packet_base64: Option<String>,
    signed_packet_sha256: Option<String>,
    expected_signature: Option<String>,
    last_valid_block_height: Option<u64>,
    routed_wire_bytes: Option<usize>,
    finalized_slot: Option<u64>,
    compute_units_consumed: Option<u64>,
    fee_lamports: Option<u64>,
    evidence_sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceV1 {
    schema: String,
    cluster: String,
    case: String,
    rpc_url: String,
    program_id: String,
    instruction_data_sha256: String,
    account_count: usize,
    genesis_account_count: usize,
    genesis_only: Vec<String>,
    absent_by_design: Vec<String>,
    signature: String,
    finalized_slot: u64,
    compute_units_consumed: Option<u64>,
    fee_lamports: Option<u64>,
    disposition: String,
    refusal_code: Option<u32>,
    expiry_slot: u64,
    minimum_execution_slot: u64,
    permit_before: AccountSnapshotV1,
    permit_after: AccountSnapshotV1,
    rent_credit_before: AccountSnapshotV1,
    rent_credit_after: AccountSnapshotV1,
    refund_lamports: u64,
    two_account_lamports_before: u64,
    two_account_lamports_after: u64,
    conservation_exact: bool,
    rollback_byte_exact: Option<bool>,
    transaction_fee_only_balance_change: Option<bool>,
    journal: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AccountSnapshotV1 {
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
    execute: bool,
    expect_refusal: Option<u32>,
}

/// Run one emitted permit-expiry world against a local validator.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let parsed = parse_arguments(arguments)?;
    let manifest = read_manifest_v1(&parsed.campaign)?;
    validate_manifest_case_v1(&manifest, parsed.expect_refusal)?;
    let program_id = parse_key(&manifest.program_id, "programId")?;
    let lookup_table = parse_key(&manifest.lookup_table, "lookupTable")?;
    let permit = parse_key(&manifest.expect.permit, "expect.permit")?;
    let rent_credit = parse_key(&manifest.expect.rent_credit, "expect.rentCredit")?;
    if manifest.genesis_only.len() != 1 {
        return Err(Error::new(
            "Series permit-expiry campaign must name exactly the invoked Core ProgramData as genesis-only",
        ));
    }
    let core_programdata = parse_key(&manifest.genesis_only[0], "genesisOnly Core ProgramData")?;
    let data = decode_canonical_base64_v1(&manifest.data_base64, "campaign data")?;
    let mut accounts = Vec::with_capacity(manifest.accounts.len());
    for meta in &manifest.accounts {
        accounts.push(AccountMeta {
            pubkey: parse_key(&meta.pubkey, "account meta")?,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }
    if accounts.len() != 25 {
        return Err(Error::new(format!(
            "Series permit-expiry campaign carries {} metas, not the shipped 25-account frame",
            accounts.len()
        )));
    }
    let writable = accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    if writable != [permit, rent_credit] {
        return Err(Error::new(
            "Series permit-expiry writable set is not exactly [permit, RentCredit]",
        ));
    }
    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };
    let instruction_data_sha256 = hex(&Sha256::digest(&instruction.data));
    let mut journal = JournalV1 {
        schema: JOURNAL_SCHEMA_V1.to_owned(),
        phase: PhaseV1::Planned,
        case: manifest.case.clone(),
        rpc_url: parsed.rpc_url.clone(),
        program_id: manifest.program_id.clone(),
        lookup_table: manifest.lookup_table.clone(),
        instruction_data_sha256: instruction_data_sha256.clone(),
        account_count: instruction.accounts.len(),
        expected_refusal: parsed.expect_refusal,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        last_valid_block_height: None,
        routed_wire_bytes: None,
        finalized_slot: None,
        compute_units_consumed: None,
        fee_lamports: None,
        evidence_sha256: None,
    };

    if let Some(previous) = read_journal_v1(&parsed.journal)?
        && previous.phase == PhaseV1::Finalized
    {
        if previous.case != journal.case
            || previous.instruction_data_sha256 != journal.instruction_data_sha256
            || previous.expected_refusal != journal.expected_refusal
        {
            return Err(Error::new(
                "the finalized journal describes another expiry world or expected disposition",
            ));
        }
        let evidence = fs::read(&parsed.evidence).map_err(|error| {
            Error::new(format!(
                "finalized journal exists but evidence {} is unavailable: {error}",
                parsed.evidence.display()
            ))
        })?;
        let observed = hex(&Sha256::digest(&evidence));
        if previous.evidence_sha256.as_deref() != Some(observed.as_str()) {
            return Err(Error::new(
                "finalized Series expiry evidence no longer matches its journal digest",
            ));
        }
        println!(
            "Series permit-expiry {} already finalized at slot {} as signature {}; evidence {} is unchanged",
            manifest.case,
            previous.finalized_slot.unwrap_or_default(),
            previous.expected_signature.as_deref().unwrap_or("missing"),
            parsed.evidence.display()
        );
        return Ok(());
    }

    let expected_permit_owner = parse_key(
        &manifest.expect.permit_owner_before,
        "expect.permitOwnerBefore",
    )?;
    let expected_credit_owner =
        parse_key(&manifest.expect.rent_credit_owner, "expect.rentCreditOwner")?;
    let expected_permit_data = decode_canonical_base64_v1(
        &manifest.expect.permit_data_base64_before,
        "expect.permitDataBase64Before",
    )?;
    let expected_credit_data = decode_canonical_base64_v1(
        &manifest.expect.rent_credit_data_base64_before,
        "expect.rentCreditDataBase64Before",
    )?;
    if manifest.expect.minimum_execution_slot
        != manifest
            .expect
            .expiry_slot
            .checked_add(1)
            .ok_or_else(|| Error::new("expiry slot overflow"))?
        || manifest.expect.refund_lamports != manifest.expect.permit_lamports_before
        || manifest.expect.refund_lamports == 0
    {
        return Err(Error::new(
            "campaign expiry boundary or exact refund amount is inconsistent",
        ));
    }

    let mut rpc = Rpc::connect(&parsed.rpc_url)?;
    let current_slot = rpc.finalized_slot()?;
    if current_slot < manifest.expect.minimum_execution_slot {
        return Err(Error::new(format!(
            "validator is at finalized slot {current_slot}, before Series permit expiry; restart it with --warp-slot {}",
            manifest.expect.minimum_execution_slot
        )));
    }
    let (_, route_accounts) = rpc.finalized_observed_accounts(&[lookup_table, program_id], 0)?;
    let table_observed = route_accounts
        .first()
        .ok_or_else(|| Error::new("preflight lost the lookup table"))?
        .clone();
    if table_observed.data.is_empty() {
        return Err(Error::new(
            "Series expiry lookup table is empty; load the emitted accounts directory",
        ));
    }
    let invoked = route_accounts
        .get(1)
        .ok_or_else(|| Error::new("preflight lost the Core program"))?;
    if !invoked.executable {
        return Err(Error::new(
            "Series expiry Core program is not executable; load the emitted genesis bundle",
        ));
    }
    let core_programdata_account = rpc.required_account(core_programdata, "Core ProgramData")?;
    if core_programdata_account.owner != solana_sdk_ids::bpf_loader_upgradeable::ID
        || core_programdata_account.executable
        || core_programdata_account.data.len() <= 45
    {
        return Err(Error::new(
            "Series expiry genesis-only Core ProgramData is not a complete Loader V3 body",
        ));
    }
    let (_, before_accounts) = rpc.finalized_accounts(&[permit, rent_credit], current_slot)?;
    let permit_before = snapshot_v1(permit, before_accounts.first().and_then(Option::as_ref));
    let credit_before = snapshot_v1(rent_credit, before_accounts.get(1).and_then(Option::as_ref));
    require_prestate_v1(
        &permit_before,
        expected_permit_owner,
        manifest.expect.permit_lamports_before,
        &expected_permit_data,
        "permit",
    )?;
    require_prestate_v1(
        &credit_before,
        expected_credit_owner,
        manifest.expect.rent_credit_lamports_before,
        &expected_credit_data,
        "RentCredit",
    )?;

    write_json_atomic_v1(&parsed.journal, &journal)?;
    if !parsed.execute {
        println!(
            "planned Series permit-expiry {}: 25 metas, {} emitted accounts, {} absent by design, minimum slot {}. Rerun with --execute.",
            manifest.case,
            manifest.genesis_account_count,
            manifest.absent_by_design.len(),
            manifest.expect.minimum_execution_slot
        );
        return Ok(());
    }

    let payer = Keypair::new_from_array(read_keypair_file(&parsed.payer, "payer")?);
    let label = format!("Series permit-expiry {}", manifest.case);
    let _ = manifest.compute_unit_limit;
    let packet = rpc.prepare_signed_v0_packet(
        &label,
        std::slice::from_ref(&instruction),
        &payer,
        &table_observed,
    )?;
    journal.phase = PhaseV1::Prepared;
    journal.routed_wire_bytes =
        Some(decode_canonical_base64_v1(&packet.packet_base64, "signed packet")?.len());
    journal.signed_packet_base64 = Some(packet.packet_base64.clone());
    journal.signed_packet_sha256 = Some(packet.packet_sha256.clone());
    journal.expected_signature = Some(packet.signature.clone());
    journal.last_valid_block_height = Some(packet.last_valid_block_height);
    write_json_atomic_v1(&parsed.journal, &journal)?;

    let finalized = if parsed.expect_refusal.is_some() {
        rpc.submit_signed_v0_packet_expecting_failure(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?;
        journal.phase = PhaseV1::Submitted;
        write_json_atomic_v1(&parsed.journal, &journal)?;
        rpc.confirm_signed_v0_packet_expecting_failure(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?
    } else {
        rpc.submit_signed_v0_packet(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?;
        journal.phase = PhaseV1::Submitted;
        write_json_atomic_v1(&parsed.journal, &journal)?;
        rpc.confirm_signed_v0_packet(
            &label,
            std::slice::from_ref(&instruction),
            payer.pubkey(),
            &table_observed,
            &packet,
        )?
    };

    let (_, after_accounts) = rpc.finalized_accounts(&[permit, rent_credit], finalized.slot)?;
    let permit_after = snapshot_v1(permit, after_accounts.first().and_then(Option::as_ref));
    let credit_after = snapshot_v1(rent_credit, after_accounts.get(1).and_then(Option::as_ref));
    let before_total = permit_before
        .lamports
        .checked_add(credit_before.lamports)
        .ok_or_else(|| Error::new("Series expiry prestate lamport sum overflow"))?;
    let after_total = permit_after
        .lamports
        .checked_add(credit_after.lamports)
        .ok_or_else(|| Error::new("Series expiry poststate lamport sum overflow"))?;

    let (disposition, refusal_code, rollback, fee_only) =
        if let Some(expected_refusal) = parsed.expect_refusal {
            if finalized.refusal_code() != Some(expected_refusal) {
                return Err(Error::new(format!(
                    "{label}: expected exact refusal {expected_refusal:#06x}, got {:?}",
                    finalized.refusal_code()
                )));
            }
            let rollback = permit_after == permit_before && credit_after == credit_before;
            if !rollback {
                return Err(Error::new(format!(
                    "{label}: refused transaction changed the permit or RentCredit"
                )));
            }
            if finalized.fee_only_balance_change != Some(true) {
                return Err(Error::new(format!(
                    "{label}: finalized transaction balances do not prove fee-only rollback"
                )));
            }
            ("refused", Some(expected_refusal), Some(true), Some(true))
        } else {
            if finalized.error.is_some() {
                return Err(Error::new(format!(
                    "{label}: accepting campaign finalized with {:?}",
                    finalized.error
                )));
            }
            require_closed_permit_v1(&permit_after, expected_permit_owner)?;
            let expected_credit = credit_before
                .lamports
                .checked_add(manifest.expect.refund_lamports)
                .ok_or_else(|| Error::new("Series expiry refund overflow"))?;
            if credit_after.owner.as_deref() != Some(&expected_credit_owner.to_string())
                || credit_after.data_base64 != credit_before.data_base64
                || credit_after.lamports != expected_credit
                || before_total != after_total
            {
                return Err(Error::new(format!(
                    "{label}: exact permit-to-RentCredit conservation failed"
                )));
            }
            ("refunded", None, None, finalized.fee_only_balance_change)
        };

    let evidence = EvidenceV1 {
        schema: EVIDENCE_SCHEMA_V1.to_owned(),
        cluster: "local-private-validator".to_owned(),
        case: manifest.case.clone(),
        rpc_url: parsed.rpc_url.clone(),
        program_id: manifest.program_id.clone(),
        instruction_data_sha256,
        account_count: instruction.accounts.len(),
        genesis_account_count: manifest.genesis_account_count,
        genesis_only: manifest.genesis_only.clone(),
        absent_by_design: manifest.absent_by_design.clone(),
        signature: packet.signature.clone(),
        finalized_slot: finalized.slot,
        compute_units_consumed: finalized.compute_units_consumed,
        fee_lamports: finalized.fee_lamports,
        disposition: disposition.to_owned(),
        refusal_code,
        expiry_slot: manifest.expect.expiry_slot,
        minimum_execution_slot: manifest.expect.minimum_execution_slot,
        permit_before,
        permit_after,
        rent_credit_before: credit_before,
        rent_credit_after: credit_after,
        refund_lamports: manifest.expect.refund_lamports,
        two_account_lamports_before: before_total,
        two_account_lamports_after: after_total,
        conservation_exact: before_total == after_total,
        rollback_byte_exact: rollback,
        transaction_fee_only_balance_change: fee_only,
        journal: parsed.journal.display().to_string(),
    };
    let evidence_bytes = json_bytes_v1(&evidence)?;
    write_bytes_atomic_v1(&parsed.evidence, &evidence_bytes)?;
    journal.phase = PhaseV1::Finalized;
    journal.finalized_slot = Some(finalized.slot);
    journal.compute_units_consumed = finalized.compute_units_consumed;
    journal.fee_lamports = finalized.fee_lamports;
    journal.evidence_sha256 = Some(hex(&Sha256::digest(&evidence_bytes)));
    write_json_atomic_v1(&parsed.journal, &journal)?;

    println!(
        "Series permit-expiry {} {} in finalized slot {} ({} CU): permit {} -> {} lamports, RentCredit {} -> {}, exact two-account conservation {}; evidence {}",
        manifest.case,
        disposition,
        finalized.slot,
        finalized.compute_units_consumed.unwrap_or_default(),
        evidence.permit_before.lamports,
        evidence.permit_after.lamports,
        evidence.rent_credit_before.lamports,
        evidence.rent_credit_after.lamports,
        evidence.conservation_exact,
        parsed.evidence.display()
    );
    Ok(())
}

fn validate_manifest_case_v1(manifest: &CampaignManifestV1, refusal: Option<u32>) -> Result<()> {
    match (manifest.case.as_str(), refusal) {
        ("successor", None) | ("wrong-address", Some(_)) | ("sealed-predecessor", Some(_)) => {
            Ok(())
        }
        ("successor", Some(_)) => Err(Error::new(
            "the authenticated successor world is the accepting control; do not relabel it a hostile",
        )),
        ("wrong-address" | "sealed-predecessor", None) => Err(Error::new(
            "a hostile profile world requires --expect-refusal with the exact Core code",
        )),
        _ => Err(Error::new(format!(
            "unknown Series expiry campaign case {:?}",
            manifest.case
        ))),
    }
}

fn snapshot_v1(address: Pubkey, account: Option<&RpcAccount>) -> AccountSnapshotV1 {
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
    AccountSnapshotV1 {
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

fn require_prestate_v1(
    observed: &AccountSnapshotV1,
    owner: Pubkey,
    lamports: u64,
    data: &[u8],
    label: &str,
) -> Result<()> {
    if !observed.present
        || observed.owner.as_deref() != Some(&owner.to_string())
        || observed.lamports != lamports
        || observed.executable
        || observed.data_base64 != BASE64.encode(data)
    {
        return Err(Error::new(format!(
            "Series expiry {label} prestate differs from the emitted fixture"
        )));
    }
    Ok(())
}

fn require_closed_permit_v1(observed: &AccountSnapshotV1, system_owner: Pubkey) -> Result<()> {
    if observed.present
        && (observed.owner.as_deref() != Some(&system_owner.to_string())
            || observed.lamports != 0
            || !observed.data_base64.is_empty())
    {
        return Err(Error::new(
            "accepted Series expiry left a nonempty or funded permit account",
        ));
    }
    if !observed.present && observed.lamports != 0 {
        return Err(Error::new("absent permit reported a nonzero balance"));
    }
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut campaign = None;
    let mut rpc_url = None;
    let mut payer = None;
    let mut journal = None;
    let mut evidence = None;
    let mut expect_refusal = None;
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
        let slot = match argument.as_str() {
            "--campaign" => &mut campaign,
            "--rpc-url" => &mut rpc_url,
            "--payer-keypair" => &mut payer,
            "--journal" => &mut journal,
            "--evidence" => &mut evidence,
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
        execute,
        expect_refusal: expect_refusal
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|error| Error::new(format!("--expect-refusal: {error}")))
            })
            .transpose()?,
    })
}

fn read_manifest_v1(path: &PathBuf) -> Result<CampaignManifestV1> {
    let bytes = fs::read(path).map_err(|error| {
        Error::new(format!(
            "{}: {error}. Emit it with emit_series_permit_expiry_validator_campaign",
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

fn read_journal_v1(path: &PathBuf) -> Result<Option<JournalV1>> {
    match fs::read(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::new(format!("{}: {error}", path.display()))),
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
    }
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn decode_canonical_base64_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    let decoded = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&decoded) != value {
        return Err(Error::new(format!("{label} was not canonical base64")));
    }
    Ok(decoded)
}

fn absolute_path(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    Ok(path)
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
    fn campaign_cases_cannot_swap_acceptance_and_hostile_meanings() {
        let mut manifest = CampaignManifestV1 {
            schema: CAMPAIGN_SCHEMA_V1.to_owned(),
            case: "successor".to_owned(),
            program_id: Pubkey::new_unique().to_string(),
            lookup_table: Pubkey::new_unique().to_string(),
            data_base64: String::new(),
            accounts: Vec::new(),
            compute_unit_limit: 1,
            genesis_account_count: 1,
            genesis_only: vec![Pubkey::new_unique().to_string()],
            absent_by_design: Vec::new(),
            expect: CampaignExpectationV1 {
                expiry_slot: 1,
                minimum_execution_slot: 2,
                permit: Pubkey::new_unique().to_string(),
                permit_owner_before: Pubkey::new_unique().to_string(),
                permit_lamports_before: 1,
                permit_data_base64_before: String::new(),
                rent_credit: Pubkey::new_unique().to_string(),
                rent_credit_owner: Pubkey::new_unique().to_string(),
                rent_credit_lamports_before: 1,
                rent_credit_data_base64_before: String::new(),
                refund_lamports: 1,
            },
        };
        assert!(validate_manifest_case_v1(&manifest, None).is_ok());
        assert!(validate_manifest_case_v1(&manifest, Some(0x3000)).is_err());
        manifest.case = "wrong-address".to_owned();
        assert!(validate_manifest_case_v1(&manifest, None).is_err());
        assert!(validate_manifest_case_v1(&manifest, Some(0x3000)).is_ok());
    }

    #[test]
    fn snapshot_distinguishes_absence_from_a_zero_lamport_account() {
        let key = Pubkey::new_unique();
        let absent = snapshot_v1(key, None);
        let account = RpcAccount {
            lamports: 0,
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        };
        let present = snapshot_v1(key, Some(&account));
        assert!(!absent.present);
        assert!(present.present);
        assert_ne!(absent, present);
    }
}
