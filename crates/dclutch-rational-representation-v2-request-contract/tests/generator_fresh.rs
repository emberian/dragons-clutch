//! Exact clean-build freshness for the Lean-owned physical ABI.

use std::{path::PathBuf, process::Command};

#[test]
fn generated_physical_abi_is_exact() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args([
            "build",
            "DClutchSemantics.RationalRepresentationV2PhysicalAbi",
        ])
        .output()
        .expect("build exact imported physical ABI target");
    assert!(
        build.status.success(),
        "physical ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .current_dir(&formal)
        .args([
            "env",
            "lean",
            "--run",
            "EmitRationalRepresentationV2PhysicalAbiRust.lean",
        ])
        .output()
        .expect("run exact physical ABI generator");
    assert!(
        generated.status.success(),
        "physical ABI generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in =
        std::fs::read(manifest.join("src/generated.rs")).expect("read generated Rust ABI");
    assert_eq!(generated.stdout, checked_in);
}
