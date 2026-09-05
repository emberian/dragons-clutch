//! Thin browser ABI over the authoritative Product V2 payoff evaluator.
//!
//! WHY THIS EXISTS. `apps/dclutch-web/lib/productV2.ts` carried
//! `evaluateProductV2` — a hand-written TypeScript reimplementation of
//! `ProductPayoffV2::evaluate_rational`, with its own `ramp` and its own
//! rational comparison — and the Studio drew a payout curve out of it. Two
//! independent authorities for one piece of exact arithmetic, one of which the
//! chain never runs. The lane that wrote the range-protection check refused to
//! fix a mirror by building a second mirror and left this one named
//! "untouched, unexcused"; this is the removal that sentence was waiting for.
//!
//! This crate owns no arithmetic. It decodes the 576-byte `DCLTPAY2` record
//! with the codec's own `decode`, asks the codec's own `evaluate_rational` for
//! each coordinate, and carries the answers back out. Every payout the browser
//! renders is the same function the on-chain family links.
//!
//! WHAT IT DELIBERATELY DOES NOT DO is take an authored FORM. The input is the
//! record's BYTES, so the boundary evaluates the artifact that would be
//! published rather than the fields someone typed next to it — and a record
//! this crate cannot decode is refused by the codec, by name, before any
//! coordinate is evaluated.
//!
//! The web shell keeps everything this crate must never have: RPC, Wallet
//! Standard, storage, and submission. It has no network of any kind.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_product::payoff::{
    ABI_BYTES_V2, MAGIC_V2, MAX_KNOTS_V2, MAX_TERMS_V2, ProductPayoffV2, VERSION_V2,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Exact JSON schema this boundary accepts. Another one is refused, not guessed.
pub const REQUEST_FORMAT_V1: &str = "dclutch-product-payoff-v2-evaluation-request-v1";
/// Exact JSON schema this boundary returns.
pub const RESPONSE_FORMAT_V1: &str = "dclutch-product-payoff-v2-evaluation-v1";

/// Most coordinates one request may carry.
///
/// A curve is drawn from a bounded sample, and a bound stated here is a bound
/// the browser cannot exceed by accident. **Provisional**, and labelled: it is
/// a transport courtesy rather than a protocol fact, and the lifting plan is
/// that nothing in the codec cares how many times it is called.
pub const MAX_COORDINATES_V1: usize = 4_096;

/// THE CANARY.
///
/// The browser must never write the record width, its magic, or its version
/// down. It reads all three from here, BY CONSTANT NAME, so a rename or a
/// resize in the codec fails this BUILD rather than silently producing a
/// boundary that decodes a record the chain would refuse. A blob can match its
/// digest and still come from a different tree; these are what the loader
/// re-checks it against after loading.
const _: () = assert!(ABI_BYTES_V2 == 576);
const _: () = assert!(MAGIC_V2.len() == 8);
const _: () = assert!(VERSION_V2 == 2);
const _: () = assert!(MAX_KNOTS_V2 == 16 && MAX_TERMS_V2 == 16);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestV1 {
    format: String,
    /// The exact 576-byte `DCLTPAY2` record, base64.
    record_base64: String,
    /// Exact signed-rational coordinates, as decimal text so no float exists.
    coordinates: Vec<CoordinateWireV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoordinateWireV1 {
    numerator: String,
    denominator: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseV1 {
    format: &'static str,
    record_bytes: usize,
    magic: String,
    version: u16,
    product_id: String,
    domain_id: String,
    coordinate_unit_id: String,
    payout_scale: String,
    knot_denominator: String,
    knot_count: u8,
    term_count: u8,
    liability_bound: String,
    payouts: Vec<String>,
}

fn decimal_i128(value: &str, field: &str) -> Result<i128, String> {
    value
        .parse::<i128>()
        .map_err(|_| format!("{field} is not exact i128 decimal text"))
}

fn decimal_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{field} is not exact u64 decimal text"))
}

fn evaluate(request_json: &str) -> Result<String, String> {
    let request: RequestV1 = serde_json::from_str(request_json)
        .map_err(|error| format!("evaluation request is not the exact accepted shape: {error}"))?;
    if request.format != REQUEST_FORMAT_V1 {
        return Err("evaluation request is not the exact accepted format".to_owned());
    }
    if request.coordinates.len() > MAX_COORDINATES_V1 {
        return Err(format!(
            "evaluation request carries {} coordinates, above the {MAX_COORDINATES_V1} this boundary accepts",
            request.coordinates.len()
        ));
    }
    let record = STANDARD
        .decode(request.record_base64.as_bytes())
        .map_err(|_| "product record is not base64".to_owned())?;
    if record.len() != ABI_BYTES_V2 {
        return Err(format!(
            "product record is {} bytes; the canonical record is {ABI_BYTES_V2}",
            record.len()
        ));
    }
    let product = ProductPayoffV2::decode(&record)
        .map_err(|error| format!("product record did not decode: {error:?}"))?;

    let mut payouts = Vec::with_capacity(request.coordinates.len());
    for (index, coordinate) in request.coordinates.iter().enumerate() {
        let numerator = decimal_i128(
            &coordinate.numerator,
            &format!("coordinate {index} numerator"),
        )?;
        let denominator = decimal_u64(
            &coordinate.denominator,
            &format!("coordinate {index} denominator"),
        )?;
        let payout = product
            .evaluate_rational(numerator, denominator)
            .map_err(|error| format!("coordinate {index} refused: {error:?}"))?;
        payouts.push(payout.to_string());
    }

    let response = ResponseV1 {
        format: RESPONSE_FORMAT_V1,
        record_bytes: ABI_BYTES_V2,
        magic: String::from_utf8_lossy(&MAGIC_V2).into_owned(),
        version: VERSION_V2,
        product_id: product.product_id().to_string(),
        domain_id: product.domain_id().to_string(),
        coordinate_unit_id: product.coordinate_unit_id().to_string(),
        payout_scale: product.payout_scale().to_string(),
        knot_denominator: product.knot_denominator().to_string(),
        knot_count: product.knot_count(),
        term_count: product.term_count(),
        liability_bound: product.liability_bound().to_string(),
        payouts,
    };
    serde_json::to_string(&response)
        .map_err(|error| format!("evaluation could not be serialized: {error}"))
}

/// Evaluate one canonical Product V2 record at every coordinate in the request.
///
/// One call per curve rather than one per point: the boundary crossing is the
/// only cost that scales with the sample, and the arithmetic inside is the
/// codec's.
#[wasm_bindgen]
#[must_use]
pub fn evaluate_product_payoff_v2_wasm(request_json: &str) -> String {
    match evaluate(request_json) {
        Ok(response) => response,
        Err(reason) => serde_json::json!({ "error": reason }).to_string(),
    }
}

/// The canonical record width, for the loader's post-load re-check.
#[wasm_bindgen]
#[must_use]
pub fn product_payoff_v2_bytes_v1() -> usize {
    ABI_BYTES_V2
}

/// The canonical record magic, for the loader's post-load re-check.
#[wasm_bindgen]
#[must_use]
pub fn product_payoff_v2_magic_v1() -> String {
    String::from_utf8_lossy(&MAGIC_V2).into_owned()
}

/// The canonical record version, for the loader's post-load re-check.
#[wasm_bindgen]
#[must_use]
pub fn product_payoff_v2_version_v1() -> u16 {
    VERSION_V2
}
