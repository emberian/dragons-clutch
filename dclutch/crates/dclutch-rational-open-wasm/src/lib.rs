//! Thin native/WASM transport for canonical Rational open-family requests.
//!
//! The Rational request contract remains the sole byte-layout owner. This
//! crate accepts one strict bounded JSON value, invokes that owner, and emits
//! the parent-free Trading family together with the exact Claims child under
//! the family's digest. It does not read a chain, select a release, construct
//! account metas, sign, or submit.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod wire;

use wasm_bindgen::prelude::*;

pub use wire::{
    RATIONAL_OPEN_INPUT_FORMAT_V1, RATIONAL_OPEN_PLAN_FORMAT_V1, plan_rational_open_json_v1,
};

/// Compile one exact Rational open family and its canonical Claims child.
#[wasm_bindgen]
pub fn plan_rational_open_v1(source: &str) -> Result<String, JsValue> {
    plan_rational_open_json_v1(source.as_bytes()).map_err(|error| JsValue::from_str(&error))
}
