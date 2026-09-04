//! Exact Lean-generator freshness check for General transition programs.
//!
//! The byte gate in `transition_artifacts_v3.rs` proves the Rust builder agrees
//! with the CHECKED-IN arrays. This proves the checked-in arrays are still what
//! the Lean module emits. Neither is sufficient alone: without this one, an edit
//! to `GeneralTransitionV3.lean` that nobody regenerated leaves two agreeing
//! Rust authorities and one silent Lean one.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_general_transition_programs_are_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralTransitionV3"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "General transition-program build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralTransitionV3Rust.lean"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "General transition-program generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );

    // Normalise before comparing, exactly as the other three generator-fresh
    // tests in this crate do, and as all thirty `check-generated.sh` scripts in
    // this tree do with their `rustfmt --edition 2024` line before `cmp`.
    //
    // This was the LAST of the four that compared RAW emitter stdout. The
    // emitter prints twelve bytes per line; `rustfmt.toml` -- the tree's one
    // formatting authority -- packs sixteen. So the moment `ea4c46e02`
    // formatted the crate, the committed file became rustfmt output and this
    // test went red on line packing and nothing else. Verified 2026-09-04:
    // whitespace-stripped, the emitter's output at HEAD and the committed file
    // have the same digest, and `rustfmt --edition 2024` over the raw emission
    // reproduces the committed bytes exactly -- so neither the Lean nor the
    // emission was ever stale and no re-emission was owed. `513f0d8e6` made
    // this same finding for `request_profiles_generator_fresh` two days
    // earlier; this file is the remaining sibling it named.
    let temporary = std::env::temp_dir().join(format!(
        "dclutch-general-transition-programs-{}.rs",
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
    let checked_in = std::fs::read(manifest.join("src/generated_transition_programs_v3.rs"))
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
