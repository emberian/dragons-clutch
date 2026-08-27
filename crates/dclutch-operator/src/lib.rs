//! Host-only construction of unsigned dClutch instructions.
//!
//! Every module here is an untrusted projection builder: it accepts one
//! finalized snapshot of canonical accounts, re-decodes their immutable
//! bindings, and constructs an unsigned instruction. None of them performs RPC,
//! signing, or submission.
//!
//! The crate root carried a body of its own until the DCLTCAT1 burial -- the
//! categorical price/failure resolution builders, the `dclutch/market-root/v1`
//! PDA domain, and the Market fact decoders behind them. They spoke to a Market
//! representation nothing in this tree writes. The root is now what it should
//! always have been: a place where the family builders are declared, plus the
//! shared observation authentication in `observation`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};

/// Canonical schema-bound CapabilityProgramSetV2 artifact construction.
pub mod capability_program_set_v2;
/// Chain-derived unsigned Dealer junior-equity Hot execution construction.
pub mod dealer_equity_hot_v3;
/// Chain-derived Dealer scenario exact-fill physical-account projection.
pub mod dealer_scenario_hot_v4;
/// Exact delegated-allowance Custody successor CPI construction.
pub mod delegated_custody;
/// Chain-derived action-selected Direct V3 inline execution construction.
pub mod direct_inline_v3;
/// Exact unsigned signing material for the Direct V2 successor.
pub mod direct_successor;
/// Chain-derived General V3 capability activation planning.
pub mod general_activation_v3;
/// Chain-derived General V3 Hot execution and packet construction.
pub mod general_hot_v3;
/// Chain-derived inspection of immutable Core/Registry/Rent infrastructure.
pub mod infrastructure;
/// Shared authentication of Rent, Clock, and finalized-record observations.
pub mod observation;
/// Lifecycle-scoped RentCredit creation, sweeping, and close evidence.
pub mod lifecycle_rent_v2 {
    pub use dclutch_product_runtime_v2_operator::lifecycle_rent_v2::*;
}
mod product_graph_observation_v3 {
    pub(crate) use dclutch_resolution_core_v3_operator::product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    };
}
/// Chain-derived real-provider submission and permissionless reclaim.
pub mod provider_transport_v3 {
    pub use dclutch_provider_transport_v3_operator::*;
}
/// Packet-safe unsigned Rational terminal Bearer redemption construction.
pub mod rational_terminal_v3;
/// Chain-derived unsigned Registry activation and reauthentication workflows.
pub mod registry;
/// Checked-release admission into unsigned Registry activation workflows.
pub mod release_activation;
/// Chain-derived Core effects for the complete funded Resolution lifecycle.
pub mod resolution_core_v3 {
    pub use dclutch_resolution_core_v3_operator::*;
}
/// Chain-derived Series V3 Hot lifecycle and packet construction.
pub mod series_hot_v3;
/// Compact projected-Market Series Consume instruction-data construction.
pub mod series_projected_v2;
/// Chain-derived address-table lifecycle and versioned-message construction.
pub mod versioned {
    pub use dclutch_versioned_message_operator::*;
}
