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

// ------------------------------------------------- the successor, verbatim
//
// These modules ARE `tools/local-validator/bootstrap/successor/src/`. They are
// not copies. A change to the founding, to a driver, or to the checked-mutable
// substrate reaches this campaign by recompilation, and a change that breaks
// this campaign breaks this build.
//
// THE SET IS NOW THE WHOLE OF IT, and that is a repair rather than a flourish.
// A CURATED subset was the arrangement until 2026-09-06, and its tripwire went
// off four times in six days -- twice silently, once for a whole day, once
// inside the hour a lane had linked the previous miss. Every one of those was
// the same accident: a producer grew a call site into a module this list did
// not name, and nothing that runs in CI builds this tier. The set below is
// generated from the successor's own `src/` directory, `main.rs` excepted, so
// there is no list to keep in sync at all; a file the successor adds is linked
// the moment this file is regenerated, and a file it deletes reds this build
// immediately. `dead_code` is allowed on all of them and nowhere else: the
// journey calls a handful of these entry points and never renders a demo-market
// spec, so most of the producer's surface is unreachable HERE while being live
// in the producer.
//
// TWO NAMES ARE NOT IN THE GENERATED SET, and both for the same reason: the
// successor's own `main.rs` declares them with a `#[path]` of its own.
// `founding_submission_journal` and `owned_loopback_capture` are submodules of
// `market.rs` and `terminal_exterior_pyth.rs`, so declaring them again here
// would put one file in two module positions; and `ledger` is THIS TIER'S
// conservation ledger, which the successor links back the other way -- the two
// crates share one file, and this file is its home.
//
// `cluster` is the one owner of the origin rail that keeps this runner on
// 127.0.0.1, so a change that weakened that rail must break THIS build too.
#[path = "../../../local-validator/bootstrap/successor/src/aggregate_retirement_exterior.rs"]
#[allow(dead_code)]
mod aggregate_retirement_exterior;
#[path = "../../../local-validator/bootstrap/successor/src/aggregate_retirement_journal.rs"]
#[allow(dead_code)]
mod aggregate_retirement_journal;
#[path = "../../../local-validator/bootstrap/successor/src/campaign.rs"]
#[allow(dead_code)]
mod campaign;
#[path = "../../../local-validator/bootstrap/successor/src/capability_seal_close.rs"]
#[allow(dead_code)]
mod capability_seal_close;
#[path = "../../../local-validator/bootstrap/successor/src/capability_seal_devnet.rs"]
#[allow(dead_code)]
mod capability_seal_devnet;
#[path = "../../../local-validator/bootstrap/successor/src/chaos_fault.rs"]
#[allow(dead_code)]
mod chaos_fault;
#[path = "../../../local-validator/bootstrap/successor/src/claims_custody_replay.rs"]
#[allow(dead_code)]
mod claims_custody_replay;
#[path = "../../../local-validator/bootstrap/successor/src/closure_receipt_projection.rs"]
#[allow(dead_code)]
mod closure_receipt_projection;
#[path = "../../../local-validator/bootstrap/successor/src/cluster.rs"]
#[allow(dead_code)]
mod cluster;
#[path = "../../../local-validator/bootstrap/successor/src/collateral_release.rs"]
#[allow(dead_code)]
mod collateral_release;
#[path = "../../../local-validator/bootstrap/successor/src/core_bump_projection.rs"]
#[allow(dead_code)]
mod core_bump_projection;
#[path = "../../../local-validator/bootstrap/successor/src/direct_capability_activation.rs"]
#[allow(dead_code)]
mod direct_capability_activation;
#[path = "../../../local-validator/bootstrap/successor/src/direct_close_maker.rs"]
#[allow(dead_code)]
mod direct_close_maker;
#[path = "../../../local-validator/bootstrap/successor/src/direct_fee_settlement.rs"]
#[allow(dead_code)]
mod direct_fee_settlement;
#[path = "../../../local-validator/bootstrap/successor/src/direct_hot_route_manifest.rs"]
#[allow(dead_code)]
mod direct_hot_route_manifest;
#[path = "../../../local-validator/bootstrap/successor/src/direct_market.rs"]
#[allow(dead_code)]
mod direct_market;
#[path = "../../../local-validator/bootstrap/successor/src/direct_resolution_campaign.rs"]
#[allow(dead_code)]
mod direct_resolution_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/direct_terminal_children.rs"]
#[allow(dead_code)]
mod direct_terminal_children;
#[path = "../../../local-validator/bootstrap/successor/src/direct_ticket.rs"]
#[allow(dead_code)]
mod direct_ticket;
#[path = "../../../local-validator/bootstrap/successor/src/direct_trade.rs"]
#[allow(dead_code)]
mod direct_trade;
#[path = "../../../local-validator/bootstrap/successor/src/direct_trade_producer.rs"]
#[allow(dead_code)]
mod direct_trade_producer;
#[path = "../../../local-validator/bootstrap/successor/src/direct_trade_setup.rs"]
#[allow(dead_code)]
mod direct_trade_setup;
#[path = "../../../local-validator/bootstrap/successor/src/direct_trade_setup_journal.rs"]
#[allow(dead_code)]
mod direct_trade_setup_journal;
#[path = "../../../local-validator/bootstrap/successor/src/direct_trade_token_setup.rs"]
#[allow(dead_code)]
mod direct_trade_token_setup;
#[path = "../../../local-validator/bootstrap/successor/src/evidence_refresh.rs"]
#[allow(dead_code)]
mod evidence_refresh;
#[path = "../../../local-validator/bootstrap/successor/src/family_hot_campaign.rs"]
#[allow(dead_code)]
mod family_hot_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/flagship_resolution.rs"]
#[allow(dead_code)]
mod flagship_resolution;
#[path = "../../../local-validator/bootstrap/successor/src/fractional_market.rs"]
#[allow(dead_code)]
mod fractional_market;
#[path = "../../../local-validator/bootstrap/successor/src/funding_readiness.rs"]
#[allow(dead_code)]
mod funding_readiness;
#[path = "../../../local-validator/bootstrap/successor/src/general_capability_activation.rs"]
#[allow(dead_code)]
mod general_capability_activation;
#[path = "../../../local-validator/bootstrap/successor/src/general_devnet_market.rs"]
#[allow(dead_code)]
mod general_devnet_market;
#[path = "../../../local-validator/bootstrap/successor/src/general_market.rs"]
#[allow(dead_code)]
mod general_market;
#[path = "../../../local-validator/bootstrap/successor/src/general_session.rs"]
#[allow(dead_code)]
mod general_session;
#[path = "../../../local-validator/bootstrap/successor/src/general_settlement_fixture.rs"]
#[allow(dead_code)]
mod general_settlement_fixture;
#[path = "../../../local-validator/bootstrap/successor/src/general_successor_plan.rs"]
#[allow(dead_code)]
mod general_successor_plan;
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
#[path = "../../../local-validator/bootstrap/successor/src/pyth_vaa_provisioning.rs"]
#[allow(dead_code)]
mod pyth_vaa_provisioning;
#[path = "../../../local-validator/bootstrap/successor/src/rational_market.rs"]
#[allow(dead_code)]
mod rational_market;
#[path = "../../../local-validator/bootstrap/successor/src/recovery_crank.rs"]
#[allow(dead_code)]
mod recovery_crank;
#[path = "../../../local-validator/bootstrap/successor/src/relayed.rs"]
#[allow(dead_code)]
mod relayed;
#[path = "../../../local-validator/bootstrap/successor/src/release_capture.rs"]
#[allow(dead_code)]
mod release_capture;
#[path = "../../../local-validator/bootstrap/successor/src/release_identity.rs"]
#[allow(dead_code)]
mod release_identity;
#[path = "../../../local-validator/bootstrap/successor/src/release_lineage.rs"]
#[allow(dead_code)]
mod release_lineage;
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
#[path = "../../../local-validator/bootstrap/successor/src/series_consume_campaign.rs"]
#[allow(dead_code)]
mod series_consume_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/series_lifecycle_campaign.rs"]
#[allow(dead_code)]
mod series_lifecycle_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/series_permit_expiry_campaign.rs"]
#[allow(dead_code)]
mod series_permit_expiry_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/series_terminal_campaign.rs"]
#[allow(dead_code)]
mod series_terminal_campaign;
#[path = "../../../local-validator/bootstrap/successor/src/source_abort_exterior.rs"]
#[allow(dead_code)]
mod source_abort_exterior;
#[path = "../../../local-validator/bootstrap/successor/src/spline_product.rs"]
#[allow(dead_code)]
mod spline_product;
#[path = "../../../local-validator/bootstrap/successor/src/sponsored_push.rs"]
#[allow(dead_code)]
mod sponsored_push;
#[path = "../../../local-validator/bootstrap/successor/src/sponsored_release_observation.rs"]
#[allow(dead_code)]
mod sponsored_release_observation;
#[path = "../../../local-validator/bootstrap/successor/src/sponsored_schedule.rs"]
#[allow(dead_code)]
mod sponsored_schedule;
#[path = "../../../local-validator/bootstrap/successor/src/structured_market.rs"]
#[allow(dead_code)]
mod structured_market;
#[path = "../../../local-validator/bootstrap/successor/src/terminal_exterior_pyth.rs"]
#[allow(dead_code)]
mod terminal_exterior_pyth;
#[path = "../../../local-validator/bootstrap/successor/src/terminal_lifecycle.rs"]
#[allow(dead_code)]
mod terminal_lifecycle;
#[path = "../../../local-validator/bootstrap/successor/src/terminal_sequence.rs"]
#[allow(dead_code)]
mod terminal_sequence;
#[path = "../../../local-validator/bootstrap/successor/src/upgrade.rs"]
#[allow(dead_code)]
mod upgrade;
#[path = "../../../local-validator/bootstrap/successor/src/user_position_admission.rs"]
#[allow(dead_code)]
mod user_position_admission;
#[path = "../../../local-validator/bootstrap/successor/src/user_position_close.rs"]
#[allow(dead_code)]
mod user_position_close;
#[path = "../../../local-validator/bootstrap/successor/src/wallet_terminal.rs"]
#[allow(dead_code)]
mod wallet_terminal;
#[path = "../../../local-validator/bootstrap/successor/src/wallet_terminal_payout_exterior.rs"]
#[allow(dead_code)]
mod wallet_terminal_payout_exterior;

// ------------------------------------------- the relayed vertical's substrate
//
// The one bring-up in this tree that leaves a validator RUNNING for a caller to
// drive more than one command against, and the reason this tier can stand up
// its own substrate at all. Linked, not forked -- the ladder links the same
// file -- so a change to how the checked-mutable substrate is stood up must
// break this tier too.
#[path = "../../relayed-vertical/src/substrate.rs"]
#[allow(dead_code)]
mod substrate;

// ------------------------------------------------------------- this campaign
mod journey;
mod ledger;
mod provider;
mod resolution;
mod spine;
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

/// Several linked producer modules propagate the payout operator's error with
/// `?`, so this binary owes the same conversion the producer's own `main.rs`
/// declares. Linking a module means owing its error boundary too.
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

/// The successor's own crate-root argument helpers.
///
/// `sponsored_push.rs` imports `crate::absolute`, so a binary that links that
/// module owes both functions the way it owes the error conversions above.
/// They are the producer's, byte for byte.
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

/// The successor's own crate-root stdout helper.
///
/// `spline_product.rs` imports `crate::stdout_json_value_v1`, so a binary that
/// links that module owes the function the way it owes the error conversions
/// above. It is the producer's, byte for byte
/// (`tools/local-validator/bootstrap/successor/src/main.rs`): a linked module
/// is entitled to the crate root its author wrote it against.
fn stdout_json_value_v1(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    Ok(())
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
    let required = |flag: &str| -> Result<String> {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| Error::new(format!("{flag} is required")))
    };
    let absolute = |value: String, flag: &str| -> Result<PathBuf> {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err(Error::new(format!("{flag} must be an absolute path")));
        }
        Ok(path)
    };
    let holder_count = match values.get("--holders") {
        None => journey::DEFAULT_HOLDER_COUNT,
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| Error::new("--holders must be a decimal count"))?,
    };
    let request = journey::JourneyRequestV1 {
        transcript: absolute(required("--transcript")?, "--transcript")?,
        work: absolute(required("--work")?, "--work")?,
        rpc_port: required("--rpc-port")?
            .parse()
            .map_err(|_| Error::new("--rpc-port must be a port number"))?,
        checked_release_gate: absolute(
            required("--checked-release-gate")?,
            "--checked-release-gate",
        )?,
        expected_gate_sha256: required("--expected-gate-sha256")?,
        expected_source_revision: required("--expected-source-revision")?,
        expected_source_tree_sha256: required("--expected-source-tree-sha256")?,
        seed: required("--seed")?,
        holder_count,
    };
    let transcript = journey::execute(request)?;
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

fn usage() {
    println!(
        "Usage:\n  dclutch-journey-campaign run \\\n      \
         --transcript ABSOLUTE_NEW_JSON --work ABSOLUTE_DIR --rpc-port PORT \\\n      \
         --checked-release-gate ABSOLUTE_CHECKED_UPGRADE_GATE_JSON \\\n      \
         --expected-gate-sha256 HEX64 --expected-source-revision HEX40 \\\n      \
         --expected-source-tree-sha256 HEX64 --seed HEX64 [--holders N]\n\nThe campaign brings up \
         its own checked-mutable loopback substrate from the named checked\nrelease gate \
         (local-mutable-prepare-v1), boots a fresh solana-test-validator over\nthe prepared \
         account directory, administers it through activation, compiles a\nMarket against the LIVE \
         deployment, founds it, and then lives that Market's whole\nlife against the validator it \
         is still holding: distribution, a holder ring, two\nstrangers admitted, a Direct Hot \
         fill, the fee settlement, the Resolution funding\nladder, the Pyth transport to Terminal, \
         a wallet-signed redemption, CloseFund and\nBeginRetiring, and the four checkpointed \
         retirement packets -- every one of them\nthrough the SHIPPED command a host would run, \
         under one conservation ledger.\n\n--holders is the load knob; it is the number of \
         synthetic holders the founder\ndistributes collateral to. Default {default}.\n--seed is \
         the prepare stage's 64-lowercase-hex determinism switch for the\nsubstrate's disposable \
         loopback roles.\n\nEverything runs on 127.0.0.1 and nothing here touches a public \
         cluster.",
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

    /// Subcommands this binary still dispatches to a working implementation.
    const LIVE: &[&str] = &["run"];
    /// Subcommands that are dispatched and refuse unconditionally.
    const RETIRED: &[&str] = &["demo-market"];

    fn runner() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/run-journey.sh"))
            .expect("run-journey.sh sits beside this crate")
    }

    /// Every subcommand `run-journey.sh` invokes on the journey binary.
    ///
    /// Two call shapes exist and both are resolved: a literal
    /// (`"$JOURNEY_BIN" demo-market …`) and an array whose first element is the
    /// subcommand (`JOURNEY_ARGS=(run …)` then `"$JOURNEY_BIN" "${JOURNEY_ARGS[@]}"`).
    fn invoked(script: &str) -> Vec<String> {
        let mut found = Vec::new();
        for line in script.lines() {
            let Some(rest) = line.split_once("\"$JOURNEY_BIN\"").map(|(_, rest)| rest) else {
                continue;
            };
            let Some(token) = rest.split_whitespace().next() else {
                continue;
            };
            let token = token.trim_matches('"');
            if let Some(name) = token.strip_prefix("${").and_then(|v| v.strip_suffix("[@]}")) {
                // Resolve the array's first element from its assignment.
                let needle = format!("{name}=(");
                if let Some(assignment) = script.split(&needle).nth(1) {
                    if let Some(first) = assignment.split_whitespace().next() {
                        found.push(first.trim_matches('"').to_string());
                    }
                }
            } else if !token.is_empty()
                && token
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && token.starts_with(|c: char| c.is_ascii_lowercase())
            {
                // Subcommand-shaped only. `run-journey.sh` also mentions the
                // binary inside a `[ -x "$JOURNEY_BIN" ]` guard, whose next
                // token is `]`; that is an existence test, not an invocation.
                found.push(token.to_string());
            }
        }
        found
    }

    /// THE GATE THIS TIER DID NOT HAVE.
    ///
    /// Public CI runs a job named "the journey campaign compiles". It does, and
    /// it compiled every day for two days while its runner called a subcommand
    /// the binary refuses unconditionally — because `demo-market` is still
    /// DISPATCHED, so nothing about the build could notice. The test above
    /// proves the refusal exists; this one proves the runner stopped calling
    /// it, which is the half that was missing.
    #[test]
    fn the_runner_invokes_no_retired_subcommand() {
        let script = runner();
        let invoked = invoked(&script);
        assert!(
            !invoked.is_empty(),
            "found no journey-binary invocation in run-journey.sh; the parser has drifted \
             from the script and this gate would pass vacuously",
        );
        for command in &invoked {
            assert!(
                !RETIRED.contains(&command.as_str()),
                "run-journey.sh invokes the retired subcommand `{command}`, which refuses \
                 unconditionally, so the campaign cannot run however well it compiles",
            );
            assert!(
                LIVE.contains(&command.as_str()),
                "run-journey.sh invokes `{command}`, which this binary does not dispatch",
            );
        }
    }

    /// Keeps `RETIRED` from becoming a list someone edits to make the gate pass.
    ///
    /// Every name on it must actually refuse when dispatched. Un-retiring a
    /// subcommand without removing it here turns this red.
    #[test]
    fn every_retired_subcommand_actually_refuses() {
        for command in RETIRED {
            assert_eq!(
                *command, "demo-market",
                "a retired subcommand was added without a refusal witness beside it",
            );
            assert!(
                super::run_demo_market(vec![
                    "--registry-program-id".into(),
                    "11111111111111111111111111111111".into(),
                ])
                .is_err(),
                "`{command}` is listed as retired but no longer refuses",
            );
        }
    }
}
