//! `RelayedMainnetStateV1` observation daemon.
//!
//! This service reads account bytes off one Solana cluster and signs
//! **observations** of them.  It never signs an interpretation: no field is
//! selected, scaled, compared, thresholded or named here (§4.1 of
//! `docs/design/MAINNET_STATE_RELAY.md`).  Every layout fact — discriminators,
//! admitted length sets, field offsets, sentinels, fixed-point scale, rounding
//! boundary — lives in the `decoding_rules_id` record and is applied by the
//! on-devnet adapter.
//!
//! That rule is enforceable rather than aspirational, and the check is a
//! one-liner: **the dependency closure contains no venue IDL, SDK, or layout
//! crate.** A relayer that cannot parse a pool cannot interpret one.  See
//! `DEPENDENCY_CLOSURE.md`.
//!
//! Every encoded byte this service signs comes from `dclutch-relay-contract`.
//! Nothing here reimplements an offset, a magic, a domain separator or a
//! preimage.

#![deny(missing_docs)]

pub mod artifacts;
pub mod chain;
pub mod config;
pub mod derive;
pub mod error;
pub mod id32;
pub mod keys;
pub mod observe;
pub mod publog;
pub mod rpc;
pub mod skew;
pub mod submit;
pub mod txn;
