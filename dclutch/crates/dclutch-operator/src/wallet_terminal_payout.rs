//! Pure wallet-terminal payout derivation, callable from a browser.
//!
//! Extracted from `dclutch-local-successor-bootstrap` so the same derivation
//! the operator toolchain runs can be compiled to WebAssembly instead of
//! reimplemented in TypeScript. The binary keeps its shell — argument parsing,
//! the two artifact files, RPC, and the cluster-origin policy — and consumes
//! this crate for everything else.
//!
//! Nothing here reads a file, opens a socket, or holds a key.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use core::str::FromStr;

use solana_program::pubkey::Pubkey;

// A VERBATIM EXTRACTION. Every item below was `pub(crate)` in
// `tools/local-validator/bootstrap/successor/src/wallet_terminal.rs` and
// carries the comments it was written with; the move changed visibility, the
// two shell couplings, and nothing else. `missing_docs` is allowed for exactly
// that reason: writing a hundred field-level doc comments inside the move
// would destroy the one property that makes a move reviewable, which is that
// it reads as a move. They are owed, and they are owed to this module rather
// than to a future note.
#[allow(missing_docs)]
pub mod wire;

pub mod snapshot_wire;

/// One refusal, with the reason the derivation gave for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    /// Name one refusal.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(format!("payout JSON: {error}"))
    }
}

/// This crate's result.
pub type Result<T> = core::result::Result<T, Error>;

/// One account's observed value, however the caller came by it.
///
/// The binary fills this from its RPC client and a browser fills it from
/// `getMultipleAccounts`. Keeping the shape here rather than taking the
/// binary's `RpcAccount` is what removes this crate's last tie to a socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedAccountValueV1 {
    /// Program that owns the account.
    pub owner: Pubkey,
    /// Observed lamports.
    pub lamports: u64,
    /// Observed executable bit.
    pub executable: bool,
    /// Exact account bytes.
    pub data: Vec<u8>,
}

/// Lowercase hex of some bytes.
pub fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

/// Exactly 64 lowercase hex characters, as 32 bytes.
pub fn hex32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new("expected 64 lowercase hex characters"));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(pair).map_err(|_| Error::new("non-UTF8 hex"))?;
        output[index] = u8::from_str_radix(text, 16).map_err(|_| Error::new("invalid hex byte"))?;
    }
    Ok(output)
}

/// One base58 public key.
pub fn pubkey(value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|error| Error::new(format!("invalid pubkey {value}: {error}")))
}
