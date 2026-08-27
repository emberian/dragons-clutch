//! Exact Lean-generator freshness and schema pin checks for DCE5.

#![allow(clippy::panic)]

#[allow(dead_code, missing_docs)]
#[path = "../src/generated_v4_abi.rs"]
mod generated;

use std::path::PathBuf;
use std::process::Command;

use dclutch_effect_kernel::v4::{SCHEMA_RELEASE_ID_V4, SCHEMA_RELEASE_PREIMAGE_V4};

#[test]
fn checked_in_effect_v4_abi_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.EffectProgramV4Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "DCE5 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated_output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitEffectProgramV4AbiRust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated_output.status.success(),
        "DCE5 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated_output.stdout),
        String::from_utf8_lossy(&generated_output.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-effect-v4-generated-{}.rs",
        std::process::id()
    ));
    std::fs::write(&temporary, &generated_output.stdout)
        .unwrap_or_else(|error| panic!("write generated Rust: {error}"));
    let formatted = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .arg(&temporary)
        .output()
        .unwrap_or_else(|error| panic!("launch rustfmt: {error}"));
    assert!(
        formatted.status.success(),
        "rustfmt failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&formatted.stdout),
        String::from_utf8_lossy(&formatted.stderr)
    );
    let formatted = std::fs::read(&temporary)
        .unwrap_or_else(|error| panic!("read formatted generated Rust: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove generated Rust: {error}"));
    let checked_in = std::fs::read(manifest.join("src/generated_v4_abi.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}

#[test]
fn effect_v4_schema_id_is_the_exact_sha256_preimage() {
    let temporary =
        std::env::temp_dir().join(format!("dclutch-effect-v4-schema-{}", std::process::id()));
    std::fs::write(&temporary, SCHEMA_RELEASE_PREIMAGE_V4)
        .unwrap_or_else(|error| panic!("write schema preimage: {error}"));
    let digest = Command::new("shasum")
        .args(["-a", "256"])
        .arg(&temporary)
        .output()
        .unwrap_or_else(|error| panic!("launch shasum: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove schema preimage: {error}"));
    assert!(digest.status.success(), "shasum failed");
    let observed = String::from_utf8(digest.stdout)
        .expect("shasum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("shasum digest")
        .to_owned();
    let expected = SCHEMA_RELEASE_ID_V4
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(observed, expected);
    assert_eq!(
        generated::EFFECT_V4_SCHEMA_RELEASE_ID_LEAN,
        SCHEMA_RELEASE_ID_V4
    );
}
