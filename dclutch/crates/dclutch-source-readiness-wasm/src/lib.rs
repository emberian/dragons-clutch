//! Thin browser ABI over the authoritative Source-readiness operator.
//!
//! This crate owns no layout, routing, or authority decision. It transports
//! one strict JSON snapshot into the native owner and returns that owner's
//! canonical JSON plan unchanged.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_source_readiness_operator::{
    derive_source_close_detail_json_v1, derive_source_readiness_base_json_v1,
    derive_source_readiness_detail_json_v1, derive_source_readiness_recovery_json_v1,
    derive_source_terminal_base_json_v1, derive_source_terminal_detail_json_v1,
    derive_source_terminal_product_json_v1, plan_funding_readiness_json_v1,
    plan_source_close_fund_json_v1, plan_source_terminal_json_v1,
    verify_source_close_receipt_json_v1,
};
use wasm_bindgen::prelude::*;

/// Plan one adjacent Source-readiness action from one finalized observation.
#[wasm_bindgen]
pub fn plan_source_readiness_v1(snapshot_json: &str) -> Result<String, JsValue> {
    plan_funding_readiness_json_v1(snapshot_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the first account frame from one exact Core Market.
#[wasm_bindgen]
pub fn derive_source_readiness_base_v1(market_json: &str) -> Result<String, JsValue> {
    derive_source_readiness_base_json_v1(market_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the recovery pair and Resolution subset ledger from exact records.
#[wasm_bindgen]
pub fn derive_source_readiness_detail_v1(records_json: &str) -> Result<String, JsValue> {
    derive_source_readiness_detail_json_v1(records_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the optional recovery-policy pair after reading SourceMaterialV3.
#[wasm_bindgen]
pub fn derive_source_readiness_recovery_v1(source_json: &str) -> Result<String, JsValue> {
    derive_source_readiness_recovery_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the initial terminal-admission Source and Product-root coordinates.
#[wasm_bindgen]
pub fn derive_source_terminal_base_v1(source_json: &str) -> Result<String, JsValue> {
    derive_source_terminal_base_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive Product child coordinates from the selected Product root.
#[wasm_bindgen]
pub fn derive_source_terminal_product_v1(source_json: &str) -> Result<String, JsValue> {
    derive_source_terminal_product_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the certificate from exact Source and ResultDomain bytes.
#[wasm_bindgen]
pub fn derive_source_terminal_detail_v1(source_json: &str) -> Result<String, JsValue> {
    derive_source_terminal_detail_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Plan terminal admission or prove exact already-admitted completion.
#[wasm_bindgen]
pub fn plan_source_terminal_v1(source_json: &str) -> Result<String, JsValue> {
    plan_source_terminal_json_v1(source_json.as_bytes()).map_err(|error| JsValue::from_str(&error))
}

/// Derive the admitted terminal certificate and Source closure receipt.
#[wasm_bindgen]
pub fn derive_source_close_detail_v1(source_json: &str) -> Result<String, JsValue> {
    derive_source_close_detail_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Plan exact receipt prepayment or the signer-free V7 direct close.
#[wasm_bindgen]
pub fn plan_source_close_fund_v1(source_json: &str) -> Result<String, JsValue> {
    plan_source_close_fund_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Authenticate one finalized Source closure receipt against persisted plan facts.
#[wasm_bindgen]
pub fn verify_source_close_receipt_v1(source_json: &str) -> Result<String, JsValue> {
    verify_source_close_receipt_json_v1(source_json.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}
