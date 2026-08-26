//! Canonical Trading adapter for recurring-Series V2.
//!
//! Immutable record decoding, content identities, occurrence proofs, funding
//! list semantics, and future-Market projection have one SDK-free owner in
//! `dclutch-series-v2-kernel`. This module retains only Solana account access,
//! PDA derivation, Core request construction, replay persistence, and physical
//! commit-last lifecycle planning.

/// Solana account authentication and commit-last physical persistence.
pub mod accounts;
/// Exact sparse family request consumed by the canonical Trading hot outer.
pub mod instruction;
/// Content-to-Solana/Core conversion at the explicit adapter boundary.
mod kernel_adapter;
/// Total commit-last lifecycle planning for recurring Series V2.
pub mod lifecycle;
/// Exact content/replay projector behind the canonical Trading hot outer.
pub mod projector;
/// Fixed-layout mutable replay state owned by the selected Trading program.
pub mod state;

#[cfg(test)]
pub(crate) use dclutch_series_v2_kernel::generated;
pub use dclutch_series_v2_kernel::{
    AccountKeyV2, AdmittedOccurrenceV2, AdmittedTicketV2, FoundingFundsV2,
    FutureMarketProjectionV2, OccurrenceV2, PrefoundingSeriesEscrowV2,
    SERIES_MAXIMUM_MERKLE_HEIGHT_V2, SERIES_OCCURRENCE_BYTES_V2, SERIES_TEMPLATE_BYTES_V2,
    SERIES_TICKET_BYTES_V2, SeriesV2Error, TemplateV2, TicketV2, admit_occurrence, admit_ticket,
    future_market_projection, occurrence_content_id, pre_founding_series_escrow,
    template_content_id, ticket_content_id,
};
#[cfg(test)]
pub(crate) use kernel_adapter::{core_identity, core_pubkey_identity};
pub(crate) use kernel_adapter::{core_request, pubkey};
pub use kernel_adapter::{funding_list_id, require_funding_list, require_market_pda};

#[cfg(test)]
mod tests;
