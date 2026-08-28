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

use std::{env, error::Error as StdError, fmt};

// ------------------------------------------------------------- tier-1, verbatim
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
#[path = "../../../local-validator/bootstrap/successor/src/runtime.rs"]
#[allow(dead_code)]
mod runtime;
#[path = "../../../local-validator/bootstrap/successor/src/seed.rs"]
#[allow(dead_code)]
mod seed;
#[path = "../../../local-validator/bootstrap/successor/src/upgrade.rs"]
#[allow(dead_code)]
mod upgrade;

// ------------------------------------------------------- the journey's ledger
#[path = "../../journey/src/ledger.rs"]
#[allow(dead_code)]
mod ledger;

// ------------------------------------------------------------- this campaign
#[allow(dead_code)]
mod daemon;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod relayworld;
#[allow(dead_code)]
mod twin;
#[allow(dead_code)]
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

fn run_vertical(_arguments: Vec<String>) -> Result<()> {
    Err(Error::new(
        "relayed-vertical is parked: Direct is deployment-bound, but this runner compiles its \
         replacement market before a checked local mutable plan and live loopback substrate \
         exist. Restore it only as prepare-mutable -> authenticate live substrate -> compile \
         market -> found -> relay; fixture Direct identities are refused",
    ))
}

fn usage() {
    println!(
        "PARKED: this command refuses before reading files or starting a validator. Direct is \
         deployment-bound now; restore this runner only as prepare checked substrate, authenticate \
         live loopback accounts, compile with DirectMarketCompilerOwnedV1::load_local, then found \
         and relay. Do not substitute fixture identities.\n\nHistorical interface (not executable):"
    );
    println!(
        "Usage:\n  dclutch-relayed-vertical-campaign run --walk success|failure \\\n      --spec-template ABSOLUTE_JSON --transcript ABSOLUTE_NEW_JSON \\\n      --relayer-bin ABSOLUTE_DCLUTCH_RELAYER --work ABSOLUTE_DIR \\\n      [--keypair-seed 64_LOWERCASE_HEX]\n\nThe spec template is a `dclutch-local-successor-run-spec-v2` document whose\n`market` field this campaign REPLACES with the relayed graduation market it\ncompiles at run time (the market's terminal window is wall-clock content and\nthe relayer key set is generated per run, so the input cannot be static).\nEverything runs on 127.0.0.1: the successor validator on the template's own\nport block, and the mainnet twin on a port this campaign binds under --work.\nNothing here signs with a persisted key or touches a public cluster."
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn relayed_vertical_refuses_pre_plan_direct_compilation() {
        let error = super::run_vertical(Vec::new()).expect_err("parked vertical must refuse");
        assert!(error.0.contains("before a checked local mutable plan"));
        assert!(error.0.contains("fixture Direct identities are refused"));
    }
}
