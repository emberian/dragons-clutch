#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Deterministic, exact-integer economic scenarios for the devnet activity
//! harness and its independent reconciler.
//!
//! This package owns scenario composition, not protocol semantics or live
//! identities. The Direct fee denominator is imported from its canonical
//! codec owner. A fixture is deliberately `scenario-only`: live addresses,
//! signatures, finalized slots, transaction fees, and account-data digests
//! remain absent until an execution harness binds them from public state.

mod engine;
mod model;
mod scenarios;

pub use engine::{
    Error, Result, canonical_manifest_bytes, canonical_manifest_set, check_fixture_directory,
    validate_manifest, write_fixture_directory,
};
pub use model::*;
