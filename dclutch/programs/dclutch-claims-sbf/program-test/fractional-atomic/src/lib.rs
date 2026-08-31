#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-ELF campaign for the production Fractional atomic Claims route.
//!
//! Two campaigns live in `tests/`: `fractional_atomic.rs` (wrap, unwrap and the
//! terminal settlement) and `fractional_compaction.rs` (a stranger compacting a
//! sleeping holder's reserve). This crate carries what they share -- the
//! width-parameterized Product/LBV2 fixture, and the account plumbing that
//! encodes real on-chain formats by hand.

pub mod campaign_support;
pub mod narrow_fixture;
