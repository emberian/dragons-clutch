// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact composition and account-authentication boundary for ADR-0007.
//!
//! The base account structs and their semantic validation remain owned by
//! `clutch-solana-layout`; retirement owns only its appended tails and complete
//! tombstones. This crate joins those owners without changing either live SBF
//! dispatcher or the authoritative central tag registry.

mod account_auth;
mod composition;
mod generation_migration;
mod live_family_auth;
mod liveness_receipt;
mod root_bundle;
mod runtime_commit;

pub use account_auth::{
    authenticate_counted_child, authenticate_direct_epoch_v4, authenticate_direct_reservation_v6,
    authenticate_direct_reservation_v8, authenticate_epoch_budget_v1_exact,
    authenticate_general_epoch_tombstone_v1, authenticate_general_epoch_tombstone_v1_exact,
    authenticate_general_epoch_v5, authenticate_general_epoch_v5_exact,
    authenticate_general_reservation_v5, authenticate_general_reservation_v7,
    authenticate_market_v2, authenticate_market_v2_exact, authenticate_position_tombstone_v1,
    authenticate_position_tombstone_v1_exact, authenticate_position_tombstone_v2_exact,
    authenticate_position_v2, authenticate_position_v2_exact, authenticate_replay_absence_v1_exact,
    authenticate_replay_successor_v1_exact, authenticate_runtime_executable_v2,
    AbsentAccountViewV1, AccountAccessV2, AccountViewV1, AccountViewV2, AuthenticatedAccountV1,
    AuthenticatedAccountV2, CanonicalPdaV1, CountedChildSchemaV1,
};
pub use composition::{
    decode_counted_child, encode_counted_child_after_base_validation,
    project_authenticated_direct_epoch_v4, project_authenticated_epoch_budget_retirement_v1,
    project_authenticated_epoch_budget_semantic_disposition_v1, project_authenticated_position_v2,
    project_authenticated_replay_successor_v1, project_general_epoch_phase_v2,
    project_live_general_epoch_retirement_v2, DirectReservationAccountV6,
    DirectReservationAccountV8, GeneralEpochAccountV5, GeneralReservationAccountV5,
    GeneralReservationAccountV7, MarketAccountV2, PositionAccountV2, ReplaySuccessorAccountV1,
};
pub use generation_migration::{
    authenticate_and_plan_position_replay_reopen_v2,
    authenticate_and_prepare_position_replay_close_v3, AuthenticatedPositionReplayReopenV2,
    FundingPayerViewV1, NeutralSinkBalanceViewV1, PositionReplayCloseRuntimeRequestV3,
    PositionReplayRentMinimumsV1, PositionReplayReopenRuntimeRequestV2, RetirementRecipientViewV1,
    VacantPdaAccountViewV2,
};
pub use live_family_auth::{
    authenticate_epoch_child_final_absence_v2, authenticate_epoch_child_terminal_account_v2,
    authenticate_general_v2_budget_retirement_v2, authenticate_general_v2_neutral_sink_binding_v1,
    authenticate_general_v2_final_pot_terminal_v1, authenticate_general_v2_root_siblings_v1,
    authenticate_general_v2_terminal_epoch_v1, authenticate_general_v2_window_retirement_v1,
    authenticate_terminal_epoch_families_v2, AuthenticatedEpochChildFamiliesV2,
    AuthenticatedEpochChildFamilyV2, AuthenticatedGeneralV2BudgetRetirementV2,
    AuthenticatedGeneralV2FinalPotTerminalV1, AuthenticatedGeneralV2NeutralSinkBindingV1,
    AuthenticatedGeneralV2RootSiblingsV1, AuthenticatedGeneralV2TerminalEpochV1,
    AuthenticatedGeneralV2WindowRetirementV1, AuthenticatedTerminalEpochFamiliesV2,
    FamilyOwnedFinalAbsenceEpochChildV1, FamilyOwnedFinalPotTerminalV1,
    FamilyOwnedTerminalEpochChildV1, FinalPotDonationOnlyLamportDispositionV1,
    GeneralV2EpochChildParentV1, GeneralV2FinalPotLiabilityCompartmentsV1,
};
pub use liveness_receipt::{
    authenticate_retirement_receipt_v1, bind_general_v2_epoch_terminal_receipt_v1,
    bind_general_v2_final_pot_terminal_receipt_v1, AuthenticatedRetirementReceiptV1,
    RetirementReceiptAccountViewV1, RetirementReceiptErrorV1, RetirementReceiptV1,
    RETIREMENT_RECEIPT_ACCOUNT_BYTES_V1,
};
pub use root_bundle::{
    authenticate_terminal_epoch_root_bundle_v1, AuthenticatedEpochChildClassV1,
    AuthenticatedTerminalEpochRootBundleV1, EPOCH_CHILD_CLASS_CAPACITY_V1,
};
pub use runtime_commit::{
    execute_epoch_root_close_v1, execute_position_replay_close_v2,
    execute_position_replay_reopen_v2, prepare_epoch_root_close_v1,
    prepare_position_replay_close_v2, prepare_position_replay_reopen_v2,
    PositionReplayReopenRuntimeV2, PreparedEpochRootCloseV1, PreparedPositionReplayCloseV2,
    PreparedPositionReplayReopenV2, RetirementCloseRuntimeV1,
};

/// Fail-closed errors at the live-layout composition boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementAdapterErrorV1 {
    /// A retirement-tail or exact-envelope check refused.
    Retirement(clutch_retirement::RetirementErrorV1),
    /// The authoritative base layout decoder refused.
    BaseCodec(clutch_solana_layout::CodecError),
    /// A central base encoder returned a width other than its frozen constant.
    BaseLengthMismatch,
    /// The runtime account is not owned by the expected program.
    WrongOwner,
    /// The runtime address is not the canonical PDA derived by the adapter.
    WrongPda,
    /// A mutation path received a read-only account.
    NotWritable,
    /// The stored bump disagrees with the canonical derivation.
    WrongBump,
    /// A caller supplied an impossible schema geometry.
    InvalidSchema,
}

impl From<clutch_retirement::RetirementErrorV1> for RetirementAdapterErrorV1 {
    fn from(error: clutch_retirement::RetirementErrorV1) -> Self {
        Self::Retirement(error)
    }
}

impl From<clutch_solana_layout::CodecError> for RetirementAdapterErrorV1 {
    fn from(error: clutch_solana_layout::CodecError) -> Self {
        Self::BaseCodec(error)
    }
}

/// Fail-closed errors for successor retirement composition and authentication.
///
/// This distinct enum keeps the committed exhaustive
/// [`RetirementAdapterErrorV1`] surface unchanged. V1 errors lift losslessly;
/// no reverse conversion exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementAdapterErrorV2 {
    /// A successor retirement-tail or exact-envelope check refused.
    Retirement(clutch_retirement::RetirementErrorV2),
    /// The authoritative base layout decoder refused.
    BaseCodec(clutch_solana_layout::CodecError),
    /// The authoritative reference Replay codec refused.
    ReferenceCodec(clutch_solana_reference::Error),
    /// The authoritative General V2 codec or semantic owner refused.
    GeneralV2Codec(clutch_general_v2_contract::CodecError),
    /// A central base encoder returned a width other than its frozen constant.
    BaseLengthMismatch,
    /// The runtime account is not owned by the expected program.
    WrongOwner,
    /// The runtime address is not the canonical PDA derived by the adapter.
    WrongPda,
    /// A mutation path received a read-only account.
    NotWritable,
    /// A required System-transfer funding authority did not sign.
    MissingSigner,
    /// A read-only role was declared writable.
    UnexpectedWritable,
    /// A state or evidence role was executable.
    ExecutableAccount,
    /// An executable program role was not executable.
    NotExecutable,
    /// An executable program role had the wrong exact address.
    WrongProgramAddress,
    /// An absence role was not System-owned, empty, and non-executable.
    AccountNotAbsent,
    /// The stored bump disagrees with the canonical derivation.
    WrongBump,
    /// A caller supplied an impossible schema geometry.
    InvalidSchema,
}

impl From<clutch_retirement::RetirementErrorV2> for RetirementAdapterErrorV2 {
    fn from(error: clutch_retirement::RetirementErrorV2) -> Self {
        Self::Retirement(error)
    }
}

impl From<clutch_retirement::RetirementErrorV1> for RetirementAdapterErrorV2 {
    fn from(error: clutch_retirement::RetirementErrorV1) -> Self {
        Self::Retirement(error.into())
    }
}

impl From<clutch_solana_layout::CodecError> for RetirementAdapterErrorV2 {
    fn from(error: clutch_solana_layout::CodecError) -> Self {
        Self::BaseCodec(error)
    }
}

impl From<clutch_solana_reference::Error> for RetirementAdapterErrorV2 {
    fn from(error: clutch_solana_reference::Error) -> Self {
        Self::ReferenceCodec(error)
    }
}

impl From<clutch_general_v2_contract::CodecError> for RetirementAdapterErrorV2 {
    fn from(error: clutch_general_v2_contract::CodecError) -> Self {
        Self::GeneralV2Codec(error)
    }
}

impl From<RetirementAdapterErrorV1> for RetirementAdapterErrorV2 {
    fn from(error: RetirementAdapterErrorV1) -> Self {
        match error {
            RetirementAdapterErrorV1::Retirement(error) => Self::Retirement(error.into()),
            RetirementAdapterErrorV1::BaseCodec(error) => Self::BaseCodec(error),
            RetirementAdapterErrorV1::BaseLengthMismatch => Self::BaseLengthMismatch,
            RetirementAdapterErrorV1::WrongOwner => Self::WrongOwner,
            RetirementAdapterErrorV1::WrongPda => Self::WrongPda,
            RetirementAdapterErrorV1::NotWritable => Self::NotWritable,
            RetirementAdapterErrorV1::WrongBump => Self::WrongBump,
            RetirementAdapterErrorV1::InvalidSchema => Self::InvalidSchema,
        }
    }
}
