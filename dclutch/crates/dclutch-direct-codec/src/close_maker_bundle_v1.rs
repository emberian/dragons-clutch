//! Canonical Direct maker-replay close lifecycle artifacts.
//!
//! Wall 22 found both released counter-writing transitions structurally
//! incapable of decrementing `open_maker_root_count`. This bundle is the
//! answer at the same level the wall found it: the released transition itself
//! refuses a drained count (`nonzero`) and computes the decrement
//! (`sub_into`), and the released effect writes it back -- so the ONLY
//! decrement the release authorizes is by exactly one, only against a
//! `Retiring` root header. The chain executable independently derives the
//! same poststate through `close_maker_replay_v2` (which also carries the
//! `fee_owed` and `live_count` refusals over the replay account these
//! artifacts never see) and commits only on agreement.

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
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
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
use dclutch_sha256_adapter::digest;
use dclutch_transition_vm::v2::{
    ProgramV2 as TransitionProgramV2,
    encode::{
        RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
        encode_transition_program_v2_atomic, transition_program_v2_bytes,
    },
};

use crate::{
    close_maker_v1::{
        DIRECT_CLOSE_MAKER_EXPECTED_MAGIC_SCALAR_V1,
        DIRECT_CLOSE_MAKER_EXPECTED_SELECTOR_SCALAR_V1, DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1,
        DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1, DIRECT_CLOSE_MAKER_ONE_SCALAR_V1,
        DIRECT_CLOSE_MAKER_POST_COUNT_SCALAR_V1, DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1,
        DIRECT_CLOSE_MAKER_RETIRING_HEADER_SCALAR_V1, DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_ROOT_HEADER_SCALAR_V1, DIRECT_CLOSE_MAKER_ROOT_IDENTITY_V1,
        DIRECT_CLOSE_MAKER_ROOT_LAMPORTS_SCALAR_V1, DIRECT_CLOSE_MAKER_ROOT_MAGIC_SCALAR_V1,
        DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1, DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1,
        DIRECT_CLOSE_MAKER_SELECTOR_V1, DIRECT_CLOSE_MAKER_TRADING_IDENTITY_V1,
    },
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleV4, validate_direct_inline_ordinary_hot_bundle_v4,
    },
    successor::{DIRECT_ROOT_STATE_BYTES_V1, DirectRootStateLayoutV1, DirectRootStateV1},
};

/// Chain-selected ordinary release facts inherited by the maker close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerBundleInputV1<'a> {
    /// Complete ordinary bundle whose immutable coordinates are inherited.
    pub ordinary: &'a DirectInlineOrdinaryHotBundleV4,
    /// Exact manifest-selected capacity-profile identity.
    pub capacity_profile: [u8; 32],
}

/// Three finalized close-maker records and their exact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerBundleV1 {
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

/// Stable close-maker artifact construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerBundleErrorV1 {
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

/// Build the canonical Direct close-maker descriptor/profile/effect bundle.
pub fn build_direct_close_maker_bundle_v1(
    input: DirectCloseMakerBundleInputV1<'_>,
) -> Result<DirectCloseMakerBundleV1, DirectCloseMakerBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Ordinary)?;
    let account_profile = build_account_profile()?;
    let transition = build_transition()?;
    let effect = build_effect()?;
    let account_profile_id = digest(&account_profile);
    let effect_id = digest(&effect);
    let width = capability_program_v1_bytes(transition.len())
        .map_err(|_| DirectCloseMakerBundleErrorV1::Descriptor)?;
    let mut scratch = vec![0_u8; width];
    let mut descriptor = vec![0_u8; width];
    encode_capability_program_v1_atomic(
        CapabilityProgramInputV1 {
            kind: ordinary.kind(),
            config_schema: ordinary.config_schema(),
            request_schema: content(DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1)?,
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
    .map_err(|_| DirectCloseMakerBundleErrorV1::Descriptor)?;
    let output = DirectCloseMakerBundleV1 {
        account_profile,
        transition,
        effect,
        descriptor_id: digest(&descriptor),
        descriptor,
        account_profile_id,
        effect_id,
    };
    validate_direct_close_maker_bundle_v1(&output, input)?;
    Ok(output)
}

/// Hostile-decode and join one close-maker bundle to its ordinary release.
pub fn validate_direct_close_maker_bundle_v1(
    bundle: &DirectCloseMakerBundleV1,
    input: DirectCloseMakerBundleInputV1<'_>,
) -> Result<(), DirectCloseMakerBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Ordinary)?;
    if bundle.account_profile_id != digest(&bundle.account_profile)
        || bundle.effect_id != digest(&bundle.effect)
        || bundle.descriptor_id != digest(&bundle.descriptor)
    {
        return Err(DirectCloseMakerBundleErrorV1::Descriptor);
    }
    if bundle.account_profile != build_account_profile()? {
        return Err(DirectCloseMakerBundleErrorV1::AccountProfile);
    }
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| DirectCloseMakerBundleErrorV1::AccountProfile)?;
    if profile.account_count() != 1
        || profile.scalar_count() != DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1
        || profile.identity_count() != DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1
    {
        return Err(DirectCloseMakerBundleErrorV1::AccountProfile);
    }
    if bundle.transition != build_transition()? {
        return Err(DirectCloseMakerBundleErrorV1::Transition);
    }
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Transition)?;
    if transition.scalar_count() != DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1
        || transition.identity_count() != DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1
    {
        return Err(DirectCloseMakerBundleErrorV1::Transition);
    }
    if bundle.effect != build_effect()? {
        return Err(DirectCloseMakerBundleErrorV1::Effect);
    }
    let effect = EffectProgramV2::decode(&bundle.effect)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Effect)?;
    if effect.account_count() != 1
        || effect.scalar_count() != DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1
        || effect.identity_count() != DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1
        || effect.request_bytes() != 0
    {
        return Err(DirectCloseMakerBundleErrorV1::Effect);
    }
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| DirectCloseMakerBundleErrorV1::Descriptor)?;
    if descriptor.kind() != ordinary.kind()
        || descriptor.config_schema() != ordinary.config_schema()
        || descriptor.request_schema().to_bytes() != DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema() != ordinary.root_schema()
        || descriptor.account_profile().to_bytes() != bundle.account_profile_id
        || descriptor.derivation_policy() != ordinary.derivation_policy()
        || descriptor.capacity_profile() != ordinary.capacity_profile()
        || descriptor.capacity_profile().to_bytes() != input.capacity_profile
        || descriptor.effect_schema().to_bytes() != bundle.effect_id
        || descriptor.root_state_bytes() != ordinary.root_state_bytes()
        || descriptor.transition_program().bytes() != bundle.transition
    {
        return Err(DirectCloseMakerBundleErrorV1::Descriptor);
    }
    Ok(())
}

/// Schema used to finalize the close-maker AccountProfile record.
pub const fn direct_close_maker_account_profile_schema_v1() -> [u8; 32] {
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
}

/// Schema used to finalize the close-maker EffectProgram record.
pub const fn direct_close_maker_effect_schema_v1() -> [u8; 32] {
    EFFECT_PROGRAM_SCHEMA_ID_V2
}

/// Schema used to finalize the close-maker descriptor record.
pub const fn direct_close_maker_descriptor_schema_v1() -> [u8; 32] {
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
}

fn build_account_profile() -> Result<Vec<u8>, DirectCloseMakerBundleErrorV1> {
    let root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(DirectCloseMakerBundleErrorV1::Geometry)?;
    let rules = [AccountRuleInputV1 {
        privileges: AccountPrivilegesV1::new(false, true, false),
        effect_permissions: AccountEffectPermissionsV1::new(false, false, true),
        alias: AccountAliasInputV1::SelfRepresentative,
        data_length: u32::try_from(root_bytes)
            .map_err(|_| DirectCloseMakerBundleErrorV1::Geometry)?,
    }];
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            expected: DIRECT_CLOSE_MAKER_ROOT_IDENTITY_V1,
        },
        AccountOperationInputV1::RequireOwner {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            expected: DIRECT_CLOSE_MAKER_TRADING_IDENTITY_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::MAGIC)?,
            destination: DIRECT_CLOSE_MAKER_ROOT_MAGIC_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::VERSION)?,
            destination: DIRECT_CLOSE_MAKER_ROOT_HEADER_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            data_offset: root_offset(DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT)?,
            destination: DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1,
        },
        AccountOperationInputV1::ProjectLamports {
            account: DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            destination: DIRECT_CLOSE_MAKER_ROOT_LAMPORTS_SCALAR_V1,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| DirectCloseMakerBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1,
            identities: DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectCloseMakerBundleErrorV1::AccountProfile)?;
    Ok(output)
}

fn build_transition() -> Result<Vec<u8>, DirectCloseMakerBundleErrorV1> {
    let retiring = DirectRootStateV1::new()
        .begin_retiring()
        .map_err(|_| DirectCloseMakerBundleErrorV1::Transition)?
        .encode();
    let instructions = [
        TransitionInstructionV2::load_const(
            DIRECT_CLOSE_MAKER_EXPECTED_SELECTOR_SCALAR_V1,
            u64::from(DIRECT_CLOSE_MAKER_SELECTOR_V1),
        ),
        TransitionInstructionV2::load_const(
            DIRECT_CLOSE_MAKER_EXPECTED_MAGIC_SCALAR_V1,
            DirectRootStateLayoutV1::MAGIC_WORD,
        ),
        TransitionInstructionV2::load_const(
            DIRECT_CLOSE_MAKER_RETIRING_HEADER_SCALAR_V1,
            read_u64(&retiring, DirectRootStateLayoutV1::VERSION)?,
        ),
        TransitionInstructionV2::load_const(DIRECT_CLOSE_MAKER_ONE_SCALAR_V1, 1),
        TransitionInstructionV2::scalar_eq(
            DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1,
            DIRECT_CLOSE_MAKER_EXPECTED_SELECTOR_SCALAR_V1,
        ),
        TransitionInstructionV2::scalar_eq(
            DIRECT_CLOSE_MAKER_ROOT_MAGIC_SCALAR_V1,
            DIRECT_CLOSE_MAKER_EXPECTED_MAGIC_SCALAR_V1,
        ),
        // The close is legal only INSIDE Retiring -- the exact inverse of
        // begin-retiring's Open-header expectation, and the reachability
        // ordering the Lean model always specified.
        TransitionInstructionV2::scalar_eq(
            DIRECT_CLOSE_MAKER_ROOT_HEADER_SCALAR_V1,
            DIRECT_CLOSE_MAKER_RETIRING_HEADER_SCALAR_V1,
        ),
        // A drained root refuses here, in release content: `rootClosable`'s
        // zero is not this route's to produce twice.
        TransitionInstructionV2::nonzero(DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1),
        // The missing decrement, authored by the release.
        TransitionInstructionV2::sub_into(
            DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1,
            DIRECT_CLOSE_MAKER_ONE_SCALAR_V1,
            DIRECT_CLOSE_MAKER_POST_COUNT_SCALAR_V1,
        ),
    ];
    let width = transition_program_v2_bytes(instructions.len())
        .map_err(|_| DirectCloseMakerBundleErrorV1::Transition)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        TransitionRegisterGeometryV2 {
            scalars: DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1,
            identities: DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectCloseMakerBundleErrorV1::Transition)?;
    Ok(output)
}

fn build_effect() -> Result<Vec<u8>, DirectCloseMakerBundleErrorV1> {
    let instructions = [
        EffectInstructionV2::write_u64(
            DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            root_offset(DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT)?,
            DIRECT_CLOSE_MAKER_POST_COUNT_SCALAR_V1,
        ),
        EffectInstructionV2::require_lamports_eq(
            DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1,
            DIRECT_CLOSE_MAKER_ROOT_LAMPORTS_SCALAR_V1,
        ),
    ];
    let width = effect_program_v2_bytes(instructions.len())
        .map_err(|_| DirectCloseMakerBundleErrorV1::Effect)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v2_atomic(
        EffectGeometryV2 {
            accounts: 1,
            scalars: DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1,
            identities: DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1,
            request_bytes: 0,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectCloseMakerBundleErrorV1::Effect)?;
    Ok(output)
}

fn root_offset(tail_offset: usize) -> Result<u32, DirectCloseMakerBundleErrorV1> {
    u32::try_from(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(tail_offset)
            .ok_or(DirectCloseMakerBundleErrorV1::Geometry)?,
    )
    .map_err(|_| DirectCloseMakerBundleErrorV1::Geometry)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DirectCloseMakerBundleErrorV1> {
    Ok(u64::from_le_bytes(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(DirectCloseMakerBundleErrorV1::Geometry)?,
            )
            .ok_or(DirectCloseMakerBundleErrorV1::Geometry)?
            .try_into()
            .map_err(|_| DirectCloseMakerBundleErrorV1::Geometry)?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectCloseMakerBundleErrorV1> {
    ContentId::new(bytes).map_err(|_| DirectCloseMakerBundleErrorV1::Geometry)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput, execute_atomic};

    fn ordinary() -> DirectInlineOrdinaryHotBundleV4 {
        crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests()
    }

    fn bundle() -> DirectCloseMakerBundleV1 {
        build_direct_close_maker_bundle_v1(DirectCloseMakerBundleInputV1 {
            ordinary: &ordinary(),
            capacity_profile: [0x44; 32],
        })
        .expect("close-maker bundle")
    }

    fn run_transition(
        transition: TransitionProgramV2<'_>,
        scalars: &mut [u64],
    ) -> core::result::Result<(), ()> {
        let identities = [[0_u8; 32]; 2];
        let mut scratch_scalars = scalars.to_vec();
        let mut scratch_identities = identities;
        let mut output_scalars = scalars.to_vec();
        let mut output_identities = identities;
        execute_atomic(
            transition,
            RegisterInput {
                scalars,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .map_err(|_| ())?;
        scalars.copy_from_slice(&output_scalars);
        Ok(())
    }

    fn registers(header: u64, count: u64) -> [u64; 10] {
        let mut scalars = [0_u64; 10];
        scalars[usize::from(DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1)] =
            u64::from(DIRECT_CLOSE_MAKER_SELECTOR_V1);
        scalars[usize::from(DIRECT_CLOSE_MAKER_ROOT_MAGIC_SCALAR_V1)] =
            DirectRootStateLayoutV1::MAGIC_WORD;
        scalars[usize::from(DIRECT_CLOSE_MAKER_ROOT_HEADER_SCALAR_V1)] = header;
        scalars[usize::from(DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1)] = count;
        scalars
    }

    fn retiring_header() -> u64 {
        let retiring = DirectRootStateV1::new()
            .begin_retiring()
            .expect("retiring")
            .encode();
        read_u64(&retiring, DirectRootStateLayoutV1::VERSION).expect("header word")
    }

    fn open_header() -> u64 {
        read_u64(
            &DirectRootStateV1::new().encode(),
            DirectRootStateLayoutV1::VERSION,
        )
        .expect("header word")
    }

    #[test]
    fn exact_close_bundle_inherits_release_coordinates() {
        let ordinary = ordinary();
        let input = DirectCloseMakerBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_close_maker_bundle_v1(input).expect("close-maker bundle");
        validate_direct_close_maker_bundle_v1(&bundle, input).expect("validate");
        let ordinary_descriptor = CapabilityProgramV4::decode(&ordinary.descriptor).expect("V4");
        let close_descriptor = CapabilityProgramV1::decode(&bundle.descriptor).expect("V1");
        assert_eq!(close_descriptor.kind(), ordinary_descriptor.kind());
        assert_eq!(
            close_descriptor.request_schema().to_bytes(),
            DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1
        );
        assert_eq!(
            close_descriptor.root_state_bytes(),
            ordinary_descriptor.root_state_bytes()
        );
    }

    /// The released transition IS the missing decrement, and it decrements by
    /// exactly one, only inside Retiring, only from a nonzero count.
    #[test]
    fn the_released_transition_decrements_by_exactly_one_inside_retiring() {
        let bundle = bundle();
        let transition = TransitionProgramV2::decode(&bundle.transition).expect("transition");
        let mut scalars = registers(retiring_header(), 3);
        run_transition(transition, &mut scalars).expect("retiring count 3");
        assert_eq!(
            scalars[usize::from(DIRECT_CLOSE_MAKER_POST_COUNT_SCALAR_V1)],
            2
        );
    }

    /// Mutation witnesses: an Open header, a drained count, and a foreign
    /// selector each refuse in release content, before any executable opinion.
    #[test]
    fn open_header_drained_count_and_foreign_selector_refuse_in_release_content() {
        let bundle = bundle();
        let transition = TransitionProgramV2::decode(&bundle.transition).expect("transition");

        let mut open = registers(open_header(), 3);
        assert!(run_transition(transition, &mut open).is_err(), "Open root");

        let mut drained = registers(retiring_header(), 0);
        assert!(
            run_transition(transition, &mut drained).is_err(),
            "count zero"
        );

        let mut foreign = registers(retiring_header(), 3);
        foreign[usize::from(DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1)] =
            u64::from(crate::retirement_v1::DIRECT_BEGIN_RETIRING_SELECTOR_V1);
        assert!(
            run_transition(transition, &mut foreign).is_err(),
            "foreign selector"
        );
    }

    #[test]
    fn substituted_profile_effect_and_descriptor_refuse() {
        let ordinary = ordinary();
        let input = DirectCloseMakerBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_close_maker_bundle_v1(input).expect("close-maker bundle");

        let mut profile = bundle.clone();
        *profile.account_profile.last_mut().expect("profile byte") ^= 1;
        assert!(validate_direct_close_maker_bundle_v1(&profile, input).is_err());

        let mut effect = bundle.clone();
        *effect.effect.last_mut().expect("effect byte") ^= 1;
        assert!(validate_direct_close_maker_bundle_v1(&effect, input).is_err());

        let mut transition = bundle.clone();
        *transition.transition.last_mut().expect("transition byte") ^= 1;
        assert!(validate_direct_close_maker_bundle_v1(&transition, input).is_err());

        assert_eq!(
            validate_direct_close_maker_bundle_v1(
                &bundle,
                DirectCloseMakerBundleInputV1 {
                    ordinary: &ordinary,
                    capacity_profile: [0x45; 32],
                },
            ),
            Err(DirectCloseMakerBundleErrorV1::Ordinary)
        );
    }
}
