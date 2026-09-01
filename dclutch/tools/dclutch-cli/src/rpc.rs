//! Finalized, base64 JSON-RPC reads.
//!
//! Deliberately the smallest possible client. This crate reads accounts and
//! nothing else, so it wants none of a full SDK's transaction machinery, and a
//! smaller surface is a smaller thing to audit before someone points it at a
//! cluster.
//!
//! THE ENDPOINT IS TREATED AS A CREDENTIAL. Commercial RPC providers put the
//! API key in the URL path. Nothing in this module ever prints a URL: every
//! refusal names [`origin`] — scheme and host — so a shell transcript, a bug
//! report, or a CI log can carry the failure without carrying the key.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_versioned_message_operator::{Finality, Observation, ObservedAccount};
use serde::{
    Deserialize,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use solana_sdk::{hash::Hash, pubkey::Pubkey};

use crate::{Error, Result};

const GENERAL_RPC_REQUEST_ID_V1: u64 = 73;

#[derive(Clone, Copy)]
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

fn parse_exact_json_v1(bytes: &[u8]) -> Result<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = ExactJsonValueSeedV1
        .deserialize(&mut deserializer)
        .map_err(|error| Error::new(format!("JSON: {error}")))?;
    deserializer
        .end()
        .map_err(|error| Error::new(format!("JSON trailing bytes: {error}")))?;
    Ok(value)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcSuccessEnvelopeV1 {
    jsonrpc: String,
    id: u64,
    result: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorEnvelopeV1 {
    jsonrpc: String,
    id: u64,
    error: RpcErrorBodyV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorBodyV1 {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcContextValueV1<T> {
    context: RpcContextV1,
    value: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcContextV1 {
    slot: u64,
    #[serde(rename = "apiVersion", default)]
    api_version: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcAccountWireV1 {
    lamports: u64,
    owner: String,
    executable: bool,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
    data: [String; 2],
    space: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LatestBlockhashValueV1 {
    blockhash: String,
    last_valid_block_height: u64,
}

struct ExactAccountV1 {
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    data: Vec<u8>,
}

/// Everything the caller is allowed to learn about a fetched account.
pub struct FetchedAccountV1 {
    /// The program that owns the account, base58.
    pub owner: String,
    /// The account's raw data.
    pub data: Vec<u8>,
    /// The slot the cluster answered from.
    pub slot: u64,
}

/// Reacquire an exact nonempty address set in one finalized observation.
///
/// Every account is required, the response width and order are rejoined to the
/// request, account data must use canonical base64 with an exact declared
/// width, and the slot must meet the route's minimum context floor.
pub fn fetch_observed_accounts_v1(
    url: &str,
    addresses: &[Pubkey],
    minimum_slot: u64,
) -> Result<Vec<ObservedAccount>> {
    fetch_observed_accounts_inner_v1(url, addresses, &[], minimum_slot)
}

/// Reacquire one exact address set while preserving named vacant PDAs.
///
/// Solana represents a never-created or already-closed PDA as `null`, whereas
/// the protocol operators represent that same observation as a zero-lamport,
/// empty, System-owned account at the requested address. Only addresses named
/// in `allowed_absent` receive that projection; every other missing account is
/// still a refusal.
pub fn fetch_observed_accounts_allow_absent_v1(
    url: &str,
    addresses: &[Pubkey],
    allowed_absent: &[Pubkey],
    minimum_slot: u64,
) -> Result<Vec<ObservedAccount>> {
    fetch_observed_accounts_inner_v1(url, addresses, allowed_absent, minimum_slot)
}

fn fetch_observed_accounts_inner_v1(
    url: &str,
    addresses: &[Pubkey],
    allowed_absent: &[Pubkey],
    minimum_slot: u64,
) -> Result<Vec<ObservedAccount>> {
    if addresses.is_empty() || addresses.len() > 100 {
        return Err(Error::new(
            "getMultipleAccounts requires one through 100 exact addresses",
        ));
    }
    let mut checked_absent = Vec::with_capacity(allowed_absent.len());
    for address in allowed_absent {
        if !addresses.contains(address) || checked_absent.contains(address) {
            return Err(Error::new(
                "allowed-absent addresses must be unique members of the exact request",
            ));
        }
        checked_absent.push(*address);
    }
    let result = rpc_read_v1(
        url,
        "getMultipleAccounts",
        &json!([addresses.iter().map(ToString::to_string).collect::<Vec<_>>(), {
            "encoding": "base64",
            "commitment": "finalized",
            "minContextSlot": minimum_slot
        }]),
    )?;
    let result: RpcContextValueV1<Vec<Option<RpcAccountWireV1>>> =
        serde_json::from_value(result)
            .map_err(|error| Error::new(format!("getMultipleAccounts result shape: {error}")))?;
    require_context_v1("getMultipleAccounts", &result.context, minimum_slot)?;
    if result.value.len() != addresses.len() {
        return Err(Error::new(
            "getMultipleAccounts response width differed from its exact request",
        ));
    }
    let unix_timestamp = rpc_read_v1(url, "getBlockTime", &json!([result.context.slot]))?
        .as_i64()
        .ok_or_else(|| Error::new("getBlockTime result was not an integer"))?;
    let observation = Observation {
        slot: result.context.slot,
        unix_timestamp,
        finality: Finality::Finalized,
    };
    addresses
        .iter()
        .copied()
        .zip(result.value)
        .map(|(key, account)| {
            project_observed_account_v1(key, account, observation, allowed_absent)
        })
        .collect()
}

fn project_observed_account_v1(
    key: Pubkey,
    account: Option<RpcAccountWireV1>,
    observation: Observation,
    allowed_absent: &[Pubkey],
) -> Result<ObservedAccount> {
    let account = match account {
        Some(account) => parse_exact_account_v1(account)?,
        None if allowed_absent.contains(&key) => ExactAccountV1 {
            lamports: 0,
            owner: Pubkey::default(),
            executable: false,
            data: Vec::new(),
        },
        None => {
            return Err(Error::new(format!(
                "finalized observation missing required account {key}"
            )));
        }
    };
    Ok(ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    })
}

/// Read one recent finalized blockhash at or after the snapshot used by a
/// General plan. The returned hash is routing liveness, never protocol state.
pub fn fetch_latest_finalized_blockhash_v1(url: &str, minimum_slot: u64) -> Result<Hash> {
    let result = rpc_read_v1(
        url,
        "getLatestBlockhash",
        &json!([{
            "commitment": "finalized",
            "minContextSlot": minimum_slot
        }]),
    )?;
    let result: RpcContextValueV1<LatestBlockhashValueV1> = serde_json::from_value(result)
        .map_err(|error| Error::new(format!("getLatestBlockhash result shape: {error}")))?;
    require_context_v1("getLatestBlockhash", &result.context, minimum_slot)?;
    if result.value.last_valid_block_height == 0 {
        return Err(Error::new(
            "getLatestBlockhash returned a zero last-valid block height",
        ));
    }
    let hash = result
        .value
        .blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("getLatestBlockhash blockhash: {error}")))?;
    if hash == Hash::default() {
        return Err(Error::new("getLatestBlockhash returned the zero blockhash"));
    }
    Ok(hash)
}

fn rpc_read_v1(url: &str, method: &str, params: &Value) -> Result<Value> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": GENERAL_RPC_REQUEST_ID_V1,
        "method": method,
        "params": params,
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("dclutch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| Error::new(format!("cannot build an HTTP client: {error}")))?;
    let response = client.post(url).json(&request).send().map_err(|error| {
        Error::new(format!(
            "{} did not answer {method}: {}",
            origin(url),
            redact(&error.to_string(), url)
        ))
    })?;
    let status = response.status();
    let bytes = response.bytes().map_err(|error| {
        Error::new(format!(
            "{} answered {method} but its body could not be read: {}",
            origin(url),
            redact(&error.to_string(), url)
        ))
    })?;
    if !status.is_success() {
        return Err(Error::new(format!(
            "{} answered {method} with HTTP {status}",
            origin(url)
        )));
    }
    parse_rpc_response_v1(method, &bytes)
}

fn parse_rpc_response_v1(method: &str, bytes: &[u8]) -> Result<Value> {
    let body =
        parse_exact_json_v1(bytes).map_err(|error| Error::new(format!("{method} {error}")))?;
    let object = body
        .as_object()
        .ok_or_else(|| Error::new(format!("{method} RPC response was not an object")))?;
    match (object.contains_key("result"), object.contains_key("error")) {
        (true, false) => {
            let envelope: RpcSuccessEnvelopeV1 = serde_json::from_value(body)
                .map_err(|error| Error::new(format!("{method} RPC response shape: {error}")))?;
            require_envelope_v1(method, &envelope.jsonrpc, envelope.id)?;
            Ok(envelope.result)
        }
        (false, true) => {
            let envelope: RpcErrorEnvelopeV1 = serde_json::from_value(body)
                .map_err(|error| Error::new(format!("{method} RPC error shape: {error}")))?;
            require_envelope_v1(method, &envelope.jsonrpc, envelope.id)?;
            let data = envelope
                .error
                .data
                .map(|value| format!(" data {value}"))
                .unwrap_or_default();
            Err(Error::new(format!(
                "{method} RPC error: code {} message {}{data}",
                envelope.error.code, envelope.error.message
            )))
        }
        _ => Err(Error::new(format!(
            "{method} RPC response must carry exactly one of result or error"
        ))),
    }
}

fn require_envelope_v1(method: &str, jsonrpc: &str, id: u64) -> Result<()> {
    if jsonrpc != "2.0" || id != GENERAL_RPC_REQUEST_ID_V1 {
        return Err(Error::new(format!(
            "{method} RPC response did not match its exact version and request id"
        )));
    }
    Ok(())
}

fn require_context_v1(method: &str, context: &RpcContextV1, minimum_slot: u64) -> Result<()> {
    if context.slot < minimum_slot {
        return Err(Error::new(format!(
            "{method} returned a snapshot before the required transaction"
        )));
    }
    if context.api_version.as_deref().is_some_and(str::is_empty) {
        return Err(Error::new(format!("{method} returned an empty apiVersion")));
    }
    Ok(())
}

fn parse_exact_account_v1(value: RpcAccountWireV1) -> Result<ExactAccountV1> {
    let RpcAccountWireV1 {
        lamports,
        owner,
        executable,
        rent_epoch,
        data: [encoded, encoding],
        space,
    } = value;
    // The planner does not project rent epoch into its semantic observation,
    // but requiring and decoding it keeps the RPC account shape exact.
    let _ = rent_epoch;
    if encoding != "base64" {
        return Err(Error::new(
            "account data must be the exact [base64, \"base64\"] tuple",
        ));
    }
    let data = BASE64
        .decode(&encoded)
        .map_err(|error| Error::new(format!("account base64: {error}")))?;
    if BASE64.encode(&data) != encoded || u64::try_from(data.len()).ok() != Some(space) {
        return Err(Error::new("account base64 or declared space was not exact"));
    }
    let parsed_owner = owner
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("account owner: {error}")))?;
    if parsed_owner.to_string() != owner {
        return Err(Error::new("account owner was not canonical base58"));
    }
    Ok(ExactAccountV1 {
        lamports,
        owner: parsed_owner,
        executable,
        data,
    })
}

/// Scheme and host of an endpoint, with the path — and therefore any API key
/// in it — dropped on the floor.
///
/// Falls back to a description rather than the input, because the input is
/// exactly the thing that must not be echoed.
#[must_use]
pub fn origin(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return "the configured endpoint".to_owned();
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("the configured endpoint");
    // A userinfo section (`user:key@host`) is also a credential.
    let host = host.rsplit('@').next().unwrap_or(host);
    if host.is_empty() {
        return "the configured endpoint".to_owned();
    }
    format!("{scheme}://{host}")
}

/// Fetch one finalized account, or refuse with a sentence naming what is
/// missing.
pub fn fetch_account_v1(url: &str, address: &str) -> Result<FetchedAccountV1> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getAccountInfo",
        "params": [address, {"encoding": "base64", "commitment": "finalized"}],
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("dclutch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| Error::new(format!("cannot build an HTTP client: {error}")))?;

    let response = client.post(url).json(&request).send().map_err(|error| {
        Error::new(format!(
            "{} did not answer: {}",
            origin(url),
            redact(&error.to_string(), url)
        ))
    })?;

    let status = response.status();
    let body: serde_json::Value = response.json().map_err(|error| {
        Error::new(format!(
            "{} answered {status} with something that is not JSON: {}",
            origin(url),
            redact(&error.to_string(), url)
        ))
    })?;

    if let Some(rpc_error) = body.get("error") {
        return Err(Error::new(format!(
            "{} refused the read: {}",
            origin(url),
            redact(&rpc_error.to_string(), url)
        )));
    }

    let result = body
        .get("result")
        .ok_or_else(|| Error::new(format!("{} answered without a result", origin(url))))?;

    let slot = result
        .get("context")
        .and_then(|context| context.get("slot"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Error::new(format!("{} answered without a slot", origin(url))))?;

    let value = result
        .get("value")
        .ok_or_else(|| Error::new(format!("{} answered without a value field", origin(url))))?;
    if value.is_null() {
        return Err(Error::new(format!(
            "{address} does not exist on {} as of slot {slot}. \
             Nothing is wrong with the address; there is no account at it.",
            origin(url)
        )));
    }

    let owner = value
        .get("owner")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new(format!("{} answered without an owner", origin(url))))?
        .to_owned();

    let encoded = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|parts| parts.first())
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Error::new(format!(
                "{} answered without base64 account data",
                origin(url)
            ))
        })?;

    Ok(FetchedAccountV1 {
        owner,
        data: crate::decode_base64(encoded)?,
        slot,
    })
}

/// Remove the endpoint URL from a message a library wrote for us.
///
/// `reqwest` puts the full URL in its `Display`, which is exactly the string a
/// provider's API key rides in. We cannot rewrite the library's message, so we
/// substitute the origin for every occurrence of the URL and of its path.
pub fn redact(message: &str, url: &str) -> String {
    let mut redacted = message.replace(url, &origin(url));
    if let Some((_, rest)) = url.split_once("://")
        && let Some(index) = rest.find('/')
        && let Some(path) = rest.get(index..)
        && path.len() > 1
    {
        redacted = redacted.replace(path, "/<redacted>");
    }
    redacted
}

#[cfg(test)]
mod tests {
    use super::{
        GENERAL_RPC_REQUEST_ID_V1, RpcAccountWireV1, origin, parse_exact_account_v1,
        parse_rpc_response_v1, project_observed_account_v1, redact,
    };
    use dclutch_versioned_message_operator::{Finality, Observation};
    use serde_json::json;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn an_origin_keeps_the_host_and_drops_the_path() {
        assert_eq!(
            origin("https://mainnet.helius-rpc.com/?api-key=SECRET"),
            "https://mainnet.helius-rpc.com"
        );
        assert_eq!(
            origin("https://rpc.example.com/v1/SECRET-KEY-HERE"),
            "https://rpc.example.com"
        );
        assert_eq!(
            origin("https://api.devnet.solana.com"),
            "https://api.devnet.solana.com"
        );
    }

    #[test]
    fn userinfo_is_a_credential_too() {
        assert_eq!(
            origin("https://user:hunter2@rpc.example.com/path"),
            "https://rpc.example.com"
        );
    }

    #[test]
    fn a_malformed_endpoint_is_described_never_echoed() {
        assert_eq!(
            origin("SECRET-KEY-WITH-NO-SCHEME"),
            "the configured endpoint"
        );
        assert_eq!(origin("https://"), "the configured endpoint");
    }

    #[test]
    fn a_library_message_carrying_the_url_loses_the_key() {
        let url = "https://rpc.example.com/v1/SECRET-KEY-HERE";
        let message = format!("error sending request for url ({url}): connection refused");
        let redacted = redact(&message, url);
        assert!(
            !redacted.contains("SECRET-KEY-HERE"),
            "the key survived redaction: {redacted}"
        );
        assert!(redacted.contains("https://rpc.example.com"));
    }

    #[test]
    fn a_path_mentioned_without_the_scheme_still_loses_the_key() {
        let url = "https://rpc.example.com/v1/SECRET-KEY-HERE";
        let redacted = redact("failed at /v1/SECRET-KEY-HERE", url);
        assert!(!redacted.contains("SECRET-KEY-HERE"), "{redacted}");
    }

    #[test]
    fn general_rpc_envelopes_refuse_duplicates_substitution_and_extra_truth() {
        let exact = format!(
            r#"{{"jsonrpc":"2.0","id":{GENERAL_RPC_REQUEST_ID_V1},"result":{{"ok":true}}}}"#
        );
        assert_eq!(
            parse_rpc_response_v1("test", exact.as_bytes()).expect("exact result"),
            json!({"ok": true})
        );

        let duplicate = format!(
            r#"{{"jsonrpc":"2.0","id":{GENERAL_RPC_REQUEST_ID_V1},"result":1,"result":2}}"#
        );
        assert!(
            parse_rpc_response_v1("test", duplicate.as_bytes())
                .expect_err("duplicate result")
                .to_string()
                .contains("duplicate")
        );

        let wrong_id = br#"{"jsonrpc":"2.0","id":74,"result":{}}"#;
        assert!(parse_rpc_response_v1("test", wrong_id).is_err());
        let extra = format!(
            r#"{{"jsonrpc":"2.0","id":{GENERAL_RPC_REQUEST_ID_V1},"result":{{}},"extra":true}}"#
        );
        assert!(parse_rpc_response_v1("test", extra.as_bytes()).is_err());
    }

    #[test]
    fn general_account_projection_requires_canonical_base64_and_exact_space() {
        let exact = RpcAccountWireV1 {
            lamports: 7,
            owner: Pubkey::new_from_array([9; 32]).to_string(),
            executable: false,
            rent_epoch: 4,
            data: ["AA==".into(), "base64".into()],
            space: 1,
        };
        let account = parse_exact_account_v1(exact).expect("exact account");
        assert_eq!(account.lamports, 7);
        assert_eq!(account.data, vec![0]);

        let noncanonical = RpcAccountWireV1 {
            lamports: 7,
            owner: Pubkey::new_from_array([9; 32]).to_string(),
            executable: false,
            rent_epoch: 4,
            data: ["AB==".into(), "base64".into()],
            space: 1,
        };
        assert!(parse_exact_account_v1(noncanonical).is_err());

        let wrong_space = RpcAccountWireV1 {
            lamports: 7,
            owner: Pubkey::new_from_array([9; 32]).to_string(),
            executable: false,
            rent_epoch: 4,
            data: ["AA==".into(), "base64".into()],
            space: 2,
        };
        assert!(parse_exact_account_v1(wrong_space).is_err());
    }

    #[test]
    fn only_named_absent_pdas_project_to_exact_vacancy() {
        let key = Pubkey::new_from_array([31; 32]);
        let observation = Observation {
            slot: 7,
            unix_timestamp: 11,
            finality: Finality::Finalized,
        };
        let vacant =
            project_observed_account_v1(key, None, observation, &[key]).expect("named vacant PDA");
        assert_eq!(vacant.key, key);
        assert_eq!(vacant.owner, Pubkey::default());
        assert_eq!(vacant.lamports, 0);
        assert!(!vacant.executable);
        assert!(vacant.data.is_empty());
        assert!(project_observed_account_v1(key, None, observation, &[]).is_err());
    }
}
