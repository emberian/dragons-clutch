// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Account-authentication boundary for the live successor failure runtime.
//!
//! The retired V1 adapter accepted a separately encoded intent and parallel
//! root/reserve account DTO. Recovery78/v1 instead authenticates the complete
//! framed V2 semantic root and the separately owned liveness compartment at
//! the SBF boundary, so retaining that unused codec would create a second
//! representation of the same authority.

pub mod external_v2;

/// Exact opaque account identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct AccountId([u8; 32]);

impl AccountId {
    /// Construct an identity from exact runtime bytes without claiming account
    /// authentication.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Complete runtime account observation consumed by the pure adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountView<'a> {
    /// Runtime account address.
    pub key: AccountId,
    /// Runtime-observed account owner.
    pub owner: AccountId,
    /// Current native balance.
    pub lamports: u64,
    /// Complete semantic-owner bytes, excluding only an outer SBF frame when
    /// that frame has already been authenticated by the caller.
    pub data: &'a [u8],
    /// Instruction write privilege.
    pub is_writable: bool,
}
