//! Complete finalized-artifact join for data-defined General execution.
//!
//! `CapabilityProgramSetV2` selects one schema-bound descriptor from the exact
//! action byte in the General request. This module joins that descriptor to its
//! config, AccountProfile, lifecycle policy, RequestProfile, ExecutionStrategy,
//! TransitionVM, and EffectProgram artifacts. It is a release/admission
//! contract, not account or CPI authority; generic Trading remains the only
//! physical executor and writer.

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE},
};
use dclutch_capability_program_contract::{
    hot_v3::{
        HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
        HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
        HOT_RUNTIME_PRODUCT_COORDINATE_V3, HOT_RUNTIME_ROOT_COORDINATE_V3,
    },
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::{
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    affine_batch_v2::{
        AFFINE_BATCH_PLAN_HEADER_BYTES_V2, AFFINE_BATCH_POSITION_BYTES_V2,
        AFFINE_BATCH_ROW_BYTES_V2, AffineBatchPlanV2,
    },
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CompartmentV1, CustodyRequestV1, OperationV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
};
use dclutch_execution_strategy_contract::v2::{
    AdmittedAotAuthorizationV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
    validate_admitted_aot_v4,
};
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    v3::{GENERAL_CONFIG_SCHEMA_ID_V3, GeneralConfigV3},
};
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use dclutch_request_profile_contract::{RequestProfileV1, validate_request};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    account_rules_v3::{
        GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3, general_account_profile_fixed_count_v3,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, GENERAL_HOT_ITEM_SCALAR_STRIDE_V3, scalar,
    },
    specialization::general_request_profile_bytes_v1,
};

/// Exact action-selector byte in the canonical 64-byte General request.
pub const GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3: u32 = 10;
/// Schema preimage for the runtime-width General controller request.
pub const GENERAL_CONTROLLER_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/general-controller-request-v5-manifest-source-split-v1";
/// SHA-256 of [`GENERAL_CONTROLLER_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x42, 0xdf, 0x00, 0x82, 0xab, 0x62, 0x58, 0xe5, 0x4e, 0xe3, 0x31, 0x35, 0x34, 0x54, 0xdc, 0x74,
    0xba, 0x36, 0x5e, 0x18, 0x01, 0x93, 0xc0, 0x3a, 0xba, 0x6a, 0xe0, 0xaa, 0x1c, 0x2f, 0x55, 0x2b,
];

/// Common scalar temporarily receiving the Product-authenticated tail width.
pub const GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3: u16 = 10;

/// Exact descriptor-selected finalized bytes for one General action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactBytesV3<'a> {
    /// Canonical action-to-program set.
    pub program_set: &'a [u8],
    /// Action-selected CapabilityProgramV4 descriptor.
    pub descriptor: &'a [u8],
    /// Manifest-selected immutable General config.
    pub config: &'a [u8],
    /// Runtime-width account projection.
    pub account_profile: &'a [u8],
    /// Trading-owned state lifecycle and rent policy.
    pub lifecycle_policy: &'a [u8],
    /// Action-specific request validator/projection.
    pub request_profile: &'a [u8],
    /// Descriptor-selected ExecutionStrategy V2.
    pub strategy: &'a [u8],
    /// Strategy-selected semantic-equivalence certificate.
    pub certificate: &'a [u8],
    /// Registry-admitted certificate authorization.
    pub admission: &'a [u8],
    /// Strategy-selected TransitionVM program.
    pub transition: &'a [u8],
    /// Common Trading local/child effect program.
    pub effect: &'a [u8],
}

/// Independently authenticated immutable selections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactSelectionV3 {
    /// Capability release selecting the exact ProgramSet bytes.
    pub program_set: [u8; 32],
    /// Manifest entry selecting the exact config bytes.
    pub config: [u8; 32],
    /// Registry-authenticated stateless accelerator ArtifactRelease.
    pub artifact_release: [u8; 32],
}

/// Complete borrowed artifact bundle after every content and geometry join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactBundleV3<'a> {
    /// Exact decoded family request.
    pub request: ControllerRequestV2,
    /// Action-selected capability descriptor.
    pub descriptor: CapabilityProgramV4,
    /// Immutable runtime-width General policy.
    pub config: GeneralConfigV3,
    /// Runtime-width account projection.
    pub account_profile: AccountProfileV2<'a>,
    /// Trading-owned lifecycle policy.
    pub lifecycle_policy: StateLifecyclePolicyV5<'a>,
    /// Exact action-specific request program.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
    /// Private proof that the selected AOT chain was admitted completely.
    pub admitted_aot: AdmittedAotAuthorizationV2,
    /// Exact underlying transition program.
    pub transition: TransitionProgramV3<'a>,
    /// Exact common effect program.
    pub effect: EffectProgramV3<'a>,
    /// Product-authenticated runtime outcome width.
    pub tail_count: u32,
}

/// Stable refusal from the complete General artifact join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralArtifactErrorV3 {
    /// A selected content identity was zero or differed from exact bytes.
    ContentIdentity,
    /// ProgramSet selector geometry or action selection refused.
    ProgramSet,
    /// General request bytes or action-specific coordinates refused.
    Request,
    /// Descriptor family, schema, root, or config binding differed.
    Descriptor,
    /// Immutable config hostile decoding or descriptor binding refused.
    Config,
    /// AccountProfile hostile decoding refused.
    AccountProfile,
    /// Lifecycle policy hostile decoding or geometry refused.
    LifecyclePolicy,
    /// RequestProfile selection, exact semantics, or projection refused.
    RequestProfile,
    /// ExecutionStrategy selection refused.
    Strategy,
    /// Translation certificate or Registry admission refused.
    Admission,
    /// Transition program selection or decoding refused.
    Transition,
    /// EffectProgram selection or route grammar refused.
    Effect,
    /// Runtime tail or register/account geometry differed.
    Geometry,
}

/// Result alias for General V3 artifact admission.
pub type Result<T> = core::result::Result<T, GeneralArtifactErrorV3>;

/// Authenticate and join one complete General runtime-width artifact bundle.
///
/// `tail_count` comes from the authenticated Product result domain. V3 has no
/// config-owned outcome count and cannot reintroduce a physical semantic cap.
pub fn authenticate_general_artifacts_v3<'a>(
    selection: GeneralArtifactSelectionV3,
    artifacts: GeneralArtifactBytesV3<'a>,
    family_request: &'a [u8],
    tail_count: u32,
) -> Result<GeneralArtifactBundleV3<'a>> {
    if tail_count == 0 {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    require_selected(selection.program_set, artifacts.program_set)?;
    let set = CapabilityProgramSetV2::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| GeneralArtifactErrorV3::ProgramSet)?;
    if set.selector_offset() != GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
    {
        return Err(GeneralArtifactErrorV3::ProgramSet);
    }
    let request =
        ControllerRequestV2::decode(family_request).map_err(|_| GeneralArtifactErrorV3::Request)?;
    let selected_descriptor = set
        .select_descriptor(family_request)
        .map_err(|_| GeneralArtifactErrorV3::ProgramSet)?;
    let descriptor_id = digest(artifacts.descriptor);
    if selected_descriptor.schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
        || selected_descriptor.program().to_bytes() != descriptor_id
    {
        return Err(GeneralArtifactErrorV3::ContentIdentity);
    }
    let descriptor = CapabilityProgramV4::decode(artifacts.descriptor)
        .map_err(|_| GeneralArtifactErrorV3::Descriptor)?;
    validate_descriptor(descriptor)?;

    require_selected(selection.config, artifacts.config)?;
    let config =
        GeneralConfigV3::decode(artifacts.config).map_err(|_| GeneralArtifactErrorV3::Config)?;
    if config.program_set_id() != selection.program_set
        || descriptor.capacity_profile().to_bytes() != config.capacity_profile_id()
    {
        return Err(GeneralArtifactErrorV3::Config);
    }

    require_content(
        descriptor.account_profile().program().to_bytes(),
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?;
    require_content(
        descriptor.lifecycle().program().to_bytes(),
        artifacts.lifecycle_policy,
    )?;
    let lifecycle_policy = StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        digest(artifacts.lifecycle_policy),
        artifacts.lifecycle_policy,
    )
    .map_err(|_| GeneralArtifactErrorV3::LifecyclePolicy)?;
    lifecycle_policy
        .validate_account_profile(account_profile)
        .map_err(|_| GeneralArtifactErrorV3::LifecyclePolicy)?;
    if lifecycle_policy
        .action_plan_count(request.action as u32)
        .map_err(|_| GeneralArtifactErrorV3::LifecyclePolicy)?
        == 0
    {
        return Err(GeneralArtifactErrorV3::LifecyclePolicy);
    }

    require_content(
        descriptor.request_profile().program().to_bytes(),
        artifacts.request_profile,
    )?;
    if artifacts.request_profile != general_request_profile_bytes_v1(request.action) {
        return Err(GeneralArtifactErrorV3::RequestProfile);
    }
    let request_profile = RequestProfileV1::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        digest(artifacts.request_profile),
        artifacts.request_profile,
    )
    .map_err(|_| GeneralArtifactErrorV3::RequestProfile)?;
    execute_request_profile(request_profile, family_request, tail_count)?;

    require_content(
        descriptor.strategy().program().to_bytes(),
        artifacts.strategy,
    )?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| GeneralArtifactErrorV3::Strategy)?;
    let strategy_id = content(digest(artifacts.strategy))?;
    strategy
        .validate_descriptor_selection_v4(strategy_id, descriptor)
        .map_err(|_| GeneralArtifactErrorV3::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.certificate_schema().to_bytes() != EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2
        || strategy.admission_schema().to_bytes() != EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2
    {
        return Err(GeneralArtifactErrorV3::Strategy);
    }
    let certificate_id = content(digest(artifacts.certificate))?;
    let admission_id = content(digest(artifacts.admission))?;
    if strategy.certificate_program() != Some(certificate_id)
        || strategy.admission_program() != Some(admission_id)
    {
        return Err(GeneralArtifactErrorV3::Admission);
    }
    let certificate = ExecutionStrategyCertificateV2::decode(artifacts.certificate)
        .map_err(|_| GeneralArtifactErrorV3::Admission)?;
    let admission = ExecutionStrategyAdmissionV2::decode(artifacts.admission)
        .map_err(|_| GeneralArtifactErrorV3::Admission)?;
    require_content(
        descriptor.transition().program().to_bytes(),
        artifacts.transition,
    )?;
    if strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID {
        return Err(GeneralArtifactErrorV3::Transition);
    }
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| GeneralArtifactErrorV3::Transition)?;

    require_content(descriptor.effect().program().to_bytes(), artifacts.effect)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect().program().to_bytes(),
        digest(artifacts.effect),
        artifacts.effect,
    )
    .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let admitted_aot = validate_admitted_aot_v4(
        strategy_id,
        strategy,
        descriptor,
        certificate_id,
        certificate,
        AuthenticatedInterpreterArtifactsV2 {
            account_profile_program: descriptor.account_profile().program(),
            request_profile_schema: descriptor.request_profile().schema(),
            request_profile_program: descriptor.request_profile().program(),
            transition_schema: strategy.transition_schema(),
            transition_program: strategy.transition_program(),
            effect_program: descriptor.effect().program(),
        },
        ArtifactReleaseIdV1::new(selection.artifact_release)
            .map_err(|_| GeneralArtifactErrorV3::Admission)?,
        Some((admission_id, admission)),
    )
    .map_err(|_| GeneralArtifactErrorV3::Admission)?;
    validate_geometry(
        request.action,
        tail_count,
        family_request.len(),
        account_profile,
        request_profile,
        transition,
        effect,
    )?;
    validate_routes(request.action, effect)?;

    Ok(GeneralArtifactBundleV3 {
        request,
        descriptor,
        config,
        account_profile,
        lifecycle_policy,
        request_profile,
        strategy,
        admitted_aot,
        transition,
        effect,
        tail_count,
    })
}

fn validate_descriptor(descriptor: CapabilityProgramV4) -> Result<()> {
    if descriptor.kind().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != GENERAL_CONFIG_SCHEMA_ID_V3
        || descriptor.request_schema().to_bytes() != GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3
        || descriptor.root_schema().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
        || descriptor.derivation_policy() != descriptor.lifecycle().program()
        || descriptor.account_profile().schema().to_bytes()
            != dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID
        || descriptor.request_profile().schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        || descriptor.strategy().schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition().schema().to_bytes()
            != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || descriptor.effect().schema().to_bytes() != dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| GeneralArtifactErrorV3::Geometry)?
            != GENERAL_ROOT_BYTES_V2
    {
        return Err(GeneralArtifactErrorV3::Descriptor);
    }
    Ok(())
}

fn execute_request_profile(
    profile: RequestProfileV1<'_>,
    request: &[u8],
    tail_count: u32,
) -> Result<()> {
    if profile
        .request_bytes(tail_count)
        .map_err(|_| GeneralArtifactErrorV3::RequestProfile)?
        != request.len()
    {
        return Err(GeneralArtifactErrorV3::RequestProfile);
    }
    let scalars = usize::from(profile.common_scalar_count());
    let identities = usize::from(profile.common_identity_count());
    if scalars != GENERAL_HOT_COMMON_SCALARS_V3 as usize
        || identities != GENERAL_HOT_COMMON_IDENTITIES_V3 as usize
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    // Artifact admission consumes no projected bank. The semantic owner walks
    // every selected Require operation and every Project read without building
    // three redundant copies of the 1.9KiB General register prefix. Generic
    // Trading still performs the failure-atomic projection before execution.
    validate_request(profile, 0, request).map_err(|_| GeneralArtifactErrorV3::RequestProfile)
}

fn validate_geometry(
    action: Action,
    tail_count: u32,
    request_bytes: usize,
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    validate_hot_account_profile(account)?;
    let expected_fixed = general_account_profile_fixed_count_v3(action)
        .map_err(|_| GeneralArtifactErrorV3::Geometry)?;
    if account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.fixed_account_count() != expected_fixed
        || account.dynamic_fixed_span_count() != 1
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    let scratch_span = account
        .dynamic_fixed_span(0)
        .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?;
    if scratch_span.insertion_coordinate() != expected_fixed
        || scratch_span.count_scalar()
            != u16::try_from(scalar::INPUT_SCRATCH_PAGE_COUNT)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || scratch_span.rule_start() != 0
        || scratch_span.rule_stride() != GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3
        || scratch_span.minimum() != 1
        || scratch_span.maximum() != u32::MAX
        || scratch_span.step() != 1
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    if request_bytes != CONTROLLER_REQUEST_BYTES_V2
        || request.item_request_bytes() != 0
        || account.common_scalar_count()
            != u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.item_scalar_stride()
            != u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.common_identity_count()
            != u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.item_identity_stride()
            != u16::try_from(GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        // Under Profile13 the item-rule table is repurposed exclusively as
        // the dynamic fixed-span template bank. It is physical scratch-page
        // geometry, not a Product-N semantic account stride.
        || account.item_account_stride() != GENERAL_SCRATCH_PAGE_RULE_STRIDE_V3
        || request.common_scalar_count() != account.common_scalar_count()
        || request.item_scalar_stride() != account.item_scalar_stride()
        || request.common_identity_count() != account.common_identity_count()
        || request.item_identity_stride() != account.item_identity_stride()
        || transition.common_scalar_count() != account.common_scalar_count()
        || transition.item_scalar_stride() != account.item_scalar_stride()
        || transition.common_identity_count() != account.common_identity_count()
        || transition.item_identity_stride() != account.item_identity_stride()
        || effect.common_scalar_count() != account.common_scalar_count()
        || effect.item_scalar_stride() != account.item_scalar_stride()
        || effect.common_identity_count() != account.common_identity_count()
        || effect.item_identity_stride() != account.item_identity_stride()
        || effect.fixed_account_count() != account.fixed_account_count()
        || effect.item_account_stride() != 0
        || request
            .scalar_count(tail_count)
            .map_err(|_| GeneralArtifactErrorV3::Geometry)?
            != effect
                .scalar_count(tail_count)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || request
            .identity_count(tail_count)
            .map_err(|_| GeneralArtifactErrorV3::Geometry)?
            != effect
                .identity_count(tail_count)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    Ok(())
}

fn validate_hot_account_profile(account: AccountProfileV2<'_>) -> Result<()> {
    if usize::from(account.fixed_account_count()) < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    let expected = [
        (
            HOT_RUNTIME_ROOT_COORDINATE_V3,
            0x02_u8,
            u32::try_from(
                dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                    .checked_add(GENERAL_ROOT_BYTES_V2)
                    .ok_or(GeneralArtifactErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralArtifactErrorV3::Geometry)?,
            0_u32,
        ),
        (
            HOT_RUNTIME_CONFIG_COORDINATE_V3,
            0_u8,
            u32::try_from(dclutch_general_config_contract::v3::GENERAL_CONFIG_BYTES_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?,
            0_u32,
        ),
        (
            HOT_RUNTIME_PRODUCT_COORDINATE_V3,
            0_u8,
            u32::try_from(PRODUCT_RECORD_BYTES_V2).map_err(|_| GeneralArtifactErrorV3::Geometry)?,
            0_u32,
        ),
        (
            HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
            0_u8,
            u32::try_from(PORTFOLIO_HEADER_BYTES).map_err(|_| GeneralArtifactErrorV3::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?,
        ),
    ];
    for (coordinate, privileges, data_length, data_item_stride) in expected {
        let rule = account
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| GeneralArtifactErrorV3::Geometry)?,
            )
            .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?;
        if rule.privileges() != privileges
            || rule.effect_permissions() != 0
            || rule.alias_kind()
                != dclutch_account_profile_contract::v2::AliasKindV2::SelfCoordinate
            || rule.alias_index() != 0
            || rule.data_length() != data_length
            || rule.data_item_stride() != data_item_stride
        {
            return Err(GeneralArtifactErrorV3::Geometry);
        }
    }
    let linked_basis = account
        .rule(
            false,
            u16::try_from(HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?,
        )
        .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?;
    // The exact linked-basis record width is selected by this immutable
    // AccountProfile and checked against the authenticated raw record by Hot.
    // It is deliberately not derived from Product N: graded bases may carry a
    // runtime term/knot tail whose width is independent of the outcome count.
    if linked_basis.privileges() != 0
        || linked_basis.effect_permissions() != 0
        || linked_basis.alias_kind()
            != dclutch_account_profile_contract::v2::AliasKindV2::SelfCoordinate
        || linked_basis.alias_index() != 0
        || linked_basis.data_length() == 0
        || linked_basis.data_item_stride() != 0
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    let projection = account
        .tail_count_projection()
        .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?
        .ok_or(GeneralArtifactErrorV3::Geometry)?;
    if usize::from(projection.account()) != HOT_RUNTIME_PORTFOLIO_COORDINATE_V3
        || projection.register() != GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3
        || usize::try_from(projection.data_offset())
            .map_err(|_| GeneralArtifactErrorV3::Geometry)?
            != PORTFOLIO_COEFFICIENT_COUNT_OFFSET
    {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    Ok(())
}

fn validate_routes(action: Action, effect: EffectProgramV3<'_>) -> Result<()> {
    match action {
        Action::Consider | Action::Freeze => require_route_count(effect, 0),
        Action::InitializeSettlement => {
            require_route_count(effect, 3)?;
            require_position_route(effect, 0, ProtocolPositionActionV2::Admit)?;
            require_custody_route(
                effect,
                1,
                OperationV1::InitializeReplay,
                CompartmentV1::None,
                CompartmentV1::None,
                None,
            )?;
            require_custody_route(
                effect,
                2,
                OperationV1::OpenVault,
                CompartmentV1::None,
                CompartmentV1::Settlement,
                Some((FixedRole::Custody, 1)),
            )
        }
        Action::Collect => {
            require_route_count(effect, 2)?;
            require_affine_route(effect, 0, 2)?;
            require_custody_transfer_route(
                effect,
                1,
                CompartmentV1::External,
                CompartmentV1::Settlement,
            )
        }
        Action::Materialize => {
            require_route_count(effect, 2)?;
            require_affine_route(effect, 0, 1)?;
            // The exact direction is selected from the authenticated
            // complete-set move: Mint is Settlement -> Hoard, Merge is the
            // inverse.  Both use one canonical Transfer template and the
            // admitted EffectProgram patches the two typed compartment bytes.
            require_custody_transfer_route_either(
                effect,
                1,
                (CompartmentV1::Settlement, CompartmentV1::HoardPrincipal),
                (CompartmentV1::HoardPrincipal, CompartmentV1::Settlement),
            )
        }
        Action::Distribute => {
            require_route_count(effect, 2)?;
            require_affine_route(effect, 0, 2)?;
            require_custody_transfer_route(
                effect,
                1,
                CompartmentV1::Settlement,
                CompartmentV1::External,
            )
        }
        Action::Close => {
            require_route_count(effect, 4)?;
            require_custody_transfer_route(
                effect,
                0,
                CompartmentV1::Settlement,
                CompartmentV1::External,
            )?;
            require_position_route(effect, 1, ProtocolPositionActionV2::Close)?;
            require_custody_route(
                effect,
                2,
                OperationV1::CloseVault,
                CompartmentV1::Settlement,
                CompartmentV1::None,
                None,
            )?;
            require_custody_route(
                effect,
                3,
                OperationV1::CloseReplay,
                CompartmentV1::None,
                CompartmentV1::None,
                Some((FixedRole::Custody, 2)),
            )
        }
    }
}

fn require_route_count(effect: EffectProgramV3<'_>, expected: u16) -> Result<()> {
    if effect.route_count() == expected {
        Ok(())
    } else {
        Err(GeneralArtifactErrorV3::Effect)
    }
}

fn require_position_route(
    effect: EffectProgramV3<'_>,
    index: u16,
    action: ProtocolPositionActionV2,
) -> Result<()> {
    let route = effect
        .route(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let (fixed, item) = effect
        .route_template(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let request =
        ProtocolPositionRequestV2::decode(fixed).map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let presence = match action {
        ProtocolPositionActionV2::Admit => ProtocolPositionPresenceV2::Vacant,
        ProtocolPositionActionV2::Close => ProtocolPositionPresenceV2::Existing,
    };
    if route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || !item.is_empty()
        || request.action != action
        || request.presence != presence
        || request.owner_kind != ProtocolPositionOwnerKindV2::TradingRecord
    {
        return Err(GeneralArtifactErrorV3::Effect);
    }
    Ok(())
}

fn require_affine_route(
    effect: EffectProgramV3<'_>,
    index: u16,
    position_count: u32,
) -> Result<()> {
    const MAX_TEMPLATE_BYTES: usize = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
        + 2 * AFFINE_BATCH_POSITION_BYTES_V2
        + AFFINE_BATCH_ROW_BYTES_V2;
    let route = effect
        .route(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let (fixed, item) = effect
        .route_template(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let expected_fixed = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
        .checked_add(
            usize::try_from(position_count)
                .map_err(|_| GeneralArtifactErrorV3::Effect)?
                .checked_mul(AFFINE_BATCH_POSITION_BYTES_V2)
                .ok_or(GeneralArtifactErrorV3::Effect)?,
        )
        .ok_or(GeneralArtifactErrorV3::Effect)?;
    if route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::AffineOnce
        || fixed.len() != expected_fixed
        || item.len() != AFFINE_BATCH_ROW_BYTES_V2
    {
        return Err(GeneralArtifactErrorV3::Effect);
    }
    // A canonical item template is one exact N=1 child request. Runtime
    // projection changes the authenticated outcome/row counts and repeats the
    // same exact row ABI; it cannot smuggle another child wire behind a magic
    // prefix.
    let total = fixed
        .len()
        .checked_add(item.len())
        .ok_or(GeneralArtifactErrorV3::Effect)?;
    let mut packet = [0_u8; MAX_TEMPLATE_BYTES];
    packet
        .get_mut(..fixed.len())
        .ok_or(GeneralArtifactErrorV3::Effect)?
        .copy_from_slice(fixed);
    packet
        .get_mut(fixed.len()..total)
        .ok_or(GeneralArtifactErrorV3::Effect)?
        .copy_from_slice(item);
    let plan =
        AffineBatchPlanV2::decode(packet.get(..total).ok_or(GeneralArtifactErrorV3::Effect)?)
            .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    if plan.caller_role() != ClaimsCallerRole::Trading
        || plan.outcome_count() != 1
        || plan.position_count() != position_count
        || plan.row_count() != 1
    {
        return Err(GeneralArtifactErrorV3::Effect);
    }
    Ok(())
}

fn require_custody_transfer_route(
    effect: EffectProgramV3<'_>,
    index: u16,
    source: CompartmentV1,
    destination: CompartmentV1,
) -> Result<()> {
    require_custody_route(
        effect,
        index,
        OperationV1::Transfer,
        source,
        destination,
        None,
    )
}

fn require_custody_route(
    effect: EffectProgramV3<'_>,
    index: u16,
    operation: OperationV1,
    source: CompartmentV1,
    destination: CompartmentV1,
    dependency: Option<(FixedRole, u16)>,
) -> Result<()> {
    let route = effect
        .route(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let (fixed, item) = effect
        .route_template(index)
        .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let request = CustodyRequestV1::decode(fixed).map_err(|_| GeneralArtifactErrorV3::Effect)?;
    if route.role() != FixedRole::Custody
        || route.kind() != RouteKindV3::Once
        || !item.is_empty()
        || request.operation != operation
        || request.source_compartment != source
        || request.destination_compartment != destination
        || route.receipt_dependency().map(|value| {
            (
                value.producer_role(),
                value.producer_route(),
                usize::from(value.expected_receipt_bytes()),
            )
        }) != dependency.map(|(role, route)| (role, route, CUSTODY_RECEIPT_BYTES_V1))
    {
        return Err(GeneralArtifactErrorV3::Effect);
    }
    Ok(())
}

fn require_custody_transfer_route_either(
    effect: EffectProgramV3<'_>,
    index: u16,
    first: (CompartmentV1, CompartmentV1),
    second: (CompartmentV1, CompartmentV1),
) -> Result<()> {
    require_custody_transfer_route(effect, index, first.0, first.1)
        .or_else(|_| require_custody_transfer_route(effect, index, second.0, second.1))
}

fn require_selected(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    if selected == [0; 32] || selected != digest(bytes) {
        Err(GeneralArtifactErrorV3::ContentIdentity)
    } else {
        Ok(())
    }
}

fn require_content(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    require_selected(selected, bytes)
}

fn content(value: [u8; 32]) -> Result<ContentId> {
    ContentId::new(value).map_err(|_| GeneralArtifactErrorV3::ContentIdentity)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_capability_program_contract::v4::CAPABILITY_PROGRAM_V4_BYTES;
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        StrategyDispositionV2,
    };
    use dclutch_general_config_contract::v3::{GENERAL_CONFIG_BYTES_V3, GeneralConfigV3Input};
    use std::{vec, vec::Vec};

    use super::*;

    struct Fixture {
        set: Vec<u8>,
        descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
        config: [u8; GENERAL_CONFIG_BYTES_V3],
        account: Vec<u8>,
        lifecycle: Vec<u8>,
        request_profile: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        request: [u8; CONTROLLER_REQUEST_BYTES_V2],
    }

    impl Fixture {
        fn artifacts(&self) -> GeneralArtifactBytesV3<'_> {
            GeneralArtifactBytesV3 {
                program_set: &self.set,
                descriptor: &self.descriptor,
                config: &self.config,
                account_profile: &self.account,
                lifecycle_policy: &self.lifecycle,
                request_profile: &self.request_profile,
                strategy: &self.strategy,
                certificate: &self.certificate,
                admission: &self.admission,
                transition: &self.transition,
                effect: &self.effect,
            }
        }

        fn selection(&self) -> GeneralArtifactSelectionV3 {
            GeneralArtifactSelectionV3 {
                program_set: digest(&self.set),
                config: digest(&self.config),
                artifact_release: [13; 32],
            }
        }
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture slice")
            .copy_from_slice(value);
    }

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("nonzero fixture identity")
    }

    fn account_profile() -> Vec<u8> {
        use crate::account_rules_v3::{
            GeneralExternalAccountWidthsV3, general_account_profile_operation_count_v3,
            general_account_profile_operation_v3, general_account_profile_rule_v3,
            general_scratch_page_rule_v3, general_scratch_page_span_v3,
        };
        use dclutch_account_profile_contract::v2::{
            TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
            encode::{
                RegisterGeometryV2,
                encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic,
            },
        };

        const WIDTHS: GeneralExternalAccountWidthsV3 = GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: 256,
            result_domain: 192,
            rent_sysvar: 17,
            core_market: 320,
            activation_cache: 160,
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: 112,
            rent_credit: 48,
        };
        let action = Action::Freeze;
        let fixed_count = general_account_profile_fixed_count_v3(action).expect("fixed count");
        let operations = (0..general_account_profile_operation_count_v3(action))
            .map(|index| {
                general_account_profile_operation_v3(action, index).expect("canonical operation")
            })
            .collect::<Vec<_>>();
        let bytes = dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (usize::from(fixed_count) + 1) * dclutch_account_profile_contract::v2::RULE_BYTES
            + operations.len() * dclutch_account_profile_contract::v2::OPERATION_BYTES;
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
                destination: u16::try_from(crate::hot_candidate_v3::identity::TRADING_PROGRAM)
                    .expect("program coordinate"),
            },
            TrustedBuiltinIdentityV2::None,
            &[general_scratch_page_span_v3(action).expect("scratch span")],
            fixed_count,
            |coordinate| {
                general_account_profile_rule_v3(action, coordinate, WIDTHS)
                    .map_err(|_| dclutch_account_profile_contract::v2::Error::InvalidLength)
            },
            &[general_scratch_page_rule_v3()],
            &operations,
            RegisterGeometryV2 {
                common_scalars: u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
                    .expect("common scalars"),
                item_scalar_stride: u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .expect("item scalars"),
                common_identities: u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                    .expect("common identities"),
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut output,
        )
        .expect("Profile13 account artifact");
        output
    }

    fn lifecycle_policy() -> Vec<u8> {
        lifecycle_policy_for(Action::Freeze)
    }

    fn lifecycle_policy_for(action: Action) -> Vec<u8> {
        let bytes = crate::state_artifacts_v3::general_state_lifecycle_bytes_v5(action)
            .expect("lifecycle width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        crate::state_artifacts_v3::encode_general_state_lifecycle_v5_atomic(
            action,
            (action == Action::InitializeSettlement).then(|| {
                crate::state_artifacts_v3::GeneralChildRentWidthsV5::new(1, 165)
                    .expect("child widths")
            }),
            &mut scratch,
            &mut output,
        )
        .expect("successor lifecycle");
        output
    }

    fn transition() -> Vec<u8> {
        let action = Action::Freeze;
        let (prelude, item, epilogue) =
            crate::transition_artifacts_v3::general_transition_instruction_count_v3(action);
        let mut instructions = vec![
            crate::transition_artifacts_v3::GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3;
            prelude + item + epilogue
        ];
        let bytes = crate::transition_artifacts_v3::general_transition_program_bytes_v3(action)
            .expect("transition width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        crate::transition_artifacts_v3::encode_general_transition_program_v3_atomic(
            action,
            &mut instructions,
            &mut scratch,
            &mut output,
        )
        .expect("successor transition");
        output
    }

    fn effect() -> Vec<u8> {
        effect_for(Action::Freeze)
    }

    fn effect_for(action: Action) -> Vec<u8> {
        use crate::effect_artifacts_v3::{
            GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v3_atomic,
            general_effect_instruction_count_v3, general_effect_program_bytes_v3,
            general_effect_template_bytes_v3,
        };

        let (fixed, item) = general_effect_instruction_count_v3(action);
        let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; fixed + item];
        let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
        let bytes = general_effect_program_bytes_v3(action).expect("effect width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        encode_general_effect_program_v3_atomic(
            action,
            &mut instructions,
            &mut templates,
            &mut scratch,
            &mut output,
        )
        .expect("successor effect");
        output
    }

    fn program_set(descriptor: [u8; 32]) -> Vec<u8> {
        use dclutch_capability_program_contract::set_v2::{
            CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
            encode_program_set_v2, encoded_program_set_bytes_v2,
        };

        let entry = CapabilityProgramSetEntryV2::new(
            Action::Freeze as u32,
            CapabilityDescriptorReferenceV2::new(
                id(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID),
                id(descriptor),
            ),
        );
        let mut output = vec![0_u8; encoded_program_set_bytes_v2(1).expect("one descriptor")];
        encode_program_set_v2(
            GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
            SelectorWidthV2::U8,
            &[entry],
            &mut output,
        )
        .expect("V2 program set");
        output
    }

    fn fixture() -> Fixture {
        use dclutch_capability_program_contract::v4::{ArtifactReferenceV4, CapabilityArtifactsV4};

        let account = account_profile();
        let lifecycle = lifecycle_policy();
        let request_profile = general_request_profile_bytes_v1(Action::Freeze).to_vec();
        let transition = transition();
        let effect = effect();
        let certificate = ExecutionStrategyCertificateV2::new(
            id(digest(&account)),
            id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
            id(digest(&request_profile)),
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            id(digest(&effect)),
            ArtifactReleaseIdV1::new([13; 32]).expect("artifact release"),
            id([14; 32]),
            id([15; 32]),
            id([16; 32]),
        )
        .to_bytes()
        .to_vec();
        let admission = ExecutionStrategyAdmissionV2::new(id(digest(&certificate)))
            .to_bytes()
            .to_vec();
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            Some(id(digest(&certificate))),
            id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            Some(id(digest(&admission))),
            id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("admitted strategy")
        .to_bytes()
        .to_vec();
        let capacity = [8; 32];
        let descriptor = CapabilityProgramV4::new(
            id(GENERAL_CAPABILITY_KIND_ID_V1),
            id(GENERAL_CONFIG_SCHEMA_ID_V3),
            id(GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3),
            id(GENERAL_ROOT_SCHEMA_ID_V2),
            id(digest(&lifecycle)),
            id(capacity),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(
                    id(dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID),
                    id(digest(&account)),
                ),
                request_profile: ArtifactReferenceV4::new(
                    id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
                    id(digest(&request_profile)),
                ),
                lifecycle: ArtifactReferenceV4::new(
                    id(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5),
                    id(digest(&lifecycle)),
                ),
                strategy: ArtifactReferenceV4::new(
                    id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
                    id(digest(&strategy)),
                ),
                transition: ArtifactReferenceV4::new(
                    id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
                    id(digest(&transition)),
                ),
                effect: ArtifactReferenceV4::new(
                    id(dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID),
                    id(digest(&effect)),
                ),
            },
            u32::try_from(GENERAL_ROOT_BYTES_V2).expect("root bytes"),
        )
        .expect("descriptor")
        .encode();
        let set = program_set(digest(&descriptor));
        let config = GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: capacity,
            claim_basis_id: [9; 32],
            program_set_id: digest(&set),
            generation: 7,
            price_scale: 1_000,
            collection_slots: 10,
            selection_slots: 10,
            settlement_slots: 10,
            max_orders_per_candidate: 10,
            max_pages_per_candidate: 10,
            continuation_reward_lamports: 1,
            selection_policy_id: [10; 32],
            quote_surplus_beneficiary: [11; 32],
        })
        .expect("legacy config")
        .to_bytes();
        let request = ControllerRequestV2 {
            action: Action::Freeze,
            expected_revision: 7,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            state_bump: 42,
            terminal_record_bump: 0,
        }
        .to_bytes()
        .expect("freeze request");
        Fixture {
            set,
            descriptor,
            config,
            account,
            lifecycle,
            request_profile,
            strategy,
            certificate,
            admission,
            transition,
            effect,
            request,
        }
    }

    #[test]
    fn exact_bundle_joins_at_product_owned_runtime_widths() {
        let fixture = fixture();
        for tail_count in [1_u32, 258] {
            let bundle = authenticate_general_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &fixture.request,
                tail_count,
            )
            .expect("complete joined bundle");
            assert_eq!(bundle.request.action, Action::Freeze);
            assert_eq!(bundle.tail_count, tail_count);
            assert_eq!(
                bundle.request_profile.scalar_count(tail_count),
                Ok(
                    usize::try_from(GENERAL_HOT_COMMON_SCALARS_V3).expect("common scalars")
                        + usize::try_from(tail_count).expect("test tail")
                            * usize::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                                .expect("item stride")
                )
            );
        }
    }

    /// Re-seal a substituted descriptor into a complete, self-consistent graph.
    ///
    /// Substituting one descriptor field moves its digest, so the ProgramSet
    /// must name the new descriptor and the config must name the new set.
    /// Without that the substitution would refuse on a content identity and
    /// never reach the conjunct under test.
    fn resealed(fixture: &Fixture, descriptor: CapabilityProgramV4) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let encoded = descriptor.encode().to_vec();
        let set = program_set(digest(&encoded));
        let existing = GeneralConfigV3::decode(&fixture.config).expect("fixture config");
        let config = GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: existing.capacity_profile_id(),
            claim_basis_id: existing.claim_basis_id(),
            program_set_id: digest(&set),
            generation: existing.generation(),
            price_scale: existing.price_scale(),
            collection_slots: existing.collection_slots(),
            selection_slots: existing.selection_slots(),
            settlement_slots: existing.settlement_slots(),
            max_orders_per_candidate: existing.max_orders_per_candidate(),
            max_pages_per_candidate: existing.max_pages_per_candidate(),
            continuation_reward_lamports: existing.continuation_reward_lamports(),
            selection_policy_id: existing.selection_policy_id(),
            quote_surplus_beneficiary: existing.quote_surplus_beneficiary(),
        })
        .expect("resealed config");
        (set, encoded, config.to_bytes().to_vec())
    }

    /// A General action against a capability that is not General refuses.
    ///
    /// The Hot executor is family-neutral by decision 0006: it proves the
    /// selected kind against the manifest entry and the descriptor and then
    /// runs whatever closure the descriptor names. That makes `validate_descriptor`
    /// the conjunct standing between a General artifact set and a capability of
    /// some other family, and it must not be satisfiable by an identity that is
    /// merely well-formed. Two of the three substitutions below are General's
    /// own real identities in each other's slots -- the strongest form of the
    /// mistake, because every byte is an identity this family publishes.
    #[test]
    fn a_descriptor_of_another_family_refuses_the_general_artifact_set() {
        let fixture = fixture();
        let canonical = CapabilityProgramV4::decode(&fixture.descriptor).expect("fixture");
        for (kind, root_schema) in [
            // General's own root schema standing in the kind slot.
            (GENERAL_ROOT_SCHEMA_ID_V2, GENERAL_ROOT_SCHEMA_ID_V2),
            // and the converse: the kind identity standing in the root slot.
            (GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_CAPABILITY_KIND_ID_V1),
            // and a foreign identity belonging to neither.
            ([0x7c; 32], GENERAL_ROOT_SCHEMA_ID_V2),
        ] {
            let substituted = CapabilityProgramV4::new(
                id(kind),
                canonical.config_schema(),
                canonical.request_schema(),
                id(root_schema),
                canonical.derivation_policy(),
                canonical.capacity_profile(),
                canonical.artifacts(),
                canonical.root_state_bytes(),
            )
            .expect("substituted descriptor");
            let (set, descriptor, config) = resealed(&fixture, substituted);
            let mut artifacts = fixture.artifacts();
            artifacts.program_set = &set;
            artifacts.descriptor = &descriptor;
            artifacts.config = &config;
            assert_eq!(
                authenticate_general_artifacts_v3(
                    GeneralArtifactSelectionV3 {
                        program_set: digest(&set),
                        config: digest(&config),
                        artifact_release: fixture.selection().artifact_release,
                    },
                    artifacts,
                    &fixture.request,
                    258,
                ),
                Err(GeneralArtifactErrorV3::Descriptor),
                "kind {kind:?} root_schema {root_schema:?}"
            );
        }

        // The control: the same reseal with nothing substituted still joins.
        let (set, descriptor, config) = resealed(&fixture, canonical);
        let mut artifacts = fixture.artifacts();
        artifacts.program_set = &set;
        artifacts.descriptor = &descriptor;
        artifacts.config = &config;
        assert!(
            authenticate_general_artifacts_v3(
                GeneralArtifactSelectionV3 {
                    program_set: digest(&set),
                    config: digest(&config),
                    artifact_release: fixture.selection().artifact_release,
                },
                artifacts,
                &fixture.request,
                258,
            )
            .is_ok()
        );
    }

    #[test]
    fn zero_tail_action_and_selected_content_substitution_refuse() {
        let fixture = fixture();
        assert_eq!(
            authenticate_general_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &fixture.request,
                0,
            ),
            Err(GeneralArtifactErrorV3::Geometry)
        );
        let mut selection = fixture.selection();
        selection.config = [0x55; 32];
        assert_eq!(
            authenticate_general_artifacts_v3(
                selection,
                fixture.artifacts(),
                &fixture.request,
                258,
            ),
            Err(GeneralArtifactErrorV3::ContentIdentity)
        );
        let mut request = fixture.request;
        *request.get_mut(10).expect("action byte") = Action::Consider as u8;
        assert!(
            authenticate_general_artifacts_v3(
                fixture.selection(),
                fixture.artifacts(),
                &request,
                258,
            )
            .is_err()
        );

        let mut hostile_profile = account_profile();
        let fixed_accounts = usize::from(
            AccountProfileV2::decode(&hostile_profile)
                .expect("fixture profile")
                .fixed_account_count(),
        );
        let operation = dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed_accounts + 1) * dclutch_account_profile_contract::v2::RULE_BYTES;
        put(
            &mut hostile_profile,
            operation + 2,
            &u16::try_from(HOT_RUNTIME_PRODUCT_COORDINATE_V3)
                .expect("Product coordinate")
                .to_le_bytes(),
        );
        let mut hostile = fixture.artifacts();
        hostile.account_profile = &hostile_profile;
        assert_eq!(
            authenticate_general_artifacts_v3(fixture.selection(), hostile, &fixture.request, 258,),
            Err(GeneralArtifactErrorV3::ContentIdentity)
        );

        let hostile_digest = digest(&hostile_profile);
        let mut descriptor =
            CapabilityProgramV4::decode(&fixture.descriptor).expect("fixture descriptor");
        let mut descriptor_artifacts = descriptor.artifacts();
        descriptor_artifacts.account_profile =
            dclutch_capability_program_contract::v4::ArtifactReferenceV4::new(
                descriptor.account_profile().schema(),
                id(hostile_digest),
            );
        descriptor = CapabilityProgramV4::new(
            descriptor.kind(),
            descriptor.config_schema(),
            descriptor.request_schema(),
            descriptor.root_schema(),
            descriptor.derivation_policy(),
            descriptor.capacity_profile(),
            descriptor_artifacts,
            descriptor.root_state_bytes(),
        )
        .expect("hostile descriptor");
        let hostile_descriptor = descriptor.encode();
        let hostile_set = program_set(digest(&hostile_descriptor));
        let mut hostile_config = GeneralConfigV3::decode(&fixture.config).expect("config");
        hostile_config = GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: hostile_config.capacity_profile_id(),
            claim_basis_id: hostile_config.claim_basis_id(),
            program_set_id: digest(&hostile_set),
            generation: hostile_config.generation(),
            price_scale: hostile_config.price_scale(),
            collection_slots: hostile_config.collection_slots(),
            selection_slots: hostile_config.selection_slots(),
            settlement_slots: hostile_config.settlement_slots(),
            max_orders_per_candidate: hostile_config.max_orders_per_candidate(),
            max_pages_per_candidate: hostile_config.max_pages_per_candidate(),
            continuation_reward_lamports: hostile_config.continuation_reward_lamports(),
            selection_policy_id: hostile_config.selection_policy_id(),
            quote_surplus_beneficiary: hostile_config.quote_surplus_beneficiary(),
        })
        .expect("hostile config");
        let hostile_config_bytes = hostile_config.to_bytes();
        assert_eq!(
            authenticate_general_artifacts_v3(
                GeneralArtifactSelectionV3 {
                    program_set: digest(&hostile_set),
                    config: digest(&hostile_config_bytes),
                    artifact_release: fixture.selection().artifact_release,
                },
                GeneralArtifactBytesV3 {
                    program_set: &hostile_set,
                    descriptor: &hostile_descriptor,
                    config: &hostile_config_bytes,
                    account_profile: &hostile_profile,
                    lifecycle_policy: &fixture.lifecycle,
                    request_profile: &fixture.request_profile,
                    strategy: &fixture.strategy,
                    certificate: &fixture.certificate,
                    admission: &fixture.admission,
                    transition: &fixture.transition,
                    effect: &fixture.effect,
                },
                &fixture.request,
                258,
            ),
            Err(GeneralArtifactErrorV3::Admission)
        );
    }

    #[test]
    fn role_tag_and_one_byte_child_fakes_refuse_admission() {
        let mut fake = effect_for(Action::Collect);
        let request_offset = fake
            .windows(dclutch_claims_svm::affine_batch_v2::AFFINE_BATCH_PLAN_MAGIC_V2.len())
            .position(|window| {
                window == dclutch_claims_svm::affine_batch_v2::AFFINE_BATCH_PLAN_MAGIC_V2
            })
            .expect("canonical affine child request");
        *fake.get_mut(request_offset).expect("one-byte child fake") ^= 1;
        let effect = EffectProgramV3::decode(&fake).expect("structurally valid fake");
        assert_eq!(
            validate_routes(Action::Collect, effect),
            Err(GeneralArtifactErrorV3::Effect)
        );
    }
}
