#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Wire records and the onchain-safe execution candidate for shard-backed
//! Structured receipts.
//!
//! The pure kernel remains the sole owner of coefficient, backing, settlement,
//! and lifecycle arithmetic.  This contract defines the family request and root
//! byte formats, and prepares a bounded opaque candidate that common Trading Hot
//! can execute and recheck without trusting the operator.
//!
//! There is no Claims child here.  A Structured receipt's single backing edge
//! points at the exact claim-shard layer, so every Structured effect is an
//! ordinary Token-2022 effect on the receipt Mint or on one shard Mint.  Native
//! claim redemption and collateral payout stay with the shard layer, which
//! already owns them.

mod hot_v2;
mod request;
mod root;

pub use hot_v2::{
    Result as StructuredHotResultV2, STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2,
    StructuredHotAccountRefV2, StructuredHotCandidateInputV2, StructuredHotCandidateV2,
    StructuredHotErrorV2, StructuredHotRentCloseV2, StructuredHotTokenEffectV2,
    StructuredHotTokenKindV2, StructuredHotTokenPostV2,
};
pub use request::{
    Result as StructuredRequestResultV2, StructuredActionV2, StructuredRequestErrorV2,
    StructuredRequestInputV2, StructuredRequestV2,
};
pub use root::{StructuredRootInputV2, StructuredRootV2};
