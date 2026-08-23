//! `operatord serve --mode trade`: the Friday clutch, opened for a person.
//!
//! Watch mode boots a validator and replays a pregenerated plan.  Trade mode
//! boots the same validator the same way, installs the Friday clutch's frozen
//! prerequisites instead of the general lane's, founds the market for real,
//! and then does nothing until somebody asks it to — the whole point being
//! that the book is authored at the keyboard rather than emitted.
//!
//! The one rule the bench keeps from watch mode is the one that matters: the
//! browser still never builds a transaction.  It posts an intent — a knot, a
//! side, a quantity, a limit — and `builders.rs` hands that to
//! `clutch_sbf_harness::general_transaction`, which is the same serializer the
//! sealed lane's plan generator uses.

use crate::bus::Bus;
use crate::friday::{Friday, Row};
use crate::integer;
use crate::session::Session;
use crate::{rpc, toolchain};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, process, thread};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Collateral atoms each actor deposits at founding.
const ENDOW_ATOMS: u64 = 20_000;
/// Complete sets each actor locks, funding the Egg side of its quotes.
const SPLIT_SETS: u64 = 4_000;

/// Write the genesis directory `solana-test-validator` is started from.
///
/// Same two artefacts the plan emitter writes and the validator reads: one
/// JSON file per account and a `genesis.txt` index of `role address file`.
fn write_genesis(dir: &Path, rows: &[Row]) -> Result<Vec<String>> {
    fs::create_dir_all(dir.join("accounts"))?;
    let mut index = String::new();
    let mut precreated = Vec::new();
    for row in rows {
        let file = format!("accounts/{}.json", row.role);
        let body = format!(
            "{{\n  \"pubkey\": \"{}\",\n  \"account\": {{\n    \"lamports\": {},\n    \
             \"data\": [\n      \"{}\",\n      \"base64\"\n    ],\n    \"owner\": \"{}\",\n    \
             \"executable\": false,\n    \"rentEpoch\": 0,\n    \"space\": {}\n  }}\n}}\n",
            row.address,
            clutch_sbf_harness::ACCOUNT_LAMPORTS,
            clutch_sbf_harness::b64_encode(&row.data),
            row.owner,
            row.data.len()
        );
        fs::write(dir.join(&file), body)?;
        writeln!(index, "{} {} {}", row.role, row.address, file)?;
        precreated.push(format!("{} {}", row.role, row.address));
    }
    fs::write(dir.join("genesis.txt"), index)?;
    Ok(precreated)
}

/// The permanent honesty header, in the trade session's own vocabulary.
fn banner(
    artifact: &toolchain::Artifact,
    validator: &toolchain::Validator,
    program_id: &str,
    precreated: &[String],
) -> Value {
    json!({
        "type": "identity",
        "mode": "trade",
        "integer_transport": integer::TRANSPORT,
        "source_profile": artifact.source_profile,
        "elf_path": artifact.path.display().to_string(),
        "elf_bytes": artifact.bytes,
        "elf_sha256": artifact.sha256,
        "program_id": program_id,
        "rpc_url": validator.url,
        "ledger": validator.ledger.display().to_string(),
        "genesis_assisted": true,
        "precreated": precreated,
        "evidence_scope": "SBF_EXECUTED",
        "promotion": "unpromoted",
        "network": "LOCAL; OPERATOR HTTP LOOPBACK; VALIDATOR RPC/FAUCET BIND AUDITED SEPARATELY",
        "value": "no value",
    })
}

/// Everything the daemon needs to open a trade session.
pub struct Options {
    pub port: u16,
    pub rpc_port: u16,
    pub faucet_port: u16,
    pub gossip_port: u16,
    pub dynamic_port_range: String,
    pub work: PathBuf,
    pub statics: PathBuf,
    pub freeze_window: u64,
    pub exit_when_settled: bool,
}

#[allow(clippy::too_many_lines)] // boot is one sequence, and it reads as one
pub fn serve(options: Options) -> Result<()> {
    toolchain::validate_validator_network(
        Some(options.port),
        options.rpc_port,
        options.faucet_port,
        options.gossip_port,
        &options.dynamic_port_range,
    )?;
    let url = format!("http://127.0.0.1:{}", options.rpc_port);
    rpc::require_loopback(&url)?;
    toolchain::refuse_occupied_port(&url)?;

    let work = options.work;
    fs::create_dir_all(&work)?;
    let plan_dir = work.join("genesis");
    let out_dir = work.join("out");
    fs::create_dir_all(&out_dir)?;

    std::env::set_var("SOLANA_BIN", toolchain::solana());
    let keys = toolchain::Keys::mint(&work)?;
    keys.export();

    let bus = Bus::new();
    let server_bus = Arc::clone(&bus);
    let stage = |stage: &str, text: &str| {
        bus.publish(&json!({"type": "boot", "stage": stage, "text": text}));
        println!("[{stage}] {text}");
    };

    stage("keys", "eight ephemeral test-only signers minted");
    bus.publish(&json!({"type": "roster", "actors": keys.roster()}));

    stage("elf", "building the NON-PRODUCTION mock-source SBF ELF");
    let artifact = toolchain::build_artifact(
        &crate::repo_path("programs/clutch-sbf/program/Cargo.toml"),
        &out_dir,
        &work.join("build.log"),
    )?;

    stage(
        "fixture",
        "deriving the Friday clutch: eight outcomes, degree 1",
    );
    let friday = Friday::build(clutch_sbf_harness::build_shared());
    let program_id = friday.shared.program.address.clone();
    let precreated = write_genesis(&plan_dir, &friday.genesis())?;

    stage("validator", "starting a fresh local ledger");
    let mut validator = toolchain::Validator::start(
        &work,
        &plan_dir,
        &artifact,
        &program_id,
        keys.public_key("payer").ok_or("no payer public key")?,
        toolchain::ValidatorNetwork {
            rpc_port: options.rpc_port,
            faucet_port: options.faucet_port,
            gossip_port: options.gossip_port,
            dynamic_port_range: &options.dynamic_port_range,
        },
    )?;
    validator.await_ready(&program_id)?;
    validator.probe_listeners(&work.join("listeners-before.txt"))?;
    bus.publish(&banner(&artifact, &validator, &program_id, &precreated));

    let keypairs: Vec<solana_keypair::Keypair> = keys
        .paths()
        .iter()
        .map(solana_keypair::read_keypair_file)
        .collect::<std::result::Result<_, _>>()?;
    let session = Arc::new(Session::new(
        friday,
        Arc::clone(&bus),
        keypairs,
        url.clone(),
        options.freeze_window,
    ));
    bus.publish(&json!({"type": "market", "identity": session.identity()}));

    let action_session = Arc::clone(&session);
    let action: crate::http::Action =
        Arc::new(move |request: &Value| respond(&action_session, request));
    let server = crate::http::Server::bind(options.port, server_bus, options.statics, action)?;
    let port = server.port()?;
    thread::spawn(move || server.serve_forever());
    println!("Operator Bench (trade): http://127.0.0.1:{port}/");

    stage("found", "founding the market and funding both actors");
    session.found(ENDOW_ATOMS, SPLIT_SETS)?;
    validator.probe_listeners(&work.join("listeners-after.txt"))?;

    if options.exit_when_settled {
        println!("trade session open; waiting for the scripted flow to settle it");
        loop {
            thread::sleep(Duration::from_millis(500));
            let phase = session.snapshot();
            match phase.get("phase").and_then(Value::as_str) {
                Some("settled") => break,
                Some("faulted") => {
                    drop(validator);
                    drop(keys);
                    process::exit(1);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_secs(2));
        drop(validator);
        drop(keys);
        process::exit(0);
    }
    println!("trade session open; press Ctrl-C to stop");
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

/// The action verbs a trade session admits.
///
/// Each one is an *intent*, never a transaction: a knot, a side, a size, a
/// limit.  What that becomes on the wire is decided in `builders.rs` and
/// serialized by the harness.
fn respond(session: &Arc<Session>, request: &Value) -> Value {
    handle(session, request).unwrap_or_else(|error| {
        json!({
            "ok": false,
            "integer_transport": integer::TRANSPORT,
            "detail": error.to_string()
        })
    })
}

fn handle(session: &Arc<Session>, request: &Value) -> Result<Value> {
    if request
        .get("integer_transport")
        .is_some_and(|value| value.as_str() != Some(integer::TRANSPORT))
    {
        return Err(format!(
            "unsupported integer_transport; expected {}",
            integer::TRANSPORT
        )
        .into());
    }
    let name = request.get("action").and_then(Value::as_str).unwrap_or("");
    let number =
        |field: &str| -> Result<u64> { integer::field_u64(request, field).map_err(Into::into) };
    let vector = |field: &str| -> Result<Vec<u64>> {
        integer::field_u64_values(request, field).map_err(Into::into)
    };
    let side = || -> Result<u8> {
        match request.get("side").and_then(Value::as_str) {
            Some("buy") => Ok(0),
            Some("sell") => Ok(1),
            _ => Err("side must be exactly \"buy\" or \"sell\"".into()),
        }
    };
    match name {
        "status" => Ok(session.snapshot()),
        "bot" => Ok(json!({
            "ok": true,
            "integer_transport": integer::TRANSPORT,
            "disclosure": session.bot.disclosure()
        })),
        "place" => session.place_single(
            u8::try_from(number("outcome")?)?,
            side()?,
            number("quantity")?,
            number("limit")?,
        ),
        "place-portfolio" => session.place_portfolio(
            &vector("coefficients")?,
            side()?,
            number("lots")?,
            number("limit_per_lot")?,
        ),
        "endow" => session.endow(number("amount")?),
        "split" => session.split(number("quantity")?),
        "cancel" => session.cancel(number("rank")?),
        "propose" => Ok(session.propose(&vector("belief")?)),
        "weights" => Ok(session.weights_at(number("cents")?)),
        "paint" => session.paint(&vector("belief")?),
        "freeze" => {
            let worker = Arc::clone(session);
            thread::spawn(move || {
                if let Err(error) = worker.freeze_and_settle() {
                    worker.fault(&error.to_string());
                }
            });
            Ok(json!({
                "ok": true,
                "integer_transport": integer::TRANSPORT,
                "detail": "closing at the deadline, then driving the epoch to settled; \
                           watch the step log",
            }))
        }
        other => Ok(json!({
            "ok": false,
            "detail": format!("a trade session has no action named {other:?}"),
        })),
    }
}
