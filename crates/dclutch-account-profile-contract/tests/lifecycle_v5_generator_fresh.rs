//! Exact Lean-generator freshness check for Lifecycle V5 fixed coordinates.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5,
};

#[test]
fn checked_in_lifecycle_v5_abi_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.StateLifecyclePolicyV5Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "Lifecycle V5 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitStateLifecyclePolicyV5AbiRust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "Lifecycle V5 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-lifecycle-v5-generated-{}.rs",
        std::process::id()
    ));
    std::fs::write(&temporary, &generated.stdout)
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
    let checked_in = std::fs::read(manifest.join("src/lifecycle_v3/generated_v5.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}

#[test]
fn lifecycle_v5_schema_id_is_the_preimage_sha256() {
    let temporary = std::env::temp_dir().join(format!(
        "dclutch-lifecycle-v5-preimage-{}",
        std::process::id()
    ));
    std::fs::write(&temporary, CURRENT_RENT_QUOTE_SCHEMA_RELEASE_PREIMAGE_V5)
        .unwrap_or_else(|error| panic!("write schema preimage: {error}"));
    let digest = Command::new("shasum")
        .args(["-a", "256"])
        .arg(&temporary)
        .output()
        .unwrap_or_else(|error| panic!("launch shasum: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove schema preimage: {error}"));
    assert!(
        digest.status.success(),
        "shasum failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&digest.stdout),
        String::from_utf8_lossy(&digest.stderr)
    );
    let observed = String::from_utf8(digest.stdout)
        .expect("shasum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("shasum digest")
        .to_owned();
    let expected = CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(observed, expected);
}
