#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Descriptor-specialized CapabilityProgram V4 artifacts and unsigned Hot
//! operator construction for Rational lifecycle actions.
//!
//! Fixed-cardinality actions are selected only through CapabilityV4,
//! LifecycleV5, and Profile13. Complete receipt retirement is selected only
//! through the compact CapabilityV4/LifecycleV5/Profile13 path, which derives
//! ordered nonzero support from the authenticated immutable descriptor. The
//! superseded V3 artifact constructors remain private implementation helpers;
//! there is no public unprofiled fallback. Claims remains the sole physical
//! mutation and typed receipt authority.

mod account_profile;
mod artifacts;
#[cfg(test)]
mod bundle;
mod compact_artifacts_v4;
mod compact_operator_v4;
mod effect;
mod operator;
mod selected_bundle_v5;
mod selected_bundle_v6;
mod selected_operator_v5;
mod selected_operator_v6;
mod selected_profile_v5;
mod selected_set_v5;

pub use compact_artifacts_v4::{
    RATIONAL_LIFECYCLE_COMPACT_DESCRIPTOR_BYTES_V4, RATIONAL_LIFECYCLE_COMPACT_STRATEGY_BYTES_V4,
    RationalLifecycleCompactArtifactInputV4, RationalLifecycleCompactArtifactsV4,
    RationalLifecycleCompactBundleInputV4, RationalLifecycleCompactBundleV4,
    build_rational_lifecycle_compact_bundle_v4, encode_rational_lifecycle_compact_artifacts_v4,
    validate_rational_lifecycle_compact_bundle_for_authenticated_selection_v4,
    validate_rational_lifecycle_compact_bundle_v4,
};
pub use compact_operator_v4::{
    RationalLifecycleCompactSelectionV4, RationalLifecycleVacancyAccountsV4,
    build_rational_lifecycle_compact_hot_instruction_v4,
};
pub use effect::{
    RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3, lifecycle_claims_account_count_v3,
    lifecycle_logical_account_count_v3,
};
pub use operator::{
    CheckedRationalLifecycleHotOuterV3, RationalLifecycleHotInstructionV3,
    RationalLifecycleHotStateV3,
};
pub use selected_bundle_v5::{
    RATIONAL_LIFECYCLE_SELECTED_DESCRIPTOR_BYTES_V5, RATIONAL_LIFECYCLE_SELECTED_STRATEGY_BYTES_V5,
    RationalLifecycleSelectedBundleInputV5, RationalLifecycleSelectedBundleV5,
    build_rational_lifecycle_selected_bundle_v5,
    validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5,
    validate_rational_lifecycle_selected_bundle_v5,
};
pub use selected_bundle_v6::{
    RationalLifecycleSelectedBundleInputV6, RationalLifecycleSelectedBundleV6,
    build_rational_lifecycle_selected_bundle_v6, validate_rational_lifecycle_selected_bundle_v6,
};
pub use selected_operator_v5::{
    RationalLifecycleSelectedSelectionV5, build_rational_lifecycle_selected_hot_instruction_v5,
};
pub use selected_operator_v6::{
    RationalLifecycleSelectedSelectionV6, build_rational_lifecycle_selected_hot_instruction_v6,
};
pub use selected_profile_v5::{
    RationalLifecycleSelectedAccountProfileInputV5,
    encode_rational_lifecycle_selected_account_profile_v5,
    encode_rational_lifecycle_selected_account_profile_v6,
};
pub use selected_set_v5::{
    RationalLifecycleProgramSetInputV5, RationalLifecycleProgramSetV5,
    build_rational_lifecycle_program_set_v5, validate_rational_lifecycle_program_set_v5,
};

use dclutch_rational_representation_v2_lifecycle_contract::LifecycleActionV2;

/// Stable lifecycle Hot artifact/operator refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Lifecycle action and descriptor support selected another exact geometry.
    ActionGeometry,
    /// Arithmetic or integer narrowing could not represent an exact artifact.
    InvalidLength,
    /// Typed RequestProfile encoding or hostile decoding refused.
    RequestProfile(dclutch_request_profile_contract::Error),
    /// Compact repeated-row RequestProfile V4 encoding or hostile decoding refused.
    RequestProfileV4(dclutch_request_profile_contract::v4::Error),
    /// Typed TransitionVM encoding or hostile decoding refused.
    Transition(dclutch_transition_vm::v3::Error),
    /// Typed AccountProfile encoding or hostile decoding refused.
    AccountProfile(dclutch_account_profile_contract::v2::Error),
    /// ProductBasisV3 or exact logical account observations differed.
    AccountObservation,
    /// Typed EffectProgram encoding or hostile decoding refused.
    Effect(dclutch_effect_kernel::v3::Error),
    /// Typed EffectProgram successor encoding or hostile decoding refused.
    EffectV4(dclutch_effect_kernel::v4::ErrorV4),
    /// A content-addressed semantic coordinate was zero.
    ContentIdentity,
    /// Interpreted execution-strategy construction or join refused.
    Strategy(dclutch_execution_strategy_contract::v2::Error),
    /// CapabilityProgram construction or hostile decoding refused.
    Descriptor(dclutch_capability_program_contract::Error),
    /// Successor lifecycle artifact decoding or AccountProfile join refused.
    LifecycleArtifact(dclutch_account_profile_contract::lifecycle_v3::Error),
    /// Canonical Token behavior selection failed hostile decoding.
    TokenBehavior(dclutch_token_svm::Error),
    /// Finalized bundle parts did not share one exact geometry.
    ArtifactGeometry,
    /// Exact family/Claims child specialization or receipt contract refused.
    Lifecycle(dclutch_rational_representation_v2_lifecycle_contract::Error),
    /// Checked release, physical account frame, or unsigned instruction refused.
    Operator,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;

fn validate_action_geometry(action: LifecycleActionV2, coordinate_count: u32) -> Result<usize> {
    let coordinates = usize::try_from(coordinate_count).map_err(|_| Error::InvalidLength)?;
    let accepted = match action {
        LifecycleActionV2::ActivateReceipt => coordinates == 0,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            coordinates == 1
        }
        // Complete support must be descriptor-derived by compact V4. Refuse
        // every caller-carried V3 retirement row set, including K=1.
        LifecycleActionV2::RetireReceipt => false,
    };
    if accepted {
        Ok(coordinates)
    } else {
        Err(Error::ActionGeometry)
    }
}
