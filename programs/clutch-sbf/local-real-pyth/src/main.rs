//! NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY campaign.

#![recursion_limit = "256"]

mod plane;
mod provider;
mod rpc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use clutch_sbf::{
    loader_state::UPGRADEABLE_LOADER_ID,
    pyth_receiver::{parse_full_price_update_v2, PriceUpdateAccountViewV1},
    source_archive_v2::{self, AccountViewV2 as ArchiveAccountViewV2},
    source_identity::{real_pyth_lab, CLOCK_SYSVAR_ID},
    source_v2::crossing::ArchiveRecordV2,
    source_v2::fixtures::{programdata_body, receiver_program_body},
};
use clutch_solana_layout::clearing::{EpochWindowAccount, CANDIDATE_WINDOW_SLOTS};
use clutch_solana_layout::{
    account_len, reservation::RESERVATION_ACCOUNT_BYTES, CandidateRecord, EpochAccount, Hash32,
    PriceGridAccount, CANDIDATE_STATUS_SELECTED, CANDIDATE_STATUS_VERIFIED, EPOCH_PHASE_CLEARED,
    EPOCH_PHASE_FROZEN,
};
use clutch_solana_reference::REPLAY_ACCOUNT_LEN;
use clutch_svm_fixture::{
    compute_unit_limit_data, outcome_mint_bytes, token_account_bytes, COMPUTE_BUDGET, PROGRAM_ID,
    RENT_SYSVAR, SYSTEM_PROGRAM,
};
use rpc::{AccountView, Rpc};
use serde_json::{json, Value};
use solana_address::Address;
use solana_clock::Clock;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::{read_keypair_file, write_keypair_file, Keypair};
use solana_rent::Rent;
use solana_signer::Signer;
use solana_system_interface::instruction as system_instruction;
use solana_transaction::Transaction;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process,
    str::FromStr,
    thread,
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CLAIM: &str = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const SOURCE_ONLY_MODE: &str = "source-only-v1";
const JOINED_LIFECYCLE_MODE: &str = "joined-user-lifecycle-v1";
const MULTIBOUNDARY_JOINED_MODE: &str = "joined-multiboundary-v1";
const SOURCE_TRANSCRIPT_SCHEMA: &str = "dragons-clutch/operator/local-real-pyth-transcript/v2";
const JOINED_TRANSCRIPT_SCHEMA: &str =
    "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v4";
const MULTIBOUNDARY_JOINED_TRANSCRIPT_SCHEMA: &str =
    "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1";
const UPSTREAM_COMMIT: &str = "f50a3faf9fc5a223a22889799b2f778900f186b3";
const WARP_SLOT: u64 = real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT + 1;
const CLOCK_SETTLED_SLOT: u64 = WARP_SLOT + 16;
const VAA_START: usize = 46;
const WRITE_SPLIT: usize = 755;
const INIT_ENCODED_VAA: [u8; 8] = [209, 193, 173, 25, 91, 202, 181, 218];
const WRITE_ENCODED_VAA: [u8; 8] = [199, 208, 110, 177, 150, 76, 118, 42];
const VERIFY_ENCODED_VAA_V1: [u8; 8] = [103, 56, 177, 229, 240, 103, 68, 73];
const WRONG_CONFIG: Address = Address::new_from_array([0xcf; 32]);
const WRONG_FEED_ID: [u8; 32] = [0x2b; 32];
const SOURCE_PROFILE_SNAPSHOT: &str =
    "../source-profiles/devnet-real-source-snapshot-2026-08-22.json";
const VALIDATOR_PROVENANCE: &str = "../../../tools/agave-loopback-validator/PROVENANCE.md";
const VALIDATOR_PINS: &str = "../../../tools/agave-loopback-validator/pins.env";
const VALIDATOR_PATCH: &str = "../../../tools/agave-loopback-validator/agave-4.0.2-loopback.patch";
const ROLLBACK_SNAPSHOT_DOMAIN: &[u8] = b"dragons-clutch/local-real-pyth/rollback-snapshot/v1";

struct Args {
    command: String,
    work: PathBuf,
    clutch_elf: Option<PathBuf>,
    validator: Option<PathBuf>,
    url: Option<String>,
    publish_time: Option<i64>,
    clock_probe_time: Option<i64>,
    repository_head: Option<String>,
    campaign_mode: String,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> std::result::Result<Args, String> {
    let command = args.next().ok_or("prepare or run is required")?;
    let mut work = None;
    let mut clutch_elf = None;
    let mut validator = None;
    let mut url = None;
    let mut publish_time = None;
    let mut clock_probe_time = None;
    let mut repository_head = None;
    let mut campaign_mode = SOURCE_ONLY_MODE.to_string();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--work" => work = Some(PathBuf::from(value)),
            "--clutch-elf" => clutch_elf = Some(PathBuf::from(value)),
            "--validator" => validator = Some(PathBuf::from(value)),
            "--url" => url = Some(value),
            "--publish-time" => {
                publish_time = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --publish-time {value}"))?,
                )
            }
            "--clock-probe-time" => {
                clock_probe_time = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --clock-probe-time {value}"))?,
                )
            }
            "--repository-head" => repository_head = Some(value),
            "--campaign-mode" => campaign_mode = value,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if command != "prepare" && command != "run" && command != "clock" {
        return Err(format!("unknown command {command}"));
    }
    if campaign_mode != SOURCE_ONLY_MODE
        && campaign_mode != JOINED_LIFECYCLE_MODE
        && campaign_mode != MULTIBOUNDARY_JOINED_MODE
    {
        return Err(format!("unknown campaign mode {campaign_mode}"));
    }
    Ok(Args {
        command,
        work: work.ok_or("--work is required")?,
        clutch_elf,
        validator,
        url,
        publish_time,
        clock_probe_time,
        repository_head,
        campaign_mode,
    })
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  clutch-local-real-pyth clock --work DIR --url http://127.0.0.1:PORT\n  \
         clutch-local-real-pyth prepare --work DIR --repository-head COMMIT --clock-probe-time UNIX --publish-time UNIX --clutch-elf FILE --validator FILE [--campaign-mode source-only-v1|joined-user-lifecycle-v1|joined-multiboundary-v1]\n  \
         clutch-local-real-pyth run --work DIR --repository-head COMMIT --url http://127.0.0.1:PORT --validator FILE [--campaign-mode source-only-v1|joined-user-lifecycle-v1|joined-multiboundary-v1]"
    );
    process::exit(2)
}

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn transcript_schema(campaign_mode: &str) -> &'static str {
    match campaign_mode {
        JOINED_LIFECYCLE_MODE => JOINED_TRANSCRIPT_SCHEMA,
        MULTIBOUNDARY_JOINED_MODE => MULTIBOUNDARY_JOINED_TRANSCRIPT_SCHEMA,
        _ => SOURCE_TRANSCRIPT_SCHEMA,
    }
}

fn is_joined_mode(campaign_mode: &str) -> bool {
    campaign_mode == JOINED_LIFECYCLE_MODE || campaign_mode == MULTIBOUNDARY_JOINED_MODE
}

fn boundary_count(campaign_mode: &str) -> u64 {
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        2
    } else {
        1
    }
}

fn publish_lag_seconds(campaign_mode: &str) -> i64 {
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        120
    } else {
        180
    }
}

fn window_publish_times(campaign_mode: &str, publish_time: i64) -> Result<Vec<i64>> {
    let count = i64::try_from(boundary_count(campaign_mode))?;
    let first = publish_time
        .checked_sub(
            count
                .checked_sub(1)
                .and_then(|value| value.checked_mul(60))
                .ok_or("window publish-time delta overflow")?,
        )
        .ok_or("window publish-time underflow")?;
    (0..count)
        .map(|index| {
            first
                .checked_add(index.checked_mul(60).ok_or("publish-time step overflow")?)
                .ok_or_else(|| "publish-time overflow".into())
        })
        .collect()
}

fn require_repository_head(repository_head: &str) -> Result<()> {
    require(
        repository_head.len() == 40
            && repository_head
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "repository HEAD must be a full lowercase 40-hex commit",
    )
}

fn address(bytes: [u8; 32]) -> Address {
    Address::new_from_array(bytes)
}

fn account(rpc: &Rpc, role: &str, key: Address) -> Result<AccountView> {
    rpc.account(&key.to_string())?
        .ok_or_else(|| format!("{role} {key} is absent").into())
}

fn token_amount(rpc: &Rpc, role: &str, key: Address) -> Result<u64> {
    let view = account(rpc, role, key)?;
    require(
        view.owner == plane::token_program().to_string(),
        format!("{role} is not Token-2022-owned"),
    )?;
    let bytes = view
        .data
        .get(64..72)
        .ok_or_else(|| format!("{role} is too short for a token amount"))?;
    Ok(u64::from_le_bytes(bytes.try_into()?))
}

fn mint_supply(rpc: &Rpc, role: &str, key: Address) -> Result<u64> {
    let view = account(rpc, role, key)?;
    require(
        view.owner == plane::token_program().to_string(),
        format!("{role} is not Token-2022-owned"),
    )?;
    let bytes = view
        .data
        .get(36..44)
        .ok_or_else(|| format!("{role} is too short for a mint supply"))?;
    Ok(u64::from_le_bytes(bytes.try_into()?))
}

fn compute_budget() -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
}

fn heap_frame() -> Instruction {
    let mut data = vec![1_u8];
    data.extend_from_slice(&262_144_u32.to_le_bytes());
    Instruction::new_with_bytes(COMPUTE_BUDGET, &data, vec![])
}

fn wait_for_slot(rpc: &Rpc, target: u64, reason: &str) -> Result<()> {
    let mut last_reported = 0_u64;
    for _attempt in 0..28_800 {
        let current = rpc.slot()?;
        if current >= target {
            return Ok(());
        }
        if current >= last_reported.saturating_add(64) {
            println!("waiting reason={reason} slot={current} target={target}");
            last_reported = current;
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!("timed out waiting for {reason} at slot {target}").into())
}

fn sign_submit(
    rpc: &Rpc,
    payer: &Keypair,
    extras: &[&Keypair],
    instructions: &[Instruction],
) -> Result<(String, Value)> {
    for _attempt in 0..3 {
        let blockhash_text = rpc.latest_blockhash()?;
        let blockhash = Hash::from_str(&blockhash_text)
            .map_err(|error| format!("blockhash {blockhash_text}: {error}"))?;
        let mut signers = vec![payer];
        signers.extend_from_slice(extras);
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let expected_signature = transaction
            .signatures
            .first()
            .ok_or("signed transaction has no payer signature")?
            .to_string();
        let wire = bincode::serialize(&transaction)?;
        if let Some((signature, mut confirmed)) =
            rpc.submit_and_confirm(&wire, &blockhash_text, &expected_signature)?
        {
            let status = confirmed
                .as_object_mut()
                .ok_or("confirmed status is not a JSON object")?;
            status.insert(
                "_local_wire_sha256".to_string(),
                Value::String(provider::sha256(&wire)),
            );
            status.insert(
                "_local_program_order".to_string(),
                json!(instructions
                    .iter()
                    .map(|instruction| instruction.program_id.to_string())
                    .collect::<Vec<_>>()),
            );
            return Ok((signature, confirmed));
        }
    }
    Err("transaction expired unobserved across three blockhashes".into())
}

fn require_accepted(role: &str, status: &Value) -> Result<()> {
    require(
        status.get("err").is_some_and(Value::is_null),
        format!("{role} was refused unexpectedly: {status}"),
    )
}

fn require_refused(role: &str, status: &Value) -> Result<()> {
    require(
        status.get("err").is_some_and(|error| !error.is_null()),
        format!("{role} has no explicit on-ledger refusal: {status}"),
    )
}

fn instruction_refusal(status: &Value) -> Option<(u64, u64)> {
    let parts = status.get("err")?.get("InstructionError")?.as_array()?;
    Some((
        parts.first()?.as_u64()?,
        parts.get(1)?.get("Custom")?.as_u64()?,
    ))
}

fn require_source_admission_refused(role: &str, status: &Value) -> Result<()> {
    require_refused(role, status)?;
    require(
        instruction_refusal(status) == Some((2, 0x007a)),
        format!(
            "{role} did not fail at adjacent Clutch instruction 2 with SourceAdmissionFailed: {status}"
        ),
    )
}

fn record_step(
    rpc: &Rpc,
    records: &mut Vec<Value>,
    label: &str,
    signature: String,
    status: &Value,
) -> Result<()> {
    let transaction = rpc.transaction(&signature)?;
    require(
        transaction
            .get("transaction")
            .and_then(|transaction| transaction.get("signatures"))
            .and_then(Value::as_array)
            .and_then(|signatures| signatures.first())
            .and_then(Value::as_str)
            == Some(&signature),
        format!("getTransaction does not contain submitted signature for {label}"),
    )?;
    let transaction_error = transaction
        .get("meta")
        .and_then(|meta| meta.get("err"))
        .ok_or_else(|| format!("transaction {signature} has no meta.err"))?;
    require(
        Some(transaction_error) == status.get("err"),
        format!("status/getTransaction error mismatch for {label}"),
    )?;
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("transaction {signature} has no integer slot"))?;
    let compute_units = transaction
        .get("meta")
        .and_then(|meta| meta.get("computeUnitsConsumed"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("transaction {signature} has no integer compute units"))?;
    let fee = transaction
        .get("meta")
        .and_then(|meta| meta.get("fee"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("transaction {signature} has no integer fee"))?;
    let wire_sha256 = status
        .get("_local_wire_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("transaction {signature} has no local wire hash"))?;
    let program_order = status
        .get("_local_program_order")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("transaction {signature} has no local program order"))?;
    require(
        !program_order.is_empty() && program_order.iter().all(Value::is_string),
        format!("transaction {signature} has malformed local program order"),
    )?;
    println!(
        "step={label} slot={} cu={} error={}",
        slot, compute_units, transaction_error
    );
    records.push(json!({
        "label": label,
        "signature": signature,
        "slot": slot,
        "compute_units_consumed": compute_units,
        "fee_lamports": fee,
        "signed_wire_sha256": wire_sha256,
        "program_order": program_order,
        "error": transaction_error,
    }));
    Ok(())
}

#[derive(Clone)]
struct GenesisRow {
    role: String,
    address: Address,
    owner: Address,
    data: Vec<u8>,
    executable: bool,
}

fn insert_row(rows: &mut BTreeMap<String, GenesisRow>, row: GenesisRow) -> Result<()> {
    let key = row.address.to_string();
    if let Some(existing) = rows.get(&key) {
        require(
            existing.owner == row.owner
                && existing.data == row.data
                && existing.executable == row.executable,
            format!(
                "genesis collision at {} between {} and {}",
                row.address, existing.role, row.role
            ),
        )?;
    } else {
        rows.insert(key, row);
    }
    Ok(())
}

fn add_plane_rows(
    rows: &mut BTreeMap<String, GenesisRow>,
    prefix: &str,
    lab: &plane::LabPlane,
) -> Result<()> {
    for (index, genesis) in lab.plane.accounts.iter().enumerate() {
        insert_row(
            rows,
            GenesisRow {
                role: format!("{prefix}-plane-{index}"),
                address: genesis.address,
                owner: genesis.owner,
                data: genesis.data.clone(),
                executable: false,
            },
        )?;
    }
    if lab.market_prestate == plane::MarketPrestate::GenesisFunded {
        for (index, mint) in lab.plane.outcome_mints.iter().enumerate() {
            insert_row(
                rows,
                GenesisRow {
                    role: format!("{prefix}-outcome-mint-{index}"),
                    address: mint.address,
                    owner: plane::token_program(),
                    data: outcome_mint_bytes(lab.plane.market.address, 0),
                    executable: false,
                },
            )?;
        }
    }
    Ok(())
}

fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut out = vec![0_u8; 82];
    out[36..44].copy_from_slice(&supply.to_le_bytes());
    out[44] = 6;
    out[45] = 1;
    out
}

fn expected_genesis_rows(
    correct: &plane::LabPlane,
    clutch_elf: &[u8],
    second_owner: Address,
) -> Result<BTreeMap<String, GenesisRow>> {
    let mut rows = BTreeMap::new();
    let loader = address(UPGRADEABLE_LOADER_ID);
    for deployment in provider::deployment_accounts()? {
        insert_row(
            &mut rows,
            GenesisRow {
                role: deployment.role.to_string(),
                address: deployment.address,
                owner: loader,
                data: deployment.data,
                executable: deployment.executable,
            },
        )?;
    }
    let clutch_programdata = Address::find_program_address(&[PROGRAM_ID.as_ref()], &loader).0;
    insert_row(
        &mut rows,
        GenesisRow {
            role: "clutch-program".to_string(),
            address: PROGRAM_ID,
            owner: loader,
            data: receiver_program_body(clutch_programdata.to_bytes()),
            executable: true,
        },
    )?;
    insert_row(
        &mut rows,
        GenesisRow {
            role: "clutch-programdata".to_string(),
            address: clutch_programdata,
            owner: loader,
            data: programdata_body(WARP_SLOT, None, [0; 32], clutch_elf),
            executable: false,
        },
    )?;
    add_plane_rows(&mut rows, "correct", correct)?;
    if correct.market_prestate == plane::MarketPrestate::SignedCreate {
        insert_row(
            &mut rows,
            GenesisRow {
                role: "joined-collateral-mint".to_string(),
                address: plane::COLLATERAL_MINT,
                owner: plane::token_program(),
                data: collateral_mint_bytes(plane::JOINED_COLLATERAL_SUPPLY),
                executable: false,
            },
        )?;
        insert_row(
            &mut rows,
            GenesisRow {
                role: "joined-second-owner-collateral-token".to_string(),
                address: plane::actor_collateral(second_owner),
                owner: plane::token_program(),
                data: token_account_bytes(
                    plane::COLLATERAL_MINT,
                    second_owner,
                    plane::USER_COLLATERAL_ATOMS,
                ),
                executable: false,
            },
        )?;
        insert_row(
            &mut rows,
            GenesisRow {
                role: "joined-user-collateral-token".to_string(),
                address: plane::actor_collateral(correct.plane.actor),
                owner: plane::token_program(),
                data: token_account_bytes(
                    plane::COLLATERAL_MINT,
                    correct.plane.actor,
                    plane::USER_COLLATERAL_ATOMS,
                ),
                executable: false,
            },
        )?;
    }
    Ok(rows)
}

fn write_key(path: &Path, key: &Keypair) -> Result<()> {
    write_keypair_file(key, path).map_err(|error| format!("{}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn write_row(path: &Path, row: &GenesisRow) -> Result<()> {
    let lamports = Rent::default().minimum_balance(row.data.len()).max(1);
    let body = json!({
        "pubkey": row.address.to_string(),
        "account": {
            "lamports": lamports,
            "data": [BASE64.encode(&row.data), "base64"],
            "owner": row.owner.to_string(),
            "executable": row.executable,
            "rentEpoch": 0,
            "space": row.data.len(),
        }
    });
    fs::write(path, serde_json::to_vec_pretty(&body)?)?;
    Ok(())
}

fn repository_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn validator_build_record(validator: &Path) -> Result<Vec<u8>> {
    let validator = fs::canonicalize(validator)?;
    let root = validator
        .parent()
        .and_then(Path::parent)
        .ok_or("validator path has no build root")?;
    let bytes = fs::read(root.join("build-provenance.txt"))?;
    let text = std::str::from_utf8(&bytes)?;
    let field = |name: &str| -> Result<&str> {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .ok_or_else(|| format!("validator build record has no {name}").into())
    };
    require(
        field("format")? == "dragons-clutch-agave-loopback-build-v1",
        "validator build record format differs",
    )?;
    require(
        field("upstream_commit")? == "549805f3e85f345c9df98d59759691443eef57aa",
        "validator build record upstream commit differs",
    )?;
    let patch_hash = provider::sha256(&fs::read(repository_file(VALIDATOR_PATCH))?);
    require(
        field("patch_sha256")? == patch_hash,
        "validator build record patch differs from tracked patch",
    )?;
    let pins = fs::read_to_string(repository_file(VALIDATOR_PINS))?;
    require(
        pins.lines()
            .any(|line| line == format!("AGAVE_PATCH_SHA256={patch_hash}")),
        "validator build patch is not bound by tracked pins",
    )?;
    require(
        fs::canonicalize(field("binary_path")?)? == validator,
        "selected validator is not the binary named by its build record",
    )?;
    require(
        field("binary_sha256")? == provider::sha256(&fs::read(&validator)?),
        "selected validator hash differs from its build record",
    )?;
    Ok(bytes)
}

fn prepare(
    work: &Path,
    elf: &Path,
    validator: &Path,
    repository_head: &str,
    clock_probe_time: i64,
    publish_time: i64,
    campaign_mode: &str,
) -> Result<()> {
    require_repository_head(repository_head)?;
    require(
        work.is_dir(),
        format!("work directory {} is absent", work.display()),
    )?;
    for output in [
        "accounts",
        "lab-secrets",
        "campaign.json",
        "genesis.tsv",
        "payer.pubkey",
        "second-owner.pubkey",
        "program.pubkey",
    ] {
        require(
            !work.join(output).exists(),
            format!("refusing to overwrite existing laboratory output {output}"),
        )?;
    }
    let elf_bytes = fs::read(elf)?;
    require(!elf_bytes.is_empty(), "Clutch ELF is empty")?;
    let validator_bytes = fs::read(validator)?;
    require(!validator_bytes.is_empty(), "validator binary is empty")?;
    let validator_build_record = validator_build_record(validator)?;
    let accounts_dir = work.join("accounts");
    let secrets_dir = work.join("lab-secrets");
    fs::create_dir_all(&accounts_dir)?;
    fs::create_dir_all(&secrets_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&secrets_dir, fs::Permissions::from_mode(0o700))?;
    }

    let payer = Keypair::new();
    let second_owner = Keypair::new();
    write_key(&secrets_dir.join("payer.json"), &payer)?;
    write_key(&secrets_dir.join("second-owner.json"), &second_owner)?;
    fs::write(work.join("payer.pubkey"), format!("{}\n", payer.pubkey()))?;
    fs::write(
        work.join("second-owner.pubkey"),
        format!("{}\n", second_owner.pubkey()),
    )?;
    fs::write(work.join("program.pubkey"), format!("{PROGRAM_ID}\n"))?;

    require(publish_time > 0, "publish time must be positive")?;
    let publish_lag = publish_lag_seconds(campaign_mode);
    let expected_publish_time = clock_probe_time
        .checked_sub(publish_lag)
        .ok_or("clock-probe time underflow")?
        .div_euclid(60)
        * 60;
    require(
        publish_time == expected_publish_time,
        format!(
            "publish time is not the named {publish_lag}-second boundary behind the probe Clock"
        ),
    )?;
    let end_bucket = u64::try_from(publish_time)?.div_euclid(60);
    let start_bucket = end_bucket
        .checked_sub(boundary_count(campaign_mode))
        .ok_or("bucket underflow")?;
    let market_prestate = if is_joined_mode(campaign_mode) {
        plane::MarketPrestate::SignedCreate
    } else {
        plane::MarketPrestate::GenesisFunded
    };
    let correct = plane::build(
        payer.pubkey(),
        plane::real_spec()?,
        start_bucket,
        end_bucket,
        clutch_svm_fixture::MARKET_NONCE,
        market_prestate,
    );
    require(
        correct.start_bucket == start_bucket && correct.end_bucket_exclusive == end_bucket,
        "rebuilt plane window differs from manifest",
    )?;
    let publish_times = window_publish_times(campaign_mode, publish_time)?;
    let observations = publish_times
        .iter()
        .map(|publish_time| provider::observation(*publish_time))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let observation = observations
        .last()
        .ok_or("campaign generated no source observations")?;
    let wrong_feed_observation = provider::observation_for_feed(publish_time, WRONG_FEED_ID)?;
    let merkle_update_hash = provider::sha256(&borsh::to_vec(&observation.update)?);
    let wrong_feed_merkle_update_hash =
        provider::sha256(&borsh::to_vec(&wrong_feed_observation.update)?);
    let window_observation_manifest = observations
        .iter()
        .enumerate()
        .map(|(index, observation)| {
            json!({
                "bucket": start_bucket + u64::try_from(index).expect("observation index fits u64"),
                "publish_time": publish_times[index],
                "vaa_sha256": provider::sha256(&observation.vaa),
                "post_update_data_sha256": provider::sha256(&observation.post_data),
                "merkle_price_update_sha256": provider::sha256(&borsh::to_vec(&observation.update).expect("observation encodes")),
            })
        })
        .collect::<Vec<_>>();

    let rows = expected_genesis_rows(&correct, &elf_bytes, second_owner.pubkey())?;

    let mut index = String::new();
    for (number, row) in rows.values().enumerate() {
        let filename = format!("accounts/{number:03}-{}.json", row.role);
        write_row(&work.join(&filename), row)?;
        index.push_str(&format!("{}\t{}\t{}\n", row.role, row.address, filename));
    }
    fs::write(work.join("genesis.tsv"), index)?;

    let mut manifest = json!({
        "claim": CLAIM,
        "transcript_schema": transcript_schema(campaign_mode),
        "campaign_mode": campaign_mode,
        "network": "127.0.0.1 loopback only",
        "observation": "synthetic deterministic local guardian quorum; not devnet price evidence",
        "value": "none",
        "upstream_pyth_crosschain_commit": UPSTREAM_COMMIT,
        "dragons_clutch_repository_head": repository_head,
        "fixture_provenance": "programs/clutch-sbf/svm-tests/tests/fixtures/real-pyth-local/PROVENANCE.md",
        "source_profile_snapshot": {
            "path": "programs/clutch-sbf/source-profiles/devnet-real-source-snapshot-2026-08-22.json",
            "sha256": provider::sha256(&fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCE_PROFILE_SNAPSHOT))?),
        },
        "validator_build_provenance": {
            "provenance_sha256": provider::sha256(&fs::read(repository_file(VALIDATOR_PROVENANCE))?),
            "pins_sha256": provider::sha256(&fs::read(repository_file(VALIDATOR_PINS))?),
            "patch_sha256": provider::sha256(&fs::read(repository_file(VALIDATOR_PATCH))?),
            "selected_build_record_sha256": provider::sha256(&validator_build_record),
        },
        "guardian_laboratory": {
            "generation": "19 throwaway secp256k1 scalars with first byte 1..19 and remaining bytes zero",
            "selected_quorum_indices": "0..12 inclusive",
            "production_keys": false,
        },
        "publish_time": publish_time,
        "clock_probe_unix_timestamp": clock_probe_time,
        "publish_time_derivation": format!("floor((same-validator warped Clock - {publish_lag} seconds) / 60) * 60"),
        "start_bucket": start_bucket,
        "end_bucket_exclusive": end_bucket,
        "warp_slot": WARP_SLOT,
        "payer": payer.pubkey().to_string(),
        "second_owner": second_owner.pubkey().to_string(),
        "program_id": PROGRAM_ID.to_string(),
        "clutch_elf_sha256": provider::sha256(&elf_bytes),
        "validator_binary": validator.display().to_string(),
        "validator_binary_sha256": provider::sha256(&validator_bytes),
        "build_toolchain": {
            "host_toolchain_pin": "programs/clutch-sbf/local-real-pyth/rust-toolchain.toml: 1.93.1",
            "host_cargo": "cargo 1.93.1 (083ac5135 2025-12-15)",
            "host_rustc": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
            "cargo_build_sbf": "cargo-build-sbf 4.0.0; platform-tools v1.53; SBF rustc 1.89.0",
            "cargo_build_sbf_binary_sha256": "37c37d1a2ef0aa44065cde8c6ad07f0685bcef24699b4a9dd101372d7d4ef6e7",
            "host_cargo_lock_sha256": provider::sha256(&fs::read(repository_file("Cargo.lock"))?),
            "sbf_cargo_lock_sha256": provider::sha256(&fs::read(repository_file("../Cargo.lock"))?),
            "network": "offline",
            "dependency_source": "locked repository-local .cache vendor tree",
            "cargo_home": "private temporary campaign directory",
            "wrappers_and_rustflags": "unset by runner",
        },
        "vaa_sha256": provider::sha256(&observation.vaa),
        "post_update_data_sha256": provider::sha256(&observation.post_data),
        "merkle_price_update_sha256": merkle_update_hash,
        "source_admission_limits": {
            "bucket_seconds": 60,
            "boundary_grace_seconds": 5,
            "max_staleness_slots": 500,
            "max_staleness_seconds": 600,
            "max_future_seconds": 15,
            "max_confidence_atoms": "1000000000000",
            "max_confidence_bps": 500,
            "confidence_multiplier": 3,
        },
        "genesis_accounts": rows.values().map(|row| json!({
            "role": row.role,
            "address": row.address.to_string(),
            "owner": row.owner.to_string(),
            "data_sha256": provider::sha256(&row.data),
            "data_len": row.data.len(),
            "lamports": Rent::default().minimum_balance(row.data.len()).max(1),
            "executable": row.executable,
        })).collect::<Vec<_>>(),
        "provider": provider::deployment_accounts()?.iter().map(|row| json!({
            "role": row.role,
            "address": row.address.to_string(),
            "complete_account_body_sha256": row.expected_hash,
            "executable": row.executable,
        })).collect::<Vec<_>>(),
        "correct": {
            "feed_id_hex": provider::FEED_ID.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "source_spec": correct.plane.source_spec.address.to_string(),
            "archive": correct.plane.source_archive.address.to_string(),
            "market": correct.plane.market.address.to_string(),
            "market_genesis_assisted": market_prestate == plane::MarketPrestate::GenesisFunded,
            "user_collateral_token": if market_prestate == plane::MarketPrestate::SignedCreate {
                Some(plane::actor_collateral(payer.pubkey()).to_string())
            } else {
                None
            },
            "second_owner": if market_prestate == plane::MarketPrestate::SignedCreate {
                Some(second_owner.pubkey().to_string())
            } else {
                None
            },
            "second_owner_collateral_token": if market_prestate == plane::MarketPrestate::SignedCreate {
                Some(plane::actor_collateral(second_owner.pubkey()).to_string())
            } else {
                None
            },
        },
        "wrong_feed": {
            "feed_id_hex": WRONG_FEED_ID.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "vaa_sha256": provider::sha256(&wrong_feed_observation.vaa),
            "post_update_data_sha256": provider::sha256(&wrong_feed_observation.post_data),
            "merkle_price_update_sha256": wrong_feed_merkle_update_hash,
        }
    });
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        let fields = manifest
            .as_object_mut()
            .ok_or("campaign manifest is not a JSON object")?;
        fields.insert(
            "boundary_count".to_string(),
            json!(boundary_count(campaign_mode)),
        );
        fields.insert(
            "window_observations".to_string(),
            json!(window_observation_manifest),
        );
    }
    fs::write(
        work.join("campaign.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("{CLAIM}");
    println!("prepared {} exact genesis accounts", rows.len());
    Ok(())
}

fn manifest(work: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        work.join("campaign.json"),
    )?)?)
}

fn manifest_i64(value: &Value, key: &str) -> Result<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("campaign manifest has no integer {key}").into())
}

fn verify_provider_accounts(rpc: &Rpc) -> Result<()> {
    let loader = address(UPGRADEABLE_LOADER_ID).to_string();
    for expected in provider::deployment_accounts()? {
        let actual = account(rpc, expected.role, expected.address)?;
        require(
            actual.owner == loader,
            format!("{} wrong loader owner", expected.role),
        )?;
        require(
            actual.executable == expected.executable,
            format!("{} executable bit differs", expected.role),
        )?;
        require(
            provider::sha256(&actual.data) == expected.expected_hash,
            format!("{} complete account body hash differs", expected.role),
        )?;
    }
    let loader_id = address(UPGRADEABLE_LOADER_ID);
    require(
        Address::find_program_address(&[&real_pyth_lab::RECEIVER_PROGRAM], &loader_id).0
            == address(real_pyth_lab::RECEIVER_PROGRAMDATA),
        "receiver ProgramData is not canonical loader PDA",
    )?;
    require(
        Address::find_program_address(&[&real_pyth_lab::ROUTER_PROGRAM], &loader_id).0
            == address(real_pyth_lab::ROUTER_PROGRAMDATA),
        "router ProgramData is not canonical loader PDA",
    )
}

fn verify_genesis_manifest(
    rpc: &Rpc,
    public: &Value,
    correct: &plane::LabPlane,
    clutch_elf: &[u8],
    second_owner: Address,
) -> Result<()> {
    let rows = public
        .get("genesis_accounts")
        .and_then(Value::as_array)
        .ok_or("campaign manifest has no genesis_accounts array")?;
    let mut expected = expected_genesis_rows(correct, clutch_elf, second_owner)?;
    require(
        rows.len() == expected.len(),
        format!(
            "campaign manifest has {} genesis rows, rebuilt campaign requires exactly {}",
            rows.len(),
            expected.len()
        ),
    )?;
    let mut seen = BTreeMap::new();
    for row in rows {
        let role = row
            .get("role")
            .and_then(Value::as_str)
            .ok_or("genesis manifest row has no role")?;
        let address_text = row
            .get("address")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("genesis manifest row {role} has no address"))?;
        require(
            seen.insert(address_text, role).is_none(),
            format!("genesis manifest repeats address {address_text}"),
        )?;
        let expected_owner = row
            .get("owner")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("genesis manifest row {role} has no owner"))?;
        let expected_hash = row
            .get("data_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("genesis manifest row {role} has no data hash"))?;
        let expected_len = row
            .get("data_len")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("genesis manifest row {role} has no data length"))?;
        let expected_executable = row
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("genesis manifest row {role} has no executable bit"))?;
        let key = Address::from_str(address_text)
            .map_err(|error| format!("genesis address {address_text}: {error}"))?;
        let rebuilt = expected.remove(address_text).ok_or_else(|| {
            format!("genesis manifest contains unexpected account {address_text}")
        })?;
        require(rebuilt.role == role, format!("{role} rebuilt role differs"))?;
        require(
            rebuilt.owner.to_string() == expected_owner,
            format!("{role} rebuilt owner differs from manifest"),
        )?;
        require(
            rebuilt.executable == expected_executable,
            format!("{role} rebuilt executable bit differs from manifest"),
        )?;
        require(
            u64::try_from(rebuilt.data.len())? == expected_len,
            format!("{role} rebuilt data length differs from manifest"),
        )?;
        require(
            provider::sha256(&rebuilt.data) == expected_hash,
            format!("{role} rebuilt data hash differs from manifest"),
        )?;
        let expected_lamports = row
            .get("lamports")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("genesis manifest row {role} has no lamports"))?;
        require(
            expected_lamports == Rent::default().minimum_balance(rebuilt.data.len()).max(1),
            format!("{role} manifest lamports differ from rebuilt rent exemption"),
        )?;
        let actual = account(rpc, role, key)?;
        require(
            actual.owner == expected_owner,
            format!("{role} owner differs"),
        )?;
        require(
            actual.lamports == expected_lamports,
            format!("{role} lamports differ"),
        )?;
        require(
            actual.executable == expected_executable,
            format!("{role} executable bit differs"),
        )?;
        require(
            u64::try_from(actual.data.len())? == expected_len,
            format!("{role} data length differs"),
        )?;
        require(
            provider::sha256(&actual.data) == expected_hash,
            format!("{role} data hash differs"),
        )?;
    }
    require(
        expected.is_empty(),
        "campaign manifest omitted a rebuilt genesis account",
    )?;
    let loader = address(UPGRADEABLE_LOADER_ID);
    let clutch_programdata = Address::find_program_address(&[PROGRAM_ID.as_ref()], &loader).0;
    require(
        account(rpc, "Clutch ProgramData", clutch_programdata)?.owner == loader.to_string(),
        "Clutch ProgramData is not loader-owned",
    )?;
    require(
        public.get("clutch_elf_sha256").and_then(Value::as_str)
            == Some(&provider::sha256(clutch_elf)),
        "running Clutch ProgramData ELF differs from prepared ELF hash",
    )
}

fn router_initialize(payer: Address) -> Result<Instruction> {
    let router = address(real_pyth_lab::ROUTER_PROGRAM);
    Ok(Instruction::new_with_bytes(
        router,
        &provider::fixture("router-initialize.data")?,
        vec![
            AccountMeta::new(
                Address::find_program_address(&[b"Bridge"], &router).0,
                false,
            ),
            AccountMeta::new(
                Address::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router).0,
                false,
            ),
            AccountMeta::new(
                Address::find_program_address(&[b"fee_collector"], &router).0,
                false,
            ),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(address(CLOCK_SYSVAR_ID), false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        ],
    ))
}

fn write_vaa(
    router: Address,
    payer: Address,
    encoded: Address,
    index: usize,
    bytes: &[u8],
) -> Instruction {
    let mut data = WRITE_ENCODED_VAA.to_vec();
    data.extend_from_slice(&u32::try_from(index).expect("VAA index fits").to_le_bytes());
    data.extend_from_slice(
        &u32::try_from(bytes.len())
            .expect("VAA length fits")
            .to_le_bytes(),
    );
    data.extend_from_slice(bytes);
    Instruction::new_with_bytes(
        router,
        &data,
        vec![
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new(encoded, false),
        ],
    )
}

fn upload_and_verify_vaa(
    rpc: &Rpc,
    payer: &Keypair,
    steps: &mut Vec<Value>,
    observation: &provider::Observation,
    role: &str,
    allocate_step: &str,
    verify_step: &str,
) -> Result<Keypair> {
    let router = address(real_pyth_lab::ROUTER_PROGRAM);
    let encoded = Keypair::new();
    let encoded_len = VAA_START + observation.vaa.len();
    let split = observation.vaa.len().min(WRITE_SPLIT);
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &encoded.pubkey(),
        rpc.minimum_rent(encoded_len)?,
        u64::try_from(encoded_len)?,
        &router,
    );
    let init = Instruction::new_with_bytes(
        router,
        &INIT_ENCODED_VAA,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(encoded.pubkey(), false),
        ],
    );
    let (signature, status) = sign_submit(
        rpc,
        payer,
        &[&encoded],
        &[
            compute_budget(),
            create,
            init,
            write_vaa(
                router,
                payer.pubkey(),
                encoded.pubkey(),
                0,
                &observation.vaa[..split],
            ),
        ],
    )?;
    require_accepted(
        &format!("{role} router encoded VAA allocation/write"),
        &status,
    )?;
    record_step(rpc, steps, allocate_step, signature, &status)?;

    let guardian_set =
        Address::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router).0;
    let mut verify = vec![compute_budget()];
    if split < observation.vaa.len() {
        verify.push(write_vaa(
            router,
            payer.pubkey(),
            encoded.pubkey(),
            split,
            &observation.vaa[split..],
        ));
    }
    verify.push(Instruction::new_with_bytes(
        router,
        &VERIFY_ENCODED_VAA_V1,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(encoded.pubkey(), false),
            AccountMeta::new_readonly(guardian_set, false),
        ],
    ));
    let (signature, status) = sign_submit(rpc, payer, &[], &verify)?;
    require_accepted(&format!("{role} real router VerifyEncodedVaa"), &status)?;
    record_step(rpc, steps, verify_step, signature, &status)?;

    let encoded_view = account(rpc, &format!("{role} encoded VAA"), encoded.pubkey())?;
    require(
        encoded_view.owner == router.to_string(),
        format!("{role} Verified VAA account is not router-owned"),
    )?;
    require(
        !encoded_view.executable && encoded_view.data.len() == encoded_len,
        format!("{role} Verified VAA account shape differs"),
    )?;
    require(
        encoded_view.data.get(8) == Some(&2),
        format!("{role} router did not persist Verified state"),
    )?;
    require(
        encoded_view.data.get(VAA_START..) == Some(observation.vaa.as_slice()),
        format!("{role} Verified VAA payload differs from locally signed bytes"),
    )?;
    Ok(encoded)
}

fn receiver_initialize(payer: Address) -> Result<Instruction> {
    Ok(Instruction::new_with_bytes(
        address(real_pyth_lab::RECEIVER_PROGRAM),
        &provider::fixture("receiver-initialize.data")?,
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(address(real_pyth_lab::RECEIVER_CONFIG), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        ],
    ))
}

fn receiver_post(
    payer: Address,
    encoded: Address,
    update: Address,
    post_data: &[u8],
) -> Instruction {
    let receiver = address(real_pyth_lab::RECEIVER_PROGRAM);
    let treasury = Address::find_program_address(&[b"treasury", &[0]], &receiver).0;
    Instruction::new_with_bytes(
        receiver,
        post_data,
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(encoded, false),
            AccountMeta::new_readonly(address(real_pyth_lab::RECEIVER_CONFIG), false),
            AccountMeta::new(treasury, false),
            AccountMeta::new(update, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(payer, true),
        ],
    )
}

fn read_clock(rpc: &Rpc) -> Result<Clock> {
    let view = account(rpc, "Clock sysvar", address(CLOCK_SYSVAR_ID))?;
    require(
        view.owner == address(clutch_sbf::instructions_sysvar::SYSVAR_OWNER_ID).to_string(),
        "Clock account is not owned by the Sysvar program",
    )?;
    require(!view.executable, "Clock account is unexpectedly executable")?;
    let clock: Clock = bincode::deserialize(&view.data)?;
    require(
        clock.slot >= WARP_SLOT,
        format!("Clock slot {} is below {WARP_SLOT}", clock.slot),
    )?;
    let rpc_slot = rpc.slot()?;
    require(
        clock.slot <= rpc_slot,
        format!(
            "Clock slot {} is ahead of current RPC slot {rpc_slot}",
            clock.slot
        ),
    )?;
    Ok(clock)
}

fn require_append_freshness(clock: &Clock, publish_time: i64) -> Result<i64> {
    require(
        clock.unix_timestamp >= publish_time + 60,
        format!(
            "Clock {} has not reached observation maturity {}",
            clock.unix_timestamp,
            publish_time + 60
        ),
    )?;
    let age = clock
        .unix_timestamp
        .checked_sub(publish_time)
        .ok_or("append Clock age underflow")?;
    require(
        age <= 300,
        format!(
            "laboratory observation is more than five minutes old: Clock {}, publish {publish_time}",
            clock.unix_timestamp
        ),
    )?;
    Ok(age)
}

fn assert_clock(rpc: &Rpc, publish_time: i64) -> Result<Clock> {
    let clock = read_clock(rpc)?;
    require_append_freshness(&clock, publish_time)?;
    Ok(clock)
}

fn source_freshness_evidence(
    append_clock: &Clock,
    final_clock: &Clock,
    publish_time: i64,
) -> Result<Value> {
    let append_age_seconds = require_append_freshness(append_clock, publish_time)?;
    require(
        final_clock.slot >= append_clock.slot,
        "final Clock slot precedes append-time Clock",
    )?;
    require(
        final_clock.unix_timestamp >= append_clock.unix_timestamp,
        "final Clock timestamp precedes append-time Clock",
    )?;
    let final_age_seconds = final_clock
        .unix_timestamp
        .checked_sub(publish_time)
        .ok_or("final Clock age underflow")?;
    Ok(json!({
        "scope": "freshness authenticated at adjacent PostUpdate plus AppendSourceArchiveV2; final lifecycle consumes the sealed archive",
        "append_clock": append_clock,
        "append_age_seconds": append_age_seconds,
        "final_clock": final_clock,
        "final_age_seconds": final_age_seconds,
    }))
}

fn print_clock(url: &str) -> Result<()> {
    let rpc = Rpc::new(url)?;
    for _attempt in 0..120 {
        let rpc_slot = rpc.slot()?;
        let view = account(&rpc, "Clock sysvar", address(CLOCK_SYSVAR_ID))?;
        let clock: Clock = bincode::deserialize(&view.data)?;
        if rpc_slot >= CLOCK_SETTLED_SLOT && clock.slot >= CLOCK_SETTLED_SLOT {
            thread::sleep(Duration::from_secs(1));
            let later_rpc_slot = rpc.slot()?;
            let later_view = account(&rpc, "Clock sysvar", address(CLOCK_SYSVAR_ID))?;
            let later: Clock = bincode::deserialize(&later_view.data)?;
            let delta = i128::from(later.unix_timestamp) - i128::from(clock.unix_timestamp);
            if later_rpc_slot >= CLOCK_SETTLED_SLOT
                && later.slot >= CLOCK_SETTLED_SLOT
                && (0..=3).contains(&delta)
            {
                println!("{}", later.unix_timestamp);
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!("warped RPC/Clock did not settle at or above slot {CLOCK_SETTLED_SLOT}").into())
}

fn snapshot(rpc: &Rpc, keys: &[Address]) -> Result<Vec<Option<AccountView>>> {
    keys.iter()
        .map(|key| rpc.account(&key.to_string()))
        .collect()
}

/// Hash one ordered set of complete RPC account observations.
///
/// The encoding is domain || target-count(u64 LE), followed for every target
/// by key(32) || presence(u8) and, when present, lamports(u64 LE) || owner(32)
/// || executable(u8) || data-length(u64 LE) || complete data. The target keys
/// are therefore committed even when an account is absent.
fn rollback_snapshot_sha256(keys: &[Address], accounts: &[Option<AccountView>]) -> Result<String> {
    require(
        keys.len() == accounts.len(),
        "rollback snapshot key/account lengths differ",
    )?;
    let mut canonical = Vec::new();
    canonical.extend_from_slice(ROLLBACK_SNAPSHOT_DOMAIN);
    canonical.extend_from_slice(&u64::try_from(keys.len())?.to_le_bytes());
    for (key, account) in keys.iter().zip(accounts) {
        canonical.extend_from_slice(key.as_ref());
        if let Some(account) = account {
            canonical.push(1);
            canonical.extend_from_slice(&account.lamports.to_le_bytes());
            canonical.extend_from_slice(Address::from_str(&account.owner)?.as_ref());
            canonical.push(u8::from(account.executable));
            canonical.extend_from_slice(&u64::try_from(account.data.len())?.to_le_bytes());
            canonical.extend_from_slice(&account.data);
        } else {
            canonical.push(0);
        }
    }
    Ok(provider::sha256(&canonical))
}

fn hex32(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn archive_record_evidence(index: usize, record: ArchiveRecordV2) -> Value {
    json!({
        "index": index,
        "bucket": record.bucket.to_string(),
        "lower": record.low.to_string(),
        "upper": record.high.to_string(),
        "sequence": record.sequence.to_string(),
        "write_slot": record.publish_slot.to_string(),
        "publish_time": record.publish_time.to_string(),
    })
}

fn source_archive_evidence(
    key: Address,
    account: &AccountView,
    page_commitment: [u8; 32],
    feed: [u8; 32],
    window: [u8; 32],
    record_count: u8,
) -> Value {
    json!({
        "key": key.to_string(),
        "owner": account.owner,
        "executable": account.executable,
        "data_len": account.data.len(),
        "body_sha256": provider::sha256(&account.data),
        "page_commitment": hex32(page_commitment),
        "feed_id": hex32(feed),
        "window_id": hex32(window),
        "record_count": record_count,
    })
}

fn out_of_order_rollback_evidence(
    skipped_boundary_index: usize,
    skipped_update_account: Address,
    refusal_signature: &str,
    before_snapshot_sha256: &str,
    after_snapshot_sha256: &str,
    archive: Address,
    treasury: Address,
) -> Value {
    json!({
        "ok": before_snapshot_sha256 == after_snapshot_sha256,
        "skipped_boundary_index": skipped_boundary_index,
        "skipped_update_account": skipped_update_account.to_string(),
        "skipped_update_absent_after_refusal": true,
        "refusal_signature": refusal_signature,
        "instruction_error": {
            "instruction_index": 2,
            "custom_code": 122,
            "custom_code_hex": "0x7a",
        },
        "snapshot_encoding": "domain || target_count:u64-le || repeated(key:32 || present:u8 || if-present(lamports:u64-le || owner:32 || executable:u8 || data_len:u64-le || data))",
        "snapshot_domain": String::from_utf8_lossy(ROLLBACK_SNAPSHOT_DOMAIN),
        "watched_accounts": [
            {"role": "source_archive", "address": archive.to_string()},
            {"role": "receiver_treasury", "address": treasury.to_string()},
        ],
        "before_snapshot_sha256": before_snapshot_sha256,
        "after_snapshot_sha256": after_snapshot_sha256,
        "snapshots_equal": before_snapshot_sha256 == after_snapshot_sha256,
    })
}

fn upload_artifact(
    rpc: &Rpc,
    payer: &Keypair,
    steps: &mut Vec<Value>,
    label: &str,
    upload: &plane::ArtifactUpload,
) -> Result<Vec<String>> {
    require(
        rpc.account(&upload.stage.address.to_string())?.is_none(),
        format!("{label} artifact stage was not absent before signed upload"),
    )?;
    require(
        rpc.account(&upload.final_account.address.to_string())?
            .is_none(),
        format!("{label} final artifact was genesis-assisted"),
    )?;
    let expires_slot = rpc
        .slot()?
        .checked_add(256)
        .ok_or("artifact expiry slot overflow")?;
    let mut signatures = Vec::new();
    let (signature, status) = sign_submit(
        rpc,
        payer,
        &[],
        &[
            compute_budget(),
            plane::begin_artifact(payer.pubkey(), upload, expires_slot),
        ],
    )?;
    require_accepted(&format!("{label} BeginArtifact"), &status)?;
    record_step(
        rpc,
        steps,
        &format!("joined-{label}-artifact-begin"),
        signature.clone(),
        &status,
    )?;
    signatures.push(signature);

    let mut cursor = 0_usize;
    for (index, chunk) in upload.body.chunks(192).enumerate() {
        let (signature, status) = sign_submit(
            rpc,
            payer,
            &[],
            &[
                compute_budget(),
                plane::write_artifact(payer.pubkey(), upload, u16::try_from(cursor)?, chunk),
            ],
        )?;
        require_accepted(&format!("{label} WriteArtifact chunk {index}"), &status)?;
        record_step(
            rpc,
            steps,
            &format!("joined-{label}-artifact-write-{index}"),
            signature.clone(),
            &status,
        )?;
        signatures.push(signature);
        cursor += chunk.len();
    }
    require(
        cursor == upload.body.len(),
        format!("{label} artifact upload cursor differs from body length"),
    )?;

    let (signature, status) = sign_submit(
        rpc,
        payer,
        &[],
        &[
            compute_budget(),
            plane::seal_artifact(payer.pubkey(), upload),
        ],
    )?;
    require_accepted(&format!("{label} SealArtifact"), &status)?;
    record_step(
        rpc,
        steps,
        &format!("joined-{label}-artifact-seal"),
        signature.clone(),
        &status,
    )?;
    signatures.push(signature);
    let sealed = account(
        rpc,
        &format!("sealed {label} artifact"),
        upload.final_account.address,
    )?;
    require(
        sealed.owner == PROGRAM_ID.to_string() && !sealed.executable && sealed.data == upload.body,
        format!("sealed {label} artifact bytes or metadata differ"),
    )?;
    require(
        rpc.account(&upload.stage.address.to_string())?.is_none(),
        format!("{label} stage did not close after sealing"),
    )?;
    Ok(signatures)
}

fn accepted_step(
    rpc: &Rpc,
    payer: &Keypair,
    extras: &[&Keypair],
    steps: &mut Vec<Value>,
    label: &str,
    instructions: &[Instruction],
) -> Result<String> {
    let (signature, status) = sign_submit(rpc, payer, extras, instructions)?;
    require_accepted(label, &status)?;
    record_step(rpc, steps, label, signature.clone(), &status)?;
    Ok(signature)
}

fn run(
    work: &Path,
    url: &str,
    validator: &Path,
    repository_head: &str,
    campaign_mode: &str,
) -> Result<()> {
    require_repository_head(repository_head)?;
    let rpc = Rpc::new(url)?;
    let public = manifest(work)?;
    require(
        public.get("claim").and_then(Value::as_str) == Some(CLAIM),
        "campaign truth label differs",
    )?;
    require(
        public.get("campaign_mode").and_then(Value::as_str) == Some(campaign_mode),
        "prepared and requested campaign modes differ",
    )?;
    require(
        public.get("transcript_schema").and_then(Value::as_str)
            == Some(transcript_schema(campaign_mode)),
        "prepared transcript schema differs from the requested campaign",
    )?;
    require(
        public
            .get("dragons_clutch_repository_head")
            .and_then(Value::as_str)
            == Some(repository_head),
        "current repository HEAD differs from prepared manifest",
    )?;
    require(
        public
            .get("validator_binary_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&fs::read(validator)?)),
        "running validator binary hash differs from prepared manifest",
    )?;
    let build_provenance = public
        .get("validator_build_provenance")
        .ok_or("campaign manifest has no validator build provenance")?;
    require(
        build_provenance
            .get("selected_build_record_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&validator_build_record(validator)?)),
        "selected validator build record differs from prepared manifest",
    )?;
    for (field, relative) in [
        ("provenance_sha256", VALIDATOR_PROVENANCE),
        ("pins_sha256", VALIDATOR_PINS),
        ("patch_sha256", VALIDATOR_PATCH),
    ] {
        require(
            build_provenance.get(field).and_then(Value::as_str)
                == Some(&provider::sha256(&fs::read(repository_file(relative))?)),
            format!("tracked validator {field} differs from prepared manifest"),
        )?;
    }
    let source_snapshot = public
        .get("source_profile_snapshot")
        .ok_or("campaign manifest has no source-profile snapshot pin")?;
    require(
        source_snapshot.get("sha256").and_then(Value::as_str)
            == Some(&provider::sha256(&fs::read(repository_file(
                SOURCE_PROFILE_SNAPSHOT,
            ))?)),
        "source-profile snapshot differs from prepared manifest",
    )?;
    let publish_time = manifest_i64(&public, "publish_time")?;
    let start_bucket = u64::try_from(manifest_i64(&public, "start_bucket")?)?;
    let end_bucket = u64::try_from(manifest_i64(&public, "end_bucket_exclusive")?)?;
    require(
        end_bucket.checked_sub(start_bucket) == Some(boundary_count(campaign_mode)),
        "prepared source window width differs from the requested campaign mode",
    )?;
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        require(
            public.get("boundary_count").and_then(Value::as_u64)
                == Some(boundary_count(campaign_mode)),
            "prepared boundary count differs from the multi-boundary schema",
        )?;
    }
    let clutch_elf = fs::read(work.join("elf/clutch_sbf.so"))?;
    require(
        public.get("clutch_elf_sha256").and_then(Value::as_str)
            == Some(&provider::sha256(&clutch_elf)),
        "retained Clutch ELF differs from prepared manifest",
    )?;
    let payer = read_keypair_file(work.join("lab-secrets/payer.json"))
        .map_err(|error| format!("explicit ephemeral payer: {error}"))?;
    let second_owner = read_keypair_file(work.join("lab-secrets/second-owner.json"))
        .map_err(|error| format!("explicit ephemeral second owner: {error}"))?;
    require(
        payer.pubkey() != second_owner.pubkey(),
        "ephemeral owner identities are not distinct",
    )?;
    require(
        public.get("second_owner").and_then(Value::as_str)
            == Some(second_owner.pubkey().to_string().as_str()),
        "explicit second owner differs from prepared manifest",
    )?;
    let market_prestate = if is_joined_mode(campaign_mode) {
        plane::MarketPrestate::SignedCreate
    } else {
        plane::MarketPrestate::GenesisFunded
    };
    let correct = plane::build(
        payer.pubkey(),
        plane::real_spec()?,
        start_bucket,
        end_bucket,
        clutch_svm_fixture::MARKET_NONCE,
        market_prestate,
    );
    let publish_times = window_publish_times(campaign_mode, publish_time)?;
    let observations = publish_times
        .iter()
        .map(|publish_time| provider::observation(*publish_time))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let observation = observations
        .last()
        .ok_or("campaign regenerated no source observations")?;
    let wrong_feed_observation = provider::observation_for_feed(publish_time, WRONG_FEED_ID)?;
    require(
        public.get("vaa_sha256").and_then(Value::as_str)
            == Some(&provider::sha256(&observation.vaa)),
        "regenerated VAA differs from public manifest",
    )?;
    require(
        public
            .get("post_update_data_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&observation.post_data)),
        "regenerated PostUpdate bytes differ from public manifest",
    )?;
    require(
        public
            .get("merkle_price_update_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&borsh::to_vec(&observation.update)?)),
        "regenerated MerklePriceUpdate differs from public manifest",
    )?;
    let expected_window_observations = observations
        .iter()
        .enumerate()
        .map(|(index, observation)| json!({
            "bucket": start_bucket + u64::try_from(index).expect("observation index fits u64"),
            "publish_time": publish_times[index],
            "vaa_sha256": provider::sha256(&observation.vaa),
            "post_update_data_sha256": provider::sha256(&observation.post_data),
            "merkle_price_update_sha256": provider::sha256(&borsh::to_vec(&observation.update).expect("observation encodes")),
        }))
        .collect::<Vec<_>>();
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        require(
            public.get("window_observations") == Some(&json!(expected_window_observations)),
            "regenerated boundary observations differ from public manifest",
        )?;
    }
    let wrong_feed_manifest = public
        .get("wrong_feed")
        .ok_or("campaign manifest has no wrong_feed observation")?;
    let wrong_feed_id_hex = WRONG_FEED_ID
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    require(
        wrong_feed_manifest
            .get("feed_id_hex")
            .and_then(Value::as_str)
            == Some(wrong_feed_id_hex.as_str()),
        "wrong-feed identity differs from public manifest",
    )?;
    require(
        wrong_feed_manifest
            .get("vaa_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&wrong_feed_observation.vaa)),
        "regenerated wrong-feed VAA differs from public manifest",
    )?;
    require(
        wrong_feed_manifest
            .get("post_update_data_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&wrong_feed_observation.post_data)),
        "regenerated wrong-feed PostUpdate bytes differ from public manifest",
    )?;
    require(
        wrong_feed_manifest
            .get("merkle_price_update_sha256")
            .and_then(Value::as_str)
            == Some(&provider::sha256(&borsh::to_vec(
                &wrong_feed_observation.update,
            )?)),
        "regenerated wrong-feed MerklePriceUpdate differs from public manifest",
    )?;

    println!("{CLAIM}");
    println!("genesis {}", rpc.genesis_hash()?);
    require(
        rpc.slot()? >= WARP_SLOT,
        "RPC slot is below the captured receiver deployment",
    )?;
    verify_provider_accounts(&rpc)?;
    verify_genesis_manifest(&rpc, &public, &correct, &clutch_elf, second_owner.pubkey())?;
    assert_clock(&rpc, publish_time)?;
    let mut steps = Vec::new();
    let mut lifecycle_signatures = BTreeMap::new();

    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[compute_budget(), router_initialize(payer.pubkey())?],
    )?;
    require_accepted("router initialize", &status)?;
    record_step(&rpc, &mut steps, "router-initialize", signature, &status)?;
    let mut encoded_observations = Vec::with_capacity(observations.len());
    for (index, observation) in observations.iter().enumerate() {
        let (role, allocate_step, verify_step) = if observations.len() == 1 {
            (
                "correct-feed".to_string(),
                "router-init-and-write-encoded-vaa".to_string(),
                "router-write-and-verify-encoded-vaa".to_string(),
            )
        } else {
            (
                format!("correct-feed-boundary-{index}"),
                format!("boundary-{index}-router-init-and-write-encoded-vaa"),
                format!("boundary-{index}-router-write-and-verify-encoded-vaa"),
            )
        };
        encoded_observations.push(upload_and_verify_vaa(
            &rpc,
            &payer,
            &mut steps,
            observation,
            &role,
            &allocate_step,
            &verify_step,
        )?);
    }

    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[compute_budget(), receiver_initialize(payer.pubkey())?],
    )?;
    require_accepted("receiver initialize", &status)?;
    record_step(&rpc, &mut steps, "receiver-initialize", signature, &status)?;
    require(
        account(
            &rpc,
            "receiver Config",
            address(real_pyth_lab::RECEIVER_CONFIG),
        )?
        .data
            == provider::fixture("receiver-config.account")?,
        "receiver did not write the pinned Config body",
    )?;

    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[compute_budget(), plane::init_spec(payer.pubkey(), &correct)],
    )?;
    require_accepted("correct InitSourceSpecV2", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "correct-init-source-spec-v2",
        signature,
        &status,
    )?;
    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[
            compute_budget(),
            plane::init_archive(payer.pubkey(), &correct),
        ],
    )?;
    require_accepted("correct InitSourceArchiveV2", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "correct-init-source-archive-v2",
        signature,
        &status,
    )?;

    let wrong_feed_encoded = upload_and_verify_vaa(
        &rpc,
        &payer,
        &mut steps,
        &wrong_feed_observation,
        "wrong-feed",
        "wrong-feed-router-init-and-write-encoded-vaa",
        "wrong-feed-router-write-and-verify-encoded-vaa",
    )?;

    let run_joined_trade = |steps: &mut Vec<Value>,
                            lifecycle_signatures: &mut BTreeMap<&'static str, String>|
     -> Result<Value> {
        if !is_joined_mode(campaign_mode) {
            return Ok(Value::Null);
        }
        let seller_position_rent = rpc.minimum_rent(account_len::POSITION)?;
        let seller_replay_rent = rpc.minimum_rent(REPLAY_ACCOUNT_LEN)?;
        let seller_reservation_rent = rpc.minimum_rent(RESERVATION_ACCOUNT_BYTES)?;
        let seller_account_creation_lamports = seller_position_rent
            .checked_add(seller_replay_rent)
            .and_then(|amount| amount.checked_add(seller_reservation_rent))
            .ok_or("second-owner account-creation funding overflow")?;
        let seller_funding_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-fund-second-owner-account-creation",
            &[system_instruction::transfer(
                &payer.pubkey(),
                &second_owner.pubkey(),
                seller_account_creation_lamports,
            )],
        )?;
        let seller_wallet = account(&rpc, "funded second-owner wallet", second_owner.pubkey())?;
        require(
            seller_wallet.owner == SYSTEM_PROGRAM.to_string()
                && seller_wallet.data.is_empty()
                && seller_wallet.lamports == seller_account_creation_lamports,
            "second owner did not receive the exact rent budget for Position, Replay, and Reservation",
        )?;
        let grid_upload = plane::price_grid_upload(payer.pubkey(), &correct);
        let grid_signatures = upload_artifact(&rpc, &payer, steps, "price-grid", &grid_upload)?;
        let grid = PriceGridAccount::decode(
            &account(&rpc, "sealed PriceGrid", correct.grid.address)?.data,
        )
        .map_err(|error| format!("sealed PriceGrid decode: {error:?}"))?;
        require(
            grid == correct.grid_value
                && grid
                    .binds_terms(&{
                        let bytes =
                            account(&rpc, "immutable Terms", correct.plane.terms.address)?.data;
                        clutch_solana_layout::TermsAccount::decode(&bytes)
                            .map_err(|error| format!("Terms decode: {error:?}"))?
                    })
                    .is_ok(),
            "sealed PriceGrid does not exactly bind the immutable real-Pyth Terms",
        )?;

        let mut absent = correct.plane.market_state_addresses().to_vec();
        absent.push(correct.plane.hoard_token.address);
        absent.extend(correct.plane.outcome_mints.iter().map(|mint| mint.address));
        for key in &absent {
            require(
                rpc.account(&key.to_string())?.is_none(),
                format!("joined market target {key} was genesis-assisted"),
            )?;
        }

        let signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-create-market",
            &[
                compute_budget(),
                plane::create_market(payer.pubkey(), &correct),
            ],
        )?;
        lifecycle_signatures.insert("create_market", signature);
        let market = plane::decode_market(
            &account(&rpc, "created market", correct.plane.market.address)?.data,
        )?;
        require(
            market.lifecycle == 0
                && market.terms == correct.plane.terms_id
                && market.feed == correct.plane.feed_id
                && market.outcome_count == 4,
            "created market identity differs from the real-Pyth-bound Terms",
        )?;
        require(
            token_amount(
                &rpc,
                "created Hoard token account",
                correct.plane.hoard_token.address,
            )? == 0,
            "created Hoard token account is not empty",
        )?;
        for (index, mint) in correct.plane.outcome_mints.iter().enumerate() {
            require(
                mint_supply(&rpc, &format!("created outcome mint {index}"), mint.address)? == 0,
                format!("created outcome mint {index} has nonzero supply"),
            )?;
        }

        let signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-endow-buyer-collateral",
            &[
                compute_budget(),
                plane::endow(payer.pubkey(), &correct, 0, plane::USER_COLLATERAL_ATOMS),
            ],
        )?;
        lifecycle_signatures.insert("buyer_endow", signature);
        let signature = accepted_step(
            &rpc,
            &payer,
            &[&second_owner],
            steps,
            "joined-endow-seller-collateral",
            &[
                compute_budget(),
                plane::endow(
                    second_owner.pubkey(),
                    &correct,
                    0,
                    plane::USER_COLLATERAL_ATOMS,
                ),
            ],
        )?;
        lifecycle_signatures.insert("seller_endow", signature);
        require(
            token_amount(
                &rpc,
                "user collateral token",
                plane::actor_collateral(payer.pubkey()),
            )? == 0
                && token_amount(
                    &rpc,
                    "Hoard token account",
                    correct.plane.hoard_token.address,
                )? == plane::JOINED_COLLATERAL_SUPPLY
                && token_amount(
                    &rpc,
                    "second owner collateral token",
                    plane::actor_collateral(second_owner.pubkey()),
                )? == 0,
            "the two Endow transitions did not move exact collateral into pooled custody",
        )?;
        let buyer_position = plane::decode_position(
            &account(&rpc, "endowed position", correct.plane.position.address)?.data,
        )?;
        let seller_state = plane::owner_state(second_owner.pubkey(), &correct);
        let seller_position = plane::decode_position(
            &account(&rpc, "seller position", seller_state.position.address)?.data,
        )?;
        require(
            buyer_position.cash_atoms == plane::USER_COLLATERAL_ATOMS
                && seller_position.cash_atoms == plane::USER_COLLATERAL_ATOMS,
            "Endow did not credit exact internal cash to both owners",
        )?;

        let signature = accepted_step(
            &rpc,
            &payer,
            &[&second_owner],
            steps,
            "joined-seller-split-complete-sets",
            &[
                compute_budget(),
                plane::split(
                    second_owner.pubkey(),
                    &correct,
                    1,
                    plane::USER_COLLATERAL_ATOMS,
                ),
            ],
        )?;
        lifecycle_signatures.insert("split", signature);
        let position = plane::decode_position(
            &account(&rpc, "split seller position", seller_state.position.address)?.data,
        )?;
        let supply = plane::decode_supply(
            &account(&rpc, "split supply", correct.plane.supply.address)?.data,
        )?;
        let hoard =
            plane::decode_hoard(&account(&rpc, "split Hoard", correct.plane.hoard.address)?.data)?;
        require(
            position.cash_atoms == 0
                && position.internal[..4]
                    .iter()
                    .all(|quantity| *quantity == plane::USER_COLLATERAL_ATOMS)
                && supply.internal_supply[..4]
                    .iter()
                    .all(|quantity| *quantity == plane::USER_COLLATERAL_ATOMS)
                && hoard.collateral_atoms == plane::USER_COLLATERAL_ATOMS,
            "Split did not create the exact backed four-outcome complete sets",
        )?;

        let general = plane::general_plane(payer.pubkey(), &correct);
        let policy_signatures =
            upload_artifact(&rpc, &payer, steps, "general-policy", &general.policy)?;
        let freeze_deadline_slot = rpc
            .slot()?
            .checked_add(32)
            .ok_or("general freeze deadline overflow")?;
        let init_epoch_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-init-epoch",
            &[
                compute_budget(),
                plane::init_epoch(payer.pubkey(), &correct, &general, freeze_deadline_slot),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-init-order-page",
            &[
                compute_budget(),
                plane::init_order_page(payer.pubkey(), &correct, &general),
            ],
        )?;
        let buyer_order = plane::place_single_order(payer.pubkey(), &correct, &general, 0, 0);
        let buyer_order_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-place-funded-buy",
            &[compute_budget(), buyer_order.instruction.clone()],
        )?;
        let seller_order =
            plane::place_single_order(second_owner.pubkey(), &correct, &general, 1, 1);
        let seller_order_signature = accepted_step(
            &rpc,
            &payer,
            &[&second_owner],
            steps,
            "joined-general-place-funded-sell",
            &[compute_budget(), seller_order.instruction.clone()],
        )?;
        require(
            rpc.account(&buyer_order.reservation.address.to_string())?
                .is_some()
                && rpc
                    .account(&seller_order.reservation.address.to_string())?
                    .is_some(),
            "funded placements did not create both reservation accounts",
        )?;

        wait_for_slot(&rpc, freeze_deadline_slot, "general epoch freeze")?;
        let freeze_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-freeze-epoch",
            &[compute_budget(), plane::freeze_epoch(&correct, &general)],
        )?;
        let epoch = EpochAccount::decode(
            &account(&rpc, "frozen general epoch", general.epoch.address)?.data,
        )
        .map_err(|error| format!("frozen general epoch decode: {error:?}"))?;
        let window = EpochWindowAccount::decode(
            &account(&rpc, "general epoch window", general.window.address)?.data,
        )
        .map_err(|error| format!("general epoch window decode: {error:?}"))?;
        require(
            epoch.phase == EPOCH_PHASE_FROZEN
                && epoch.owner_count == 2
                && epoch.order_count == 2
                && epoch.page_count == 1
                && window.live_order_count == 2
                && window.selection_deadline_slot >= freeze_deadline_slot + CANDIDATE_WINDOW_SLOTS,
            "frozen general epoch did not stamp the exact two-owner book and candidate window",
        )?;
        let page = account(&rpc, "frozen general order page", general.page.address)?;
        let candidate = plane::candidate_plan(&correct, &general, &epoch, &page.data)?;

        let submit_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-submit-candidate",
            &[
                compute_budget(),
                plane::submit_candidate(payer.pubkey(), &correct, &general, &candidate),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-write-candidate-fills",
            &[
                compute_budget(),
                plane::write_candidate_fills(&correct, &general, &candidate),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-write-candidate-slices",
            &[
                compute_budget(),
                plane::write_candidate_slices(&correct, &general, &candidate),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-seal-candidate",
            &[
                compute_budget(),
                plane::seal_candidate(&correct, &general, &candidate),
            ],
        )?;
        let mut create_work = vec![
            compute_budget(),
            plane::init_clear_work(payer.pubkey(), &correct, &general, &candidate),
        ];
        for sequence in 1..=4 {
            create_work.push(plane::grow_clear_work(
                &correct, &general, &candidate, sequence,
            ));
        }
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-create-clear-work",
            &create_work,
        )?;
        let reservations = [buyer_order.reservation, seller_order.reservation];
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-verify-pass-one",
            &[
                heap_frame(),
                compute_budget(),
                plane::advance_clear_work(&correct, &general, &candidate, &reservations),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-verify-slices",
            &[
                heap_frame(),
                compute_budget(),
                plane::advance_clear_slices(&correct, &general, &candidate),
            ],
        )?;
        accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-verify-pass-two",
            &[
                heap_frame(),
                compute_budget(),
                plane::advance_clear_work(&correct, &general, &candidate, &[]),
            ],
        )?;
        let complete_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-complete-clear-work",
            &[
                heap_frame(),
                compute_budget(),
                plane::complete_clear_work(&correct, &general, &candidate),
            ],
        )?;
        let verified = CandidateRecord::decode(
            &account(&rpc, "verified general candidate", candidate.record.address)?.data,
        )
        .map_err(|error| format!("verified candidate decode: {error:?}"))?;
        require(
            verified.status == CANDIDATE_STATUS_VERIFIED
                && verified.score_digest != Hash32::ZERO
                && verified.distinct_owners == 2,
            "streaming verifier did not persist a verified two-owner score",
        )?;

        wait_for_slot(
            &rpc,
            window.selection_deadline_slot,
            "best-valid-submitted-candidate selection",
        )?;
        let selection_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-finalize-selection",
            &[
                compute_budget(),
                plane::finalize_selection(&correct, &general, &candidate),
            ],
        )?;
        let selected_epoch = EpochAccount::decode(
            &account(&rpc, "cleared general epoch", general.epoch.address)?.data,
        )
        .map_err(|error| format!("selected epoch decode: {error:?}"))?;
        let selected = CandidateRecord::decode(
            &account(&rpc, "selected candidate", candidate.record.address)?.data,
        )
        .map_err(|error| format!("selected candidate decode: {error:?}"))?;
        require(
            selected_epoch.phase == EPOCH_PHASE_CLEARED
                && selected.status == CANDIDATE_STATUS_SELECTED,
            "best valid submitted candidate was not selected into a cleared epoch",
        )?;
        let freeze_entitlement_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-freeze-entitlement",
            &[
                heap_frame(),
                compute_budget(),
                plane::freeze_entitlement(payer.pubkey(), &correct, &general, &candidate),
            ],
        )?;
        let entitle_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-entitle-direct-slice",
            &[
                compute_budget(),
                plane::entitle_slice(
                    payer.pubkey(),
                    &correct,
                    &general,
                    &candidate,
                    buyer_order.reservation,
                    seller_order.reservation,
                ),
            ],
        )?;
        let settlement_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            steps,
            "joined-general-settle-direct-slice",
            &[
                compute_budget(),
                plane::settle_slice(
                    payer.pubkey(),
                    second_owner.pubkey(),
                    &correct,
                    &general,
                    &candidate,
                    buyer_order.reservation,
                    seller_order.reservation,
                ),
            ],
        )?;
        let buyer_position = plane::decode_position(
            &account(
                &rpc,
                "settled buyer position",
                correct.plane.position.address,
            )?
            .data,
        )?;
        let seller_position = plane::decode_position(
            &account(
                &rpc,
                "settled seller position",
                seller_state.position.address,
            )?
            .data,
        )?;
        require(
            buyer_position.cash_atoms == 60
                && buyer_position.reserved_cash_atoms == 0
                && buyer_position.internal[..4] == [0, 16, 0, 0]
                && seller_position.cash_atoms == 4
                && seller_position.reserved_cash_atoms == 0
                && seller_position.internal[..4] == [64, 48, 64, 64],
            "direct settlement did not land the exact two-owner cash/Egg conservation state",
        )?;
        Ok(json!({
            "status": "settled",
            "grid_genesis_assisted": false,
            "epoch_genesis_assisted": false,
            "order_genesis_assisted": false,
            "candidate_genesis_assisted": false,
            "price_grid": correct.grid.address.to_string(),
            "price_grid_digest": correct.grid_value.grid.bytes(),
            "grid_upload_signatures": grid_signatures,
            "policy_upload_signatures": policy_signatures,
            "second_owner_account_creation_funding": {
                "lamports": seller_account_creation_lamports.to_string(),
                "signature": seller_funding_signature,
                "genesis_assisted": false,
            },
            "epoch": general.epoch.address.to_string(),
            "epoch_id": general.epoch_id.bytes(),
            "init_epoch_signature": init_epoch_signature,
            "freeze_epoch_signature": freeze_signature,
            "owners": [payer.pubkey().to_string(), second_owner.pubkey().to_string()],
            "orders": {
                "buyer": {"outcome": 1, "side": "buy", "quantity": "16", "limit": "7500"},
                "seller": {"outcome": 1, "side": "sell", "quantity": "16", "limit": "2500"},
                "buyer_signature": buyer_order_signature,
                "seller_signature": seller_order_signature,
            },
            "candidate": candidate.candidate.bytes(),
            "prices": candidate.prices,
            "fills": [plane::ORDER_QUANTITY, plane::ORDER_QUANTITY],
            "witness_slices": candidate.slice_count,
            "submit_signature": submit_signature,
            "complete_verification_signature": complete_signature,
            "selection_signature": selection_signature,
            "freeze_entitlement_signature": freeze_entitlement_signature,
            "entitle_signature": entitle_signature,
            "settlement_signature": settlement_signature,
            "post_settlement": {
                "buyer_cash": "60",
                "buyer_internal": ["0", "16", "0", "0"],
                "seller_cash": "4",
                "seller_internal": ["64", "48", "64", "64"],
                "locked_collateral": "64",
                "pooled_custody": "128",
            },
        }))
    };

    let receiver = address(real_pyth_lab::RECEIVER_PROGRAM);
    let treasury = Address::find_program_address(&[b"treasury", &[0]], &receiver).0;
    let bad_config_update = Keypair::new();
    let watched = [correct.plane.source_archive.address, treasury];
    let before = snapshot(&rpc, &watched)?;
    assert_clock(&rpc, publish_times[0])?;
    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[&bad_config_update],
        &[
            compute_budget(),
            receiver_post(
                payer.pubkey(),
                encoded_observations[0].pubkey(),
                bad_config_update.pubkey(),
                &observations[0].post_data,
            ),
            plane::append(&correct, 0, bad_config_update.pubkey(), WRONG_CONFIG),
        ],
    )?;
    require_source_admission_refused("wrong Config joined transaction", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "wrong-config-post-update-plus-append-rollback",
        signature,
        &status,
    )?;
    require(
        rpc.account(&bad_config_update.pubkey().to_string())?
            .is_none(),
        "wrong Config did not roll back receiver update",
    )?;
    require(
        snapshot(&rpc, &watched)? == before,
        "wrong Config changed archive or treasury",
    )?;

    let bad_feed_update = Keypair::new();
    let before = snapshot(&rpc, &watched)?;
    assert_clock(&rpc, publish_time)?;
    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[&bad_feed_update],
        &[
            compute_budget(),
            receiver_post(
                payer.pubkey(),
                wrong_feed_encoded.pubkey(),
                bad_feed_update.pubkey(),
                &wrong_feed_observation.post_data,
            ),
            plane::append(
                &correct,
                0,
                bad_feed_update.pubkey(),
                address(real_pyth_lab::RECEIVER_CONFIG),
            ),
        ],
    )?;
    require_source_admission_refused("wrong feed joined transaction", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "wrong-feed-post-update-plus-append-rollback",
        signature,
        &status,
    )?;
    require(
        rpc.account(&bad_feed_update.pubkey().to_string())?
            .is_none(),
        "wrong feed did not roll back receiver update",
    )?;
    require(
        snapshot(&rpc, &watched)? == before,
        "wrong feed changed registered archive or treasury",
    )?;

    let out_of_order_rollback = if observations.len() > 1 {
        let index = observations.len() - 1;
        let skipped_update = Keypair::new();
        let before = snapshot(&rpc, &watched)?;
        let before_snapshot_sha256 = rollback_snapshot_sha256(&watched, &before)?;
        assert_clock(&rpc, publish_times[index])?;
        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[&skipped_update],
            &[
                compute_budget(),
                receiver_post(
                    payer.pubkey(),
                    encoded_observations[index].pubkey(),
                    skipped_update.pubkey(),
                    &observations[index].post_data,
                ),
                plane::append(
                    &correct,
                    0,
                    skipped_update.pubkey(),
                    address(real_pyth_lab::RECEIVER_CONFIG),
                ),
            ],
        )?;
        require_source_admission_refused("out-of-order boundary joined transaction", &status)?;
        record_step(
            &rpc,
            &mut steps,
            "out-of-order-boundary-post-update-plus-append-rollback",
            signature.clone(),
            &status,
        )?;
        let skipped_update_absent = rpc.account(&skipped_update.pubkey().to_string())?.is_none();
        require(
            skipped_update_absent,
            "out-of-order boundary did not roll back receiver update",
        )?;
        let after = snapshot(&rpc, &watched)?;
        require(
            after == before,
            "out-of-order boundary changed archive or treasury",
        )?;
        let after_snapshot_sha256 = rollback_snapshot_sha256(&watched, &after)?;
        require(
            after_snapshot_sha256 == before_snapshot_sha256,
            "equal rollback account snapshots produced different canonical digests",
        )?;
        Some(out_of_order_rollback_evidence(
            index,
            skipped_update.pubkey(),
            &signature,
            &before_snapshot_sha256,
            &after_snapshot_sha256,
            correct.plane.source_archive.address,
            treasury,
        ))
    } else {
        None
    };

    let mut update_accounts = Vec::with_capacity(observations.len());
    let mut append_signatures = Vec::with_capacity(observations.len());
    let mut parsed_updates = Vec::with_capacity(observations.len());
    let mut append_clocks = Vec::with_capacity(observations.len());
    for (index, observation) in observations.iter().enumerate() {
        assert_clock(&rpc, publish_times[index])?;
        let update = Keypair::new();
        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[&update],
            &[
                compute_budget(),
                receiver_post(
                    payer.pubkey(),
                    encoded_observations[index].pubkey(),
                    update.pubkey(),
                    &observation.post_data,
                ),
                plane::append(
                    &correct,
                    u64::try_from(index)?,
                    update.pubkey(),
                    address(real_pyth_lab::RECEIVER_CONFIG),
                ),
            ],
        )?;
        require_accepted(
            &format!("boundary {index} atomic adjacent PostUpdate + AppendSourceArchiveV2"),
            &status,
        )?;
        let step_label = if observations.len() == 1 {
            "real-post-update-plus-clutch-append-atomic".to_string()
        } else {
            format!("boundary-{index}-real-post-update-plus-clutch-append-atomic")
        };
        record_step(&rpc, &mut steps, &step_label, signature.clone(), &status)?;
        let update_view = account(
            &rpc,
            &format!("boundary {index} receiver price update"),
            update.pubkey(),
        )?;
        let parsed = parse_full_price_update_v2(
            PriceUpdateAccountViewV1::new(
                update.pubkey().to_bytes(),
                Address::from_str(&update_view.owner)?.to_bytes(),
                update_view.executable,
                &update_view.data,
            ),
            real_pyth_lab::RECEIVER_PROGRAM,
            provider::FEED_ID,
        )
        .map_err(|error| format!("boundary {index} receiver update parse: {error:?}"))?;
        require(parsed.price == provider::PRICE, "receiver price differs")?;
        require(
            parsed.confidence == provider::CONFIDENCE,
            "receiver confidence differs",
        )?;
        require(
            parsed.exponent == provider::EXPONENT,
            "receiver exponent differs",
        )?;
        require(
            parsed.publish_time == publish_times[index],
            format!("boundary {index} receiver publish time differs"),
        )?;
        append_clocks.push(assert_clock(&rpc, publish_times[index])?);
        update_accounts.push(update.pubkey());
        append_signatures.push(signature);
        parsed_updates.push(parsed);
    }

    let source_spec_account = account(
        &rpc,
        "correct SourceSpec v2",
        correct.plane.source_spec.address,
    )?;
    let verified_spec = source_archive_v2::verify_source_spec_v2_account(
        PROGRAM_ID.to_bytes(),
        correct.plane.source_spec.address.to_bytes(),
        ArchiveAccountViewV2::new(
            correct.plane.source_spec.address.to_bytes(),
            Address::from_str(&source_spec_account.owner)?.to_bytes(),
            source_spec_account.executable,
            &source_spec_account.data,
        ),
    )
    .map_err(|error| format!("canonical SourceSpec v2 verification failed: {error:?}"))?;
    let archive = account(
        &rpc,
        "open correct source archive",
        correct.plane.source_archive.address,
    )?;
    let expected_record_count = u8::try_from(observations.len())?;
    require(
        archive.owner == PROGRAM_ID.to_string(),
        "open source archive is not Clutch-owned",
    )?;
    require(!archive.executable, "open source archive is executable")?;
    require(
        archive.data.len() == source_archive_v2::SOURCE_ARCHIVE_ACCOUNT_V2_BYTES,
        "open source archive does not have the exact SourceArchiveV2 account length",
    )?;
    let open_record_count = source_archive_v2::open_archive_v2_sequence(
        &archive.data,
        verified_spec,
        real_pyth_lab::RELEASE,
        correct.window,
    )
    .map_err(|error| format!("canonical open SourceArchiveV2 verification failed: {error:?}"))?;
    require(
        open_record_count == u64::from(expected_record_count),
        format!("archive does not contain exactly {expected_record_count} records"),
    )?;
    if observations.len() > 1 {
        require(
            publish_times
                .windows(2)
                .all(|times| times[1] == times[0] + 60),
            "multi-boundary publish times are not exactly consecutive",
        )?;
    }
    let append_clock = append_clocks
        .last()
        .ok_or("campaign retained no append-time Clock")?;

    let (seal_signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[compute_budget(), plane::seal(&correct)],
    )?;
    require_accepted("SealSourceArchiveV2", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "seal-source-archive-v2",
        seal_signature.clone(),
        &status,
    )?;
    let sealed_archive = account(
        &rpc,
        "sealed correct source archive",
        correct.plane.source_archive.address,
    )?;
    let sealed_view = source_archive_v2::verify_recorded_sealed_archive_v2_view(
        PROGRAM_ID.to_bytes(),
        correct.plane.source_archive.address.to_bytes(),
        ArchiveAccountViewV2::new(
            correct.plane.source_archive.address.to_bytes(),
            Address::from_str(&sealed_archive.owner)?.to_bytes(),
            sealed_archive.executable,
            &sealed_archive.data,
        ),
        verified_spec,
        real_pyth_lab::RELEASE,
        correct.window,
    )
    .map_err(|error| format!("canonical sealed SourceArchiveV2 verification failed: {error:?}"))?;
    let receipt = sealed_view.receipt();
    require(
        receipt.record_count() == expected_record_count
            && receipt.start_bucket() == start_bucket
            && receipt.end_bucket_exclusive() == end_bucket
            && receipt.sealed_feed_cursor() == end_bucket,
        "canonical sealed archive receipt differs from the prepared complete window",
    )?;
    let lower = 99_980_929_u128;
    let upper = 100_019_071_u128;
    let mut archive_records = Vec::with_capacity(observations.len());
    for (index, parsed) in parsed_updates.iter().enumerate() {
        let record = sealed_view
            .archived_record(index)
            .map_err(|error| format!("canonical archive record {index} read failed: {error:?}"))?;
        let expected_bucket = start_bucket + u64::try_from(index)?;
        let expected_publish_time = u64::try_from(publish_times[index])?;
        require(
            record.bucket == expected_bucket,
            format!("archive record {index} bucket differs"),
        )?;
        require(
            record.low == lower && record.high == upper,
            format!("archive record {index} interval differs"),
        )?;
        require(
            record.sequence == expected_publish_time
                && record.publish_time == expected_publish_time,
            format!("archive record {index} sequence/publish time differs"),
        )?;
        require(
            record.publish_slot == parsed.posted_slot,
            format!("archive record {index} receiver-write slot differs"),
        )?;
        require(
            99_000_000 < record.low && record.high < 101_000_000,
            format!("archive record {index} interval does not uniquely select cell 1"),
        )?;
        archive_records.push(archive_record_evidence(index, record));
    }
    let source_archive = source_archive_evidence(
        correct.plane.source_archive.address,
        &sealed_archive,
        receipt.page_commitment().bytes(),
        receipt.feed().bytes(),
        receipt.window().bytes(),
        receipt.record_count(),
    );
    let feed = plane::decode_feed(&account(&rpc, "feed", correct.plane.feed.address)?.data)?;
    require(
        feed.cursor == end_bucket && feed.archive_pages == 1,
        "sealed feed cursor/page count differs",
    )?;
    // Seal the real source evidence before the protocol's fixed 1,000-slot
    // candidate window. Resolution remains after trading, so the market stays
    // live while the exact source admission bounds remain unchanged.
    let joined_trade = run_joined_trade(&mut steps, &mut lifecycle_signatures)?;
    let (resolve_signature, status) = sign_submit(
        &rpc,
        &payer,
        &[],
        &[
            compute_budget(),
            plane::resolve(payer.pubkey(), &correct, 1),
        ],
    )?;
    require_accepted("categorical Resolve cell 1", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "categorical-resolve-cell-1",
        resolve_signature.clone(),
        &status,
    )?;
    let resolution = plane::decode_resolution(
        &account(&rpc, "resolution", correct.plane.resolution.address)?.data,
    )?;
    let market =
        plane::decode_market(&account(&rpc, "market", correct.plane.market.address)?.data)?;
    require(
        resolution.payout_index == 1,
        "resolved payout is not cell 1",
    )?;
    require(
        resolution.feed_cursor == end_bucket,
        "resolution cursor differs",
    )?;
    require(market.lifecycle == 1, "market is not resolved")?;

    let lifecycle = if is_joined_mode(campaign_mode) {
        let mut redeem_signatures = Vec::new();
        let buyer_redeem = accepted_step(
            &rpc,
            &payer,
            &[],
            &mut steps,
            "joined-buyer-redeem-winning-eggs",
            &[
                compute_budget(),
                plane::redeem_internal(payer.pubkey(), &correct, 1, 1, 16),
            ],
        )?;
        redeem_signatures.push(json!({
            "owner": payer.pubkey().to_string(),
            "outcome": 1,
            "quantity": "16",
            "payout_atoms": "16",
            "signature": buyer_redeem,
        }));
        for (index, (outcome, quantity)) in [(0_u8, 64_u64), (1, 48), (2, 64), (3, 64)]
            .into_iter()
            .enumerate()
        {
            let sequence = 2 + u64::try_from(index)?;
            let label = format!("joined-seller-redeem-outcome-{outcome}");
            let signature = accepted_step(
                &rpc,
                &payer,
                &[&second_owner],
                &mut steps,
                &label,
                &[
                    compute_budget(),
                    plane::redeem_internal(
                        second_owner.pubkey(),
                        &correct,
                        sequence,
                        outcome,
                        quantity,
                    ),
                ],
            )?;
            redeem_signatures.push(json!({
                "owner": second_owner.pubkey().to_string(),
                "outcome": outcome,
                "quantity": quantity.to_string(),
                "payout_atoms": if outcome == 1 {
                    quantity.to_string()
                } else {
                    "0".to_string()
                },
                "signature": signature,
            }));
        }
        let buyer_state = plane::owner_state(payer.pubkey(), &correct);
        let seller_state = plane::owner_state(second_owner.pubkey(), &correct);
        let buyer_position = plane::decode_position(
            &account(
                &rpc,
                "fully redeemed buyer position",
                buyer_state.position.address,
            )?
            .data,
        )?;
        let seller_position = plane::decode_position(
            &account(
                &rpc,
                "fully redeemed seller position",
                seller_state.position.address,
            )?
            .data,
        )?;
        let supply = plane::decode_supply(
            &account(&rpc, "fully redeemed supply", correct.plane.supply.address)?.data,
        )?;
        let hoard = plane::decode_hoard(
            &account(&rpc, "fully redeemed Hoard", correct.plane.hoard.address)?.data,
        )?;
        require(
            buyer_position.cash_atoms == 76
                && buyer_position.internal[..4]
                    .iter()
                    .all(|quantity| *quantity == 0)
                && seller_position.cash_atoms == 52
                && seller_position.internal[..4]
                    .iter()
                    .all(|quantity| *quantity == 0)
                && supply.internal_supply[..4]
                    .iter()
                    .all(|quantity| *quantity == 0)
                && hoard.collateral_atoms == 0,
            "redemption did not extinguish both owners' internal claims and credit exact cash",
        )?;
        let buyer_withdraw_signature = accepted_step(
            &rpc,
            &payer,
            &[],
            &mut steps,
            "joined-buyer-withdraw-redeemed-collateral",
            &[
                compute_budget(),
                plane::withdraw(payer.pubkey(), &correct, 2, 76),
            ],
        )?;
        let seller_withdraw_signature = accepted_step(
            &rpc,
            &payer,
            &[&second_owner],
            &mut steps,
            "joined-seller-withdraw-redeemed-collateral",
            &[
                compute_budget(),
                plane::withdraw(second_owner.pubkey(), &correct, 6, 52),
            ],
        )?;
        lifecycle_signatures.insert("buyer_withdraw", buyer_withdraw_signature.clone());
        lifecycle_signatures.insert("seller_withdraw", seller_withdraw_signature.clone());
        let terminal_buyer = plane::decode_position(
            &account(
                &rpc,
                "terminal buyer position",
                buyer_state.position.address,
            )?
            .data,
        )?;
        let terminal_seller = plane::decode_position(
            &account(
                &rpc,
                "terminal seller position",
                seller_state.position.address,
            )?
            .data,
        )?;
        require(
            terminal_buyer.cash_atoms == 0
                && terminal_seller.cash_atoms == 0
                && token_amount(
                    &rpc,
                    "terminal buyer collateral token",
                    plane::actor_collateral(payer.pubkey()),
                )? == 76
                && token_amount(
                    &rpc,
                    "terminal seller collateral token",
                    plane::actor_collateral(second_owner.pubkey()),
                )? == 52
                && token_amount(
                    &rpc,
                    "terminal Hoard token account",
                    correct.plane.hoard_token.address,
                )? == 0,
            "WithdrawCash did not return the exact conserved collateral to both ephemeral owners",
        )?;
        let create_market_signature = lifecycle_signatures
            .get("create_market")
            .ok_or("joined campaign lost its CreateMarket signature")?;
        let buyer_endow_signature = lifecycle_signatures
            .get("buyer_endow")
            .ok_or("joined campaign lost its buyer Endow signature")?;
        let seller_endow_signature = lifecycle_signatures
            .get("seller_endow")
            .ok_or("joined campaign lost its seller Endow signature")?;
        let split_signature = lifecycle_signatures
            .get("split")
            .ok_or("joined campaign lost its Split signature")?;
        json!({
            "market_genesis_assisted": false,
            "market": correct.plane.market.address.to_string(),
            "ephemeral_users": [payer.pubkey().to_string(), second_owner.pubkey().to_string()],
            "user_collateral_tokens": [
                plane::actor_collateral(payer.pubkey()).to_string(),
                plane::actor_collateral(second_owner.pubkey()).to_string()
            ],
            "collateral_atoms": plane::JOINED_COLLATERAL_SUPPLY.to_string(),
            "create_market_signature": create_market_signature,
            "buyer_endow_signature": buyer_endow_signature,
            "seller_endow_signature": seller_endow_signature,
            "split_signature": split_signature,
            "redeem_internal": redeem_signatures,
            "buyer_withdraw_signature": buyer_withdraw_signature,
            "seller_withdraw_signature": seller_withdraw_signature,
            "terminal": {
                "buyer_position_cash_atoms": "0",
                "buyer_position_internal": ["0", "0", "0", "0"],
                "seller_position_cash_atoms": "0",
                "seller_position_internal": ["0", "0", "0", "0"],
                "supply_internal": ["0", "0", "0", "0"],
                "hoard_collateral_atoms": "0",
                "hoard_token_atoms": "0",
                "buyer_token_atoms": "76",
                "seller_token_atoms": "52",
            },
            "trade": joined_trade,
        })
    } else {
        Value::Null
    };

    let expected_step_count = match campaign_mode {
        JOINED_LIFECYCLE_MODE => 52,
        MULTIBOUNDARY_JOINED_MODE => 56,
        _ => 13,
    };
    require(
        steps.len() == expected_step_count,
        format!(
            "campaign recorded {} steps, schema requires exactly {expected_step_count}",
            steps.len()
        ),
    )?;
    let final_clock = read_clock(&rpc)?;
    let source_freshness = source_freshness_evidence(append_clock, &final_clock, publish_time)?;
    require(
        append_clocks.windows(2).all(|clocks| {
            clocks[1].slot >= clocks[0].slot && clocks[1].unix_timestamp >= clocks[0].unix_timestamp
        }),
        "append-time Clocks are not monotone across the source window",
    )?;
    let observation_freshness = append_clocks
        .iter()
        .zip(publish_times.iter())
        .enumerate()
        .map(|(index, (clock, publish_time))| {
            Ok(json!({
                "bucket": start_bucket + u64::try_from(index)?,
                "publish_time": publish_time,
                "append_clock": clock,
                "append_age_seconds": require_append_freshness(clock, *publish_time)?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let joined_signature = append_signatures
        .last()
        .ok_or("campaign retained no append signature")?;
    let mut result = json!({
        "claim": CLAIM,
        "transcript_schema": transcript_schema(campaign_mode),
        "campaign_mode": campaign_mode,
        "network": "loopback validator only",
        "genesis_hash": rpc.genesis_hash()?,
        "clock": final_clock,
        "source_freshness": source_freshness,
        "publish_time": publish_time,
        "provider_feed_id": Hash32::from_bytes(provider::FEED_ID).bytes(),
        "price": provider::PRICE,
        "confidence": provider::CONFIDENCE,
        "exponent": provider::EXPONENT,
        "interval": {"lower": lower.to_string(), "upper": upper.to_string()},
        "verified_vaa_account": encoded_observations.last().ok_or("campaign retained no verified VAA")?.pubkey().to_string(),
        "wrong_feed_verified_vaa_account": wrong_feed_encoded.pubkey().to_string(),
        "update_account": update_accounts.last().ok_or("campaign retained no receiver update")?.to_string(),
        "joined_post_append_signature": joined_signature,
        "seal_signature": seal_signature,
        "resolve_signature": resolve_signature,
        "wrong_config_rollback": true,
        "wrong_feed_rollback": true,
        "sealed": true,
        "resolved_payout": 1,
        "lifecycle": lifecycle,
        "steps": steps,
    });
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        let fields = result
            .as_object_mut()
            .ok_or("campaign result is not a JSON object")?;
        fields.insert("boundary_count".to_string(), json!(observations.len()));
        fields.insert(
            "observation_freshness".to_string(),
            json!(observation_freshness),
        );
        fields.insert(
            "verified_vaa_accounts".to_string(),
            json!(encoded_observations
                .iter()
                .map(|account| account.pubkey().to_string())
                .collect::<Vec<_>>()),
        );
        fields.insert(
            "update_accounts".to_string(),
            json!(update_accounts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()),
        );
        fields.insert(
            "post_append_signatures".to_string(),
            json!(append_signatures),
        );
        fields.insert("archive_records".to_string(), json!(archive_records));
        fields.insert("source_archive".to_string(), source_archive);
        fields.insert(
            "out_of_order_boundary_rollback".to_string(),
            out_of_order_rollback
                .ok_or("multi-boundary campaign retained no out-of-order rollback evidence")?,
        );
    }
    fs::write(
        work.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    if campaign_mode == MULTIBOUNDARY_JOINED_MODE {
        println!("PASS: signed PriceGrid/policy artifacts -> CreateMarket -> two-owner funded general book -> freeze -> best valid submitted candidate verification/selection -> entitlement/settlement -> real router/receiver source window -> Resolve(1) -> two-owner redemption/withdrawal");
    } else if campaign_mode == JOINED_LIFECYCLE_MODE {
        println!("PASS: signed PriceGrid/policy artifacts -> CreateMarket -> two-owner funded general book -> freeze -> best valid submitted candidate verification/selection -> entitlement/settlement -> real router/receiver source -> Resolve(1) -> two-owner redemption/withdrawal");
    } else {
        println!("PASS: real router verify -> persisted VAA -> atomic PostUpdate+Append -> Seal -> Resolve(1)");
    }
    Ok(())
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            usage();
        }
    };
    let result = match args.command.as_str() {
        "prepare" => args
            .clutch_elf
            .as_deref()
            .ok_or_else(|| "prepare requires --clutch-elf".into())
            .and_then(|elf| {
                args.validator
                    .as_deref()
                    .ok_or_else(|| "prepare requires --validator".into())
                    .and_then(|validator| {
                        args.repository_head
                            .as_deref()
                            .ok_or_else(|| "prepare requires --repository-head".into())
                            .and_then(|repository_head| {
                                args.publish_time
                                    .ok_or_else(|| "prepare requires --publish-time".into())
                                    .and_then(|publish_time| {
                                        args.clock_probe_time
                                            .ok_or_else(|| {
                                                "prepare requires --clock-probe-time".into()
                                            })
                                            .and_then(|clock_probe_time| {
                                                prepare(
                                                    &args.work,
                                                    elf,
                                                    validator,
                                                    repository_head,
                                                    clock_probe_time,
                                                    publish_time,
                                                    &args.campaign_mode,
                                                )
                                            })
                                    })
                            })
                    })
            }),
        "run" => args
            .url
            .as_deref()
            .ok_or_else(|| "run requires --url".into())
            .and_then(|url| {
                args.validator
                    .as_deref()
                    .ok_or_else(|| "run requires --validator".into())
                    .and_then(|validator| {
                        args.repository_head
                            .as_deref()
                            .ok_or_else(|| "run requires --repository-head".into())
                            .and_then(|repository_head| {
                                run(
                                    &args.work,
                                    url,
                                    validator,
                                    repository_head,
                                    &args.campaign_mode,
                                )
                            })
                    })
            }),
        "clock" => args
            .url
            .as_deref()
            .ok_or_else(|| "clock requires --url".into())
            .and_then(print_clock),
        _ => unreachable!(),
    };
    if let Err(error) = result {
        eprintln!("FAIL: {error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod argument_tests {
    use super::*;

    #[test]
    fn joined_campaign_mode_is_explicitly_accepted() {
        let args = parse_args(
            [
                "prepare",
                "--work",
                "/tmp/unused-local-real-pyth-test",
                "--campaign-mode",
                JOINED_LIFECYCLE_MODE,
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert_eq!(args.campaign_mode, JOINED_LIFECYCLE_MODE);
    }

    #[test]
    fn multiboundary_campaign_freezes_two_consecutive_closing_boundaries() {
        let args = parse_args(
            [
                "prepare",
                "--work",
                "/tmp/unused-local-real-pyth-test",
                "--campaign-mode",
                MULTIBOUNDARY_JOINED_MODE,
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert_eq!(args.campaign_mode, MULTIBOUNDARY_JOINED_MODE);
        assert_eq!(
            transcript_schema(&args.campaign_mode),
            MULTIBOUNDARY_JOINED_TRANSCRIPT_SCHEMA
        );
        assert!(is_joined_mode(&args.campaign_mode));
        assert_eq!(boundary_count(&args.campaign_mode), 2);
        assert_eq!(publish_lag_seconds(&args.campaign_mode), 120);
        assert_eq!(
            window_publish_times(&args.campaign_mode, 1_800_000_000).unwrap(),
            [1_799_999_940, 1_800_000_000]
        );
    }

    #[test]
    fn unknown_campaign_mode_is_refused_before_any_io() {
        let error = parse_args(
            [
                "prepare",
                "--work",
                "/tmp/unused-local-real-pyth-test",
                "--campaign-mode",
                "mocked-trade",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .err()
        .unwrap();
        assert!(error.contains("unknown campaign mode mocked-trade"));
    }

    #[test]
    fn sealed_lifecycle_accepts_a_final_clock_beyond_live_update_freshness() {
        let publish_time = 1_787_540_520;
        let append_clock = Clock {
            slot: WARP_SLOT + 42,
            unix_timestamp: publish_time + 120,
            ..Clock::default()
        };
        let final_clock = Clock {
            slot: append_clock.slot + 1_000,
            unix_timestamp: publish_time + 561,
            ..Clock::default()
        };

        let evidence =
            source_freshness_evidence(&append_clock, &final_clock, publish_time).unwrap();
        assert_eq!(evidence["append_age_seconds"], 120);
        assert_eq!(evidence["final_age_seconds"], 561);

        let stale_append = Clock {
            unix_timestamp: publish_time + 301,
            ..append_clock
        };
        assert!(source_freshness_evidence(&stale_append, &final_clock, publish_time).is_err());
    }

    #[test]
    fn rollback_snapshot_digest_binds_every_watched_account_fact() {
        let keys = [address([1; 32]), address([2; 32])];
        let owner = address([3; 32]);
        let baseline = vec![
            Some(AccountView {
                lamports: 41,
                owner: owner.to_string(),
                executable: false,
                data: vec![5, 6, 7],
            }),
            Some(AccountView {
                lamports: 42,
                owner: owner.to_string(),
                executable: false,
                data: vec![8, 9],
            }),
        ];
        let digest = rollback_snapshot_sha256(&keys, &baseline).unwrap();
        assert_eq!(rollback_snapshot_sha256(&keys, &baseline).unwrap(), digest);

        let mut changed = baseline.clone();
        changed[0].as_mut().unwrap().lamports += 1;
        assert_ne!(rollback_snapshot_sha256(&keys, &changed).unwrap(), digest);
        let mut changed = baseline.clone();
        changed[0].as_mut().unwrap().owner = address([4; 32]).to_string();
        assert_ne!(rollback_snapshot_sha256(&keys, &changed).unwrap(), digest);
        let mut changed = baseline.clone();
        changed[0].as_mut().unwrap().executable = true;
        assert_ne!(rollback_snapshot_sha256(&keys, &changed).unwrap(), digest);
        let mut changed = baseline.clone();
        changed[0].as_mut().unwrap().data.push(10);
        assert_ne!(rollback_snapshot_sha256(&keys, &changed).unwrap(), digest);
        let mut changed = baseline.clone();
        changed[0] = None;
        assert_ne!(rollback_snapshot_sha256(&keys, &changed).unwrap(), digest);
        assert_ne!(
            rollback_snapshot_sha256(&[keys[1], keys[0]], &baseline).unwrap(),
            digest
        );
        assert!(rollback_snapshot_sha256(&keys[..1], &baseline).is_err());
    }

    #[test]
    fn multiboundary_public_schema_retains_exact_archive_and_rollback_facts() {
        let archive = address([11; 32]);
        let treasury = address([12; 32]);
        let skipped = address([13; 32]);
        let record = archive_record_evidence(
            1,
            ArchiveRecordV2 {
                bucket: 29_792_341,
                low: 99_980_929,
                high: 100_019_071,
                sequence: 1_787_540_460,
                publish_slot: 460_336_349,
                publish_time: 1_787_540_460,
            },
        );
        assert_eq!(
            record,
            json!({
                "index": 1,
                "bucket": "29792341",
                "lower": "99980929",
                "upper": "100019071",
                "sequence": "1787540460",
                "write_slot": "460336349",
                "publish_time": "1787540460",
            })
        );

        let account = AccountView {
            lamports: 99,
            owner: PROGRAM_ID.to_string(),
            executable: false,
            data: vec![7; source_archive_v2::SOURCE_ARCHIVE_ACCOUNT_V2_BYTES],
        };
        let archive_json =
            source_archive_evidence(archive, &account, [14; 32], [15; 32], [16; 32], 2);
        assert_eq!(archive_json["key"], archive.to_string());
        assert_eq!(archive_json["owner"], PROGRAM_ID.to_string());
        assert_eq!(archive_json["executable"], false);
        assert_eq!(archive_json["data_len"], 2_560);
        assert_eq!(archive_json["body_sha256"], provider::sha256(&account.data));
        assert_eq!(archive_json["record_count"], 2);
        for field in ["page_commitment", "feed_id", "window_id"] {
            assert_eq!(archive_json[field].as_str().unwrap().len(), 64);
        }

        let snapshot_digest = "ab".repeat(32);
        let rollback = out_of_order_rollback_evidence(
            1,
            skipped,
            "local-signature",
            &snapshot_digest,
            &snapshot_digest,
            archive,
            treasury,
        );
        assert_eq!(rollback["ok"], true);
        assert_eq!(rollback["skipped_update_account"], skipped.to_string());
        assert_eq!(rollback["skipped_update_absent_after_refusal"], true);
        assert_eq!(rollback["instruction_error"]["instruction_index"], 2);
        assert_eq!(rollback["instruction_error"]["custom_code"], 122);
        assert_eq!(rollback["before_snapshot_sha256"], snapshot_digest);
        assert_eq!(rollback["after_snapshot_sha256"], snapshot_digest);
        assert_eq!(
            rollback["before_snapshot_sha256"].as_str().unwrap().len(),
            64
        );
        assert_eq!(rollback["snapshots_equal"], true);
        assert_eq!(
            rollback["watched_accounts"][0]["address"],
            archive.to_string()
        );
        assert_eq!(
            rollback["watched_accounts"][1]["address"],
            treasury.to_string()
        );
    }
}
