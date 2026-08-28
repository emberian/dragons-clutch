//! Canonical physical coordinates of the ordinary Runtime V2 Found frame.
//!
//! These coordinates are shared by the Core parser, the host-side Found
//! builder, and composed callers that forward the exact frame. Keeping the
//! manifest coordinate here prevents a records migration from silently moving
//! the record a downstream caller decodes as the capability manifest.

/// Exact ordinary `ProjectFound`/`Found` V3 account count.
pub const FOUND_ACCOUNT_COUNT_V3: usize = 37;

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
