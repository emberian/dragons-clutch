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
#[path = "../../../local-validator/bootstrap/successor/src/chaos_fault.rs"]
#[allow(dead_code)]
mod chaos_fault;
#[path = "../../../local-validator/bootstrap/successor/src/cluster.rs"]
#[allow(dead_code)]
mod cluster;
#[path = "../../../local-validator/bootstrap/successor/src/collateral_release.rs"]
#[allow(dead_code)]
mod collateral_release;
#[path = "../../../local-validator/bootstrap/successor/src/direct_market.rs"]
#[allow(dead_code)]
mod direct_market;
#[path = "../../../local-validator/bootstrap/successor/src/funding_readiness.rs"]
#[allow(dead_code)]
mod funding_readiness;
#[path = "../../../local-validator/bootstrap/successor/src/general_market.rs"]
#[allow(dead_code)]
mod general_market;
#[path = "../../../local-validator/bootstrap/successor/src/infrastructure_succession.rs"]
#[allow(dead_code)]
mod infrastructure_succession;
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
#[path = "../../../local-validator/bootstrap/successor/src/rational_market.rs"]
#[allow(dead_code)]
mod rational_market;
#[path = "../../../local-validator/bootstrap/successor/src/relayed.rs"]
#[allow(dead_code)]
mod relayed;
#[path = "../../../local-validator/bootstrap/successor/src/release_identity.rs"]
#[allow(dead_code)]
mod release_identity;
#[path = "../../../local-validator/bootstrap/successor/src/rpc.rs"]
#[allow(dead_code)]
mod rpc;
// `local_mutable.rs` grew a fourth capability branch and calls
// `crate::structured_market` from it. This tier compiles the producer's files
// verbatim rather than forking them, so a call site the subset does not link
// is a build break -- the intended tripwire, and it went off silently because
// nothing in CI builds this tier. Linking the module is the fix; guarding the
// call site would fork a file whose whole point is that it is not forked.
#[path = "../../../local-validator/bootstrap/successor/src/structured_market.rs"]
#[allow(dead_code)]
mod structured_market;
#[path = "../../../local-validator/bootstrap/successor/src/selected_capability.rs"]
#[allow(dead_code)]
mod selected_capability;
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
mod substrate;
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

fn required<'a>(
    values: &'a std::collections::BTreeMap<String, String>,
    flag: &str,
) -> Result<&'a str> {
    values
        .get(flag)
        .map(String::as_str)
        .ok_or_else(|| Error::new(format!("{flag} is required")))
}

fn absolute(value: &str, flag: &str) -> Result<std::path::PathBuf> {
    let path = std::path::PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be an absolute path")));
    }
    Ok(path)
}

/// The restored vertical, in exactly the order the park banner prescribed:
/// prepare-mutable -> authenticate live substrate -> compile market -> found
/// -> relay. Fixture Direct identities remain refused: the market compiler is
/// `DirectMarketCompilerOwnedV1::load_local`, which authenticates the checked
/// local mutable plan against the gate on disk and observes the LIVE loopback
/// deployment before anything compiles.
fn run_vertical(arguments: Vec<String>) -> Result<()> {
    let mut values = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} needs a value")))?;
        if values.insert(flag.clone(), value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let walk = match required(&values, "--walk")? {
        "success" => vertical::WalkV1::Success,
        "failure" => vertical::WalkV1::Failure,
        other => return Err(Error::new(format!("--walk must be success or failure: {other}"))),
    };
    let rpc_port: u16 = required(&values, "--rpc-port")?
        .parse()
        .map_err(|_| Error::new("--rpc-port must be a port number"))?;
    let request = vertical::VerticalRequestV1 {
        walk,
        transcript: absolute(required(&values, "--transcript")?, "--transcript")?,
        relayer_bin: absolute(required(&values, "--relayer-bin")?, "--relayer-bin")?,
        work: absolute(required(&values, "--work")?, "--work")?,
        rpc_port,
        checked_release_gate: absolute(
            required(&values, "--checked-release-gate")?,
            "--checked-release-gate",
        )?,
        expected_gate_sha256: required(&values, "--expected-gate-sha256")?.to_owned(),
        expected_source_revision: required(&values, "--expected-source-revision")?.to_owned(),
        expected_source_tree_sha256: required(&values, "--expected-source-tree-sha256")?.to_owned(),
        seed: required(&values, "--seed")?.to_owned(),
    };
    vertical::execute(request).map(|_| ())
}

fn usage() {
    println!(
        "Usage:\n  dclutch-relayed-vertical-campaign run --walk success|failure \\\n      \
         --transcript ABSOLUTE_NEW_JSON --relayer-bin ABSOLUTE_DCLUTCH_RELAYER \\\n      \
         --work ABSOLUTE_DIR --rpc-port PORT \\\n      \
         --checked-release-gate ABSOLUTE_CHECKED_UPGRADE_GATE_JSON \\\n      \
         --expected-gate-sha256 HEX64 --expected-source-revision HEX40 \\\n      \
         --expected-source-tree-sha256 HEX64 --seed HEX64\n\nThe campaign brings up its own \
         checked-mutable loopback substrate from the named\nchecked release gate \
         (local-mutable-prepare-v1), boots a fresh solana-test-validator\nover the prepared \
         account directory, runs the administration campaign through\nactivation, compiles the \
         relayed graduation market against the LIVE deployment\n(load_local; fixture Direct \
         identities are refused), founds it with\ncampaign --founding-only, and only then \
         relays. Everything runs on 127.0.0.1:\nthe successor validator on --rpc-port and the \
         mainnet twin on a port this\ncampaign binds under --work. The prepared keys are \
         disposable loopback-only\nfiles under --work; nothing here touches a public cluster."
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_vertical_refuses_missing_and_relative_arguments_before_any_work() {
        let error = super::run_vertical(Vec::new()).expect_err("no arguments must refuse");
        assert!(error.0.contains("--walk is required"));
        let error = super::run_vertical(vec![
            "--walk".into(),
            "failure".into(),
            "--rpc-port".into(),
            "21000".into(),
            "--transcript".into(),
            "relative.json".into(),
        ])
        .expect_err("a relative transcript must refuse");
        assert!(error.0.contains("--transcript must be an absolute path"));
    }
}
