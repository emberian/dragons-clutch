#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Proposed allocation-free adapter contract for the SourcePlane V3 core.
//!
//! This crate is deliberately not a live Solana instruction dispatcher. It
//! defines the fixed bodies, PDA seed recipes, model-derived V2 fixture boundary,
//! canonical intent preimages, and pure transition plans a small SBF adapter
//! can implement. The `clutch-source-plane-v3` crate remains the sole owner of
//! feed, page, window, statistic, Template, Instance, and Series semantics.
//! This crate authenticates no signature, sysvar, owner, PDA, program release,
//! oracle account, CPI, rent quote, or token balance by itself.
//!
//! V2 interoperability is a checked reprojection. No V2 account body, record,
//! page commitment, window identity, or PDA is accepted as a V3 identity.

mod account;
mod intent;
mod pda;
mod transition;
mod v2;

pub use account::{
    canonical_account_state_digest, decode_account, encode_account, AccountBodyV3, AccountFamilyV3,
    AccountHeaderV3, ACCOUNT_HEADER_BYTES, ACCOUNT_LAYOUT_VERSION,
};
pub use intent::{IntentPreimageV3, INTENT_PREIMAGE_BYTES};
pub use pda::{PdaFamilyV3, PdaRecipeV3, SeedComponentV3, MAX_PDA_SEEDS};
pub use transition::{
    project_activate_series, project_advance_existing_instance, project_append_v2_boundary,
    project_create_next_instance, project_create_window_work, project_fold_window_page,
    project_initialize_source_head, project_lapse_next_instance, project_open_raw_page,
    project_refund_series_funding, project_runtime_append_boundary,
    project_runtime_fold_window_page, project_runtime_initialize_source_head,
    project_runtime_initialize_window_work, project_runtime_open_raw_page,
    project_runtime_seal_raw_page, project_runtime_seal_window, project_seal_window,
    project_write_drawdown_result, project_write_terminal_result, AccountCloseV3,
    AccountClosureV3, AccountCreationV3, AccountMutationV3, AccountStateV3,
    AuthenticatedInstanceV3, AuthenticatedRawPageV3, CoreTransitionV3,
    RuntimeCloseProjectionV1, RuntimeCreationProjectionV1, RuntimeMutationProjectionV1,
    SeriesActivationTransfersV3, SeriesBindingsV3, SeriesInstantiationTransfersV3,
    SourceGenesisAuthorizationV3, StateMutationV3, TerminalEvaluationV3, TransitionActionV3,
    TransitionPlanV3, VerifiedDrawdownEvaluationV3, WindowWorkLineageV3, MAX_CLOSES,
    MAX_CREATIONS, MAX_MUTATIONS, TRANSITION_PLAN_BYTES,
};
pub use v2::{
    project_v2_source_spec_fixture, V2AccountView, V2ArchiveRecord, V2AuthenticatedRecord,
    V2AuthenticatedSourceRoute, V2SourceSpecBinding, V2SourceSpecRefusal, ARCHIVE_RECORD_V2_BYTES,
    SOURCE_SPEC_ACCOUNT_V2_BYTES,
};

use clutch_source_plane_v3::Error as CoreError;
use clutch_terminal_identity_v1::Error as TerminalError;

/// A fail-closed refusal from the proposed adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The semantic core refused a value or transition.
    Core(CoreError),
    /// The shared terminal-identity codec refused an account header.
    Terminal(TerminalError),
    /// Input or output was not the exact fixed width.
    WrongLength,
    /// A discriminator or digest domain did not match.
    WrongMagic,
    /// A version was not exactly the registered version.
    BadVersion,
    /// An enum, scalar, count, or fixed shape was outside its domain.
    InvalidParameter,
    /// A required identity was the reserved all-zero value.
    ZeroIdentity,
    /// Reserved or inactive fixed-width bytes were not canonical zeroes.
    NonCanonicalPadding,
    /// The account family did not own the supplied core body width.
    WrongAccountFamily,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// An expected core binding or before-state did not match.
    MismatchedState,
    /// The model-derived V2 fixture projection refused metadata or body bytes.
    V2SourceSpec(V2SourceSpecRefusal),
    /// A V2 record is valid in V2 but cannot be represented under V3 rules.
    V2ProjectionUnavailable,
    /// The current crate has no live V2 runtime-auth capability constructor.
    V2RuntimeAuthenticationUnavailable,
    /// The current crate has no page-root-authenticated evaluator capability.
    EvaluatorAuthenticationUnavailable,
    /// A close/reopen generation needs a durable lineage owner not yet present.
    ReopenGenerationUnavailable,
    /// The current crate has no checked typed Series asset/vault transfer graph.
    SeriesTransferGraphUnavailable,
    /// Terminal Series refund has no checked typed asset/vault disposition yet.
    SeriesTerminalRefundUnavailable,
}

impl From<CoreError> for Error {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

impl From<TerminalError> for Error {
    fn from(value: TerminalError) -> Self {
        Self::Terminal(value)
    }
}

/// Result alias for the proposed adapter boundary.
pub type Result<T> = core::result::Result<T, Error>;
