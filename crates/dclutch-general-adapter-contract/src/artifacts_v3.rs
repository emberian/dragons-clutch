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
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
    v3::CapabilityProgramV3,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{v2::FixedRole, v3::ProgramV3 as EffectProgramV3};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyProgramV2,
};
use dclutch_general_codec::{Action, CONTROLLER_REQUEST_BYTES, ControllerRequestV1};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_CONFIG_SCHEMA_ID_V2, GENERAL_ROOT_BYTES_V2,
    GENERAL_ROOT_SCHEMA_ID_V2, GeneralConfigV2,
};
use dclutch_request_profile_contract::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    runtime_candidate::{
        GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2, GENERAL_SETTLEMENT_COMMON_SCALARS_V2,
        GENERAL_SETTLEMENT_ITEM_IDENTITY_STRIDE_V2, GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2,
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
}

/// Complete borrowed artifact bundle after every content and geometry join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralArtifactBundleV3<'a> {
    /// Exact decoded family request.
    pub request: ControllerRequestV1,
    /// Action-selected capability descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Immutable General policy. Its legacy physical width is not authority.
    pub config: GeneralConfigV2,
    /// Runtime-width account projection.
    pub account_profile: AccountProfileV2<'a>,
    /// Trading-owned lifecycle policy.
    pub lifecycle_policy: StateLifecyclePolicyV3<'a>,
    /// Exact action-specific request program.
    pub request_profile: RequestProfileV1<'a>,
    /// Exact execution strategy.
    pub strategy: ExecutionStrategyProgramV2,
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
/// `tail_count` comes from the authenticated Product result domain. The legacy
/// provisional `GeneralConfigV2.outcome_count` is intentionally not consulted:
/// it is not allowed to reintroduce the prototype's `N <= 16` semantic cap.
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
        GeneralConfigV2::decode(artifacts.config).map_err(|_| GeneralArtifactErrorV3::Config)?;
    if config.capability_program_id() != selection.program_set
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
        transition,
        effect,
        tail_count,
    })
}

fn validate_descriptor(descriptor: CapabilityProgramV3) -> Result<()> {
    if descriptor.kind().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != GENERAL_CONFIG_SCHEMA_ID_V2
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
    const MAX_TEST_SCALARS: usize = 11;
    const MAX_TEST_IDENTITIES: usize = 4;
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
    if request_bytes != CONTROLLER_REQUEST_BYTES
        || request.item_request_bytes() != 0
        || account.common_scalar_count()
            != u16::try_from(GENERAL_SETTLEMENT_COMMON_SCALARS_V2)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.item_scalar_stride()
            != u16::try_from(GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.common_identity_count()
            != u16::try_from(GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2)
                .map_err(|_| GeneralArtifactErrorV3::Geometry)?
        || account.item_identity_stride()
            != u16::try_from(GENERAL_SETTLEMENT_ITEM_IDENTITY_STRIDE_V2)
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

fn validate_routes(action: Action, effect: EffectProgramV3<'_>) -> Result<()> {
    let settlement = matches!(
        action,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close
    );
    let expected = if settlement { 2 } else { 0 };
    if effect.route_count() != expected {
        return Err(GeneralArtifactErrorV3::Effect);
    }
    for (index, role) in [(0_u16, FixedRole::Claims), (1_u16, FixedRole::Custody)] {
        if settlement
            && effect
                .route(index)
                .map_err(|_| GeneralArtifactErrorV3::Effect)?
                .role()
                != role
        {
            return Err(GeneralArtifactErrorV3::Effect);
        }
    }
    Ok(())
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
    use dclutch_general_config_contract::{GENERAL_CONFIG_BYTES_V2, GeneralConfigV2Input};
    use std::{vec, vec::Vec};

    use super::*;

    struct Fixture {
        set: Vec<u8>,
        descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
        config: [u8; GENERAL_CONFIG_BYTES_V2],
        account: Vec<u8>,
        lifecycle: Vec<u8>,
        request_profile: Vec<u8>,
        strategy: Vec<u8>,
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
                transition: &self.transition,
                effect: &self.effect,
            }
        }

        fn selection(&self) -> GeneralArtifactSelectionV3 {
            GeneralArtifactSelectionV3 {
                program_set: digest(&self.set),
                config: digest(&self.config),
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
        let mut output = vec![0_u8; 64];
        put(&mut output, 0, &dclutch_account_profile_contract::v2::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_account_profile_contract::v2::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_account_profile_contract::v2::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 16, &1_u16.to_le_bytes());
        put(
            &mut output,
            20,
            &u16::try_from(GENERAL_SETTLEMENT_COMMON_SCALARS_V2)
                .expect("common scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            22,
            &u16::try_from(GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2)
                .expect("item scalars")
                .to_le_bytes(),
        );
        put(
            &mut output,
            24,
            &u16::try_from(GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2)
                .expect("common identities")
                .to_le_bytes(),
        );
        // The one fixed Product observation owns the authenticated runtime
        // outcome count. A nonzero affine register geometry is canonical only
        // when exactly one tail-count projection establishes that authority.
        put(&mut output, 40, &4_u32.to_le_bytes());
        set_byte(&mut output, 48, 8);
        put(&mut output, 54, &10_u16.to_le_bytes());
        output
    }

    fn lifecycle_policy() -> Vec<u8> {
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
        put(&mut output, plan, &(Action::Freeze as u32).to_le_bytes());
        set_byte(&mut output, plan + 4, 2);
        set_byte(&mut output, plan + 8, u8::MAX);
        output
    }

    fn transition() -> Vec<u8> {
        let mut output = vec![0_u8; 56];
        put(&mut output, 0, &dclutch_transition_vm::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_transition_vm::v3::VERSION);
        put(&mut output, 6, &1_u16.to_le_bytes());
        put(&mut output, 12, &11_u16.to_le_bytes());
        put(&mut output, 14, &1_u16.to_le_bytes());
        put(&mut output, 16, &4_u16.to_le_bytes());
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
        let mut output = vec![0_u8; dclutch_effect_kernel::v3::HEADER_BYTES];
        put(&mut output, 0, &dclutch_effect_kernel::v3::MAGIC);
        set_byte(&mut output, 4, dclutch_effect_kernel::v3::VERSION);
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 16, &11_u16.to_le_bytes());
        put(&mut output, 18, &1_u16.to_le_bytes());
        put(&mut output, 20, &4_u16.to_le_bytes());
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
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::Interpreted,
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            id(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            None,
            id(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            None,
            id(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            id(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("interpreted strategy")
        .to_bytes()
        .to_vec();
        let effect = effect();
        let capacity = [8; 32];
        let descriptor = CapabilityProgramV3::new(
            id(GENERAL_CAPABILITY_KIND_ID_V1),
            id(GENERAL_CONFIG_SCHEMA_ID_V2),
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
        let config = GeneralConfigV2::new(GeneralConfigV2Input {
            capacity_profile_id: capacity,
            claim_basis_id: [9; 32],
            capability_program_id: digest(&set),
            generation: 7,
            price_scale: 1_000,
            collection_slots: 10,
            selection_slots: 10,
            settlement_slots: 10,
            max_orders_per_candidate: 10,
            max_pages_per_candidate: 10,
            continuation_reward_lamports: 1,
            selection_policy_id: [10; 32],
            outcome_count: 2,
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
            transition,
            effect,
            request,
        }
    }

    #[test]
    fn exact_bundle_joins_at_runtime_width_beyond_legacy_config_cap() {
        let fixture = fixture();
        let bundle = authenticate_general_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &fixture.request,
            258,
        )
        .expect("complete joined bundle");
        assert_eq!(bundle.request.action, Action::Freeze);
        assert_eq!(bundle.tail_count, 258);
        assert_eq!(bundle.config.outcome_count(), 2);
        assert_eq!(bundle.request_profile.scalar_count(258), Ok(269));
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
    }
}
