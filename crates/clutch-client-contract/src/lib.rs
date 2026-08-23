#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Shared, untrusted client projections for Dragon's Clutch.
//!
//! This crate owns labels and capability classification only. It deliberately
//! borrows the authoritative persisted-layout and relation types instead of
//! defining client DTO copies that could become a second semantic truth.

pub mod evidence;
pub mod intent;
pub mod owner_settlement;
pub mod settlement;
