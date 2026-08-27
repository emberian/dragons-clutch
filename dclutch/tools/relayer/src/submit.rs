//! The submission gate.
//!
//! Submission is implemented as code and gated at execution.  Devnet or
//! mainnet submission is a **separately authorized act** that has to be named
//! by a current authorization; nothing in this service can grant it, and no
//! amount of config makes it self-authorizing.  What config can do is assert
//! that such an authorization exists, which is what
//! `allow_public_submission = true` means and why it defaults to false.
//!
//! The gate is on the *host*, not on a cluster name, because a mainnet RPC
//! reached through a proxy is still mainnet and a "devnet" URL is only a
//! string.  Local means loopback.

use crate::config::SubmitConfig;
use crate::error::{RelayerError, Result};

/// Hosts treated as local.
const LOCAL_HOSTS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "[::1]"];

/// Whether a host is loopback.
pub fn is_local_host(host: &str) -> bool {
    let lowered = host.to_ascii_lowercase();
    LOCAL_HOSTS.contains(&lowered.as_str())
}

/// Refuse a submit endpoint that is not local unless it was explicitly
/// authorized.
pub fn require_local_or_authorized(endpoint: &str, allow_public_submission: bool) -> Result<()> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|error| RelayerError::config(format!("submit.endpoint is not a URL: {error}")))?;
    let host = url.host_str().unwrap_or("").to_owned();
    if is_local_host(&host) {
        return Ok(());
    }
    if allow_public_submission {
        eprintln!(
            "WARNING: submitting to non-local host {host:?} because allow_public_submission is \
             set. This is only correct under a current authorization that names devnet or \
             mainnet submission."
        );
        return Ok(());
    }
    Err(RelayerError::PublicSubmissionRefused { host })
}

/// Check a resolved submit configuration before anything is built.
pub fn require_submission_admitted(submit: &SubmitConfig) -> Result<()> {
    require_local_or_authorized(&submit.endpoint, submit.allow_public_submission)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_endpoints_are_admitted_without_any_extra_authorization() {
        for endpoint in [
            "http://127.0.0.1:8899",
            "http://localhost:8899",
            "http://LOCALHOST:8899",
            "http://[::1]:8899",
        ] {
            require_local_or_authorized(endpoint, false)
                .unwrap_or_else(|error| panic!("{endpoint} refused: {error}"));
        }
    }

    #[test]
    fn a_public_cluster_is_refused_by_default() {
        for endpoint in [
            "https://api.devnet.solana.com",
            "https://api.mainnet-beta.solana.com",
            "https://example-rpc-provider.example/api-key",
            "http://192.168.1.10:8899",
        ] {
            let error = require_local_or_authorized(endpoint, false).unwrap_err();
            assert!(
                matches!(error, RelayerError::PublicSubmissionRefused { .. }),
                "{endpoint} was admitted: {error:?}"
            );
        }
    }

    #[test]
    fn a_public_cluster_needs_the_named_authorization_flag() {
        require_local_or_authorized("https://api.devnet.solana.com", true)
            .expect("explicitly authorized");
    }

    #[test]
    fn a_lan_address_is_not_loopback() {
        assert!(!is_local_host("192.168.1.10"));
        assert!(!is_local_host("0.0.0.0"));
        assert!(!is_local_host("localhost.evil.example"));
    }
}
