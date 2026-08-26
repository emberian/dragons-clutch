//! Independent property-space comparison against Lean evaluation.

use std::{path::PathBuf, process::Command};

use dclutch_rational_representation_v2_kernel::{
    Error as KernelError, SCHEMA_VERSION_V2, STRUCTURED_HEADER_BYTES, STRUCTURED_MAGIC_V2,
    StructuredProjectionV2, coalesce, prepare_issue, prepare_reconstitute,
};

fn scalar(fields: &[&str], index: usize) -> u64 {
    fields
        .get(index)
        .expect("corpus field")
        .parse()
        .expect("u64 corpus scalar")
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture offset")
        .copy_from_slice(value);
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn projection(
    denominator: u64,
    receipt_supply: u64,
    coefficient: u64,
    native: u64,
    shards: u64,
    custody: u64,
    free: u64,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; STRUCTURED_HEADER_BYTES + 40];
    put(&mut bytes, 0, &STRUCTURED_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &[1; 32]);
    put(&mut bytes, 48, &[2; 32]);
    put(&mut bytes, 80, &[3; 32]);
    put(&mut bytes, 112, &1_u32.to_le_bytes());
    put_u64(&mut bytes, 120, denominator);
    put_u64(&mut bytes, 128, receipt_supply);
    put_u64(&mut bytes, 136, 0);
    for (index, value) in [coefficient, native, shards, custody, free]
        .iter()
        .enumerate()
    {
        put_u64(&mut bytes, STRUCTURED_HEADER_BYTES + index * 8, *value);
    }
    bytes
}

#[test]
fn rust_kernel_matches_lean_property_corpus() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .current_dir(&formal)
        .args(["build", "DClutchSemantics.RationalRepresentationV2"])
        .output()
        .expect("build exact RationalRepresentationV2 target");
    assert!(
        build.status.success(),
        "Lean semantics build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let emitted = Command::new("lake")
        .current_dir(&formal)
        .args([
            "env",
            "lean",
            "--run",
            "EmitRationalRepresentationV2TranslationCorpus.lean",
        ])
        .output()
        .expect("run Lean property corpus");
    assert!(
        emitted.status.success(),
        "Lean corpus failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&emitted.stdout),
        String::from_utf8_lossy(&emitted.stderr)
    );
    let corpus = String::from_utf8(emitted.stdout).expect("UTF-8 corpus");
    let mut rows = 0_usize;
    for line in corpus.lines() {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        match fields.first().copied().expect("row kind") {
            "C" => {
                let denominator = scalar(&fields, 1);
                let input = scalar(&fields, 2);
                let accepted = scalar(&fields, 3) == 1;
                let result = coalesce(denominator, input);
                assert_eq!(result.is_ok(), accepted, "{line}");
                if let Ok(value) = result {
                    assert_eq!(value.native_claims, scalar(&fields, 4), "{line}");
                    assert_eq!(value.change_shards, scalar(&fields, 5), "{line}");
                    assert_eq!(
                        denominator * value.native_claims + value.change_shards,
                        input,
                        "{line}"
                    );
                }
            }
            kind @ ("S" | "I" | "R") => {
                let denominator = scalar(&fields, 1);
                let receipt_supply = scalar(&fields, 2);
                let coefficient = scalar(&fields, 3);
                let native = scalar(&fields, 4);
                let shards = scalar(&fields, 5);
                let custody = scalar(&fields, 6);
                let free = scalar(&fields, 7);
                let bytes = projection(
                    denominator,
                    receipt_supply,
                    coefficient,
                    native,
                    shards,
                    custody,
                    free,
                );
                let decoded = StructuredProjectionV2::decode(&bytes);
                if kind == "S" {
                    assert_eq!(decoded.is_ok(), scalar(&fields, 8) == 1, "{line}");
                } else {
                    let projection = decoded.expect("action corpus has exact prestate");
                    let quantity = scalar(&fields, 8);
                    let accepted = scalar(&fields, 9) == 1;
                    let result = if kind == "I" {
                        prepare_issue(projection, quantity).map(|_| ())
                    } else {
                        prepare_reconstitute(projection, 0, quantity).map(|_| ())
                    };
                    assert_eq!(result.is_ok(), accepted, "{line}: {result:?}");
                    if !accepted && quantity == 0 {
                        assert_eq!(result, Err(KernelError::ZeroQuantity), "{line}");
                    }
                }
            }
            kind => assert!(kind.is_empty(), "unknown corpus row {kind}"),
        }
        rows += 1;
    }
    assert!(
        rows > 2_000,
        "expected broad deterministic corpus, got {rows}"
    );
}
