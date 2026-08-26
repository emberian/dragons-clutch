//! Exact finalized-artifact joins for data-driven Direct execution.
//!
//! A `CapabilityProgramSetV1` minimally selects one full
//! `CapabilityProgramV3` by the action in [`DirectExecutionRequestV3`]. This
//! module then joins every independently finalized artifact by its SHA-256
//! content identity, hostile-decodes all four interpreters, and requires one
//! coherent runtime-width register/account geometry. It does not authorize an
//! AOT-only path: Trading still executes the selected Transition and Effect
//! programs and commits once after fixed-role receipts.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::{
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
    v3::CapabilityProgramV3,
};
use dclutch_effect_kernel::{v2::FixedRole, v3::ProgramV3 as EffectProgramV3};
use dclutch_request_profile_contract::{
    RequestProfileV1,
    v2::{REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2},
};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
        DIRECT_SUCCESSOR_KIND_ID_V3, DirectExecutionActionV3, DirectExecutionRequestV3,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_MAKER_REPLAY_DERIVATION_ID_V1,
        DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
    },
};

/// Exact descriptor-selected raw finalized artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectArtifactBytesV3<'a> {
    /// Canonical action-to-program set bytes.
    pub program_set: &'a [u8],
    /// Selected fixed CapabilityProgramV3 descriptor bytes.
    pub descriptor: &'a [u8],
    /// Immutable Direct execution config bytes.
    pub config: &'a [u8],
    /// Runtime-tail account profile bytes.
    pub account_profile: &'a [u8],
    /// Exact family request profile bytes.
    pub request_profile: &'a [u8],
    /// Runtime-tail transition program bytes.
    pub transition: &'a [u8],
    /// Fixed-role effect program bytes.
    pub effect: &'a [u8],
}

/// External immutable content selections already authenticated from records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectArtifactSelectionV3 {
    /// Capability release selected by the manifest; this is the ProgramSet ID.
    pub program_set: [u8; 32],
    /// Config content selected by the manifest entry.
    pub config: [u8; 32],
}

/// Stable artifact-selection or geometry refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectArtifactErrorV3 {
    /// A selected content identity was zero or its raw bytes hashed differently.
    ContentIdentity,
    /// ProgramSet selector geometry was not offset-12 canonical `u32` LE.
    ProgramSet,
    /// Direct request or selected action refused.
    Request,
    /// Descriptor selected another kind/config/root/request/derivation schema.
    Descriptor,
    /// Immutable Direct economics refused.
    Config,
    /// AccountProfile hostile decode refused.
    AccountProfile,
    /// RequestProfile hostile decode or request width refused.
    RequestProfile,
    /// Transition hostile decode refused.
    Transition,
    /// Effect hostile decode or role admission refused.
    Effect,
    /// Account/register/request affine geometry differed across artifacts.
    Geometry,
}

/// Result alias for Direct V3 artifact joins.
pub type Result<T> = core::result::Result<T, DirectArtifactErrorV3>;

/// Fully joined borrowed Direct artifact bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectArtifactBundleV3<'a> {
    /// Selected action from the exact family request.
    pub action: DirectExecutionActionV3,
    /// Selected full descriptor.
    pub descriptor: CapabilityProgramV3,
    /// Immutable price and fee policy.
    pub config: DirectExecutionConfigV1,
    /// Exact runtime-tail account interpreter.
    pub account_profile: AccountProfileV2<'a>,
    /// Exact request interpreter.
    pub request_profile: DirectRequestProfileV3<'a>,
    /// Exact transition interpreter.
    pub transition: TransitionProgramV3<'a>,
    /// Exact fixed-role effect interpreter.
    pub effect: EffectProgramV3<'a>,
}

/// Action-checked request projection and signature-evidence profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRequestProfileV3<'a> {
    /// Permissionless or matcher-selected action with no maker signature.
    Unsigned(RequestProfileV1<'a>),
    /// Maker-authorized action with nonempty native-Ed25519 requirements.
    Signed(RequestProfileV2<'a>),
}

impl<'a> DirectRequestProfileV3<'a> {
    /// Exact embedded RequestProfile V1 used for geometry and projection.
    pub const fn request_profile(self) -> RequestProfileV1<'a> {
        match self {
            Self::Unsigned(profile) => profile,
            Self::Signed(profile) => profile.request_profile(),
        }
    }

    /// Whether this descriptor requires adjacent native-Ed25519 evidence.
    pub const fn requires_native_signature(self) -> bool {
        matches!(self, Self::Signed(_))
    }
}

/// Authenticate and join one complete action bundle.
pub fn authenticate_direct_artifacts_v3<'a>(
    selection: DirectArtifactSelectionV3,
    artifacts: DirectArtifactBytesV3<'a>,
    request: &'a [u8],
    tail_count: u32,
) -> Result<DirectArtifactBundleV3<'a>> {
    require_selected(selection.program_set, artifacts.program_set)?;
    let set = CapabilityProgramSetV1::decode_selected(
        selection.program_set,
        digest(artifacts.program_set),
        artifacts.program_set,
    )
    .map_err(|_| DirectArtifactErrorV3::ProgramSet)?;
    if set.selector_offset() != DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U32
    {
        return Err(DirectArtifactErrorV3::ProgramSet);
    }
    let semantic_request = DirectExecutionRequestV3::decode(request, tail_count)
        .map_err(|_| DirectArtifactErrorV3::Request)?;
    let selected_descriptor = set
        .select(request)
        .map_err(|_| DirectArtifactErrorV3::ProgramSet)?;
    if selected_descriptor.to_bytes() != digest(artifacts.descriptor) {
        return Err(DirectArtifactErrorV3::ContentIdentity);
    }
    let descriptor = CapabilityProgramV3::decode(artifacts.descriptor)
        .map_err(|_| DirectArtifactErrorV3::Descriptor)?;
    if descriptor.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || descriptor.request_schema().to_bytes() != DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
        || descriptor.derivation_policy().to_bytes() != DIRECT_MAKER_REPLAY_DERIVATION_ID_V1
        || descriptor.root_state_bytes()
            != u32::try_from(DIRECT_ROOT_STATE_BYTES_V1)
                .map_err(|_| DirectArtifactErrorV3::Geometry)?
        || descriptor.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
    {
        return Err(DirectArtifactErrorV3::Descriptor);
    }

    require_selected(selection.config, artifacts.config)?;
    let config = DirectExecutionConfigV1::decode_selected(
        selection.config,
        digest(artifacts.config),
        artifacts.config,
    )
    .map_err(|_| DirectArtifactErrorV3::Config)?;
    require_content(
        descriptor.account_profile().to_bytes(),
        artifacts.account_profile,
    )?;
    let account_profile = AccountProfileV2::decode(artifacts.account_profile)
        .map_err(|_| DirectArtifactErrorV3::AccountProfile)?;
    require_content(
        descriptor.request_profile_program().to_bytes(),
        artifacts.request_profile,
    )?;
    let request_profile = decode_request_profile(
        semantic_request.action(),
        descriptor,
        artifacts.request_profile,
    )?;
    require_content(
        descriptor.transition_program().to_bytes(),
        artifacts.transition,
    )?;
    let transition = TransitionProgramV3::decode(artifacts.transition)
        .map_err(|_| DirectArtifactErrorV3::Transition)?;
    require_content(descriptor.effect_program().to_bytes(), artifacts.effect)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        digest(artifacts.effect),
        artifacts.effect,
    )
    .map_err(|_| DirectArtifactErrorV3::Effect)?;

    validate_geometry(
        semantic_request.action(),
        tail_count,
        request.len(),
        account_profile,
        request_profile.request_profile(),
        transition,
        effect,
    )?;
    Ok(DirectArtifactBundleV3 {
        action: semantic_request.action(),
        descriptor,
        config,
        account_profile,
        request_profile,
        transition,
        effect,
    })
}

fn decode_request_profile<'a>(
    action: DirectExecutionActionV3,
    descriptor: CapabilityProgramV3,
    bytes: &'a [u8],
) -> Result<DirectRequestProfileV3<'a>> {
    let selected = descriptor.request_profile_program().to_bytes();
    let authenticated = digest(bytes);
    if action_requires_native_signature(action) {
        if descriptor.request_profile_schema().to_bytes() != REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID {
            return Err(DirectArtifactErrorV3::Descriptor);
        }
        RequestProfileV2::decode_selected(selected, authenticated, bytes)
            .map(DirectRequestProfileV3::Signed)
            .map_err(|_| DirectArtifactErrorV3::RequestProfile)
    } else {
        if descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        {
            return Err(DirectArtifactErrorV3::Descriptor);
        }
        RequestProfileV1::decode_selected(selected, authenticated, bytes)
            .map(DirectRequestProfileV3::Unsigned)
            .map_err(|_| DirectArtifactErrorV3::RequestProfile)
    }
}

const fn action_requires_native_signature(action: DirectExecutionActionV3) -> bool {
    matches!(
        action,
        DirectExecutionActionV3::InlineOrdinary
            | DirectExecutionActionV3::RegisterSell
            | DirectExecutionActionV3::RegisterBuy
            | DirectExecutionActionV3::CancelRegistered
            | DirectExecutionActionV3::CancelThrough
            | DirectExecutionActionV3::SplitInline
            | DirectExecutionActionV3::MergeInline
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_geometry(
    action: DirectExecutionActionV3,
    tail_count: u32,
    request_bytes: usize,
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<()> {
    if request
        .request_bytes(tail_count)
        .map_err(|_| DirectArtifactErrorV3::RequestProfile)?
        != request_bytes
        || account.common_scalar_count() != request.common_scalar_count()
        || account.item_scalar_stride() != request.item_scalar_stride()
        || account.common_identity_count() != request.common_identity_count()
        || account.item_identity_stride() != request.item_identity_stride()
        || account.common_scalar_count() != transition.common_scalar_count()
        || account.item_scalar_stride() != transition.item_scalar_stride()
        || account.common_identity_count() != transition.common_identity_count()
        || account.item_identity_stride() != transition.item_identity_stride()
        || account.common_scalar_count() != effect.common_scalar_count()
        || account.item_scalar_stride() != effect.item_scalar_stride()
        || account.common_identity_count() != effect.common_identity_count()
        || account.item_identity_stride() != effect.item_identity_stride()
        || account.fixed_account_count() != effect.fixed_account_count()
        || account.item_account_stride() != effect.item_account_stride()
    {
        return Err(DirectArtifactErrorV3::Geometry);
    }
    let complementary = matches!(
        action,
        DirectExecutionActionV3::SplitRegistered
            | DirectExecutionActionV3::MergeRegistered
            | DirectExecutionActionV3::SplitInline
            | DirectExecutionActionV3::MergeInline
    );
    if complementary
        != (account.item_account_stride() != 0
            && transition.item_scalar_stride() != 0
            && effect.item_account_stride() != 0)
    {
        return Err(DirectArtifactErrorV3::Geometry);
    }
    let mut route = 0_u16;
    while route < effect.route_count() {
        let role = effect
            .route(route)
            .map_err(|_| DirectArtifactErrorV3::Effect)?
            .role();
        if !matches!(role, FixedRole::Claims | FixedRole::Custody) {
            return Err(DirectArtifactErrorV3::Effect);
        }
        route = route
            .checked_add(1)
            .ok_or(DirectArtifactErrorV3::Geometry)?;
    }
    Ok(())
}

fn require_selected(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    if selected == [0; 32] || selected != digest(bytes) {
        Err(DirectArtifactErrorV3::ContentIdentity)
    } else {
        Ok(())
    }
}

fn require_content(selected: [u8; 32], bytes: &[u8]) -> Result<()> {
    require_selected(selected, bytes)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_capability_program_contract::v3::CAPABILITY_PROGRAM_V3_BYTES;
    use dclutch_core_contract::ContentId;
    use std::{vec, vec::Vec};

    use super::*;
    use crate::execution_v3::{DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, encode_header_v3};

    struct Fixture {
        set: Vec<u8>,
        descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
        config: [u8; crate::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1],
        account: Vec<u8>,
        request_profile: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        request: [u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3],
    }

    impl Fixture {
        fn artifacts(&self) -> DirectArtifactBytesV3<'_> {
            DirectArtifactBytesV3 {
                program_set: &self.set,
                descriptor: &self.descriptor,
                config: &self.config,
                account_profile: &self.account,
                request_profile: &self.request_profile,
                transition: &self.transition,
                effect: &self.effect,
            }
        }

        fn selection(&self) -> DirectArtifactSelectionV3 {
            DirectArtifactSelectionV3 {
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

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("nonzero fixture identity")
    }

    fn account_profile() -> Vec<u8> {
        let mut output = vec![0_u8; 48];
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
        put(&mut output, 20, &1_u16.to_le_bytes());
        output
    }

    fn request_profile() -> Vec<u8> {
        let mut output = vec![0_u8; 56];
        put(&mut output, 0, &dclutch_request_profile_contract::MAGIC);
        put(
            &mut output,
            8,
            &dclutch_request_profile_contract::VERSION.to_le_bytes(),
        );
        put(
            &mut output,
            10,
            &dclutch_request_profile_contract::ARTIFACT_PROFILE.to_le_bytes(),
        );
        put(
            &mut output,
            12,
            &u32::try_from(DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3)
                .expect("request width")
                .to_le_bytes(),
        );
        put(&mut output, 20, &1_u16.to_le_bytes());
        put(&mut output, 24, &1_u16.to_le_bytes());
        *output.get_mut(32).expect("request opcode") = 2;
        put(&mut output, 36, &12_u32.to_le_bytes());
        put(
            &mut output,
            44,
            &(DirectExecutionActionV3::CloseDirectRoot as u64).to_le_bytes(),
        );
        output
    }

    fn transition() -> Vec<u8> {
        let mut output = vec![0_u8; 56];
        put(&mut output, 0, &dclutch_transition_vm::v3::MAGIC);
        *output.get_mut(4).expect("transition version") = dclutch_transition_vm::v3::VERSION;
        put(&mut output, 6, &1_u16.to_le_bytes());
        put(&mut output, 12, &1_u16.to_le_bytes());
        output
    }

    fn effect() -> Vec<u8> {
        let mut output = vec![0_u8; 32];
        put(&mut output, 0, &dclutch_effect_kernel::v3::MAGIC);
        *output.get_mut(4).expect("effect version") = dclutch_effect_kernel::v3::VERSION;
        put(&mut output, 12, &1_u16.to_le_bytes());
        put(&mut output, 16, &1_u16.to_le_bytes());
        output
    }

    fn program_set(action: DirectExecutionActionV3, descriptor: [u8; 32]) -> Vec<u8> {
        let mut output = vec![0_u8; 72];
        put(&mut output, 0, b"DCLTCPS1");
        put(&mut output, 8, &1_u16.to_le_bytes());
        put(&mut output, 10, &1_u16.to_le_bytes());
        put(
            &mut output,
            12,
            &DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3.to_le_bytes(),
        );
        *output.get_mut(16).expect("selector width") = 4;
        put(&mut output, 18, &1_u16.to_le_bytes());
        put(&mut output, 32, &(action as u32).to_le_bytes());
        put(&mut output, 36, &descriptor);
        output
    }

    fn fixture() -> Fixture {
        let account = account_profile();
        let request_profile = request_profile();
        let transition = transition();
        let effect = effect();
        let config = DirectExecutionConfigV1::new(1_000, 25, [7; 32])
            .expect("config")
            .encode();
        let descriptor = CapabilityProgramV3::new(
            id(DIRECT_SUCCESSOR_KIND_ID_V3),
            id(DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1),
            id(DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3),
            id(DIRECT_ROOT_SCHEMA_ID_V1),
            id(digest(&account)),
            id(DIRECT_MAKER_REPLAY_DERIVATION_ID_V1),
            id([8; 32]),
            id(digest(&effect)),
            id(dclutch_request_profile_contract::SCHEMA_RELEASE_ID),
            id(digest(&request_profile)),
            id(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID),
            id(digest(&transition)),
            u32::try_from(DIRECT_ROOT_STATE_BYTES_V1).expect("root width"),
        )
        .expect("descriptor")
        .encode();
        let set = program_set(
            DirectExecutionActionV3::CloseDirectRoot,
            digest(&descriptor),
        );
        let mut request = [0_u8; DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3];
        encode_header_v3(DirectExecutionActionV3::CloseDirectRoot, &mut request).expect("request");
        Fixture {
            set,
            descriptor,
            config,
            account,
            request_profile,
            transition,
            effect,
            request,
        }
    }

    #[test]
    fn exact_unsigned_bundle_joins_all_finalized_content() {
        let fixture = fixture();
        let joined = authenticate_direct_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &fixture.request,
            0,
        )
        .expect("complete joined bundle");
        assert_eq!(joined.action, DirectExecutionActionV3::CloseDirectRoot);
        assert!(!joined.request_profile.requires_native_signature());
    }

    #[test]
    fn program_set_descriptor_config_and_profile_substitution_refuse() {
        let fixture = fixture();
        let mut wrong_selection = fixture.selection();
        wrong_selection.program_set[0] ^= 1;
        assert_eq!(
            authenticate_direct_artifacts_v3(
                wrong_selection,
                fixture.artifacts(),
                &fixture.request,
                0,
            ),
            Err(DirectArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_descriptor = fixture.descriptor;
        *wrong_descriptor.get_mut(64).expect("descriptor mutation") ^= 1;
        assert_eq!(
            authenticate_direct_artifacts_v3(
                fixture.selection(),
                DirectArtifactBytesV3 {
                    descriptor: &wrong_descriptor,
                    ..fixture.artifacts()
                },
                &fixture.request,
                0,
            ),
            Err(DirectArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_config = fixture.config;
        *wrong_config.get_mut(16).expect("config mutation") ^= 1;
        assert_eq!(
            authenticate_direct_artifacts_v3(
                fixture.selection(),
                DirectArtifactBytesV3 {
                    config: &wrong_config,
                    ..fixture.artifacts()
                },
                &fixture.request,
                0,
            ),
            Err(DirectArtifactErrorV3::ContentIdentity)
        );

        let mut wrong_profile = fixture.account.clone();
        *wrong_profile.get_mut(12).expect("profile mutation") ^= 1;
        assert_eq!(
            authenticate_direct_artifacts_v3(
                fixture.selection(),
                DirectArtifactBytesV3 {
                    account_profile: &wrong_profile,
                    ..fixture.artifacts()
                },
                &fixture.request,
                0,
            ),
            Err(DirectArtifactErrorV3::ContentIdentity)
        );
    }

    #[test]
    fn signed_action_cannot_downgrade_to_unsigned_request_profile() {
        let fixture = fixture();
        let mut request = fixture.request;
        put(
            &mut request,
            12,
            &(DirectExecutionActionV3::InlineOrdinary as u32).to_le_bytes(),
        );
        assert!(authenticate_direct_artifacts_v3(
            fixture.selection(),
            fixture.artifacts(),
            &request,
            0,
        )
        .is_err());
    }
}
