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
//! derivation, AccountProfile expansion, and CPI belong to a physical adapter
//! that does not exist yet, and that will not consume
//! [`StructuredHotCandidateInputV2`](dclutch_structured_v2_contract::StructuredHotCandidateInputV2)
//! when it does.  Decision 0011: the chain reaches a family through its sealed
//! artifact closure, so the candidate is this crate's own adversary rather than
//! the executor's.

mod action;
mod child_request;
mod descriptor;
mod lowering;

pub use action::{
    StructuredActionObservationV2, StructuredActionPlanV2, StructuredIntentV2,
    StructuredRequestContextV2, StructuredShardAccountObservationV2, StructuredTokenEffectPlanV2,
    plan_structured_action_v2,
};
pub use child_request::{
    STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2, StructuredChildActorV2, StructuredChildCoordinateV2,
    StructuredChildDescriptorV2, StructuredChildEffectSlotV2, StructuredChildWireV2,
    bind_structured_child_descriptor_v2, encode_structured_child_representation_v2,
    structured_child_effect_order_v2, structured_child_lifecycle_action_v2,
    structured_child_request_bytes_v2, structured_child_token_style_v2, structured_child_wire_v2,
};
pub use descriptor::{
    StructuredDescriptorAuthorityV2, StructuredRepresentationDescriptorV2,
    decode_derived_structured_descriptor_v2, derive_structured_representation_descriptor_v2,
    structured_child_descriptor_from_derivation_v2,
};
pub use lowering::{
    StructuredHotProfileV2, lower_structured_hot_effects_v2, lower_structured_hot_rent_close_v2,
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
    /// A kind or action was lowered onto the wrong adopted child wire.
    ChildWire,
    /// A Rational descriptor coordinate did not represent these Structured terms.
    ChildIdentity,
    /// A child request width was zero, mismatched, or above the executable ceiling.
    ChildWidth,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;
