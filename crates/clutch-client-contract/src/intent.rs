//! Typed views of untrusted intent coordinates.
//!
//! Allocation remains owned by [`clutch_solana_layout::registry`]. This module
//! stores its returned type rather than copying any tag or version constant.

use clutch_solana_layout::registry::{
    classify_intent, decode_extension_action, ExtensionAction, IntentAllocation,
};

/// One observed intent tag/version coordinate classified by the central
/// registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedIntentCoordinate {
    tag: u8,
    version: u8,
    allocation: IntentAllocation,
}

impl ObservedIntentCoordinate {
    /// Classify untrusted coordinates through the authoritative registry.
    ///
    /// # Errors
    ///
    /// Refuses an exact tag/version pair that has no central allocation.
    pub const fn classify(tag: u8, version: u8) -> Result<Self, IntentLinkRefusal> {
        match classify_intent(tag, version) {
            Some(allocation) => Ok(Self {
                tag,
                version,
                allocation,
            }),
            None => Err(IntentLinkRefusal::UnallocatedCoordinate),
        }
    }

    /// Observed tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self.tag
    }

    /// Observed wire version.
    #[must_use]
    pub const fn version(self) -> u8 {
        self.version
    }

    /// Authoritative allocation classification.
    #[must_use]
    pub const fn allocation(self) -> IntentAllocation {
        self.allocation
    }
}

/// Refusal to link untrusted intent coordinates to the central registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentLinkRefusal {
    /// No allocation owns the exact tag/version pair.
    UnallocatedCoordinate,
}

/// One observed successor envelope prefix classified by the central registry.
///
/// This proves allocation only. It does not say the action is enabled by a
/// release, supported by a client, or executable by a deployed program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedExtensionAction {
    family_tag: u8,
    family_version: u8,
    local_action: u8,
    action: ExtensionAction,
}

impl ObservedExtensionAction {
    /// Classify an untrusted successor prefix through the authoritative
    /// registry.
    ///
    /// # Errors
    ///
    /// Refuses an unknown family/version pair or an unallocated local action.
    pub const fn classify(
        family_tag: u8,
        family_version: u8,
        local_action: u8,
    ) -> Result<Self, IntentLinkRefusal> {
        match decode_extension_action(family_tag, family_version, local_action) {
            Ok(action) => Ok(Self {
                family_tag,
                family_version,
                local_action,
                action,
            }),
            Err(_) => Err(IntentLinkRefusal::UnallocatedCoordinate),
        }
    }

    /// Observed successor family tag.
    #[must_use]
    pub const fn family_tag(self) -> u8 {
        self.family_tag
    }

    /// Observed successor family version.
    #[must_use]
    pub const fn family_version(self) -> u8 {
        self.family_version
    }

    /// Observed family-local action tag.
    #[must_use]
    pub const fn local_action(self) -> u8 {
        self.local_action
    }

    /// Authoritative allocation classification.
    #[must_use]
    pub const fn action(self) -> ExtensionAction {
        self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::registry::{
        ExtensionFamily, GeneralV2Action, GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION,
        SOURCE_ARCHIVE_V2_ACCOUNT_TAG,
    };

    #[test]
    fn successor_link_uses_the_registry_type() {
        let linked =
            ObservedIntentCoordinate::classify(GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION)
                .expect("General V2 is reserved in the central registry");
        assert_eq!(
            linked.allocation(),
            IntentAllocation::Extension(ExtensionFamily::GeneralV2)
        );
    }

    #[test]
    fn account_tag_spelling_cannot_be_misread_as_an_intent() {
        assert_eq!(
            ObservedIntentCoordinate::classify(SOURCE_ARCHIVE_V2_ACCOUNT_TAG, 1),
            Err(IntentLinkRefusal::UnallocatedCoordinate)
        );
    }

    #[test]
    fn wrong_versions_and_unallocated_tags_refuse() {
        assert_eq!(
            ObservedIntentCoordinate::classify(GENERAL_V2_FAMILY_TAG, 2),
            Err(IntentLinkRefusal::UnallocatedCoordinate)
        );
        assert_eq!(
            ObservedIntentCoordinate::classify(0, 3),
            Err(IntentLinkRefusal::UnallocatedCoordinate)
        );
    }

    #[test]
    fn action_link_is_allocation_not_runtime_enablement() {
        let linked = ObservedExtensionAction::classify(
            GENERAL_V2_FAMILY_TAG,
            GENERAL_V2_FAMILY_VERSION,
            GeneralV2Action::EntitleSlice.tag(),
        )
        .expect("the action is allocated, though every extension remains disabled");
        assert_eq!(
            linked.action(),
            ExtensionAction::GeneralV2(GeneralV2Action::EntitleSlice)
        );
        assert_eq!(
            ObservedExtensionAction::classify(GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION, 0,),
            Err(IntentLinkRefusal::UnallocatedCoordinate)
        );
    }
}
