//! NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY campaign.

mod plane;
mod provider;
mod rpc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use clutch_sbf::{
    loader_state::UPGRADEABLE_LOADER_ID,
    pyth_receiver::{parse_full_price_update_v2, PriceUpdateAccountViewV1},
    source_identity::{real_pyth_lab, CLOCK_SYSVAR_ID},
    source_v2::fixtures::{programdata_body, receiver_program_body},
};
use clutch_solana_layout::Hash32;
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
    if campaign_mode != SOURCE_ONLY_MODE && campaign_mode != JOINED_LIFECYCLE_MODE {
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
         clutch-local-real-pyth prepare --work DIR --repository-head COMMIT --clock-probe-time UNIX --publish-time UNIX --clutch-elf FILE --validator FILE [--campaign-mode source-only-v1|joined-user-lifecycle-v1]\n  \
         clutch-local-real-pyth run --work DIR --repository-head COMMIT --url http://127.0.0.1:PORT --validator FILE [--campaign-mode source-only-v1|joined-user-lifecycle-v1]"
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
    wrong_feed: &plane::LabPlane,
    clutch_elf: &[u8],
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
    add_plane_rows(&mut rows, "wrong-feed", wrong_feed)?;
    if correct.market_prestate == plane::MarketPrestate::SignedCreate {
        insert_row(
            &mut rows,
            GenesisRow {
                role: "joined-collateral-mint".to_string(),
                address: plane::COLLATERAL_MINT,
                owner: plane::token_program(),
                data: collateral_mint_bytes(plane::USER_COLLATERAL_ATOMS),
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
    write_key(&secrets_dir.join("payer.json"), &payer)?;
    fs::write(work.join("payer.pubkey"), format!("{}\n", payer.pubkey()))?;
    fs::write(work.join("program.pubkey"), format!("{PROGRAM_ID}\n"))?;

    require(publish_time > 0, "publish time must be positive")?;
    let expected_publish_time = clock_probe_time
        .checked_sub(180)
        .ok_or("clock-probe time underflow")?
        .div_euclid(60)
        * 60;
    require(
        publish_time == expected_publish_time,
        "publish time is not the named three-minute boundary behind the probe Clock",
    )?;
    let end_bucket = u64::try_from(publish_time)?.div_euclid(60);
    let start_bucket = end_bucket.checked_sub(1).ok_or("bucket underflow")?;
    let market_prestate = if campaign_mode == JOINED_LIFECYCLE_MODE {
        plane::MarketPrestate::SignedCreate
    } else {
        plane::MarketPrestate::GenesisFunded
    };
    let correct = plane::build(
        payer.pubkey(),
        plane::real_spec(provider::FEED_ID)?,
        start_bucket,
        end_bucket,
        clutch_svm_fixture::MARKET_NONCE,
        market_prestate,
    );
    let wrong_feed = plane::build(
        payer.pubkey(),
        plane::real_spec(WRONG_FEED_ID)?,
        start_bucket,
        end_bucket,
        plane::WRONG_MARKET_NONCE,
        market_prestate,
    );
    require(
        correct.start_bucket == start_bucket
            && correct.end_bucket_exclusive == end_bucket
            && wrong_feed.start_bucket == start_bucket
            && wrong_feed.end_bucket_exclusive == end_bucket,
        "rebuilt plane window differs from manifest",
    )?;
    let observation = provider::observation(publish_time)?;
    let merkle_update_hash = provider::sha256(&borsh::to_vec(&observation.update)?);

    let rows = expected_genesis_rows(&correct, &wrong_feed, &elf_bytes)?;

    let mut index = String::new();
    for (number, row) in rows.values().enumerate() {
        let filename = format!("accounts/{number:03}-{}.json", row.role);
        write_row(&work.join(&filename), row)?;
        index.push_str(&format!("{}\t{}\t{}\n", row.role, row.address, filename));
    }
    fs::write(work.join("genesis.tsv"), index)?;

    let manifest = json!({
        "claim": CLAIM,
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
        "publish_time_derivation": "floor((same-validator warped Clock - 180 seconds) / 60) * 60",
        "start_bucket": start_bucket,
        "end_bucket_exclusive": end_bucket,
        "warp_slot": WARP_SLOT,
        "payer": payer.pubkey().to_string(),
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
        },
        "wrong_feed": {
            "feed_id_hex": WRONG_FEED_ID.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "source_spec": wrong_feed.plane.source_spec.address.to_string(),
            "archive": wrong_feed.plane.source_archive.address.to_string(),
        }
    });
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
    wrong_feed: &plane::LabPlane,
    clutch_elf: &[u8],
) -> Result<()> {
    let rows = public
        .get("genesis_accounts")
        .and_then(Value::as_array)
        .ok_or("campaign manifest has no genesis_accounts array")?;
    let mut expected = expected_genesis_rows(correct, wrong_feed, clutch_elf)?;
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

fn assert_clock(rpc: &Rpc, publish_time: i64) -> Result<Clock> {
    let view = account(rpc, "Clock sysvar", address(CLOCK_SYSVAR_ID))?;
    let clock: Clock = bincode::deserialize(&view.data)?;
    require(
        clock.slot >= WARP_SLOT,
        format!("Clock slot {} is below {WARP_SLOT}", clock.slot),
    )?;
    require(
        clock.unix_timestamp >= publish_time + 60,
        format!(
            "Clock {} has not reached observation maturity {}",
            clock.unix_timestamp,
            publish_time + 60
        ),
    )?;
    require(
        clock.unix_timestamp - publish_time <= 300,
        format!(
            "laboratory observation is more than five minutes old: Clock {}, publish {publish_time}",
            clock.unix_timestamp
        ),
    )?;
    Ok(clock)
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
    let clutch_elf = fs::read(work.join("elf/clutch_sbf.so"))?;
    require(
        public.get("clutch_elf_sha256").and_then(Value::as_str)
            == Some(&provider::sha256(&clutch_elf)),
        "retained Clutch ELF differs from prepared manifest",
    )?;
    let payer = read_keypair_file(work.join("lab-secrets/payer.json"))
        .map_err(|error| format!("explicit ephemeral payer: {error}"))?;
    let market_prestate = if campaign_mode == JOINED_LIFECYCLE_MODE {
        plane::MarketPrestate::SignedCreate
    } else {
        plane::MarketPrestate::GenesisFunded
    };
    let correct = plane::build(
        payer.pubkey(),
        plane::real_spec(provider::FEED_ID)?,
        start_bucket,
        end_bucket,
        clutch_svm_fixture::MARKET_NONCE,
        market_prestate,
    );
    let wrong_feed = plane::build(
        payer.pubkey(),
        plane::real_spec(WRONG_FEED_ID)?,
        start_bucket,
        end_bucket,
        plane::WRONG_MARKET_NONCE,
        market_prestate,
    );
    let observation = provider::observation(publish_time)?;
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

    println!("{CLAIM}");
    println!("genesis {}", rpc.genesis_hash()?);
    require(
        rpc.slot()? >= WARP_SLOT,
        "RPC slot is below the captured receiver deployment",
    )?;
    verify_provider_accounts(&rpc)?;
    verify_genesis_manifest(&rpc, &public, &correct, &wrong_feed, &clutch_elf)?;
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
        &rpc,
        &payer,
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
    require_accepted("router encoded VAA allocation/write", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "router-init-and-write-encoded-vaa",
        signature,
        &status,
    )?;
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
    let (signature, status) = sign_submit(&rpc, &payer, &[], &verify)?;
    require_accepted("real router VerifyEncodedVaa", &status)?;
    record_step(
        &rpc,
        &mut steps,
        "router-write-and-verify-encoded-vaa",
        signature,
        &status,
    )?;
    let encoded_view = account(&rpc, "encoded VAA", encoded.pubkey())?;
    require(
        encoded_view.owner == router.to_string(),
        "Verified VAA account is not router-owned",
    )?;
    require(
        !encoded_view.executable && encoded_view.data.len() == encoded_len,
        "Verified VAA account shape differs",
    )?;
    require(
        encoded_view.data.get(8) == Some(&2),
        "router did not persist Verified state",
    )?;
    require(
        encoded_view.data.get(VAA_START..) == Some(observation.vaa.as_slice()),
        "Verified VAA account payload differs from locally signed bytes",
    )?;

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

    for (label, lab) in [("correct", &correct), ("wrong-feed", &wrong_feed)] {
        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[compute_budget(), plane::init_spec(payer.pubkey(), lab)],
        )?;
        require_accepted(&format!("{label} InitSourceSpecV2"), &status)?;
        record_step(
            &rpc,
            &mut steps,
            &format!("{label}-init-source-spec-v2"),
            signature,
            &status,
        )?;
        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[compute_budget(), plane::init_archive(payer.pubkey(), lab)],
        )?;
        require_accepted(&format!("{label} InitSourceArchiveV2"), &status)?;
        record_step(
            &rpc,
            &mut steps,
            &format!("{label}-init-source-archive-v2"),
            signature,
            &status,
        )?;
    }

    if campaign_mode == JOINED_LIFECYCLE_MODE {
        let mut absent = correct.plane.market_state_addresses().to_vec();
        absent.push(correct.plane.hoard_token.address);
        absent.extend(correct.plane.outcome_mints.iter().map(|mint| mint.address));
        for key in &absent {
            require(
                rpc.account(&key.to_string())?.is_none(),
                format!("joined market target {key} was genesis-assisted"),
            )?;
        }

        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[
                compute_budget(),
                plane::create_market(payer.pubkey(), &correct),
            ],
        )?;
        require_accepted("signed CreateMarket", &status)?;
        record_step(
            &rpc,
            &mut steps,
            "joined-create-market",
            signature.clone(),
            &status,
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

        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[
                compute_budget(),
                plane::endow(payer.pubkey(), &correct, 0, plane::USER_COLLATERAL_ATOMS),
            ],
        )?;
        require_accepted("signed Endow", &status)?;
        record_step(
            &rpc,
            &mut steps,
            "joined-endow-collateral",
            signature.clone(),
            &status,
        )?;
        lifecycle_signatures.insert("endow", signature);
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
                )? == plane::USER_COLLATERAL_ATOMS,
            "Endow did not move the exact collateral into pooled custody",
        )?;
        let position = plane::decode_position(
            &account(&rpc, "endowed position", correct.plane.position.address)?.data,
        )?;
        require(
            position.cash_atoms == plane::USER_COLLATERAL_ATOMS,
            "Endow did not credit exact internal cash",
        )?;

        let (signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[
                compute_budget(),
                plane::split(payer.pubkey(), &correct, 1, plane::USER_COLLATERAL_ATOMS),
            ],
        )?;
        require_accepted("signed Split", &status)?;
        record_step(
            &rpc,
            &mut steps,
            "joined-split-complete-sets",
            signature.clone(),
            &status,
        )?;
        lifecycle_signatures.insert("split", signature);
        let position = plane::decode_position(
            &account(&rpc, "split position", correct.plane.position.address)?.data,
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
    }

    let receiver = address(real_pyth_lab::RECEIVER_PROGRAM);
    let treasury = Address::find_program_address(&[b"treasury", &[0]], &receiver).0;
    let bad_config_update = Keypair::new();
    let watched = [correct.plane.source_archive.address, treasury];
    let before = snapshot(&rpc, &watched)?;
    assert_clock(&rpc, publish_time)?;
    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[&bad_config_update],
        &[
            compute_budget(),
            receiver_post(
                payer.pubkey(),
                encoded.pubkey(),
                bad_config_update.pubkey(),
                &observation.post_data,
            ),
            plane::append(&correct, bad_config_update.pubkey(), WRONG_CONFIG),
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
    let wrong_watched = [wrong_feed.plane.source_archive.address, treasury];
    let before = snapshot(&rpc, &wrong_watched)?;
    assert_clock(&rpc, publish_time)?;
    let (signature, status) = sign_submit(
        &rpc,
        &payer,
        &[&bad_feed_update],
        &[
            compute_budget(),
            receiver_post(
                payer.pubkey(),
                encoded.pubkey(),
                bad_feed_update.pubkey(),
                &observation.post_data,
            ),
            plane::append(
                &wrong_feed,
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
        snapshot(&rpc, &wrong_watched)? == before,
        "wrong feed changed archive or treasury",
    )?;

    assert_clock(&rpc, publish_time)?;
    let update = Keypair::new();
    let (joined_signature, status) = sign_submit(
        &rpc,
        &payer,
        &[&update],
        &[
            compute_budget(),
            receiver_post(
                payer.pubkey(),
                encoded.pubkey(),
                update.pubkey(),
                &observation.post_data,
            ),
            plane::append(
                &correct,
                update.pubkey(),
                address(real_pyth_lab::RECEIVER_CONFIG),
            ),
        ],
    )?;
    require_accepted(
        "atomic adjacent PostUpdate + AppendSourceArchiveV2",
        &status,
    )?;
    record_step(
        &rpc,
        &mut steps,
        "real-post-update-plus-clutch-append-atomic",
        joined_signature.clone(),
        &status,
    )?;
    let update_view = account(&rpc, "receiver price update", update.pubkey())?;
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
    .map_err(|error| format!("receiver update parse: {error:?}"))?;
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
        parsed.publish_time == publish_time,
        "receiver publish time differs",
    )?;

    let archive = account(
        &rpc,
        "correct source archive",
        correct.plane.source_archive.address,
    )?;
    require(
        archive.data.get(3) == Some(&1),
        "archive does not contain exactly one record",
    )?;
    require(
        archive.data.len() >= 576,
        "archive is too short for record zero",
    )?;
    let record = &archive.data[512..576];
    let u64_at =
        |offset: usize| u64::from_le_bytes(record[offset..offset + 8].try_into().expect("slice"));
    let u128_at =
        |offset: usize| u128::from_le_bytes(record[offset..offset + 16].try_into().expect("slice"));
    let lower = u128_at(8);
    let upper = u128_at(24);
    require(u64_at(0) == start_bucket, "archive bucket differs")?;
    require(
        lower == 99_980_929 && upper == 100_019_071,
        "archive interval differs",
    )?;
    require(
        u64_at(40) == u64::try_from(publish_time)?,
        "archive publish time differs",
    )?;
    require(
        u64_at(48) == parsed.posted_slot,
        "archive posted slot differs",
    )?;
    require(
        99_000_000 < lower && upper < 101_000_000,
        "interval does not uniquely select cell 1",
    )?;

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
    let feed = plane::decode_feed(&account(&rpc, "feed", correct.plane.feed.address)?.data)?;
    require(
        feed.cursor == end_bucket && feed.archive_pages == 1,
        "sealed feed cursor/page count differs",
    )?;
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

    let lifecycle = if campaign_mode == JOINED_LIFECYCLE_MODE {
        let mut redeem_signatures = Vec::new();
        for outcome in 0..4_u8 {
            let sequence = 2 + u64::from(outcome);
            let label = format!("joined-redeem-internal-outcome-{outcome}");
            let (signature, status) = sign_submit(
                &rpc,
                &payer,
                &[],
                &[
                    compute_budget(),
                    plane::redeem_internal(
                        payer.pubkey(),
                        &correct,
                        sequence,
                        outcome,
                        plane::USER_COLLATERAL_ATOMS,
                    ),
                ],
            )?;
            require_accepted(&format!("RedeemInternal outcome {outcome}"), &status)?;
            record_step(&rpc, &mut steps, &label, signature.clone(), &status)?;
            redeem_signatures.push(json!({
                "outcome": outcome,
                "quantity": plane::USER_COLLATERAL_ATOMS.to_string(),
                "payout_atoms": if outcome == 1 {
                    plane::USER_COLLATERAL_ATOMS.to_string()
                } else {
                    "0".to_string()
                },
                "signature": signature,
            }));
        }
        let position = plane::decode_position(
            &account(
                &rpc,
                "fully redeemed position",
                correct.plane.position.address,
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
            position.cash_atoms == plane::USER_COLLATERAL_ATOMS
                && position.internal[..4].iter().all(|quantity| *quantity == 0)
                && supply.internal_supply[..4]
                    .iter()
                    .all(|quantity| *quantity == 0)
                && hoard.collateral_atoms == 0,
            "RedeemInternal did not extinguish every internal claim and credit exact cash",
        )?;
        let (withdraw_signature, status) = sign_submit(
            &rpc,
            &payer,
            &[],
            &[
                compute_budget(),
                plane::withdraw(payer.pubkey(), &correct, 6, plane::USER_COLLATERAL_ATOMS),
            ],
        )?;
        require_accepted("WithdrawCash after redemption", &status)?;
        record_step(
            &rpc,
            &mut steps,
            "joined-withdraw-redeemed-collateral",
            withdraw_signature.clone(),
            &status,
        )?;
        lifecycle_signatures.insert("withdraw", withdraw_signature.clone());
        let terminal_position = plane::decode_position(
            &account(&rpc, "terminal position", correct.plane.position.address)?.data,
        )?;
        require(
            terminal_position.cash_atoms == 0
                && token_amount(
                    &rpc,
                    "terminal user collateral token",
                    plane::actor_collateral(payer.pubkey()),
                )? == plane::USER_COLLATERAL_ATOMS
                && token_amount(
                    &rpc,
                    "terminal Hoard token account",
                    correct.plane.hoard_token.address,
                )? == 0,
            "WithdrawCash did not return exact redeemed collateral to the ephemeral user",
        )?;
        let create_market_signature = lifecycle_signatures
            .get("create_market")
            .ok_or("joined campaign lost its CreateMarket signature")?;
        let endow_signature = lifecycle_signatures
            .get("endow")
            .ok_or("joined campaign lost its Endow signature")?;
        let split_signature = lifecycle_signatures
            .get("split")
            .ok_or("joined campaign lost its Split signature")?;
        json!({
            "market_genesis_assisted": false,
            "market": correct.plane.market.address.to_string(),
            "ephemeral_user": payer.pubkey().to_string(),
            "user_collateral_token": plane::actor_collateral(payer.pubkey()).to_string(),
            "collateral_atoms": plane::USER_COLLATERAL_ATOMS.to_string(),
            "create_market_signature": create_market_signature,
            "endow_signature": endow_signature,
            "split_signature": split_signature,
            "redeem_internal": redeem_signatures,
            "withdraw_signature": withdraw_signature,
            "terminal": {
                "position_cash_atoms": "0",
                "position_internal": ["0", "0", "0", "0"],
                "supply_internal": ["0", "0", "0", "0"],
                "hoard_collateral_atoms": "0",
                "hoard_token_atoms": "0",
                "user_token_atoms": plane::USER_COLLATERAL_ATOMS.to_string(),
            },
            "trade": {
                "status": "blocked",
                "reason_code": "missing-sealed-price-grid-and-epoch-plane",
                "detail": "the immutable real-Pyth-bound Terms name a PriceGrid digest, but this campaign has no matching sealed PriceGrid artifact, Epoch, order page, or candidate plane; InitEpoch authenticates that exact grid, so placing or settling orders would require additional signed artifact/epoch construction and is not replaced with genesis or mock trading state",
            },
        })
    } else {
        Value::Null
    };

    let result = json!({
        "claim": CLAIM,
        "campaign_mode": campaign_mode,
        "network": "loopback validator only",
        "genesis_hash": rpc.genesis_hash()?,
        "clock": assert_clock(&rpc, publish_time)?,
        "publish_time": publish_time,
        "provider_feed_id": Hash32::from_bytes(provider::FEED_ID).bytes(),
        "price": provider::PRICE,
        "confidence": provider::CONFIDENCE,
        "exponent": provider::EXPONENT,
        "interval": {"lower": lower.to_string(), "upper": upper.to_string()},
        "verified_vaa_account": encoded.pubkey().to_string(),
        "update_account": update.pubkey().to_string(),
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
    fs::write(
        work.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    if campaign_mode == JOINED_LIFECYCLE_MODE {
        println!("PASS: signed CreateMarket -> Endow -> Split -> real router/receiver source -> Seal -> Resolve(1) -> RedeemInternal(all outcomes) -> WithdrawCash");
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
}
