//! Host-only compile and hostile-test seam for the disjoint Series V2 module.
//!
//! This keeps the content specialization executable as its own compilation
//! unit rather than only as part of the common Trading library.
//!
//! `src/series` reaches exactly two family-neutral siblings through `crate::`:
//! `projected_market_v2::AuthenticatedFoundSpanV2` and
//! `execution_strategy_v2::AuthenticatedExecutionStrategyV2`. The common
//! Trading library owns and publicly links both, so this root binds the real
//! modules under those names instead of recompiling them here. Binding them
//! keeps one owner for each authenticated type: the seam cannot drift into a
//! second definition that the library would never accept.

#![allow(dead_code, unused_imports)]

pub(crate) use dclutch_trading_sbf::{execution_strategy_v2, projected_market_v2};

#[path = "../src/series/mod.rs"]
mod series;
