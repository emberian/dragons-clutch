//! Exact freshness check for the Lean-owned generated Rust data.

use std::{path::PathBuf, process::Command};

#[test]
fn checked_in_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let generated = Command::new("lake")
        .current_dir(&formal)
        .args([
            "env",
            "lean",
            "--run",
            "EmitClaimsRepresentationAbiRust.lean",
        ])
        .output()
        .expect("run Lean ClaimsRepresentation ABI generator");
    assert!(
        generated.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in =
        std::fs::read(manifest.join("src/generated.rs")).expect("read checked-in generated Rust");
    assert_eq!(generated.stdout, checked_in);
}
