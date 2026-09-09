//! Immutable Structured entry into the existing Core/Trading native closer.

use dclutch_claims::structured_kernel::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
};
use dclutch_market::capability_program::v4::CapabilityProgramV4;
use dclutch_sha256_adapter::digest;
use dclutch_trading::native_close_bundle_v1::{
    NativeCapabilityCloseBundleV1, NativeCapabilityCloseContractV1,
    build_native_capability_close_bundle_v1,
};
use dclutch_trading::structured_root_v2::*;

/// Canonical selector in the same immutable Structured ProgramSet.
pub const STRUCTURED_ROOT_CLOSE_SELECTOR_V1: u32 = 254;
/// Exact root-close request width.
pub const STRUCTURED_ROOT_CLOSE_REQUEST_BYTES_V1: usize = 16;

/// Canonical request selecting physical root closure.
pub fn structured_root_close_request_v1() -> [u8; STRUCTURED_ROOT_CLOSE_REQUEST_BYTES_V1] {
    let mut bytes = [0; STRUCTURED_ROOT_CLOSE_REQUEST_BYTES_V1];
    bytes[..8].copy_from_slice(b"DCSTCLS1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10] = 254;
    bytes
}

/// Canonical root-close request schema identity.
pub fn structured_root_close_request_schema_v1() -> [u8; 32] {
    digest(b"dclutch/schema/structured-root-close-request-v1")
}

/// Compile the root-close predicate from the native Structured root owner.
/// The generic closer destroys the root and selected funding ledger, credits
/// canonical RentCredit, and obtains Core's atomic capability-count decrement.
pub fn build_structured_root_close_bundle_v1(
    action_descriptor: &[u8],
    funding_ledger_slot_count: u16,
) -> Option<NativeCapabilityCloseBundleV1> {
    let action = CapabilityProgramV4::decode(action_descriptor).ok()?;
    if action.kind().to_bytes() != STRUCTURED_CAPABILITY_KIND_ID_V2
        || action.capacity_profile().to_bytes() != STRUCTURED_CAPACITY_PROFILE_ID_V2
        || action.root_schema().to_bytes() != STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V2
    {
        return None;
    }
    build_native_capability_close_bundle_v1(
        action_descriptor,
        structured_root_close_request_schema_v1(),
        NativeCapabilityCloseContractV1 {
            tail_bytes: STRUCTURED_CAPABILITY_ROOT_BYTES_V2,
            magic_word: STRUCTURED_CAPABILITY_ROOT_MAGIC_WORD_V2,
            header_word: STRUCTURED_CAPABILITY_ROOT_HEADER_WORD_V2,
            obligation_count_offset: STRUCTURED_RESOURCE_GROUP_COUNT_OFFSET_V2,
            funding_ledger_slot_count,
        },
    )
    .ok()
}
