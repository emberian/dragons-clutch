//! Exact integer transport at the daemon/browser boundary.
//!
//! JSON numbers are IEEE-754 numbers in the browser. They cannot distinguish
//! every `u64` value, so onchain quantities, slots, counters, and scaled prices
//! leave this daemon as canonical decimal strings. Trade intents use the same
//! spelling. Legacy JSON-number intents are admitted only through JavaScript's
//! largest safe integer; values at or above 2^53 must be strings.

use serde_json::Value;

/// Advertised by Operator identity events and trade responses.
pub const TRANSPORT: &str = "canonical-decimal-v1";

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[must_use]
pub fn u64_value(value: u64) -> Value {
    Value::String(value.to_string())
}

#[must_use]
pub fn u128_value(value: u128) -> Value {
    Value::String(value.to_string())
}

#[must_use]
pub fn u64_values(values: impl IntoIterator<Item = u64>) -> Vec<Value> {
    values.into_iter().map(u64_value).collect()
}

#[must_use]
pub fn optional_u64(value: Option<u64>) -> Value {
    value.map_or(Value::Null, u64_value)
}

fn canonical_decimal(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
}

/// Parse one externally supplied unsigned quantity without a lossy cast.
///
/// Canonical decimal strings cover the whole `u64` domain. JSON numbers are a
/// compatibility lane only below 2^53, where a browser can represent every
/// integer exactly; larger numeric literals are refused even when this one
/// literal happens to be representable.
pub fn parse_u64(value: &Value, role: &str) -> Result<u64, String> {
    if let Some(text) = value.as_str() {
        if !canonical_decimal(text) {
            return Err(format!(
                "{role} must be a canonical unsigned decimal string (no sign or leading zero)"
            ));
        }
        return text
            .parse::<u64>()
            .map_err(|_| format!("{role} exceeds the u64 domain"));
    }
    if let Some(number) = value.as_u64() {
        if number <= MAX_SAFE_JSON_INTEGER {
            return Ok(number);
        }
        return Err(format!(
            "{role} at or above 2^53 must be sent as a canonical decimal string"
        ));
    }
    Err(format!(
        "{role} must be a canonical unsigned decimal string"
    ))
}

pub fn field_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .ok_or_else(|| format!("{field} is required"))
        .and_then(|entry| parse_u64(entry, field))
}

pub fn field_u64_values(value: &Value, field: &str) -> Result<Vec<u64>, String> {
    let entries = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{field} must be an array of canonical unsigned decimals"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_u64(entry, &format!("{field}[{index}]")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn full_width_u64_values_emit_as_exact_decimal_strings() {
        for value in [1_u64 << 53, (1_u64 << 53) + 1, u64::MAX] {
            assert_eq!(u64_value(value), json!(value.to_string()));
            assert_eq!(parse_u64(&json!(value.to_string()), "quantity"), Ok(value));
        }
        assert_eq!(
            u128_value(u128::MAX),
            json!("340282366920938463463374607431768211455")
        );
    }

    #[test]
    fn legacy_numbers_stop_before_the_javascript_precision_cliff() {
        assert_eq!(
            parse_u64(&json!(MAX_SAFE_JSON_INTEGER), "quantity"),
            Ok(MAX_SAFE_JSON_INTEGER)
        );
        assert!(parse_u64(&json!(1_u64 << 53), "quantity")
            .unwrap_err()
            .contains("canonical decimal string"));
        assert!(parse_u64(&json!((1_u64 << 53) + 1), "quantity").is_err());
        assert!(parse_u64(&json!(u64::MAX), "quantity").is_err());
    }

    #[test]
    fn malformed_or_partially_valid_vectors_are_refused_whole() {
        for value in [
            json!(""),
            json!("01"),
            json!("+1"),
            json!("-1"),
            json!(" 1"),
            json!("1e3"),
            json!("١"),
            json!(1.0),
            json!(1.5),
        ] {
            assert!(parse_u64(&value, "quantity").is_err());
        }
        assert!(field_u64_values(
            &json!({"belief": ["1", "9007199254740993", "2x"]}),
            "belief"
        )
        .is_err());
    }
}
