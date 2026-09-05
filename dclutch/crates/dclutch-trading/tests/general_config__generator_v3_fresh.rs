//! Exact Lean-generator freshness check for GeneralConfigV3.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_v3_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralConfigV3Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean V3 build: {error}"));
    assert!(
        build.status.success(),
        "General V3 config ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralConfigV3AbiRust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean V3 generator: {error}"));
    assert!(
        generated.status.success(),
        "General V3 config generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/general_config/generated_v3.rs"))
        .unwrap_or_else(|error| panic!("read generated V3 Rust: {error}"));
    // Normalise before comparing, as the other guards in this tree do: a raw
    // compare holds `committed == emission` and reds the first time anyone runs
    // `tools/lane.sh fmt` on a `do not edit` file, because a direct rustfmt never
    // sees the `#[rustfmt::skip]` that lives in the sibling module.
    let temporary = std::env::temp_dir().join(format!(
        "dclutch-{}-{}.rs",
        env!("CARGO_PKG_NAME"),
        std::process::id()
    ));
    std::fs::write(&temporary, &generated.stdout).expect("write generated Rust");
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
    let formatted = std::fs::read(&temporary).expect("read formatted generated Rust");
    std::fs::remove_file(&temporary).expect("remove generated Rust");
    assert_eq!(formatted, checked_in);
}
