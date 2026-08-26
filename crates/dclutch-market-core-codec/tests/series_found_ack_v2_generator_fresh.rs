//! Exact Lean-generator freshness check for Series Core Found Ack V2.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_series_found_ack_v2_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.SeriesCoreFoundAckV2Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "Series Core Found Ack V2 build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args(["env", "lean", "--run", "EmitSeriesCoreFoundAckV2Rust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "Series Core Found Ack V2 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_series_found_ack_v2.rs"))
        .unwrap_or_else(|error| panic!("read generated Rust: {error}"));
    assert_eq!(generated.stdout, checked_in);
}
