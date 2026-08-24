//! Reusable host-only construction boundary for real SourceSeries workflows.
//!
//! The default library exposes only fail-closed operator/index construction.
//! Synthetic observations, captured Pyth executables, fixture signing, and the
//! local campaign binary require the explicit `campaign` laboratory feature.

#[cfg(feature = "operator")]
pub mod account_index;
#[cfg(feature = "operator")]
pub mod action_material;
#[cfg(feature = "operator")]
pub mod collateral_release_catalog;
#[cfg(feature = "operator")]
pub mod direct_candidate_material;
#[cfg(feature = "operator")]
pub mod direct_action8_material;
#[cfg(feature = "operator")]
pub mod direct_terminal_material;
#[cfg(feature = "operator")]
pub mod dealer_terminal_material;
#[cfg(feature = "operator")]
pub mod failure_action11_material;
#[cfg(feature = "operator")]
pub mod failure_source_action10_material;
#[cfg(feature = "operator")]
pub mod failure_action13_material;
#[cfg(feature = "operator")]
pub mod general_action39_material;
#[cfg(feature = "operator")]
pub mod general_action47_material;
#[cfg(feature = "campaign")]
mod capture;
#[cfg(feature = "operator")]
pub mod index_service;
#[cfg(feature = "operator")]
pub mod operatord;
#[cfg(feature = "campaign")]
pub mod plane;
#[cfg(feature = "campaign")]
pub mod provider;
#[cfg(feature = "operator")]
pub mod rpc_index;
#[cfg(feature = "operator")]
pub mod source_action11_material;
#[cfg(feature = "operator")]
pub mod source_action12_material;
#[cfg(feature = "operator")]
pub mod source_action1_material;
#[cfg(feature = "builder")]
pub mod session;
#[cfg(feature = "campaign")]
pub mod session_builder;
pub mod transaction_builder;
pub mod workflow_graph;
