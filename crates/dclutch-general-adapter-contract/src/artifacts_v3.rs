//! Complete finalized-artifact join for data-defined General execution.
//!
//! `CapabilityProgramSetV1` selects one complete descriptor from the exact
//! action byte in the General request. This module joins that descriptor to its
//! config, AccountProfile, lifecycle policy, RequestProfile, ExecutionStrategy,
//! TransitionVM, and EffectProgram artifacts. It is a release/admission
//! contract, not account or CPI authority; generic Trading remains the only
//! physical executor and writer.

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV3, v2::AccountProfileV2,
};
use dclutch_capability_program_contract::{
    hot_v3::{
        HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
        HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
        HOT_RUNTIME_PRODUCT_COORDINATE_V3, HOT_RUNTIME_ROOT_COORDINATE_V3,
    },
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
    v3::CapabilityProgramV3,
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
use dclutch_custody_contract::{CompartmentV1, CustodyRequestV1, OperationV1};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
};
use dclutch_execution_strategy_contract::v2::{
    AdmittedAotAuthorizationV2, AuthenticatedInterpreterArtifactsV2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
    validate_admitted_aot_v2,
};
use dclutch_general_codec::{Action, CONTROLLER_REQUEST_BYTES, ControllerRequestV1};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    v3::{GENERAL_CONFIG_SCHEMA_ID_V3, GeneralConfigV3},
};
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use dclutch_request_profile_contract::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3, GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
    },
    specialization::general_request_profile_bytes_v1,
};

/// Exact action-selector byte in the canonical 64-byte General request.
pub const GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3: u32 = 10;
/// Schema preimage for the runtime-width General controller request.
pub const GENERAL_CONTROLLER_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/general-controller-request-v2";
/// SHA-256 of [`GENERAL_CONTROLLER_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x3d, 0x55, 0xce, 0xaf, 0x28, 0x96, 0xaa, 0x66, 0xbb, 0x07, 0xf8, 0x4b, 0x71, 0x16, 0x2f, 0xd8,
    0x63, 0x10, 0xa9, 0xa0, 0x2c, 0x35, 0x53, 0x59, 0xe9, 0x39, 0x06, 0xc9, 0x08, 0x64, 0x82, 0x45,
];

/// Common scalar temporarily receiving the Product-authenticated tail width.
pub const GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3: u16 = 10;

/// Exact descriptor-selected finalized bytes for one General action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactBytesV3<'a> {
    /// Canonical action-to-program set.
    pub program_set: &'a [u8],
    /// Action-selected CapabilityProgramV3 descriptor.
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
    pub request: ControllerRequestV1,
    /// Action-selected capability descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Immutable runtime-width General policy.
    pub config: GeneralConfigV3,
    /// Runtime-width account projection.
    pub account_profile: AccountProfileV2<'a>,
    /// Trading-owned lifecycle policy.
    pub lifecycle_policy: StateLifecyclePolicyV3<'a>,
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
    let set = CapabilityProgramSetV1::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| GeneralArtifactErrorV3::ProgramSet)?;
    if set.selector_offset() != GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U8
    {
        return Err(GeneralArtifactErrorV3::ProgramSet);
    }
    let request =
        ControllerRequestV1::decode(family_request).map_err(|_| GeneralArtifactErrorV3::Request)?;
    let selected_descriptor = set
        .select(family_request)
        .map_err(|_| GeneralArtifactErrorV3::ProgramSet)?;
    let descriptor_id = digest(artifacts.descriptor);
    if selected_descriptor.to_bytes() != descriptor_id {
        return Err(GeneralArtifactErrorV3::ContentIdentity);
    }
    let descriptor = CapabilityProgramV3::decode(artifacts.descriptor)
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
        descriptor.account_profile().to_bytes(),
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| GeneralArtifactErrorV3::AccountProfile)?;
    require_content(
        descriptor.derivation_policy().to_bytes(),
        artifacts.lifecycle_policy,
    )?;
    let lifecycle_policy = StateLifecyclePolicyV3::decode_selected(
        descriptor.derivation_policy().to_bytes(),
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
        descriptor.request_profile_program().to_bytes(),
        artifacts.request_profile,
    )?;
    if artifacts.request_profile != general_request_profile_bytes_v1(request.action) {
        return Err(GeneralArtifactErrorV3::RequestProfile);
    }
    let request_profile = RequestProfileV1::decode_selected(
        descriptor.request_profile_program().to_bytes(),
        digest(artifacts.request_profile),
        artifacts.request_profile,
    )
    .map_err(|_| GeneralArtifactErrorV3::RequestProfile)?;
    execute_request_profile(request_profile, family_request, tail_count)?;

    require_content(
        descriptor.transition_program().to_bytes(),
        artifacts.strategy,
    )?;
    let strategy = ExecutionStrategyProgramV2::decode(artifacts.strategy)
        .map_err(|_| GeneralArtifactErrorV3::Strategy)?;
    let strategy_id = content(digest(artifacts.strategy))?;
    strategy
        .validate_descriptor_selection(strategy_id, descriptor)
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
        strategy.transition_program().to_bytes(),
        artifacts.transition,
    )?;
    if strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID {
        return Err(GeneralArtifactErrorV3::Transition);
    }
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| GeneralArtifactErrorV3::Transition)?;

    require_content(descriptor.effect_program().to_bytes(), artifacts.effect)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        digest(artifacts.effect),
        artifacts.effect,
    )
    .map_err(|_| GeneralArtifactErrorV3::Effect)?;
    let admitted_aot = validate_admitted_aot_v2(
        strategy_id,
        strategy,
        descriptor,
        certificate_id,
        certificate,
        AuthenticatedInterpreterArtifactsV2 {
            account_profile_program: descriptor.account_profile(),
            request_profile_schema: descriptor.request_profile_schema(),
            request_profile_program: descriptor.request_profile_program(),
            transition_schema: strategy.transition_schema(),
            transition_program: strategy.transition_program(),
            effect_program: descriptor.effect_program(),
        },
        ArtifactReleaseIdV1::new(selection.artifact_release)
            .map_err(|_| GeneralArtifactErrorV3::Admission)?,
        Some((admission_id, admission)),
    )
    .map_err(|_| GeneralArtifactErrorV3::Admission)?;
    validate_geometry(
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

fn validate_descriptor(descriptor: CapabilityProgramV3) -> Result<()> {
    if descriptor.kind().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != GENERAL_CONFIG_SCHEMA_ID_V3
        || descriptor.request_schema().to_bytes() != GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3
        || descriptor.root_schema().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
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
    const MAX_TEST_SCALARS: usize = GENERAL_HOT_COMMON_SCALARS_V3 as usize;
    const MAX_TEST_IDENTITIES: usize = GENERAL_HOT_COMMON_IDENTITIES_V3 as usize;
    let scalars = usize::from(profile.common_scalar_count());
    let identities = usize::from(profile.common_identity_count());
    if scalars != MAX_TEST_SCALARS || identities != MAX_TEST_IDENTITIES {
        return Err(GeneralArtifactErrorV3::Geometry);
    }
    // RequestProfile has no item operations, so the affine item bank is
    // preserved without allocating it here. Generic Trading executes the full
    // runtime bank after AccountProfile establishes the authenticated width.
    let input_scalars = [0_u64; MAX_TEST_SCALARS];
    let input_identities = [[0_u8; 32]; MAX_TEST_IDENTITIES];
    let mut scratch_scalars = input_scalars;
    let mut scratch_identities = input_identities;
    let mut output_scalars = input_scalars;
    let mut output_identities = input_identities;
    project_atomic(
        profile,
        0,
        request,
        ProjectionRegistersV1 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
    )
    .map_err(|_| GeneralArtifactErrorV3::RequestProfile)
}

fn validate_geometry(
    tail_count: u32,
    request_bytes: usize,
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    validate_hot_account_profile(account)?;
    if request_bytes != CONTROLLER_REQUEST_BYTES
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
        || account.item_account_stride() != 0
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
        Action::Consider | Action::Freeze | Action::InitializeSettlement => {
            require_route_count(effect, 0)
        }
        Action::Collect => {
            require_route_count(effect, 3)?;
            require_position_route(effect, 0, ProtocolPositionActionV2::Admit)?;
            require_affine_route(effect, 1, 2)?;
            require_custody_transfer_route(
                effect,
                2,
                CompartmentV1::External,
                CompartmentV1::Settlement,
            )
        }
        Action::Materialize => {
            require_route_count(effect, 3)?;
            require_position_route(effect, 0, ProtocolPositionActionV2::Admit)?;
            require_affine_route(effect, 1, 1)?;
            // The exact direction is selected from the authenticated
            // complete-set move: Mint is Settlement -> Hoard, Merge is the
            // inverse.  Both use one canonical Transfer template and the
            // admitted EffectProgram patches the two typed compartment bytes.
            require_custody_transfer_route_either(
                effect,
                2,
                (CompartmentV1::Settlement, CompartmentV1::HoardPrincipal),
                (CompartmentV1::HoardPrincipal, CompartmentV1::Settlement),
            )
        }
        Action::Distribute => {
            require_route_count(effect, 3)?;
            require_affine_route(effect, 0, 2)?;
            require_position_route(effect, 1, ProtocolPositionActionV2::Close)?;
            require_custody_transfer_route(
                effect,
                2,
                CompartmentV1::Settlement,
                CompartmentV1::External,
            )
        }
        Action::Close => {
            require_route_count(effect, 1)?;
            require_custody_transfer_route(
                effect,
                0,
                CompartmentV1::Settlement,
                CompartmentV1::External,
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
        || request.operation != OperationV1::Transfer
        || request.source_compartment != source
        || request.destination_compartment != destination
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

    use dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES;
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
        descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
        config: [u8; GENERAL_CONFIG_BYTES_V3],
        account: Vec<u8>,
        lifecycle: Vec<u8>,
        request_profile: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        request: [u8; CONTROLLER_REQUEST_BYTES],
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

    fn set_byte(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("nonzero fixture identity")
    }

    fn account_profile() -> Vec<u8> {
        use dclutch_account_profile_contract::v2::{
            HEADER_BYTES, OPERATION_BYTES, RULE_BYTES, SELECTED_WINDOW_ARTIFACT_PROFILE,
        };

        const FIXED_ACCOUNTS: usize = HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3;
        let operations = 1_usize;
        let mut output =
            vec![0_u8; HEADER_BYTES + FIXED_ACCOUNTS * RULE_BYTES + operations * OPERATION_BYTES];
        put(&mut output, 0, &dclutch_account_profile_contract::v2::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_account_profile_contract::v2::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &SELECTED_WINDOW_ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(
            &mut output,
            12,
            &u16::try_from(FIXED_ACCOUNTS)
                .expect("fixed accounts")
                .to_le_bytes(),
        );
        put(&mut output, 16, &1_u16.to_le_bytes());
        put(
            &mut output,
            20,
            &u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
                .expect("common scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            22,
            &u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .expect("item scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            24,
            &u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .expect("common identities")
                .to_le_bytes(),
        );
        for (coordinate, privileges, base, stride) in [
            (
                HOT_RUNTIME_ROOT_COORDINATE_V3,
                0x02,
                dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                    + GENERAL_ROOT_BYTES_V2,
                0,
            ),
            (
                HOT_RUNTIME_CONFIG_COORDINATE_V3,
                0,
                dclutch_general_config_contract::v3::GENERAL_CONFIG_BYTES_V3,
                0,
            ),
            (
                HOT_RUNTIME_PRODUCT_COORDINATE_V3,
                0,
                PRODUCT_RECORD_BYTES_V2,
                0,
            ),
            (
                HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
                0,
                PORTFOLIO_HEADER_BYTES,
                PORTFOLIO_COEFFICIENT_BYTES,
            ),
            (HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3, 0, 256, 0),
        ] {
            let rule = HEADER_BYTES + coordinate * RULE_BYTES;
            set_byte(&mut output, rule, privileges);
            put(
                &mut output,
                rule + 8,
                &u32::try_from(base).expect("base width").to_le_bytes(),
            );
            put(
                &mut output,
                rule + 12,
                &u32::try_from(stride).expect("stride width").to_le_bytes(),
            );
        }
        let operation = HEADER_BYTES + FIXED_ACCOUNTS * RULE_BYTES;
        set_byte(&mut output, operation, 8);
        put(
            &mut output,
            operation + 2,
            &u16::try_from(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3)
                .expect("portfolio coordinate")
                .to_le_bytes(),
        );
        put(
            &mut output,
            operation + 6,
            &GENERAL_PRODUCT_TAIL_COUNT_SCALAR_V3.to_le_bytes(),
        );
        put(
            &mut output,
            operation + 8,
            &u32::try_from(PORTFOLIO_COEFFICIENT_COUNT_OFFSET)
                .expect("portfolio count offset")
                .to_le_bytes(),
        );
        output
    }

    fn lifecycle_policy() -> Vec<u8> {
        lifecycle_policy_for(Action::Freeze)
    }

    fn lifecycle_policy_for(action: Action) -> Vec<u8> {
        use dclutch_account_profile_contract::lifecycle_v3::{
            ACTION_PLAN_BYTES, ARTIFACT_PROFILE, HEADER_BYTES, MAGIC, RECIPE_BYTES, SEED_BYTES,
            VERSION,
        };

        let mut output = vec![0_u8; HEADER_BYTES + RECIPE_BYTES + SEED_BYTES + ACTION_PLAN_BYTES];
        put(&mut output, 0, &MAGIC);
        put(&mut output, 8, &VERSION.to_le_bytes());
        put(&mut output, 10, &ARTIFACT_PROFILE.to_le_bytes());
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 14, &1_u16.to_le_bytes());
        put(&mut output, 16, &1_u16.to_le_bytes());
        let recipe = HEADER_BYTES;
        set_byte(&mut output, recipe + 6, 1);
        put(
            &mut output,
            recipe + 8,
            &u32::try_from(GENERAL_ROOT_BYTES_V2)
                .expect("root bytes")
                .to_le_bytes(),
        );
        let seed = HEADER_BYTES + RECIPE_BYTES;
        set_byte(&mut output, seed, 3);
        set_byte(&mut output, seed + 1, 1);
        let plan = HEADER_BYTES + RECIPE_BYTES + SEED_BYTES;
        put(&mut output, plan, &(action as u32).to_le_bytes());
        set_byte(&mut output, plan + 4, 2);
        set_byte(&mut output, plan + 8, u8::MAX);
        output
    }

    fn transition() -> Vec<u8> {
        let mut output = vec![0_u8; 56];
        put(&mut output, 0, &dclutch_transition_vm::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_transition_vm::v3::VERSION);
        put(&mut output, 6, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
                .expect("common scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            14,
            &u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .expect("item scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            16,
            &u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .expect("common identities")
                .to_le_bytes(),
        );
        // One canonical LoadConst into an otherwise preserved common register.
        set_byte(&mut output, dclutch_transition_vm::v3::HEADER_BYTES, 0);
        put(
            &mut output,
            dclutch_transition_vm::v3::HEADER_BYTES + 2,
            &10_u16.to_le_bytes(),
        );
        output
    }

    fn effect() -> Vec<u8> {
        effect_for(Action::Freeze)
    }

    fn effect_for(action: Action) -> Vec<u8> {
        let settlement = matches!(
            action,
            Action::Collect | Action::Materialize | Action::Distribute | Action::Close
        );
        let route_count = if settlement { 2_u16 } else { 0 };
        let route_bytes = usize::from(route_count) * dclutch_effect_kernel::v3::ROUTE_BYTES;
        let request_bytes = usize::from(route_count);
        let mut output = vec![0_u8; dclutch_effect_kernel::v3::HEADER_BYTES];
        output.resize(
            dclutch_effect_kernel::v3::HEADER_BYTES + route_bytes + request_bytes,
            0,
        );
        put(&mut output, 0, &dclutch_effect_kernel::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_effect_kernel::v3::VERSION);
        put(&mut output, 6, &route_count.to_le_bytes());
        put(
            &mut output,
            12,
            &u16::try_from(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
                .expect("fixed account count")
                .to_le_bytes(),
        );
        put(
            &mut output,
            16,
            &u16::try_from(GENERAL_HOT_COMMON_SCALARS_V3)
                .expect("common scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            18,
            &u16::try_from(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .expect("item scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            20,
            &u16::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3)
                .expect("common identities")
                .to_le_bytes(),
        );
        if settlement {
            for (index, role) in [1_u8, 4].into_iter().enumerate() {
                let route = dclutch_effect_kernel::v3::HEADER_BYTES
                    + index * dclutch_effect_kernel::v3::ROUTE_BYTES;
                set_byte(&mut output, route, role);
                put(
                    &mut output,
                    route + 6,
                    &u16::try_from(index).expect("route account").to_le_bytes(),
                );
                put(&mut output, route + 8, &1_u16.to_le_bytes());
                put(&mut output, route + 16, &1_u32.to_le_bytes());
            }
        }
        output
    }

    fn program_set(descriptor: [u8; 32]) -> Vec<u8> {
        let mut output = vec![0_u8; 72];
        put(&mut output, 0, b"DCLTCPS1");
        put(&mut output, 8, &1_u16.to_le_bytes());
        put(&mut output, 10, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        set_byte(&mut output, 16, 1);
        put(&mut output, 18, &1_u16.to_le_bytes());
        put(&mut output, 32, &(Action::Freeze as u32).to_le_bytes());
        put(&mut output, 36, &descriptor);
        output
    }

    fn fixture() -> Fixture {
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
        let descriptor = CapabilityProgramV3::new(
            id(GENERAL_CAPABILITY_KIND_ID_V1),
            id(GENERAL_CONFIG_SCHEMA_ID_V3),
            id(GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3),
            id(GENERAL_ROOT_SCHEMA_ID_V2),
            id(digest(&account)),
            id(digest(&lifecycle)),
            id(capacity),
            id(digest(&effect)),
            id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
            id(digest(&request_profile)),
            id(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
            id(digest(&strategy)),
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
        let request = ControllerRequestV1 {
            action: Action::Freeze,
            expected_revision: 7,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
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
        let operation = dclutch_account_profile_contract::v2::HEADER_BYTES
            + HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
                * dclutch_account_profile_contract::v2::RULE_BYTES;
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
            CapabilityProgramV3::decode(&fixture.descriptor).expect("fixture descriptor");
        descriptor = CapabilityProgramV3::new(
            descriptor.kind(),
            descriptor.config_schema(),
            descriptor.request_schema(),
            descriptor.root_schema(),
            id(hostile_digest),
            descriptor.derivation_policy(),
            descriptor.capacity_profile(),
            descriptor.effect_program(),
            descriptor.request_profile_schema(),
            descriptor.request_profile_program(),
            descriptor.transition_schema(),
            descriptor.transition_program(),
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
        let fake = effect_for(Action::Collect);
        let effect = EffectProgramV3::decode(&fake).expect("structurally valid fake");
        assert_eq!(
            validate_routes(Action::Collect, effect),
            Err(GeneralArtifactErrorV3::Effect)
        );
    }
}
