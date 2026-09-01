//! Canonical Direct capability-activation artifacts.
//!
//! Activation is the one Core-signed action that CREATES the Direct capability
//! root. Its V1 descriptor lives in the same ProgramSet as ordinary execution,
//! begin-retiring, and native-close, and inherits their manifest-selected kind,
//! config, capacity, root schema, derivation policy, and root width. What is
//! unique to activation is the effect: it composes the exact initial
//! [`DirectRootStateV1`] tail into its request buffer (the outer prepends the
//! immutable `CapabilityRootHeaderV1` and writes the concatenation as the new
//! root account), and it moves the funding ledger's parked rent quote into the
//! vacant root so the created account is rent-exempt.
//!
//! This is the artifact whose absence left every founded Direct market
//! unactivatable — the Direct family's own instance of the missing activation
//! ProgramSet entry `docs/OMISSION_INDEX.md` records (there for General's
//! seven-action set; here it is Direct's fourth entry). Nothing about the root
//! layout is restated here: the
//! magic and the version/phase/reserved header word are read out of
//! `DirectRootStateV1::new().encode()` and loaded as transition constants, so a
//! layout change moves this artifact with it or refuses.

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
use dclutch_capability_contract::{
    FundingCompartment, funding_ledger_bytes_v2, funding_ledger_remaining_offset_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1,
    activation_registers_v2::{
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
    successor::{DIRECT_ROOT_STATE_BYTES_V1, DirectRootStateLayoutV1, DirectRootStateV1},
};

/// High selector reserved for the lifecycle activation route.
///
/// Direct executable action selectors occupy the low namespace; begin-retiring
/// is `0xffff_ff00` and native-close `0xffff_ff01`. Activation takes the next
/// reserved high selector and cannot alias any executable action. It is
/// numerically equal to `DIRECT_TOKEN_SETUP_SELECTOR_V1`, which lives in a
/// different namespace entirely (Trading top-level instruction routing,
/// disambiguated by magic and width) - the same accepted precedent as
/// native-close sharing its value with the replay-setup route.
pub const DIRECT_ACTIVATION_SELECTOR_V1: u32 = 0xffff_ff02;
/// Exact activation selector-request width; the selector is canonical `u32` at 12.
pub const DIRECT_ACTIVATION_REQUEST_BYTES_V1: usize = 16;
/// Domain-separating activation selector-request magic.
pub const DIRECT_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDAC1";
/// Activation selector-request schema version.
pub const DIRECT_ACTIVATION_REQUEST_VERSION_V1: u16 = 1;
/// Finalized schema label for the lifecycle activation selector request.
pub const DIRECT_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-activation-request-v1";
/// SHA-256 of [`DIRECT_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_ACTIVATION_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0xb2, 0xd6, 0x96, 0x74, 0xd8, 0xb5, 0xcc, 0x75, 0xbb, 0x24, 0x0b, 0xa5, 0x78, 0xf1, 0x8e, 0x2b,
    0x7a, 0xaf, 0x60, 0x0b, 0xc0, 0x5e, 0x1a, 0xb5, 0x7d, 0xa7, 0xab, 0xc8, 0xae, 0xfe, 0xfa, 0x9a,
];

const ROOT_ACCOUNT: u16 = ACTIVATION_ROOT_ACCOUNT_V2;
const FUNDING_LEDGER_ACCOUNT: u16 = ACTIVATION_FIRST_FUNDING_ACCOUNT_V2;
/// The parked rent quote projected out of the funding ledger and moved to the
/// vacant root. This is the first family-owned scalar; it must not reuse a
/// seam-seeded common slot (0..8), which the outer relies on downstream.
const FUNDING_RENT_SCALAR: u16 = ACTIVATION_FIRST_FAMILY_SCALAR_V2;
/// Constant [`DirectRootStateLayoutV1::MAGIC_WORD`] loaded by the transition.
const ROOT_MAGIC_SCALAR: u16 = FUNDING_RENT_SCALAR + 1;
/// Constant version/phase/reserved header word loaded by the transition.
const ROOT_HEADER_WORD_SCALAR: u16 = ROOT_MAGIC_SCALAR + 1;
const ACTIVATION_SCALAR_COUNT: u16 = ROOT_HEADER_WORD_SCALAR + 1;
/// Activation reads only seam-seeded identities; it declares no family ones.
///
/// The narrowing is total because of the assertion beside it, not because a
/// bank of twelve looks small.
#[allow(clippy::cast_possible_truncation)]
const ACTIVATION_IDENTITY_COUNT: u16 =
    dclutch_capability_program_contract::activation_registers_v2::ACTIVATION_COMMON_IDENTITIES_V2
        as u16;
const _: () = assert!(
    dclutch_capability_program_contract::activation_registers_v2::ACTIVATION_COMMON_IDENTITIES_V2
        < 0x1_0000
);
/// The root being created and the sole selected funding ledger.
const ACTIVATION_ACCOUNT_COUNT: u16 = 2;
/// The founding provisions exactly one Rent compartment row in the ledger.
const FUNDING_LEDGER_SLOT_COUNT: u16 = 1;

/// Chain-selected ordinary release facts inherited by activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectActivationBundleInputV1<'a> {
    /// Complete ordinary bundle whose manifest-bound coordinates are inherited.
    pub ordinary: &'a DirectInlineOrdinaryHotBundleV4,
    /// Exact manifest-selected capacity-profile identity.
    pub capacity_profile: [u8; 32],
}

/// Three finalized activation records and their exact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectActivationBundleV1 {
    /// Exact two-account AccountProfileV1 record.
    pub account_profile: Vec<u8>,
    /// Embedded TransitionVM V2 bytes, published here for audit evidence.
    pub transition: Vec<u8>,
    /// Exact request-composing EffectProgramV2 record.
    pub effect: Vec<u8>,
    /// Exact CapabilityProgramV1 activation descriptor record.
    pub descriptor: Vec<u8>,
    /// SHA-256 identity of `account_profile`.
    pub account_profile_id: [u8; 32],
    /// SHA-256 identity of `effect`.
    pub effect_id: [u8; 32],
    /// SHA-256 identity of `descriptor`.
    pub descriptor_id: [u8; 32],
}

/// Stable activation construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectActivationBundleErrorV1 {
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
    /// Activation selector request bytes were noncanonical.
    Request,
}

/// Encode the sole canonical lifecycle-activation selector request.
pub fn direct_activation_request_v1() -> [u8; DIRECT_ACTIVATION_REQUEST_BYTES_V1] {
    let mut output = [0_u8; DIRECT_ACTIVATION_REQUEST_BYTES_V1];
    output[..8].copy_from_slice(&DIRECT_ACTIVATION_REQUEST_MAGIC_V1);
    output[8..10].copy_from_slice(&DIRECT_ACTIVATION_REQUEST_VERSION_V1.to_le_bytes());
    output[12..16].copy_from_slice(&DIRECT_ACTIVATION_SELECTOR_V1.to_le_bytes());
    output
}

/// Hostile-check one exact lifecycle-activation selector request.
pub fn validate_direct_activation_request_v1(
    bytes: &[u8],
) -> Result<(), DirectActivationBundleErrorV1> {
    if bytes != direct_activation_request_v1() {
        return Err(DirectActivationBundleErrorV1::Request);
    }
    Ok(())
}

/// Schema used to finalize the activation AccountProfile record.
pub const fn direct_activation_account_profile_schema_v1() -> [u8; 32] {
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
}

/// Schema used to finalize the activation EffectProgram record.
pub const fn direct_activation_effect_schema_v1() -> [u8; 32] {
    EFFECT_PROGRAM_SCHEMA_ID_V2
}

/// Schema used to finalize the activation descriptor record.
pub const fn direct_activation_descriptor_schema_v1() -> [u8; 32] {
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
}

/// Build the canonical Direct activation descriptor/profile/effect bundle.
pub fn build_direct_activation_bundle_v1(
    input: DirectActivationBundleInputV1<'_>,
) -> Result<DirectActivationBundleV1, DirectActivationBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectActivationBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectActivationBundleErrorV1::Ordinary)?;
    let account_profile = build_account_profile()?;
    let transition = build_transition()?;
    let effect = build_effect()?;
    let account_profile_id = digest(&account_profile);
    let effect_id = digest(&effect);
    let descriptor_width = capability_program_v1_bytes(transition.len())
        .map_err(|_| DirectActivationBundleErrorV1::Descriptor)?;
    let mut descriptor_scratch = vec![0_u8; descriptor_width];
    let mut descriptor = vec![0_u8; descriptor_width];
    encode_capability_program_v1_atomic(
        CapabilityProgramInputV1 {
            kind: ordinary.kind(),
            config_schema: ordinary.config_schema(),
            request_schema: content(DIRECT_ACTIVATION_REQUEST_SCHEMA_ID_V1)?,
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
    .map_err(|_| DirectActivationBundleErrorV1::Descriptor)?;
    let output = DirectActivationBundleV1 {
        account_profile,
        transition,
        effect,
        descriptor_id: digest(&descriptor),
        descriptor,
        account_profile_id,
        effect_id,
    };
    validate_direct_activation_bundle_v1(&output, input)?;
    Ok(output)
}

/// Hostile-decode and join one activation bundle to its ordinary release.
pub fn validate_direct_activation_bundle_v1(
    bundle: &DirectActivationBundleV1,
    input: DirectActivationBundleInputV1<'_>,
) -> Result<(), DirectActivationBundleErrorV1> {
    validate_direct_inline_ordinary_hot_bundle_v4(input.ordinary, input.capacity_profile)
        .map_err(|_| DirectActivationBundleErrorV1::Ordinary)?;
    let ordinary = CapabilityProgramV4::decode(&input.ordinary.descriptor)
        .map_err(|_| DirectActivationBundleErrorV1::Ordinary)?;
    if bundle.account_profile_id != digest(&bundle.account_profile)
        || bundle.effect_id != digest(&bundle.effect)
        || bundle.descriptor_id != digest(&bundle.descriptor)
    {
        return Err(DirectActivationBundleErrorV1::Descriptor);
    }
    let expected_profile = build_account_profile()?;
    if bundle.account_profile != expected_profile {
        return Err(DirectActivationBundleErrorV1::AccountProfile);
    }
    let profile = AccountProfileV1::decode_selected(
        bundle.account_profile_id,
        digest(&bundle.account_profile),
        &bundle.account_profile,
    )
    .map_err(|_| DirectActivationBundleErrorV1::AccountProfile)?;
    if profile.account_count() != ACTIVATION_ACCOUNT_COUNT
        || profile.scalar_count() != ACTIVATION_SCALAR_COUNT
        || profile.identity_count() != ACTIVATION_IDENTITY_COUNT
    {
        return Err(DirectActivationBundleErrorV1::AccountProfile);
    }
    let expected_transition = build_transition()?;
    if bundle.transition != expected_transition {
        return Err(DirectActivationBundleErrorV1::Transition);
    }
    let transition = TransitionProgramV2::decode(&bundle.transition)
        .map_err(|_| DirectActivationBundleErrorV1::Transition)?;
    if transition.scalar_count() != ACTIVATION_SCALAR_COUNT
        || transition.identity_count() != ACTIVATION_IDENTITY_COUNT
    {
        return Err(DirectActivationBundleErrorV1::Transition);
    }
    let expected_effect = build_effect()?;
    if bundle.effect != expected_effect {
        return Err(DirectActivationBundleErrorV1::Effect);
    }
    let effect = EffectProgramV2::decode(&bundle.effect)
        .map_err(|_| DirectActivationBundleErrorV1::Effect)?;
    if effect.account_count() != ACTIVATION_ACCOUNT_COUNT
        || effect.scalar_count() != ACTIVATION_SCALAR_COUNT
        || effect.identity_count() != ACTIVATION_IDENTITY_COUNT
        || usize::from(effect.request_bytes()) != DIRECT_ROOT_STATE_BYTES_V1
        || u32::from(effect.request_bytes()) != ordinary.root_state_bytes()
    {
        return Err(DirectActivationBundleErrorV1::Effect);
    }
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| DirectActivationBundleErrorV1::Descriptor)?;
    if descriptor.kind() != ordinary.kind()
        || descriptor.config_schema() != ordinary.config_schema()
        || descriptor.request_schema().to_bytes() != DIRECT_ACTIVATION_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema() != ordinary.root_schema()
        || descriptor.account_profile().to_bytes() != bundle.account_profile_id
        || descriptor.derivation_policy() != ordinary.derivation_policy()
        || descriptor.capacity_profile() != ordinary.capacity_profile()
        || descriptor.capacity_profile().to_bytes() != input.capacity_profile
        || descriptor.effect_schema().to_bytes() != bundle.effect_id
        || descriptor.root_state_bytes() != ordinary.root_state_bytes()
        || descriptor.transition_program().bytes() != bundle.transition
    {
        return Err(DirectActivationBundleErrorV1::Descriptor);
    }
    Ok(())
}

fn build_account_profile() -> Result<Vec<u8>, DirectActivationBundleErrorV1> {
    let rules = [
        // The composite root: vacant and System-owned at activation, credited
        // by the funding transfer and allocated/assigned by the outer's commit.
        // A vacant account has zero data, so the rule declares length zero.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: 0,
        },
        // The selected Trading FundingLedger: debited of its parked rent quote,
        // and rewritten in place by the outer's own activation commit. The
        // write-data permission is what the outer reads back to recognise this
        // as the ledger it may activate.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(
                funding_ledger_bytes_v2(FUNDING_LEDGER_SLOT_COUNT)
                    .map_err(|_| DirectActivationBundleErrorV1::Geometry)?,
            )
            .map_err(|_| DirectActivationBundleErrorV1::Geometry)?,
        },
    ];
    let rent_quote_offset = u32::try_from(
        funding_ledger_remaining_offset_v2(0, FundingCompartment::Rent)
            .map_err(|_| DirectActivationBundleErrorV1::Geometry)?,
    )
    .map_err(|_| DirectActivationBundleErrorV1::Geometry)?;
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: ROOT_ACCOUNT,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: FUNDING_LEDGER_ACCOUNT,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        // An EffectProgram has no arithmetic over account data, so the parked
        // rent the transfer must move is projected here out of the ledger's
        // Rent compartment into the family scalar the effect reads.
        AccountOperationInputV1::ProjectDataU64 {
            account: FUNDING_LEDGER_ACCOUNT,
            data_offset: rent_quote_offset,
            destination: FUNDING_RENT_SCALAR,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len())
        .map_err(|_| DirectActivationBundleErrorV1::AccountProfile)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v1_atomic(
        &rules,
        &operations,
        RegisterGeometryV1 {
            scalars: ACTIVATION_SCALAR_COUNT,
            identities: ACTIVATION_IDENTITY_COUNT,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectActivationBundleErrorV1::AccountProfile)?;
    Ok(output)
}

fn build_transition() -> Result<Vec<u8>, DirectActivationBundleErrorV1> {
    // The two words the initial tail needs that no account carries and no seam
    // register seeds: the magic and the version/phase/reserved header word.
    // Both are read out of the canonical initial state so the layout is never
    // restated. The `open_maker_root_count` word is left to the zero-initialised
    // request buffer, which is exactly its initial value.
    let initial = DirectRootStateV1::new().encode();
    let header_word = read_u64(&initial, DirectRootStateLayoutV1::VERSION)?;
    let instructions = [
        TransitionInstructionV2::load_const(ROOT_MAGIC_SCALAR, DirectRootStateLayoutV1::MAGIC_WORD),
        TransitionInstructionV2::load_const(ROOT_HEADER_WORD_SCALAR, header_word),
    ];
    let width = transition_program_v2_bytes(instructions.len())
        .map_err(|_| DirectActivationBundleErrorV1::Transition)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        TransitionRegisterGeometryV2 {
            scalars: ACTIVATION_SCALAR_COUNT,
            identities: ACTIVATION_IDENTITY_COUNT,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectActivationBundleErrorV1::Transition)?;
    Ok(output)
}

fn build_effect() -> Result<Vec<u8>, DirectActivationBundleErrorV1> {
    let magic_offset = u32::try_from(DirectRootStateLayoutV1::MAGIC)
        .map_err(|_| DirectActivationBundleErrorV1::Geometry)?;
    let header_offset = u32::try_from(DirectRootStateLayoutV1::VERSION)
        .map_err(|_| DirectActivationBundleErrorV1::Geometry)?;
    let instructions = [
        // Move the ledger's parked rent quote into the vacant root; the outer
        // requires the root to end at exactly its rent-exempt minimum.
        EffectInstructionV2::transfer_lamports(
            FUNDING_LEDGER_ACCOUNT,
            ROOT_ACCOUNT,
            FUNDING_RENT_SCALAR,
        ),
        // Compose the initial DirectRootStateV1 tail into the request buffer.
        EffectInstructionV2::write_request_u64(magic_offset, ROOT_MAGIC_SCALAR),
        EffectInstructionV2::write_request_u64(header_offset, ROOT_HEADER_WORD_SCALAR),
    ];
    let width = effect_program_v2_bytes(instructions.len())
        .map_err(|_| DirectActivationBundleErrorV1::Effect)?;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v2_atomic(
        EffectGeometryV2 {
            accounts: ACTIVATION_ACCOUNT_COUNT,
            scalars: ACTIVATION_SCALAR_COUNT,
            identities: ACTIVATION_IDENTITY_COUNT,
            request_bytes: u16::try_from(DIRECT_ROOT_STATE_BYTES_V1)
                .map_err(|_| DirectActivationBundleErrorV1::Geometry)?,
        },
        &instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DirectActivationBundleErrorV1::Effect)?;
    Ok(output)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, DirectActivationBundleErrorV1> {
    Ok(u64::from_le_bytes(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(DirectActivationBundleErrorV1::Geometry)?,
            )
            .ok_or(DirectActivationBundleErrorV1::Geometry)?
            .try_into()
            .map_err(|_| DirectActivationBundleErrorV1::Geometry)?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectActivationBundleErrorV1> {
    ContentId::new(bytes).map_err(|_| DirectActivationBundleErrorV1::Geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::{
        AccountObservationV1, ProjectionRegistersV2, project_atomic,
    };
    use dclutch_capability_program_contract::CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET;
    use dclutch_effect_kernel::v2::{
        AccountInput, AccountPermission, project_with_aliases_and_requests_atomic,
    };
    use dclutch_transition_vm::v2::{
        RegisterInput as TvmRegisterInput, RegisterOutput as TvmRegisterOutput, execute_atomic,
    };

    fn ordinary() -> DirectInlineOrdinaryHotBundleV4 {
        crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests()
    }

    fn built() -> DirectActivationBundleV1 {
        let ordinary = ordinary();
        let input = DirectActivationBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        build_direct_activation_bundle_v1(input).expect("activation bundle")
    }

    #[test]
    fn request_schema_id_and_high_selector_are_frozen() {
        assert_eq!(
            digest(DIRECT_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1),
            DIRECT_ACTIVATION_REQUEST_SCHEMA_ID_V1
        );
        assert_eq!(DIRECT_ACTIVATION_SELECTOR_V1, 0xffff_ff02);
        assert_ne!(DIRECT_ACTIVATION_SELECTOR_V1, 2);
        let request = direct_activation_request_v1();
        validate_direct_activation_request_v1(&request).expect("request");
        assert_eq!(
            u32::from_le_bytes(request[12..16].try_into().expect("selector")),
            DIRECT_ACTIVATION_SELECTOR_V1
        );
        for offset in [0_usize, 8, 10, 12] {
            let mut hostile = request;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert_eq!(
                validate_direct_activation_request_v1(&hostile),
                Err(DirectActivationBundleErrorV1::Request)
            );
        }
    }

    #[test]
    fn exact_activation_bundle_inherits_release_and_binds_root_width() {
        let ordinary = ordinary();
        let input = DirectActivationBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_activation_bundle_v1(input).expect("activation bundle");
        validate_direct_activation_bundle_v1(&bundle, input).expect("validate");
        let ordinary_descriptor = CapabilityProgramV4::decode(&ordinary.descriptor).expect("V4");
        let descriptor = CapabilityProgramV1::decode(&bundle.descriptor).expect("V1");
        assert_eq!(descriptor.kind(), ordinary_descriptor.kind());
        assert_eq!(
            descriptor.config_schema(),
            ordinary_descriptor.config_schema()
        );
        assert_eq!(descriptor.root_schema(), ordinary_descriptor.root_schema());
        assert_eq!(
            descriptor.derivation_policy(),
            ordinary_descriptor.derivation_policy()
        );
        assert_eq!(
            descriptor.capacity_profile(),
            ordinary_descriptor.capacity_profile()
        );
        assert_eq!(
            descriptor.root_state_bytes(),
            ordinary_descriptor.root_state_bytes()
        );
        assert_eq!(
            descriptor.root_state_bytes() as usize,
            DIRECT_ROOT_STATE_BYTES_V1
        );
        let effect = EffectProgramV2::decode(&bundle.effect).expect("effect");
        assert_eq!(
            usize::from(effect.request_bytes()),
            DIRECT_ROOT_STATE_BYTES_V1
        );
        assert_eq!(
            direct_activation_account_profile_schema_v1(),
            ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            direct_activation_effect_schema_v1(),
            EFFECT_PROGRAM_SCHEMA_ID_V2
        );
        assert_eq!(
            direct_activation_descriptor_schema_v1(),
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
    }

    /// The brick-safety gate: run the REAL effect kernel over the built effect,
    /// fed by what the profile and transition would produce, and assert the
    /// projected request buffer decodes as exactly the canonical initial root
    /// tail and the vacant root ends rent-exempt. A wrong effect here would
    /// permanently brick every root it activates, so this exercises the actual
    /// evaluator rather than trusting the encoders.
    #[test]
    fn the_real_effect_kernel_composes_the_exact_initial_root_tail() {
        let bundle = built();
        let effect = EffectProgramV2::decode(&bundle.effect).expect("effect");
        let scalar_count = usize::from(effect.scalar_count());
        let identity_count = usize::from(effect.identity_count());

        // The founding parks exactly the root's rent-exempt minimum; the vacant
        // root holds nothing before the transfer.
        let root_rent: u64 = 2_672_640;
        let ledger_rent: u64 = 1_726_080;
        let mut scalars = vec![0_u64; scalar_count];
        scalars[usize::from(FUNDING_RENT_SCALAR)] = root_rent;
        scalars[usize::from(ROOT_MAGIC_SCALAR)] = DirectRootStateLayoutV1::MAGIC_WORD;
        scalars[usize::from(ROOT_HEADER_WORD_SCALAR)] = read_u64(
            &DirectRootStateV1::new().encode(),
            DirectRootStateLayoutV1::VERSION,
        )
        .expect("header word");
        let identities = vec![[0_u8; 32]; identity_count];
        // SelfRepresentative rules alias each coordinate to itself.
        let aliases = [ROOT_ACCOUNT, FUNDING_LEDGER_ACCOUNT];
        let accounts = [
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
            AccountInput {
                lamports: ledger_rent + root_rent,
                data_len: funding_ledger_bytes_v2(FUNDING_LEDGER_SLOT_COUNT).expect("ledger width"),
            },
        ];
        let permissions = [
            AccountPermission::new(false, true, false),
            AccountPermission::new(true, false, true),
        ];
        let mut scratch_lamports = [0_u64; 2];
        let mut output_lamports = [0_u64; 2];
        let mut scratch_request = vec![0_u8; DIRECT_ROOT_STATE_BYTES_V1];
        let mut output_request = vec![0_u8; DIRECT_ROOT_STATE_BYTES_V1];
        project_with_aliases_and_requests_atomic(
            effect,
            &scalars,
            &identities,
            &aliases,
            &accounts,
            &permissions,
            &mut scratch_lamports,
            &mut output_lamports,
            &mut scratch_request,
            &mut output_request,
        )
        .expect("effect projection");

        // The request buffer IS the root tail the outer writes: it must be the
        // canonical initial state, byte for byte, and decode as such.
        assert_eq!(
            output_request.as_slice(),
            DirectRootStateV1::new().encode().as_slice()
        );
        assert_eq!(
            DirectRootStateV1::decode(&output_request),
            Ok(DirectRootStateV1::new())
        );
        // The vacant root ends at exactly its rent-exempt minimum; the ledger is
        // drained of exactly the parked quote.
        assert_eq!(output_lamports[usize::from(ROOT_ACCOUNT)], root_rent);
        assert_eq!(
            output_lamports[usize::from(FUNDING_LEDGER_ACCOUNT)],
            ledger_rent
        );
    }

    /// The transition loads exactly the two constants the effect reads, and
    /// preserves the profile-projected rent scalar untouched.
    #[test]
    fn the_transition_loads_the_two_layout_constants() {
        let bundle = built();
        let transition = TransitionProgramV2::decode(&bundle.transition).expect("transition");
        let scalar_count = usize::from(transition.scalar_count());
        let identity_count = usize::from(transition.identity_count());
        let mut input_scalars = vec![0_u64; scalar_count];
        input_scalars[usize::from(FUNDING_RENT_SCALAR)] = 2_672_640;
        let input_identities = vec![[0_u8; 32]; identity_count];
        let mut scratch_scalars = input_scalars.clone();
        let mut scratch_identities = input_identities.clone();
        let mut output_scalars = input_scalars.clone();
        let mut output_identities = input_identities.clone();
        execute_atomic(
            transition,
            TvmRegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            TvmRegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            TvmRegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .expect("transition execute");
        assert_eq!(
            output_scalars[usize::from(ROOT_MAGIC_SCALAR)],
            DirectRootStateLayoutV1::MAGIC_WORD
        );
        assert_eq!(
            output_scalars[usize::from(ROOT_HEADER_WORD_SCALAR)],
            read_u64(
                &DirectRootStateV1::new().encode(),
                DirectRootStateLayoutV1::VERSION
            )
            .expect("header word")
        );
        // The seam-projected rent scalar survives the transition unchanged.
        assert_eq!(output_scalars[usize::from(FUNDING_RENT_SCALAR)], 2_672_640);
    }

    /// The account profile projects the ledger's parked rent quote through the
    /// real profile evaluator into the family scalar the effect reads.
    #[test]
    fn the_real_profile_projects_the_ledger_rent_into_its_scalar() {
        let bundle = built();
        let profile = AccountProfileV1::decode_selected(
            bundle.account_profile_id,
            digest(&bundle.account_profile),
            &bundle.account_profile,
        )
        .expect("profile");
        let scalar_count = usize::from(profile.scalar_count());
        let identity_count = usize::from(profile.identity_count());

        // A minimal one-row FundingLedger image whose Rent compartment carries
        // the parked quote at the offset the profile projects.
        let root_rent: u64 = 2_672_640;
        let ledger_len = funding_ledger_bytes_v2(FUNDING_LEDGER_SLOT_COUNT).expect("ledger width");
        let mut ledger = vec![0_u8; ledger_len];
        let rent_offset =
            funding_ledger_remaining_offset_v2(0, FundingCompartment::Rent).expect("rent offset");
        ledger[rent_offset..rent_offset + 8].copy_from_slice(&root_rent.to_le_bytes());

        let root_key = [0x01_u8; 32];
        let trading_program = [0x02_u8; 32];
        let ledger_key = [0x03_u8; 32];

        // Seam-seeded input registers: the root identity slot names the root,
        // the Trading program identity slot names the ledger owner.
        let mut input_scalars = vec![0_u64; scalar_count];
        let mut input_identities = vec![[0_u8; 32]; identity_count];
        input_identities[usize::from(ACTIVATION_ROOT_IDENTITY_V2)] = root_key;
        input_identities[usize::from(ACTIVATION_TRADING_PROGRAM_IDENTITY_V2)] = trading_program;

        let observations = [
            AccountObservationV1::new(&root_key, &trading_program, 0, &[], false, true, false),
            AccountObservationV1::new(
                &ledger_key,
                &trading_program,
                root_rent + 1_726_080,
                &ledger,
                false,
                true,
                false,
            ),
        ];
        let mut scratch_scalars = input_scalars.clone();
        let mut scratch_identities = input_identities.clone();
        let mut output_scalars = input_scalars.clone();
        let mut output_identities = input_identities.clone();
        project_atomic(
            profile,
            &observations,
            ProjectionRegistersV2::new(
                TvmRegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                TvmRegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut scratch_identities,
                },
                TvmRegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut output_identities,
                },
            ),
        )
        .expect("profile projection");
        assert_eq!(output_scalars[usize::from(FUNDING_RENT_SCALAR)], root_rent);
        // Nothing clobbered the seam-seeded common scalar bank (0..8).
        let _ = &mut input_scalars;
        assert!(output_scalars[..8].iter().all(|value| *value == 0));
    }

    #[test]
    fn substituted_profile_effect_and_descriptor_refuse() {
        let ordinary = ordinary();
        let input = DirectActivationBundleInputV1 {
            ordinary: &ordinary,
            capacity_profile: [0x44; 32],
        };
        let bundle = build_direct_activation_bundle_v1(input).expect("activation bundle");

        let mut profile = bundle.clone();
        *profile.account_profile.last_mut().expect("profile byte") ^= 1;
        assert!(validate_direct_activation_bundle_v1(&profile, input).is_err());

        let mut effect = bundle.clone();
        *effect.effect.last_mut().expect("effect byte") ^= 1;
        assert!(validate_direct_activation_bundle_v1(&effect, input).is_err());

        let mut descriptor = bundle.clone();
        *descriptor
            .descriptor
            .get_mut(CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET)
            .expect("descriptor profile") ^= 1;
        descriptor.descriptor_id = digest(&descriptor.descriptor);
        assert_eq!(
            validate_direct_activation_bundle_v1(&descriptor, input),
            Err(DirectActivationBundleErrorV1::Descriptor)
        );

        assert_eq!(
            validate_direct_activation_bundle_v1(
                &bundle,
                DirectActivationBundleInputV1 {
                    ordinary: &ordinary,
                    capacity_profile: [0x45; 32],
                },
            ),
            Err(DirectActivationBundleErrorV1::Ordinary)
        );
    }

    /// The template evidence gate.
    ///
    /// `dclutch-capability-activation-codec` is the family-neutral author of
    /// this artifact shape, written so that General, and any family after it,
    /// does not hand-roll a second answer to a question that bricks roots when
    /// answered wrongly. The only evidence that makes it safe to reuse is that
    /// it reproduces THIS bundle -- the one that was reviewed, sealed, and
    /// wired as the Direct release's fourth ProgramSet entry -- byte for byte
    /// in all three records, from the same inherited coordinates and Direct's
    /// own canonical initial tail.
    ///
    /// If this test ever goes red, the template has drifted from the reviewed
    /// artifact and no family may use it until it is explained.
    #[test]
    fn the_family_neutral_template_reproduces_this_sealed_bundle_byte_for_byte() {
        use dclutch_capability_activation_codec::{
            ActivationBundleInputV1, ActivationSeamImageV1, build_activation_bundle_v1,
            project_activation_root_tail_v1,
        };
        use dclutch_capability_program_contract::activation_registers_v2::{
            ACTIVATION_COMMON_IDENTITIES_V2, ACTIVATION_COMMON_SCALARS_V2,
        };

        let ordinary = ordinary();
        let mine = built();
        let descriptor = CapabilityProgramV4::decode(&ordinary.descriptor).expect("V4");
        let initial = DirectRootStateV1::new().encode();
        let template = build_activation_bundle_v1(ActivationBundleInputV1 {
            kind: descriptor.kind(),
            config_schema: descriptor.config_schema(),
            request_schema: content(DIRECT_ACTIVATION_REQUEST_SCHEMA_ID_V1).expect("request"),
            root_schema: descriptor.root_schema(),
            derivation_policy: descriptor.derivation_policy(),
            capacity_profile: descriptor.capacity_profile(),
            root_state_bytes: descriptor.root_state_bytes(),
            // Direct's tail is entirely constant; it declares no seam field.
            constant_root_tail: initial.as_slice(),
            seam_fields: &[],
            funding_ledger_slot_count: FUNDING_LEDGER_SLOT_COUNT,
            // This family funds its root with its exact Rent reserve alone.
            delivers_creation_principal: false,
        })
        .expect("template bundle");

        assert_eq!(template.account_profile, mine.account_profile);
        assert_eq!(template.transition, mine.transition);
        assert_eq!(template.effect, mine.effect);
        assert_eq!(template.descriptor, mine.descriptor);
        assert_eq!(template.account_profile_id, mine.account_profile_id);
        assert_eq!(template.effect_id, mine.effect_id);
        assert_eq!(template.descriptor_id, mine.descriptor_id);

        // And the template's published projection agrees with this crate's own
        // decoder about what the created root will contain.
        let (projected, lamports) = project_activation_root_tail_v1(
            &template,
            ActivationSeamImageV1 {
                scalars: &[0_u64; ACTIVATION_COMMON_SCALARS_V2],
                identities: &[[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2],
                rent_quote: 2_672_640,
            },
        )
        .expect("projection");
        assert_eq!(projected.as_slice(), initial.as_slice());
        assert_eq!(
            DirectRootStateV1::decode(&projected),
            Ok(DirectRootStateV1::new())
        );
        assert_eq!(lamports, [2_672_640, 0]);
    }
}
