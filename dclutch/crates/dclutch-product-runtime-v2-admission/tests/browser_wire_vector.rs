//! The two-sided Product Runtime V2 admission wire vector.
//!
//! `apps/dclutch-web/lib/productRuntimeV2Admission.ts` builds `DCLTPRQ2`
//! requests for `programs/dclutch-product-runtime-v2-sbf`. Its widths, offsets
//! and reserved spans are generated out of this crate's source text, so they
//! cannot be typed wrong -- but a generator only proves the browser read the
//! right numbers, not that it assembled them into the right bytes.
//!
//! This test closes that. One vector file, two independent producers: the Rust
//! encoders here, and the TypeScript encoder in
//! `apps/dclutch-web/lib/productRuntimeV2Admission.test.ts`. If the wire moves,
//! THIS test goes red first -- the authority stays in the crate, and the
//! browser is the side that has to catch up.
//!
//! DERIVATION OF THE VECTOR'S INPUTS, so no field here is a number chosen to
//! make something pass:
//!
//! - The three request digests ARE the three record schema identities
//!   (`PRODUCT_RECORD_SCHEMA_ID_V2`, `RESULT_DOMAIN_SCHEMA_ID_V2`,
//!   `PORTFOLIO_SCHEMA_ID_V2`), each of which is the SHA-256 of a documented
//!   preimage in `src/lib.rs`. Every byte of the request past its header is
//!   therefore traceable to a label, and the vector doubles as a check that the
//!   browser's generated copies of those identities still equal these.
//! - The receipt's per-coordinate content/raw/staging identities are arbitrary
//!   distinct nonzero fills. They are INPUTS to the encoder, not answers: the
//!   assertion is that both languages lay the same inputs out identically.
//!
//! Regenerate with `DCLUTCH_WRITE_WIRE_VECTOR=1 cargo test -p
//! dclutch-product-runtime-v2-admission --test browser_wire_vector`, and only
//! when the wire deliberately moved.

use std::{env, fs, path::PathBuf};

use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, ADMISSION_REQUEST_BYTES_V2, AdmissionReceiptV2,
    AdmissionRequestV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};

const NOTE: &str = "Two-sided wire vector for the LIVE Product Runtime V2 admission ABI. Produced by crates/dclutch-product-runtime-v2-admission/tests/browser_wire_vector.rs and re-produced independently by apps/dclutch-web/lib/productRuntimeV2Admission.test.ts. The Rust crate is the authority: if the wire moves, the Rust test fails first. The three request digests are the three record schema identities, each the SHA-256 of a preimage documented in the crate; the receipt's content/raw/staging identities are arbitrary distinct nonzero fills supplied as encoder INPUTS, never as expected answers. DCLTPRQ2 names two incompatible 112-byte requests -- this is the live admission one, whose bytes 10..16 must be zero, not the dead evaluator one that wrote 1 at byte 10.";

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/dclutch-web/fixtures/product-runtime-v2-admission-wire.json")
}

fn id(schema: [u8; 32]) -> ContentId {
    ContentId::new(schema).expect("documented schema identity is nonzero")
}

fn fill(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("arbitrary nonzero encoder input")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn coordinate(schema: [u8; 32], digest: u8, raw: u8, staging: u8) -> FinalizedRecordCoordinateV2 {
    FinalizedRecordCoordinateV2 {
        schema_id: id(schema),
        content_digest: fill(digest),
        raw_account: fill(raw),
        staging_account: fill(staging),
    }
}

fn canonical_request() -> [u8; ADMISSION_REQUEST_BYTES_V2] {
    let request = AdmissionRequestV2 {
        product_digest: id(PRODUCT_RECORD_SCHEMA_ID_V2),
        result_domain_digest: id(RESULT_DOMAIN_SCHEMA_ID_V2),
        portfolio_digest: id(PORTFOLIO_SCHEMA_ID_V2),
    };
    let mut bytes = [0_u8; ADMISSION_REQUEST_BYTES_V2];
    request.encode_into(&mut bytes).expect("request encodes");
    bytes
}

fn canonical_product_record() -> [u8; PRODUCT_RECORD_BYTES_V2] {
    let record = ProductRecordV2::new(
        fill(0x11),
        id(RESULT_DOMAIN_SCHEMA_ID_V2),
        id(PORTFOLIO_SCHEMA_ID_V2),
    );
    let mut bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    record.encode_into(&mut bytes).expect("Product record encodes");
    bytes
}

fn canonical_receipt() -> [u8; ADMISSION_RECEIPT_BYTES_V2] {
    let receipt = AdmissionReceiptV2 {
        product: coordinate(PRODUCT_RECORD_SCHEMA_ID_V2, 0x21, 0x31, 0x41),
        result_domain: coordinate(RESULT_DOMAIN_SCHEMA_ID_V2, 0x22, 0x32, 0x42),
        portfolio: coordinate(PORTFOLIO_SCHEMA_ID_V2, 0x23, 0x33, 0x43),
    };
    let mut bytes = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    receipt.encode_into(&mut bytes).expect("receipt encodes");
    bytes
}

fn rendered_vector() -> String {
    let request = canonical_request();
    let record = canonical_product_record();
    let receipt = canonical_receipt();
    format!(
        concat!(
            "{{\n",
            "  \"format\": \"dclutch/product-runtime-v2-admission-wire/v1\",\n",
            "  \"note\": \"{note}\",\n",
            "  \"requestHex\": \"{request}\",\n",
            "  \"productRecordHex\": \"{record}\",\n",
            "  \"receiptHex\": \"{receipt}\",\n",
            "  \"inputs\": {{\n",
            "    \"requestDigestsAreSchemaIds\": true,\n",
            "    \"productRecordProductId\": \"{product_id}\",\n",
            "    \"receiptContentDigestFills\": [\"0x21\", \"0x22\", \"0x23\"],\n",
            "    \"receiptRawAccountFills\": [\"0x31\", \"0x32\", \"0x33\"],\n",
            "    \"receiptStagingAccountFills\": [\"0x41\", \"0x42\", \"0x43\"]\n",
            "  }}\n",
            "}}\n"
        ),
        note = NOTE,
        request = hex(&request),
        record = hex(&record),
        receipt = hex(&receipt),
        product_id = hex(&[0x11_u8; 32]),
    )
}

#[test]
fn browser_wire_vector_matches_the_live_encoders() {
    let rendered = rendered_vector();
    let path = vector_path();
    if env::var_os("DCLUTCH_WRITE_WIRE_VECTOR").is_some() {
        fs::write(&path, &rendered).expect("write wire vector");
        return;
    }
    let recorded = fs::read_to_string(&path).expect("wire vector is present");
    assert_eq!(
        recorded, rendered,
        "the Product Runtime V2 admission wire moved; regenerate with DCLUTCH_WRITE_WIRE_VECTOR=1 and update the browser encoder in the same commit"
    );
}

#[test]
fn the_live_request_reserved_span_is_the_dead_encoders_byte() {
    // The collision, stated as an executable fact rather than a comment: the
    // dead DCLTPRQ2 evaluator request wrote 1 at byte 10, and the live decoder
    // refuses exactly that. Same magic, same 112 bytes, incompatible meaning.
    let mut bytes = canonical_request();
    assert!(AdmissionRequestV2::decode(&bytes).is_ok());
    bytes[10] = 1;
    assert!(
        AdmissionRequestV2::decode(&bytes).is_err(),
        "a request carrying the dead evaluator's byte 10 must refuse"
    );
}
