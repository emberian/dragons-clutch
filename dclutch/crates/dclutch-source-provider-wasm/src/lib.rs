//! Thin native/WASM transport for Source provider transaction planning.
//!
//! The provider-transport operator remains the sole semantic owner. This crate
//! only decodes a strict bounded JSON account observation, invokes that owner,
//! and serializes the owner's exact unsigned message and signer boundary.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod wire;

use wasm_bindgen::prelude::*;
pub use wire::{
    derive_provider_programdata_json_v1, derive_provider_reclaim_coordinates_json_v1,
    derive_provider_submit_base_json_v1, derive_provider_submit_fresh_json_v1,
    derive_provider_submit_material_json_v1, derive_provider_submit_provider_release_json_v1,
    derive_provider_submit_pyth_json_v1, derive_provider_submit_pyth_release_json_v1,
    plan_provider_reclaim_json_v1, plan_provider_submit_json_v1,
    read_sponsored_price_update_json_v1, verify_provider_submit_poststate_json_v1,
};

/// Plan one exact permissionless provider reclaim from finalized chain state.
#[wasm_bindgen]
pub fn plan_source_provider_reclaim_v1(source: &str) -> Result<String, JsValue> {
    plan_provider_reclaim_json_v1(source.as_bytes()).map_err(|error| JsValue::from_str(&error))
}

/// Read one sponsored `PriceUpdateV2` account through the Source family's own decoder.
#[wasm_bindgen]
pub fn read_source_provider_price_update_v1(source: &str) -> Result<String, JsValue> {
    read_sponsored_price_update_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the complete reclaim routing hints from one lifecycle account.
#[wasm_bindgen]
pub fn derive_source_provider_reclaim_coordinates_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_reclaim_coordinates_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Decode one exact Upgradeable Loader Program-to-ProgramData link.
#[wasm_bindgen]
pub fn derive_source_provider_programdata_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_programdata_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the first provider-submit coordinates from one exact Market.
#[wasm_bindgen]
pub fn derive_source_provider_submit_base_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_base_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Continue submit discovery through SourceMaterial and infrastructure.
#[wasm_bindgen]
pub fn derive_source_provider_submit_material_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_material_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the ProviderRelease pair selected by one SourceSpec.
#[wasm_bindgen]
pub fn derive_source_provider_submit_provider_release_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_provider_release_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the Pyth release pair selected by one ProviderRelease.
#[wasm_bindgen]
pub fn derive_source_provider_submit_pyth_release_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_pyth_release_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the exact Receiver and Router frame from Pyth and verified VAA.
#[wasm_bindgen]
pub fn derive_source_provider_submit_pyth_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_pyth_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Derive the lifecycle and Receiver authority for one fresh update signer.
#[wasm_bindgen]
pub fn derive_source_provider_submit_fresh_v1(source: &str) -> Result<String, JsValue> {
    derive_provider_submit_fresh_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}

/// Plan one exact provider submission from one complete finalized frame.
#[wasm_bindgen]
pub fn plan_source_provider_submit_v1(source: &str) -> Result<String, JsValue> {
    plan_provider_submit_json_v1(source.as_bytes()).map_err(|error| JsValue::from_str(&error))
}

/// Reauthenticate the lifecycle and Receiver update created by a submission.
#[wasm_bindgen]
pub fn verify_source_provider_submit_poststate_v1(source: &str) -> Result<String, JsValue> {
    verify_provider_submit_poststate_json_v1(source.as_bytes())
        .map_err(|error| JsValue::from_str(&error))
}
