//! Exact Lean-generator freshness check for the decision-0012 slot-pin corpus.
//!
//! The corpus is the only thing checking `infrastructure.rs`'s hand-written
//! mirror against `ProtocolInfrastructure.lean`. A corpus that drifted from its
//! emitter would check the mirror against a stale opinion, so it is byte-gated
//! like every other emission in the tree.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_slot_pin_corpus_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.ProtocolInfrastructure"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "ProtocolInfrastructure build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitProtocolInfrastructurePinCorpusRust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "slot-pin corpus generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    let temporary =
        std::env::temp_dir().join(format!("dclutch-slot-pin-corpus-{}.rs", std::process::id()));
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
    let checked_in = std::fs::read(manifest.join("src/generated_slot_pin_corpus.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}
