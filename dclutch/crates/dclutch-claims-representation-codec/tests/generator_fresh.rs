//! Exact freshness check for the Lean-owned generated Rust data.

use std::{path::PathBuf, process::Command};

#[test]
fn checked_in_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args(["build", "DClutchSemantics.ClaimsRepresentationAbi"])
        .output()
        .expect("build exact imported Lean ClaimsRepresentation ABI target");
    assert!(
        build.status.success(),
        "semantic target build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
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
        "generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in =
        std::fs::read(manifest.join("src/generated.rs")).expect("read checked-in generated Rust");
    assert_eq!(generated.stdout, checked_in);
}
