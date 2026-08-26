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
mod exposure_v2;
#[allow(missing_docs)]
mod generated_abi;
mod projection_encode;
mod terms_encode;
mod transition;

pub use abi::{
    Error, FRACTIONAL_PROJECTION_HEADER_BYTES_V1, FRACTIONAL_PROJECTION_MAGIC_V1,
    FRACTIONAL_PROJECTION_ROW_BYTES_V1, FRACTIONAL_TERMS_HEADER_BYTES_V1,
    FRACTIONAL_TERMS_MAGIC_V1, FRACTIONAL_TERMS_MINT_BYTES_V1, FRACTIONAL_TERMS_SCHEMA_ID_V1,
    FRACTIONAL_TERMS_SCHEMA_PREIMAGE_V1, FractionalPhaseV1, FractionalProjectionV1,
    FractionalTermsAdmissionV1, FractionalTermsV1, OutcomeReserveV1, Result, SCHEMA_VERSION_V1,
};
pub use exposure_v2::{
    ExposureShardDivisionV2, ExposureShardInstrumentV2, ExposureTerminalPlanV2,
    ExposureTranslationBuffersV2, FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2,
    FRACTIONAL_EXPOSURE_TERMS_MAGIC_V2, FRACTIONAL_EXPOSURE_TERMS_MINT_BYTES_V2,
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_PREIMAGE_V2,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2, FractionalExposureTermsV2,
    check_fractional_exposure_bundle_v2, divide_exposure_shards_v2,
    encode_fractional_exposure_terms_v2, evaluate_exposure_terminal_v2,
    fractional_exposure_terms_bytes_v2, require_categorical_embedding_v2,
};
pub use projection_encode::{encode_fractional_projection_v1, fractional_projection_bytes_v1};
pub use terms_encode::{
    FractionalTermsInputV1, encode_fractional_terms_v1, fractional_terms_bytes_v1,
};
pub use transition::{
    ClaimShardDivisionV1, ClaimShardInstrumentV1, RetirePlanV1, TerminalizePlanV1,
    TransferObservationV1, TransferPlanV1, UnwrapPlanV1, WrapPlanV1, ZeroBurnPlanV1,
    divide_claim_shards_v1, prepare_open_unwrap_v1, prepare_retire_v1, prepare_terminal_redeem_v1,
    prepare_terminal_zero_burn_v1, prepare_terminalize_v1, prepare_transfer_v1, prepare_wrap_v1,
};
