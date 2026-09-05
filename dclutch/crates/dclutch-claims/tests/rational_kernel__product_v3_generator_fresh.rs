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
    let checked_in = std::fs::read(manifest.join("src/rational_kernel/generated_product_v3.rs"))
        .expect("read generated Product Representation V3 ABI");
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

    // Byte-identity alone catches drift between the Lean and the committed
    // file, but not a Lean edit that moves a value and regenerates both. These
    // pin the literals a deployed decoder actually reads, so such an edit
    // fails here rather than at settlement.
    //
    // The kind vocabulary is pinned on both sides of the wire: the codec's
    // `check-generated-runtime-v3.sh` pins the same three values for
    // `DCLTPAY3`, and `kind_tags_agree_with_the_basis_record` proves the
    // equality in Lean. Byte 3 is allocated and refused by
    // `RepresentationAdmissionV3::decode`; pinning it here is what keeps the
    // two authors of the byte from drifting apart on what it means.
    let text = String::from_utf8(checked_in).expect("generated ABI is UTF-8");
    for pinned in [
        "pub const PRODUCT_REPRESENTATION_ADMISSION_VERSION_V3: u16 = 3;",
        "pub const PRODUCT_REPRESENTATION_ADMISSION_BYTES_V3: usize = 528;",
        "pub const ADMISSION_BASIS_KIND_OFFSET_V3: usize = 10;",
        // The degree and its flags travel beside the kind byte, spending two
        // of the five reserved header bytes. Before that spend this receipt
        // carried a kind byte and no degree, so a spline receipt could be
        // written and not read back; `to_bytes` and `decode` are now inverse
        // for every kind. Three reserved bytes survive at 13.
        "pub const ADMISSION_SPLINE_DEGREE_OFFSET_V3: usize = 11;",
        "pub const ADMISSION_SPLINE_FLAGS_OFFSET_V3: usize = 12;",
        "pub const ADMISSION_RESERVED_HEADER_OFFSET_V3: usize = 13;",
        "pub const PRODUCT_REPRESENTATION_CATEGORICAL_KIND_V3: u8 = 1;",
        "pub const PRODUCT_REPRESENTATION_GRADED_KIND_V3: u8 = 2;",
        "pub const PRODUCT_REPRESENTATION_SPLINE_DEGREE_2_TO_3_KIND_V3: u8 = 3;",
    ] {
        assert!(
            text.lines().any(|line| line == pinned),
            "generated ABI no longer declares `{pinned}`"
        );
    }
}
