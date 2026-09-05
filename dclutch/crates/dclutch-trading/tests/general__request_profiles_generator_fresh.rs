//! Exact Lean-generator freshness check for General request profiles.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_general_request_profiles_are_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralRequestProfilesV1"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "General request-profile build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitGeneralRequestProfilesV1Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "General request-profile generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    // Normalise before comparing, exactly as `runtime_wire_v2_generator_fresh`
    // and `selection_decision_corpus_generator_fresh` do in this same crate.
    //
    // This test compared RAW emitter stdout, and the committed file is rustfmt
    // output -- the emitter prints each corpus record on one line and rustfmt
    // wraps it at sixteen bytes -- so it was red at clean HEAD on a formatting
    // difference and nothing else. Verified 2026-09-02: the emitter's output at
    // HEAD is byte-identical to the committed file ONCE FORMATTED, so neither
    // the Lean nor the emission was stale and no re-emission was owed. Two of
    // the four generator-fresh tests in this crate already normalised; this was
    // one of the two that did not.
    let temporary = std::env::temp_dir().join(format!(
        "dclutch-general-request-profiles-{}.rs",
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
    let checked_in = std::fs::read(manifest.join("src/general/generated_request_profiles_v1.rs"))
        .unwrap_or_else(|error| panic!("read generated Rust: {error}"));
    if formatted != checked_in {
        // Printed before the assertion, because `assert_eq!` over two `Vec<u8>`
        // dumps both files as byte vectors and this is the line a reader wants.
        // The assertion itself stays `assert_eq!`: the emission census
        // recognises a Rust guard by `fs::read` plus `assert_eq!`.
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
