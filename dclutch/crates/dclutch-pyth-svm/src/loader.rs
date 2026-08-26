//! Shared exact, unauthenticated Upgradeable Loader V3 byte views.
//!
//! Loader byte layout has one semantic owner in `dclutch-registry-svm`.
//! Pyth consumers retain these names only as compatibility re-exports; they do
//! not carry a second parser or a second ELF-offset rule.

pub use dclutch_registry_svm::{
    Error as LoaderV3Error, LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES,
    ProgramDataV3View, ProgramV3View,
};

/// Result alias for the shared Upgradeable Loader V3 views.
pub type LoaderV3Result<T> = core::result::Result<T, LoaderV3Error>;
