//! One JSON value, with duplicate object keys refused rather than collapsed.
//!
//! WHY THIS IS NOT `serde_json::from_slice::<Value>`. That reader keeps the
//! LAST of two identical keys and reports nothing, so a ticket carrying both
//! `"maximumFill": "1000"` and `"maximumFill": "1001"` would reopen as one of
//! the two and the reader could not say which one the signer meant. A ticket is
//! read hostilely or not at all.
//!
//! NAMED DEBT, so nobody discovers it as a surprise: this is the THIRD copy of
//! this reader in the tree. The other two are
//! `tools/local-validator/bootstrap/successor/src/rpc.rs` and that crate's
//! `campaign.rs`, where it reads RPC responses and plan files. Collapsing all
//! three is a mechanical lane that this one deliberately did not open, because
//! a Direct-ticket crate is the wrong owner for an RPC JSON facility -- the
//! right owner is a small crate that does not exist yet.

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use crate::{Error, Result};

/// Parse exactly one JSON value out of `bytes`, refusing duplicate object keys
/// anywhere in it and refusing trailing bytes after it.
pub fn parse_json_without_duplicate_keys_v1(bytes: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| Error::new(format!("JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| Error::new(format!("JSON trailing bytes: {error}")))?;
    Ok(value)
}

struct ExactJsonValueSeedV1;

impl<'de> DeserializeSeed<'de> for ExactJsonValueSeedV1 {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> core::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ExactJsonValueVisitorV1)
    }
}

struct ExactJsonValueVisitorV1;

impl<'de> Visitor<'de> for ExactJsonValueVisitorV1 {
    type Value = Value;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("one JSON value with no duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> core::result::Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> core::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> core::result::Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> core::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON number was not finite"))
    }

    fn visit_str<E>(self, value: &str) -> core::result::Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> core::result::Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> core::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExactJsonValueSeedV1.deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(ExactJsonValueSeedV1)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::with_capacity(map.size_hint().unwrap_or(0));
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            let value = map.next_value_seed(ExactJsonValueSeedV1)?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_json_without_duplicate_keys_v1;

    #[test]
    fn a_duplicate_key_is_refused_rather_than_resolved() {
        let error = parse_json_without_duplicate_keys_v1(br#"{"a": 1, "a": 2}"#)
            .expect_err("a duplicate key must not silently resolve");
        assert!(format!("{error}").contains("duplicate JSON object key"));
    }

    #[test]
    fn a_duplicate_key_nested_inside_an_object_is_refused_too() {
        let error = parse_json_without_duplicate_keys_v1(br#"{"intent": {"a": 1, "a": 2}}"#)
            .expect_err("a nested duplicate key must not silently resolve");
        assert!(format!("{error}").contains("duplicate JSON object key"));
    }

    #[test]
    fn trailing_bytes_after_the_value_are_refused() {
        let error = parse_json_without_duplicate_keys_v1(br#"{"a": 1} {"b": 2}"#)
            .expect_err("a second value must not be ignored");
        assert!(format!("{error}").contains("trailing bytes"));
    }
}
