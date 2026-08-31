#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The portable Direct intent ticket: one author, one reader, one shape.
//!
//! A Direct inline fill settles two independently signed intents. There is
//! exactly one author of a ticket per language and there must go on being
//! exactly one: a second implementation of a signing preimage is a signature
//! that verifies nowhere, discovered at the refused trade. This crate is the
//! Rust one. The other is `encodeDirectIntentTicketV1` in
//! `packages/dclutch-sdk/lib/directTicket.ts`, which the browser trade panel
//! calls, and the two are pinned byte-for-byte -- signature included -- against
//! `packages/dclutch-sdk/fixtures/direct-intent-ticket.json`.
//!
//! WHO OWNS WHAT, so nothing here is an inferred layout:
//!
//! - The SIGNED MESSAGE is owned by `dclutch_direct_codec::intent_v2`, emitted
//!   from `formal/dclutch-semantics/EmitDirectIntentV2Rust.lean`. This crate
//!   calls `CompactIntentV2::signed_preimage()`; it does not lay out a byte.
//! - The JSON ENVELOPE is owned by [`PortableDirectTicketV1`]. Its serde field
//!   order IS the wire order, and [`encode_portable_direct_ticket_v1`] and
//!   [`parse_portable_direct_ticket_v1`] are the only writer and the only
//!   reader in this workspace.
//!
//! WHAT THIS CRATE REFUSES TO DO: read chain state, guess a nonce, guess a slot
//! window, take the maker on faith, or submit anything. Every field the
//! signature binds is a required argument. Authoring and submitting are
//! separate acts by separate programs; nothing here opens a socket.
//!
//! THE KEY PATH IS NEVER AN ARGUMENT. [`author::parse_arguments_v1`] refuses
//! `--keypair`, `--keypair-path` and `--secret-key` at parse, before the value
//! travels any further; `--keypair-env` names an ENVIRONMENT VARIABLE holding
//! the absolute path. Nothing about the path or the key reaches the command
//! line, the process table, the receipt, or a refusal message.

use core::fmt;

#[cfg(feature = "author")]
pub mod author;
mod envelope;
mod strict_json;

#[cfg(feature = "author")]
pub use author::{
    DIRECT_TICKET_AUTHOR_COMMAND_V1, DirectTicketAuthorArgumentsV1, DirectTicketAuthorReceiptV1,
    author_direct_intent_ticket_v1, author_with_keypair_path_v1,
    keypair_path_from_environment_v1, keypair_seed_from_file_v1, parse_arguments_v1, run_v1,
    sign_direct_intent_v1, usage_v1,
};
pub use envelope::{
    MAXIMUM_TICKET_BYTES_V1, PORTABLE_DIRECT_TICKET_KIND_V1, PortableDirectTicketIntentV1,
    PortableDirectTicketV1, SignedDirectIntentV3, canonical_ticket_pubkey_v1,
    canonical_ticket_u64_v1, decode_hex_v1, encode_portable_direct_ticket_v1, hex_lower_v1,
    parse_portable_direct_ticket_v1, sha256_hex_v1,
};
pub use strict_json::parse_json_without_duplicate_keys_v1;

/// A refusal, carrying the sentence the operator should read.
///
/// Deliberately a string and not an enum of causes. Every consumer of this
/// crate is a command-line program whose whole contract with the caller is the
/// sentence it prints, and a caller cannot act on a variant name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    /// Build one refusal from anything that can name itself.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

/// This crate's result.
pub type Result<T> = core::result::Result<T, Error>;

/// Build the one refusal shape every message in this crate carries.
///
/// The `REFUSED:` prefix is load-bearing: consumers assert on it, and an
/// operator reading a terminal needs to know instantly that nothing happened.
pub(crate) fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}
