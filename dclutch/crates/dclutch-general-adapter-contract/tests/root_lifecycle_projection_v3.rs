//! The root-lifecycle conjunct, joined to a real composite capability root.
//!
//! `2e890d4` put the conjunct in the shared transition prelude and proved it
//! refuses every non-`Active` value of `scalar::ROOT_LIFECYCLE_OBSERVATION`,
//! seventy cases across seven actions and two runtime widths. Those cases WRITE
//! that register by hand. That leaves one link argued rather than executed: does
//! the real `AccountProfileV2` projection, run over a real composite capability
//! root account, actually put the root's own lifecycle byte there?
//!
//! It does, and this is where that is checked. The root here is the same shape
//! the Trading activation seam creates -- `CapabilityRootHeaderV1` followed by a
//! `GeneralRootV2` tail composed by `general_root_creation_tail_v2` -- built
//! through `initialize_root_account_v1`, at the exact 360-byte width General's
//! own Profile13 rule for coordinate zero declares.
//!
//! What is still NOT executed, and the reason is worth keeping precise: this
//! runs the projection and the fold directly rather than through
//! `hot_v3::process_hot_execution_v3` on a real ELF. A General Hot bundle in
//! `programs/dclutch-trading-sbf/program-test` needs the Hot38 frame, the seal,
//! the ALT and five ELFs -- the Direct analogue is roughly four thousand lines
//! of fixture support in `program-test/direct-hot` -- and none of it exists for
//! General yet.

use dclutch_account_profile_contract::{
    AccountObservationV1,
    v2::{
        AccountProfileV2, PhysicalAccountDataGeometryV2, ProjectionRegistersV2,
        project_dynamic_fixed_spans_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1, CapabilityRootHeaderV1,
    initialize_root_account_v1,
};
use dclutch_core_contract::ContentId;
use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3,
    },
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, GENERAL_HOT_COMMON_SCALARS_V3,
        GENERAL_HOT_ITEM_SCALAR_STRIDE_V3, identity, scalar,
    },
    release_v3::GENERAL_ACTIONS_V3,
    transition_artifacts_v3::{
        GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3, encode_general_transition_program_v3_atomic,
        general_transition_instruction_count_v3, general_transition_program_bytes_v3,
    },
};
use dclutch_general_codec::Action;
use dclutch_general_config_contract::{
    GENERAL_ROOT_BYTES_V2, GeneralLifecycleV2, GeneralRootV2, root::general_root_creation_tail_v2,
};
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use dclutch_transition_vm::v3::{ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic};

/// Release-selected external widths; none of them can move an account count.
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

/// Product-authenticated runtime width this fixture projects at.
const TAIL_COUNT: u32 = 1;
/// Trading-owned authenticated scratch pages the dynamic span expands to.
const SCRATCH_PAGES: u32 = 3;
/// The Market this capability belongs to.
const MARKET: [u8; 32] = [0x21; 32];
/// The executing Trading program, which owns every General runtime account.
///
/// `Close` anchors its primary state's owner against `identity::TRADING_PROGRAM`
/// explicitly, because it destroys that account rather than creating it, so this
/// has to be a real value in both the frame and the identity bank.
const TRADING_PROGRAM: [u8; 32] = [0x71; 32];
/// Immutable Market occurrence generation.
const GENERATION: u64 = 7;

fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content")
}

/// One exact composite capability root at the given General lifecycle.
///
/// This is the account the activation seam creates: the immutable common
/// header, then the family tail `general_root_creation_tail_v2` composes. A
/// retired capability differs from a live one ONLY inside that tail, which is
/// exactly why the header can never be the admission argument.
fn composite_root(lifecycle: GeneralLifecycleV2) -> (Vec<u8>, Vec<u8>) {
    let config_id = content([0x15; 32]);
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        content([0x16; 32]),
        content([0x11; 32]),
        content([0x17; 32]),
        config_id,
    )
    .expect("selection");
    let header = CapabilityRootHeaderV1::new(content([0x26; 32]), MARKET, GENERATION, selection)
        .expect("root header");

    let mut root = GeneralRootV2::decode(
        &general_root_creation_tail_v2(MARKET, config_id.to_bytes(), GENERATION)
            .expect("creation tail"),
    )
    .expect("the seam's own tail decodes");
    match lifecycle {
        GeneralLifecycleV2::Active => {}
        GeneralLifecycleV2::Retiring => {
            root.begin_retiring(root.revision())
                .expect("begin retiring");
        }
        GeneralLifecycleV2::Retired => {
            root.begin_retiring(root.revision())
                .expect("begin retiring");
            root.retire(root.revision()).expect("retire");
        }
    }
    assert_eq!(root.lifecycle(), lifecycle);

    let descriptor = descriptor_bytes();
    let program = CapabilityProgramV1::decode(&descriptor).expect("descriptor");
    let mut account = vec![0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2];
    initialize_root_account_v1(&mut account, header, program, &root.to_bytes())
        .expect("composite root");
    (account, descriptor)
}

/// The minimum descriptor `initialize_root_account_v1` needs to place a tail.
///
/// Only `root_state_bytes` is load-bearing here: it is what makes the composite
/// account 360 bytes, which is the width General's own Profile13 rule for the
/// root coordinate declares.
fn descriptor_bytes() -> Vec<u8> {
    use dclutch_capability_program_contract::{
        CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
        CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
        CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1,
        CAPABILITY_PROGRAM_PROFILE_OFFSET, CAPABILITY_PROGRAM_PROFILE_V2,
        CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    };
    use dclutch_transition_vm::v2::encode::{
        RegisterGeometryV2, TransitionInstructionV2, encode_transition_program_v2_atomic,
        transition_program_v2_bytes,
    };

    let instructions = [TransitionInstructionV2::load_const(0, 1)];
    let width = transition_program_v2_bytes(instructions.len()).expect("transition width");
    let mut scratch = vec![0_u8; width];
    let mut transition = vec![0_u8; width];
    encode_transition_program_v2_atomic(
        RegisterGeometryV2 {
            scalars: 1,
            identities: 1,
        },
        &instructions,
        &mut scratch,
        &mut transition,
    )
    .expect("descriptor transition");

    let mut output = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
    put(&mut output, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
    put(&mut output, 8, &1_u16.to_le_bytes());
    put(
        &mut output,
        CAPABILITY_PROGRAM_PROFILE_OFFSET,
        &CAPABILITY_PROGRAM_PROFILE_V2.to_le_bytes(),
    );
    for offset in [
        CAPABILITY_PROGRAM_KIND_OFFSET,
        CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
        CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET,
    ] {
        put(&mut output, offset, &[0x33; 32]);
    }
    put(
        &mut output,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        &u32::try_from(GENERAL_ROOT_BYTES_V2)
            .expect("tail width")
            .to_le_bytes(),
    );
    put(&mut output, CAPABILITY_PROGRAM_HEADER_BYTES_V1, &transition);
    CapabilityProgramV1::decode(&output).expect("descriptor decodes");
    output
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let end = offset.checked_add(source.len()).expect("fixture width");
    output
        .get_mut(offset..end)
        .expect("fixture destination")
        .copy_from_slice(source);
}

/// Exact Profile13 bytes for one action, from the artifact's one author.
fn account_profile(action: Action) -> Vec<u8> {
    let bytes = general_account_profile_bytes_v3(action).expect("profile width");
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(action, WIDTHS, &mut scratch, &mut output)
        .expect("Profile13 artifact");
    output
}

/// Exact emitted `TransitionProgramV3` bytes for one action.
fn transition_program(action: Action) -> Vec<u8> {
    let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
    let mut instructions =
        vec![GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3; prelude + item + epilogue];
    let bytes = general_transition_program_bytes_v3(action).expect("transition width");
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_transition_program_v3_atomic(
        action,
        &mut instructions,
        &mut scratch,
        &mut output,
    )
    .expect("transition artifact");
    output
}

/// One materialised runtime frame: per-coordinate bytes, keys, owners and the
/// profile's own alias partition.
struct ObservedFrame {
    /// Account data per logical coordinate, aliases sharing their representative's.
    data: Vec<Vec<u8>>,
    /// Account key per logical coordinate, derived from the representative.
    keys: Vec<[u8; 32]>,
    /// Account owner per logical coordinate.
    owners: Vec<[u8; 32]>,
    /// The representative each logical coordinate resolves to.
    representatives: Vec<usize>,
}

/// One observation per logical coordinate the action's Profile13 declares.
///
/// The widths, privileges, executability and ALIAS PARTITION all come from the
/// profile and its own rule generator. A route alias is a second logical name
/// for one physical account, so its observation is derived from its
/// representative's coordinate -- same key, same bytes, same privileges --
/// which is exactly what `validate_aliases` requires and what a runtime adapter
/// materialises.
fn observed_data(action: Action, profile: AccountProfileV2<'_>, root: &[u8]) -> ObservedFrame {
    let logical = profile
        .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[SCRATCH_PAGES])
        .expect("logical count");
    let mut data = Vec::with_capacity(logical);
    let mut keys = Vec::with_capacity(logical);
    let mut owners = Vec::with_capacity(logical);
    let mut representatives: Vec<usize> = Vec::with_capacity(logical);
    for coordinate in 0..logical {
        let representative = profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[SCRATCH_PAGES], coordinate)
            .expect("representative");
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(
                TAIL_COUNT,
                &[SCRATCH_PAGES],
                profile
                    .physical_account_ordinal_with_dynamic_spans(
                        TAIL_COUNT,
                        &[SCRATCH_PAGES],
                        coordinate,
                    )
                    .expect("physical ordinal"),
            )
            .expect("geometry");
        let width = exact_width(geometry.data());
        data.push(if representative == 0 {
            assert_eq!(
                width,
                root.len(),
                "the root coordinate declares the composite width"
            );
            root.to_vec()
        } else {
            vec![0x5a_u8; width]
        });
        keys.push(coordinate_key(representative));
        owners.push(TRADING_PROGRAM);
        representatives.push(representative);
    }
    let _ = action;
    ObservedFrame {
        data,
        keys,
        owners,
        representatives,
    }
}

/// The one live width a physical geometry admits.
fn exact_width(geometry: PhysicalAccountDataGeometryV2) -> usize {
    match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => bytes,
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => live_bytes,
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            minimum_bytes
        }
        PhysicalAccountDataGeometryV2::Opaque => 0,
    }
}

fn coordinate_key(coordinate: usize) -> [u8; 32] {
    let mut key = [0x40_u8; 32];
    key.get_mut(0..2)
        .expect("key prefix")
        .copy_from_slice(&u16::try_from(coordinate).expect("coordinate").to_le_bytes());
    key
}

fn scalar_width() -> usize {
    usize::try_from(GENERAL_HOT_COMMON_SCALARS_V3 + TAIL_COUNT * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
        .expect("scalar width")
}

fn identity_width() -> usize {
    usize::try_from(GENERAL_HOT_COMMON_IDENTITIES_V3).expect("identity width")
}

fn register(coordinate: u32) -> usize {
    usize::try_from(coordinate).expect("register coordinate")
}

/// Project one real composite root through one action's real Profile13.
///
/// Returns the projected scalar bank, which is exactly what the runtime feeds
/// the emitted `TransitionProgramV3`.
fn project(action: Action, root: &[u8]) -> Vec<u64> {
    let profile_bytes = account_profile(action);
    let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13 decodes");
    let frame = observed_data(action, profile, root);
    let accounts = frame
        .data
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            let representative = *frame.representatives.get(index).expect("representative");
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(
                    TAIL_COUNT,
                    &[SCRATCH_PAGES],
                    profile
                        .physical_account_ordinal_with_dynamic_spans(
                            TAIL_COUNT,
                            &[SCRATCH_PAGES],
                            index,
                        )
                        .expect("physical ordinal"),
                )
                .expect("geometry");
            let privileges = geometry.privileges();
            // The adapter-authenticated bit is an opt-in the profile's own
            // prestate demands, and it belongs to the REPRESENTATIVE only: an
            // alias carrying it is refused, which is the point of it being a
            // separate constructor.
            let variable = matches!(
                geometry.data(),
                PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { .. }
            ) && representative == index;
            let key = frame.keys.get(index).expect("key");
            let owner = frame.owners.get(index).expect("owner");
            if variable {
                AccountObservationV1::new_adapter_authenticated_variable_data(
                    key,
                    owner,
                    1,
                    bytes,
                    privileges.signer(),
                    privileges.writable(),
                    privileges.executable(),
                )
            } else {
                AccountObservationV1::new(
                    key,
                    owner,
                    1,
                    bytes,
                    privileges.signer(),
                    privileges.writable(),
                    privileges.executable(),
                )
            }
        })
        .collect::<Vec<_>>();

    let mut input_scalars = vec![0_u64; scalar_width()];
    *input_scalars
        .get_mut(register(scalar::INPUT_SCRATCH_PAGE_COUNT))
        .expect("page count register") = u64::from(SCRATCH_PAGES);
    let mut input_identities = vec![[0_u8; 32]; identity_width()];
    *input_identities
        .get_mut(register(identity::TRADING_PROGRAM))
        .expect("trading program register") = TRADING_PROGRAM;
    let mut scratch_scalars = vec![0_u64; scalar_width()];
    let mut scratch_identities = vec![[0_u8; 32]; identity_width()];
    let mut output_scalars = vec![0_u64; scalar_width()];
    let mut output_identities = vec![[0_u8; 32]; identity_width()];
    project_dynamic_fixed_spans_atomic(
        profile,
        TAIL_COUNT,
        &[SCRATCH_PAGES],
        &accounts,
        ProjectionRegistersV2 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
    )
    .expect("the real projection accepts the real frame");
    output_scalars
}

/// The missing link, executed: a real root's lifecycle byte reaches the register
/// the conjunct reads.
///
/// The seventy-case fold in `transition_artifacts_v3` writes
/// `scalar::ROOT_LIFECYCLE_OBSERVATION` by hand. Nothing checked that the real
/// AccountProfile projection puts the real root's byte there -- and admission
/// cannot check it either, because `AccountProfileV2::operation` is private, so
/// the artifact's operation list is unreadable from the outside. This is the
/// only place the join is observable, and it holds for every action and every
/// lifecycle a `GeneralRootV2` can be in.
#[test]
fn the_real_projection_carries_a_real_roots_lifecycle_into_the_conjuncts_register() {
    for lifecycle in [
        GeneralLifecycleV2::Active,
        GeneralLifecycleV2::Retiring,
        GeneralLifecycleV2::Retired,
    ] {
        let (root, _) = composite_root(lifecycle);
        for action in GENERAL_ACTIONS_V3 {
            let projected = project(action, &root);
            assert_eq!(
                projected
                    .get(register(scalar::ROOT_LIFECYCLE_OBSERVATION))
                    .copied(),
                Some(u64::from(lifecycle.tag())),
                "{action:?} did not project the root's own lifecycle",
            );
        }
    }
}

/// The zombie refusal, driven by a projection instead of a hand-written bank.
///
/// Two claims, and the second is what makes the first mean the right thing:
///
/// 1. Every action's emitted transition refuses the bank the real projection
///    produced from a `Retiring` or `Retired` composite root.
/// 2. That bank differs from the `Active` one at EXACTLY ONE register --
///    `scalar::ROOT_LIFECYCLE_OBSERVATION` -- and patching that one register
///    back to `Active` reproduces the `Active` projection exactly. A live and a
///    retired capability are otherwise indistinguishable to the whole
///    runtime-width path, which is the property `2e890d4` had to introduce a
///    conjunct to break.
///
/// Together with `the_active_bank_passes_the_lifecycle_conjunct_for_every_action`
/// in `transition_artifacts_v3` -- which shows an otherwise-complete bank
/// accepting at `Active` and refusing at `Retired` -- the refusal below is the
/// lifecycle conjunct and not an unmet prelude requirement.
#[test]
fn every_action_refuses_a_projected_retiring_or_retired_root() {
    let (active_root, _) = composite_root(GeneralLifecycleV2::Active);
    for lifecycle in [GeneralLifecycleV2::Retiring, GeneralLifecycleV2::Retired] {
        let (root, _) = composite_root(lifecycle);
        for action in GENERAL_ACTIONS_V3 {
            let bytes = transition_program(action);
            let program = ProgramV3::decode(&bytes).expect("transition decodes");
            let projected = project(action, &root);
            assert!(
                fold(program, &projected).is_err(),
                "{action:?} accepted a {lifecycle:?} capability root",
            );

            let active = project(action, &active_root);
            let mut patched = projected.clone();
            *patched
                .get_mut(register(scalar::ROOT_LIFECYCLE_OBSERVATION))
                .expect("lifecycle register") = u64::from(GeneralLifecycleV2::Active.tag());
            assert_eq!(
                patched, active,
                "{action:?} at {lifecycle:?} differed from Active somewhere other than the \
                 lifecycle register",
            );
        }
    }
}

/// Run one emitted transition over a projected bank exactly as the runtime does.
fn fold(program: ProgramV3<'_>, scalars: &[u64]) -> Result<(), ()> {
    let identities = vec![[0_u8; 32]; identity_width()];
    let mut scalar_scratch = vec![0_u64; scalars.len()];
    let mut identity_scratch = vec![[0_u8; 32]; identity_width()];
    let mut scalar_output = vec![0_u64; scalars.len()];
    let mut identity_output = vec![[0_u8; 32]; identity_width()];
    execute_fold_atomic(
        program,
        TAIL_COUNT,
        RegisterInput {
            scalars,
            identities: &identities,
        },
        RegisterOutput {
            scalars: &mut scalar_scratch,
            identities: &mut identity_scratch,
        },
        RegisterOutput {
            scalars: &mut scalar_output,
            identities: &mut identity_output,
        },
    )
    .map_err(|_| ())
}
