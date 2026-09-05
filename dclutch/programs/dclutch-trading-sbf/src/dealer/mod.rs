//! The Dealer family: the junior-equity and LP-lifecycle artifact authors
//! and the accelerator evaluators that re-derive them on chain.
//!
//! The common Trading outer owns Registry, deployment and finalized-record
//! authentication; nothing here performs a CPI, creates an account, or
//! commits state. The `v4_*_release` and `lp_set_request` modules are
//! host-only emitters of the sealed artifacts; the `*_accelerator_accounts`
//! modules are the read-only evaluators linked into the Dealer accelerator.

/// Finalized RequestProfile/Transition/Effect joins for junior equity.
pub mod equity_artifacts;
/// Exact scenario-residual junior pool-equity kernel.
pub mod equity;
/// Canonical sparse SignedDeltaV3 packet for junior-equity Claims movement.
pub mod equity_claims;
/// Runtime-width chain-derived junior-equity contribution/redemption requests.
pub mod equity_request;
/// Reproducible typed Hot EffectProgram artifact for junior equity.
pub mod equity_effect;
/// Canonical LP Open/Close Profile6 and lifecycle artifacts.
pub mod lp_artifacts;
/// Scenario-solvent, custody-backed multi-LP capital under canonical Trading.
pub mod multi_lp;
/// Trading-owned runtime-width terminal obligations for scenario-solvent Dealer V3.
pub mod obligation;
/// Chain-derived unsigned requests for every Dealer V3 multi-LP action.
pub mod lp_request;
/// Exact logical AccountProfile for admitted junior-equity execution.
pub mod equity_profile;
/// One global selector authority and finalized V3 capability descriptors.
pub mod release;
/// Physical account authentication and candidate evaluation for junior equity.
pub mod equity_accelerator;
/// Schema-bound selector-1..=6 V4 release finalization for junior equity.
#[cfg(not(target_os = "solana"))]
pub mod equity_release;
/// Physical account authentication and candidate evaluation for LP Open/Close.
pub mod lp_accelerator;
/// Schema-bound SetV2 request construction for LP Open/Close.
#[cfg(not(target_os = "solana"))]
pub mod lp_set_request;
/// Schema-bound selector-7/8 V4 release finalization for LP Open and Close.
#[cfg(not(target_os = "solana"))]
pub mod lp_release;

/// Canonical Dealer capability-kind label.
pub const DEALER_KIND_PREIMAGE_V2: &[u8] = b"dclutch/capability/dealer-v2";
/// Canonical immutable Dealer Policy config schema.
pub const DEALER_CONFIG_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-config-v2";
/// Canonical hot request schema with exact Claims Position revision.
pub const DEALER_REQUEST_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-request-v2";
/// Canonical inventory-free mutable root-tail schema.
pub const DEALER_ROOT_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/dealer-root-tail-v2";
/// Canonical Dealer account/register projection profile.
pub const DEALER_ACCOUNT_PROFILE_PREIMAGE_V2: &[u8] = b"dclutch/account-profile/dealer-v2";
/// Canonical bounded Dealer child-effect plan schema.
pub const DEALER_EFFECT_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/effect/dealer-v2";
/// Candidate PDA domain beneath one immutable Trading child root.
pub const DEALER_CANDIDATE_PDA_DOMAIN_V2: &[u8] = b"dclutch:dealer-candidate:v2";

const _: () = assert!(DEALER_CANDIDATE_PDA_DOMAIN_V2.len() <= 32);
