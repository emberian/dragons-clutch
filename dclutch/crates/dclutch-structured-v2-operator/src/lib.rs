#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived effect planning for shard-backed Structured receipts.
//!
//! This layer authenticates nothing on its own.  It consumes already-decoded
//! immutable Structured terms, already-decoded exact claim-shard terms, and an
//! explicitly named adapter observation, re-derives every amount through the
//! pure kernel, and emits typed effect plans plus the borrowed inputs of the
//! onchain-safe candidate.  It never signs, submits, invents a Mint, derives a
//! PDA, or persists a supply projection.
//!
//! Solana SDK types are deliberately absent: instruction construction, PDA
//! derivation, AccountProfile expansion, and CPI belong to the physical
//! integration adapter, which consumes
//! [`StructuredHotCandidateInputV2`](dclutch_structured_v2_contract::StructuredHotCandidateInputV2)
//! and revalidates every field independently.

mod action;
mod lowering;

pub use action::{
    StructuredActionObservationV2, StructuredActionPlanV2, StructuredIntentV2,
    StructuredRequestContextV2, StructuredShardAccountObservationV2, StructuredTokenEffectPlanV2,
    plan_structured_action_v2,
};
pub use lowering::{
    StructuredHotEffectCoordinatesV2, StructuredHotProfileV2, lower_structured_hot_effects_v2,
    lower_structured_hot_rent_close_v2,
};

/// Stable operator refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The named adapter observation was unfinalized, mis-sized, or inconsistent.
    ChainObservation,
    /// Immutable terms, shard terms, and the context did not join.
    Terms,
    /// The pure kernel refused the requested exact transition.
    Kernel,
    /// The canonical family request could not be built for this action.
    Request,
    /// A Token-owned pre-state contradicted the exact derived effect.
    Token,
    /// A profile coordinate omitted or substituted a selected account.
    AccountFrame,
    /// Lifecycle-Rent closure evidence was absent or noncanonical.
    Rent,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;
