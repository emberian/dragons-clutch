#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact categorical claim shards with explicit Token-owned remainder.
//!
//! Immutable terms identify the Market, result domain, release, Token program,
//! selected Token behavior, denominator, and one shard Mint per Product-owned
//! outcome. An adapter-owned projection joins Claims-native custody to exact
//! Token Mint supply. This kernel persists neither observation.

mod abi;
mod transition;

pub use abi::{
    Error, FRACTIONAL_PROJECTION_HEADER_BYTES_V1, FRACTIONAL_PROJECTION_MAGIC_V1,
    FRACTIONAL_PROJECTION_ROW_BYTES_V1, FRACTIONAL_TERMS_HEADER_BYTES_V1,
    FRACTIONAL_TERMS_MAGIC_V1, FRACTIONAL_TERMS_MINT_BYTES_V1, FractionalPhaseV1,
    FractionalProjectionV1, FractionalTermsAdmissionV1, FractionalTermsV1, OutcomeReserveV1,
    Result, SCHEMA_VERSION_V1,
};
pub use transition::{
    ClaimShardDivisionV1, ClaimShardInstrumentV1, RetirePlanV1, TerminalizePlanV1,
    TransferObservationV1, TransferPlanV1, UnwrapPlanV1, WrapPlanV1, ZeroBurnPlanV1,
    divide_claim_shards_v1, prepare_open_unwrap_v1, prepare_retire_v1, prepare_terminal_redeem_v1,
    prepare_terminal_zero_burn_v1, prepare_terminalize_v1, prepare_transfer_v1, prepare_wrap_v1,
};
