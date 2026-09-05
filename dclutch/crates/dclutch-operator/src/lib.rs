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

/// Bearer specialization of the Rational Representation V2 actions.
pub mod bearer;
/// Fractional Claims family construction, lowering and retirement planning.
pub mod fractional;
/// Evidence bridge for the Fractional cubic-life campaign.
pub mod fractional_cubic_life_evidence;
/// Read-only General V5 successor-plan production over a route document.
#[cfg(feature = "successor")]
pub mod general_successor;
/// Bump-hint mining over the decodable Hot corpus.
pub mod hot_bump_miner;
/// Artifact-derived construction for generic Market founding.
pub mod market_founding;
/// Rational lifecycle Hot request, selected-set and bundle construction.
pub mod rational_lifecycle_hot;
/// Unsigned instruction construction for exact Rational Representation V2 actions.
pub mod rational_representation;
/// Product-to-representation composition and its unsigned workflows.
pub mod representation_composition;
/// One chain-authenticated selector for the Source funding-readiness walk.
pub mod source_readiness;
/// Effect planning for shard-backed Structured receipts.
pub mod structured;
/// Wallet-terminal payout input derivation, callable from a browser.
pub mod wallet_terminal_input;
/// Wallet-terminal payout derivation, callable from a browser.
pub mod wallet_terminal_payout;

pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};

/// What a holder needs to find and redeem a claim-check.
pub mod claim_check_v1;

/// Canonical logical-to-physical routing and finalized poststate planning for Direct V3.
/// The family-neutral producer for Trading's permissionless validated-artifact seal.
pub mod capability_seal_v1;
/// Conservative complete-set split and merge planning.
pub mod claims_conservation_v1;
/// Chain-derived unsigned Dealer junior-equity Hot execution construction.
#[cfg(feature = "dealer-series")]
pub mod dealer_equity_hot_v3;
/// Chain-derived unsigned Dealer LP Open/Close Hot construction.
#[cfg(feature = "dealer-series")]
pub mod dealer_lp_hot_v4;
/// Exact delegated-allowance Custody successor CPI construction.
pub mod delegated_custody;
/// Chain-derived unsigned Direct root retirement construction.
pub mod direct_begin_retiring_v1;
/// Chain-derived unsigned Direct maker-replay close construction.
pub mod direct_close_maker_v1;
pub mod direct_inline_route_v3;
/// Chain-derived action-selected Direct V3 inline execution construction.
pub mod direct_inline_v3;
/// Chain-derived General V3 capability activation planning.
pub mod general_activation_v3;
/// Chain-derived General V3 Hot execution and packet construction.
pub mod general_hot_v3;
/// Compile the seven General actions into one publishable, selectable release.
pub mod general_selected_release_v1;
/// Chain-derived inspection of immutable Core/Registry/Rent infrastructure.
pub mod infrastructure;
/// Chain-derived unsigned Core infrastructure succession composition.
pub mod infrastructure_succession_v1;
/// Shared authentication of Rent, Clock, and finalized-record observations.
pub mod observation;
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
pub mod rational_selected_release_v1;
pub mod structured_selected_release_v1;

pub mod rational_terminal_v3;
/// Chain-derived unsigned Registry activation and reauthentication workflows.
pub mod registry;
/// Checked-release admission into unsigned Registry activation workflows.
pub mod release_activation;
/// Chain-derived Core effects for the complete funded Resolution lifecycle.
pub mod resolution_core_v3 {
    pub use dclutch_resolution_core_v3_operator::*;
}
/// Production acquisition of current Series V5 physical account banks.
#[cfg(feature = "dealer-series")]
pub mod series_current_acquisition_v5;
/// Chain-derived Series V3 Hot lifecycle and packet construction.
#[cfg(feature = "dealer-series")]
pub mod series_hot_v3;
/// Chain-derived selection of the next recurring-Series lifecycle act.
#[cfg(feature = "dealer-series")]
pub mod series_lifecycle_v3;
/// Chain-derived Direct close and retirement replay-handoff construction.
pub mod terminal_retirement_v1;
/// Wallet-authorized Claims terminal payout and exact v0 routing construction.
pub mod wallet_terminal_payout_v3;
/// Chain-derived address-table lifecycle and versioned-message construction.
pub mod versioned {
    pub use dclutch_versioned_message_operator::*;
}
/// Finalized devnet planning for wallet-authorized Claims Position admission.
pub mod user_position_admission_v1;
/// Finalized devnet planning for wallet-authorized Claims Position close.
pub mod user_position_close_v1;

/// Shared decodable Market/root/activation corpus for the family bump-hint
/// positive controls. Test-only; see the module's own documentation.
#[cfg(test)]
mod hot_bump_corpus_fixture_v1;
