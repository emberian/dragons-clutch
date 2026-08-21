//! The R2 pull-profile source plane: spec generation 2, the crossing rule, and
//! the authentication join.
//!
//! ## Why a second source plane exists
//!
//! [`crate::source`] and [`crate::source_archive`] are the V1 plane: a
//! provider-neutral kernel parameterized by two traits, binding one **exact
//! immutable source data-account key** and a caller-supplied
//! `deployment_generation` number. Neither assumption survives a pull oracle.
//! A Pyth pull update lands in a caller-created, ephemeral, closable account,
//! so there is no key to pin; and a generation number supplied alongside the
//! data is not evidence of anything.
//!
//! The R2 plan's answer is a new spec generation under a new digest domain, not
//! a widening of V1 (`R2_PULL_PROMOTION_PLAN.md` §1). This module is that
//! generation:
//!
//! | submodule | owns |
//! | --- | --- |
//! | [`spec`] | the 368-byte canonical v2 body, its `dragons-clutch/feed/v2` identity, and every field refusal |
//! | [`crossing`] | the closing-boundary `CROSSING_V1` selection rule, its archive record, and the non-strict cursor |
//! | [`auth`] | the runtime join: identity, deployment, config generation, Clock, adjacent post, freshness, maturity, caps |
//!
//! ## Where the identity bytes are
//!
//! Nowhere in here. Every pinned address, digest, slot, and activation instant
//! lives in [`crate::source_identity`], which is the single place the E2 freeze
//! act edits. This module is pure mechanism over whatever that module names.
//!
//! ## Relationship to the research crate
//!
//! [`spec`] and [`crossing`] are ports of `research/source-profile-v1`'s
//! `spec_v2.rs` and `crossing_v1.rs` under P0.2, with `sha2` swapped for the
//! runtime SHA-256 syscall wrapper and the research crate's two self-flagged
//! `MODEL_*` envelope constants replaced by the real [`clutch_accumulator`]
//! ones. [`auth`] is a deliberate strengthening of that crate's `auth_v2.rs`:
//! the model *accepts* the loader-state and immediate-post projections as
//! inputs and says plainly that a caller-supplied struct proves nothing, while
//! the runtime **derives** both from the pinned decoders, so there is no
//! signature through which a caller could supply them.

pub mod auth;
pub mod crossing;
pub mod spec;
