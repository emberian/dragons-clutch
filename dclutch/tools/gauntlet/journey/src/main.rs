//! JRNY: one campaign that lives a Market's whole life.
//!
//! The gauntlet's route census answered "does each route run at all." This
//! answers a different question: does a Market, founded the way a founder
//! founds one, survive being USED — distributed, custodied, resolved,
//! redeemed, retired — with every collateral atom accounted for at every step.
//!
//! It reaches: founded, distributed, traded around a ring, resolved through a
//! real Pyth publication verified by a real Wormhole router, and retiring with
//! its Source subtree closed. What it does not reach is one door — every Claims
//! mutation needs a program to sign its own PDA — and the gap register says so
//! route by route.
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
// These modules ARE `tools/local-validator/bootstrap/successor/src/`.
// They are not copies. A change to the founding reaches this campaign by
// recompilation, and a change that breaks this campaign breaks this build.
//
// `dead_code` is allowed on exactly these and nowhere else: the journey
// calls `found_through_open`, not `execute`, and never renders a demo-market
// spec, so a handful of the producer's own entry points are unreachable HERE
// while being live in the producer. Silencing it per-item would mean editing
// the producer to describe a consumer, which is backwards.
//
// `campaign` and `cluster` joined the list when the devnet driver landed. The
// journey does not drive an external cluster and never will -- it is
// loopback-only by construction, and its founder key is ephemeral. They are
// compiled here anyway, and deliberately: `cluster` is now the one owner of
// the origin rail that keeps this runner on 127.0.0.1, so a change that
// weakened that rail must break THIS build too, not only the producer's. That
// is the whole point of the `#[path]` arrangement.
#[path = "../../../local-validator/bootstrap/successor/src/campaign.rs"]
#[allow(dead_code)]
mod campaign;
#[path = "../../../local-validator/bootstrap/successor/src/cluster.rs"]
#[allow(dead_code)]
mod cluster;
#[path = "../../../local-validator/bootstrap/successor/src/direct_market.rs"]
#[allow(dead_code)]
mod direct_market;
#[path = "../../../local-validator/bootstrap/successor/src/local_mutable.rs"]
#[allow(dead_code)]
mod local_mutable;
#[path = "../../../local-validator/bootstrap/successor/src/market.rs"]
#[allow(dead_code)]
mod market;
#[path = "../../../local-validator/bootstrap/successor/src/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "../../../local-validator/bootstrap/successor/src/plan.rs"]
#[allow(dead_code)]
mod plan;
#[path = "../../../local-validator/bootstrap/successor/src/relayed.rs"]
#[allow(dead_code)]
mod relayed;
#[path = "../../../local-validator/bootstrap/successor/src/rpc.rs"]
#[allow(dead_code)]
mod rpc;
// SIX MODULES THE JOURNEY DOES NOT USE AND CANNOT BUILD WITHOUT.
//
// The journey founds one Direct market. It does not found a Structured, a
// General or a Rational one, does not select a capability, and does not drive
// the funding-readiness suffix — and it links all six anyway, because the
// files it DOES share have grown call sites into them: `market.rs` calls
// `crate::selected_capability::` and imports `crate::funding_readiness`,
// `local_mutable.rs` calls `crate::general_market::`, `crate::rational_market::`
// and `crate::structured_market::`, and `campaign.rs` calls
// `crate::release_identity::`.
//
// This is the `#[path]` tripwire in the header doing exactly what it says, and
// it went off silently: nothing runs this tier in CI, so the subset fell six
// modules and one crate behind the successor and the whole whole-life campaign
// was simply un-buildable until somebody tried to run it. The closure is
// finite — none of the six reaches outside the set now linked here — so the
// honest fix is to link them, not to fork the files or to guard the call sites.
#[path = "../../../local-validator/bootstrap/successor/src/funding_readiness.rs"]
#[allow(dead_code)]
mod funding_readiness;
#[path = "../../../local-validator/bootstrap/successor/src/general_market.rs"]
#[allow(dead_code)]
mod general_market;
#[path = "../../../local-validator/bootstrap/successor/src/rational_market.rs"]
#[allow(dead_code)]
mod rational_market;
#[path = "../../../local-validator/bootstrap/successor/src/release_identity.rs"]
#[allow(dead_code)]
mod release_identity;
#[path = "../../../local-validator/bootstrap/successor/src/selected_capability.rs"]
#[allow(dead_code)]
mod selected_capability;
#[path = "../../../local-validator/bootstrap/successor/src/structured_market.rs"]
#[allow(dead_code)]
mod structured_market;
#[path = "../../../local-validator/bootstrap/successor/src/runtime.rs"]
#[allow(dead_code)]
mod runtime;
#[path = "../../../local-validator/bootstrap/successor/src/seed.rs"]
#[allow(dead_code)]
mod seed;
#[path = "../../../local-validator/bootstrap/successor/src/upgrade.rs"]
#[allow(dead_code)]
mod upgrade;

// ------------------------------------------------------------- this campaign
mod journey;
mod ledger;
mod provider;
mod resolution;
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
        Some("demo-market") => run_demo_market(arguments.collect()),
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
    let mut keypair_seed = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--spec" => &mut spec,
            "--transcript" => &mut transcript,
            "--holders" => &mut holders,
            "--keypair-seed" => &mut keypair_seed,
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
        keypair_seed.as_deref(),
    )?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&transcript)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Refuse the retired standalone demo compiler.
///
/// Direct is deployment-bound now. A registry address alone cannot prove the
/// checked five-role release set, so this compatibility command must not mint
/// a fixture authority. The journey's `run` command instead accepts the full
/// market-bearing spec emitted by the checked local planner.
fn run_demo_market(_arguments: Vec<String>) -> Result<()> {
    Err(Error::new(
        "demo-market is retired: a standalone registry address cannot authenticate the checked \
         local Direct deployment. Supply a market compiled by \
         dclutch-local-successor-bootstrap local-private-validator-market-v1 to `run` instead",
    ))
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
        "Usage:\n  dclutch-journey-campaign run --spec ABSOLUTE_JSON --transcript ABSOLUTE_NEW_JSON [--holders N] [--keypair-seed 64_LOWERCASE_HEX]\n\nThe spec is a `dclutch-local-successor-run-spec-v2` document, exactly the one\nthe tier-1 bootstrap consumes: the journey reaches Open through that producer's\nown code, then keeps going on the same validator as the same in-memory founder.\nThe run-evidence document the census consumes is written to the spec's own\n`output` path and covers the WHOLE journey, founding transactions included.\n--transcript is the journey's own document: stages, gaps, and every\nconservation-ledger census.\n--holders is the load knob; it is the number of synthetic holders the founder\ndistributes collateral to. Default {default}.\n--keypair-seed is the producer's TEST-ONLY, LOOPBACK-ONLY determinism switch,\npassed straight through: it collapses the find_program_address bump-search\nnoise, which is what lets a conservation ledger's numbers be compared between\nruns at all. Read seed.rs before using it anywhere but here.\n\nNothing here signs with a persisted key, funds an external account, publishes,\nor deploys anywhere but a fresh localhost ledger on 127.0.0.1:20890.",
        default = journey::DEFAULT_HOLDER_COUNT
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn standalone_demo_market_refuses_to_invent_direct_authority() {
        let error = super::run_demo_market(vec![
            "--registry-program-id".into(),
            "11111111111111111111111111111111".into(),
        ])
        .expect_err("standalone compiler must refuse");
        assert!(
            error
                .0
                .contains("cannot authenticate the checked local Direct deployment")
        );
    }
}
