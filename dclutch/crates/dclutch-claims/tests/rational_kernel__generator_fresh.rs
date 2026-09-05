//! Exact clean-build freshness for the Lean-owned descriptor layout.

use std::{path::PathBuf, process::Command};

#[test]
fn generated_descriptor_is_exact() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args([
            "build",
            "DClutchSemantics.RationalRepresentationV2PhysicalAbi",
        ])
        .output()
        .expect("build exact imported descriptor ABI target");
    assert!(
        build.status.success(),
        "descriptor ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .current_dir(&formal)
        .args([
            "env",
            "lean",
            "--run",
            "EmitRationalRepresentationV2DescriptorRust.lean",
        ])
        .output()
        .expect("run exact descriptor generator");
    assert!(
        generated.status.success(),
        "descriptor generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/rational_kernel/generated_descriptor.rs"))
        .expect("read generated ABI");
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
