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

pub use account_auth::{
    authenticate_counted_child, authenticate_direct_epoch_v4, authenticate_direct_reservation_v6,
    authenticate_direct_reservation_v8, authenticate_general_epoch_tombstone_v1,
    authenticate_general_epoch_v5, authenticate_general_reservation_v5,
    authenticate_general_reservation_v7, authenticate_market_v2,
    authenticate_position_tombstone_v1, authenticate_position_v2, AccountViewV1,
    AuthenticatedAccountV1, CanonicalPdaV1, CountedChildSchemaV1,
};
pub use composition::{
    decode_counted_child, encode_counted_child_after_base_validation,
    project_authenticated_direct_epoch_v4, project_general_epoch_phase_v2,
    project_live_general_epoch_retirement_v2, DirectReservationAccountV6,
    DirectReservationAccountV8, GeneralEpochAccountV5, GeneralReservationAccountV5,
    GeneralReservationAccountV7, MarketAccountV2, PositionAccountV2,
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

impl From<clutch_retirement::RetirementErrorV2> for RetirementAdapterErrorV2 {
    fn from(error: clutch_retirement::RetirementErrorV2) -> Self {
        Self::Retirement(error)
    }
}

impl From<clutch_solana_layout::CodecError> for RetirementAdapterErrorV2 {
    fn from(error: clutch_solana_layout::CodecError) -> Self {
        Self::BaseCodec(error)
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
