//! Exact Lean-generator freshness check for the General order record V2 layout.
//!
//! The order record and its identity preimage are a persisted wire layout whose
//! every offset derives from Lean (`GeneralOrderV2Abi`), so the emission is
//! byte-gated like every other emission in the tree.
#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_general_order_v2_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralOrderV2Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "GeneralOrderV2Abi build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralOrderV2AbiRust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "General order record V2 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-general-order_v2-{}.rs",
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
    let checked_in = std::fs::read(manifest.join("src/general/generated_order_v2.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    if formatted != checked_in {
        // Printed before the assertion, because `assert_eq!` over two `Vec<u8>`
        // dumps both files as byte vectors and this is the line a reader wants.
        // The assertion itself stays `assert_eq!`: the emission census
        // recognises a Rust guard by `fs::read` plus `assert_eq!`, so replacing
        // it with a `panic!` removes the guard from the census while leaving the
        // test green -- which is exactly what happened in 5fa46416.
        let offset = formatted
            .iter()
            .zip(checked_in.iter())
            .position(|(left, right)| left != right);
        eprintln!(
            "first difference at byte {offset:?}: emitted {} bytes, committed {} bytes. \
             Regenerate it.",
            formatted.len(),
            checked_in.len()
        );
    }
    assert_eq!(formatted, checked_in);
}
