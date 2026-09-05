//! Lean-owned exposure ABI freshness and hostile-corpus translation checks.

#![allow(clippy::panic)]

#[allow(dead_code, missing_docs)]
#[path = "../src/composition/generated_exposure_abi.rs"]
mod generated;

use std::path::PathBuf;
use std::process::Command;

use dclutch_claims::composition::{
    CompositionExposureBundleV3, CompositionExposureExpectedV3, Error, RecordAdmissionV3,
};

fn admission() -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: [7; 32],
        finalized_id: [7; 32],
        recomputed_digest: [8; 32],
        finalized_digest: [8; 32],
        record_authenticated: true,
    }
}

fn expected(product_width: u32) -> CompositionExposureExpectedV3 {
    CompositionExposureExpectedV3 {
        market: [1; 32],
        result_domain: [2; 32],
        release_set: [3; 32],
        product_basis: [4; 32],
        representation_basis: [5; 32],
        graph_id: [6; 32],
        product_width,
        representation_width: 3,
    }
}

#[test]
fn checked_in_exposure_abi_is_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args([
            "build",
            "DClutchSemantics.ProductRepresentationExposureV3Abi",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "exposure V3 ABI build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated_output = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitProductRepresentationExposureV3AbiRust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated_output.status.success(),
        "exposure V3 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated_output.stdout),
        String::from_utf8_lossy(&generated_output.stderr)
    );

    let temporary = std::env::temp_dir().join(format!(
        "dclutch-exposure-v3-generated-{}.rs",
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
    let formatted =
        std::fs::read(&temporary).unwrap_or_else(|error| panic!("read generated Rust: {error}"));
    std::fs::remove_file(&temporary)
        .unwrap_or_else(|error| panic!("remove generated Rust: {error}"));
    let checked_in = std::fs::read(manifest.join("src/composition/generated_exposure_abi.rs"))
        .unwrap_or_else(|error| panic!("read checked-in generated Rust: {error}"));
    assert_eq!(formatted, checked_in);
}

#[test]
fn lean_constants_are_the_pinned_exposure_coordinates() {
    assert_eq!(generated::COMPOSITION_EXPOSURE_VERSION_LEAN_V3, 3);
    assert_eq!(generated::COMPOSITION_EXPOSURE_MIN_PRODUCT_WIDTH_LEAN_V3, 1);
    assert_eq!(
        generated::COMPOSITION_EXPOSURE_MAX_PRODUCT_WIDTH_LEAN_V3,
        512
    );
    assert_eq!(
        generated::COMPOSITION_EXPOSURE_MAX_REPRESENTATION_WIDTH_LEAN_V3,
        256
    );
    assert_eq!(generated::COMPOSITION_EXPOSURE_MAX_TERMS_LEAN_V3, 65536);
    assert_eq!(generated::COMPOSITION_EXPOSURE_HEADER_BYTES_LEAN_V3, 304);
    assert_eq!(generated::COMPOSITION_EXPOSURE_ROW_BYTES_LEAN_V3, 56);
    assert_eq!(generated::COMPOSITION_EXPOSURE_TERM_BYTES_LEAN_V3, 16);
    assert_eq!(generated::COMPOSITION_EXPOSURE_MAGIC_LEAN_V3, *b"DCRCEX03");
    assert_eq!(
        generated::COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_LEAN_V3,
        b"dclutch/schema/product-representation-exposure-bundle-v3"
    );
    assert_eq!(
        generated::COMPOSITION_EXPOSURE_SCHEMA_ID_LEAN_V3,
        [
            0xc8, 0xbf, 0x29, 0xb9, 0x97, 0x67, 0x94, 0xa7, 0x7d, 0x32, 0xbe, 0xd9, 0xd7, 0xfc,
            0x93, 0x3d, 0xcb, 0xfc, 0x78, 0x75, 0x91, 0x0c, 0x99, 0xc8, 0x0d, 0xe7, 0x18, 0xc3,
            0xc0, 0x10, 0x07, 0x5a
        ]
    );
    assert_eq!(generated::COMPOSITION_EXPOSURE_CAPACITY_PREIMAGE_LEAN_V3, b"dclutch/capacity/product-representation-exposure-v3/product512/representation256/terms65536/u128");
    assert_eq!(
        generated::COMPOSITION_EXPOSURE_CAPACITY_ID_LEAN_V3,
        [
            0x44, 0x0b, 0x9a, 0x61, 0x16, 0x31, 0xa2, 0x3e, 0x68, 0x74, 0xaa, 0x94, 0x54, 0x07,
            0xe2, 0x35, 0x7a, 0xea, 0xab, 0x3f, 0xea, 0x4d, 0xd0, 0xd8, 0xc7, 0x31, 0x00, 0x9b,
            0xdc, 0x83, 0x63, 0x9a
        ]
    );

    assert_eq!(
        [
            generated::COMPOSITION_EXPOSURE_MAGIC_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_VERSION_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_RESERVED_HEADER_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_MARKET_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_RESULT_DOMAIN_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_RELEASE_SET_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_PRODUCT_BASIS_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_REPRESENTATION_BASIS_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_GRAPH_ID_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_CAPACITY_PROFILE_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_PRODUCT_WIDTH_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_REPRESENTATION_WIDTH_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_COUNT_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_RESERVED_TAIL_OFFSET_V3,
        ],
        [
            0, 8, 10, 16, 48, 80, 112, 144, 176, 208, 240, 244, 248, 252, 256
        ]
    );
    assert_eq!(
        [
            generated::COMPOSITION_EXPOSURE_ROW_NODE_ID_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_COORDINATE_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_RANK_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_FIRST_TERM_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_ROW_DENOMINATOR_OFFSET_V3,
        ],
        [0, 32, 36, 40, 44, 48]
    );
    assert_eq!(
        [
            generated::COMPOSITION_EXPOSURE_TERM_PRODUCT_COORDINATE_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_TERM_RESERVED_OFFSET_V3,
            generated::COMPOSITION_EXPOSURE_TERM_NUMERATOR_OFFSET_V3,
        ],
        [0, 4, 8]
    );
}

#[test]
fn lean_witnesses_translate_and_hostile_substitutions_refuse() {
    let n1 = CompositionExposureBundleV3::decode(
        &generated::COMPOSITION_EXPOSURE_K3_N1_WITNESS_V3,
        admission(),
    )
    .and_then(|bundle| bundle.verify_for(expected(1)))
    .expect("Lean K3/N1 witness");
    let mut scratch = [0_u64; 3];
    let mut output = [0_u64; 3];
    n1.translate_product_payouts(&[7], &mut scratch, &mut output)
        .expect("K3/N1 translation");
    assert_eq!(output, [7, 14, 21]);

    let n258 = CompositionExposureBundleV3::decode(
        &generated::COMPOSITION_EXPOSURE_K3_N258_WITNESS_V3,
        admission(),
    )
    .and_then(|bundle| bundle.verify_for(expected(258)))
    .expect("Lean K3/N258 witness");
    let mut payouts = [0_u64; 258];
    payouts[0] = 3;
    payouts[128] = 5;
    payouts[257] = 8;
    n258.translate_product_payouts(&payouts, &mut scratch, &mut output)
        .expect("K3/N258 translation");
    assert_eq!(output, [3, 5, 8]);

    assert_eq!(
        CompositionExposureBundleV3::decode(
            &generated::COMPOSITION_EXPOSURE_RANK_CYCLE_REFUSAL_V3,
            admission(),
        )
        .err(),
        Some(Error::InvalidNode)
    );
    assert_eq!(
        CompositionExposureBundleV3::decode(
            &generated::COMPOSITION_EXPOSURE_WIDTH_REFUSAL_V3,
            admission(),
        )
        .err(),
        Some(Error::NonCanonicalPayoff)
    );
    let transplant = CompositionExposureBundleV3::decode(
        &generated::COMPOSITION_EXPOSURE_RELEASE_TRANSPLANT_REFUSAL_V3,
        admission(),
    )
    .expect("release transplant remains structurally decodable");
    assert_eq!(
        transplant.verify_for(expected(258)).err(),
        Some(Error::ContentAdmission)
    );
}
