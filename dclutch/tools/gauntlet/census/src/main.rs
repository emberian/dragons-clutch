//! `dclutch-route-census` — the gauntlet's execution census.
//!
//! Three commands:
//!
//!   inventory --root DIR --out FILE [--revision REV]
//!       Statically enumerate every program's public dispatch surface and
//!       refusal taxonomy from the Rust AST.
//!
//!   observe --inventory FILE --ledger FILE --bindings FILE
//!           --programs FILE --evidence FILE --campaign NAME
//!       Fold one campaign's chain evidence into the append-only ledger,
//!       cross-checking every claim against the finalized transaction logs.
//!
//!   report --inventory FILE --ledger FILE --blocked FILE --out FILE
//!       Render EXECUTED / NEVER-EXECUTED per route and per refusal code.
//!
//! Nothing here touches a chain, signs anything, or writes into the source
//! tree it reads.

mod enumerate;
mod ledger;
mod model;
mod report;

use std::{collections::BTreeMap, fs, path::Path, process::ExitCode};

use model::{Bindings, BlockedSet, Inventory, Ledger, ProgramMap};

/// The programs the census enumerates, and their short labels.
///
/// The list is explicit rather than globbed so that adding a program is a
/// visible decision. A program in `programs/` that is missing here is reported
/// by `inventory --check-complete`.
///
/// The reverse also has to stay true: a package listed here that is no longer
/// in `programs/` is silently filtered out at run time, so a stale entry looks
/// exactly like a live one from the report. `dclutch-general-sbf` sat here for
/// exactly that reason, four months after `5b19626` deleted it. Delete a program's entry in the same commit that
/// deletes the program.
const TARGETS: &[(&str, &str)] = &[
    ("dclutch-claims-proof-sbf", "claims-proof"),
    ("dclutch-claims-sbf", "claims"),
    ("dclutch-controller-proof-sbf", "controller-proof"),
    ("dclutch-core-sbf", "core"),
    ("dclutch-custody-proof-sbf", "custody-proof"),
    ("dclutch-custody-sbf", "custody"),
    ("dclutch-dealer-accelerator-sbf", "dealer-accelerator"),
    ("dclutch-dealer-sbf", "dealer"),
    ("dclutch-direct-aot-sbf", "direct-aot"),
    ("dclutch-general-accelerator-sbf", "general-accelerator"),
    ("dclutch-product-runtime-v2-sbf", "product-runtime-v2"),
    ("dclutch-registry-sbf", "registry"),
    ("dclutch-rent-sbf", "rent"),
    ("dclutch-resolution-proof-sbf", "resolution"),
    ("dclutch-series-shadow-sbf", "series-shadow"),
    ("dclutch-trading-sbf", "trading"),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("dclutch-route-census: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = arguments.first().map(String::as_str) else {
        usage();
        return Ok(());
    };
    let options = parse_options(&arguments[1..])?;
    match command {
        "inventory" => command_inventory(&options),
        "observe" => command_observe(&options),
        "report" => command_report(&options),
        "help" | "-h" | "--help" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn usage() {
    eprintln!(
        "usage:\n  \
         dclutch-route-census inventory --root DIR --out FILE [--revision REV]\n  \
         dclutch-route-census observe --inventory FILE --ledger FILE --bindings FILE \\\n    \
             --programs FILE --evidence FILE\n  \
         dclutch-route-census report --inventory FILE --ledger FILE --blocked FILE --out FILE"
    );
}

type Options = BTreeMap<String, String>;

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options::new();
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        let Some(name) = argument.strip_prefix("--") else {
            return Err(format!("unexpected positional argument: {argument}"));
        };
        let value = iterator
            .next()
            .ok_or_else(|| format!("--{name} requires a value"))?;
        if options.insert(name.to_string(), value.clone()).is_some() {
            return Err(format!("--{name} may be supplied only once"));
        }
    }
    Ok(options)
}

fn require<'a>(options: &'a Options, name: &str) -> Result<&'a str, String> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("--{name} is required"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {path}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {path}: {error}"))
}

/// Write through a same-directory temporary file so a failed run leaves the
/// last accepted output byte-for-byte intact (`AGENTS.md`, generator policy).
fn write_atomic(path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = Path::new(path);
    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)
        .map_err(|error| format!("create {}: {error}", directory.display()))?;
    let temporary = directory.join(format!(
        ".{}.census-tmp",
        target
            .file_name()
            .map_or_else(|| "out".into(), |name| name.to_string_lossy().into_owned())
    ));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|error| format!("replace {path}: {error}"))
}

fn command_inventory(options: &Options) -> Result<(), String> {
    let root = require(options, "root")?;
    let out = require(options, "out")?;
    let root_path = Path::new(root)
        .canonicalize()
        .map_err(|error| format!("canonicalize {root}: {error}"))?;

    // Every program directory must be either enumerated or explicitly absent
    // from the list on purpose. Silence here would be the first crack.
    let mut present: Vec<String> = Vec::new();
    let programs_dir = root_path.join("programs");
    for entry in fs::read_dir(&programs_dir)
        .map_err(|error| format!("read {}: {error}", programs_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("read entry: {error}"))?;
        if entry.path().join("Cargo.toml").is_file() {
            present.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    present.sort();
    let known: Vec<&str> = TARGETS.iter().map(|(package, _)| *package).collect();
    let missing: Vec<&String> = present
        .iter()
        .filter(|package| !known.contains(&package.as_str()))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "these program packages exist but are not in the census target list, so their \
             dispatch surface would be invisible: {}. Add them to TARGETS in src/main.rs.",
            missing
                .iter()
                .map(|package| package.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    eprintln!("census: indexing constants under {}", root_path.display());
    let constants = enumerate::index_constants(&root_path)?;
    let targets: Vec<enumerate::ProgramTarget> = TARGETS
        .iter()
        .filter(|(package, _)| present.iter().any(|found| found == package))
        .map(|(package, label)| enumerate::ProgramTarget {
            package: (*package).to_string(),
            label: (*label).to_string(),
        })
        .collect();
    eprintln!("census: enumerating {} programs", targets.len());
    let inventory = enumerate::enumerate(
        &root_path,
        &targets,
        &constants,
        options.get("revision").cloned(),
    )?;

    let routes: usize = inventory
        .programs
        .iter()
        .map(|program| program.routes.len())
        .sum();
    let refusals: usize = inventory
        .programs
        .iter()
        .map(|program| program.refusals.len())
        .sum();
    let unclassified: usize = inventory
        .programs
        .iter()
        .map(|program| program.unclassified.len())
        .sum();
    eprintln!(
        "census: {routes} routes, {refusals} refusal codes, {unclassified} unclassified positions"
    );

    let mut bytes = serde_json::to_vec_pretty(&inventory)
        .map_err(|error| format!("encode inventory: {error}"))?;
    bytes.push(b'\n');
    write_atomic(out, &bytes)
}

fn command_observe(options: &Options) -> Result<(), String> {
    let inventory: Inventory = read_json(require(options, "inventory")?)?;
    let bindings: Bindings = read_json(require(options, "bindings")?)?;
    let programs: ProgramMap = read_json(require(options, "programs")?)?;
    let evidence_path = require(options, "evidence")?;
    let ledger_path = require(options, "ledger")?;

    let evidence_bytes =
        fs::read(evidence_path).map_err(|error| format!("read {evidence_path}: {error}"))?;
    let evidence: serde_json::Value = serde_json::from_slice(&evidence_bytes)
        .map_err(|error| format!("parse {evidence_path}: {error}"))?;

    let mut held: Ledger = if Path::new(ledger_path).is_file() {
        read_json(ledger_path)?
    } else {
        Ledger::default()
    };

    let report = ledger::fold(
        &mut held,
        &inventory,
        &bindings,
        &programs,
        &evidence,
        evidence_path,
        &evidence_bytes,
    )?;

    let mut bytes =
        serde_json::to_vec_pretty(&held).map_err(|error| format!("encode ledger: {error}"))?;
    bytes.push(b'\n');
    write_atomic(ledger_path, &bytes)?;

    eprintln!(
        "census: admitted {} observations into {ledger_path}",
        report.admitted
    );
    if report.problems.is_empty() {
        return Ok(());
    }
    for problem in &report.problems {
        eprintln!("census PROBLEM: {problem}");
    }
    Err(format!(
        "{} binding/evidence problems; the census refuses to record coverage it cannot corroborate",
        report.problems.len()
    ))
}

fn command_report(options: &Options) -> Result<(), String> {
    let inventory: Inventory = read_json(require(options, "inventory")?)?;
    let ledger_path = require(options, "ledger")?;
    let held: Ledger = if Path::new(ledger_path).is_file() {
        read_json(ledger_path)?
    } else {
        Ledger::default()
    };
    let blocked: BlockedSet = read_json(require(options, "blocked")?)?;
    let out = require(options, "out")?;

    let (rendered, totals) = report::render(&inventory, &held, &blocked);
    write_atomic(out, rendered.as_bytes())?;
    eprintln!(
        "census: {} routes | {} executed | {} refused-only | {} never-executed \
         ({} blocked, {} unclaimed) | {}/{} refusal codes observed | {} unclassified \
         | {} stale blocking entries",
        totals.routes,
        totals.routes_executed,
        totals.routes_refused_only,
        totals.routes_never,
        totals.routes_never_blocked,
        totals.routes_never - totals.routes_never_blocked,
        totals.refusals_observed,
        totals.refusals,
        totals.unclassified,
        totals.stale_blocked
    );
    Ok(())
}
