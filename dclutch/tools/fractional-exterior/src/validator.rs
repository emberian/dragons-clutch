//! Private validator lifecycle and finalized submission.
//!
//! The validator is consumed at the process boundary: this spawns the same
//! pinned `solana-test-validator` the successor launcher wraps, on its own
//! ledger and port, and talks to it only over RPC. No code, build, or workspace
//! is shared with any other tool.

use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey as RpcPubkey,
    signature::{Keypair, Signer, keypair_from_seed},
    transaction::{Transaction, VersionedTransaction},
};

use crate::{
    Error, Result, journal,
    stage::{self, Elves, Staged, StagedAccount},
};
use dclutch_fractional_exterior::bridge::{
    ElfPinsV1, PRETERMINAL_SCHEMA_V1, PreterminalBridgeV1, ROUNDING_BOUNDARY_V1, write_atomic,
};

/// RPC port this exterior owns. Deliberately not the successor launcher's 20890.
///
/// The whole port block is derived from it, the way the successor launcher
/// derives its own: gossip at BASE+3 and a dynamic range at BASE+10..BASE+41.
/// The default gossip port is 8000, which is a busy address on a developer
/// machine and produces a bind panic rather than a readable refusal.
pub const RPC_PORT: u16 = 20961;
const ACTOR_SEED: [u8; 32] = [0x2c; 32];
const SLEEPER_SEED: [u8; 32] = [0x5c; 32];

pub(crate) fn rpc(key: solana_program::pubkey::Pubkey) -> RpcPubkey {
    RpcPubkey::new_from_array(key.to_bytes())
}

pub(crate) fn rent_exempt(space: usize) -> u64 {
    solana_program::rent::Rent::default()
        .minimum_balance(space)
        .max(1)
}

pub(crate) fn account_file(
    key: &RpcPubkey,
    owner: &RpcPubkey,
    data: &[u8],
    executable: bool,
) -> Value {
    json!({
        "pubkey": key.to_string(),
        "account": {
            "lamports": rent_exempt(data.len()),
            "data": [STANDARD.encode(data), "base64"],
            "owner": owner.to_string(),
            "executable": executable,
            "rentEpoch": 0,
            "space": data.len(),
        }
    })
}

/// Loader-V3 `Program` account: variant 2 then the ProgramData address.
pub(crate) fn program_account_bytes(programdata: &RpcPubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; 36];
    bytes[0..4].copy_from_slice(&2_u32.to_le_bytes());
    bytes[4..36].copy_from_slice(&programdata.to_bytes());
    bytes
}

/// Loader-V3 `ProgramData`: variant 3, slot 0, authority `None`, then the ELF.
///
/// Written directly rather than through `--upgradeable-program ... none`,
/// because Solana 4.0.2 encodes that spelling as option tag 1 plus the zero
/// Pubkey rather than immutable option tag 0 -- the release authentication
/// reads the difference.
pub(crate) fn programdata_bytes(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[45..].copy_from_slice(elf);
    bytes
}

pub(crate) fn programdata_address(program: &RpcPubkey) -> RpcPubkey {
    RpcPubkey::find_program_address(&[program.as_ref()], &rpc(stage::loader())).0
}

pub(crate) fn write_account(dir: &Path, value: &Value) -> Result<()> {
    let key = value
        .get("pubkey")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("account file has no pubkey"))?;
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(dir.join(format!("{key}.json")), bytes)?;
    Ok(())
}

fn staged_programs(elves: &Elves<'_>) -> Vec<(RpcPubkey, Vec<u8>)> {
    vec![
        (rpc(stage::CLAIMS), elves.claims.to_vec()),
        (rpc(stage::REGISTRY), elves.registry.to_vec()),
        (rpc(stage::CORE), elves.core.to_vec()),
        (rpc(stage::CUSTODY), elves.custody.to_vec()),
        (rpc(stage::CALLER), elves.caller.to_vec()),
    ]
}

/// Stage the fixture and every program as genesis account files.
pub fn prepare(elf_dir: &Path, out: &Path) -> Result<usize> {
    let accounts_dir = out.join("accounts");
    if accounts_dir.exists() {
        fs::remove_dir_all(&accounts_dir)?;
    }
    fs::create_dir_all(&accounts_dir)?;

    let claims = crate::read_elf(elf_dir, "dclutch_claims_sbf.so")?;
    let registry = crate::read_elf(elf_dir, "dclutch_registry_sbf.so")?;
    let core = crate::read_elf(elf_dir, "dclutch_core_sbf.so")?;
    let custody = crate::read_elf(elf_dir, "dclutch_custody_sbf.so")?;
    let caller = crate::read_elf(elf_dir, "dclutch_fractional_compaction_test_caller_sbf.so")?;
    let token = crate::read_elf(elf_dir, "spl_token_2022.so")?;
    let elves = Elves {
        claims: &claims,
        registry: &registry,
        core: &core,
        custody: &custody,
        caller: &caller,
    };

    let actor = keypair_from_seed(&ACTOR_SEED).map_err(|error| Error::new(error.to_string()))?;
    let sleeper =
        keypair_from_seed(&SLEEPER_SEED).map_err(|error| Error::new(error.to_string()))?;
    let staged = stage::stage(
        &elves,
        solana_program::pubkey::Pubkey::new_from_array(actor.pubkey().to_bytes()),
        solana_program::pubkey::Pubkey::new_from_array(sleeper.pubkey().to_bytes()),
    );

    let mut written = 0_usize;
    for StagedAccount { key, owner, data } in &staged.accounts {
        write_account(
            &accounts_dir,
            &account_file(&rpc(*key), &rpc(*owner), data, false),
        )?;
        written += 1;
    }
    let mut programs = staged_programs(&elves);
    programs.push((rpc(stage::token_program()), token));
    for (program, elf) in &programs {
        let data_key = programdata_address(program);
        write_account(
            &accounts_dir,
            &account_file(
                program,
                &rpc(stage::loader()),
                &program_account_bytes(&data_key),
                true,
            ),
        )?;
        write_account(
            &accounts_dir,
            &account_file(
                &data_key,
                &rpc(stage::loader()),
                &programdata_bytes(elf),
                false,
            ),
        )?;
        written += 2;
    }

    let manifest = json!({
        "schema": "dclutch/fractional-exterior/manifest/v1",
        "representation_width": stage::WIDTH,
        "accounts": written,
        "actor": actor.pubkey().to_string(),
        "sleeping_holder": sleeper.pubkey().to_string(),
        "actions": staged.actions.iter().map(|a| a.name).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(out.join("manifest.json"), bytes)?;
    Ok(written)
}

pub(crate) struct Validator {
    child: Child,
}

impl Validator {
    fn start(out: &Path) -> Result<Self> {
        Self::start_on(out, RPC_PORT, "")
    }

    pub(crate) fn start_on(out: &Path, rpc_port: u16, prefix: &str) -> Result<Self> {
        let ledger = out.join(format!("{prefix}ledger"));
        if ledger.exists() {
            fs::remove_dir_all(&ledger)?;
        }
        let log = fs::File::create(out.join(format!("{prefix}validator.log")))?;
        let gossip_port = rpc_port
            .checked_add(3)
            .ok_or_else(|| Error::new("validator gossip port overflow"))?;
        let dynamic_low = rpc_port
            .checked_add(10)
            .ok_or_else(|| Error::new("validator dynamic port overflow"))?;
        let dynamic_high = rpc_port
            .checked_add(41)
            .ok_or_else(|| Error::new("validator dynamic port overflow"))?;
        let child = Command::new("solana-test-validator")
            .arg("--ledger")
            .arg(&ledger)
            .arg("--account-dir")
            .arg(out.join(format!("{prefix}accounts")))
            .arg("--rpc-port")
            .arg(rpc_port.to_string())
            .arg("--gossip-port")
            .arg(gossip_port.to_string())
            .arg("--dynamic-port-range")
            .arg(format!("{dynamic_low}-{dynamic_high}"))
            .arg("--reset")
            .arg("--quiet")
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|error| Error::new(format!("solana-test-validator did not start: {error}")))?;
        Ok(Self { child })
    }

    pub(crate) fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn submit_legacy(client: &RpcClient, payer: &Keypair, instructions: &[Instruction]) -> Result<()> {
    let blockhash = client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    client.send_and_confirm_transaction(&transaction)?;
    Ok(())
}

pub(crate) fn await_health(client: &RpcClient) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last = String::new();
    while Instant::now() < deadline {
        match client.get_health() {
            Ok(()) => return Ok(()),
            Err(error) => last = error.to_string(),
        }
        sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!("validator never became healthy: {last}")).into())
}

fn balance_at(data: &[u8], offset: usize) -> u64 {
    let mut bytes = [0_u8; 8];
    if data.len() >= offset + 8 {
        bytes.copy_from_slice(&data[offset..offset + 8]);
    }
    u64::from_le_bytes(bytes)
}

fn poststate(client: &RpcClient, staged: &Staged) -> Result<Value> {
    const POSITION_HEADER: usize = 128;
    let claim_offset = staged
        .representation_coordinate
        .checked_mul(8)
        .and_then(|relative| POSITION_HEADER.checked_add(relative))
        .ok_or_else(|| Error::new("representation-coordinate offset overflow"))?;
    let mint = client.get_account_data(&rpc(staged.shard_mint))?;
    let holder = client.get_account_data(&rpc(staged.holder_token))?;
    let sleeper = client.get_account_data(&rpc(staged.sleeper_token))?;
    let actor = client.get_account_data(&rpc(staged.actor_position))?;
    let reserve = client.get_account_data(&rpc(staged.reserve_position))?;
    Ok(json!({
        "shard_mint_supply": balance_at(&mint, 36),
        "holder_token_amount": balance_at(&holder, 64),
        "sleeper_token_amount": balance_at(&sleeper, 64),
        "actor_native_claims": balance_at(&actor, claim_offset),
        "reserve_native_claims": balance_at(&reserve, claim_offset),
    }))
}

fn expected_poststate(action: &stage::StagedAction) -> Value {
    json!({
        "shard_mint_supply": action.expected.shard_mint_supply,
        "holder_token_amount": action.expected.holder_token_amount,
        "sleeper_token_amount": action.expected.sleeper_token_amount,
        "actor_native_claims": action.expected.actor_native_claims,
        "reserve_native_claims": action.expected.reserve_native_claims,
    })
}

pub(crate) fn refusal_code(error: &str) -> Option<u32> {
    let marker = "custom program error: 0x";
    let start = error.find(marker)? + marker.len();
    let hex: String = error[start..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    u32::from_str_radix(&hex, 16).ok()
}

/// Prepare, run every action to finalized, and journal the result.
pub fn run(elf_dir: &Path, out: &Path, keep: bool) -> Result<()> {
    let written = prepare(elf_dir, out)?;
    println!("staged {written} genesis accounts");

    let claims = crate::read_elf(elf_dir, "dclutch_claims_sbf.so")?;
    let registry = crate::read_elf(elf_dir, "dclutch_registry_sbf.so")?;
    let core = crate::read_elf(elf_dir, "dclutch_core_sbf.so")?;
    let custody = crate::read_elf(elf_dir, "dclutch_custody_sbf.so")?;
    let caller = crate::read_elf(elf_dir, "dclutch_fractional_compaction_test_caller_sbf.so")?;
    let actor = keypair_from_seed(&ACTOR_SEED).map_err(|error| Error::new(error.to_string()))?;
    let sleeper =
        keypair_from_seed(&SLEEPER_SEED).map_err(|error| Error::new(error.to_string()))?;
    let staged = stage::stage(
        &Elves {
            claims: &claims,
            registry: &registry,
            core: &core,
            custody: &custody,
            caller: &caller,
        },
        solana_program::pubkey::Pubkey::new_from_array(actor.pubkey().to_bytes()),
        solana_program::pubkey::Pubkey::new_from_array(sleeper.pubkey().to_bytes()),
    );

    let validator = Validator::start(out)?;
    let client = RpcClient::new_with_commitment(
        format!("http://127.0.0.1:{RPC_PORT}"),
        CommitmentConfig::finalized(),
    );
    let outcome = (|| -> Result<Vec<journal::Entry>> {
        await_health(&client)?;
        let payer = Keypair::new();
        let signature = client.request_airdrop(&payer.pubkey(), 5_000_000_000)?;
        let deadline = Instant::now() + Duration::from_secs(60);
        while !client.confirm_transaction(&signature)? {
            if Instant::now() > deadline {
                return Err(Error::new("airdrop never confirmed").into());
            }
            sleep(Duration::from_millis(250));
        }

        // A Fractional action names 31 accounts and carries a 416-byte request.
        // That does not fit a legacy transaction -- it compiles to 2,192 bytes
        // against Solana's 1,232-byte packet -- so a real cluster requires the
        // frame to travel through an address lookup table. ProgramTest cannot
        // surface this: it enforces no packet size. The static topology census
        // predicted it; this is where it is paid.
        let mut table_addresses: Vec<RpcPubkey> = Vec::new();
        for action in &staged.actions {
            for meta in &action.metas {
                let key = rpc(meta.key);
                if key != actor.pubkey() && key != payer.pubkey() && !table_addresses.contains(&key)
                {
                    table_addresses.push(key);
                }
            }
        }
        // The lookup-table program requires `recent_slot` to already be in the
        // SlotHashes sysvar, so it must be strictly behind the current slot.
        // The ProgramTest campaign warps a slot to arrange that; a real cluster
        // is simply waited on.
        let slot = client.get_slot()?;
        while client.get_slot()? <= slot {
            sleep(Duration::from_millis(200));
        }
        let (create, table_key) = create_lookup_table(payer.pubkey(), payer.pubkey(), slot);
        submit_legacy(&client, &payer, &[create])?;
        for chunk in table_addresses.chunks(20) {
            let extend = extend_lookup_table(
                table_key,
                payer.pubkey(),
                Some(payer.pubkey()),
                chunk.to_vec(),
            );
            submit_legacy(&client, &payer, &[extend])?;
        }
        // A table is only usable one slot after the block that extended it.
        let warm = client.get_slot()?;
        while client.get_slot()? <= warm + 1 {
            sleep(Duration::from_millis(200));
        }
        let table = AddressLookupTableAccount {
            key: table_key,
            addresses: table_addresses.clone(),
        };
        println!(
            "lookup table {table_key} carries {} addresses",
            table.addresses.len()
        );

        let mut entries = Vec::new();
        for action in &staged.actions {
            // Every meta is an account the caller receives, including the Claims
            // program at index 0. The invoked program id is separate and is not
            // one of them.
            let metas: Vec<AccountMeta> = action
                .metas
                .iter()
                .map(|meta| AccountMeta {
                    pubkey: rpc(meta.key),
                    is_signer: meta.signer,
                    is_writable: meta.writable,
                })
                .collect();
            let mut frame = Vec::new();
            for meta in &action.metas {
                frame.extend_from_slice(&meta.key.to_bytes());
                frame.push(u8::from(meta.signer));
                frame.push(u8::from(meta.writable));
            }
            let instruction = Instruction {
                program_id: rpc(action.program),
                accounts: metas,
                data: action.data.clone(),
            };
            // A Fractional action costs far more than the 200,000-unit default a
            // transaction gets without asking. ProgramTest is configured with a
            // ceiling; a real cluster must be told.
            let budget = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
            let blockhash = client.get_latest_blockhash()?;
            let message = v0::Message::try_compile(
                &payer.pubkey(),
                &[budget, instruction.clone()],
                core::slice::from_ref(&table),
                blockhash,
            )?;
            let transaction =
                VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer, &actor])?;
            let wire = bincode::serialize(&transaction)?.len();
            let submitted = client.send_and_confirm_transaction(&transaction);
            let (accepted, refusal, signature, detail) = match &submitted {
                Ok(signature) => (true, None, signature.to_string(), String::new()),
                Err(error) => {
                    let text = error.to_string();
                    (false, refusal_code(&text), String::new(), text)
                }
            };
            let state = poststate(&client, &staged)?;
            journal::append_observed(
                out,
                &json!({
                    "action": action.name,
                    "signature": signature,
                    "accepted": accepted,
                    "wire_bytes": wire,
                    "detail": detail,
                }),
            )?;
            println!(
                "{:>14}  accepted={accepted}  {}",
                action.name,
                serde_json::to_string(&state)?
            );
            if !accepted {
                return Err(Error::new(format!(
                    "{} refused instead of committing: code={refusal:?} {detail}",
                    action.name
                ))
                .into());
            }
            let expected = expected_poststate(action);
            if state != expected {
                return Err(Error::new(format!(
                    "{} committed an unexpected poststate: expected {} got {}",
                    action.name,
                    serde_json::to_string(&expected)?,
                    serde_json::to_string(&state)?,
                ))
                .into());
            }
            entries.push(journal::Entry {
                name: action.name.to_string(),
                data_digest: journal::digest(&action.data),
                frame_digest: journal::digest(&frame),
                accepted,
                refusal,
                poststate: state,
            });
        }
        Ok(entries)
    })();

    if keep {
        println!("validator left running on port {RPC_PORT}");
    } else {
        validator.stop();
    }
    let entries = outcome?;
    let digest = journal::write_canonical(out, &entries)?;
    println!("canonical journal sha256 {digest}");
    Ok(())
}

/// Emit the exact preterminal output facts consumed by real-ELF compaction.
pub fn write_preterminal_bridge(
    elf_dir: &Path,
    out: &Path,
    bridge_path: &Path,
    source_commit: String,
    source_tree_sha256: String,
) -> Result<String> {
    let (_, journal_sha256) = journal::verify(out)?;
    let claims = crate::read_elf(elf_dir, "dclutch_claims_sbf.so")?;
    let registry = crate::read_elf(elf_dir, "dclutch_registry_sbf.so")?;
    let core = crate::read_elf(elf_dir, "dclutch_core_sbf.so")?;
    let custody = crate::read_elf(elf_dir, "dclutch_custody_sbf.so")?;
    let rent = crate::read_elf(elf_dir, "dclutch_rent_sbf.so")?;
    let trading = crate::read_elf(elf_dir, "dclutch_fractional_compaction_test_caller_sbf.so")?;
    let token = crate::read_elf(elf_dir, "spl_token_2022.so")?;
    let actor = keypair_from_seed(&ACTOR_SEED).map_err(|error| Error::new(error.to_string()))?;
    let sleeper =
        keypair_from_seed(&SLEEPER_SEED).map_err(|error| Error::new(error.to_string()))?;
    let staged = stage::stage(
        &Elves {
            claims: &claims,
            registry: &registry,
            core: &core,
            custody: &custody,
            caller: &trading,
        },
        solana_program::pubkey::Pubkey::new_from_array(actor.pubkey().to_bytes()),
        solana_program::pubkey::Pubkey::new_from_array(sleeper.pubkey().to_bytes()),
    );
    let bridge = PreterminalBridgeV1 {
        schema: PRETERMINAL_SCHEMA_V1.into(),
        source_commit,
        source_tree_sha256,
        elves: ElfPinsV1 {
            claims: journal::digest(&claims),
            registry: journal::digest(&registry),
            core: journal::digest(&core),
            custody: journal::digest(&custody),
            rent: journal::digest(&rent),
            trading: journal::digest(&trading),
            token_2022: journal::digest(&token),
        },
        journal_sha256,
        release_set: staged.release_set,
        realm: staged.realm,
        market: staged.market.to_bytes(),
        aggregate: staged.aggregate.to_bytes(),
        product: staged.product,
        product_basis: staged.product_basis,
        terms: staged.terms,
        root: staged.root.to_bytes(),
        shard_mint: staged.shard_mint.to_bytes(),
        holder: staged.sleeper_owner.to_bytes(),
        holder_shard_token: staged.sleeper_token.to_bytes(),
        denominator: 10,
        representation_coordinate: u32::try_from(staged.representation_coordinate)
            .map_err(|_| Error::new("representation coordinate overflow"))?,
        outstanding_shards: staged.sleeper_shards,
        reserve_native_claims: staged.sleeper_shards / 10,
        curve_degree: 3,
        payout_scale: 11,
        rounding_boundary: ROUNDING_BOUNDARY_V1.into(),
    };
    bridge.validate().map_err(Error::new)?;
    write_atomic(bridge_path, &bridge)
        .map_err(Error::new)
        .map_err(Into::into)
}
