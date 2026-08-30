//! Freshness check for the checked-in Lean-generated V3 request geometry.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_v3_request_geometry_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralControllerRequestV3"])
        .current_dir(&formal)
        .output()
        .expect("build imported General V3 request target");
    assert!(
        build.status.success(),
        "General V3 request target build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitGeneralControllerRequestV3Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .expect("run Lean General V3 request generator");
    assert!(
        output.status.success(),
        "generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_general_controller_request_v3.rs"))
        .expect("read checked-in generated V3 request geometry");
    assert_eq!(
        output.stdout, checked_in,
        "regenerate the General V3 request geometry"
    );
}
