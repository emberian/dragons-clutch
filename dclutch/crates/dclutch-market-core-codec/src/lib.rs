#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Generated fixed-memory interpreter for the Lean-owned Market Core ABI.
//!
//! The generated implementation is safe Rust, performs no allocation or
//! floating-point arithmetic, and accepts runtime Product widths through
//! exact-length borrowed slices. Account and release observations are adapter
//! facts; this crate performs no Solana access, hashing, CPI, or signing.

#[allow(missing_docs)]
mod generated {
    include!("generated.rs");
}

#[allow(missing_docs)]
mod generated_physical {
    include!("generated_physical.rs");
}

mod capability;
mod physical;

pub use capability::*;
pub use generated::*;
pub use generated_physical::{
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CAPABILITY_FUNDING_LIST_MAGIC_V1,
    CAPABILITY_FUNDING_MAX_ENTRIES_V1, CORE_EFFECT_ACK_BYTES_V1, CORE_EFFECT_ACK_MAGIC_V1,
    CORE_EFFECT_DIGEST_DOMAIN_V1, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CORE_EFFECT_INITIALIZE_CLAIMS_ACTION_TAG_V1, CORE_EFFECT_MAGIC_V1,
    MARKET_CORE_STATE_PDA_DOMAIN_V1, PHYSICAL_ABI_VERSION_V1, SERIES_CORE_ACK_BYTES_V1,
    SERIES_CORE_ACK_MAGIC_V1, SERIES_CORE_CALLER_AUTHORITY_PDA_DOMAIN_V1,
    SERIES_CORE_REQUEST_BYTES_V1, SERIES_CORE_REQUEST_MAGIC_V1,
};
pub use physical::*;
