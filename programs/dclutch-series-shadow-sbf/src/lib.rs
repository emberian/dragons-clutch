#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Stateless Shadow-AOT evaluation for one exact recurring-Series artifact bundle.
//!
//! This crate exposes the checked comparison core first. A physical SBF entry
//! is enabled only by a later generated-bundle module: there is deliberately no
//! generic instruction that accepts caller-supplied artifact bytes.

extern crate alloc;

/// Exact generic-interpreter and Series-semantic comparison boundary.
pub mod evaluator;
