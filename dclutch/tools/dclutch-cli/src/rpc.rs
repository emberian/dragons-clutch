//! One JSON-RPC call: `getAccountInfo`, finalized, base64.
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

use crate::{Error, Result};

/// Everything the caller is allowed to learn about a fetched account.
pub struct FetchedAccountV1 {
    /// The program that owns the account, base58.
    pub owner: String,
    /// The account's raw data.
    pub data: Vec<u8>,
    /// The slot the cluster answered from.
    pub slot: u64,
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
fn redact(message: &str, url: &str) -> String {
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
    use super::{origin, redact};

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
}
