//! Exact freshness check for the scoring Dealer's Lean-generated module.
//!
//! The same guard as `dealer__generator_fresh.rs`: build the ABI module, run
//! the emitter, rustfmt-normalise its stdout, and byte-compare with the
//! checked-in file. Normalised for the reason that file gives -- a raw compare
//! is green only while the emission is already a rustfmt fixpoint.

use std::path::PathBuf;
use std::process::Command;

fn rustfmt_normalised(stdout: &[u8], tag: &str) -> Vec<u8> {
    let temporary = std::env::temp_dir().join(format!("dclutch-{tag}-{}.rs", std::process::id()));
    std::fs::write(&temporary, stdout).expect("write generated Rust");
    let formatted = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .arg(&temporary)
        .output()
        .expect("launch rustfmt");
    assert!(
        formatted.status.success(),
        "rustfmt failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&formatted.stdout),
        String::from_utf8_lossy(&formatted.stderr)
    );
    let normalised = std::fs::read(&temporary).expect("read formatted generated Rust");
    std::fs::remove_file(&temporary).expect("remove generated Rust");
    normalised
}

#[test]
fn checked_in_scoring_rule_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.ScoringRuleAbiV1"])
        .current_dir(&formal)
        .output()
        .expect("build the scoring rule ABI module");
    assert!(
        build.status.success(),
        "scoring rule ABI build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitScoringRuleV1Rust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run the scoring rule generator");
    assert!(
        output.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/scoring_rule/generated_scoring_rule.rs"))
        .expect("read the checked-in generated scoring rule module");
    let formatted = rustfmt_normalised(&output.stdout, "scoring-rule");
    assert_eq!(
        formatted, checked_in,
        "regenerate the scoring rule module: lake env lean --run EmitScoringRuleV1Rust.lean | rustfmt"
    );
}
