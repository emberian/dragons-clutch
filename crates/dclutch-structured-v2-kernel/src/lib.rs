#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Structured receipts backed by exact claim shards.
//!
//! One Structured receipt atom is backed by `c_i` exact claim-shard atoms of
//! every representation coordinate `i`.  For receipt supply `S` and Structured
//! shard custody `K_i` the exact backing invariant is
//!
//! ```text
//! K_i = S * c_i
//! ```
//!
//! Because the shard layer already denominates one native claim into `D`
//! transferable atoms, a receipt denotes exactly `c_i / D` native claims without
//! a residual credit, a remainder ledger, or any rounding.  This removes the
//! Structured V1 restriction that a portfolio recipe's least realization lot had
//! to equal the Product denominator.
//!
//! The representation graph is the deliberately finite depth-two chain
//! `Structured receipt -> exact claim shard -> native Position -> Market
//! liability`.  Each node has one supply owner, one backing edge, and a strictly
//! decreasing rank, so a receipt can never be backed by a receipt.  The kernel
//! enforces the physical form of that rule by refusing terms whose receipt Mint
//! aliases any shard Mint.
//!
//! Structured owns no quotient/remainder boundary.  Terminal settlement derives
//! every row through `dclutch_fractional_claim_kernel::divide_exposure_shards_v2`,
//! and a sub-denominator remainder stays an ordinary transferable shard atom of
//! the same Mint.
//!
//! A Structured shard custody account is an ordinary Token account, so anyone may
//! donate into it.  The projection therefore requires only solvency
//! (`observed >= required`) and names the difference `surplus_shard_custody`.  No
//! plan reads, spends, or distributes it, and retirement refuses while it is
//! nonzero.
//!
//! `DClutchSemantics.StructuredV2` proves conservation, exact backing
//! preservation, replay protection, rank-decreasing acyclicity, terminal-zero
//! honesty, and change aggregability, and owns the fixed byte layout emitted into
//! `src/generated_abi.rs`.  This kernel does not parse Solana accounts, own
//! replay state, persist balances, call Token-2022, or authorize a second Claims
//! or Custody writer.

mod abi;
#[allow(missing_docs)]
mod generated_abi;
mod projection_encode;
mod terms_encode;
mod transition;

pub use abi::{
    Error, Result, STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPABILITY_KIND_PREIMAGE_V2,
    STRUCTURED_CAPACITY_PROFILE_ID_V2, STRUCTURED_CAPACITY_PROFILE_PREIMAGE_V2,
    STRUCTURED_MAX_COORDINATES_V2, STRUCTURED_MIN_COORDINATES_V2, STRUCTURED_MIN_DENOMINATOR_V2,
    STRUCTURED_NO_COORDINATE_V2, STRUCTURED_PHASE_OPEN_V2, STRUCTURED_PHASE_RETIRED_V2,
    STRUCTURED_PHASE_TERMINAL_V2, STRUCTURED_PROJECTION_HEADER_BYTES_V2,
    STRUCTURED_PROJECTION_MAGIC_V2, STRUCTURED_PROJECTION_ROW_BYTES_V2,
    STRUCTURED_RECEIPT_DECIMALS_V2, STRUCTURED_SCHEMA_VERSION_V2,
    STRUCTURED_TERMS_COEFFICIENT_BYTES_V2, STRUCTURED_TERMS_HEADER_BYTES_V2,
    STRUCTURED_TERMS_MAGIC_V2, STRUCTURED_TERMS_SCHEMA_ID_V2, STRUCTURED_TERMS_SCHEMA_PREIMAGE_V2,
    StructuredCoordinateObservationV2, StructuredPhaseV2, StructuredProjectionV2,
    StructuredTermsAdmissionV2, StructuredTermsV2,
};
pub use generated_abi::{
    STRUCTURED_ACTION_ISSUE_V2, STRUCTURED_ACTION_TERMINAL_REDEEM_V2, STRUCTURED_ACTION_UNWRAP_V2,
    STRUCTURED_ACTION_ZERO_SUPPLY_RETIRE_V2, STRUCTURED_REQUEST_ACTION_OFFSET_V2,
    STRUCTURED_REQUEST_BYTES_V2, STRUCTURED_REQUEST_EXPECTED_REVISION_OFFSET_V2,
    STRUCTURED_REQUEST_MAGIC_OFFSET_V2, STRUCTURED_REQUEST_MAGIC_V2,
    STRUCTURED_REQUEST_MARKET_OFFSET_V2, STRUCTURED_REQUEST_OWNER_OFFSET_V2,
    STRUCTURED_REQUEST_PRODUCT_RECORD_OFFSET_V2, STRUCTURED_REQUEST_QUANTITY_OFFSET_V2,
    STRUCTURED_REQUEST_RECEIPT_DESTINATION_OFFSET_V2, STRUCTURED_REQUEST_RECEIPT_SOURCE_OFFSET_V2,
    STRUCTURED_REQUEST_RELEASE_SET_OFFSET_V2, STRUCTURED_REQUEST_RESERVED_HEADER_OFFSET_V2,
    STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V2, STRUCTURED_REQUEST_RESULT_DOMAIN_OFFSET_V2,
    STRUCTURED_REQUEST_SCHEMA_ID_V2, STRUCTURED_REQUEST_SCHEMA_PREIMAGE_V2,
    STRUCTURED_REQUEST_SHARD_EXPOSURE_OFFSET_V2, STRUCTURED_REQUEST_SHARD_TERMS_OFFSET_V2,
    STRUCTURED_REQUEST_TERMINAL_DIGEST_OFFSET_V2, STRUCTURED_REQUEST_TERMS_OFFSET_V2,
    STRUCTURED_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2, STRUCTURED_REQUEST_VERSION_OFFSET_V2,
    STRUCTURED_ROOT_BUMP_OFFSET_V2, STRUCTURED_ROOT_BYTES_V2, STRUCTURED_ROOT_MAGIC_OFFSET_V2,
    STRUCTURED_ROOT_MAGIC_V2, STRUCTURED_ROOT_MARKET_OFFSET_V2,
    STRUCTURED_ROOT_RENT_BENEFICIARY_OFFSET_V2, STRUCTURED_ROOT_RENT_PRINCIPAL_OFFSET_V2,
    STRUCTURED_ROOT_RESERVED_HEADER_OFFSET_V2, STRUCTURED_ROOT_REVISION_OFFSET_V2,
    STRUCTURED_ROOT_SCHEMA_ID_V2, STRUCTURED_ROOT_SCHEMA_PREIMAGE_V2,
    STRUCTURED_ROOT_TERMS_OFFSET_V2, STRUCTURED_ROOT_VERSION_OFFSET_V2,
};
pub use projection_encode::{encode_structured_projection_v2, structured_projection_bytes_v2};
pub use terms_encode::{
    StructuredTermsInputV2, encode_structured_terms_v2, structured_terms_bytes_v2,
};
pub use transition::{
    ReceiptEffectV2, ShardMovementV2, StructuredIssuePlanV2, StructuredReleasePlanV2,
    StructuredRetirePlanV2, StructuredSettlementRowV2, StructuredTerminalPlanV2,
    plan_structured_issue_v2, plan_structured_retire_v2, plan_structured_terminal_redeem_v2,
    plan_structured_unwrap_v2,
};
