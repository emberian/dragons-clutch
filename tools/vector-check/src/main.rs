//! `vector-check` — the `rust-reference` executor of the Dragon's Clutch
//! semantic vector spine.
//!
//! It loads every manifest under `fixtures/vectors/`, runs every vector against
//! the landed semantic crates, maps each refusal through the taxonomy tables of
//! `docs/implementation/VECTOR_SPINE_PROPOSAL.md` §2.4, and reports one
//! disposition line per executor.
//!
//! Direction (§6): this crate depends on the vectors and on the semantic
//! crates.  Nothing depends on it, and no semantic crate depends on it or on
//! the vectors.  A disagreement it reports is a finding to triage, never a
//! reason to edit a vector.

mod exec;
mod json;
mod sha256;
mod taxonomy;
mod vectors;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::taxonomy::{Coarsening, Observed, Taxonomy};
use crate::vectors::{Expect, Manifest, Vector, EXECUTOR_IDS};

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root = PathBuf::from("fixtures/vectors");
    let mut show_facts = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => match args.next() {
                Some(path) => root = PathBuf::from(path),
                None => {
                    eprintln!("--root needs a path");
                    return std::process::ExitCode::from(2);
                }
            },
            "--facts" => show_facts = true,
            "--help" | "-h" => {
                println!("vector-check [--root fixtures/vectors] [--facts]");
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other:?}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    match run(&root, show_facts) {
        Ok(report) => {
            print!("{}", report.render());
            if report.failures == 0 {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("vector-check: {error}");
            std::process::ExitCode::from(2)
        }
    }
}

#[derive(Default)]
pub struct Report {
    lines: Vec<String>,
    pub vectors: usize,
    pub steps: usize,
    pub asserted_facts: usize,
    pub failures: usize,
    pub coarsened: usize,
    /// executor id -> disposition mode -> count (COMP-4's printable ratio).
    pub dispositions: BTreeMap<String, BTreeMap<String, usize>>,
    pub by_domain: BTreeMap<String, usize>,
    /// COMP-5 discipline: how many refusal vectors are single-fault, and how
    /// many declare a precedence note instead.
    pub single_fault: usize,
    pub precedence_declared: usize,
    pub byte_exact: BTreeMap<String, usize>,
    /// D7: every named blocker, counted, so the gap is a number and not a mood.
    pub blockers: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
    pub by_provenance: BTreeMap<String, usize>,
    pub by_post_state_rule: BTreeMap<String, usize>,
    pub properties: BTreeMap<String, usize>,
    pub byte_artifacts: usize,
    pub by_surface: BTreeMap<String, usize>,
    pub codes_exercised: BTreeMap<u32, usize>,
    pub taxonomy_codes: usize,
}

impl Report {
    fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
        out.push_str("== coverage ==\n");
        out.push_str(&format!(
            "vectors {}   steps {}   asserted facts {}   failures {}\n",
            self.vectors, self.steps, self.asserted_facts, self.failures
        ));
        out.push_str(&format!(
            "taxonomy codes defined {}   distinct codes exercised {}\n",
            self.taxonomy_codes,
            self.codes_exercised.len()
        ));
        out.push_str(&format!("coarsened acceptances {}\n", self.coarsened));
        out.push_str("by domain: ");
        out.push_str(&join(&self.by_domain));
        out.push('\n');
        out.push_str("by surface: ");
        out.push_str(&join(&self.by_surface));
        out.push('\n');
        out.push_str(&format!(
            "COMP-5: {} single-fault, {} with a declared check-order precedence\n",
            self.single_fault, self.precedence_declared
        ));
        out.push_str("COMP-2 byte_exact: ");
        out.push_str(&join(&self.byte_exact));
        out.push_str(&format!(
            "   named byte artifacts {}\n",
            self.byte_artifacts
        ));
        out.push_str("COMP-6 post_state_on_error: ");
        out.push_str(&join(&self.by_post_state_rule));
        out.push('\n');
        out.push_str("status: ");
        out.push_str(&join(&self.by_status));
        out.push_str("   provenance: ");
        out.push_str(&join(&self.by_provenance));
        out.push('\n');
        out.push_str("properties: ");
        out.push_str(&join(&self.properties));
        out.push('\n');
        out.push_str("\n== executor dispositions (COMP-4) ==\n");
        for id in EXECUTOR_IDS {
            let modes = self.dispositions.get(id).cloned().unwrap_or_default();
            out.push_str(&format!("{id:<18} {}\n", join(&modes)));
        }
        out.push_str("\n== named blockers and scope reasons (D2, D7) ==\n");
        for (blocker, count) in &self.blockers {
            out.push_str(&format!("  {count:>3}  {blocker}\n"));
        }
        out.push_str(
            "\nOnly `rust-reference` executed. Four executors are declared, counted, and did not\n\
             run, so a pass here is one Rust implementation agreeing with a manifest, never\n\
             cross-runtime agreement (VECTOR_SPINE_PROPOSAL.md §5).\n",
        );
        out
    }
}

fn join(map: &BTreeMap<String, usize>) -> String {
    if map.is_empty() {
        return "-".to_string();
    }
    map.iter()
        .map(|(key, count)| format!("{key} {count}"))
        .collect::<Vec<_>>()
        .join("   ")
}

pub fn run(root: &Path, show_facts: bool) -> Result<Report, String> {
    let taxonomy_path = root.join("TAXONOMY.json");
    let taxonomy_text = std::fs::read_to_string(&taxonomy_path)
        .map_err(|error| format!("{}: {error}", taxonomy_path.display()))?;
    let taxonomy = Taxonomy::load(&taxonomy_text)
        .map_err(|error| format!("{}: {error}", taxonomy_path.display()))?;

    let mut report = Report {
        taxonomy_codes: taxonomy.len(),
        ..Report::default()
    };
    report.lines.push(format!(
        "taxonomy v{} — {} codes, digest {}",
        taxonomy.version,
        taxonomy.len(),
        taxonomy.digest
    ));

    let mut paths = Vec::new();
    collect(root, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no manifests under {}", root.display()));
    }

    let mut seen_ids: BTreeMap<String, String> = BTreeMap::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let display = path.display().to_string();
        let manifest = Manifest::load(&display, &text, &taxonomy)?;
        report.lines.push(format!(
            "\n== {} — family {} ({} vectors, status {}, schema v{}, taxonomy v{})",
            manifest.path,
            manifest.family,
            manifest.vectors.len(),
            manifest.status,
            manifest.schema_version,
            manifest.taxonomy_version
        ));
        // DIG-2 and DIG-5: a manifest's own digest is recomputed, never trusted,
        // and a placeholder is refused outright.
        check_digest(
            &mut report,
            &manifest.path,
            "manifest",
            &manifest.declared_manifest_digest,
            &manifest.computed_manifest_digest,
        );
        // DIG-3: the taxonomy binding is checked before any executor runs.
        match &manifest.declared_taxonomy_digest {
            Some(declared) if declared == &taxonomy.digest => {}
            Some(declared) => {
                report.failures += 1;
                report.lines.push(format!(
                    "  FAIL digests.taxonomy {declared} does not match the checked-out taxonomy {}",
                    taxonomy.digest
                ));
            }
            None => {
                report.failures += 1;
                report
                    .lines
                    .push("  FAIL digests.taxonomy is absent (DIG-3)".to_string());
            }
        }

        for vector in &manifest.vectors {
            if let Some(other) = seen_ids.insert(vector.id.clone(), manifest.path.clone()) {
                report.failures += 1;
                report.lines.push(format!(
                    "  FAIL vector id {} is declared in both {other} and {}",
                    vector.id, manifest.path
                ));
            }
            run_vector(&mut report, &taxonomy, vector, show_facts);
        }
    }
    Ok(report)
}

fn check_digest(report: &mut Report, path: &str, what: &str, declared: &str, computed: &str) {
    if declared.starts_with("pending") || declared.is_empty() {
        report.failures += 1;
        report.lines.push(format!(
            "  FAIL {path}: {what} digest is the placeholder {declared:?}; DIG-5 admits placeholders only in the proposal document"
        ));
    } else if declared != computed {
        report.failures += 1;
        report.lines.push(format!(
            "  FAIL {path}: declared {what} digest {declared} but the JCS recomputation is {computed}"
        ));
    }
}

fn run_vector(report: &mut Report, taxonomy: &Taxonomy, vector: &Vector, show_facts: bool) {
    report.vectors += 1;
    *report.by_domain.entry(vector.domain.clone()).or_default() += 1;
    *report.by_surface.entry(vector.surface.clone()).or_default() += 1;
    for (id, disposition) in &vector.executors {
        *report
            .dispositions
            .entry(id.clone())
            .or_default()
            .entry(disposition.mode.clone())
            .or_default() += 1;
        if let Some(blocker) = &disposition.blocked_by {
            *report.blockers.entry(blocker.clone()).or_default() += 1;
        }
        if let Some(reason) = &disposition.reason {
            *report
                .blockers
                .entry(format!("reason: {reason}"))
                .or_default() += 1;
        }
    }
    match vector.comparison.single_fault {
        Some(true) => report.single_fault += 1,
        Some(false) => report.precedence_declared += 1,
        None => {}
    }
    if vector.comparison.precedence_note.is_some() && vector.comparison.single_fault != Some(false)
    {
        report.precedence_declared += 1;
    }
    *report
        .byte_exact
        .entry(vector.comparison.byte_exact.clone())
        .or_default() += 1;
    report.byte_artifacts += vector.comparison.byte_artifacts;
    *report.by_status.entry(vector.status.clone()).or_default() += 1;
    *report
        .by_provenance
        .entry(vector.provenance_kind.clone())
        .or_default() += 1;
    *report
        .by_post_state_rule
        .entry(vector.comparison.post_state_on_error.clone())
        .or_default() += 1;
    for property in &vector.property_ids {
        *report.properties.entry(property.clone()).or_default() += 1;
    }

    let mut failures = Vec::new();
    let declared = &vector.executors["rust-reference"];
    let mut coarsened_here = 0usize;
    let digest_before = report.failures;
    check_digest(
        report,
        &vector.id,
        "vector",
        &vector.declared_digest,
        &vector.computed_digest,
    );
    let digest_ok = report.failures == digest_before;

    let mut facts = 0usize;
    let executor = exec::open(
        &vector.initial_state.form,
        &vector.initial_state.constructed_by,
        &vector.initial_state.value,
    );
    let mut executor = match executor {
        Ok(executor) => executor,
        Err(error) => {
            report.failures += 1;
            report.lines.push(format!("  FAIL {}: {error}", vector.id));
            return;
        }
    };

    for step in &vector.operations {
        report.steps += 1;
        let observed = match executor.apply(&step.op, &step.args) {
            Ok(observed) => observed,
            Err(error) => {
                failures.push(format!("step {} ({}): {error}", step.step, step.op));
                break;
            }
        };
        match compare(taxonomy, &step.expect, &observed, declared, report) {
            Ok((count, coarsened)) => {
                facts += count;
                if coarsened {
                    coarsened_here += 1;
                }
            }
            Err(error) => failures.push(format!("step {} ({}): {error}", step.step, step.op)),
        }
        if let Some(expected) = &step.post_state {
            match exec::named_fact_match(&expected.value, &executor.render_state(), "post_state") {
                Ok(count) => facts += count,
                Err(error) => {
                    failures.push(format!("step {} post_state: {error}", step.step));
                }
            }
        }
    }

    if let Some(final_state) = &vector.final_state {
        match exec::named_fact_match(&final_state.value, &executor.render_state(), "final_state") {
            Ok(count) => facts += count,
            Err(error) => failures.push(format!("final_state: {error}")),
        }
    }

    // A `coarsened` disposition is a claim that the executor cannot answer
    // exactly; a vector that declares it and then matches exactly everywhere is
    // stating a loss of resolution that no longer exists.
    if declared.mode == "coarsened" && coarsened_here == 0 {
        failures.push(
            "rust-reference declares mode \"coarsened\" but every step matched exactly (TAX-6)"
                .to_string(),
        );
    }
    if vector.comparison.byte_exact == "required" {
        failures.push(
            "byte_exact \"required\" but this executor has no byte comparison implemented (COMP-2)"
                .to_string(),
        );
    }
    report.asserted_facts += facts;
    if failures.is_empty() && digest_ok {
        report.lines.push(format!(
            "  ok   {:<52} {:>3} facts  [{}]",
            vector.id, facts, vector.primary_property_id
        ));
        if show_facts {
            report.lines.push(format!("       {}", vector.title));
        }
    } else {
        for failure in &failures {
            report.failures += 1;
            report
                .lines
                .push(format!("  FAIL {}: {failure}", vector.id));
        }
    }
}

fn compare(
    taxonomy: &Taxonomy,
    expect: &Expect,
    observed: &Observed,
    declared: &crate::vectors::Disposition,
    report: &mut Report,
) -> Result<(usize, bool), String> {
    match (expect, observed) {
        (Expect::Ok { value }, Observed::Ok(produced)) => match value {
            // COMP-7: a named success value is part of the vector.
            Some(expected) => {
                exec::named_fact_match(expected, produced, "value").map(|n| (n, false))
            }
            None => Ok((1, false)),
        },
        (Expect::Ok { .. }, Observed::Error(refusal)) => Err(format!(
            "expected ok, the implementation refused with {} (code {})",
            refusal.variant, refusal.code
        )),
        (Expect::Ok { .. }, Observed::Refused(refusal)) => Err(format!(
            "expected ok, the implementation returned refusal {} (code {})",
            refusal.variant, refusal.code
        )),
        (
            Expect::Refusal {
                kind, name, code, ..
            },
            Observed::Ok(_),
        ) => Err(format!(
            "expected a {kind} of {name} ({code}), the implementation succeeded"
        )),
        (
            Expect::Refusal {
                kind,
                code,
                name,
                frame,
            },
            Observed::Error(refusal) | Observed::Refused(refusal),
        ) => {
            // TAX-4 / D6: refusal and error never substitute for each other.
            let observed_kind = match observed {
                Observed::Error(_) => "error",
                _ => "refusal",
            };
            if kind != observed_kind {
                return Err(format!(
                    "expected result_kind {kind} for {name} ({code}), the implementation produced {observed_kind} (TAX-4)"
                ));
            }
            if let Some(frame) = frame {
                if frame != refusal.frame {
                    return Err(format!(
                        "expected frame {frame:?}, the refusal came from {:?}",
                        refusal.frame
                    ));
                }
            }
            let mut coarsened = false;
            match taxonomy.accepts(*code, refusal.code)? {
                Coarsening::Exact => {
                    // D5 is directional: a declared coarsening does not license
                    // a *different* exact answer, only the declared coarse one.
                }
                Coarsening::Coarsened { to } => {
                    // TAX-6 acceptance is admissible only where the vector
                    // declared this executor coarsened, and only to this code.
                    if declared.mode != "coarsened" {
                        return Err(format!(
                            "code {} coarsens over the expected {code}, but rust-reference is declared {:?}, not \"coarsened\" (TAX-6)",
                            refusal.code, declared.mode
                        ));
                    }
                    if declared.coarsens_to != Some(refusal.code) {
                        return Err(format!(
                            "rust-reference declares coarsens_to {:?}, the observed coarse code is {}",
                            declared.coarsens_to, refusal.code
                        ));
                    }
                    coarsened = true;
                    report.coarsened += 1;
                    report
                        .lines
                        .push(format!("       coarsened: {code} accepted as {to} (TAX-6)"));
                }
            }
            *report.codes_exercised.entry(refusal.code).or_default() += 1;
            Ok((1, coarsened))
        }
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().map(|ext| ext == "json").unwrap_or(false) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "TAXONOMY.json" && name != "SCHEMA.json" {
                out.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR is `<repo>/tools/vector-check`.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("repository root")
            .to_path_buf()
    }

    #[test]
    fn every_committed_vector_passes_the_rust_reference_executor() {
        let report = run(&repo_root().join("fixtures/vectors"), false).expect("manifests load");
        print!("{}", report.render());
        assert_eq!(
            report.failures, 0,
            "a failing vector is a finding to triage, never a reason to edit the vector"
        );
        assert!(report.vectors >= 18, "vector count {}", report.vectors);
        assert!(report.asserted_facts > 0);
    }

    #[test]
    fn every_vector_carries_all_five_executor_dispositions() {
        let report = run(&repo_root().join("fixtures/vectors"), false).expect("manifests load");
        for id in EXECUTOR_IDS {
            let total: usize = report
                .dispositions
                .get(id)
                .map(|modes| modes.values().sum())
                .unwrap_or(0);
            assert_eq!(total, report.vectors, "executor {id} (COMP-4)");
        }
    }

    /// DIG-1: the digest is over the vector body, so any edit to an expectation
    /// changes it.  A manifest whose digest was not restamped fails to load.
    #[test]
    fn a_tampered_expectation_changes_the_vector_digest() {
        let path = repo_root().join("fixtures/vectors/kernel/core.json");
        let text = std::fs::read_to_string(&path).expect("manifest");
        let root = crate::json::parse(&text).expect("parses");
        let vector = &root.require("vectors").unwrap().as_array().unwrap()[0];
        let before = sha256::hex(vector.without("digests").to_jcs().as_bytes());
        assert_eq!(
            before,
            vector
                .require("digests")
                .unwrap()
                .require("vector")
                .unwrap()
                .as_str()
                .unwrap()
        );
        let tampered = crate::json::parse(&text.replacen("\"11\"", "\"12\"", 1)).expect("parses");
        let tampered_vector = &tampered.require("vectors").unwrap().as_array().unwrap()[0];
        let after = sha256::hex(tampered_vector.without("digests").to_jcs().as_bytes());
        assert_ne!(before, after);
    }

    /// TAX-6 and D5: a coarse code is admitted only where it declares the exact
    /// code, and never in the other direction or between siblings.
    #[test]
    fn coarsening_is_directional_and_declared() {
        let text = std::fs::read_to_string(repo_root().join("fixtures/vectors/TAXONOMY.json"))
            .expect("taxonomy");
        let taxonomy = Taxonomy::load(&text).expect("loads");
        assert!(taxonomy.accepts(2062, 2060).is_ok());
        assert_eq!(
            taxonomy.accepts(2062, 2062),
            Ok(taxonomy::Coarsening::Exact)
        );
        // The other direction is not a coarsening.
        assert!(taxonomy.accepts(2060, 2062).is_err());
        // Siblings under one coarse code are not interchangeable.
        assert!(taxonomy.accepts(2061, 2062).is_err());
        // An unrelated code is never "close enough".
        assert!(taxonomy.accepts(2062, 5001).is_err());
        // VER-8: a code the taxonomy does not define cannot be referenced.
        assert!(taxonomy.row(9999).is_err());
    }

    /// INT-1 and INT-3: the reader is total and refuses anything a conforming
    /// parser could round differently.
    #[test]
    fn the_reader_refuses_floats_and_inexact_integers() {
        assert!(crate::json::parse("{\"a\": 1.0}").is_err());
        assert!(crate::json::parse("{\"a\": 1e2}").is_err());
        assert!(json::Value::Str("18446744073709551616".into())
            .as_u64()
            .is_err());
    }

    #[test]
    fn only_the_rust_reference_executor_claims_to_have_run() {
        let report = run(&repo_root().join("fixtures/vectors"), false).expect("manifests load");
        for id in [
            "verus-host",
            "rocq-extracted",
            "lean-checker",
            "sbf-program-test",
        ] {
            let modes = report.dispositions.get(id).expect("declared");
            assert_eq!(
                modes.get("exact").copied().unwrap_or(0),
                0,
                "executor {id} may not be `exact` while it does not exist"
            );
        }
    }
}
