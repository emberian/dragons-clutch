//! Exact clean-build freshness for the Lean-owned Hot V3 terminal ABI.

use std::{path::PathBuf, process::Command};

#[test]
fn generated_hot_v3_abi_is_exact() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args(["build", "DClutchSemantics.RationalTerminalHotV3Abi"])
        .output()
        .expect("build exact imported Hot V3 ABI target");
    assert!(
        build.status.success(),
        "Hot V3 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .current_dir(&formal)
        .args(["env", "lean", "--run", "EmitRationalTerminalHotV3Rust.lean"])
        .output()
        .expect("run exact Hot V3 ABI generator");
    assert!(
        generated.status.success(),
        "Hot V3 ABI generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in =
        std::fs::read(manifest.join("src/generated_hot_v3.rs")).expect("read generated Hot ABI");
    assert_eq!(generated.stdout, checked_in);
}
