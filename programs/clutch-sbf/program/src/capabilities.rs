//! Compile-time identity and wire admission for one deployable product.
//!
//! Capability profiles are protocol identities, not build-size aliases.
//! Disabled canonical tags refuse before any account is read. The successor
//! profile has one frozen identity; its exact callable tuple set is compiled
//! below, while release readiness and observed external-program manifests stay
//! outside runtime dispatch. The label and SHA-256 identity are copied into the
//! artifact manifest by `scripts/measure_capability_profiles.py`.

/// Full research/runtime surface retained by the historical default build.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab"),
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/full/v10-source-ingest-current-collateral-legacy-general-founder-clearing-withdrawn-legacy-direct-retired-direct-80-staged-disabled";
/// Explicit local-only artifact catalog containing successor Product/Series kinds.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    feature = "non-production-product-series-lab",
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/non-production-product-series-artifact-catalog-lab/v10-source-ingest-current-collateral-legacy-general-founder-clearing-withdrawn-legacy-direct-retired-direct-80-staged-disabled";
/// Source V2 and archive exact-point d1-d3 resolution product. The retained
/// feature spelling is build-input compatibility only; legacy Direct and
/// General founder routes are withdrawn.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_LABEL: &str =
    "dragons-clutch/capability-profile/source-v2-point/v4-current-collateral-legacy-general-founder-withdrawn-legacy-direct-retired";
/// Source V2 and archive-direct exact-point d1-d3 resolution product. The
/// withdrawn General V3 request family is not resident in this identity.
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

/// Identity selected for the single successor development product.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const PROFILE_LABEL: &str = SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_LABEL;

/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(all(
    feature = "profile-full",
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab"),
    not(feature = "non-production-product-series-lab")
))]
pub const PROFILE_ID: [u8; 32] = [
    0x4c, 0x6f, 0x21, 0xdf, 0x7b, 0x06, 0x6e, 0x26, 0xa9, 0x37, 0xe5, 0xd4, 0x13, 0x92, 0xe0, 0x75,
    0xd9, 0x3b, 0x96, 0xe2, 0x50, 0xe3, 0x87, 0xfa, 0x5c, 0xe7, 0x7d, 0x18, 0xfe, 0x92, 0xd2, 0x8b,
];
/// SHA-256 of the local-only Product/Series artifact catalog profile label.
#[cfg(all(
    feature = "profile-full",
    feature = "non-production-product-series-lab",
    not(feature = "profile-successor-chain-attached-dev"),
    not(feature = "profile-non-production-dealer-policy-catalog-lab")
))]
pub const PROFILE_ID: [u8; 32] = [
    0xf4, 0xef, 0x38, 0xb5, 0x4d, 0xfd, 0xaf, 0xbf, 0x64, 0x51, 0x03, 0x5b, 0xa2, 0x95, 0xc0, 0x45,
    0xf3, 0xbf, 0x7c, 0xe8, 0x4b, 0x3d, 0xde, 0xd1, 0x1d, 0x28, 0x3b, 0x36, 0xeb, 0x5c, 0x17, 0x2e,
];
/// SHA-256 of [`PROFILE_LABEL`], frozen into release metadata.
#[cfg(feature = "profile-direct-v3-source-v2-point")]
pub const PROFILE_ID: [u8; 32] = [
    0xd6, 0xec, 0x0c, 0xf5, 0xfc, 0xe9, 0xf6, 0x02, 0xb7, 0x9b, 0xde, 0x81, 0xd8, 0x97, 0x0f, 0x69,
    0xb5, 0x88, 0xc3, 0x5e, 0x66, 0x07, 0x9a, 0xed, 0x52, 0xb2, 0x09, 0x69, 0xb4, 0x0f, 0x11, 0x6f,
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

/// Frozen identity of the successor development product.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const PROFILE_ID: [u8; 32] = SUCCESSOR_CHAIN_ATTACHED_DEV_COMPLETE_ID;

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
        // The old feed buffer and Source V1 family. SubmitDirectPage (22) and
        // Direct V2 actions (27..=31) remain allocated for hostile decoding,
        // but no current artifact can execute them.
        6 | 23..=26 => cfg!(feature = "profile-full"),
        // Resumable occupation work.
        32..=35 => cfg!(feature = "profile-full"),
        // Withdrawn General construction, clearing, settlement, and close
        // routes are absent from every checked release. Tag 1 was the legacy
        // seven-account Market founder. Tags 12/13 named constructors whose
        // sole live handler already refused in favor of typed artifact sealing.
        1 | 8..=9 | 12..=13 | 47..=67 | 69 => false,
        // Shared PlaceOrder was retained only for retired DirectEpochV4; the
        // account-width-selected General fallback is also withdrawn.
        7 => false,
        // This shared wire coordinate belonged to the retired Direct V4
        // page-zero constructor; the General Epoch fallback is also gone.
        14 => false,
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
    let _ = tag;
    false
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

#[cfg(feature = "profile-successor-chain-attached-dev")]
const _: () = assert!(
    crate::instructions::structured_custody::STRUCTURED_BASE_HANDLER_ACTION_MASK_V1
        == clutch_structured_claim_adapter::STRUCTURED_JOINED_RELEASE_ACTION_MASK_V1
);

/// Exact extension actions executable by the successor product at this
/// dependency checkpoint.
///
/// Failure actions 10 through 13 have complete current session creation,
/// advance, resolution, and archive handlers. Other target tuples remain
/// absent until their current authority tranche is integrated. In particular,
/// Source actions 1 through 12 stay gated until Product consumes the private
/// whole-lifecycle funding-custody retirement transition.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub const ENABLED_EXTENSION_ACTIONS: &[(u8, u8, u8)] = &[
    (78, 1, 10),
    (78, 1, 11),
    (78, 1, 12),
];

/// Exact extension actions executable by other products.
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

/// Return whether one exact current Dealer terminal-cut payload is callable.
///
/// Dealer action 25 multiplexes historical retirement steps and the two
/// current facility-credit terminal cuts under one allocated action byte. The
/// whole action therefore remains absent from [`ENABLED_EXTENSION_ACTIONS`]:
/// admitting that tuple would also admit the unrelated target values. The
/// dispatcher hostile-decodes the payload before account access and calls this
/// narrower capability for only the complete current targets.
pub const fn dealer_terminal_retire_target_enabled(retire_target: u8) -> bool {
    SUCCESSOR_CHAIN_ATTACHED_DEV
        && (retire_target
            == crate::instructions::dealer_runtime::DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1
            || retire_target
                == crate::instructions::dealer_runtime::DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1)
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
        assert!(!direct_v3_tag_enabled(36));
        assert!(!legacy_intent_tag_enabled(47));
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
        assert!(!legacy_intent_tag_enabled(14));
        assert!(!legacy_intent_tag_enabled(7));
        assert_eq!(legacy_intent_tag_enabled(23), SOURCE_V1);
        assert!(!legacy_intent_tag_enabled(22));
        assert!(!legacy_intent_tag_enabled(27));
        assert!(!legacy_intent_tag_enabled(31));
        assert!(!direct_v3_tag_enabled(46));
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
    fn dealer_terminal_cut_capability_is_exactly_payload_scoped() {
        use crate::instructions::dealer_runtime::{
            DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1,
            DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1,
        };

        assert_eq!(
            dealer_terminal_retire_target_enabled(
                DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1,
            ),
            SUCCESSOR_CHAIN_ATTACHED_DEV,
        );
        assert_eq!(
            dealer_terminal_retire_target_enabled(
                DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1,
            ),
            SUCCESSOR_CHAIN_ATTACHED_DEV,
        );
        for target in 0u8..=7 {
            assert!(!dealer_terminal_retire_target_enabled(target));
        }
        assert!(!dealer_terminal_retire_target_enabled(10));
        assert!(!extension_intent_action_enabled(
            clutch_solana_layout::registry::DEALER_FAMILY_TAG,
            clutch_solana_layout::registry::DEALER_FAMILY_VERSION,
            clutch_solana_layout::registry::DealerFacilityAction::Retire.tag(),
        ));
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
                    let successor_enabled = SUCCESSOR_CHAIN_ATTACHED_DEV
                        && family_tag == 78
                        && family_version == 1
                        && matches!(local_action, 10..=13);
                    let expected_enabled = dealer_enabled
                        || general_enabled
                        || source_runtime_enabled
                        || successor_enabled;
                    assert_eq!(
                        extension_intent_action_enabled(family_tag, family_version, local_action,),
                        expected_enabled,
                    );
                }
            }
        }
        assert_eq!(
            ENABLED_EXTENSION_ACTIONS.is_empty(),
            !(SUCCESSOR_CHAIN_ATTACHED_DEV
                || DEALER_POLICY_CATALOG_LAB
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

    #[test]
    fn direct_successor_is_allocated_but_independently_disabled() {
        let mut action = clutch_solana_layout::registry::DirectMarketAction::FIRST_TAG;
        while action <= clutch_solana_layout::registry::DirectMarketAction::LAST_TAG {
            assert!(extension_intent_action_allocated(
                clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_TAG,
                clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_VERSION,
                action,
            ));
            assert!(!extension_intent_action_enabled(
                clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_TAG,
                clutch_solana_layout::registry::DIRECT_MARKET_FAMILY_VERSION,
                action,
            ));
            action += 1;
        }
    }
}
