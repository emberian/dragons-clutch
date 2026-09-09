//! Browser/native transport for ordinary Dealer LP and junior equity operations.
//!
//! Every request and instruction is reconstructed by the retained native
//! semantic owners. This transport never reads RPC, signs, or submits.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod discovery;
mod projection;
pub mod wire;

use wasm_bindgen::prelude::*;

pub use discovery::discover_dealer_liquidity_json_v1;
pub use wire::plan_dealer_liquidity_json_v1;

/// Reconstruct one typed intent from its complete finalized account corpus.
#[wasm_bindgen]
pub fn plan_dealer_liquidity_v1(source: &str) -> Result<String, JsValue> {
    plan_dealer_liquidity_json_v1(source).map_err(|error| JsValue::from_str(&error))
}

/// Discover canonical Dealer account coordinates from a bounded chain corpus.
#[wasm_bindgen]
pub fn discover_dealer_liquidity_v1(source: &str) -> Result<String, JsValue> {
    discovery::discover_dealer_liquidity_json_v1(source).map_err(|error| JsValue::from_str(&error))
}
