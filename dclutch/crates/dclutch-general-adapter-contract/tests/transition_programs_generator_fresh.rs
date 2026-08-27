//! Exact Lean-generator freshness check for General transition programs.
//!
//! The byte gate in `transition_artifacts_v3.rs` proves the Rust builder agrees
//! with the CHECKED-IN arrays. This proves the checked-in arrays are still what
//! the Lean module emits. Neither is sufficient alone: without this one, an edit
//! to `GeneralTransitionV3.lean` that nobody regenerated leaves two agreeing
//! Rust authorities and one silent Lean one.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_general_transition_programs_are_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralTransitionV3"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "General transition-program build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralTransitionV3Rust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "General transition-program generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_transition_programs_v3.rs"))
        .unwrap_or_else(|error| panic!("read generated Rust: {error}"));
    assert_eq!(generated.stdout, checked_in);
}
