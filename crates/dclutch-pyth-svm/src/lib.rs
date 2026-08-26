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
/// Exact borrowed Pyth Receiver `PostUpdateParams` body view.
pub mod post_update;
/// Exact Pyth Receiver SDK 2.0.0 `PriceUpdateV2` byte view.
pub mod price_update;
/// Exact Pyth Receiver SDK 2.0.0 `Config` account view.
pub mod receiver_config;
/// Immutable Pyth adapter release contracts and the empty production catalog.
pub mod release;
/// Exact borrowed Wormhole router account views used by Pyth Receiver.
pub mod router_accounts;
/// Provenance-pinned synthetic-local release facts, never production catalog data.
#[cfg(feature = "synthetic-local-fixture")]
pub mod synthetic_fixture;

pub use loader::{LoaderV3Error, ProgramDataV3View, ProgramV3View};
pub use post_update::{POST_UPDATE_PROOF_ELEMENT_LEN, PostUpdateParamsError, PostUpdateParamsView};
pub use price_update::{FULL_PRICE_UPDATE_V2_LEN, FullPriceUpdateV2, PriceUpdateV2Error};
pub use receiver_config::{
    DATA_SOURCE_V2_LEN, DataSourceV2View, RECEIVER_CONFIG_V2_DISCRIMINATOR, RECEIVER_CONFIG_V2_LEN,
    ReceiverConfigV2Error, ReceiverConfigV2View,
};
pub use release::{
    PRODUCTION_RELEASES, PYTH_RELEASE_V1_ENCODED_LEN, PYTH_RELEASE_V1_MAGIC,
    PYTH_RELEASE_V1_SCHEMA_VERSION, PythReleaseV1, PythReleaseV1Error, PythReleaseV1Input,
    ReleaseField, SyntheticLocalReleaseV1, SyntheticLocalReleaseV1Error,
    SyntheticLocalReleaseV1Input,
};
pub use router_accounts::{
    ENCODED_VAA_DISCRIMINATOR_V1, ENCODED_VAA_HEADER_BYTES_V1, ENCODED_VAA_VERIFIED_STATUS_V1,
    GUARDIAN_ADDRESS_BYTES_V1, GUARDIAN_SET_HEADER_BYTES_V1, GuardianSetV1, RouterAccountErrorV1,
    VerifiedEncodedVaaV1,
};
#[cfg(feature = "synthetic-local-fixture")]
pub use synthetic_fixture::{
    LOCAL_VALIDATOR_LABEL_V1, SYNTHETIC_LOCAL_LABEL_V1, local_validator_release_v1,
    synthetic_local_release_v1,
};
