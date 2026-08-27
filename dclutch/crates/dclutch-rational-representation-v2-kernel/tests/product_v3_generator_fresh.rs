//! Exact freshness check for the Lean-owned Product Representation V3 ABI.

use std::{path::PathBuf, process::Command};

#[test]
fn generated_product_representation_v3_abi_is_exact() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args(["build", "DClutchSemantics.ProductRepresentationV3Abi"])
        .output()
        .expect("build exact Product Representation V3 ABI target");
    assert!(
        build.status.success(),
        "Product Representation V3 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .current_dir(&formal)
        .args([
            "env",
            "lean",
            "--run",
            "EmitProductRepresentationV3AbiRust.lean",
        ])
        .output()
        .expect("run exact Product Representation V3 ABI generator");
    assert!(
        generated.status.success(),
        "Product Representation V3 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_product_v3.rs"))
        .expect("read generated Product Representation V3 ABI");
    assert_eq!(generated.stdout, checked_in);
}
