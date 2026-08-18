//! A minimal JSON reader and an RFC 8785 (JCS) canonical writer.
//!
//! Hand-written for the same reason as `sha256`: the semantic crates are
//! offline and dependency-free, and a vector reader that needed a registry
//! fetch would be a worse dependency than a few hundred lines of parser.
//!
//! The reader is strict where `VECTOR_SPINE_PROPOSAL.md` §3.2 is strict: it
//! refuses any number carrying a fraction or an exponent (INT-3), so a vector
//! can never smuggle a float past it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A parsed JSON value.
///
/// Numbers are kept as `i64` because INT-2 bounds every JSON number in a vector
/// to 65535; INT-1 quantities arrive as `Str`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Value>),
    /// Ordered by key so a decoded value re-serializes canonically.
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "integer",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    pub fn as_object(&self) -> Result<&BTreeMap<String, Value>, String> {
        match self {
            Value::Object(map) => Ok(map),
            other => Err(format!("expected object, found {}", other.kind())),
        }
    }

    pub fn as_array(&self) -> Result<&[Value], String> {
        match self {
            Value::Array(items) => Ok(items),
            other => Err(format!("expected array, found {}", other.kind())),
        }
    }

    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            Value::Str(text) => Ok(text),
            other => Err(format!("expected string, found {}", other.kind())),
        }
    }

    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(flag) => Ok(*flag),
            other => Err(format!("expected boolean, found {}", other.kind())),
        }
    }

    /// INT-2: a bounded structural integer.
    pub fn as_small(&self) -> Result<u64, String> {
        match self {
            Value::Int(value) if *value >= 0 && *value <= 65535 => Ok(*value as u64),
            Value::Int(value) => Err(format!("integer {value} is outside INT-2's 0..=65535")),
            other => Err(format!("expected bounded integer, found {}", other.kind())),
        }
    }

    /// INT-1: an exact protocol quantity, carried as a decimal string.
    pub fn as_u128(&self) -> Result<u128, String> {
        let text = self.as_str()?;
        if text.is_empty() {
            return Err("empty decimal string".into());
        }
        if text.len() > 1 && text.starts_with('0') {
            return Err(format!("INT-1 forbids leading zeros: {text:?}"));
        }
        if !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("INT-1 requires exact decimal digits: {text:?}"));
        }
        text.parse::<u128>()
            .map_err(|_| format!("decimal string {text:?} exceeds u128"))
    }

    pub fn as_u64(&self) -> Result<u64, String> {
        let wide = self.as_u128()?;
        u64::try_from(wide).map_err(|_| format!("decimal string {wide} exceeds u64"))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn require(&self, key: &str) -> Result<&Value, String> {
        self.get(key)
            .ok_or_else(|| format!("missing required key {key:?}"))
    }

    /// A copy with one top-level key removed; the digest rules of §3.5 hash the
    /// object with its own `digests` member removed.
    pub fn without(&self, key: &str) -> Value {
        match self {
            Value::Object(map) => {
                let mut copy = map.clone();
                copy.remove(key);
                Value::Object(copy)
            }
            other => other.clone(),
        }
    }

    /// RFC 8785 canonical serialization.
    ///
    /// Object keys sort by UTF-16 code unit, there is no insignificant
    /// whitespace, and every number here is an integer, so ECMAScript number
    /// serialization degenerates to plain decimal.
    pub fn to_jcs(&self) -> String {
        let mut out = String::new();
        self.write_jcs(&mut out);
        out
    }

    fn write_jcs(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Int(value) => {
                let _ = write!(out, "{value}");
            }
            Value::Str(text) => write_json_string(text, out),
            Value::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write_jcs(out);
                }
                out.push(']');
            }
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort_by(|left, right| utf16_order(left, right));
                out.push('{');
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_json_string(key, out);
                    out.push(':');
                    map[key].write_jcs(out);
                }
                out.push('}');
            }
        }
    }
}

fn utf16_order(left: &str, right: &str) -> std::cmp::Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parse one JSON document, refusing trailing content.
pub fn parse(text: &str) -> Result<Value, String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut reader = Reader {
        chars: &bytes,
        at: 0,
    };
    reader.skip_whitespace();
    let value = reader.value()?;
    reader.skip_whitespace();
    if reader.at != reader.chars.len() {
        return Err(format!("trailing content at character {}", reader.at));
    }
    Ok(value)
}

struct Reader<'a> {
    chars: &'a [char],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.at += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while matches!(
            self.peek(),
            Some(' ') | Some('\t') | Some('\n') | Some('\r')
        ) {
            self.at += 1;
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        if self.bump() == Some(ch) {
            Ok(())
        } else {
            Err(format!("expected {ch:?} at character {}", self.at))
        }
    }

    fn literal(&mut self, text: &str) -> Result<(), String> {
        for ch in text.chars() {
            self.expect(ch)?;
        }
        Ok(())
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Value::Str(self.string()?)),
            Some('t') => {
                self.literal("true")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.literal("false")?;
                Ok(Value::Bool(false))
            }
            Some('n') => {
                self.literal("null")?;
                Ok(Value::Null)
            }
            Some(ch) if ch == '-' || ch.is_ascii_digit() => self.number(),
            Some(ch) => Err(format!("unexpected {ch:?} at character {}", self.at)),
            None => Err("unexpected end of input".into()),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.expect('{')?;
        let mut map = BTreeMap::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.at += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            self.expect(':')?;
            self.skip_whitespace();
            let value = self.value()?;
            if map.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate object key {key:?}"));
            }
            self.skip_whitespace();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(Value::Object(map)),
                other => return Err(format!("expected ',' or '}}', found {other:?}")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value()?);
            self.skip_whitespace();
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(Value::Array(items)),
                other => return Err(format!("expected ',' or ']', found {other:?}")),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("unterminated string".into()),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{08}'),
                    Some('f') => out.push('\u{0c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let code = self.hex4()?;
                        match char::from_u32(u32::from(code)) {
                            Some(ch) => out.push(ch),
                            None => return Err("surrogate escapes are not supported".into()),
                        }
                    }
                    other => return Err(format!("invalid escape {other:?}")),
                },
                Some(ch) if (ch as u32) < 0x20 => {
                    return Err(format!("raw control character {:#x} in string", ch as u32))
                }
                Some(ch) => out.push(ch),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, String> {
        let mut value = 0u16;
        for _ in 0..4 {
            let ch = self.bump().ok_or("truncated \\u escape")?;
            let digit = ch.to_digit(16).ok_or("invalid \\u escape")?;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        if self.peek() == Some('-') {
            self.at += 1;
        }
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.at += 1;
        }
        // INT-3: no fraction, no exponent, anywhere, ever.
        if matches!(self.peek(), Some('.') | Some('e') | Some('E')) {
            return Err("INT-3 forbids fractional and exponential numbers".into());
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("integer {text:?} is out of range"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_sorts_keys_and_drops_whitespace() {
        let value = parse(r#"{ "b": 1, "a": [ 2, "x" ] }"#).expect("parses");
        assert_eq!(value.to_jcs(), r#"{"a":[2,"x"],"b":1}"#);
    }

    #[test]
    fn floats_and_duplicate_keys_are_refused() {
        assert!(parse("1.5").is_err());
        assert!(parse("1e3").is_err());
        assert!(parse(r#"{"a":1,"a":2}"#).is_err());
    }

    #[test]
    fn decimal_strings_are_exact_and_strict() {
        assert_eq!(
            Value::Str("18446744073709551615".into()).as_u64(),
            Ok(u64::MAX)
        );
        assert!(Value::Str("007".into()).as_u128().is_err());
        assert!(Value::Str("-1".into()).as_u128().is_err());
        assert!(Value::Int(7).as_u128().is_err());
    }

    #[test]
    fn jcs_orders_keys_by_utf16_code_unit() {
        // The JCS worked example: the astral key sorts after the BMP keys.
        let value = parse("{\"\u{20ac}\":1,\"a\":2,\"\u{10336}\":3}").expect("parses");
        assert_eq!(value.to_jcs(), "{\"a\":2,\"\u{20ac}\":1,\"\u{10336}\":3}");
    }
}
