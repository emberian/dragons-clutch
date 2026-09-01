//! Host-only compile and hostile-test seam for the disjoint Series V2 module.
//!
//! This keeps the content specialization executable as its own compilation
//! unit rather than only as part of the common Trading library.
//!
//! `src/series` reaches a small set of family-neutral siblings through
//! `crate::` — currently `projected_market_v2::AuthenticatedFoundSpanV2` and
//! `execution_strategy_v2::AuthenticatedExecutionStrategyV2`. The common
//! Trading library owns and publicly links each of them, so this root binds the
//! real modules under those names instead of recompiling them here. Binding
//! them keeps one owner for each authenticated type: the seam cannot drift into
//! a second definition that the library would never accept.
//!
//! The list below is the authority for what `src/series` may reach; the prose
//! deliberately states no COUNT, because a stale count is what hid the last
//! break. If a new `crate::` reach appears here, the choice is to bind the real
//! module — which requires the item to be genuinely public, not `pub(crate)` —
//! or to establish that the reach should not exist. A `pub(crate)` wrapper that
//! only re-exports a public codec item is the second case: point `src/series`
//! at the codec that owns the wire and delete the wrapper.

#![allow(dead_code, unused_imports)]

pub(crate) use dclutch_trading_sbf::{execution_strategy_v2, projected_market_v2};

#[path = "../src/series/mod.rs"]
mod series;
