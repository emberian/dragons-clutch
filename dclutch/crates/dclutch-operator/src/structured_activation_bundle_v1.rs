//! Canonical root-creation artifacts for the selected Structured release.
//!
//! The eight action descriptors route Hot work; this distinct V1 descriptor is
//! the sole Core-authorized route that creates their Trading capability root.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_activation::{
    ActivationBundleInputV1, ActivationBundleV1, activation_descriptor_schema_v1,
    build_activation_bundle_v1, validate_activation_bundle_v1,
};
use dclutch_market::capability_program::v4::CapabilityProgramV4;
use dclutch_sha256_adapter::digest;

use dclutch_claims::structured_kernel::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
};

/// Selector unreachable from a Structured action request.
pub const STRUCTURED_ACTIVATION_SELECTOR_V1: u32 = 255;
/// Activation request width and domain separation.
pub const STRUCTURED_ACTIVATION_REQUEST_BYTES_V1: usize = 16;
/// Domain-separating activation request magic.
pub const STRUCTURED_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCSTACT1";
/// SHA-256 identity of the activation request grammar.
pub const STRUCTURED_ACTIVATION_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0x11, 0x5a, 0x46, 0xe4, 0x6b, 0xcb, 0x7e, 0x54, 0x76, 0xcc, 0xbe, 0xf8, 0x6a, 0x08, 0x74, 0x2c,
    0x2d, 0x5f, 0x85, 0x45, 0xab, 0x03, 0x6b, 0xca, 0x8b, 0x5a, 0x64, 0x2a, 0xe5, 0xe2, 0x7d, 0xe8,
];
/// The emitted Structured capability-tail schema and fixed root tail.
pub const STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1: [u8; 32] = [
    0x0b, 0x63, 0x65, 0x31, 0xa3, 0xc2, 0x04, 0x29, 0x12, 0x6e, 0xcc, 0x1d, 0x75, 0xad, 0xc2, 0xfa,
    0x7d, 0xde, 0x1a, 0x27, 0xe8, 0xd1, 0x95, 0x45, 0x6d, 0x48, 0x47, 0x99, 0xf6, 0x92, 0xac, 0x2c,
];
/// Fixed byte width of the Trading-owned Structured capability tail.
pub const STRUCTURED_CAPABILITY_ROOT_BYTES_V1: usize = 16;
/// Canonical active Structured capability-tail bytes.
pub const STRUCTURED_CAPABILITY_ROOT_TAIL_V1: [u8; STRUCTURED_CAPABILITY_ROOT_BYTES_V1] = [
    b'D', b'C', b'S', b'T', b'C', b'R', b'T', b'1', 1, 0, 1, 0, 0, 0, 0, 0,
];

/// The exact root-creation selector request.
pub fn structured_activation_request_v1() -> [u8; STRUCTURED_ACTIVATION_REQUEST_BYTES_V1] {
    let mut bytes = [0; STRUCTURED_ACTIVATION_REQUEST_BYTES_V1];
    bytes[..8].copy_from_slice(&STRUCTURED_ACTIVATION_REQUEST_MAGIC_V1);
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10] = u8::try_from(STRUCTURED_ACTIVATION_SELECTOR_V1).expect("selector fits U8");
    bytes
}

/// Refuse every noncanonical root-creation request.
pub fn validate_structured_activation_request_v1(bytes: &[u8]) -> bool {
    bytes == structured_activation_request_v1()
}

/// Build the one V1 descriptor that creates an active Structured capability root.
pub fn build_structured_activation_bundle_v1(
    action_descriptor: &[u8],
    funding_ledger_slot_count: u16,
) -> Option<ActivationBundleV1> {
    let action = CapabilityProgramV4::decode(action_descriptor).ok()?;
    if action.kind().to_bytes() != STRUCTURED_CAPABILITY_KIND_ID_V2
        || action.capacity_profile().to_bytes() != STRUCTURED_CAPACITY_PROFILE_ID_V2
        || action.root_schema().to_bytes() != STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1
        || usize::try_from(action.root_state_bytes()).ok()? != STRUCTURED_CAPABILITY_ROOT_BYTES_V1
    {
        return None;
    }
    let input = ActivationBundleInputV1 {
        kind: ContentId::new(STRUCTURED_CAPABILITY_KIND_ID_V2).ok()?,
        config_schema: action.config_schema(),
        request_schema: ContentId::new(STRUCTURED_ACTIVATION_REQUEST_SCHEMA_ID_V1).ok()?,
        root_schema: ContentId::new(STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1).ok()?,
        derivation_policy: action.derivation_policy(),
        capacity_profile: action.capacity_profile(),
        root_state_bytes: u32::try_from(STRUCTURED_CAPABILITY_ROOT_BYTES_V1).ok()?,
        constant_root_tail: &STRUCTURED_CAPABILITY_ROOT_TAIL_V1,
        seam_fields: &[],
        funding_ledger_slot_count,
        delivers_creation_principal: false,
    };
    let bundle = build_activation_bundle_v1(input).ok()?;
    validate_activation_bundle_v1(&bundle, input).ok()?;
    Some(bundle)
}

/// Schema carried by the ProgramSet's activation entry.
pub const fn structured_activation_descriptor_schema_v1() -> [u8; 32] {
    activation_descriptor_schema_v1()
}

/// Identity is exposed so publication can prove the exact selected descriptor.
pub fn structured_activation_descriptor_id_v1(bundle: &ActivationBundleV1) -> [u8; 32] {
    digest(&bundle.descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_request_refuses_an_action_selector() {
        let mut bytes = structured_activation_request_v1();
        bytes[10] = 6;
        assert!(!validate_structured_activation_request_v1(&bytes));
    }
}
