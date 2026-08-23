#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Production-bound, allocation-free Solana seam for transferable structured
//! claims.
//!
//! This crate owns the wrapper descriptor and request bytes, binds them to the
//! existing base layouts and executable deployments, projects exact Token-2022
//! effects, and stages checked CPI plans. It deliberately owns no backing or
//! supply ledger: those facts remain in the base [`clutch_solana_layout::PositionAccount`],
//! [`clutch_solana_layout::SupplyLedgerAccount`], Hoard/kernel state, and the
//! actual Token-2022 mint.
//!
//! Account borrowing, upgradeable-loader decoding, Token-2022 byte decoding,
//! signer checks, and `invoke_signed` remain in the small SBF dispatcher seam.
//! The dispatcher must feed only authenticated projections into this crate and
//! reconcile every successful CPI receipt and final post-state.

mod codec;
mod identity;
mod plan;
mod projection;

pub use codec::{
    Action, RequestV1, StructuredClaimDescriptorV1, WrapperReplayV1, DESCRIPTOR_BYTES,
    REPLAY_BYTES, REQUEST_BYTES,
};
#[cfg(target_os = "solana")]
pub use identity::SolanaPdaVerifier;
pub use identity::{
    bind_descriptor, canonical_replay_namespace, canonical_wrapper_product_id, AddressBinding,
    PdaVerifier, RuntimeDeployments, DESCRIPTOR_SEED, MINT_SEED, REPLAY_SEED, VAULT_OWNER_SEED,
};
#[cfg(not(target_os = "solana"))]
pub use plan::plan_route;
#[cfg(target_os = "solana")]
pub use plan::plan_route_solana;
pub use plan::{
    plan_route_into, reconcile_post_state, reconcile_receipts, AdapterContext,
    BaseReplayProjection, CpiReceipt, CpiStep, CpiStepKind, ExpectedPostState, RoutePlan,
    RouteScratch, MAX_CPI_STEPS,
};
pub use projection::{
    check_market_closure, AccountAccess, AccountRole, AccountSet, AuthenticatedMarket,
    MintProjection, TokenAccountProjection,
};

/// Canonical identity or Solana address bytes.
pub type Key = [u8; 32];
/// Maximum native Egg width.
pub const MAX_OUTCOMES: usize = clutch_structured_claim::MAX_OUTCOMES;

/// A total refusal from the wrapper adapter seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// A fixed codec received too few bytes.
    Truncated,
    /// A fixed codec received trailing bytes.
    TrailingBytes,
    /// A codec discriminator was wrong.
    WrongTag,
    /// A codec version was wrong.
    WrongVersion,
    /// An enum, flag, or reserved byte was noncanonical.
    NonCanonical,
    /// An identity was zero, aliased, or mismatched.
    InvalidIdentity,
    /// A descriptor did not bind the authenticated Market and Terms.
    DescriptorBinding,
    /// A supplied digest did not equal the canonical digest.
    DigestMismatch,
    /// A descriptor, mint, or vault-owner PDA did not derive canonically.
    PdaMismatch,
    /// A deployment differed from the descriptor's immutable binding.
    DeploymentMismatch,
    /// Account roles alias or lack required signer/writable/executable access.
    InvalidAccountSet,
    /// A Position was closed, foreign, stale, reserved, or otherwise unusable.
    InvalidPosition,
    /// A replay sequence or generation was stale, skipped, or exhausted.
    ReplayMismatch,
    /// A Token-2022 mint or token-account projection violated the V1 profile.
    InvalidTokenProjection,
    /// An observed Token-2022 delta differed from the staged exact delta.
    TokenDeltaMismatch,
    /// The internal/external SupplyLedger did not close against kernel truth.
    SupplyClosureMismatch,
    /// A checked transition would exceed the immutable collateral cap.
    CollateralCapExceeded,
    /// A checked integer operation overflowed or underflowed.
    Arithmetic,
    /// A CPI receipt did not match the staged program, operation, or arguments.
    CpiReceiptMismatch,
    /// The final authenticated accounts differed from the fully staged post-state.
    PostStateMismatch,
    /// The structured-claim semantic core refused the route.
    StructuredClaim(clutch_structured_claim::Error),
}

impl From<clutch_structured_claim::Error> for Error {
    fn from(value: clutch_structured_claim::Error) -> Self {
        Self::StructuredClaim(value)
    }
}

/// Result alias for the adapter seam.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn is_zero(key: &Key) -> bool {
    *key == [0; 32]
}
