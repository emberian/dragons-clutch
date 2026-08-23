#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Runtime-facing account contract for SourcePlane V3.
//!
//! This crate promotes the V3 core's former default-deny seams into concrete,
//! fixed-memory joins over complete account bytes, deployment releases,
//! parser invocations, Clock windows, page lineage, evaluator results,
//! Product/Series occurrence records, and exact native-lamport disposition.
//! It remains below the Solana adapter: the adapter must obtain account owner,
//! signer/writable/executable, PDA derivation, CPI invocation, return-data, and
//! sysvar facts from the runtime before presenting them here.
//!
//! A typed adapter attestation is an explicitly unverified boundary, not a
//! cryptographic proof. This crate recomputes every digest and semantic join it
//! can from supplied bytes and refuses free-form booleans or caller-selected
//! opaque authorization hashes.

mod account;
mod auth;
mod funding;
mod ingest;
mod lineage;
mod window;

pub use account::{
    canonical_runtime_account_data_id, decode_runtime_account, encode_runtime_account,
    observe_runtime_account_header, registered_runtime_account_tag, RuntimeAccountBodyV1,
    RuntimeAccountHeaderV1, OPEN_RAW_PAGE_ACCOUNT_TAG, RAW_PAGE_ACCOUNT_TAG,
    RUNTIME_ACCOUNT_GLOBAL_VERSION, RUNTIME_ACCOUNT_HEADER_BYTES, RUNTIME_ACCOUNT_LAYOUT_VERSION,
    SOURCE_HEAD_ACCOUNT_TAG, STATISTIC_RESULT_ACCOUNT_TAG, WINDOW_SEAL_ACCOUNT_TAG,
    WINDOW_WORK_ACCOUNT_TAG,
};
pub use auth::{
    account_data_id, authenticate_boundary, authenticate_source_release_account,
    authenticate_source_route, AdapterInvocationV1, AuthenticatedBoundaryV1,
    AuthenticatedClockBucketV1, AuthenticatedSourceReleaseV1, AuthenticatedSourceRouteV1,
    ClockPolicyV1, ClockSnapshotV1, DeploymentBindingV1, ParserOutputV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey, SourceReleaseManifestV1, CLOCK_POLICY_BYTES,
    SOURCE_RELEASE_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_VERSION, SOURCE_RELEASE_MANIFEST_BYTES,
};
pub use funding::{
    authenticate_source_work_receipt_account, plan_runtime_account_close_from_header,
    plan_source_account_close, plan_source_account_creation, AccountCloseFundingV1,
    AccountCreationFundingV1, AuthenticatedSourceWorkReceiptV1, RentExemptionQuoteV1,
    SourceAccountFundingLedgerV1, SourceReceiptDispositionV1, SourceTerminalAuthorizationV1,
    SourceTerminalOutcomeV1, SourceWorkAuthorizationV1, SourceWorkKindV1,
    SourceWorkReceiptAccessV1, SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1,
    SOURCE_WORK_RECEIPT_ACCOUNT_BYTES, SOURCE_WORK_RECEIPT_ACCOUNT_TAG,
    SOURCE_WORK_RECEIPT_ACCOUNT_VERSION, SOURCE_WORK_SCHEDULE_BYTES,
};
pub use ingest::{
    authenticate_open_raw_page_account, authenticate_source_generation_request,
    authenticate_source_head_account, ingest_boundary_batch, initialize_source_head,
    seal_authenticated_open_page, AuthenticatedOpenRawPageV1, AuthenticatedSourceGenerationV1,
    AuthenticatedSourceHeadV1, BoundaryBatchV1, IngestBatchOutputV1, SealBatchModeV1,
    SealOpenPageOutputV1, SourceGenerationRequestV1, MAX_BOUNDARIES_PER_INGEST,
    SOURCE_GENERATION_REQUEST_BYTES,
};
pub use lineage::{
    advance_lineage_state, authenticate_reopen_lineage_account, authorize_reopen,
    close_lineage_generation, open_lineage_generation, AuthenticatedReopenLineageV1,
    LineageAccessV1, LineageFamilyV1, ReopenAuthorizationV1, ReopenLineageV1,
    REOPEN_LINEAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_VERSION, REOPEN_LINEAGE_BYTES,
};
pub use window::{
    authenticate_evaluation_authority, authenticate_raw_page_account,
    authenticate_statistic_result, authenticate_statistic_result_absence,
    authenticate_statistic_result_account, authenticate_window_seal_account,
    authenticate_window_work_account, fold_authenticated_pages, join_source_occurrence,
    seal_authenticated_window, source_occurrence_record_id, AuthenticatedEvaluationV1,
    AuthenticatedRawPageV1, AuthenticatedStatisticResultAbsenceV1,
    AuthenticatedStatisticResultAccountV1, AuthenticatedWindowEvidenceV1,
    AuthenticatedWindowSealAccountV1, AuthenticatedWindowWorkV1, EvaluationAuthorityV1,
    EvaluationReleaseBindingV1, FailurePolicySourceHandoffV1, FoldPagesOutputV1,
    OccurrenceDispositionV1, OccurrenceSourceReceiptV1, SourceFailureKindV1,
    SourcePolicyHandoffJoinV1, SuccessfulEvaluationHandoffV1, MAX_PAGES_PER_FOLD,
    SOURCE_OCCURRENCE_RECORD_BYTES,
};

use clutch_source_plane_v3::Error as CoreError;
use clutch_source_plane_v3_adapter::Error as AdapterError;

/// Fail-closed runtime-contract refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// SourcePlane semantic core refusal.
    Core(CoreError),
    /// SourcePlane account-envelope refusal.
    Adapter(AdapterError),
    /// A required runtime key or content identity was zero.
    ZeroIdentity,
    /// Two roles that must be disjoint used the same identity.
    IdentityAlias,
    /// Runtime account key did not match its immutable binding.
    WrongAccount,
    /// Runtime account owner did not match its immutable binding.
    WrongOwner,
    /// Runtime executable state did not match the account role.
    WrongExecutableState,
    /// Runtime signer or writable privilege did not match the operation.
    WrongPrivilege,
    /// Complete account bytes did not match the frozen release digest.
    WrongAccountData,
    /// Program account did not link the frozen ProgramData address.
    WrongProgramDataLink,
    /// ProgramData deployment slot did not match the release manifest.
    WrongDeploymentSlot,
    /// The SBF adapter's runtime-derived PDA facts did not bind this account.
    WrongPda,
    /// Parser/evaluator invocation or return-data facts did not bind this call.
    WrongInvocation,
    /// Clock or publication facts lay outside the immutable admission window.
    OutsideClockWindow,
    /// A source, release, generation, page, Window, or occurrence join differed.
    MismatchedBinding,
    /// Batch count or fixed-capacity shape was invalid.
    InvalidCount,
    /// A fixed codec had the wrong length, magic, version, or padding.
    InvalidCodec,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// Reopen lineage was stale, active, skipped, or otherwise noncanonical.
    InvalidLineage,
    /// An account creation debit or observed balance was not exact.
    FundingMismatch,
    /// An account close attempted to reclassify principal or donation.
    CloseMismatch,
    /// A failure handoff did not carry an exact stable source fact.
    InvalidFailureHandoff,
}

impl From<CoreError> for Error {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

impl From<AdapterError> for Error {
    fn from(value: AdapterError) -> Self {
        Self::Adapter(value)
    }
}

/// Result alias for the runtime contract.
pub type Result<T> = core::result::Result<T, Error>;
