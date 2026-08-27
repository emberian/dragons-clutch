//! Exact Lean-generator freshness and schema-digest checks for composition V3.

#![allow(clippy::panic)]

#[allow(dead_code, missing_docs)]
#[path = "../src/generated_abi.rs"]
mod generated;

use std::path::{Path, PathBuf};
use std::process::Command;

use dclutch_representation_composition_v3_kernel::{
    CAPACITY_PROFILE_ID_V3, CAPACITY_PROFILE_PREIMAGE_V3, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3, COMPOSITION_GRAPH_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3, COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
    COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3,
};

#[test]
fn checked_in_composition_v3_abi_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.RepresentationCompositionV3Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "composition V3 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated_output = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitRepresentationCompositionV3AbiRust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated_output.status.success(),
        "composition V3 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated_output.stdout),
        String::from_utf8_lossy(&generated_output.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-composition-v3-generated-{}.rs",
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
    let checked_in = std::fs::read(manifest.join("src/generated_abi.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}

fn assert_sha256(preimage: &[u8], expected: [u8; 32], label: &str, directory: &Path) {
    let temporary = directory.join(format!(
        "dclutch-composition-v3-{label}-{}",
        std::process::id()
    ));
    std::fs::write(&temporary, preimage)
        .unwrap_or_else(|error| panic!("write {label} preimage: {error}"));
    let digest = Command::new("shasum")
        .args(["-a", "256"])
        .arg(&temporary)
        .output()
        .unwrap_or_else(|error| panic!("launch shasum for {label}: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove {label} preimage: {error}"));
    assert!(digest.status.success(), "shasum failed for {label}");
    let observed = String::from_utf8(digest.stdout)
        .expect("shasum output is UTF-8")
        .split_whitespace()
        .next()
        .expect("shasum digest")
        .to_owned();
    let expected = expected
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(observed, expected, "{label} schema digest");
}

#[test]
fn schema_and_capacity_ids_are_exact_sha256_preimages() {
    let directory = std::env::temp_dir();
    assert_sha256(
        CAPACITY_PROFILE_PREIMAGE_V3,
        CAPACITY_PROFILE_ID_V3,
        "capacity",
        &directory,
    );
    assert_sha256(
        COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3,
        COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
        "descriptor",
        &directory,
    );
    assert_sha256(
        COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3,
        COMPOSITION_GRAPH_SCHEMA_ID_V3,
        "graph",
        &directory,
    );
    assert_sha256(
        COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3,
        COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
        "translation",
        &directory,
    );

    assert_eq!(
        generated::COMPOSITION_CAPACITY_PROFILE_ID_LEAN_V3,
        CAPACITY_PROFILE_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_SCHEMA_ID_LEAN_V3,
        COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_SCHEMA_ID_LEAN_V3,
        COMPOSITION_GRAPH_SCHEMA_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_SCHEMA_ID_LEAN_V3,
        COMPOSITION_TRANSLATION_SCHEMA_ID_V3
    );
}
