//! Compile-time identity and wire admission for one deployable product.
//!
//! Capability profiles are protocol identities, not build-size aliases.  A
//! profile fixes which canonical instruction families this ELF can decode and
//! execute.  Disabled canonical tags refuse before any account is read.  The
//! label and SHA-256 identity below are copied into the artifact manifest by
//! `scripts/measure_capability_profiles.py`; changing either the membership or
//! the label therefore creates a different release identity.

/// Full research/runtime surface retained by the historical default build.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/full/v1";
/// Direct V3, Source V2, and archive-direct exact-point d1-d3 resolution product.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/direct-v3-source-v2-point/v1";
/// General clearing, Source V2, and archive-direct exact-point d1-d3 resolution product.
#[cfg(feature = "profile-general-source-v2-point")]
pub const PROFILE_LABEL: &str = "dragons-clutch/capability-profile/general-source-v2-point/v1";
/// Dealer-policy catalog laboratory. This identity is non-production and
/// contains no legacy intent capability.
#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-dealer-policy-catalog-lab/v1";

/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
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
/// SHA-256 of [`PROFILE_LABEL`], frozen into the laboratory artifact identity.
#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
pub const PROFILE_ID: [u8; 32] = [
    0xcb, 0x80, 0x25, 0xae, 0x72, 0xa0, 0xbc, 0x86, 0x66, 0xd9, 0x31, 0x9b, 0xe6, 0xfb, 0x67, 0x82,
    0x82, 0xd5, 0xa9, 0x12, 0x96, 0x9e, 0x6a, 0x10, 0xdf, 0xcd, 0xdd, 0x84, 0x06, 0x23, 0x7d, 0x72,
];

/// Whether the profile contains legacy Source V1 ingestion and resolution.
pub const SOURCE_V1: bool = cfg!(feature = "profile-full")
    && !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");
/// Whether the profile contains Source V2 ingestion and resolution.
pub const SOURCE_V2: bool = !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");
/// Whether the profile contains legacy Direct V2 clearing.
pub const DIRECT_V2: bool = cfg!(feature = "profile-full")
    && !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");
/// Whether the profile contains Direct V3 clearing.
pub const DIRECT_V3: bool =
    cfg!(any(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    )) && !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");
/// Whether the profile contains general clearing.
pub const GENERAL_CLEARING: bool =
    cfg!(any(
        feature = "profile-full",
        feature = "profile-general-source-v2-point"
    )) && !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");
/// Whether the profile contains occupation and resumable resolution.
pub const OCCUPATION_RESOLUTION: bool = cfg!(feature = "profile-full")
    && !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");

/// Return whether one canonical legacy Intent tag belongs to this product.
///
/// Direct V3 tags `36..=46` use their own strict decoder and are handled by
/// [`direct_v3_tag_enabled`].  Unknown values are false.
pub const fn legacy_intent_tag_enabled(tag: u8) -> bool {
    if cfg!(feature = "profile-non-production-dealer-policy-catalog-lab") {
        return false;
    }
    match tag {
        // Common construction, custody, trading, exit and artifact plane.
        1..=5 | 7 | 10..=21 | 68 | 70..=73 => true,
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
/// Allocation does not imply execution capability. General V2, Dealer policy
/// transport, and the bounded SourcePlane V3 portion of SourceSeries have
/// registered local actions; each exact tuple has an independent activation gate.
pub const fn extension_intent_action_allocated(
    family_tag: u8,
    family_version: u8,
    local_action: u8,
) -> bool {
    clutch_solana_layout::registry::decode_extension_action(
        family_tag,
        family_version,
        local_action,
    )
    .is_ok()
}

/// Exact extension actions executable by this product.
///
/// In every ordinary profile the empty slice is the mechanical activation
/// gate. The separately identified laboratory profile below enables only its
/// four policy-transport actions.
#[cfg(not(feature = "profile-non-production-dealer-policy-catalog-lab"))]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[];

/// The laboratory enables only the bounded immutable policy-catalog transport.
#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] =
    &[(76, 1, 1), (76, 1, 2), (76, 1, 3), (76, 1, 4)];

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
        assert_eq!(
            legacy_intent_tag_enabled(1),
            !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab")
        );
        assert_eq!(
            legacy_intent_tag_enabled(70),
            !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab")
        );
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
    fn extension_membership_and_activation_are_exact() {
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
                    let dealer = family_tag
                        == clutch_solana_layout::registry::DEALER_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::DEALER_FAMILY_VERSION
                        && (clutch_solana_layout::registry::DealerPolicyAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::DealerPolicyAction::LAST_TAG)
                            .contains(&local_action);
                    let source = family_tag
                        == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION
                        && (clutch_solana_layout::registry::SourceSeriesAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::SourceSeriesAction::LAST_TAG)
                            .contains(&local_action);
                    let expected_allocated = general || dealer || source;
                    assert_eq!(
                        extension_intent_action_allocated(family_tag, family_version, local_action,),
                        expected_allocated,
                        "{family_tag}/{family_version}/{local_action}"
                    );
                    let expected_enabled =
                        cfg!(feature = "profile-non-production-dealer-policy-catalog-lab")
                            && family_tag == clutch_solana_layout::registry::DEALER_FAMILY_TAG
                            && family_version
                                == clutch_solana_layout::registry::DEALER_FAMILY_VERSION
                            && (clutch_solana_layout::registry::DealerPolicyAction::FIRST_TAG
                                ..=clutch_solana_layout::registry::DealerPolicyAction::LAST_TAG)
                                .contains(&local_action);
                    assert_eq!(
                        extension_intent_action_enabled(family_tag, family_version, local_action,),
                        expected_enabled,
                    );
                }
            }
        }
        assert_eq!(
            ENABLED_EXTENSION_ACTIONS.is_empty(),
            !cfg!(feature = "profile-non-production-dealer-policy-catalog-lab")
        );
    }
}
