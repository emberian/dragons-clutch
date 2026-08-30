//! Canonical Direct native-root close artifacts.
//!
//! Native close is one immutable action in the same ProgramSet as ordinary
//! execution. Its V1 descriptor is deliberately distinct from Hot's V4
//! descriptor, but both must carry the same manifest-selected kind, config,
//! capacity, root schema, derivation policy, and root width.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountProfileV1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_capability_contract::funding_ledger_bytes_v2;
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    activation_registers_v2::{
        ACTIVATION_ACTION_SCALAR_V2, ACTIVATION_FIRST_FAMILY_IDENTITY_V2,
        ACTIVATION_FIRST_FAMILY_SCALAR_V2, ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_ROOT_ACCOUNT_V2, ACTIVATION_ROOT_IDENTITY_V2,
        ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
    },
    encode_v1::{
        CapabilityProgramInputV1, capability_program_v1_bytes, encode_capability_program_v1_atomic,
    },
    v4::CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v2::{
    ProgramV2 as EffectProgramV2, SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID_V2,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
};
use dclutch_market_core_codec::CoreEffectActionV1;
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_sha256_adapter::digest;
use dclutch_transition_vm::v2::{
    ProgramV2 as TransitionProgramV2,
    encode::{
        RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
        encode_transition_program_v2_atomic, transition_program_v2_bytes,
    },
};

use crate::{
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleV4, validate_direct_inline_ordinary_hot_bundle_v4,
    },
    successor::{DirectRootStateLayoutV1, DirectRootStateV1},
};

/// High selector reserved for the lifecycle-native close route.
///
/// Direct executable action selectors occupy the low namespace. This value is
/// intentionally not derived from `CoreEffectActionV1` and cannot alias
/// `RegisterSell = 2` or another family action.
pub const DIRECT_NATIVE_CLOSE_SELECTOR_V1: u32 = 0xffff_ff01;
/// Exact close selector-request width; the selector is canonical `u32` at 12.
pub const DIRECT_NATIVE_CLOSE_REQUEST_BYTES_V1: usize = 16;
/// Domain-separating close selector-request magic.
pub const DIRECT_NATIVE_CLOSE_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDNC1";
/// Close selector-request schema version.
pub const DIRECT_NATIVE_CLOSE_REQUEST_VERSION_V1: u16 = 1;
/// Finalized schema label for the lifecycle-only close selector request.
pub const DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-native-close-request-v1";
/// SHA-256 of [`DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0x7d, 0xba, 0x78, 0xcb, 0xb7, 0x01, 0x0b, 0x07, 0x58, 0x23, 0x35, 0x2c, 0x09, 0xc6, 0x62, 0xba,
    0x1e, 0x6e, 0xa2, 0x6b, 0xd6, 0xfa, 0x3c, 0x84, 0x91, 0x92, 0xae, 0x1e, 0x16, 0xe7, 0xa1, 0xc7,
];

const ROOT_ACCOUNT: u16 = ACTIVATION_ROOT_ACCOUNT_V2;
const FUNDING_LEDGER_ACCOUNT: u16 = ACTIVATION_FIRST_FUNDING_ACCOUNT_V2;
const RENT_CREDIT_ACCOUNT: u16 = ACTIVATION_FIRST_FUNDING_ACCOUNT_V2 + 1;
const RENT_CREDIT_IDENTITY: u16 = ACTIVATION_FIRST_FAMILY_IDENTITY_V2;
const ROOT_MAGIC_SCALAR: u16 = ACTIVATION_FIRST_FAMILY_SCALAR_V2;
const ROOT_HEADER_WORD_SCALAR: u16 = ROOT_MAGIC_SCALAR + 1;
const ROOT_OPEN_MAKER_COUNT_SCALAR: u16 = ROOT_HEADER_WORD_SCALAR + 1;
const ROOT_LAMPORTS_SCALAR: u16 = ROOT_OPEN_MAKER_COUNT_SCALAR + 1;
const EXPECTED_CLOSE_ACTION_SCALAR: u16 = ROOT_LAMPORTS_SCALAR + 1;
const EXPECTED_ROOT_MAGIC_SCALAR: u16 = EXPECTED_CLOSE_ACTION_SCALAR + 1;
const EXPECTED_ROOT_HEADER_WORD_SCALAR: u16 = EXPECTED_ROOT_MAGIC_SCALAR + 1;
const EXPECTED_ZERO_SCALAR: u16 = EXPECTED_ROOT_HEADER_WORD_SCALAR + 1;
const CLOSE_SCALAR_COUNT: u16 = EXPECTED_ZERO_SCALAR + 1;
const CLOSE_IDENTITY_COUNT: u16 = RENT_CREDIT_IDENTITY + 1;
const CLOSE_ACCOUNT_COUNT: u16 = 3;

/// Chain-selected ordinary release facts inherited by native close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectNativeCloseBundleInputV1<'a> {
    /// Complete ordinary bundle whose manifest-bound coordinates are inherited.
    pub ordinary: &'a DirectInlineOrdinaryHotBundleV4,
    /// Exact manifest-selected capacity-profile identity.
    pub capacity_profile: [u8; 32],
}

/// Three finalized native-close records and their exact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectNativeCloseBundleV1 {
    /// Exact three-account AccountProfileV1 record.
    pub account_profile: Vec<u8>,
    /// Embedded TransitionVM V2 bytes, published here for audit evidence.
    pub transition: Vec<u8>,
    /// Exact validation-only EffectProgramV2 record.
    pub effect: Vec<u8>,
    /// Exact CapabilityProgramV1 close descriptor record.
    pub descriptor: Vec<u8>,
    /// SHA-256 identity of `account_profile`.
    pub account_profile_id: [u8; 32],
    /// SHA-256 identity of `effect`.
    pub effect_id: [u8; 32],
    /// SHA-256 identity of `descriptor`.
    pub descriptor_id: [u8; 32],
}

/// Stable native-close construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectNativeCloseBundleErrorV1 {
    /// The supplied ordinary bundle or capacity join refused.
    Ordinary,
    /// AccountProfile construction, geometry, or exact content refused.
    AccountProfile,
    /// Transition construction, geometry, or exact content refused.
    Transition,
    /// Effect construction, geometry, or exact content refused.
    Effect,
    /// Descriptor construction, inherited coordinates, or exact content refused.
    Descriptor,
    /// A fixed width or content identity was invalid.
    Geometry,
    /// Close selector request bytes were noncanonical.
    Request,
}

/// Encode the sole canonical lifecycle-close selector request.
pub fn direct_native_close_request_v1() -> [u8; DIRECT_NATIVE_CLOSE_REQUEST_BYTES_V1] {
    let mut output = [0_u8; DIRECT_NATIVE_CLOSE_REQUEST_BYTES_V1];
    output[..8].copy_from_slice(&DIRECT_NATIVE_CLOSE_REQUEST_MAGIC_V1);
    output[8..10].copy_from_slice(&DIRECT_NATIVE_CLOSE_REQUEST_VERSION_V1.to_le_bytes());
    output[12..16].copy_from_slice(&DIRECT_NATIVE_CLOSE_SELECTOR_V1.to_le_bytes());
    output
}

/// Hostile-check one exact lifecycle-close selector request.
pub fn validate_direct_native_close_request_v1(
    bytes: &[u8],
) -> Result<(), DirectNativeCloseBundleErrorV1> {
    if bytes != direct_native_close_request_v1() {
        return Err(DirectNativeCloseBundleErrorV1::Request);
    }
    Ok(())
}

/// Build the canonical Direct native-close descriptor/profile/effect bundle.
pub fn build_direct_native_close_bundle_v1(
    input: DirectNativeCloseBundleInputV1<'_>,
) -> Result<DirectNativeCloseBundleV1, DirectNativeCloseBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Ordinary)?;
    let account_profile = build_account_profile()?;
    let transition = build_transition()?;
    let effect = build_effect()?;
    let account_profile_id = digest(&account_profile);
    let effect_id = digest(&effect);
    let descriptor_width = capability_program_v1_bytes(transition.len())
        .map_err(|_| DirectNativeCloseBundleErrorV1::Descriptor)?;
    let mut descriptor_scratch = vec![0_u8; descriptor_width];
    let mut descriptor = vec![0_u8; descriptor_width];
    encode_capability_program_v1_atomic(
        CapabilityProgramInputV1 {
            kind: ordinary.kind(),
            config_schema: ordinary.config_schema(),
            request_schema: content(DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_ID_V1)?,
            root_schema: ordinary.root_schema(),
            account_profile: content(account_profile_id)?,
            derivation_policy: ordinary.derivation_policy(),
            capacity_profile: ordinary.capacity_profile(),
            effect_schema: content(effect_id)?,
            root_state_bytes: ordinary.root_state_bytes(),
            transition_program: &transition,
        },
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::Descriptor)?;
    let output = DirectNativeCloseBundleV1 {
        account_profile,
        transition,
        effect,
        descriptor_id: digest(&descriptor),
        descriptor,
        account_profile_id,
        effect_id,
    };
    validate_direct_native_close_bundle_v1(&output, input)?;
    Ok(output)
}

/// Hostile-decode and join one native-close bundle to its ordinary release.
pub fn validate_direct_native_close_bundle_v1(
    bundle: &DirectNativeCloseBundleV1,
    input: DirectNativeCloseBundleInputV1<'_>,
) -> Result<(), DirectNativeCloseBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Ordinary)?;
    if bundle.account_profile_id != digest(&bundle.account_profile)
        || bundle.effect_id != digest(&bundle.effect)
        || bundle.descriptor_id != digest(&bundle.descriptor)
    {
        return Err(DirectNativeCloseBundleErrorV1::Descriptor);
    }
    let expected_profile = build_account_profile()?;
    if bundle.account_profile != expected_profile {
        return Err(DirectNativeCloseBundleErrorV1::AccountProfile);
    }
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::AccountProfile)?;
    if profile.account_count() != CLOSE_ACCOUNT_COUNT
        || profile.scalar_count() != CLOSE_SCALAR_COUNT
        || profile.identity_count() != CLOSE_IDENTITY_COUNT
    {
        return Err(DirectNativeCloseBundleErrorV1::AccountProfile);
    }
    let expected_transition = build_transition()?;
    if bundle.transition != expected_transition {
        return Err(DirectNativeCloseBundleErrorV1::Transition);
    }
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Transition)?;
    if transition.scalar_count() != CLOSE_SCALAR_COUNT
        || transition.identity_count() != CLOSE_IDENTITY_COUNT
    {
        return Err(DirectNativeCloseBundleErrorV1::Transition);
    }
    let expected_effect = build_effect()?;
    if bundle.effect != expected_effect {
        return Err(DirectNativeCloseBundleErrorV1::Effect);
    }
    let effect = EffectProgramV2::decode(&bundle.effect)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Effect)?;
    if effect.account_count() != CLOSE_ACCOUNT_COUNT
        || effect.scalar_count() != CLOSE_SCALAR_COUNT
        || effect.identity_count() != CLOSE_IDENTITY_COUNT
        || effect.request_bytes() != 0
    {
        return Err(DirectNativeCloseBundleErrorV1::Effect);
    }
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| DirectNativeCloseBundleErrorV1::Descriptor)?;
    if descriptor.kind() != ordinary.kind()
        || descriptor.config_schema() != ordinary.config_schema()
        || descriptor.request_schema().to_bytes() != DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema() != ordinary.root_schema()
        || descriptor.account_profile().to_bytes() != bundle.account_profile_id
        || descriptor.derivation_policy() != ordinary.derivation_policy()
        || descriptor.capacity_profile() != ordinary.capacity_profile()
        || descriptor.capacity_profile().to_bytes() != input.capacity_profile
        || descriptor.effect_schema().to_bytes() != bundle.effect_id
        || descriptor.root_state_bytes() != ordinary.root_state_bytes()
        || descriptor.transition_program().bytes() != bundle.transition
    {
        return Err(DirectNativeCloseBundleErrorV1::Descriptor);
    }
    Ok(())
}

/// Schema used to finalize the close AccountProfile record.
pub const fn direct_native_close_account_profile_schema_v1() -> [u8; 32] {
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
}

/// Schema used to finalize the close EffectProgram record.
pub const fn direct_native_close_effect_schema_v1() -> [u8; 32] {
    EFFECT_PROGRAM_SCHEMA_ID_V2
}

/// Schema used to finalize the close descriptor record.
pub const fn direct_native_close_descriptor_schema_v1() -> [u8; 32] {
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
}

fn build_account_profile() -> Result<Vec<u8>, DirectNativeCloseBundleErrorV1> {
    let root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(crate::successor::DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectNativeCloseBundleErrorV1::Geometry)?;
    let rules = [
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(root_bytes)
                .map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)?,
        },
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(
                funding_ledger_bytes_v2(1).map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)?,
            )
            .map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)?,
        },
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2)
                .map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)?,
        },
    ];
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: ROOT_ACCOUNT,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ROOT_ACCOUNT,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: FUNDING_LEDGER_ACCOUNT,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: ROOT_ACCOUNT,
            data_offset: root_offset(DirectRootStateLayoutV1::MAGIC)?,
            destination: ROOT_MAGIC_SCALAR,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: ROOT_ACCOUNT,
            data_offset: root_offset(DirectRootStateLayoutV1::VERSION)?,
            destination: ROOT_HEADER_WORD_SCALAR,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: ROOT_ACCOUNT,
            data_offset: root_offset(DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT)?,
            destination: ROOT_OPEN_MAKER_COUNT_SCALAR,
        },
        AccountOperationInputV1::ProjectLamports {
            account: ROOT_ACCOUNT,
            destination: ROOT_LAMPORTS_SCALAR,
        },
        AccountOperationInputV1::RequireKey {
            account: RENT_CREDIT_ACCOUNT,
            expected: RENT_CREDIT_IDENTITY,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| DirectNativeCloseBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: CLOSE_SCALAR_COUNT,
            identities: CLOSE_IDENTITY_COUNT,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::AccountProfile)?;
    Ok(output)
}

fn build_transition() -> Result<Vec<u8>, DirectNativeCloseBundleErrorV1> {
    let retiring = DirectRootStateV1::new()
        .begin_retiring()
        .map_err(|_| DirectNativeCloseBundleErrorV1::Transition)?
        .encode();
    let expected_header = read_u64(&retiring, DirectRootStateLayoutV1::VERSION)?;
    let instructions = [
        TransitionInstructionV2::load_const(
            EXPECTED_CLOSE_ACTION_SCALAR,
            CoreEffectActionV1::CloseCapability as u64,
        ),
        TransitionInstructionV2::load_const(
            EXPECTED_ROOT_MAGIC_SCALAR,
            DirectRootStateLayoutV1::MAGIC_WORD,
        ),
        TransitionInstructionV2::load_const(EXPECTED_ROOT_HEADER_WORD_SCALAR, expected_header),
        TransitionInstructionV2::load_const(EXPECTED_ZERO_SCALAR, 0),
        TransitionInstructionV2::scalar_eq(
            ACTIVATION_ACTION_SCALAR_V2,
            EXPECTED_CLOSE_ACTION_SCALAR,
        ),
        TransitionInstructionV2::scalar_eq(ROOT_MAGIC_SCALAR, EXPECTED_ROOT_MAGIC_SCALAR),
        TransitionInstructionV2::scalar_eq(
            ROOT_HEADER_WORD_SCALAR,
            EXPECTED_ROOT_HEADER_WORD_SCALAR,
        ),
        TransitionInstructionV2::scalar_eq(ROOT_OPEN_MAKER_COUNT_SCALAR, EXPECTED_ZERO_SCALAR),
    ];
    let width = transition_program_v2_bytes(instructions.len())
        .map_err(|_| DirectNativeCloseBundleErrorV1::Transition)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        TransitionRegisterGeometryV2 {
            scalars: CLOSE_SCALAR_COUNT,
            identities: CLOSE_IDENTITY_COUNT,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::Transition)?;
    Ok(output)
}

fn build_effect() -> Result<Vec<u8>, DirectNativeCloseBundleErrorV1> {
    let instructions = [EffectInstructionV2::require_lamports_eq(
        ROOT_ACCOUNT,
        ROOT_LAMPORTS_SCALAR,
    )];
    let width = effect_program_v2_bytes(instructions.len())
        .map_err(|_| DirectNativeCloseBundleErrorV1::Effect)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v2_atomic(
        EffectGeometryV2 {
            accounts: CLOSE_ACCOUNT_COUNT,
            scalars: CLOSE_SCALAR_COUNT,
            identities: CLOSE_IDENTITY_COUNT,
            request_bytes: 0,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::Effect)?;
    Ok(output)
}

fn root_offset(tail_offset: usize) -> Result<u32, DirectNativeCloseBundleErrorV1> {
    u32::try_from(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(tail_offset)
            .ok_or(DirectNativeCloseBundleErrorV1::Geometry)?,
    )
    .map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DirectNativeCloseBundleErrorV1> {
    Ok(u64::from_le_bytes(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(DirectNativeCloseBundleErrorV1::Geometry)?,
            )
            .ok_or(DirectNativeCloseBundleErrorV1::Geometry)?
            .try_into()
            .map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectNativeCloseBundleErrorV1> {
    ContentId::new(bytes).map_err(|_| DirectNativeCloseBundleErrorV1::Geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_program_contract::CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET;
    use dclutch_sha256_adapter::digest;

    fn ordinary() -> DirectInlineOrdinaryHotBundleV4 {
        crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests()
    }

    #[test]
    fn request_schema_id_and_high_selector_are_frozen() {
        assert_eq!(
            digest(DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_PREIMAGE_V1),
            DIRECT_NATIVE_CLOSE_REQUEST_SCHEMA_ID_V1
        );
        assert_eq!(DIRECT_NATIVE_CLOSE_SELECTOR_V1, 0xffff_ff01);
        assert_ne!(DIRECT_NATIVE_CLOSE_SELECTOR_V1, 2);
        let request = direct_native_close_request_v1();
        validate_direct_native_close_request_v1(&request).expect("request");
        assert_eq!(
            u32::from_le_bytes(request[12..16].try_into().expect("selector")),
            DIRECT_NATIVE_CLOSE_SELECTOR_V1
        );
        for offset in [0_usize, 8, 10, 12] {
            let mut hostile = request;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert_eq!(
                validate_direct_native_close_request_v1(&hostile),
                Err(DirectNativeCloseBundleErrorV1::Request)
            );
        }
    }

    #[test]
    fn exact_close_bundle_inherits_release_and_checks_terminal_root_geometry() {
        let ordinary = ordinary();
        let input = DirectNativeCloseBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_native_close_bundle_v1(input).expect("close bundle");
        validate_direct_native_close_bundle_v1(&bundle, input).expect("validate");
        assert_eq!(bundle.account_profile_id, digest(&bundle.account_profile));
        assert_eq!(bundle.effect_id, digest(&bundle.effect));
        assert_eq!(bundle.descriptor_id, digest(&bundle.descriptor));
        let ordinary_descriptor = CapabilityProgramV4::decode(&ordinary.descriptor).expect("V4");
        let close_descriptor = CapabilityProgramV1::decode(&bundle.descriptor).expect("V1");
        assert_eq!(close_descriptor.kind(), ordinary_descriptor.kind());
        assert_eq!(
            close_descriptor.config_schema(),
            ordinary_descriptor.config_schema()
        );
        assert_eq!(
            close_descriptor.root_schema(),
            ordinary_descriptor.root_schema()
        );
        assert_eq!(
            close_descriptor.derivation_policy(),
            ordinary_descriptor.derivation_policy()
        );
        assert_eq!(
            close_descriptor.capacity_profile(),
            ordinary_descriptor.capacity_profile()
        );
        assert_eq!(
            close_descriptor.root_state_bytes(),
            ordinary_descriptor.root_state_bytes()
        );
        assert_eq!(
            direct_native_close_account_profile_schema_v1(),
            ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            direct_native_close_effect_schema_v1(),
            EFFECT_PROGRAM_SCHEMA_ID_V2
        );
        assert_eq!(
            direct_native_close_descriptor_schema_v1(),
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
    }

    #[test]
    fn substituted_profile_effect_descriptor_and_capacity_refuse() {
        let ordinary = ordinary();
        let input = DirectNativeCloseBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_native_close_bundle_v1(input).expect("close bundle");

        let mut profile = bundle.clone();
        *profile.account_profile.last_mut().expect("profile byte") ^= 1;
        assert!(validate_direct_native_close_bundle_v1(&profile, input).is_err());

        let mut effect = bundle.clone();
        *effect.effect.last_mut().expect("effect byte") ^= 1;
        assert!(validate_direct_native_close_bundle_v1(&effect, input).is_err());

        let mut descriptor = bundle.clone();
        *descriptor
            .descriptor
            .get_mut(CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET)
            .expect("descriptor profile") ^= 1;
        descriptor.descriptor_id = digest(&descriptor.descriptor);
        assert_eq!(
            validate_direct_native_close_bundle_v1(&descriptor, input),
            Err(DirectNativeCloseBundleErrorV1::Descriptor)
        );

        assert_eq!(
            validate_direct_native_close_bundle_v1(
                &bundle,
                DirectNativeCloseBundleInputV1 {
                    ordinary: &ordinary,
                    capacity_profile: [0x45; 32],
                },
            ),
            Err(DirectNativeCloseBundleErrorV1::Ordinary)
        );
    }
}
