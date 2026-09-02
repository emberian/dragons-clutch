#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived construction for the executable Fractional Claims family.
//!
//! The operator compiles and validates the CapabilityV4/LifecycleV5 artifacts
//! a Fractional release selects, lowers `FractionalExposureActionV2` requests
//! into Claims and Token-2022 effects, and plans producer-subtree retirement.
//! It never signs, submits, invents a shard Mint, or persists a supply
//! projection. Parsed Claims/Token balances are an explicitly named adapter
//! input and are rechecked by the onchain child route.

mod artifacts_v4;
mod atomic_v3;
mod exposure_action_v2;
mod hot_v2;
mod retirement_v3;
mod selected_release_v4;
mod topology_v3;

pub use artifacts_v4::{
    FRACTIONAL_COMMON_IDENTITIES_V4, FRACTIONAL_COMMON_SCALARS_V4,
    FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4, FractionalClaimsAccountRuleV1,
    FractionalSelectedArtifactErrorV4, FractionalSelectedBundleInputV4, FractionalSelectedBundleV4,
    FractionalSelectedProfileInputV4, build_fractional_current_selected_bundle_v4,
    build_fractional_selected_bundle_v4, validate_fractional_current_selected_bundle_v4,
    validate_fractional_selected_bundle_v4,
};
pub use atomic_v3::{
    build_fractional_atomic_claims_instruction_v3,
    build_fractional_terminal_atomic_claims_instruction_v3,
};
pub use exposure_action_v2::{
    CheckedFractionalTokenBehaviorV2, FractionalExposureMintSnapshotV2,
    FractionalExposureRentCloseObservationV2, FractionalExposureRentClosePlanV2,
    FractionalExposureRetirementContextV2, FractionalExposureRetirementPlanV2,
    FractionalExposureTerminalCandidateV2, FractionalExposureTerminalInputV2,
    FractionalExposureTerminalPostObservationV2, FractionalExposureTokenEffectV2,
    FractionalExposureTokenObservationV2, FractionalExposureTokenPlanV2,
    FractionalTokenAccountSnapshotV1, FractionalTokenBehaviorRecordAdmissionV2,
    authenticate_fractional_token_behavior_v2, fractional_exposure_record_admission_v2,
    plan_fractional_exposure_rent_close_v2, plan_fractional_exposure_retirement_v2,
    plan_fractional_exposure_terminal_candidate_v2, plan_fractional_exposure_token_effect_v2,
    validate_fractional_exposure_terminal_postcondition_v2,
};
pub use hot_v2::{
    FractionalHotChildCoordinatesV2, FractionalHotProfileV2, FractionalHotRetirementCoordinatesV2,
    FractionalHotTokenCoordinatesV2, lower_fractional_hot_rent_close_v2,
    lower_fractional_hot_retirement_effects_v2, lower_fractional_hot_signed_delta_v2,
    lower_fractional_hot_terminal_v2, lower_fractional_hot_token_effect_v2,
};
pub use retirement_v3::{
    FractionalRetirementCoordinateSnapshotV3, FractionalRetirementDeploymentV3,
    FractionalRetirementDiscoveryV3, FractionalRetirementInstructionPlanV3,
    FractionalRetirementNextPlanV3, FractionalRetirementRecordV3, FractionalRetirementSnapshotV3,
    discover_fractional_retirement_next_v3, plan_fractional_retirement_instruction_v3,
    plan_fractional_retirement_next_v3,
};
pub use selected_release_v4::{
    FRACTIONAL_ACTIVATION_REQUEST_BYTES_V1, FRACTIONAL_ACTIVATION_REQUEST_MAGIC_V1,
    FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_ID_V1, FRACTIONAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1,
    FRACTIONAL_ACTIVATION_SELECTOR_V1, FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4,
    FRACTIONAL_SELECTED_ACTION_COUNT_V4, FRACTIONAL_SELECTED_ACTIONS_V4,
    FRACTIONAL_SELECTED_FUNDING_LEDGER_SLOTS_V1, FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4,
    FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4, FractionalActivationBundleErrorV1,
    FractionalActivationBundleInputV1, FractionalCurrentReleaseV4, FractionalFrameWidthsV4,
    FractionalPublicationRecordV1, FractionalSelectedPublicationV4,
    FractionalSelectedReleaseErrorV4, FractionalSelectedReleaseInputV4,
    FractionalSelectedReleaseV4, build_fractional_activation_bundle_v1,
    fractional_activation_request_v1, fractional_claims_frame_spec_v4,
    fractional_current_release_v4, fractional_selected_release_v4,
    validate_fractional_activation_bundle_v1, validate_fractional_activation_request_v1,
    validate_fractional_current_release_v4, validate_fractional_selected_release_v4,
};
pub use topology_v3::{
    FRACTIONAL_CHILD_ENVELOPE_BYTES_V3, FRACTIONAL_DEVNET_MAX_ACCOUNT_LOCKS_V3,
    FractionalFrameCensusV3, FractionalFrameKindV3, TOKEN_2022_TRANSFER_CHECKED_BYTES,
    admit_fractional_devnet_locks_v3, fractional_frame_census_v3,
};

/// Stable operator refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Finalized artifact or Product observation refused.
    ChainArtifacts,
    /// Terms/projection bytes or runtime-width reserve rows refused.
    Projection,
    /// Trading account frame omitted or substituted a selected program/signer.
    AccountFrame,
    /// Unsigned v0 compilation or packet sizing refused.
    Message,
    /// Canonical Claims lowering, frame, receipt, or post-state refused.
    Claims,
    /// Selected TokenBehaviorV2, Token-owned state, or exact Token-2022 effect refused.
    Token,
    /// Canonical producer-subtree retirement or lifecycle RentV2 closure refused.
    Rent,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;
