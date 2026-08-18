//! The `rust-reference` executor: it runs a vector against the landed crates.
//!
//! Every mapping from an implementation variant to a taxonomy code lives here,
//! in the checker, not in the semantic crates.  TAX-3 forbids serializing an
//! enum discriminant, and the ownership rule of §6 forbids a vector depending
//! on a crate, so the translation belongs to a third party that depends on
//! both: this one.

pub mod accumulator;
pub mod adapter;
pub mod batch;
pub mod kernel;

use crate::json::Value;
use crate::taxonomy::Observed;

/// One surface's live state, driven step by step.
pub trait Executor {
    /// Run one operation.  `Err` is a *checker* failure (an unknown op, a
    /// malformed argument); an implementation refusal is `Ok(Observed::Error)`.
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String>;

    /// The state in the vector's own language-neutral form, for `post_state`
    /// and `final_state` comparison.
    fn render_state(&self) -> Value;
}

/// Build the executor a state form names.
pub fn open(form: &str, constructed_by: &str, value: &Value) -> Result<Box<dyn Executor>, String> {
    match form {
        "kernel.market-position/v1" => Ok(Box::new(kernel::KernelExecutor::open(
            constructed_by,
            value,
        )?)),
        "batch.book/v1" => Ok(Box::new(batch::ScalarExecutor::open(
            constructed_by,
            value,
        )?)),
        "batch.relation-v1/v1" => Ok(Box::new(batch::RelationExecutor::open(
            constructed_by,
            value,
        )?)),
        "accumulator.window/v1" => Ok(Box::new(accumulator::WindowExecutor::open(
            constructed_by,
            value,
        )?)),
        "adapter.reference-transition/v1" => Ok(Box::new(adapter::AdapterExecutor::open(
            constructed_by,
            value,
        )?)),
        other => Err(format!(
            "no rust-reference executor for state form {other:?}"
        )),
    }
}

// ---------------------------------------------------------------- helpers ---

pub fn field<'a>(args: &'a Value, key: &str) -> Result<&'a Value, String> {
    args.require(key)
}

pub fn u64_field(args: &Value, key: &str) -> Result<u64, String> {
    field(args, key)?.as_u64()
}

pub fn u128_field(args: &Value, key: &str) -> Result<u128, String> {
    field(args, key)?.as_u128()
}

pub fn small_field(args: &Value, key: &str) -> Result<u64, String> {
    field(args, key)?.as_small()
}

pub fn str_field<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    field(args, key)?.as_str()
}

pub fn dec(value: u128) -> Value {
    Value::Str(value.to_string())
}

pub fn small(value: u64) -> Value {
    Value::Int(value as i64)
}

pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = std::collections::BTreeMap::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

/// ARR-1: a fixed array ships as exactly its active prefix.
pub fn prefix(values: &[u64], active: usize) -> Value {
    Value::Array(
        values[..active]
            .iter()
            .map(|v| dec(u128::from(*v)))
            .collect(),
    )
}

/// ARR-1 / ARR-4 in the reading direction: a list must be exactly the declared
/// active length, and the inactive tail is the type's zero (ARR-2).
pub fn read_prefix<const N: usize>(value: &Value, active: usize) -> Result<[u64; N], String> {
    let items = value.as_array()?;
    if items.len() != active {
        return Err(format!(
            "ARR-1: expected exactly {active} active entries, found {}",
            items.len()
        ));
    }
    let mut out = [0u64; N];
    for (index, item) in items.iter().enumerate() {
        out[index] = item.as_u64()?;
    }
    Ok(out)
}

pub fn read_hash32(value: &Value) -> Result<[u8; 32], String> {
    let text = value.as_str()?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "BYTE-1: expected 64 lowercase hex characters, found {text:?}"
        ));
    }
    let mut out = [0u8; 32];
    for index in 0..32 {
        out[index] = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid hex".to_string())?;
    }
    Ok(out)
}

/// The comparison of COMP-1 and COMP-7: every fact the vector names must be
/// present and equal in what the executor produced.  A value the vector does
/// not name is not asserted, so the count of asserted leaves is reported and a
/// vector that names nothing can never look like agreement.
pub fn named_fact_match(expected: &Value, observed: &Value, path: &str) -> Result<usize, String> {
    match (expected, observed) {
        (Value::Object(want), Value::Object(have)) => {
            let mut leaves = 0;
            for (key, value) in want {
                let found = have.get(key).ok_or_else(|| {
                    format!("{path}.{key}: the vector names this fact, the executor produced none")
                })?;
                leaves += named_fact_match(value, found, &format!("{path}.{key}"))?;
            }
            Ok(leaves)
        }
        (Value::Array(want), Value::Array(have)) => {
            if want.len() != have.len() {
                return Err(format!(
                    "{path}: expected {} entries, executor produced {}",
                    want.len(),
                    have.len()
                ));
            }
            let mut leaves = 0;
            for (index, (value, found)) in want.iter().zip(have.iter()).enumerate() {
                leaves += named_fact_match(value, found, &format!("{path}[{index}]"))?;
            }
            Ok(leaves)
        }
        (want, have) if want == have => Ok(1),
        (want, have) => Err(format!(
            "{path}: expected {}, executor produced {}",
            want.to_jcs(),
            have.to_jcs()
        )),
    }
}
