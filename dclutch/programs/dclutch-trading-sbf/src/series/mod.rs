//! Canonical Trading adapter for recurring-Series V3.
//!
//! Immutable record decoding, content identities, occurrence proofs, funding
//! list semantics, and future-Market projection have one SDK-free owner in
//! `dclutch-series-v3-kernel`. This module retains only Solana account access,
//! PDA derivation, Core request construction, replay persistence, and physical
//! commit-last occurrence planning.

/// Exact dynamic-span physical AccountProfile for global Consume execution.
pub mod account_profile_v4;
/// Solana account authentication and commit-last physical persistence.
pub mod accounts;
/// Exact action-selected generic V3 interpreter artifacts.
pub mod artifacts_v3;
/// Schema-bound V4 descriptor and global DCE5 Consume artifact admission.
pub mod artifacts_v4;
/// Commit-last occurrence, funding, and terminal plans for recurring Series V3.
///
/// This module is deliberately NOT named "lifecycle": the protocol-wide term
/// means the `StateLifecyclePolicyV5` artifact a capability release binds
/// (see [`release_v4`]), while everything here plans the FUNDING and
/// commit-last replay flows — `FundingStateV1` top-ups, Ticket refunds,
/// occurrence commits, retirement, and closure.
pub mod commit_plans;
/// Canonical typed emitters for the occurrence-specific Consume artifacts.
pub mod consume_artifacts_v4;
/// Complete Core/Custody/replay physical plans behind authenticated actions.
/// Canonical SeriesEscrow projection into the sole Custody writer.
pub mod custody_v3;
/// Global five-route Consume Effect V4 topology and route-window admission.
pub mod effect_v4;
/// Exact Core-to-Custody call staging behind the common Hot V3 outer.
pub mod execute_v3;
/// Canonical current-source Expire ProfileV3/EffectV5 artifacts.
pub mod expire_funding_artifacts_v5;
/// Canonical AccountProfileV3/EffectV5 funding-owned action artifacts.
pub mod funding_artifacts_v5;
/// Exact sparse family request consumed by the canonical Trading hot outer.
pub mod instruction;
/// Content-to-Solana/Core conversion at the explicit adapter boundary.
mod kernel_adapter;
/// The Series `StateLifecyclePolicyV5`: root-only, derived, lamport-silent.
pub mod lifecycle_policy_v5;
/// Canonical fixed Prepare/Expire request, transition, and Effect artifacts.
pub mod occurrence_artifacts_v4;
/// Chain-derived unsigned hot-action request construction.
pub mod operator;
pub mod prepare_funding_artifacts_v5;
/// Canonical projected-Hoard Custody request construction.
pub mod projected_custody_v3;
/// Exact content/replay projector behind the canonical Trading hot outer.
pub mod projector;
/// Chain-derived Shadow-AOT release selection and generic request construction.
/// Self-consistent Series Consume capability release assembly.
///
/// This compiler emits immutable publication artifacts. The onchain runtime
/// authenticates and executes those artifacts through `artifacts_v4` and
/// `effect_v4`; it never recompiles a release inside the Trading ELF.
#[cfg(not(target_os = "solana"))]
pub mod release_v4;
/// Host-only current release publication and operator reauthentication.
///
/// The selected artifacts it emits are authenticated by the common Hot
/// runtime, but the allocator-heavy release compiler itself is not an onchain
/// instruction path and must not be linked into the Trading ELF.
#[cfg(not(target_os = "solana"))]
pub mod release_v5;
/// Canonical current-source Retire ProfileV3/EffectV5 artifacts.
pub mod retire_funding_artifacts_v5;
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
pub(crate) use kernel_adapter::core_request;
#[cfg(test)]
pub(crate) use kernel_adapter::{core_identity, core_pubkey_identity};
pub use kernel_adapter::{funding_list_id, require_funding_list, require_market_pda};

#[cfg(test)]
mod tests;
