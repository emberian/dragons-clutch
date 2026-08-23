//! `operatord` — the Operator Bench daemon.
//!
//! A loopback-only Operator HTTP daemon that boots a local `solana-test-validator`
//! exactly as `scripts/run_general_committed.sh` does, drives the
//! forty-four-step general-clearing walk through the committed-harness code
//! path, and publishes every transition to a zero-dependency static page.
//!
//! The design rule, restated where it is enforced: **the browser never builds
//! a transaction.**  It has no RPC client, no wallet, no serializer, and no
//! key material.  Every byte submitted here is built by `clutch_sbf_harness`,
//! the same library the sealed lane's plan generator is, and
//! `operatord replay` byte-diffs that claim.
//!
//! The chain-attached surface is `chain-serve` and requires an explicit
//! genesis/release/decoder configuration. Historical mock and synthetic
//! laboratories remain reachable only through command modes whose names begin
//! with `non-production-`; none is a live discovery fallback.

mod bot;
mod builders;
mod bus;
mod chain_server;
mod crank;
mod decode;
mod friday;
mod http;
mod index_api;
mod integer;
mod payoff_compiler;
mod plan;
mod processed_ws;
mod pyth;
mod pyth_live;
mod quantize;
mod replay;
mod rpc;
mod session;
mod toolchain;
mod trade;
mod walk;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bus::Bus;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, process, thread};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Where this crate sits, so the daemon works from any working directory.
const CRATE_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub(crate) fn b64_decode(text: &str) -> Result<Vec<u8>> {
    Ok(BASE64.decode(text)?)
}

pub(crate) fn repo_path(relative: &str) -> PathBuf {
    Path::new(CRATE_DIR).join("../../..").join(relative)
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         operatord serve --mode non-production-mock-watch|non-production-mock-trade|non-production-retained-source-v2|non-production-synthetic-source-v2-live [--port N] [--rpc-port N] [--faucet-port N] \
         [--gossip-port N] [--dynamic-port-range START-END] [--work DIR] [--static DIR] \
         [--freeze-window SLOTS] [--transcript DIR] [--exit-when-done]\n  \
         operatord emit <plan-dir>\n  \
         operatord replay <plan-dir>\n  \
         operatord compiler-serve --compiler-release-sha256 HASH [--port N] [--static DIR]\n  \
         operatord compile-payoff --compiler-release-sha256 HASH < request.json\n  \
         operatord chain-serve --config FILE [--port N] [--static DIR]"
    );
    process::exit(2)
}

struct CompilerServeOptions {
    port: u16,
    statics: PathBuf,
    compiler_release_sha256: String,
}

fn parse_compiler_serve(mut args: impl Iterator<Item = String>) -> Result<CompilerServeOptions> {
    let mut port = 9130_u16;
    let mut statics = repo_path("apps/static-client");
    let mut compiler_release_sha256 = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--port" => port = value()?.parse()?,
            "--static" => statics = PathBuf::from(value()?),
            "--compiler-release-sha256" => compiler_release_sha256 = Some(value()?),
            other => return Err(format!("unknown compiler-serve flag {other}").into()),
        }
    }
    Ok(CompilerServeOptions {
        port,
        statics,
        compiler_release_sha256: compiler_release_sha256
            .ok_or("compiler-serve requires --compiler-release-sha256 HASH")?,
    })
}

fn parse_compiler_release(mut args: impl Iterator<Item = String>) -> Result<String> {
    let Some(flag) = args.next() else {
        return Err("compile-payoff requires --compiler-release-sha256 HASH".into());
    };
    if flag != "--compiler-release-sha256" {
        return Err(format!("unknown compile-payoff flag {flag}").into());
    }
    let release = args
        .next()
        .ok_or("--compiler-release-sha256 needs a value")?;
    if let Some(extra) = args.next() {
        return Err(format!("unexpected compile-payoff argument {extra}").into());
    }
    Ok(release)
}

struct ChainServeOptions {
    port: u16,
    statics: PathBuf,
    config: PathBuf,
}

fn parse_chain_serve(mut args: impl Iterator<Item = String>) -> Result<ChainServeOptions> {
    let mut port = 9130_u16;
    let mut statics = repo_path("apps/static-client");
    let mut config = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--port" => port = value()?.parse()?,
            "--static" => statics = PathBuf::from(value()?),
            "--config" => config = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown chain-serve flag {other}").into()),
        }
    }
    Ok(ChainServeOptions {
        port,
        statics,
        config: config.ok_or("chain-serve requires --config FILE")?,
    })
}

struct Options {
    mode: String,
    port: u16,
    rpc_port: u16,
    faucet_port: u16,
    gossip_port: u16,
    dynamic_port_range: String,
    work: Option<PathBuf>,
    statics: PathBuf,
    transcript: Option<PathBuf>,
    freeze_window: u64,
    exit_when_done: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: String::new(),
            port: 9130,
            rpc_port: 9137,
            // Agave reserves rpc_port + 1 (9138) for RPC WebSocket.
            faucet_port: 9139,
            // The active local Pyth clone owns gossip 9150 and 9151-9199.
            gossip_port: 9200,
            dynamic_port_range: "9201-9250".to_string(),
            work: None,
            statics: repo_path("apps/operator"),
            transcript: None,
            freeze_window: session::FREEZE_WINDOW_SLOTS_DEFAULT,
            exit_when_done: false,
        }
    }
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--mode" => options.mode = value()?,
            "--port" => options.port = value()?.parse()?,
            "--rpc-port" => options.rpc_port = value()?.parse()?,
            "--faucet-port" => options.faucet_port = value()?.parse()?,
            "--gossip-port" => options.gossip_port = value()?.parse()?,
            "--dynamic-port-range" => options.dynamic_port_range = value()?,
            "--work" => options.work = Some(PathBuf::from(value()?)),
            "--static" => options.statics = PathBuf::from(value()?),
            "--transcript" => options.transcript = Some(PathBuf::from(value()?)),
            "--freeze-window" => options.freeze_window = value()?.parse()?,
            "--exit-when-done" => options.exit_when_done = true,
            other => return Err(format!("unknown flag {other}").into()),
        }
    }
    Ok(options)
}

/// Dispatch one explicitly named non-production laboratory session.
fn dispatch(options: Options) -> Result<()> {
    if options.mode.is_empty() {
        return Err("serve requires an explicit non-production --mode; use chain-serve --config FILE for real/local chain discovery".into());
    }
    if options.mode != "non-production-retained-source-v2" {
        toolchain::validate_validator_network(
            Some(options.port),
            options.rpc_port,
            options.faucet_port,
            options.gossip_port,
            &options.dynamic_port_range,
        )?;
    }
    match options.mode.as_str() {
        "non-production-mock-watch" => serve(options),
        "non-production-mock-trade" => trade::serve(trade::Options {
            port: options.port,
            rpc_port: options.rpc_port,
            faucet_port: options.faucet_port,
            gossip_port: options.gossip_port,
            dynamic_port_range: options.dynamic_port_range,
            work: scratch_dir(options.work)?,
            statics: options.statics,
            freeze_window: options.freeze_window,
            exit_when_settled: options.exit_when_done,
        }),
        "non-production-retained-source-v2" => pyth::serve(pyth::Options {
            port: options.port,
            transcript: options
                .transcript
                .ok_or("non-production-retained-source-v2 mode requires --transcript DIR")?,
            statics: options.statics,
            exit_when_done: options.exit_when_done,
        }),
        "non-production-synthetic-source-v2-live" => {
            if options.transcript.is_some() {
                return Err(
                    "non-production-synthetic-source-v2-live refuses --transcript; use non-production-retained-source-v2 for retained evidence".into(),
                );
            }
            pyth_live::serve(pyth_live::Options {
                port: options.port,
                rpc_port: options.rpc_port,
                faucet_port: options.faucet_port,
                gossip_port: options.gossip_port,
                dynamic_port_range: options.dynamic_port_range,
                work: options.work,
                statics: options.statics,
                exit_when_done: options.exit_when_done,
            })
        }
        other => Err(format!(
            "unknown session mode {other}; every laboratory mode must use its complete non-production-* name"
        )
        .into()),
    }
}

fn scratch_dir(requested: Option<PathBuf>) -> Result<PathBuf> {
    let dir = requested.unwrap_or_else(|| {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        env::temp_dir().join(format!("clutch-operator-{}-{stamp}", process::id()))
    });
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The permanent honesty header, published before any state can arrive.
fn banner(
    artifact: &toolchain::Artifact,
    validator: &toolchain::Validator,
    plan: &plan::Plan,
) -> Value {
    json!({
        "type": "identity",
        "mode": "non-production-mock-watch",
        "integer_transport": integer::TRANSPORT,
        "source_profile": artifact.source_profile,
        "elf_path": artifact.path.display().to_string(),
        "elf_bytes": artifact.bytes,
        "elf_sha256": artifact.sha256,
        "program_id": plan.program_id,
        "rpc_url": validator.url,
        "ledger": validator.ledger.display().to_string(),
        "genesis_assisted": plan.genesis_assisted,
        "precreated": plan.precreated_program_accounts,
        "evidence_scope": "SBF_EXECUTED",
        "promotion": "unpromoted",
        "network": "LOCAL; OPERATOR HTTP LOOPBACK; VALIDATOR RPC/FAUCET BIND AUDITED SEPARATELY",
        "value": "no value",
    })
}

fn plan_event(plan: &plan::Plan) -> Value {
    let steps: Vec<Value> = plan
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            json!({
                "ordinal": index + 1,
                "name": step.name,
                "kind": step.kind,
                "family": step.family,
                "oracle": step.oracle,
                "note": step.note,
                "bytes": step.bytes,
                "instructions": step.instructions,
                "compute_limit": step.compute_limit,
                "expect_code_hex": step.expect_code_hex,
                "reference": step.reference,
                "reloads": step.compare.len() + step.identical_to_pre.len(),
                "wait_slot": step.wait_slot.map(integer::u64_value),
                "wait_after": step.wait_after.as_ref().map(|after| json!({
                    "step": after.step, "delta": integer::u64_value(after.delta)
                })),
            })
        })
        .collect();
    json!({
        "type": "plan",
        "compute_unit_ceiling": integer::u64_value(
            u64::from(clutch_sbf_harness::COMPUTE_UNIT_CEILING)
        ),
        "steps": steps,
        "conservation_offsets": {
            "position_cash": plan.conservation.offsets.position_cash,
            "position_reserved": plan.conservation.offsets.position_reserved,
            "position_internal0": plan.conservation.offsets.position_internal0,
            "position_internal1": plan.conservation.offsets.position_internal1,
            "hoard_collateral": plan.conservation.offsets.hoard_collateral,
            "token_amount": plan.conservation.offsets.token_amount,
        },
        "endowed_total": integer::u64_value(plan.conservation.endowed_total),
        "split_total": integer::u64_value(plan.conservation.split_total),
    })
}

fn stage(bus: &Bus, stage: &str, text: &str) {
    bus.publish(&json!({"type": "boot", "stage": stage, "text": text}));
    println!("[{stage}] {text}");
}

#[allow(clippy::too_many_lines)]
fn serve(options: Options) -> Result<()> {
    let url = format!("http://127.0.0.1:{}", options.rpc_port);
    rpc::require_loopback(&url)?;
    toolchain::refuse_occupied_port(&url)?;

    let work = scratch_dir(options.work)?;
    let plan_dir = work.join("plan");
    let out_dir = work.join("out");
    fs::create_dir_all(&out_dir)?;

    // Exported before any thread exists: the plan emitter reads these.
    env::set_var("SOLANA_BIN", toolchain::solana());
    let keys = toolchain::Keys::mint(&work)?;
    keys.export();

    let bus = Bus::new();
    let handle = crank::Crank::new();
    let action_bus = Arc::clone(&bus);
    let action_crank = Arc::clone(&handle);
    /* The only thing an action can change is pacing.  There is deliberately
     * no verb here that builds, reorders, or skips a transaction: the plan is
     * the plan, and a reading surface must not become an authoring one. */
    let action: http::Action = Arc::new(move |request: &Value| {
        let name = request.get("action").and_then(Value::as_str).unwrap_or("");
        match name {
            "pause" => action_crank.set_auto(false),
            "resume" => action_crank.set_auto(true),
            "crank" => action_crank.grant(),
            "ping" => {}
            other => {
                return json!({
                    "ok": false,
                    "detail": format!("the bench has no action named {other:?}"),
                })
            }
        }
        let (auto, granted) = action_crank.snapshot();
        json!({"ok": true, "auto": auto, "granted": granted, "events": action_bus.len()})
    });
    let server = http::Server::bind(options.port, Arc::clone(&bus), options.statics, action)?;
    let port = server.port()?;
    thread::spawn(move || server.serve_forever());
    println!("Operator Bench: http://127.0.0.1:{port}/");

    stage(&bus, "keys", "eight ephemeral test-only signers minted");
    bus.publish(&json!({"type": "roster", "actors": keys.roster()}));

    stage(
        &bus,
        "elf",
        "building the NON-PRODUCTION mock-source SBF ELF",
    );
    let artifact = toolchain::build_artifact(
        &repo_path("programs/clutch-sbf/program/Cargo.toml"),
        &out_dir,
        &work.join("build.log"),
    )?;

    stage(
        &bus,
        "plan",
        "emitting the general-clearing plan in process",
    );
    replay::emit(&plan_dir)?;
    let parsed = plan::Plan::load(&plan_dir)?;
    bus.publish(&plan_event(&parsed));

    stage(&bus, "validator", "starting a fresh local ledger");
    let mut validator = toolchain::Validator::start(
        &work,
        &plan_dir,
        &artifact,
        &parsed.program_id,
        keys.public_key("payer").ok_or("no payer public key")?,
        toolchain::ValidatorNetwork {
            rpc_port: options.rpc_port,
            faucet_port: options.faucet_port,
            gossip_port: options.gossip_port,
            dynamic_port_range: &options.dynamic_port_range,
        },
    )?;
    validator.await_ready(&parsed.program_id)?;
    validator.probe_listeners(&work.join("listeners-before.txt"))?;
    bus.publish(&banner(&artifact, &validator, &parsed));
    stage(
        &bus,
        "walk",
        "driving the signed, confirmed, committed walk",
    );

    let keypairs = walk::load_keypairs(&parsed, &keys.paths())?;
    let walker = walk::Walker {
        url: &validator.url,
        plan_dir: &plan_dir,
        plan: &parsed,
        bus: &bus,
        crank: &handle,
        observed_dir: work.join("observed"),
    };
    let outcome = walker.run(&keypairs);
    let green = match &outcome {
        Ok(terminal) => {
            let (event, green) = walk::conservation_epilogue(&parsed, terminal);
            bus.publish(&event);
            green
        }
        Err(error) => {
            bus.publish(&json!({"type": "fault", "text": error.to_string()}));
            false
        }
    };
    validator.probe_listeners(&work.join("listeners-after.txt"))?;
    bus.publish(&json!({
        "type": "done",
        "verdict": if green { "PASS" } else { "FAIL" },
        "work_dir": work.display().to_string(),
        "elf_sha256": artifact.sha256,
    }));
    println!(
        "verdict={} work_dir={}",
        if green { "PASS" } else { "FAIL" },
        work.display()
    );

    if options.exit_when_done {
        // A moment for an attached watcher to drain the stream.
        thread::sleep(Duration::from_secs(2));
        drop(validator);
        drop(keys);
        process::exit(i32::from(!green));
    }
    println!("bench still serving; press Ctrl-C to stop");
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn run_replay(plan_dir: &Path) -> Result<()> {
    let scratch = replay::scratch_beside(plan_dir);
    let verdict = replay::replay(plan_dir, &scratch)?;
    let _ignored = fs::remove_dir_all(&scratch);
    println!("replay_files_compared={}", verdict.compared);
    println!("replay_transactions_compared={}", verdict.transactions);
    println!("replay_differing={}", verdict.differing.len());
    for name in verdict.differing.iter().take(20) {
        println!("  DIFFERS {name}");
    }
    for name in verdict.missing.iter().take(20) {
        println!("  MISSING {name}");
    }
    println!("replay_corruption_detected={}", verdict.corruption_detected);
    if verdict.green() {
        println!("replay=PASS every plan file rebuilds byte-identically through the builders");
        Ok(())
    } else {
        Err("replay=FAIL".into())
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else { usage() };
    let outcome = match command.as_str() {
        "serve" => parse(args).and_then(dispatch),
        "emit" => match args.next() {
            Some(dir) => replay::emit(Path::new(&dir)),
            None => usage(),
        },
        "replay" => match args.next() {
            Some(dir) => run_replay(Path::new(&dir)),
            None => usage(),
        },
        "compiler-serve" => parse_compiler_serve(args).and_then(|options| {
            payoff_compiler::serve(
                options.port,
                options.statics,
                options.compiler_release_sha256,
            )
        }),
        "compile-payoff" => parse_compiler_release(args).and_then(payoff_compiler::compile_cli),
        "chain-serve" => parse_chain_serve(args).and_then(|options| {
            chain_server::serve(options.port, options.statics, &options.config)
        }),
        _ => usage(),
    };
    if let Err(error) = outcome {
        eprintln!("operatord: {error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod option_tests {
    use super::{dispatch, parse, toolchain, Options};

    #[test]
    fn serve_has_no_implicit_laboratory_mode() {
        let error = dispatch(Options::default()).unwrap_err().to_string();
        assert!(error.contains("requires an explicit non-production --mode"));
        assert!(error.contains("chain-serve --config FILE"));
    }

    #[test]
    fn parses_an_explicit_disjoint_validator_network() {
        let options = parse(
            [
                "--port",
                "9560",
                "--rpc-port",
                "9567",
                "--faucet-port",
                "9569",
                "--gossip-port",
                "9570",
                "--dynamic-port-range",
                "9571-9620",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert_eq!(options.gossip_port, 9570);
        assert_eq!(options.dynamic_port_range, "9571-9620");
        assert!(options.mode.is_empty());
        toolchain::validate_validator_network(
            Some(options.port),
            options.rpc_port,
            options.faucet_port,
            options.gossip_port,
            &options.dynamic_port_range,
        )
        .unwrap();
    }

    #[test]
    fn live_pyth_mode_is_explicit_and_does_not_imply_a_transcript() {
        let options = parse(
            [
                "--mode",
                "non-production-synthetic-source-v2-live",
                "--exit-when-done",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert_eq!(options.mode, "non-production-synthetic-source-v2-live");
        assert!(options.exit_when_done);
        assert!(options.transcript.is_none());
        assert!(options.work.is_none());
    }
}
