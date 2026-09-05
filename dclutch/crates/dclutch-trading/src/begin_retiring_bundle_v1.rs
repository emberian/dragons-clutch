//! Canonical Direct Open-to-Retiring lifecycle artifacts.
//!
//! These records authorize only the existing composite Direct root's exact
//! phase-word update. The Trading adapter separately authenticates the current
//! Core Market as `Retiring`; the artifacts cannot move lamports, close an
//! account, or alter the maker-root count.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_vm::account_profile::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountProfileV1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    encode_v1::{
        CapabilityProgramInputV1, capability_program_v1_bytes, encode_capability_program_v1_atomic,
    },
    v4::CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_vm::effect::v2::{
    ProgramV2 as EffectProgramV2, SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID_V2,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
};
use dclutch_sha256_adapter::digest;
use dclutch_vm::v2::{
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
    retirement_v1::{
        DIRECT_BEGIN_RETIRING_EXPECTED_MAGIC_SCALAR_V1,
        DIRECT_BEGIN_RETIRING_EXPECTED_OPEN_HEADER_SCALAR_V1,
        DIRECT_BEGIN_RETIRING_EXPECTED_SELECTOR_SCALAR_V1, DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1,
        DIRECT_BEGIN_RETIRING_MAKER_COUNT_SCALAR_V1, DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1,
        DIRECT_BEGIN_RETIRING_RETIRING_HEADER_SCALAR_V1, DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_ROOT_HEADER_SCALAR_V1, DIRECT_BEGIN_RETIRING_ROOT_IDENTITY_V1,
        DIRECT_BEGIN_RETIRING_ROOT_LAMPORTS_SCALAR_V1, DIRECT_BEGIN_RETIRING_ROOT_MAGIC_SCALAR_V1,
        DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1, DIRECT_BEGIN_RETIRING_SELECTOR_SCALAR_V1,
        DIRECT_BEGIN_RETIRING_SELECTOR_V1, DIRECT_BEGIN_RETIRING_TRADING_IDENTITY_V1,
    },
    successor::{DIRECT_ROOT_STATE_BYTES_V1, DirectRootStateLayoutV1, DirectRootStateV1},
};

/// Chain-selected ordinary release facts inherited by retirement start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringBundleInputV1<'a> {
    /// Complete ordinary bundle whose immutable coordinates are inherited.
    pub ordinary: &'a DirectInlineOrdinaryHotBundleV4,
    /// Exact manifest-selected capacity-profile identity.
    pub capacity_profile: [u8; 32],
}

/// Three finalized begin-retiring records and their exact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringBundleV1 {
    /// Exact one-root AccountProfileV1 record.
    pub account_profile: Vec<u8>,
    /// Embedded TransitionVM V2 bytes, exposed for audit evidence.
    pub transition: Vec<u8>,
    /// Exact two-operation EffectProgramV2 record.
    pub effect: Vec<u8>,
    /// Exact CapabilityProgramV1 lifecycle descriptor.
    pub descriptor: Vec<u8>,
    /// SHA-256 identity of `account_profile`.
    pub account_profile_id: [u8; 32],
    /// SHA-256 identity of `effect`.
    pub effect_id: [u8; 32],
    /// SHA-256 identity of `descriptor`.
    pub descriptor_id: [u8; 32],
}

/// Stable begin-retiring artifact construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringBundleErrorV1 {
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
}

/// Build the canonical Direct begin-retiring descriptor/profile/effect bundle.
pub fn build_direct_begin_retiring_bundle_v1(
    input: DirectBeginRetiringBundleInputV1<'_>,
) -> Result<DirectBeginRetiringBundleV1, DirectBeginRetiringBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Ordinary)?;
    let account_profile = build_account_profile()?;
    let transition = build_transition()?;
    let effect = build_effect()?;
    let account_profile_id = digest(&account_profile);
    let effect_id = digest(&effect);
    let width = capability_program_v1_bytes(transition.len())
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Descriptor)?;
    let mut scratch = vec![0_u8; width];
    let mut descriptor = vec![0_u8; width];
    encode_capability_program_v1_atomic(
        CapabilityProgramInputV1 {
            kind: ordinary.kind(),
            config_schema: ordinary.config_schema(),
            request_schema: content(DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1)?,
            root_schema: ordinary.root_schema(),
            account_profile: content(account_profile_id)?,
            derivation_policy: ordinary.derivation_policy(),
            capacity_profile: ordinary.capacity_profile(),
            effect_schema: content(effect_id)?,
            root_state_bytes: ordinary.root_state_bytes(),
            transition_program: &transition,
        },
        &mut scratch,
        &mut descriptor,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::Descriptor)?;
    let output = DirectBeginRetiringBundleV1 {
        account_profile,
        transition,
        effect,
        descriptor_id: digest(&descriptor),
        descriptor,
        account_profile_id,
        effect_id,
    };
    validate_direct_begin_retiring_bundle_v1(&output, input)?;
    Ok(output)
}

/// Hostile-decode and join one retirement bundle to its ordinary release.
pub fn validate_direct_begin_retiring_bundle_v1(
    bundle: &DirectBeginRetiringBundleV1,
    input: DirectBeginRetiringBundleInputV1<'_>,
) -> Result<(), DirectBeginRetiringBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Ordinary)?;
    if bundle.account_profile_id != digest(&bundle.account_profile)
        || bundle.effect_id != digest(&bundle.effect)
        || bundle.descriptor_id != digest(&bundle.descriptor)
    {
        return Err(DirectBeginRetiringBundleErrorV1::Descriptor);
    }
    if bundle.account_profile != build_account_profile()? {
        return Err(DirectBeginRetiringBundleErrorV1::AccountProfile);
    }
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::AccountProfile)?;
    if profile.account_count() != 1
        || profile.scalar_count() != DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1
        || profile.identity_count() != DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1
    {
        return Err(DirectBeginRetiringBundleErrorV1::AccountProfile);
    }
    if bundle.transition != build_transition()? {
        return Err(DirectBeginRetiringBundleErrorV1::Transition);
    }
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Transition)?;
    if transition.scalar_count() != DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1
        || transition.identity_count() != DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1
    {
        return Err(DirectBeginRetiringBundleErrorV1::Transition);
    }
    if bundle.effect != build_effect()? {
        return Err(DirectBeginRetiringBundleErrorV1::Effect);
    }
    let effect = EffectProgramV2::decode(&bundle.effect)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Effect)?;
    if effect.account_count() != 1
        || effect.scalar_count() != DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1
        || effect.identity_count() != DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1
        || effect.request_bytes() != 0
        || effect.instruction_count() != 2
    {
        return Err(DirectBeginRetiringBundleErrorV1::Effect);
    }
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Descriptor)?;
    if descriptor.kind() != ordinary.kind()
        || descriptor.config_schema() != ordinary.config_schema()
        || descriptor.request_schema().to_bytes() != DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema() != ordinary.root_schema()
        || descriptor.account_profile().to_bytes() != bundle.account_profile_id
        || descriptor.derivation_policy() != ordinary.derivation_policy()
        || descriptor.capacity_profile() != ordinary.capacity_profile()
        || descriptor.capacity_profile().to_bytes() != input.capacity_profile
        || descriptor.effect_schema().to_bytes() != bundle.effect_id
        || descriptor.root_state_bytes() != ordinary.root_state_bytes()
        || descriptor.transition_program().bytes() != bundle.transition
    {
        return Err(DirectBeginRetiringBundleErrorV1::Descriptor);
    }
    Ok(())
}

/// Schema used to finalize the begin-retiring AccountProfile record.
pub const fn direct_begin_retiring_account_profile_schema_v1() -> [u8; 32] {
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
}

/// Schema used to finalize the begin-retiring EffectProgram record.
pub const fn direct_begin_retiring_effect_schema_v1() -> [u8; 32] {
    EFFECT_PROGRAM_SCHEMA_ID_V2
}

/// Schema used to finalize the begin-retiring descriptor record.
pub const fn direct_begin_retiring_descriptor_schema_v1() -> [u8; 32] {
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
}

fn build_account_profile() -> Result<Vec<u8>, DirectBeginRetiringBundleErrorV1> {
    let root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectBeginRetiringBundleErrorV1::Geometry)?;
    let rules = [AccountRuleInputV1 {
        privileges: AccountPrivilegesV1::new(false, true, false),
        effect_permissions: AccountEffectPermissionsV1::new(false, false, true),
        alias: AccountAliasInputV1::SelfRepresentative,
        data_length: u32::try_from(root_bytes)
            .map_err(|_| DirectBeginRetiringBundleErrorV1::Geometry)?,
    }];
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            expected: DIRECT_BEGIN_RETIRING_ROOT_IDENTITY_V1,
        },
        AccountOperationInputV1::RequireOwner {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            expected: DIRECT_BEGIN_RETIRING_TRADING_IDENTITY_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::MAGIC)?,
            destination: DIRECT_BEGIN_RETIRING_ROOT_MAGIC_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::VERSION)?,
            destination: DIRECT_BEGIN_RETIRING_ROOT_HEADER_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT)?,
            destination: DIRECT_BEGIN_RETIRING_MAKER_COUNT_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectLamports {
            account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            destination: DIRECT_BEGIN_RETIRING_ROOT_LAMPORTS_SCALAR_V1,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| DirectBeginRetiringBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1,
            identities: DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::AccountProfile)?;
    Ok(output)
}

fn build_transition() -> Result<Vec<u8>, DirectBeginRetiringBundleErrorV1> {
    let open = DirectRootStateV1::new().encode();
    let retiring = DirectRootStateV1::new()
        .begin_retiring()
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Transition)?
        .encode();
    let instructions = [
        TransitionInstructionV2::load_const(
            DIRECT_BEGIN_RETIRING_EXPECTED_SELECTOR_SCALAR_V1,
            u64::from(DIRECT_BEGIN_RETIRING_SELECTOR_V1),
        ),
        TransitionInstructionV2::load_const(
            DIRECT_BEGIN_RETIRING_EXPECTED_MAGIC_SCALAR_V1,
            DirectRootStateLayoutV1::MAGIC_WORD,
        ),
        TransitionInstructionV2::load_const(
            DIRECT_BEGIN_RETIRING_EXPECTED_OPEN_HEADER_SCALAR_V1,
            read_u64(&open, DirectRootStateLayoutV1::VERSION)?,
        ),
        TransitionInstructionV2::load_const(
            DIRECT_BEGIN_RETIRING_RETIRING_HEADER_SCALAR_V1,
            read_u64(&retiring, DirectRootStateLayoutV1::VERSION)?,
        ),
        TransitionInstructionV2::scalar_eq(
            DIRECT_BEGIN_RETIRING_SELECTOR_SCALAR_V1,
            DIRECT_BEGIN_RETIRING_EXPECTED_SELECTOR_SCALAR_V1,
        ),
        TransitionInstructionV2::scalar_eq(
            DIRECT_BEGIN_RETIRING_ROOT_MAGIC_SCALAR_V1,
            DIRECT_BEGIN_RETIRING_EXPECTED_MAGIC_SCALAR_V1,
        ),
        TransitionInstructionV2::scalar_eq(
            DIRECT_BEGIN_RETIRING_ROOT_HEADER_SCALAR_V1,
            DIRECT_BEGIN_RETIRING_EXPECTED_OPEN_HEADER_SCALAR_V1,
        ),
        // Deliberately NO `MAKER_COUNT == 0` conjunct (cohort-9 review item 1,
        // amendment 1): begin-retiring admits standing maker roots, which
        // drain inside Retiring via `close_maker_replay_v2`. The count gate
        // that protects Retired stays at both physical-close sites -- this
        // bundle's sibling in `native_close_bundle_v1` still pins it to zero.
        // The count scalar is still projected (register geometry and the
        // account profile are unchanged); its zero-expectation register is
        // simply no longer compared here.
    ];
    let width = transition_program_v2_bytes(instructions.len())
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Transition)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        TransitionRegisterGeometryV2 {
            scalars: DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1,
            identities: DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::Transition)?;
    Ok(output)
}

fn build_effect() -> Result<Vec<u8>, DirectBeginRetiringBundleErrorV1> {
    let instructions = [
        EffectInstructionV2::write_u64(
            DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            root_offset(DirectRootStateLayoutV1::VERSION)?,
            DIRECT_BEGIN_RETIRING_RETIRING_HEADER_SCALAR_V1,
        ),
        EffectInstructionV2::require_lamports_eq(
            DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
            DIRECT_BEGIN_RETIRING_ROOT_LAMPORTS_SCALAR_V1,
        ),
    ];
    let width = effect_program_v2_bytes(instructions.len())
        .map_err(|_| DirectBeginRetiringBundleErrorV1::Effect)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v2_atomic(
        EffectGeometryV2 {
            accounts: 1,
            scalars: DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1,
            identities: DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1,
            request_bytes: 0,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::Effect)?;
    Ok(output)
}

fn root_offset(tail_offset: usize) -> Result<u32, DirectBeginRetiringBundleErrorV1> {
    u32::try_from(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(tail_offset)
            .ok_or(DirectBeginRetiringBundleErrorV1::Geometry)?,
    )
    .map_err(|_| DirectBeginRetiringBundleErrorV1::Geometry)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DirectBeginRetiringBundleErrorV1> {
    Ok(u64::from_le_bytes(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(DirectBeginRetiringBundleErrorV1::Geometry)?,
            )
            .ok_or(DirectBeginRetiringBundleErrorV1::Geometry)?
            .try_into()
            .map_err(|_| DirectBeginRetiringBundleErrorV1::Geometry)?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectBeginRetiringBundleErrorV1> {
    ContentId::new(bytes).map_err(|_| DirectBeginRetiringBundleErrorV1::Geometry)
}

#[cfg(test)]
mod tests {
    use dclutch_market::capability_program::CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET;

    use super::*;

    fn ordinary() -> DirectInlineOrdinaryHotBundleV4 {
        crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests()
    }

    #[test]
    fn exact_bundle_inherits_release_and_freezes_lifecycle_geometry() {
        let ordinary = ordinary();
        let input = DirectBeginRetiringBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_begin_retiring_bundle_v1(input).expect("retirement bundle");
        validate_direct_begin_retiring_bundle_v1(&bundle, input).expect("validate");
        let inherited = CapabilityProgramV4::decode(&ordinary.descriptor).expect("ordinary");
        let descriptor = CapabilityProgramV1::decode(&bundle.descriptor).expect("retirement");
        assert_eq!(descriptor.kind(), inherited.kind());
        assert_eq!(descriptor.config_schema(), inherited.config_schema());
        assert_eq!(descriptor.root_schema(), inherited.root_schema());
        assert_eq!(
            descriptor.derivation_policy(),
            inherited.derivation_policy()
        );
        assert_eq!(descriptor.capacity_profile(), inherited.capacity_profile());
        assert_eq!(descriptor.root_state_bytes(), inherited.root_state_bytes());
        assert_eq!(bundle.account_profile_id, digest(&bundle.account_profile));
        assert_eq!(bundle.effect_id, digest(&bundle.effect));
        assert_eq!(bundle.descriptor_id, digest(&bundle.descriptor));
    }

    #[test]
    fn profile_effect_descriptor_and_capacity_substitution_refuse() {
        let ordinary = ordinary();
        let input = DirectBeginRetiringBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_begin_retiring_bundle_v1(input).expect("retirement bundle");
        let mut profile = bundle.clone();
        *profile.account_profile.last_mut().expect("profile byte") ^= 1;
        assert!(validate_direct_begin_retiring_bundle_v1(&profile, input).is_err());
        let mut effect = bundle.clone();
        *effect.effect.last_mut().expect("effect byte") ^= 1;
        assert!(validate_direct_begin_retiring_bundle_v1(&effect, input).is_err());
        let mut descriptor = bundle.clone();
        *descriptor
            .descriptor
            .get_mut(CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET)
            .expect("descriptor profile") ^= 1;
        descriptor.descriptor_id = digest(&descriptor.descriptor);
        assert_eq!(
            validate_direct_begin_retiring_bundle_v1(&descriptor, input),
            Err(DirectBeginRetiringBundleErrorV1::Descriptor)
        );
        assert_eq!(
            validate_direct_begin_retiring_bundle_v1(
                &bundle,
                DirectBeginRetiringBundleInputV1 {
                    ordinary: &ordinary,
                    capacity_profile: [0x45; 32],
                },
            ),
            Err(DirectBeginRetiringBundleErrorV1::Ordinary)
        );
    }
}
