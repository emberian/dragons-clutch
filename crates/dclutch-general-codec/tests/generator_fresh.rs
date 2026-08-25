//! Freshness check for the checked-in Lean-generated Rust module.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralControllerAbi"])
        .current_dir(&formal)
        .output()
        .expect("build imported General semantic target");
    assert!(
        build.status.success(),
        "General semantic target build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralControllerAbiRust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Lean General ABI generator");
    assert!(
        output.status.success(),
        "generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_general_controller.rs"))
        .expect("read checked-in generated codec");
    assert_eq!(
        output.stdout, checked_in,
        "regenerate the General controller codec"
    );
}
