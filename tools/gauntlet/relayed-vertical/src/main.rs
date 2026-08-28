//! DEMO-VERT: the relayed graduation market, end to end, on a local rehearsal.
//!
//! One journey-shaped campaign for `MAINNET_STATE_RELAY.md`'s
//! `RelayedMainnetStateV1` family, against TWO local validators:
//!
//! - a **mainnet twin**: a stock `solana-test-validator` carrying nothing but
//!   the synthetic-of-real Meteora DBC world — the venue Program and
//!   ProgramData accounts at their real mainnet addresses with a synthetic ELF
//!   tail, and one graduated `VirtualPool` at its real byte layout. The layout
//!   is real; the values are invented; every artifact says so.
//! - the **successor validator**: the tier-1 producer's own founding campaign
//!   (`found_through_open`, compiled in by `#[path]`), in transaction-only
//!   record publication, founding a zero-cut graduation Product whose
//!   240-byte `SourceMaterialV3` carries NO recovery policy — the §12.8 demo shape the
//!   no-recovery admission (`e5b6923`) made foundable.
//!
//! The success walk drives the REAL `dclutch-relayer` daemon binary: observe
//! the twin (dry run, rehearsal-twin labelled), create the slot-seeded
//! observation record, re-submit the recorded attestations (`submit-artifacts`
//! — append ×4 and seal, the full-body append over the Market's address lookup
//! table), then consume the sealed record as a packet-safe v0 transaction and
//! read the `ResolutionSuccess` certificate back. The failure sibling runs the
//! same market with the daemon never started: past `end + max_age` the funded
//! deadline walk pays a walker on a bare legacy transaction.
//!
//! The journey's conservation ledger is threaded through every stage boundary.
#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, io::Write, path::PathBuf};

// ------------------------------------------------------------- tier-1, verbatim
#[path = "../../../local-validator/bootstrap/successor/src/cluster.rs"]
#[allow(dead_code)]
mod cluster;
#[path = "../../../local-validator/bootstrap/successor/src/market.rs"]
#[allow(dead_code)]
mod market;
#[path = "../../../local-validator/bootstrap/successor/src/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "../../../local-validator/bootstrap/successor/src/plan.rs"]
#[allow(dead_code)]
mod plan;
#[path = "../../../local-validator/bootstrap/successor/src/rpc.rs"]
#[allow(dead_code)]
mod rpc;
#[path = "../../../local-validator/bootstrap/successor/src/runtime.rs"]
#[allow(dead_code)]
mod runtime;
#[path = "../../../local-validator/bootstrap/successor/src/seed.rs"]
#[allow(dead_code)]
mod seed;

// ------------------------------------------------------- the journey's ledger
#[path = "../../journey/src/ledger.rs"]
#[allow(dead_code)]
mod ledger;

// ------------------------------------------------------------- this campaign
mod daemon;
mod input;
mod relayworld;
mod twin;
mod vertical;

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self::new(error.to_string())
    }
}

fn main() -> core::result::Result<(), Box<dyn StdError>> {
    match run() {
        Ok(()) => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

fn run() -> Result<()> {
    let mut arguments = env::args();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some("run") => run_vertical(arguments.collect()),
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn run_vertical(arguments: Vec<String>) -> Result<()> {
    let mut walk = None;
    let mut spec_template = None;
    let mut transcript = None;
    let mut relayer_bin = None;
    let mut work = None;
    let mut keypair_seed = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--walk" => &mut walk,
            "--spec-template" => &mut spec_template,
            "--transcript" => &mut transcript,
            "--relayer-bin" => &mut relayer_bin,
            "--work" => &mut work,
            "--keypair-seed" => &mut keypair_seed,
            _ => return Err(Error::new(format!("unknown run argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let walk = match walk.as_deref() {
        Some("success") => vertical::WalkV1::Success,
        Some("failure") => vertical::WalkV1::Failure,
        other => {
            return Err(Error::new(format!(
                "--walk must be success or failure, found {other:?}"
            )));
        }
    };
    let transcript_value = vertical::execute(vertical::VerticalRequestV1 {
        walk,
        spec_template: absolute(spec_template, "--spec-template")?,
        transcript: absolute(transcript, "--transcript")?,
        relayer_bin: absolute(relayer_bin, "--relayer-bin")?,
        work: absolute(work, "--work")?,
        keypair_seed,
    })?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&transcript_value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required(value, label)?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn usage() {
    println!(
        "Usage:\n  dclutch-relayed-vertical-campaign run --walk success|failure \\\n      --spec-template ABSOLUTE_JSON --transcript ABSOLUTE_NEW_JSON \\\n      --relayer-bin ABSOLUTE_DCLUTCH_RELAYER --work ABSOLUTE_DIR \\\n      [--keypair-seed 64_LOWERCASE_HEX]\n\nThe spec template is a `dclutch-local-successor-run-spec-v2` document whose\n`market` field this campaign REPLACES with the relayed graduation market it\ncompiles at run time (the market's terminal window is wall-clock content and\nthe relayer key set is generated per run, so the input cannot be static).\nEverything runs on 127.0.0.1: the successor validator on the template's own\nport block, and the mainnet twin on a port this campaign binds under --work.\nNothing here signs with a persisted key or touches a public cluster."
    );
}
