//! JRNY-1: one campaign that lives a Market's whole life.
//!
//! The gauntlet's route census answered "does each route run at all." This
//! answers a different question: does a Market, founded the way a founder
//! founds one, survive being USED — distributed, custodied, resolved,
//! redeemed, retired — with every collateral atom accounted for at every step.
//!
//! Stage 1 is not a reimplementation of the founding. The tier-1 producer's
//! own source files are compiled into this binary by `#[path]` and its
//! `runtime::found_through_open` is called directly, so the journey begins on
//! the same chain, under the same activation cache, as the same in-memory
//! founder. Everything after Open is this campaign's own.
#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, io::Write, path::PathBuf};

// ------------------------------------------------------------- tier-1, verbatim
//
// These five modules ARE `tools/local-validator/bootstrap/successor/src/`.
// They are not copies. A change to the founding reaches this campaign by
// recompilation, and a change that breaks this campaign breaks this build.
//
// `dead_code` is allowed on exactly these five and nowhere else: the journey
// calls `found_through_open`, not `execute`, and never renders a demo-market
// spec, so a handful of the producer's own entry points are unreachable HERE
// while being live in the producer. Silencing it per-item would mean editing
// the producer to describe a consumer, which is backwards.
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

// ------------------------------------------------------------- this campaign
mod journey;
mod ledger;
mod stages;

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
        Some("run") => run_journey(arguments.collect()),
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn run_journey(arguments: Vec<String>) -> Result<()> {
    let mut spec = None;
    let mut transcript = None;
    let mut holders = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--spec" => &mut spec,
            "--transcript" => &mut transcript,
            "--holders" => &mut holders,
            _ => return Err(Error::new(format!("unknown run argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let holders = match holders {
        None => journey::DEFAULT_HOLDER_COUNT,
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| Error::new("--holders must be a decimal count"))?,
    };
    let transcript = journey::execute(
        &absolute(spec, "--spec")?,
        &absolute(transcript, "--transcript")?,
        holders,
    )?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&transcript)?)?;
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
        "Usage:\n  dclutch-journey-campaign run --spec ABSOLUTE_JSON --transcript ABSOLUTE_NEW_JSON [--holders N]\n\nThe spec is a `dclutch-local-successor-run-spec-v2` document, exactly the one\nthe tier-1 bootstrap consumes: the journey reaches Open through that producer's\nown code, then keeps going on the same validator as the same in-memory founder.\nThe run-evidence document the census consumes is written to the spec's own\n`output` path and covers the WHOLE journey, founding transactions included.\n--transcript is the journey's own document: stages, gaps, and every\nconservation-ledger census.\n--holders is the load knob; it is the number of synthetic holders the founder\ndistributes collateral to. Default {default}.\n\nNothing here signs with a persisted key, funds an external account, publishes,\nor deploys anywhere but a fresh localhost ledger on 127.0.0.1:20890.",
        default = journey::DEFAULT_HOLDER_COUNT
    );
}
