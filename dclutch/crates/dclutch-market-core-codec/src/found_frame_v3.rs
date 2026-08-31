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

/// Price-gate certificate raw-record index, in the extended Found frame.
///
/// **The pair is appended, and appended optionally.** A `DCLTPGT1` no-arbitrage
/// certificate is required exactly when the Market's basis declares a degree at
/// or above 2 — which is the spline family and nothing else. Every categorical
/// and graded founding that works today carries no certificate and has nothing
/// to put in these slots, so making them mandatory would mean inventing a
/// placeholder account for a record that does not exist.
///
/// So the frame takes the shape it already uses for the Rent sysvar: two
/// admissible lengths rather than one. [`FOUND_ACCOUNT_COUNT_V3`] is unchanged
/// and remains the canonical frame; [`FOUND_PRICE_GATE_ACCOUNT_COUNT_V3`] is
/// that frame plus the certificate pair. Because the pair is **last**, no
/// existing coordinate moves and no existing caller changes — which is also
/// why composed callers that forward a fixed-width Found prefix keep working
/// untouched, and simply cannot found a curved basis until they widen too.
pub const FOUND_PRICE_GATE_RAW_INDEX_V3: usize = FOUND_ACCOUNT_COUNT_V3;

/// Price-gate certificate staging-cursor index, in the extended Found frame.
pub const FOUND_PRICE_GATE_STAGING_INDEX_V3: usize = FOUND_PRICE_GATE_RAW_INDEX_V3 + 1;

/// Exact ordinary mutating `Found` V3 account count **with** a price-gate
/// certificate: 39.
pub const FOUND_PRICE_GATE_ACCOUNT_COUNT_V3: usize = FOUND_ACCOUNT_COUNT_V3 + 2;

/// Exact readonly `ProjectFound` V2 account count with a certificate: 38.
pub const PROJECT_FOUND_PRICE_GATE_ACCOUNT_COUNT_V2: usize = FOUND_PRICE_GATE_ACCOUNT_COUNT_V3 - 1;

const _: () = assert!(FOUND_CAPABILITY_MANIFEST_STAGING_INDEX_V3 < FOUND_ACCOUNT_COUNT_V3);
const _: () = assert!(FOUND_RENT_SYSVAR_INDEX_V3 < FOUND_ACCOUNT_COUNT_V3);
// The certificate pair is strictly beyond the canonical frame, so nothing
// inside it moved; and it is strictly inside the extended one, so both indices
// are addressable whenever the extension is present.
const _: () = assert!(FOUND_PRICE_GATE_RAW_INDEX_V3 >= FOUND_ACCOUNT_COUNT_V3);
const _: () = assert!(FOUND_PRICE_GATE_STAGING_INDEX_V3 < FOUND_PRICE_GATE_ACCOUNT_COUNT_V3);
// The pair sits after the Rent sysvar, so the ProjectFound left-shift reaches
// it exactly as it reaches every other coordinate past that index.
const _: () = assert!(FOUND_PRICE_GATE_RAW_INDEX_V3 > FOUND_RENT_SYSVAR_INDEX_V3);
