#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Immutable categorical resolution policies.
//!
//! Resolution evaluates supplied observations only.  It does not read a clock,
//! retain observations, or mutate a market ledger.  Nothing here owns Market
//! state: a policy is a total function from an authenticated observation to an
//! outcome or a refusal, and the caller owns everything else.

/// Pyth V1-shaped categorical price policy and total resolution fold.
pub mod categorical_pyth_v1;
