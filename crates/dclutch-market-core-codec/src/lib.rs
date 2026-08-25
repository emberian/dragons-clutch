#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Generated fixed-memory interpreter for the Lean-owned Market Core ABI.
//!
//! The generated implementation is safe Rust, performs no allocation or
//! floating-point arithmetic, and accepts runtime Product widths through
//! exact-length borrowed slices. Account and release observations are adapter
//! facts; this crate performs no Solana access, hashing, CPI, or signing.

#[allow(missing_docs)]
mod generated {
    include!("generated.rs");
}

pub use generated::*;
