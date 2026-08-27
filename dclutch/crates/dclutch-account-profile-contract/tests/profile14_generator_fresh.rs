//! Exact Lean-generator freshness check for AccountProfile V2 Profile 14.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use dclutch_account_profile_contract::v2::{
    FIXED_DATA_PREDICATE_PROFILE_ID, FIXED_DATA_PREDICATE_PROFILE_PREIMAGE,
};

#[test]
fn checked_in_profile14_abi_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.AccountProfileV2Profile14"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "Profile14 build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitAccountProfileV2Profile14Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "Profile14 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-profile14-generated-{}.rs",
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
    let checked_in = std::fs::read(manifest.join("src/v2/generated_profile14.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}

#[test]
fn profile14_identity_is_the_preimage_sha256() {
    let temporary =
        std::env::temp_dir().join(format!("dclutch-profile14-preimage-{}", std::process::id()));
    std::fs::write(&temporary, FIXED_DATA_PREDICATE_PROFILE_PREIMAGE)
        .unwrap_or_else(|error| panic!("write profile preimage: {error}"));
    let digest = Command::new("shasum")
        .args(["-a", "256"])
        .arg(&temporary)
        .output()
        .unwrap_or_else(|error| panic!("launch shasum: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove profile preimage: {error}"));
    assert!(digest.status.success());
    let observed = String::from_utf8(digest.stdout)
        .expect("shasum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("shasum digest")
        .to_owned();
    let expected = FIXED_DATA_PREDICATE_PROFILE_ID
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(observed, expected);
}
