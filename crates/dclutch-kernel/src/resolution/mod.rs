//! Immutable categorical resolution policies.
//!
//! Resolution evaluates supplied observations only.  It does not read a clock,
//! retain observations, or mutate a market ledger.

/// Pyth V1-shaped categorical price policy and total resolution fold.
pub mod categorical_pyth_v1;
