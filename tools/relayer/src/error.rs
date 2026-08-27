//! Every refusal this service can produce.
//!
//! Operator input never panics.  A malformed config, an unreadable keypair, a
//! genesis-hash mismatch, an RPC that answers something other than what was
//! asked — each is a typed refusal that the caller turns into an exit code and
//! a diagnostic.

use std::path::PathBuf;

/// Result alias for this service.
pub type Result<T> = core::result::Result<T, RelayerError>;

/// A refusal.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RelayerError {
    /// A filesystem operation failed, naming the path it failed on.
    #[error("io error at {path}: {source}")]
    Io {
        /// The path the operation was attempted on.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The configuration file was not valid TOML for this schema.
    #[error("config file {path} is not valid: {source}")]
    ConfigSyntax {
        /// The configuration path.
        path: PathBuf,
        /// The underlying parse error.
        #[source]
        source: toml::de::Error,
    },

    /// The configuration parsed but is not admissible.
    #[error("config refused: {0}")]
    Config(String),

    /// A 32-byte identifier was not decodable as hex or base58.
    #[error("{field}: {value:?} is not a 32-byte identifier in hex or base58")]
    Identifier {
        /// Which configuration field carried it.
        field: String,
        /// The rejected text.
        value: String,
    },

    /// The keypair path names a location this service refuses to read.
    ///
    /// The relayer never scans for wallets and never opens a path it was not
    /// explicitly handed in config, and it additionally refuses paths that look
    /// like a user's real wallet store.
    #[error("refusing keypair path {path}: {reason}")]
    UnsafeKeypairPath {
        /// The refused path.
        path: PathBuf,
        /// Why it was refused.
        reason: String,
    },

    /// The keypair file did not carry a well-formed Ed25519 keypair.
    #[error("keypair file {path} is malformed: {reason}")]
    MalformedKeypair {
        /// The keypair path.
        path: PathBuf,
        /// Why it was refused.
        reason: String,
    },

    /// The observed cluster is not the cluster the config pinned.
    ///
    /// Nothing else distinguishes a mainnet account from its byte-identical
    /// twin on another cluster, so this refusal is fatal and never repaired.
    #[error(
        "genesis hash mismatch on {endpoint}: config pinned {expected}, cluster reported {observed}"
    )]
    GenesisMismatch {
        /// Host of the endpoint that was asked.
        endpoint: String,
        /// The base58 genesis hash the config pinned.
        expected: String,
        /// The base58 genesis hash the cluster reported.
        observed: String,
    },

    /// The HTTP transport failed.
    #[error("rpc transport error against {endpoint}: {source}")]
    Transport {
        /// Host of the endpoint.
        endpoint: String,
        /// The underlying error.
        #[source]
        source: reqwest::Error,
    },

    /// The RPC returned a JSON-RPC error object.
    #[error("rpc error from {endpoint} calling {method}: code {code}: {message}")]
    RpcError {
        /// Host of the endpoint.
        endpoint: String,
        /// The JSON-RPC method.
        method: String,
        /// The JSON-RPC error code.
        code: i64,
        /// The JSON-RPC error message.
        message: String,
    },

    /// The RPC answered something structurally other than what was asked.
    #[error("rpc response from {endpoint} calling {method} is malformed: {reason}")]
    MalformedRpcResponse {
        /// Host of the endpoint.
        endpoint: String,
        /// The JSON-RPC method.
        method: String,
        /// What was wrong.
        reason: String,
    },

    /// The wire codec refused.
    #[error("wire codec refused in {context}: {error:?}")]
    Wire {
        /// Where the refusal happened.
        context: String,
        /// The exact codec refusal.
        error: dclutch_relay_contract::Error,
    },

    /// The observation was refused, and the named set stops being attested.
    ///
    /// §4.11: on RPC disagreement, a missing account, a `data_len` outside the
    /// admitted set, or a `deployment_slot` change, the daemon stops attesting
    /// that set and emits a diagnostic.  It never attests a partial or repaired
    /// observation.  The market's own funded failure path (§4.8) is the correct
    /// handling of a stopped relayer and is better than any repair invented
    /// here.
    #[error("observation refused for account set {set}: {reason}")]
    ObservationRefused {
        /// The configured set name.
        set: String,
        /// Why the set stopped.
        reason: String,
    },

    /// The configured submit endpoint is not local and was not explicitly
    /// authorized.
    #[error(
        "refusing to submit to non-local host {host:?}: devnet or mainnet submission is a \
         separately authorized act; set allow_public_submission = true in [submit] only under \
         an authorization that names it"
    )]
    PublicSubmissionRefused {
        /// The host that was refused.
        host: String,
    },

    /// A subcommand needed configuration that was not supplied.
    #[error("{0}")]
    MissingCapability(String),

    /// Serializing an artifact failed.
    #[error("serialization failed: {0}")]
    Serialization(String),
}

impl RelayerError {
    /// Wrap a wire-codec refusal with the site that produced it.
    pub fn wire(context: &str, error: dclutch_relay_contract::Error) -> Self {
        Self::Wire {
            context: context.to_owned(),
            error,
        }
    }

    /// Wrap a filesystem failure with the path it happened on.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// A configuration refusal.
    pub fn config(reason: impl Into<String>) -> Self {
        Self::Config(reason.into())
    }
}
