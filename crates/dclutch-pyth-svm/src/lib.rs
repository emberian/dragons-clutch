#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact, SDK-free byte views for the Pyth receiver and SVM Loader V3 ABIs.
//!
//! This crate accepts untrusted raw bytes but does not receive Solana account
//! metadata. An integrating adapter must separately establish the expected
//! account owner, executable bit, public keys, Program-to-ProgramData linkage,
//! and any hash or release-catalog comparison. Shape alone is never identity.

/// Exact Upgradeable Loader V3 byte views.
pub mod loader;
/// Exact Pyth Receiver SDK 2.0.0 `PriceUpdateV2` byte view.
pub mod price_update;
/// Immutable Pyth adapter release contracts and the empty production catalog.
pub mod release;

pub use loader::{LoaderV3Error, ProgramDataV3View, ProgramV3View};
pub use price_update::{FULL_PRICE_UPDATE_V2_LEN, FullPriceUpdateV2, PriceUpdateV2Error};
pub use release::{
    PRODUCTION_RELEASES, PythReleaseV1, PythReleaseV1Error, PythReleaseV1Input, ReleaseField,
    SyntheticLocalReleaseV1, SyntheticLocalReleaseV1Error, SyntheticLocalReleaseV1Input,
};
