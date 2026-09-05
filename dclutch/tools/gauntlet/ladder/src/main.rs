//! LADDER: a market's funded ordered recovery ladder, on ONE live validator.
//!
//! Decision 0027 built the funded ordered ladder, `6a3079454` taught the
//! successor's market compiler to found a market that buys one, and
//! `61706bc9a` gave the permissionless crank its driver. None of the three had
//! ever met a chain together, because the sequence they form -- found, crank,
//! answer or exhaust -- is three commands against ONE live cluster, and tier 1
//! founds and resolves inside a single process whose `runtime::found_through_open`
//! owns the validator child.
//!
//! This tier is that cluster. It brings up the checked-mutable substrate the
//! relayed vertical brought up first -- prepare, spawn a `solana-test-validator`
//! over the prepared account directory, administer through activation -- and
//! then keeps the child alive while it compiles a TWO-SOURCE market, founds it,
//! and drives the crank against it.
//!
//! Nothing here warps a clock. The crank's admissibility is a fact about the
//! market's own published `WindowSpecV1` and `RecoveryPolicyV2` read against
//! the cluster's own clock, and this campaign reports what it found rather than
//! editing the chain until the answer is the one it wanted.
#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt};

// ------------------------------------------------------------- tier-1, verbatim
//
// These modules ARE `tools/local-validator/bootstrap/successor/src/`. They are
// not copies. The set is the journey's, plus the four `recovery_crank` needs.
// If it goes red, EXTEND it -- do not fork a file.
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
#[path = "../../../local-validator/bootstrap/successor/src/core_bump_projection.rs"]
#[allow(dead_code)]
mod core_bump_projection;
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
#[path = "../../../local-validator/bootstrap/successor/src/release_capture.rs"]
#[allow(dead_code)]
mod release_capture;
#[path = "../../../local-validator/bootstrap/successor/src/rpc.rs"]
#[allow(dead_code)]
mod rpc;
#[path = "../../../local-validator/bootstrap/successor/src/runtime.rs"]
#[allow(dead_code)]
mod runtime;
#[path = "../../../local-validator/bootstrap/successor/src/seed.rs"]
#[allow(dead_code)]
mod seed;
#[path = "../../../local-validator/bootstrap/successor/src/selected_capability.rs"]
#[allow(dead_code)]
mod selected_capability;
#[path = "../../../local-validator/bootstrap/successor/src/structured_market.rs"]
#[allow(dead_code)]
mod structured_market;
#[path = "../../../local-validator/bootstrap/successor/src/upgrade.rs"]
#[allow(dead_code)]
mod upgrade;

// ------------------------------------------------------ the crank, verbatim
//
// THE POINT OF THIS TIER. `recovery_crank` is the shipped
// `local-private-validator-advance-recovery-v1` driver, and this campaign calls
// its own entry point with an argument vector rather than reimplementing the
// frame -- a tier that built its own 18-account frame would be measuring a
// second author, not the driver a host runs.
#[path = "../../../local-validator/bootstrap/successor/src/recovery_crank.rs"]
#[allow(dead_code)]
mod recovery_crank;
#[path = "../../../local-validator/bootstrap/successor/src/sponsored_schedule.rs"]
#[allow(dead_code)]
mod sponsored_schedule;
#[path = "../../../local-validator/bootstrap/successor/src/terminal_lifecycle.rs"]
#[allow(dead_code)]
mod terminal_lifecycle;
#[path = "../../../local-validator/bootstrap/successor/src/wallet_terminal.rs"]
#[allow(dead_code)]
mod wallet_terminal;

// ------------------------------------------- the relayed vertical's substrate
//
// The one bring-up in this tree that leaves a validator RUNNING for a caller
// to drive more than one command against. Linked, not forked: a change to how
// the checked-mutable substrate is stood up must break this tier too.
#[path = "../../relayed-vertical/src/substrate.rs"]
#[allow(dead_code)]
mod substrate;

// ------------------------------------------------------------- this campaign
mod ladder;

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

/// `terminal_lifecycle` and `wallet_terminal` propagate the payout operator's
/// error with `?`, so this binary owes the same conversion the producer's own
/// `main.rs` declares. Linking a module means owing its error boundary too.
impl From<dclutch_operator::wallet_terminal_payout::Error> for Error {
    fn from(error: dclutch_operator::wallet_terminal_payout::Error) -> Self {
        Self(error.to_string())
    }
}

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
        Some("run") => run_ladder(arguments.collect()),
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

fn run_ladder(arguments: Vec<String>) -> Result<()> {
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
        "exhaust" => ladder::WalkV1::Exhaust,
        "capture" => ladder::WalkV1::Capture,
        other => {
            return Err(Error::new(format!(
                "--walk must be exhaust or capture: {other}"
            )));
        }
    };
    let rpc_port: u16 = required(&values, "--rpc-port")?
        .parse()
        .map_err(|_| Error::new("--rpc-port must be a port number"))?;
    // The rung the market buys, in the SHIPPED command's own spelling. This
    // tier does not invent a second syntax for a ladder: it hands the string a
    // host would type to `local_mutable::parse_recovery_rungs_v1`, so a change
    // to what `--recovery-rungs` means changes what this tier founds.
    let request = ladder::LadderRequestV1 {
        walk,
        transcript: absolute(required(&values, "--transcript")?, "--transcript")?,
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
        recovery_rungs: values
            .get("--recovery-rungs")
            .cloned()
            .unwrap_or_else(|| ladder::DEFAULT_RECOVERY_RUNGS_V1.to_owned()),
        max_wait_seconds: match values.get("--max-wait-seconds") {
            None => ladder::DEFAULT_MAX_WAIT_SECONDS_V1,
            Some(raw) => raw
                .parse()
                .map_err(|_| Error::new("--max-wait-seconds must be a decimal i64"))?,
        },
    };
    ladder::execute(request).map(|_| ())
}

fn usage() {
    println!(
        "Usage:\n  dclutch-ladder-campaign run --walk exhaust|capture \\\n      \
         --transcript ABSOLUTE_NEW_JSON --work ABSOLUTE_DIR --rpc-port PORT \\\n      \
         --checked-release-gate ABSOLUTE_CHECKED_UPGRADE_GATE_JSON \\\n      \
         --expected-gate-sha256 HEX64 --expected-source-revision HEX40 \\\n      \
         --expected-source-tree-sha256 HEX64 --seed HEX64 \\\n      \
         [--recovery-rungs BPS:SECONDS_AFTER_PREVIOUS] [--max-wait-seconds I64]\n\nThe campaign \
         brings up its own checked-mutable loopback substrate from the named\nchecked release \
         gate (local-mutable-prepare-v1), boots a fresh solana-test-validator\nover the prepared \
         account directory, administers it through activation, compiles a\nTWO-SOURCE market \
         against the LIVE deployment, founds it, and then drives the\nshipped advance-recovery \
         crank against the market it founded. Everything runs on\n127.0.0.1 and nothing here \
         touches a public cluster. No clock is warped: the\ncrank's admissibility is read off \
         the market's own records against the cluster's\nown clock."
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_ladder_refuses_missing_and_relative_arguments_before_any_work() {
        let error = super::run_ladder(Vec::new()).expect_err("no arguments must refuse");
        assert!(error.0.contains("--walk is required"), "{}", error.0);
        let error = super::run_ladder(vec![
            "--walk".into(),
            "exhaust".into(),
            "--rpc-port".into(),
            "21000".into(),
            "--transcript".into(),
            "relative.json".into(),
        ])
        .expect_err("a relative transcript must refuse");
        assert!(
            error.0.contains("--transcript must be an absolute path"),
            "{}",
            error.0
        );
    }

    #[test]
    fn the_default_rung_is_the_shipped_flag_spelling() {
        let rungs = super::local_mutable::parse_recovery_rungs_v1(
            super::ladder::DEFAULT_RECOVERY_RUNGS_V1,
        )
        .expect("the tier's default rung must parse as the shipped --recovery-rungs value");
        assert_eq!(rungs.len(), 1, "the tier founds a TWO-source market");
    }
}
