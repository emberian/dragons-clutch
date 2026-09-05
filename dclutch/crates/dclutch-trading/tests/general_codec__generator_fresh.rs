//! Freshness check for the checked-in Lean-generated Rust module.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn checked_in_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.GeneralControllerAbi"])
        .current_dir(&formal)
        .output()
        .expect("build imported General semantic target");
    assert!(
        build.status.success(),
        "General semantic target build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitGeneralControllerAbiRust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Lean General ABI generator");
    assert!(
        output.status.success(),
        "generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Normalise before comparing, as the forty normalising guards in this tree
    // already do. This compared RAW emitter stdout, so it held
    // `committed == emission` and would have gone red the first time anyone ran
    // `tools/lane.sh fmt` on a `do not edit` file -- a direct rustfmt never sees
    // the `#[rustfmt::skip]` that lives in the sibling module, which is exactly
    // how `generated_transition_programs_v3.rs` went red at `ea4c46e02`.
    // Measured 2026-09-04: the emitter's output at HEAD is byte-identical to the
    // committed file ONCE FORMATTED, so nothing was stale and no re-emission was
    // owed -- only this comparison and the committed file's line wrapping moved.
    let temporary = std::env::temp_dir().join(format!(
        "dclutch-general-controller-{}.rs",
        std::process::id()
    ));
    std::fs::write(&temporary, &output.stdout).expect("write generated Rust");
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
    let checked_in =
        std::fs::read(manifest.join("src/general_codec/generated_general_controller.rs"))
            .expect("read checked-in generated codec");
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
    assert_eq!(
        formatted, checked_in,
        "regenerate the General controller codec"
    );
}
