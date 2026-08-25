#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Generated fixed-memory interpreter for the Lean-owned Product payoff ABI.
//!
//! The checked-in implementation is emitted by
//! `EmitProductPayoffRust.lean`. It allocates no memory, performs no floating
//! point arithmetic, and authenticates no Product, account, or release.

include!("generated.rs");
