//! Host-only compile and hostile-test seam for the disjoint Series V2 module.
//!
//! This keeps the content specialization executable while the common Trading
//! library owner performs the one-line public-module linkage.

#![allow(dead_code, unused_imports)]

#[path = "../src/series/mod.rs"]
mod series;
