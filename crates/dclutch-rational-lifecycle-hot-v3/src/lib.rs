#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Descriptor-specialized CapabilityProgram V3 artifacts and unsigned Hot
//! operator construction for the four canonical Rational lifecycle actions.
//!
//! Product outcome width and descriptor nonzero support are distinct. Each
//! finalized bundle fixes one action and exact lifecycle row count in common
//! registers while retaining the independently authenticated Product width.
//! Claims remains the sole physical mutation and typed receipt authority.

mod account_profile;
mod artifacts;
mod bundle;
mod compact_artifacts_v4;
mod compact_operator_v4;
mod effect;
mod operator;

pub use account_profile::{
    RationalLifecycleAccountProfileInputV3, encode_rational_lifecycle_account_profile_v3,
};
pub use artifacts::{
    encode_rational_lifecycle_request_profile_v3, encode_rational_lifecycle_transition_v3,
};
pub use bundle::{
    RationalLifecycleHotBundleInputV3, RationalLifecycleHotBundleV3,
    build_rational_lifecycle_hot_bundle_v3, validate_rational_lifecycle_hot_bundle_v3,
};
pub use compact_artifacts_v4::{
    RATIONAL_LIFECYCLE_COMPACT_DESCRIPTOR_BYTES_V4, RATIONAL_LIFECYCLE_COMPACT_STRATEGY_BYTES_V4,
    RationalLifecycleCompactArtifactInputV4, RationalLifecycleCompactArtifactsV4,
    RationalLifecycleCompactBundleInputV4, RationalLifecycleCompactBundleV4,
    build_rational_lifecycle_compact_bundle_v4, encode_rational_lifecycle_compact_artifacts_v4,
    validate_rational_lifecycle_compact_bundle_v4,
};
pub use compact_operator_v4::{
    RationalLifecycleVacancyAccountsV4, build_rational_lifecycle_compact_hot_instruction_v4,
};
pub use effect::{
    RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3, encode_rational_lifecycle_effect_v3,
    lifecycle_claims_account_count_v3, lifecycle_logical_account_count_v3,
};
pub use operator::{
    CheckedRationalLifecycleHotOuterV3, RationalLifecycleHotInstructionV3,
    RationalLifecycleHotStateV3, build_rational_lifecycle_hot_instruction_v3,
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
    /// A content-addressed semantic coordinate was zero.
    ContentIdentity,
    /// Interpreted execution-strategy construction or join refused.
    Strategy(dclutch_execution_strategy_contract::v2::Error),
    /// CapabilityProgram construction or hostile decoding refused.
    Descriptor(dclutch_capability_program_contract::Error),
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
        LifecycleActionV2::RetireReceipt => coordinates != 0,
    };
    if accepted {
        Ok(coordinates)
    } else {
        Err(Error::ActionGeometry)
    }
}
