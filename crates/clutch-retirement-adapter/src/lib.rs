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
    authenticate_counted_child, authenticate_direct_reservation_v6,
    authenticate_general_epoch_tombstone_v1, authenticate_general_epoch_v5,
    authenticate_general_reservation_v5, authenticate_market_v2,
    authenticate_position_tombstone_v1, authenticate_position_v2, AccountViewV1,
    AuthenticatedAccountV1, CanonicalPdaV1, CountedChildSchemaV1,
};
pub use composition::{
    decode_counted_child, encode_counted_child_after_base_validation, DirectReservationAccountV6,
    GeneralEpochAccountV5, GeneralReservationAccountV5, MarketAccountV2, PositionAccountV2,
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
