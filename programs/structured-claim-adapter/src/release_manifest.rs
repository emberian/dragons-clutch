//! Checked capability-manifest join for the current Structured release set.
//!
//! Each runtime route authenticates a content-addressed
//! `RegistryProgramReleaseV2` for the wrapper, base, and Token-2022 program.
//! That artifact binds the exact Program address, linked ProgramData address,
//! complete ProgramData hash (including ELF), positive loader slot, and exact
//! `ObservedPositive` locus.
//! This module owns the compiled semantic manifest expected from each release.

use crate::{
    Key, IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
    STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
};

/// Wrapper capability-profile label for the unified successor development release.
pub const STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1: &str =
    "dragons-clutch/capability-profile/structured-claim-wrapper/successor-chain-attached-dev/v1/actions-1-3-5-6-7-8/account-3478c3ac6065c22916f457b36ff36dbb95acd53446f8624e3704af4574ac769a";
/// SHA-256 identity of [`STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1`].
pub const STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1: Key = [
    0xb4, 0xcd, 0x47, 0xa7, 0x1a, 0xb3, 0x22, 0x36, 0x21, 0x21, 0x78, 0xfe, 0x88, 0x0b, 0xa8, 0x19,
    0xc9, 0x82, 0x26, 0x7c, 0xb8, 0xda, 0x88, 0xa6, 0x4e, 0xaf, 0xd9, 0x4a, 0x06, 0xfe, 0xb8, 0x95,
];

/// Central base capability-profile label selected by the same development release.
pub const STRUCTURED_BASE_CAPABILITY_MANIFEST_LABEL_V1: &str =
    "dragons-clutch/capability-profile/successor-chain-attached-dev/complete-product-source-general-direct-fractional-structured-dealer-failure-release-closure/v1";
/// SHA-256 identity of [`STRUCTURED_BASE_CAPABILITY_MANIFEST_LABEL_V1`].
pub const STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1: Key = [
    0xf1, 0xd4, 0xc9, 0xbb, 0xb8, 0x9e, 0x89, 0xbf, 0x13, 0xfe, 0x0a, 0x54, 0xae, 0x82, 0x42, 0x20,
    0xdc, 0x0d, 0x11, 0x09, 0xbd, 0xf2, 0x13, 0x16, 0xe2, 0x95, 0x3a, 0xa3, 0x34, 0xaf, 0xd4, 0xca,
];

/// Token-2022 interface-manifest label selected by the Structured release.
pub const STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_LABEL_V1: &str =
    "dragons-clutch/capability-profile/token-2022-structured-claim-interface/successor-chain-attached-dev/v1/actions-1-3-5-6-7-8/account-3478c3ac6065c22916f457b36ff36dbb95acd53446f8624e3704af4574ac769a";
/// SHA-256 identity of [`STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_LABEL_V1`].
pub const STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1: Key = [
    0x14, 0x3b, 0xa4, 0x77, 0xbf, 0x97, 0x91, 0x7b, 0xf1, 0x7d, 0xa4, 0x8f, 0xdf, 0x96, 0x10, 0x1d,
    0xf6, 0x15, 0xc9, 0x26, 0xcc, 0x36, 0x90, 0x90, 0x74, 0xb6, 0x78, 0x8f, 0x13, 0x97, 0xbe, 0xdc,
];

/// Disjoint executable role in the Structured three-release join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredReleaseRoleV1 {
    /// Separately deployed Structured wrapper program.
    Wrapper = 1,
    /// Central Dragon's Clutch base program.
    Base = 2,
    /// Exact Token-2022 implementation used for wrapper and collateral effects.
    Token2022 = 3,
}

/// Compiled semantic manifest required from one exact loader release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCheckedCapabilityManifestV1 {
    /// Disjoint executable role.
    pub role: StructuredReleaseRoleV1,
    /// Human-reviewable preimage whose SHA-256 is `manifest_id`.
    pub label: &'static str,
    /// Exact capability identity stored by `RegistryProgramReleaseV2`.
    pub manifest_id: Key,
    /// Current Structured actions this release admits.
    pub admitted_action_mask: u16,
    /// Exact shared action/count/Token-effect contract.
    pub account_contract_id: Key,
}

/// Exact wrapper/base/Token-2022 semantic manifests, in deployment-binding order.
pub const STRUCTURED_CHECKED_CAPABILITY_MANIFESTS_V1:
    [StructuredCheckedCapabilityManifestV1; 3] = [
    StructuredCheckedCapabilityManifestV1 {
        role: StructuredReleaseRoleV1::Wrapper,
        label: STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1,
        manifest_id: STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
        admitted_action_mask: IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        account_contract_id: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
    },
    StructuredCheckedCapabilityManifestV1 {
        role: StructuredReleaseRoleV1::Base,
        label: STRUCTURED_BASE_CAPABILITY_MANIFEST_LABEL_V1,
        manifest_id: STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
        admitted_action_mask: IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        account_contract_id: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
    },
    StructuredCheckedCapabilityManifestV1 {
        role: StructuredReleaseRoleV1::Token2022,
        label: STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_LABEL_V1,
        manifest_id: STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
        admitted_action_mask: IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        account_contract_id: STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
    },
];

/// Intersect every release capability; no one artifact can expand the family.
pub const fn joined_structured_action_mask_v1(
    manifests: &[StructuredCheckedCapabilityManifestV1; 3],
) -> u16 {
    manifests[0].admitted_action_mask
        & manifests[1].admitted_action_mask
        & manifests[2].admitted_action_mask
}

/// Exact Structured action mask admitted by all three checked manifests.
pub const STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1: u16 =
    joined_structured_action_mask_v1(&STRUCTURED_CHECKED_CAPABILITY_MANIFESTS_V1);

const _: () = assert!(
    STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1 == IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1
);
const _: () = assert!(STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1 == 0x01ea);

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn checked_release_manifest_preimages_and_join_are_exact() {
        let expected_roles = [
            StructuredReleaseRoleV1::Wrapper,
            StructuredReleaseRoleV1::Base,
            StructuredReleaseRoleV1::Token2022,
        ];
        for (index, manifest) in STRUCTURED_CHECKED_CAPABILITY_MANIFESTS_V1
            .iter()
            .enumerate()
        {
            assert_eq!(manifest.role, expected_roles[index]);
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(manifest.label.as_bytes())),
                manifest.manifest_id,
            );
            assert_eq!(
                manifest.admitted_action_mask,
                IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
            );
            assert_eq!(
                manifest.account_contract_id,
                STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
            );
        }
        assert_eq!(
            STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1,
            IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1,
        );
        for withdrawn in [2_u8, 4_u8] {
            assert_eq!(
                STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1 & (1_u16 << withdrawn),
                0,
            );
        }
    }
}
