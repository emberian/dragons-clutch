//! Canonical Trading adapter for recurring-Series V3.
//!
//! Immutable record decoding, content identities, occurrence proofs, funding
//! list semantics, and future-Market projection have one SDK-free owner in
//! `dclutch-series-v3-kernel`. This module retains only Solana account access,
//! PDA derivation, Core request construction, replay persistence, and physical
//! commit-last lifecycle planning.

/// Solana account authentication and commit-last physical persistence.
pub mod accounts;
/// Exact action-selected generic V3 interpreter artifacts.
pub mod artifacts_v3;
/// Schema-bound V4 descriptor and global DCE5 Consume artifact admission.
pub mod artifacts_v4;
/// Complete Core/Custody/replay physical plans behind authenticated actions.
/// Canonical SeriesEscrow projection into the sole Custody writer.
pub mod custody_v3;
/// Global five-route Consume Effect V4 topology and route-window admission.
pub mod effect_v4;
/// Exact Core-to-Custody call staging behind the common Hot V3 outer.
pub mod execute_v3;
/// Exact sparse family request consumed by the canonical Trading hot outer.
pub mod instruction;
/// Content-to-Solana/Core conversion at the explicit adapter boundary.
mod kernel_adapter;
/// Total commit-last lifecycle planning for recurring Series V3.
pub mod lifecycle;
/// Chain-derived unsigned hot-action request construction.
pub mod operator;
/// Canonical projected-Hoard Custody request construction.
pub mod projected_custody_v3;
/// Exact content/replay projector behind the canonical Trading hot outer.
pub mod projector;
/// Chain-derived Shadow-AOT release selection and generic request construction.
pub mod shadow_operator;
/// Fixed-layout mutable replay state owned by the selected Trading program.
pub mod state;
/// Terminal Ticket-retire/root-close differential execution oracle.
pub mod terminal;

pub use dclutch_series_v3_kernel::composition::{
    SeriesConsumeCompositionErrorV3, SeriesConsumeCompositionV3, compose_series_consume_v3,
};
pub use dclutch_series_v3_kernel::escrow::{
    ConsumeSeriesEscrowPlanV3, PrepareSeriesEscrowPlanV3, SeriesEscrowEffectKindV3,
    SeriesEscrowEffectV3, TerminalSeriesEscrowPlanV3, consume_series_escrow_v3,
    expire_series_escrow_v3, prepare_series_escrow_v3,
};
#[cfg(test)]
pub(crate) use dclutch_series_v3_kernel::generated;
pub use dclutch_series_v3_kernel::{
    AccountKeyV3, AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2,
    FoundingFundsV3, FutureMarketProjectionV3, OccurrenceV3, PrefoundingSeriesEscrowV3,
    SERIES_MAXIMUM_MERKLE_HEIGHT_V3, SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3,
    SERIES_TICKET_BYTES_V3, SeriesV3Error, TemplateV3, TicketV3, admit_occurrence,
    admit_occurrence_bytes, admit_ticket, future_market_projection, occurrence_content_id,
    pre_founding_series_escrow, template_content_id, ticket_content_id,
};
#[cfg(test)]
pub(crate) use kernel_adapter::{core_identity, core_pubkey_identity};
pub(crate) use kernel_adapter::{core_request, pubkey};
pub use kernel_adapter::{funding_list_id, require_funding_list, require_market_pda};

#[cfg(test)]
mod tests;
