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
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab"),
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/full/v10-source-ingest-current-collateral-legacy-general-founder-clearing-withdrawn";
/// Explicit local-only artifact catalog containing successor Product/Series kinds.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    feature = "non-production-product-series-lab",
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-product-series-artifact-catalog-lab/v10-source-ingest-current-collateral-legacy-general-founder-clearing-withdrawn";
/// Direct V3, Source V2, and archive-direct exact-point d1-d3 resolution product.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/direct-v3-source-v2-point/v4-current-collateral-legacy-general-founder-withdrawn";
/// Source V2 and archive-direct exact-point d1-d3 resolution product. The
/// withdrawn General V3 request family is not resident in this successor
/// identity.
#[cfg(feature = "profile-general-source-v2-point")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/general-source-v2-point/v5-current-collateral-legacy-general-founder-placement-withdrawn";
/// Dealer facility binding laboratory. This identity is non-production and
/// contains no legacy intent capability.
#[cfg(all(
    feature = "profile-non-production-dealer-policy-catalog-lab",
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-dealer-self-hosted-liquidity-refund-bind-lapse-lab/v8";
/// Non-production General V2 successor laboratory. All action tuples are
/// fail-closed until the Product admission, counted settlement, and retirement
/// chain is reachable under one exact current account family.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-general-v2-successor-lab/v10-unreachable";

/// Frozen label for the complete unified successor development closure.
pub const SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_LABEL: &str =
    "dragons-clutch/capability-profile/successor-chain-attached-dev/complete-product-source-general-direct-fractional-structured-dealer-failure-release-closure/v1";
/// SHA-256 of [`SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_LABEL`].
pub const SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ID: [u8; 32] = [
    0xf1, 0xd4, 0xc9, 0xbb, 0xb8, 0x9e, 0x89, 0xbf, 0x13, 0xfe, 0x0a, 0x54, 0xae, 0x82, 0x42, 0x20,
    0xdc, 0x0d, 0x11, 0x09, 0xbd, 0xf2, 0x13, 0x16, 0xe2, 0x95, 0x3a, 0xa3, 0x34, 0xaf, 0xd4, 0xca,
];

const SUCCESSOR_CHAIN_ATTACHED_DEV_INFLIGHT_LABEL: &str =
    "dragons-clutch/capability-profile/successor-chain-attached-dev/inflight-family-and-release-closure/v1";
const SUCCESSOR_CHAIN_ATTACHED_DEV_INFLIGHT_ID: [u8; 32] = [
    0xee, 0x2a, 0x5e, 0x1e, 0xde, 0xca, 0xf3, 0x5e, 0x7f, 0xc6, 0x26, 0x7b, 0x33, 0x4d, 0x43, 0xfb,
    0x9d, 0xa6, 0x75, 0x74, 0x0a, 0x84, 0xf9, 0xfa, 0x05, 0x73, 0xbc, 0x3f, 0x2e, 0x65, 0xc0, 0xe6,
];

// This is the only acceptance switch. It may become true only in the commit
// that joins every target handler and checked release row. Both identity and
// admission derive from it, so no partial family can be exposed under either
// the in-flight or the complete identity.
const SUCCESSOR_CHAIN_ATTACHED_DEV_CLOSURE_COMPLETE: bool = false;

/// Identity selected for the single successor development product.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const PROFILE_LABEL: &str = if SUCCESSOR_CHAIN_ATTACHED_DEV_CLOSURE_COMPLETE {
    SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_LABEL
} else {
    SUCCESSOR_CHAIN_ATTACHED_DEV_INFLIGHT_LABEL
};

/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab"),
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_ID: [u8; 32] = [
    0xa8, 0xdc, 0x96, 0x5e, 0x73, 0xb3, 0x31, 0xbc, 0x8d, 0xbb, 0x80, 0x06, 0x91, 0x4b, 0xa0, 0x07,
    0x1d, 0x19, 0x38, 0xf2, 0xde, 0x1b, 0x58, 0xa9, 0xfc, 0x46, 0x7c, 0x61, 0x4e, 0x8f, 0x78, 0x96,
];
/// SHA-256 of the local-only Product/Series artifact catalog profile label.
#[cfg(all(
    feature = "profile-full",
    feature = "non-production-product-series-lab",
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
pub const PROFILE_ID: [u8; 32] = [
    0x21, 0xc6, 0x15, 0x02, 0xef, 0xe8, 0xaf, 0x83, 0x76, 0xf8, 0x32, 0xb8, 0xac, 0xfa, 0x48, 0x4c,
    0xb9, 0x27, 0x9f, 0x59, 0x59, 0x04, 0x52, 0x0a, 0x47, 0x09, 0x7e, 0xd3, 0xb4, 0x9e, 0x11, 0xa9,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_ID: [u8; 32] = [
    0x2f, 0x0d, 0xf0, 0x77, 0x87, 0x1a, 0xe2, 0x89, 0xc0, 0x89, 0x04, 0x58, 0x7b, 0x7f, 0xa2, 0xb6,
    0xe0, 0x05, 0xe0, 0x96, 0x29, 0x2e, 0x44, 0x23, 0x95, 0xc9, 0x96, 0x4d, 0x73, 0xe7, 0x97, 0x7e,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-general-source-v2-point")]
pub const PROFILE_ID: [u8; 32] = [
    0xb7, 0xe2, 0x37, 0x0c, 0xcf, 0x13, 0xa5, 0xa1, 0x78, 0x42, 0x73, 0xce, 0xfb, 0x58, 0xb2, 0x2f,
    0xa4, 0x5e, 0xc6, 0x69, 0xf8, 0x33, 0x23, 0x5d, 0x7f, 0xc3, 0x0a, 0x92, 0x9f, 0x2d, 0xf7, 0xc2,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into the laboratory artifact identity.
#[cfg(all(
    feature = "profile-non-production-dealer-policy-catalog-lab",
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_ID: [u8; 32] = [
    0xb3, 0x67, 0xe8, 0x43, 0x41, 0x38, 0xe6, 0x07, 0x3b, 0x46, 0xf5, 0x39, 0x66, 0x4d, 0x31, 0xa7,
    0x29, 0x6e, 0x88, 0xd6, 0x5b, 0x02, 0x75, 0x72, 0x00, 0x4e, 0x10, 0x6f, 0xcd, 0x54, 0xfd, 0x8b,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const PROFILE_ID: [u8; 32] = [
    0xcf, 0xb6, 0x11, 0x64, 0xcc, 0xf9, 0xe4, 0x60, 0x42, 0x4d, 0x82, 0x1e, 0x0b, 0x9c, 0x5b, 0x63,
    0xac, 0x9a, 0xf5, 0xc6, 0xd0, 0x40, 0x1c, 0x17, 0x74, 0x1b, 0x1a, 0x5d, 0x74, 0x82, 0x6b, 0xfc,
];

/// Identity selected atomically with the successor action closure.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const PROFILE_ID: [u8; 32] = if SUCCESSOR_CHAIN_ATTACHED_DEV_CLOSURE_COMPLETE {
    SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ID
} else {
    SUCCESSOR_CHAIN_ATTACHED_DEV_INFLIGHT_ID
};

/// Whether this artifact is the explicitly non-production identity lab.
pub const GENERAL_V2_IDENTITY_LAB: bool =
    cfg!(feature = "profile-non-production-general-v2-empty-book-identity-lab");

/// Whether this artifact is the explicitly non-production Dealer catalog lab.
pub const DEALER_POLICY_CATALOG_LAB: bool =
    cfg!(feature = "profile-non-production-dealer-policy-catalog-lab");

/// Whether this is the unified successor development product.
pub const SUCCESSOR_CHAIN_ATTACHED_DEV: bool =
    cfg!(feature = "profile-successor-chain-attached-dev");

/// Whether the profile contains legacy Source V1 ingestion and resolution.
pub const SOURCE_V1: bool =
    cfg!(feature = "profile-full") && !DEALER_POLICY_CATALOG_LAB && !SUCCESSOR_CHAIN_ATTACHED_DEV;
/// Whether the profile contains Source V2 ingestion and resolution.
pub const SOURCE_V2: bool = !DEALER_POLICY_CATALOG_LAB
    && !GENERAL_V2_IDENTITY_LAB
    && !SUCCESSOR_CHAIN_ATTACHED_DEV;
/// Whether the profile contains legacy Direct V2 clearing.
pub const DIRECT_V2: bool =
    cfg!(feature = "profile-full") && !DEALER_POLICY_CATALOG_LAB && !SUCCESSOR_CHAIN_ATTACHED_DEV;
/// Whether the profile contains Direct V3 clearing.
pub const DIRECT_V3: bool = cfg!(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point"
)) && !DEALER_POLICY_CATALOG_LAB
    && !SUCCESSOR_CHAIN_ATTACHED_DEV;
/// Whether the profile contains the withdrawn legacy General clearing family.
/// No checked release does; current General successors remain allocated but
/// unreachable until their complete Product-to-retirement chain is admitted.
pub const GENERAL_CLEARING: bool = false;
/// Whether the profile contains occupation and resumable resolution.
pub const OCCUPATION_RESOLUTION: bool =
    cfg!(feature = "profile-full") && !DEALER_POLICY_CATALOG_LAB && !SUCCESSOR_CHAIN_ATTACHED_DEV;

/// Return whether one canonical legacy Intent tag belongs to this product.
///
/// Direct V3 tags `36..=46` use their own strict decoder and are handled by
/// [`direct_v3_tag_enabled`].  Unknown values are false.
pub const fn legacy_intent_tag_enabled(tag: u8) -> bool {
    if DEALER_POLICY_CATALOG_LAB || SUCCESSOR_CHAIN_ATTACHED_DEV {
        return false;
    }
    match tag {
        // Current full-width collateral/value, Realm/Profile, artifact,
        // revenue-record, and Source V2 planes.
        2..=5 | 10..=11 | 15..=21 | 68 | 70..=73 => !GENERAL_V2_IDENTITY_LAB,
        // The old feed buffer, direct-page settlement and Source V1 families.
        6 | 22..=31 => cfg!(feature = "profile-full"),
        // Resumable occupation work.
        32..=35 => cfg!(feature = "profile-full"),
        // Withdrawn General construction, clearing, settlement, and close
        // routes are absent from every checked release. Tag 1 was the legacy
        // seven-account Market founder. Tags 12/13 named constructors whose
        // sole live handler already refused in favor of typed artifact sealing.
        1 | 8..=9 | 12..=13 | 47..=67 | 69 => false,
        // The shared PlaceOrder wire is current only for exact DirectEpochV4.
        // The account-width-selected General fallback is withdrawn.
        7 => DIRECT_V3 && !GENERAL_V2_IDENTITY_LAB,
        // This shared wire coordinate remains admitted only for the exact
        // Direct V4 page-zero constructor; the General Epoch fallback is gone.
        14 => DIRECT_V3 && !GENERAL_V2_IDENTITY_LAB,
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
/// and facility, StructuredClaim, SourcePlane V3, recurring-Series, and
/// Recovery actions have registered local actions; every exact tuple remains
/// separately disabled until its handler is admitted. Frozen payload and
/// account codecs do not activate any runtime tuple.
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

/// Number of successor actions in the complete unified closure.
pub const SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT: usize = 117;

const fn complete_successor_actions(
) -> [(u8, u8, u8); SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT] {
    let mut rows = [(0, 0, 0); SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT];
    let mut cursor = 0usize;
    let mut action = 1u8;
    while action <= 42 {
        if action != 35 {
            rows[cursor] = (74, 1, action);
            cursor += 1;
        }
        action += 1;
    }
    let structured = [1u8, 3, 5, 6, 7, 8];
    let mut index = 0usize;
    while index < structured.len() {
        rows[cursor] = (75, 1, structured[index]);
        cursor += 1;
        index += 1;
    }
    action = 1;
    while action <= 25 {
        rows[cursor] = (76, 1, action);
        cursor += 1;
        action += 1;
    }
    action = 1;
    while action <= 18 {
        rows[cursor] = (77, 2, action);
        cursor += 1;
        action += 1;
    }
    action = 10;
    while action <= 13 {
        rows[cursor] = (78, 1, action);
        cursor += 1;
        action += 1;
    }
    action = 1;
    while action <= 10 {
        rows[cursor] = (79, 1, action);
        cursor += 1;
        action += 1;
    }
    action = 1;
    while action <= 13 {
        rows[cursor] = (80, 1, action);
        cursor += 1;
        action += 1;
    }
    assert!(cursor == SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT);
    rows
}

/// Exact canonical tuple set admitted only by the complete unified profile.
pub const SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTIONS:
    [(u8, u8, u8); SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT] =
    complete_successor_actions();

/// Exact extension actions executable by this product.
///
/// Full profiles execute artifact-authenticated Source release registration
/// plus release-bound atomic SourceHead/OpenRawPage creation and receiver-
/// authenticated parser ingestion. Actions 5 through 12 remain independently
/// disabled.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    not(any(
        feature = "profile-non-production-dealer-policy-catalog-lab",
        feature = "profile-non-production-general-v2-empty-book-identity-lab"
    ))
))]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] =
    &[(77, 2, 1), (77, 2, 2), (77, 2, 3), (77, 2, 4)];

/// In-flight successor checkpoint. The final identity is not compiled and no
/// tuple is reachable until the complete cross-family/release closure lands.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] =
    if SUCCESSOR_CHAIN_ATTACHED_DEV_CLOSURE_COMPLETE {
        &SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTIONS
    } else {
        &[]
    };

/// Narrow non-laboratory profiles have not yet admitted Source execution.
#[cfg(all(
    not(feature = "profile-full"),
    not(any(
        feature = "profile-non-production-dealer-policy-catalog-lab",
        feature = "profile-non-production-general-v2-empty-book-identity-lab"
    ))
))]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[];

/// The laboratory enables typed Dealer catalog publication plus exact facility
/// initialization, bounded LP funding, activation/recovery/refund, and bounded
/// Epoch binding/lapse.
#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[
    (76, 1, 1),
    (76, 1, 2),
    (76, 1, 3),
    (76, 1, 4),
    (76, 1, 5),
    (76, 1, 6),
    (76, 1, 7),
    (76, 1, 8),
    (76, 1, 9),
    (76, 1, 10),
    (76, 1, 11),
    (76, 1, 12),
    (76, 1, 13),
];

/// The General successor laboratory has no executable action tuple until its
/// full current-state producer and retirement closure is complete.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[];

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
        assert!(!legacy_intent_tag_enabled(1));
        assert_eq!(
            legacy_intent_tag_enabled(70),
            !DEALER_POLICY_CATALOG_LAB
                && !GENERAL_V2_IDENTITY_LAB
                && !SUCCESSOR_CHAIN_ATTACHED_DEV
        );
        assert!(!legacy_intent_tag_enabled(0));
        assert!(!legacy_intent_tag_enabled(74));
        assert_eq!(direct_v3_tag_enabled(36), DIRECT_V3);
        assert_eq!(legacy_intent_tag_enabled(47), GENERAL_CLEARING);
        for tag in [1, 8, 9, 12, 13, 69] {
            assert!(!legacy_intent_tag_enabled(tag), "withdrawn General tag {tag}");
        }
        for tag in [2, 3, 4, 5, 15, 16, 17] {
            assert_eq!(
                legacy_intent_tag_enabled(tag),
                !DEALER_POLICY_CATALOG_LAB
                    && !GENERAL_V2_IDENTITY_LAB
                    && !SUCCESSOR_CHAIN_ATTACHED_DEV,
                "current full-width Collateral tag {tag}",
            );
        }
        for tag in 47..=67 {
            assert!(!legacy_intent_tag_enabled(tag), "withdrawn General tag {tag}");
        }
        assert_eq!(legacy_intent_tag_enabled(14), DIRECT_V3 && !GENERAL_V2_IDENTITY_LAB);
        assert_eq!(legacy_intent_tag_enabled(7), DIRECT_V3 && !GENERAL_V2_IDENTITY_LAB);
        assert_eq!(legacy_intent_tag_enabled(23), SOURCE_V1);
        assert_eq!(legacy_intent_tag_enabled(27), DIRECT_V2);
        assert_eq!(
            solana_sha256_hasher::hash(SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_LABEL.as_bytes())
                .to_bytes(),
            SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ID
        );
        assert_eq!(
            SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTIONS.len(),
            SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTION_COUNT
        );
        for (family_tag, family_version, local_action) in
            SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ACTIONS
        {
            assert!(extension_intent_action_allocated(
                family_tag,
                family_version,
                local_action
            ));
        }
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
                    let dealer = family_tag == clutch_solana_layout::registry::DEALER_FAMILY_TAG
                        && family_version == clutch_solana_layout::registry::DEALER_FAMILY_VERSION
                        && ((clutch_solana_layout::registry::DealerPolicyAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::DealerPolicyAction::LAST_TAG)
                            .contains(&local_action)
                            || (clutch_solana_layout::registry::DealerFacilityAction::FIRST_TAG
                                ..=clutch_solana_layout::registry::DealerFacilityAction::LAST_TAG)
                                .contains(&local_action));
                    let source_or_series = family_tag
                        == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION
                        && ((clutch_solana_layout::registry::SourceSeriesAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::SourceSeriesAction::LAST_TAG)
                            .contains(&local_action)
                            || (clutch_solana_layout::registry::RecurringSeriesAction::FIRST_TAG
                                ..=clutch_solana_layout::registry::RecurringSeriesAction::LAST_TAG)
                                .contains(&local_action));
                    let structured = family_tag
                        == clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_VERSION
                        && (clutch_solana_layout::registry::StructuredClaimAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::StructuredClaimAction::LAST_TAG)
                            .contains(&local_action);
                    let recovery = family_tag
                        == clutch_solana_layout::registry::RECOVERY_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::RECOVERY_FAMILY_VERSION
                        && (clutch_solana_layout::registry::RecoveryAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::RecoveryAction::LAST_TAG)
                            .contains(&local_action);
                    let fractional = family_tag
                        == clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::FRACTIONAL_REDEMPTION_FAMILY_VERSION
                        && (clutch_solana_layout::registry::FractionalRedemptionAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::FractionalRedemptionAction::LAST_TAG)
                            .contains(&local_action);
                    let direct = family_tag
                        == clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_TAG
                        && family_version
                            == clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_VERSION
                        && (clutch_solana_layout::registry::DirectMarketAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::DirectMarketAction::LAST_TAG)
                            .contains(&local_action);
                    let expected_allocated = general
                        || dealer
                        || structured
                        || source_or_series
                        || recovery
                        || fractional
                        || direct;
                    assert_eq!(
                        extension_intent_action_allocated(family_tag, family_version, local_action,),
                        expected_allocated,
                        "{family_tag}/{family_version}/{local_action}"
                    );
                    let dealer_enabled = DEALER_POLICY_CATALOG_LAB
                        && family_tag == clutch_solana_layout::registry::DEALER_FAMILY_TAG
                        && family_version == clutch_solana_layout::registry::DEALER_FAMILY_VERSION
                        && ((clutch_solana_layout::registry::DealerPolicyAction::FIRST_TAG
                            ..=clutch_solana_layout::registry::DealerPolicyAction::LAST_TAG)
                            .contains(&local_action)
                            || matches!(local_action, 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12));
                    let general_enabled = false;
                    let source_runtime_enabled = cfg!(feature = "profile-full")
                        && !SUCCESSOR_CHAIN_ATTACHED_DEV
                        && !DEALER_POLICY_CATALOG_LAB
                        && !GENERAL_V2_IDENTITY_LAB
                        && family_tag == 77
                        && family_version == 2
                        && matches!(local_action, 1 | 2 | 3 | 4);
                    let expected_enabled = dealer_enabled
                        || general_enabled
                        || source_runtime_enabled;
                    assert_eq!(
                        extension_intent_action_enabled(family_tag, family_version, local_action,),
                        expected_enabled,
                    );
                }
            }
        }
        assert_eq!(
            ENABLED_EXTENSION_ACTIONS.is_empty(),
            !(DEALER_POLICY_CATALOG_LAB
                || (cfg!(feature = "profile-full")
                    && !GENERAL_V2_IDENTITY_LAB
                    && !SUCCESSOR_CHAIN_ATTACHED_DEV))
        );
    }

    #[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
    #[test]
    fn general_successor_lab_keeps_every_action_unreachable() {
        for action in clutch_solana_layout::registry::GeneralV2Action::FIRST_TAG
            ..=clutch_solana_layout::registry::GeneralV2Action::LAST_TAG
        {
            assert!(!extension_intent_action_enabled(74, 1, action));
        }
    }
}
