// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fixed-memory two-window candidate lifecycle kernel.
//!
//! The crate is deliberately independent of Solana and clearing/score code.
//! Identities are fixed bytes authenticated by an adapter. A score policy
//! supplies one canonical rank key; this crate only binds and orders it.

#![no_std]
#![forbid(unsafe_code)]

mod codec;
mod state;
mod successor;
mod transition;
mod wire;

pub use codec::CodecError;
pub use state::*;
pub use successor::*;
pub use transition::*;
pub use wire::*;
