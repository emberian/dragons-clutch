#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Finalized physical artifacts for exact fractional categorical claims.
//!
//! This contract keeps the pure allocation kernel as the sole owner of
//! denomination, remainder, reserve, and lifecycle arithmetic. It only joins
//! independently finalized generic interpreter artifacts and defines the
//! family request/root wire formats used by a future thin SBF adapter.

mod artifacts;
mod request;
mod root;

pub use artifacts::{
    ArtifactAdmissionV1, FractionalArtifactAdmissionsV1, FractionalArtifactBundleV1,
    FractionalArtifactBytesV1, FractionalArtifactErrorV1, FractionalArtifactSelectionV1,
    FractionalChildProgramsV1, Result as ArtifactResult,
    authenticate_fractional_artifact_bundle_v1,
};
pub use request::{
    FRACTIONAL_FAMILY_REQUEST_BYTES_V1, FRACTIONAL_FAMILY_REQUEST_MAGIC_V1,
    FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1, FRACTIONAL_FAMILY_REQUEST_SCHEMA_PREIMAGE_V1,
    FractionalActionV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    FractionalRequestErrorV1, NO_TERMINAL_OUTCOME_V1, Result as RequestResult,
};
pub use root::{
    FRACTIONAL_ROOT_BYTES_V1, FRACTIONAL_ROOT_MAGIC_V1, FRACTIONAL_ROOT_SCHEMA_ID_V1,
    FRACTIONAL_ROOT_SCHEMA_PREIMAGE_V1, FractionalRootInputV1, FractionalRootV1,
};

/// Finalized capability-kind semantic identity.
pub const FRACTIONAL_CAPABILITY_KIND_ID_V1: [u8; 32] = [
    0x99, 0x3d, 0xb3, 0xca, 0x5f, 0xfc, 0x4d, 0x67, 0x42, 0xe0, 0x48, 0x77, 0x9f, 0x8d, 0xac,
    0xa2, 0x3c, 0x18, 0x06, 0xc0, 0x3b, 0x27, 0xab, 0x19, 0x39, 0x9c, 0xd8, 0x68, 0x83, 0xcc,
    0x1e, 0x9b,
];
