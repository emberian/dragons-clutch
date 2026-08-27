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

mod frame;
mod hot_v2;
mod request;
mod root;
mod seeds;

pub use frame::{
    Result as StructuredFrameResultV2, STRUCTURED_ACCOUNT_ACTIVATION_CACHE_V2,
    STRUCTURED_ACCOUNT_ACTOR_V2, STRUCTURED_ACCOUNT_CALLER_AUTHORITY_V2,
    STRUCTURED_ACCOUNT_CALLER_PROGRAM_V2, STRUCTURED_ACCOUNT_CALLER_PROGRAMDATA_V2,
    STRUCTURED_ACCOUNT_CLAIMS_PROGRAM_V2, STRUCTURED_ACCOUNT_CLAIMS_PROGRAMDATA_V2,
    STRUCTURED_ACCOUNT_CORE_MARKET_V2, STRUCTURED_ACCOUNT_CORE_PROGRAM_V2,
    STRUCTURED_ACCOUNT_CORE_PROGRAMDATA_V2, STRUCTURED_ACCOUNT_RECEIPT_MINT_V2,
    STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2, STRUCTURED_ACCOUNT_REGISTRY_PROGRAM_V2,
    STRUCTURED_ACCOUNT_RENT_CREDIT_V2, STRUCTURED_ACCOUNT_RENT_PROGRAM_V2,
    STRUCTURED_ACCOUNT_RENT_SYSVAR_V2, STRUCTURED_ACCOUNT_ROOT_V2,
    STRUCTURED_ACCOUNT_SHARD_TERMS_RAW_V2, STRUCTURED_ACCOUNT_SHARD_TERMS_STAGING_V2,
    STRUCTURED_ACCOUNT_SYSTEM_PROGRAM_V2, STRUCTURED_ACCOUNT_TERMS_RAW_V2,
    STRUCTURED_ACCOUNT_TERMS_STAGING_V2, STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2,
    STRUCTURED_ASSET_ACCOUNT_COUNT_V2, STRUCTURED_ASSET_ACTOR_SHARD_V2,
    STRUCTURED_ASSET_CUSTODY_SHARD_V2, STRUCTURED_ASSET_SHARD_MINT_V2,
    STRUCTURED_BASE_ACCOUNT_COUNT_V2, StructuredFrameEffectSlotsV2, StructuredFrameErrorV2,
    StructuredFrameSpecV2, structured_account_is_active_v2, structured_account_is_writable_v2,
    structured_frame_effect_slots_v2,
};
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
pub use seeds::{
    Result as StructuredSeedResultV2, STRUCTURED_RECEIPT_MINT_PDA_SEED_V2,
    STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2, STRUCTURED_ROOT_PDA_SEED_V2,
    STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2, StructuredReceiptMintSeedsV2, StructuredRootSeedsV2,
    StructuredSeedErrorV2, StructuredShardCustodySeedsV2,
    structured_receipt_mint_preimage_bytes_v2, structured_receipt_mint_preimage_v2,
    structured_terms_bytes_for_width_v2,
};
