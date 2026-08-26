//! General family integration for the canonical Trading controller.
//!
//! This module is not a dispatcher and owns no executable authority. It
//! consumes the common layer's preauthenticated context and produces family
//! plans which the common layer may apply atomically.

/// Pure, preauthenticated General activation planning.
pub mod activation;
/// Commit-last fixed-role child execution for General settlement.
pub mod controller;
/// Atomic two-pass settlement preparation and exact fixed-role child packets.
pub mod settlement;
