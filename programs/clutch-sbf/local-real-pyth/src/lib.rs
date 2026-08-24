//! Reusable host-only construction boundary for real SourceSeries workflows.
//!
//! The default library exposes only fail-closed operator/index construction.
//! Synthetic observations, captured Pyth executables, fixture signing, and the
//! local campaign binary require the explicit `campaign` laboratory feature.

#[cfg(feature = "operator")]
pub mod account_index;
#[cfg(feature = "operator")]
pub mod action_material;
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
pub mod source_action12_material;
#[cfg(feature = "builder")]
pub mod session;
#[cfg(feature = "campaign")]
pub mod session_builder;
pub mod transaction_builder;
pub mod workflow_graph;
