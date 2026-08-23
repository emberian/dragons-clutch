//! Compile-time identity and wire admission for one deployable product.
//!
//! Capability profiles are protocol identities, not build-size aliases.  A
//! profile fixes which canonical instruction families this ELF can decode and
//! execute.  Disabled canonical tags refuse before any account is read.  The
//! label and SHA-256 identity below are copied into the artifact manifest by
//! `scripts/measure_capability_profiles.py`; changing either the membership or
//! the label therefore creates a different release identity.

/// Full research/runtime surface retained by the historical default build.
#[cfg(feature = "profile-full")]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/full/v1";
/// Direct V3, Source V2, and archive-direct exact-point d1-d3 resolution product.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/direct-v3-source-v2-point/v1";
/// General clearing, Source V2, and archive-direct exact-point d1-d3 resolution product.
#[cfg(feature = "profile-general-source-v2-point")]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/general-source-v2-point/v1";
/// Non-production General V2 empty-book identity laboratory.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-general-v2-empty-book-identity-lab/v5";

/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-full")]
pub const PROFILE_ID: [u8; 32] = [
    0xf2, 0x06, 0x66, 0x13, 0x61, 0x0b, 0x8e, 0x3c, 0xff, 0x18, 0x48, 0x5d, 0x2e, 0x6f, 0x3e, 0x3c,
    0x9f, 0xdc, 0xfc, 0xbb, 0x75, 0x7b, 0x46, 0xb4, 0x07, 0x73, 0x3e, 0xa1, 0x5c, 0x5e, 0x9a, 0xc8,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_ID: [u8; 32] = [
    0xb7, 0x35, 0x87, 0x22, 0x84, 0x69, 0x1c, 0xed, 0x6a, 0x71, 0x29, 0xe4, 0x58, 0x83, 0x3e, 0x21,
    0x21, 0x79, 0x30, 0x09, 0xba, 0xd1, 0x2c, 0x45, 0xe6, 0xcb, 0xaa, 0x3c, 0x88, 0x6e, 0x78, 0x97,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-general-source-v2-point")]
pub const PROFILE_ID: [u8; 32] = [
    0x1f, 0x9e, 0x2f, 0x27, 0x4c, 0x09, 0xa8, 0x30, 0x14, 0x50, 0x60, 0xef, 0xe1, 0x70, 0x91, 0x28,
    0x78, 0x0a, 0x12, 0x72, 0xc0, 0x83, 0xc7, 0xc2, 0x25, 0x4f, 0x35, 0x3a, 0xa7, 0x8b, 0xf8, 0x20,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const PROFILE_ID: [u8; 32] = [
    0x6e, 0x0e, 0xb3, 0x55, 0xbd, 0x8b, 0xf2, 0x0b, 0xd6, 0x72, 0x2c, 0xfd, 0x32, 0x5c, 0x73, 0x08,
    0x92, 0xb0, 0x23, 0x41, 0x0d, 0xc2, 0x88, 0x52, 0x05, 0xaf, 0x0f, 0xbe, 0x68, 0xb7, 0x0e, 0xda,
];

/// Whether this artifact is the explicitly non-production identity lab.
pub const GENERAL_V2_IDENTITY_LAB: bool =
    cfg!(feature = "profile-non-production-general-v2-empty-book-identity-lab");

/// Whether the profile contains legacy Source V1 ingestion and resolution.
pub const SOURCE_V1: bool = cfg!(feature = "profile-full");
/// Whether the profile contains Source V2 ingestion and resolution.
pub const SOURCE_V2: bool = !GENERAL_V2_IDENTITY_LAB;
/// Whether the profile contains legacy Direct V2 clearing.
pub const DIRECT_V2: bool = cfg!(feature = "profile-full");
/// Whether the profile contains Direct V3 clearing.
pub const DIRECT_V3: bool = cfg!(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
));
/// Whether the profile contains general clearing.
pub const GENERAL_CLEARING: bool = cfg!(any(
    feature = "profile-full",
    feature = "profile-general-source-v2-point"
));
/// Whether the profile contains occupation and resumable resolution.
pub const OCCUPATION_RESOLUTION: bool = cfg!(feature = "profile-full");

/// Return whether one canonical legacy Intent tag belongs to this product.
///
/// Direct V3 tags `36..=46` use their own strict decoder and are handled by
/// [`direct_v3_tag_enabled`].  Unknown values are false.
pub const fn legacy_intent_tag_enabled(tag: u8) -> bool {
    match tag {
        // Common construction, custody, trading, exit and artifact plane.
        1..=5 | 7 | 10..=21 | 68 | 70..=73 => !GENERAL_V2_IDENTITY_LAB,
        // The old feed buffer, direct-page settlement and Source V1 families.
        6 | 22..=31 => cfg!(feature = "profile-full"),
        // Resumable occupation work.
        32..=35 => cfg!(feature = "profile-full"),
        // General clearing and its terminal routes. ClosePosition remains
        // disabled with the family because its current implementation is
        // owned by the general terminal-closure ledger.
        8..=9 | 47..=67 | 69 => cfg!(any(
            feature = "profile-full",
            feature = "profile-general-source-v2-point"
        )),
        _ => false,
    }
}

/// Return whether an exact legacy intent tag/version pair belongs to this product.
///
/// This is the version-aware admission boundary.  The tag-only helper remains
/// for compile-time profile descriptions, but dispatch must use this function
/// so a future version cannot inherit version-3 capability accidentally.
pub const fn legacy_intent_enabled(tag: u8, version: u8) -> bool {
    version == clutch_solana_layout::registry::LEGACY_INTENT_VERSION
        && legacy_intent_tag_enabled(tag)
}

/// Return whether one dedicated Direct V3 tag belongs to this product.
pub const fn direct_v3_tag_enabled(tag: u8) -> bool {
    (tag >= 36 && tag <= 46) && DIRECT_V3
}

/// Return whether an exact dedicated Direct V3 tag/version pair belongs to this product.
pub const fn direct_v3_intent_enabled(tag: u8, version: u8) -> bool {
    version == clutch_solana_layout::registry::LEGACY_INTENT_VERSION && direct_v3_tag_enabled(tag)
}

/// Return whether a family-local action has an allocation in the central registry.
///
/// Allocation does not imply execution capability. General V2, SourcePlane V3
/// actions 1 through 12, and recurring-Series actions 13 through 18 have
/// registered local actions; every exact tuple remains separately disabled
/// until its handler is admitted.
pub const fn extension_intent_action_allocated(
    family_tag: u8,
    family_version: u8,
    local_action: u8,
) -> bool {
    matches!(
        clutch_solana_layout::registry::decode_extension_action(
            family_tag,
            family_version,
            local_action,
        ),
        Ok(
            clutch_solana_layout::registry::ExtensionAction::GeneralV2(_)
                | clutch_solana_layout::registry::ExtensionAction::SourceV3(_)
                | clutch_solana_layout::registry::ExtensionAction::RecurringSeries(_)
        )
    )
}

/// Exact extension actions executable by this product.
///
/// The empty slice is the mechanical activation gate for this registry-only
/// wave.  A later runtime wave must add each exact `(family, version, action)`
/// tuple atomically with its handler and account contract.
#[cfg(not(feature = "profile-non-production-general-v2-empty-book-identity-lab"))]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[];

/// Exact identity, unrevealed-expiry, and solver-claim action set.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[
    (74, 1, 2),
    (74, 1, 6),
    (74, 1, 7),
    (74, 1, 8),
    (74, 1, 9),
    (74, 1, 10),
    (74, 1, 14),
    (74, 1, 15),
    (74, 1, 16),
    (74, 1, 20),
    (74, 1, 21),
    (74, 1, 32),
];

/// Return whether an exact versioned extension action belongs to this product.
pub fn extension_intent_action_enabled(
    family_tag: u8,
    family_version: u8,
    local_action: u8,
) -> bool {
    ENABLED_EXTENSION_ACTIONS.iter().any(|candidate| {
        *candidate == (family_tag, family_version, local_action)
            && extension_intent_action_allocated(family_tag, family_version, local_action)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_identity_is_nonzero_and_membership_is_exact() {
        assert_ne!(PROFILE_ID, [0; 32]);
        assert_eq!(
            solana_sha256_hasher::hash(PROFILE_LABEL.as_bytes()).to_bytes(),
            PROFILE_ID
        );
        assert_eq!(legacy_intent_tag_enabled(1), !GENERAL_V2_IDENTITY_LAB);
        assert_eq!(legacy_intent_tag_enabled(70), !GENERAL_V2_IDENTITY_LAB);
        assert!(!legacy_intent_tag_enabled(0));
        assert!(!legacy_intent_tag_enabled(74));
        assert_eq!(direct_v3_tag_enabled(36), DIRECT_V3);
        assert_eq!(legacy_intent_tag_enabled(47), GENERAL_CLEARING);
        assert_eq!(legacy_intent_tag_enabled(23), SOURCE_V1);
        assert_eq!(legacy_intent_tag_enabled(27), DIRECT_V2);
    }

    #[test]
    fn version_aware_legacy_membership_never_inherits_another_version() {
        for tag in u8::MIN..=u8::MAX {
            for version in u8::MIN..=u8::MAX {
                assert_eq!(
                    legacy_intent_enabled(tag, version),
                    version == clutch_solana_layout::registry::LEGACY_INTENT_VERSION
                        && legacy_intent_tag_enabled(tag),
                    "legacy {tag}/{version}"
                );
                assert_eq!(
                    direct_v3_intent_enabled(tag, version),
                    version == clutch_solana_layout::registry::LEGACY_INTENT_VERSION
                        && direct_v3_tag_enabled(tag),
                    "direct {tag}/{version}"
                );
            }
        }
    }

    #[test]
    fn extension_membership_is_exact_and_capability_bound() {
        for family_tag in u8::MIN..=u8::MAX {
            for family_version in u8::MIN..=3 {
                for local_action in u8::MIN..=u8::MAX {
                    let general = family_tag
                        == clutch_solana_layout::registry::GENERAL_V2_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::GENERAL_V2_FAMILY_VERSION
                        && (clutch_solana_layout::registry::GeneralV2Action::FIRST_TAG
                            ..=clutch_solana_layout::registry::GeneralV2Action::LAST_TAG)
                            .contains(&local_action);
                    let source = family_tag
                        == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION
                        && (clutch_solana_layout::registry::SourceSeriesAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::SourceSeriesAction::LAST_TAG)
                            .contains(&local_action);
                    let expected_allocated = general || source;
                    assert_eq!(
                        extension_intent_action_allocated(family_tag, family_version, local_action,),
                        expected_allocated,
                        "{family_tag}/{family_version}/{local_action}"
                    );
                    let expected_enabled = GENERAL_V2_IDENTITY_LAB
                        && family_tag == 74
                        && family_version == 1
                        && matches!(local_action, 2 | 6 | 7 | 8 | 9 | 10 | 14 | 15);
                    assert_eq!(
                        extension_intent_action_enabled(family_tag, family_version, local_action,),
                        expected_enabled,
                    );
                }
            }
        }
        assert_eq!(
            ENABLED_EXTENSION_ACTIONS.is_empty(),
            !GENERAL_V2_IDENTITY_LAB
        );
    }
}
