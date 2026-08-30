//! Canonical physical coordinates of the Runtime V2 Found and ProjectFound frames.
//!
//! These coordinates are shared by the Core parser, the host-side Found
//! builder, and composed callers that forward the exact frame. Keeping the
//! manifest coordinate here prevents a records migration from silently moving
//! the record a downstream caller decodes as the capability manifest.

/// Exact ordinary mutating `Found` V3 account count.
pub const FOUND_ACCOUNT_COUNT_V3: usize = 37;

/// Rent-sysvar coordinate in the ordinary mutating Found V3 frame.
pub const FOUND_RENT_SYSVAR_INDEX_V3: usize = 28;

/// Exact readonly `ProjectFound` V2 account count.
///
/// ProjectFound presents the complete ordinary authority graph except for the
/// runtime-owned Rent sysvar. Core obtains Rent through the runtime getter;
/// every coordinate after [`FOUND_RENT_SYSVAR_INDEX_V3`] shifts left by one.
pub const PROJECT_FOUND_ACCOUNT_COUNT_V2: usize = FOUND_ACCOUNT_COUNT_V3 - 1;

/// Capability-manifest raw-record index in the ordinary V3 Found frame.
///
/// Eight source-policy accounts precede this coordinate: SourceMaterial,
/// SourceSpec, capacity profile, and manipulation floor, each as a finalized
/// raw/staging pair. The compact `ProjectedFound` frame has a different shape
/// and must not reuse this coordinate.
pub const FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3: usize = 22;

/// Capability-manifest staging-cursor index in the ordinary V3 Found frame.
pub const FOUND_CAPABILITY_MANIFEST_STAGING_INDEX_V3: usize =
    FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3 + 1;

const _: () = assert!(FOUND_CAPABILITY_MANIFEST_STAGING_INDEX_V3 < FOUND_ACCOUNT_COUNT_V3);
const _: () = assert!(FOUND_RENT_SYSVAR_INDEX_V3 < FOUND_ACCOUNT_COUNT_V3);
