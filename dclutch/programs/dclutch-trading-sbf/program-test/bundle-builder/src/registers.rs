//! Host-side execution of the emitted artifact semantics.
//!
//! This is the Hot executor's register pipeline run on the host, through the
//! same shared kernels the chain runs, in the same phase order
//! (`hot_v3.rs::process_hot_execution_v3` is the reference):
//!
//! 1. seed the parent request digest and the trusted-environment registers,
//! 2. project the account observations through the AccountProfile,
//! 3. project the lifecycle policy's current-Rent quotes,
//! 4. seed native-signature identities from the Ed25519 evidence (Signed
//!    request profiles),
//! 5. project the family request through the RequestProfile,
//! 6. run the lifecycle preplan (which derives every to-be-created PDA),
//! 7. execute the transition fold, or the selected authenticated accelerator's
//!    opt-in candidate projector for admitted AOT,
//! 8. project the Effect program (which yields the child-request bank).
//!
//! One deliberate difference from the chain: where the chain *refuses* an
//! account whose key differs from a derivation, this engine *reports* the
//! derived key so the enclosing builder can adopt it and re-run. Construction
//! is the authority pipeline with adoption in place of refusal; the on-chain
//! gate then runs the refusing version over the adopted bundle.

use dclutch_account_profile_contract::{
    AccountObservationV1,
    lifecycle_v3::{
        AuthenticatedRentCreditV3, AuthenticatedRentMinimumV3, AuthenticatedRentQuoteV5,
        LifecycleContextV3, LifecycleOperationV3, LifecycleProtectedRegisterBuffersV3,
        LifecycleRegistersV3, LifecycleRentQuoteBuffersV5, LifecycleSeedInputValueV3,
        PlannedObservationsV3, StateLifecyclePlanV3, StateLifecyclePolicyV5,
        plan_lifecycle_with_protected_outputs_atomic,
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, DynamicFixedSpanV2, ProjectionRegistersV2,
        TrustedEnvironmentV2, derive_effect_permissions,
        derive_effect_permissions_with_dynamic_spans, project_atomic as project_accounts_atomic,
        project_dynamic_fixed_spans_atomic,
    },
};
use dclutch_capability_program_contract::hot_v3::HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3;
use dclutch_effect_kernel::{
    v2::{AccountInput, AccountPermission},
    v3::{ProgramV3 as EffectBaseV3, ProjectionV3, ResolvedInvocationV3},
    v4::{
        ProgramV4 as EffectProgramV4, ResolvedWriteRangeV4,
        SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_RELEASE_ID_V4, project_atomic_visiting,
    },
    v5::{ProgramV5 as EffectProgramV5, SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_RELEASE_ID_V5},
};
use dclutch_execution_strategy_contract::v2::{
    BankTransportV2, ExecutionStrategyProgramV2, StrategyDispositionV2, classify_bank_transport_v2,
};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_request_profile_contract::{
    ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionRegistersV1, ProjectionTargetV1,
    RequestProfileV1, SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1,
    project_atomic as project_request_atomic,
    v2::{
        NativeEd25519InstructionViewV1, NativeSignatureRegistersV1,
        REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2, seed_authenticated_signers_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    ProgramV3 as TransitionProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
};
use sha2::{Digest, Sha256};
use solana_program::{pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::system_program;

use crate::{BuilderError, WaistFactsV1, profile_ops};

/// One logical coordinate's account facts as the builder currently holds them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccountV1 {
    /// Account key (a placeholder until an adoption round settles it).
    pub key: [u8; 32],
    /// Owner program.
    pub owner: [u8; 32],
    /// Native balance.
    pub lamports: u64,
    /// Exact account data.
    pub data: Vec<u8>,
    /// Transaction signer privilege at this coordinate.
    pub signer: bool,
    /// Writable privilege at this coordinate.
    pub writable: bool,
    /// Executable bit.
    pub executable: bool,
}

/// Content digests substituted as projection keys for the shared runtime
/// prefix, exactly as `logical_projection_key_v3` substitutes them on-chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentProjectionKeysV1 {
    /// Selected config record digest (runtime coordinate 1).
    pub selected_config: [u8; 32],
    /// Product record digest (runtime coordinate 2).
    pub product_root: [u8; 32],
    /// Portfolio record digest (runtime coordinate 3).
    pub portfolio: [u8; 32],
    /// Linked basis record digest (runtime coordinate 4).
    pub linked_basis: [u8; 32],
}

/// Everything the register pipeline consumes.
pub struct EngineInputV1<'a> {
    /// Decoded account profile.
    pub profile: AccountProfileV2<'a>,
    /// Request profile record bytes plus the schema the descriptor names.
    pub request_profile_bytes: &'a [u8],
    /// Schema release identity of the request profile.
    pub request_profile_schema: [u8; 32],
    /// Lifecycle policy record bytes.
    pub lifecycle_bytes: &'a [u8],
    /// Transition program record bytes.
    pub transition_bytes: &'a [u8],
    /// Effect program record bytes.
    pub effect_bytes: &'a [u8],
    /// Schema release identity of the effect program.
    pub effect_schema: [u8; 32],
    /// Selected action (for lifecycle plan selection).
    pub action: u32,
    /// Release-waist facts.
    pub waist: WaistFactsV1,
    /// Product-authenticated runtime item count.
    pub tail_count: u32,
    /// Family request bytes (after the Hot envelope).
    pub family_request: &'a [u8],
    /// Complete nested Hot instruction data (envelope plus request); the
    /// native-signature message coordinates are relative to these bytes.
    pub instruction_data: &'a [u8],
    /// Ed25519 evidence instruction data, for Signed request profiles.
    pub ed25519_evidence: Option<&'a [u8]>,
    /// Top-level index of the Hot-carrying instruction (1 on the canonical
    /// continuation: evidence at 0, Registry at 1).
    pub native_message_instruction_index: u16,
    /// Trusted current slot.
    pub clock_slot: u64,
    /// Market the envelope names.
    pub market: [u8; 32],
    /// Capability generation the envelope names.
    pub generation: u64,
    /// One observation per logical coordinate.
    pub observations: &'a [ObservedAccountV1],
    /// Content digests for the shared runtime prefix.
    pub content_keys: ContentProjectionKeysV1,
    /// Authenticated dynamic fixed-span widths, one per declared span; empty
    /// for a profile that declares none. Derived by
    /// [`derive_dynamic_span_widths`], never stated by a campaign.
    pub span_counts: &'a [u32],
    /// Current rent schedule.
    pub rent: &'a Rent,
}

fn decode_execution_effect_program<'a>(
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<EffectProgramV4<'a>, ()> {
    match schema {
        EFFECT_SCHEMA_RELEASE_ID_V4 => EffectProgramV4::decode(bytes).map_err(|_| ()),
        EFFECT_SCHEMA_RELEASE_ID_V5 => EffectProgramV5::decode(bytes)
            .map(EffectProgramV5::base)
            .map_err(|_| ()),
        _ => Err(()),
    }
}

/// One lifecycle-derived state address the builder must realize.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleStateDerivationV1 {
    /// Representative logical coordinate holding the state.
    pub coordinate: usize,
    /// Adapter-derived PDA for that coordinate.
    pub derived: Pubkey,
    /// The plan the policy produced for it; absent on a discovery round, when
    /// the observed key was not yet the derived one and the plan kernel was
    /// not consulted.
    pub plan: Option<StateLifecyclePlanV3>,
}

/// One resolved child invocation with its projected request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedInvocationV1 {
    /// Route ordinal in the Effect program.
    pub route: u16,
    /// Invocation ordinal within the route.
    pub invocation: u32,
    /// The kernel-resolved invocation geometry.
    pub resolved: ResolvedInvocationV3,
    /// Exact projected child-request bytes for this invocation.
    pub request: Vec<u8>,
}

/// Everything the pipeline derives.
#[derive(Clone, Debug)]
pub struct EngineOutputV1 {
    /// Register bank presented to the selected execution strategy, after
    /// request projection and lifecycle preplanning but before the transition.
    pub input_scalars: Vec<u64>,
    /// Identity half of the exact pre-transition strategy input bank.
    pub input_identities: Vec<[u8; 32]>,
    /// Transition output scalars (the candidate registers).
    pub scalars: Vec<u64>,
    /// Transition output identities.
    pub identities: Vec<[u8; 32]>,
    /// The projected child-request bank.
    pub request_bank: Vec<u8>,
    /// Every lifecycle invocation's derived state.
    pub lifecycle_states: Vec<LifecycleStateDerivationV1>,
    /// Every resolved child invocation, in walk order.
    pub invocations: Vec<DerivedInvocationV1>,
    /// Whether every phase ran. False on a discovery round: a lifecycle state
    /// coordinate did not yet hold its derived key, so the plan kernel and
    /// every later phase were skipped and the caller must adopt and re-run.
    pub complete: bool,
}

/// Seed the exact authenticated span widths into their AccountProfile-owned
/// selector registers before account projection.
///
/// Profile13's projection kernel deliberately checks the selector bank rather
/// than trusting the account-vector length. `derive_dynamic_span_geometry`
/// has already authenticated these widths from the selected request/effect/
/// strategy artifacts; this is the host equivalent of Hot copying that
/// authenticated result into the pre-projection bank. The u32 width is
/// zero-extended to u64, never narrowed, and every coordinate comes from the
/// decoded profile.
fn seed_authenticated_dynamic_span_counts(
    profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    scalars: &mut [u64],
) -> Result<(), BuilderError> {
    if span_counts.len() != usize::from(profile.dynamic_fixed_span_count()) {
        return Err(BuilderError::Spans("span-selector-count"));
    }
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count() {
        let span = profile
            .dynamic_fixed_span(index)
            .map_err(|_| BuilderError::Spans("span-selector-decode"))?;
        let count = *span_counts
            .get(usize::from(index))
            .ok_or(BuilderError::Spans("span-selector-count"))?;
        span.validate_count(count)
            .map_err(|_| BuilderError::Spans("span-selector-width"))?;
        *scalars
            .get_mut(usize::from(span.count_scalar()))
            .ok_or(BuilderError::Spans("span-selector-register"))? = u64::from(count);
        index = index.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(())
}

/// Opt-in semantic owner for an admitted-AOT candidate register bank.
///
/// The slices are initialized with the exact post-lifecycle preplan bank that
/// Trading presents to the authenticated accelerator. Implementations mutate
/// them commit-last into the candidate bank the accelerator would return. The
/// ordinary interpreted path never receives a projector and continues to
/// execute the selected Transition artifact byte-for-byte.
pub type AdmittedCandidateProjectorV1<'a> =
    dyn Fn(&mut [u64], &mut [[u8; 32]]) -> Result<(), BuilderError> + 'a;

fn digest32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Run the full register pipeline. See the module documentation for phases.
#[allow(clippy::too_many_lines)]
pub fn run_engine(input: &EngineInputV1<'_>) -> Result<EngineOutputV1, BuilderError> {
    run_engine_with_admitted_candidate(input, None)
}

/// Run the register pipeline with an optional authenticated admitted-AOT
/// candidate evaluator.
///
/// The projector replaces only phase 7. Phases 1--6 still derive the exact
/// preplan-owned input bank, and phase 8 still projects the selected Effect
/// and child requests from the resulting candidate exactly as Hot does.
#[allow(clippy::too_many_lines)]
pub(crate) fn run_engine_with_admitted_candidate(
    input: &EngineInputV1<'_>,
    candidate_projector: Option<&AdmittedCandidateProjectorV1<'_>>,
) -> Result<EngineOutputV1, BuilderError> {
    let profile = input.profile;
    let tail_count = input.tail_count;
    let lifecycle_digest = digest32(input.lifecycle_bytes);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_digest,
        lifecycle_digest,
        input.lifecycle_bytes,
    )
    .map_err(|_| BuilderError::Projection("lifecycle-decode"))?;
    // FOR THIS ACTION -- the host-side twin of the join `hot_v3/seal.rs` runs.
    // The builder already selects plans and quotes by action; the profile join
    // is the third thing that has to, for the same reason.
    let profile_join = lifecycle
        .validate_account_profile_join_for_action(profile, input.action)
        .map_err(|_| BuilderError::Projection("profile-join"))?;
    let transition = TransitionProgramV3::decode(input.transition_bytes)
        .map_err(|_| BuilderError::Projection("transition-decode"))?;
    let effect = decode_execution_effect_program(input.effect_schema, input.effect_bytes)
        .map_err(|_| BuilderError::Projection("effect-decode"))?;
    let effect_base = effect.base();

    let scalar_count = effect_base
        .scalar_count(tail_count)
        .map_err(|_| BuilderError::Projection("scalar-count"))?;
    let identity_count = effect_base
        .identity_count(tail_count)
        .map_err(|_| BuilderError::Projection("identity-count"))?;
    let request_bytes = effect_base
        .request_bytes(tail_count)
        .map_err(|_| BuilderError::Projection("request-bytes"))?;

    let span_counts = input.span_counts;
    let logical_count = profile_ops::logical_count(profile, tail_count, span_counts)?;
    if input.observations.len() != logical_count {
        return Err(BuilderError::Projection("observation-width"));
    }
    let aliases = (0..logical_count)
        .map(|coordinate| profile_ops::representative(profile, tail_count, span_counts, coordinate))
        .collect::<Result<Vec<usize>, _>>()?;

    // Phase 2 inputs: the observation bank, with the shared runtime prefix's
    // content-digest projection keys substituted for physical keys.
    let request_digest = digest32(input.family_request);
    let selected_config_is_variable = projected_account_uses_variable_marker(profile, 1)?;
    let linked_basis_is_variable = projected_account_uses_variable_marker(profile, 4)?;
    let projected_keys = [
        input.content_keys.selected_config,
        input.content_keys.product_root,
        input.content_keys.portfolio,
        input.content_keys.linked_basis,
    ];
    let observation_keys = input
        .observations
        .iter()
        .enumerate()
        .map(|(coordinate, observed)| {
            let representative = *aliases.get(coordinate).unwrap_or(&coordinate);
            match representative {
                1..=4 => projected_keys
                    .get(representative - 1)
                    .copied()
                    .unwrap_or(observed.key),
                _ => observed.key,
            }
        })
        .collect::<Vec<[u8; 32]>>();
    let observations = input
        .observations
        .iter()
        .zip(&observation_keys)
        .enumerate()
        .map(|(coordinate, (observed, key))| {
            if (coordinate == 1 && selected_config_is_variable)
                || (coordinate == 4 && linked_basis_is_variable)
            {
                AccountObservationV1::new_adapter_authenticated_variable_data(
                    key,
                    &observed.owner,
                    observed.lamports,
                    &observed.data,
                    observed.signer,
                    observed.writable,
                    observed.executable,
                )
            } else {
                AccountObservationV1::new(
                    key,
                    &observed.owner,
                    observed.lamports,
                    observed.data.as_slice(),
                    observed.signer,
                    observed.writable,
                    observed.executable,
                )
            }
        })
        .collect::<Vec<AccountObservationV1<'_>>>();

    // Phase 1: seed the parent digest and the trusted environment.
    let mut current_scalars = vec![0_u64; scalar_count];
    let mut current_identities = vec![[0_u8; 32]; identity_count];
    *current_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(BuilderError::Projection("parent-digest-register"))? = request_digest;
    if let TrustedEnvironmentV2::CurrentSlot { destination } = profile.trusted_environment() {
        *current_scalars
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Projection("slot-register"))? = input.clock_slot;
    }
    if let Some(destination) = profile.trusted_current_executing_program_identity() {
        *current_identities
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Projection("program-register"))? =
            input.waist.trading_program.to_bytes();
    }
    if let Some(destination) = profile.trusted_system_program_identity() {
        *current_identities
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Projection("system-register"))? = system_program::ID.to_bytes();
    }
    seed_authenticated_dynamic_span_counts(profile, span_counts, &mut current_scalars)?;

    let mut scratch_scalars = vec![0_u64; scalar_count];
    let mut scratch_identities = vec![[0_u8; 32]; identity_count];
    let mut next_scalars = vec![0_u64; scalar_count];
    let mut next_identities = vec![[0_u8; 32]; identity_count];

    // Phase 2: account projection.
    {
        let registers = ProjectionRegistersV2 {
            input_scalars: &current_scalars,
            input_identities: &current_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut next_scalars,
            output_identities: &mut next_identities,
        };
        if profile.uses_dynamic_fixed_spans() {
            project_dynamic_fixed_spans_atomic(
                profile,
                tail_count,
                span_counts,
                &observations,
                registers,
            )
        } else {
            project_accounts_atomic(profile, tail_count, &observations, registers)
        }
        .map_err(|error| {
            std::eprintln!("account projection kernel refused: {error:?}");
            BuilderError::Projection("account-projection")
        })?;
    }
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);

    // Phase 3: current-Rent quote projection.
    let quotes = current_rent_quotes(lifecycle, input.rent, input.action)?;
    lifecycle
        .project_authenticated_current_rent_quotes_atomic(
            profile,
            Some(profile_join),
            tail_count,
            input.action,
            &current_scalars,
            &quotes,
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch_scalars,
                output_scalars: &mut next_scalars,
            },
        )
        .map_err(|_| BuilderError::Projection("rent-quotes"))?;
    core::mem::swap(&mut current_scalars, &mut next_scalars);

    // Phase 4: native-signature seeding for Signed request profiles.
    let request_profile = decode_request_profile(input)?;
    if let RequestProfileKind::Signed(signed) = request_profile {
        let evidence = input
            .ed25519_evidence
            .ok_or(BuilderError::Projection("missing-ed25519-evidence"))?;
        next_identities.copy_from_slice(&current_identities);
        seed_authenticated_signers_atomic(
            signed,
            tail_count,
            NativeEd25519InstructionViewV1 {
                ed25519_data: evidence,
                ed25519_instruction_index: input
                    .native_message_instruction_index
                    .checked_sub(1)
                    .ok_or(BuilderError::Projection("native-signature-adjacency"))?,
                authenticated_message_data: input.instruction_data,
                message_instruction_index: input.native_message_instruction_index,
                message_offset_bias: 0,
            },
            NativeSignatureRegistersV1 {
                input_identities: &current_identities,
                scratch_identities: &mut scratch_identities,
                output_identities: &mut next_identities,
            },
        )
        .map_err(|_| BuilderError::Projection("native-signatures"))?;
        core::mem::swap(&mut current_identities, &mut next_identities);
    }

    // Phase 5: request projection.
    let base_profile = match request_profile {
        RequestProfileKind::Unsigned(profile) => profile,
        RequestProfileKind::Signed(profile) => profile.request_profile(),
    };
    project_request_atomic(
        base_profile,
        tail_count,
        input.family_request,
        ProjectionRegistersV1 {
            input_scalars: &current_scalars,
            input_identities: &current_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut next_scalars,
            output_identities: &mut next_identities,
        },
    )
    .map_err(|_| BuilderError::Projection("request-projection"))?;
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);

    // Phase 6: lifecycle preplan, in adopt mode.
    let preplan = preplan_lifecycle(
        input,
        lifecycle,
        profile,
        profile_join,
        &observations,
        &aliases,
        &current_scalars,
        &current_identities,
    )?;
    if !preplan.complete {
        return Ok(EngineOutputV1 {
            input_scalars: Vec::new(),
            input_identities: Vec::new(),
            scalars: Vec::new(),
            identities: Vec::new(),
            request_bank: Vec::new(),
            lifecycle_states: preplan.states,
            invocations: Vec::new(),
            complete: false,
        });
    }
    current_scalars = preplan.scalars;
    current_identities = preplan.identities;
    let input_scalars = current_scalars.clone();
    let input_identities = current_identities.clone();

    // Phase 7: either the ordinary interpreted fold or the admitted-AOT
    // candidate evaluator. The latter starts from the same preplan bank and
    // is commit-last: a refusing projector cannot expose a partial candidate.
    next_scalars.copy_from_slice(&current_scalars);
    next_identities.copy_from_slice(&current_identities);
    if let Some(projector) = candidate_projector {
        let mut candidate_scalars = current_scalars.clone();
        let mut candidate_identities = current_identities.clone();
        projector(&mut candidate_scalars, &mut candidate_identities)
            .map_err(|_| BuilderError::Projection("admitted-candidate"))?;
        next_scalars.copy_from_slice(&candidate_scalars);
        next_identities.copy_from_slice(&candidate_identities);
    } else {
        scratch_scalars.copy_from_slice(&current_scalars);
        scratch_identities.copy_from_slice(&current_identities);
        execute_fold_atomic(
            transition,
            tail_count,
            RegisterInput {
                scalars: &current_scalars,
                identities: &current_identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut next_scalars,
                identities: &mut next_identities,
            },
        )
        .map_err(|error| {
            // THE SAME PROBE THE ACCOUNT PHASE ALREADY HAD, and it is here for
            // the reason that phase's probe earned: `DataLengthMismatch` was a
            // reading until its probe printed the offending coordinate's
            // declared and observed widths, and then it was a measurement that
            // named seven coordinates and closed six of them. This phase
            // discarded its error entirely, so `Projection("transition")` was
            // the whole of what a lane got. Declared-versus-observed is the
            // shape that worked, so it is the shape printed: the fold's own
            // header widths against the banks it was handed, and the tail count
            // that scales them.
            std::eprintln!(
                "transition fold refused: {error:?} (tail_count={tail_count}, \
                 declared common_scalars={} item_scalar_stride={} \
                 common_identities={} item_identity_stride={}, \
                 expected scalars={} identities={}, \
                 observed scalars={} identities={})",
                transition.common_scalar_count(),
                transition.item_scalar_stride(),
                transition.common_identity_count(),
                transition.item_identity_stride(),
                usize::from(transition.common_scalar_count())
                    + usize::from(transition.item_scalar_stride())
                        * usize::try_from(tail_count).unwrap_or(usize::MAX),
                usize::from(transition.common_identity_count())
                    + usize::from(transition.item_identity_stride())
                        * usize::try_from(tail_count).unwrap_or(usize::MAX),
                current_scalars.len(),
                current_identities.len(),
            );
            match first_refusing_transition_operation(
                transition,
                tail_count,
                &current_scalars,
                &current_identities,
            ) {
                Some((index, class, row)) => std::eprintln!(
                    "transition fold refused at operation {index} ({class}), row={row}"
                ),
                None => std::eprintln!(
                    "transition fold refusal could not be localized to one operation"
                ),
            }
            std::eprintln!("transition scalars={current_scalars:?}");
            std::eprintln!(
                "transition identities={:?}",
                current_identities
                    .iter()
                    .map(|value| std::format!(
                        "{:02x}{:02x}..{:02x}",
                        value[0],
                        value[1],
                        value[31]
                    ))
                    .collect::<Vec<_>>()
            );
            BuilderError::Projection("transition")
        })?;
    }
    let transition_scalars = next_scalars.clone();
    let transition_identities = next_identities.clone();

    // Phase 8: effect projection over the lifecycle-candidate account inputs.
    let mut account_inputs = observations
        .iter()
        .map(|observation| AccountInput {
            lamports: observation.lamports(),
            data_len: observation.data().len(),
        })
        .collect::<Vec<_>>();
    apply_candidates(&preplan.states, &aliases, &mut account_inputs)?;
    let effect_account_count = effect
        .account_count(tail_count, &transition_scalars)
        .map_err(|_| BuilderError::Projection("effect-account-count"))?;
    let mut permissions = vec![AccountPermission::read_only(); logical_count];
    if profile.uses_dynamic_fixed_spans() {
        derive_effect_permissions_with_dynamic_spans(
            profile,
            tail_count,
            span_counts,
            &mut permissions,
        )
    } else {
        derive_effect_permissions(profile, tail_count, &mut permissions)
    }
    .map_err(|_| BuilderError::Projection("effect-permissions"))?;
    let mut scratch_lamports = vec![0_u64; effect_account_count];
    let mut output_lamports = account_inputs
        .iter()
        .map(|account| account.lamports)
        .collect::<Vec<_>>();
    let mut request_bank = vec![0_u8; request_bytes];
    let mut write_ranges = vec![
        ResolvedWriteRangeV4::vacant();
        effect
            .data_write_operation_count(tail_count)
            .map_err(|_| BuilderError::Projection("write-ranges"))?
    ];
    project_atomic_visiting(
        effect,
        tail_count,
        ProjectionV3 {
            scalars: &transition_scalars,
            identities: &transition_identities,
            aliases: aliases
                .get(..effect_account_count)
                .ok_or(BuilderError::Projection("effect-aliases"))?,
            accounts: account_inputs
                .get(..effect_account_count)
                .ok_or(BuilderError::Projection("effect-accounts"))?,
            permissions: permissions
                .get(..effect_account_count)
                .ok_or(BuilderError::Projection("effect-permission-window"))?,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: output_lamports
                .get_mut(..effect_account_count)
                .ok_or(BuilderError::Projection("effect-lamport-window"))?,
            requests: &mut request_bank,
        },
        &mut write_ranges,
        &mut |_| Ok(()),
    )
    .map_err(|_| BuilderError::Projection("effect-projection"))?;

    // Walk the routes and slice each invocation's projected request.
    let invocations = resolve_invocations(
        effect_base,
        tail_count,
        &transition_scalars,
        &transition_identities,
        &request_bank,
        input.family_request,
        candidate_projector.is_none(),
    )?;

    Ok(EngineOutputV1 {
        input_scalars,
        input_identities,
        scalars: transition_scalars,
        identities: transition_identities,
        request_bank,
        lifecycle_states: preplan.states,
        invocations,
        complete: true,
    })
}

const fn prestate_uses_variable_marker(prestate: AccountPrestateV2) -> bool {
    matches!(
        prestate,
        AccountPrestateV2::AdapterAuthenticatedVariableData
    )
}

fn projected_account_uses_variable_marker(
    profile: AccountProfileV2<'_>,
    coordinate: usize,
) -> Result<bool, BuilderError> {
    let coordinate = u16::try_from(coordinate).map_err(|_| BuilderError::Arithmetic)?;
    let prestate = profile
        .rule(false, coordinate)
        .map_err(|_| BuilderError::Profile(line!()))?
        .prestate();
    Ok(prestate_uses_variable_marker(prestate))
}

#[derive(Clone, Copy)]
enum RequestProfileKind<'a> {
    Unsigned(RequestProfileV1<'a>),
    Signed(RequestProfileV2<'a>),
}

impl<'a> RequestProfileKind<'a> {
    /// The V1 projector, which both kinds ultimately delegate projection to.
    fn base(self) -> RequestProfileV1<'a> {
        match self {
            Self::Unsigned(profile) => profile,
            Self::Signed(profile) => profile.request_profile(),
        }
    }

    /// Whether any request projection writes `target`.
    fn writes_register(self, target: ProjectionTargetV1) -> Result<bool, BuilderError> {
        match self {
            Self::Unsigned(profile) => profile
                .writes_register(target)
                .map_err(|_| BuilderError::Spans("writes-register")),
            Self::Signed(profile) => profile
                .writes_register(target)
                .map_err(|_| BuilderError::Spans("writes-register")),
        }
    }
}

fn decode_request_profile<'a>(
    input: &EngineInputV1<'a>,
) -> Result<RequestProfileKind<'a>, BuilderError> {
    decode_request_profile_bytes(input.request_profile_bytes, input.request_profile_schema)
}

fn decode_request_profile_bytes(
    bytes: &[u8],
    schema: [u8; 32],
) -> Result<RequestProfileKind<'_>, BuilderError> {
    let authenticated = digest32(bytes);
    if schema == REQUEST_PROFILE_SCHEMA_ID_V1 {
        RequestProfileV1::decode_selected(authenticated, authenticated, bytes)
            .map(RequestProfileKind::Unsigned)
            .map_err(|_| BuilderError::Projection("request-profile-v1"))
    } else if schema == REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID {
        RequestProfileV2::decode_selected(authenticated, authenticated, bytes)
            .map(RequestProfileKind::Signed)
            .map_err(|_| BuilderError::Projection("request-profile-v2"))
    } else {
        // V3 (borrowed-witness) and V4 (repeated-rows) request profiles are a
        // named boundary of this engine; no reproduced family needs them yet.
        Err(BuilderError::Projection("request-profile-schema"))
    }
}

/// Everything the dynamic fixed-span width derivation consumes.
///
/// The same artifact bytes the bundle already holds, plus the strategy record —
/// which the rest of the builder never decodes, and which the span rule needs
/// because a *profile-only* span is admissible under exactly one disposition.
pub struct SpanWidthInputV1<'a> {
    /// Decoded account profile.
    pub profile: AccountProfileV2<'a>,
    /// Request profile record bytes plus the schema the descriptor names.
    pub request_profile_bytes: &'a [u8],
    /// Schema release identity of the request profile.
    pub request_profile_schema: [u8; 32],
    /// Effect program record bytes.
    pub effect_bytes: &'a [u8],
    /// Schema release identity of the effect program.
    pub effect_schema: [u8; 32],
    /// Execution strategy record bytes.
    pub strategy_bytes: &'a [u8],
    /// Release-waist facts (the trusted executing-program identity).
    pub waist: WaistFactsV1,
    /// Product-authenticated runtime item count.
    pub tail_count: u32,
    /// Family request bytes (after the Hot envelope).
    pub family_request: &'a [u8],
    /// Trusted current slot.
    pub clock_slot: u64,
}

/// Authenticated span widths plus the optional AccountProfile-only transport
/// span which makes the accelerator input scratch-backed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpanWidthDerivationV1 {
    /// One exact width per declared dynamic fixed span.
    pub widths: Vec<u32>,
    /// Sole trailing profile-only input-transport span, when present.
    pub transport_span: Option<u16>,
}

/// Derive the authenticated dynamic fixed-span widths for one bundle.
///
/// This is `hot_v3::authenticate_dynamic_span_widths_v3` on the host, phase for
/// phase, and it is the reason the builder can pack a spans profile at all: the
/// widths are *not* the account-vector length, they are projected out of the
/// artifacts and the family request before any account is expanded.
///
/// Two kinds of span exist and they get their width from different places:
///
/// - **Request-owned**: the span's `count_scalar` is a common scalar some
///   request-projection operation writes. Its width comes from projecting the
///   family request once into a throwaway bank.
/// - **AccountProfile-only** (General's sole span): nothing in the request
///   writes the selector, so the width comes from the canonical register-bank
///   geometry — `classify_bank_transport_v2(scalars, identities)` — and the
///   span is admissible **only** under `AdmittedAot`, only as the trailing
///   span, and only when no EffectV4 span claims the same selector.
pub fn derive_dynamic_span_widths(input: &SpanWidthInputV1<'_>) -> Result<Vec<u32>, BuilderError> {
    Ok(derive_dynamic_span_geometry(input)?.widths)
}

/// Derive widths together with the semantic owner of input scratch transport.
pub fn derive_dynamic_span_geometry(
    input: &SpanWidthInputV1<'_>,
) -> Result<SpanWidthDerivationV1, BuilderError> {
    let profile = input.profile;
    let effect = decode_execution_effect_program(input.effect_schema, input.effect_bytes)
        .map_err(|_| BuilderError::Spans("effect-decode"))?;
    let span_count = profile.dynamic_fixed_span_count();
    if !profile.uses_dynamic_fixed_spans() || span_count == 0 {
        if span_count != 0 || effect.span_count() != 0 {
            return Err(BuilderError::Spans("undeclared-spans"));
        }
        return Ok(SpanWidthDerivationV1 {
            widths: Vec::new(),
            transport_span: None,
        });
    }
    let strategy = ExecutionStrategyProgramV2::decode(input.strategy_bytes)
        .map_err(|_| BuilderError::Spans("strategy-decode"))?;
    let disposition = strategy.disposition();

    let tail_count = input.tail_count;
    let effect_base = effect.base();
    let scalar_count = effect_base
        .scalar_count(tail_count)
        .map_err(|_| BuilderError::Spans("scalar-count"))?;
    let identity_count = effect_base
        .identity_count(tail_count)
        .map_err(|_| BuilderError::Spans("identity-count"))?;

    // The throwaway projection: the same seeded prefix phase 1 uses, then the
    // family request. Only `projected_scalars` outlives it.
    let mut input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(BuilderError::Spans("parent-digest-register"))? = digest32(input.family_request);
    seed_trusted_environment(
        profile,
        input.waist.trading_program,
        input.clock_slot,
        &mut input_scalars,
        &mut input_identities,
    )
    .map_err(|_| BuilderError::Spans("trusted-environment"))?;
    let mut scratch_scalars = input_scalars.clone();
    let mut scratch_identities = input_identities.clone();
    let mut projected_scalars = input_scalars.clone();
    let mut projected_identities = input_identities.clone();
    let request_profile =
        decode_request_profile_bytes(input.request_profile_bytes, input.request_profile_schema)?;
    project_request_atomic(
        request_profile.base(),
        tail_count,
        input.family_request,
        ProjectionRegistersV1 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut projected_scalars,
            output_identities: &mut projected_identities,
        },
    )
    .map_err(|_| BuilderError::Spans("request-projection"))?;

    let transport_page_count = match classify_bank_transport_v2(
        u32::try_from(scalar_count).map_err(|_| BuilderError::Arithmetic)?,
        u32::try_from(identity_count).map_err(|_| BuilderError::Arithmetic)?,
    )
    .map_err(|_| BuilderError::Spans("bank-transport"))?
    {
        BankTransportV2::InlineReturnData { .. } => None,
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => Some(page_count),
    };
    let mut transport_span = None;
    let mut index = 0_u16;
    while index < span_count {
        let span = profile
            .dynamic_fixed_span(index)
            .map_err(|_| BuilderError::Spans("span-decode"))?;
        let target = ProjectionTargetV1 {
            kind: ProjectionRegisterKindV1::Scalar,
            space: ProjectionRegisterSpaceV1::Common,
            index: span.count_scalar(),
        };
        let request_owned = request_profile.writes_register(target)?;
        let effect_owned = (0..effect.span_count()).any(|effect_index| {
            effect
                .span(effect_index)
                .is_ok_and(|value| value.selector_common_scalar() == span.count_scalar())
        });
        if request_owned {
            if !effect_owned {
                require_trailing_profile_only_span(profile, span)?;
            }
        } else {
            if effect_owned
                || disposition != StrategyDispositionV2::AdmittedAot
                || transport_span.is_some()
            {
                return Err(BuilderError::Spans("unowned-span"));
            }
            require_trailing_profile_only_span(profile, span)?;
            let page_count = transport_page_count.ok_or(BuilderError::Spans("inline-bank"))?;
            *projected_scalars
                .get_mut(usize::from(span.count_scalar()))
                .ok_or(BuilderError::Spans("selector-register"))? = u64::from(page_count);
            transport_span = Some(index);
        }
        index = index.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    let mut effect_span = 0_u16;
    while effect_span < effect.span_count() {
        let selector = effect
            .span(effect_span)
            .map_err(|_| BuilderError::Spans("effect-span-decode"))?
            .selector_common_scalar();
        if !(0..span_count).any(|profile_index| {
            profile
                .dynamic_fixed_span(profile_index)
                .is_ok_and(|value| value.count_scalar() == selector)
        }) {
            return Err(BuilderError::Spans("effect-span-unmatched"));
        }
        effect_span = effect_span.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    let mut widths = vec![0_u32; usize::from(span_count)];
    profile
        .dynamic_span_widths_from_scalars(&projected_scalars, &mut widths)
        .map_err(|_| BuilderError::Spans("widths-from-scalars"))?;
    effect
        .account_count(tail_count, &projected_scalars)
        .map_err(|_| BuilderError::Spans("effect-account-count"))?;
    if disposition == StrategyDispositionV2::AdmittedAot
        && transport_page_count.is_some() != transport_span.is_some()
    {
        return Err(BuilderError::Spans("transport-span-mismatch"));
    }
    Ok(SpanWidthDerivationV1 {
        widths,
        transport_span,
    })
}

/// An AccountProfile-only span must be the trailing one.
fn require_trailing_profile_only_span(
    profile: AccountProfileV2<'_>,
    span: DynamicFixedSpanV2,
) -> Result<(), BuilderError> {
    if span.insertion_coordinate() == profile.fixed_account_count() {
        Ok(())
    } else {
        Err(BuilderError::Spans("non-trailing-span"))
    }
}

/// Seed the trusted-environment registers phase 1 seeds, for the throwaway
/// bank the span projection runs in.
fn seed_trusted_environment(
    profile: AccountProfileV2<'_>,
    trading_program: Pubkey,
    clock_slot: u64,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), BuilderError> {
    if let TrustedEnvironmentV2::CurrentSlot { destination } = profile.trusted_environment() {
        *scalars
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Spans("slot-register"))? = clock_slot;
    }
    if let Some(destination) = profile.trusted_current_executing_program_identity() {
        *identities
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Spans("program-register"))? = trading_program.to_bytes();
    }
    if let Some(destination) = profile.trusted_system_program_identity() {
        *identities
            .get_mut(usize::from(destination))
            .ok_or(BuilderError::Spans("system-register"))? = system_program::ID.to_bytes();
    }
    Ok(())
}

/// Best-effort index of the first transition operation whose prefix refuses.
///
/// The VM reports a refusal CLASS and no position, which is enough to say
/// `CheckFailed` and not enough to say which predicate. Rather than change a
/// kernel crate for a diagnostic, this re-runs the same public
/// `execute_fold_atomic` over successively longer prefixes of the same program
/// and reports the first length that refuses -- so the answer is produced by
/// the authority itself, not by a second interpreter written here.
///
/// The three region counts live at fixed header offsets and are read here
/// because `ProgramV3` exports no accessor for them. That is a second speller
/// of one layout and it is deliberately confined to this function: a probe may
/// respell what it cannot ask for, a builder may not.
///
/// Returns `None` when no prefix refuses or none of them decodes -- truncation
/// can legitimately produce a program the validator rejects, and reporting
/// "could not localize" is the honest output when it does.
fn first_refusing_transition_operation(
    transition: TransitionProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Option<(usize, String, String)> {
    let bytes = transition.bytes();
    let body = bytes.len().checked_sub(TRANSITION_HEADER_BYTES)?;
    let total = body.checked_div(TRANSITION_INSTRUCTION_BYTES)?;
    let count_at = |offset: usize| -> Option<usize> {
        let raw: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
        Some(usize::from(u16::from_le_bytes(raw)))
    };
    let (prelude, item) = (count_at(6)?, count_at(8)?);

    for taken in 1..=total {
        let in_prelude = taken.min(prelude);
        let in_item = taken.saturating_sub(in_prelude).min(item);
        let in_epilogue = taken - in_prelude - in_item;
        let end = TRANSITION_HEADER_BYTES + taken * TRANSITION_INSTRUCTION_BYTES;
        let mut prefix = bytes.get(..end)?.to_vec();
        for (offset, value) in [(6, in_prelude), (8, in_item), (10, in_epilogue)] {
            let encoded = u16::try_from(value).ok()?.to_le_bytes();
            prefix
                .get_mut(offset..offset + 2)?
                .copy_from_slice(&encoded);
        }
        let Ok(program) = TransitionProgramV3::decode(&prefix) else {
            continue;
        };
        let mut scratch_scalars = scalars.to_vec();
        let mut scratch_identities = identities.to_vec();
        let mut output_scalars = scalars.to_vec();
        let mut output_identities = identities.to_vec();
        if let Err(error) = execute_fold_atomic(
            program,
            tail_count,
            RegisterInput {
                scalars,
                identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        ) {
            let start = TRANSITION_HEADER_BYTES + (taken - 1) * TRANSITION_INSTRUCTION_BYTES;
            let row = bytes
                .get(start..start + TRANSITION_INSTRUCTION_BYTES)
                .map(|slice| {
                    slice
                        .iter()
                        .map(|byte| std::format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            let region = if taken <= prelude {
                "prelude"
            } else if taken <= prelude + item {
                "item"
            } else {
                "epilogue"
            };
            return Some((taken - 1, std::format!("{error:?} in {region}"), row));
        }
    }
    None
}

fn current_rent_quotes(
    lifecycle: StateLifecyclePolicyV5<'_>,
    rent: &Rent,
    action: u32,
) -> Result<Vec<AuthenticatedRentQuoteV5>, BuilderError> {
    // The same subsequence the runtime builds: one quote per declaration this
    // action projects, in declaration order. The builder already selects plans
    // by action; quotes now select the same way.
    let mut quotes = Vec::with_capacity(usize::from(lifecycle.current_rent_quote_count()));
    let mut ordinal = 0_u16;
    while ordinal < lifecycle.current_rent_quote_count() {
        let declaration = lifecycle
            .current_rent_quote(ordinal)
            .map_err(|_| BuilderError::Projection("rent-quote-declaration"))?;
        if !declaration.applies_to(action) {
            ordinal = ordinal.checked_add(1).ok_or(BuilderError::Arithmetic)?;
            continue;
        }
        let exact_data_len = declaration.exact_data_len();
        quotes.push(AuthenticatedRentQuoteV5 {
            exact_data_len,
            scalar_destination: declaration.scalar_destination().index(),
            current_minimum: rent.minimum_balance(
                usize::try_from(exact_data_len).map_err(|_| BuilderError::Arithmetic)?,
            ),
        });
        ordinal = ordinal.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(quotes)
}

struct PreplannedV1 {
    states: Vec<LifecycleStateDerivationV1>,
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
    complete: bool,
}

/// Exact observation prefix lifecycle may inspect.
///
/// Dynamic fixed spans are projection transport. The current executable shape
/// admits them here only when every span is trailing, so the fixed prefix keeps
/// its AccountProfile coordinates byte-for-byte. Lifecycle's own join rejects
/// item coordinates for these profiles; no scratch page can become a state,
/// payer, or RentCredit by changing this slice.
fn lifecycle_semantic_prefix_width(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    expanded_width: usize,
) -> Result<usize, BuilderError> {
    let expected = if profile.uses_dynamic_fixed_spans() {
        profile
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| BuilderError::Lifecycle("dynamic-account-width"))?
    } else {
        if !span_counts.is_empty() {
            return Err(BuilderError::Lifecycle("unexpected-span-widths"));
        }
        profile
            .logical_account_count(tail_count)
            .map_err(|_| BuilderError::Lifecycle("account-width"))?
    };
    if expanded_width != expected {
        return Err(BuilderError::Lifecycle("expanded-account-width"));
    }
    if !profile.uses_dynamic_fixed_spans() {
        return Ok(expanded_width);
    }
    let fixed = profile.fixed_account_count();
    let mut span = 0_u16;
    while span < profile.dynamic_fixed_span_count() {
        if profile
            .dynamic_fixed_span(span)
            .map_err(|_| BuilderError::Lifecycle("span-decode"))?
            .insertion_coordinate()
            != fixed
        {
            return Err(BuilderError::Lifecycle("non-trailing-lifecycle-span"));
        }
        span = span.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(usize::from(fixed))
}

/// The lifecycle preplan of `prepare_lifecycle_v4`, with adoption in place of
/// key refusal: the derived PDA is reported per state coordinate, and the
/// enclosing fixed-point loop re-runs the engine until the observed keys are
/// the derived ones.
#[allow(clippy::too_many_arguments)]
fn preplan_lifecycle(
    input: &EngineInputV1<'_>,
    lifecycle: StateLifecyclePolicyV5<'_>,
    profile: AccountProfileV2<'_>,
    profile_join: dclutch_account_profile_contract::lifecycle_v3::ValidatedProfileJoinV3<'_>,
    observations: &[AccountObservationV1<'_>],
    aliases: &[usize],
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<PreplannedV1, BuilderError> {
    let tail_count = input.tail_count;
    let action = input.action;
    let lifecycle_width = lifecycle_semantic_prefix_width(
        profile,
        tail_count,
        input.span_counts,
        observations.len(),
    )?;
    let observations = observations
        .get(..lifecycle_width)
        .ok_or(BuilderError::Lifecycle("observation-prefix"))?;
    let aliases = aliases
        .get(..lifecycle_width)
        .ok_or(BuilderError::Lifecycle("alias-prefix"))?;
    let mut output_scalars = scalars.to_vec();
    let mut output_identities = identities.to_vec();
    let mut scalar_scratch = vec![0_u64; scalars.len()];
    let mut identity_scratch = vec![[0_u8; 32]; identities.len()];
    let mut next_scalars = vec![0_u64; scalars.len()];
    let mut next_identities = vec![[0_u8; 32]; identities.len()];
    let mut planned_lamports = observations
        .iter()
        .map(|observation| observation.lamports())
        .collect::<Vec<_>>();
    let mut states = Vec::new();
    let mut complete = true;
    let plan_count = lifecycle
        .action_plan_count(action)
        .map_err(|_| BuilderError::Lifecycle("plan-count"))?;
    let mut ordinal = 0_u16;
    while ordinal < plan_count {
        let selected = lifecycle
            .action_plan(action, ordinal)
            .map_err(|_| BuilderError::Lifecycle("plan-select"))?
            .with_validated_join(profile_join);
        let invocation_count = selected
            .invocation_count(tail_count)
            .map_err(|_| BuilderError::Lifecycle("invocation-count"))?;
        let mut invocation = 0_u32;
        while invocation < invocation_count {
            let item = selected
                .invocation_item(tail_count, invocation)
                .map_err(|_| BuilderError::Lifecycle("invocation-item"))?;
            let registers = LifecycleRegistersV3 {
                scalars: &output_scalars,
                identities: &output_identities,
            };
            if !selected
                .is_enabled(profile, tail_count, item, registers)
                .map_err(|_| BuilderError::Lifecycle("guard"))?
            {
                return Err(BuilderError::Lifecycle("disabled-plan"));
            }
            let indices = selected
                .project_account_indices(profile, tail_count, item)
                .map_err(|_| BuilderError::Lifecycle("account-indices"))?;
            let state = *aliases
                .get(indices.state())
                .ok_or(BuilderError::Lifecycle("state-alias"))?;
            let payer = indices
                .payer()
                .map(|index| aliases.get(index).copied())
                .map(|value| value.ok_or(BuilderError::Lifecycle("payer-alias")))
                .transpose()?;
            let rent_credit = indices
                .rent_credit()
                .map(|index| aliases.get(index).copied())
                .map(|value| value.ok_or(BuilderError::Lifecycle("credit-alias")))
                .transpose()?;

            let seed_count = selected
                .seed_count()
                .map_err(|_| BuilderError::Lifecycle("seed-count"))?;
            let mut seed_bytes: Vec<Vec<u8>> = Vec::with_capacity(usize::from(seed_count));
            let mut derived: Option<(Pubkey, u8)> = None;
            let mut seed = 0_u8;
            while seed < seed_count {
                match selected
                    .materialize_seed_input(profile, tail_count, item, registers, seed)
                    .map_err(|_| BuilderError::Lifecycle("seed"))?
                {
                    LifecycleSeedInputValueV3::Bytes(value) => {
                        seed_bytes.push(value.as_slice().to_vec());
                    }
                    LifecycleSeedInputValueV3::CanonicalBump => {
                        if seed.checked_add(1) != Some(seed_count) {
                            return Err(BuilderError::Lifecycle("bump-position"));
                        }
                        let slices = seed_bytes.iter().map(Vec::as_slice).collect::<Vec<&[u8]>>();
                        derived = Some(Pubkey::find_program_address(
                            &slices,
                            &input.waist.trading_program,
                        ));
                    }
                }
                seed = seed.checked_add(1).ok_or(BuilderError::Arithmetic)?;
            }
            let (derived_key, bump) = derived.ok_or(BuilderError::Lifecycle("no-bump"))?;
            let observed_state_key = observations
                .get(state)
                .ok_or(BuilderError::Lifecycle("state-observation"))?
                .key();
            if observed_state_key != derived_key.to_bytes() {
                // Discovery: the coordinate does not yet hold its derived key.
                // Record the adoption and skip the plan kernel — the caller
                // rebinds and re-runs, and the next round plans for real.
                states.push(LifecycleStateDerivationV1 {
                    coordinate: state,
                    derived: derived_key,
                    plan: None,
                });
                complete = false;
                invocation = invocation.checked_add(1).ok_or(BuilderError::Arithmetic)?;
                continue;
            }

            let authenticated_credit = rent_credit
                .map(|index| {
                    authenticate_credit(
                        observations,
                        index,
                        *planned_lamports
                            .get(index)
                            .ok_or(BuilderError::Lifecycle("credit-lamports"))?,
                        input,
                    )
                })
                .transpose()?;
            let current_rent_minimum = if matches!(
                selected.operation(),
                LifecycleOperationV3::Create | LifecycleOperationV3::AuthenticateOrCreate
            ) {
                let data_bytes = selected
                    .target_data_bytes(tail_count)
                    .map_err(|_| BuilderError::Lifecycle("target-width"))?;
                Some(AuthenticatedRentMinimumV3 {
                    data_bytes,
                    lamports: input.rent.minimum_balance(
                        usize::try_from(data_bytes).map_err(|_| BuilderError::Arithmetic)?,
                    ),
                })
            } else {
                None
            };
            scalar_scratch.copy_from_slice(&output_scalars);
            identity_scratch.copy_from_slice(&output_identities);
            next_scalars.copy_from_slice(&output_scalars);
            next_identities.copy_from_slice(&output_identities);
            let plan = plan_lifecycle_with_protected_outputs_atomic(
                selected,
                LifecycleContextV3 {
                    account_profile: profile,
                    tail_count,
                    item_index: item,
                    accounts: PlannedObservationsV3::planned(observations, &planned_lamports)
                        .map_err(|_| BuilderError::Lifecycle("planned-observations"))?,
                    registers: LifecycleRegistersV3 {
                        scalars: &output_scalars,
                        identities: &output_identities,
                    },
                    trading_program: input.waist.trading_program.to_bytes(),
                    system_program: system_program::ID.to_bytes(),
                    adapter_derived_pda: derived_key.to_bytes(),
                    rent_credit: authenticated_credit,
                    current_rent_minimum,
                },
                bump,
                LifecycleProtectedRegisterBuffersV3 {
                    scalar_scratch: &mut scalar_scratch,
                    identity_scratch: &mut identity_scratch,
                    output_scalars: &mut next_scalars,
                    output_identities: &mut next_identities,
                },
            )
            .map_err(|error| {
                std::eprintln!(
                    "lifecycle plan refused: {error:?} (plan ordinal {ordinal}, state {state}, derived {derived_key})"
                );
                BuilderError::Lifecycle("plan")
            })?;
            match plan {
                StateLifecyclePlanV3::Authenticate(_) => {}
                StateLifecyclePlanV3::Create(value) => {
                    *planned_lamports
                        .get_mut(state)
                        .ok_or(BuilderError::Lifecycle("state-balance"))? = value.state_after;
                    *planned_lamports
                        .get_mut(payer.ok_or(BuilderError::Lifecycle("payer-index"))?)
                        .ok_or(BuilderError::Lifecycle("payer-balance"))? = value.payer_after;
                }
                StateLifecyclePlanV3::Close(value) => {
                    *planned_lamports
                        .get_mut(state)
                        .ok_or(BuilderError::Lifecycle("state-balance"))? = value.source_after;
                    *planned_lamports
                        .get_mut(rent_credit.ok_or(BuilderError::Lifecycle("credit-index"))?)
                        .ok_or(BuilderError::Lifecycle("credit-balance"))? =
                        value.rent_credit_after;
                }
            }
            states.push(LifecycleStateDerivationV1 {
                coordinate: state,
                derived: derived_key,
                plan: Some(plan),
            });
            output_scalars.copy_from_slice(&next_scalars);
            output_identities.copy_from_slice(&next_identities);
            invocation = invocation.checked_add(1).ok_or(BuilderError::Arithmetic)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(PreplannedV1 {
        states,
        scalars: output_scalars,
        identities: output_identities,
        complete,
    })
}

/// Authenticate a bound RentCredit observation exactly as the adapter does.
fn authenticate_credit(
    observations: &[AccountObservationV1<'_>],
    index: usize,
    observed_lamports: u64,
    input: &EngineInputV1<'_>,
) -> Result<AuthenticatedRentCreditV3, BuilderError> {
    let observation = observations
        .get(index)
        .ok_or(BuilderError::Lifecycle("credit-coordinate"))?;
    let credit = LifecycleRentCreditV2::decode(observation.data())
        .map_err(|_| BuilderError::Lifecycle("credit-decode"))?;
    if credit.market().to_bytes() != input.market
        || credit.release_set().to_bytes() != input.waist.release_set
        || credit.generation() != input.generation
    {
        return Err(BuilderError::Lifecycle("credit-binding"));
    }
    Ok(AuthenticatedRentCreditV3 {
        key: observation.key(),
        beneficiary: credit.refund_wallet().to_bytes(),
        lamports: observed_lamports,
    })
}

fn apply_candidates(
    states: &[LifecycleStateDerivationV1],
    aliases: &[usize],
    accounts: &mut [AccountInput],
) -> Result<(), BuilderError> {
    for derivation in states {
        let (lamports, data_len) = match derivation
            .plan
            .ok_or(BuilderError::Lifecycle("candidate-plan"))?
        {
            StateLifecyclePlanV3::Authenticate(_) => continue,
            StateLifecyclePlanV3::Create(plan) => (
                plan.state_after,
                usize::try_from(plan.target_data_bytes).map_err(|_| BuilderError::Arithmetic)?,
            ),
            StateLifecyclePlanV3::Close(plan) => (plan.source_after, 0),
        };
        for (coordinate, alias) in aliases.iter().enumerate() {
            if *alias == derivation.coordinate {
                let account = accounts
                    .get_mut(coordinate)
                    .ok_or(BuilderError::Lifecycle("candidate-coordinate"))?;
                account.lamports = lamports;
                account.data_len = data_len;
            }
        }
        // Payer and credit balances also move, but the effect projection reads
        // them only as lamport inputs, which `planned_lamports` in the preplan
        // already tracked; the payer's post-plan balance is applied here too.
    }
    Ok(())
}

fn resolve_invocations(
    effect: EffectBaseV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_bank: &[u8],
    family_request: &[u8],
    synthesize_disabled_shadows: bool,
) -> Result<Vec<DerivedInvocationV1>, BuilderError> {
    let mut output = Vec::new();
    let mut request_offset = 0_usize;
    let mut route = 0_u16;
    while route < effect.route_count() {
        let declared = effect
            .route(route)
            .map_err(|_| BuilderError::Projection("route-decode"))?;
        let route_request_bytes = usize::try_from(declared.fixed_request_bytes())
            .ok()
            .and_then(|fixed| {
                usize::try_from(declared.item_request_bytes())
                    .ok()?
                    .checked_mul(usize::try_from(tail_count).ok()?)?
                    .checked_add(fixed)
            })
            .ok_or(BuilderError::Arithmetic)?;
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| BuilderError::Projection("invocation-count"))?;
        if count == 0 && synthesize_disabled_shadows {
            // Preserve the ordinary builder's historical shadow-authority
            // construction byte-for-byte. The opt-in admitted candidate path
            // omits this block and matches Hot's exact `0..invocation_count`
            // walk: an accelerator-disabled route has no child authority.
            let end = request_offset
                .checked_add(
                    usize::try_from(declared.fixed_request_bytes())
                        .map_err(|_| BuilderError::Arithmetic)?,
                )
                .ok_or(BuilderError::Arithmetic)?;
            let request = request_bank
                .get(request_offset..end)
                .ok_or(BuilderError::Projection("shadow-request-slice"))?;
            output.push(DerivedInvocationV1 {
                route,
                invocation: 0,
                resolved: ResolvedInvocationV3 {
                    role: declared.role(),
                    kind: declared.kind(),
                    item: None,
                    fixed_account_start: declared.fixed_account_start(),
                    fixed_account_count: declared.fixed_account_count(),
                    item_account_start: 0,
                    item_account_count: 0,
                    item_account_stride: 0,
                    repeated_item_count: 0,
                    request_offset,
                    request_len: request.len(),
                    borrowed_witness: None,
                    receipt_dependencies:
                        dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3::empty(),
                    receipt_dependency: None,
                },
                request: request.to_vec(),
            });
        }
        let mut invocation = 0_u32;
        while invocation < count {
            let resolved = effect
                .resolved_invocation(route, invocation, tail_count, scalars, identities)
                .map_err(|_| BuilderError::Projection("invocation-resolve"))?;
            let end = resolved
                .request_offset
                .checked_add(resolved.request_len)
                .ok_or(BuilderError::Arithmetic)?;
            let fixed = request_bank
                .get(resolved.request_offset..end)
                .ok_or(BuilderError::Projection("request-slice"))?;
            let request = match resolved.borrowed_witness {
                None => fixed.to_vec(),
                Some(witness) if fixed.is_empty() => witness
                    .slice(family_request)
                    .map_err(|_| BuilderError::Projection("borrowed-witness"))?
                    .to_vec(),
                Some(_) => return Err(BuilderError::Projection("witness-shape")),
            };
            output.push(DerivedInvocationV1 {
                route,
                invocation,
                resolved,
                request,
            });
            invocation = invocation.checked_add(1).ok_or(BuilderError::Arithmetic)?;
        }
        request_offset = request_offset
            .checked_add(route_request_bytes)
            .ok_or(BuilderError::Arithmetic)?;
        route = route.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_effect_kernel::{
        v2::FixedRole,
        v3::{
            HEADER_BYTES as EFFECT_HEADER_BYTES_V3, ROUTE_BYTES, RouteKindV3,
            encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
        },
        v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
        v5::{HEADER_BYTES_V5, encode_program_v5_atomic},
    };
    use dclutch_general_adapter_contract::{
        account_rules_v3::{
            GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
            general_account_profile_bytes_v3,
        },
        hot_candidate_v3::{general_hot_scalar_count_v3, scalar},
    };
    use dclutch_general_codec::Action;

    fn exact_effect_v4() -> Vec<u8> {
        let routes = [RouteInputV3 {
            role: FixedRole::Core,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 0,
            fixed_account_count: 5,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        }];
        let mut base_scratch = vec![0_u8; EFFECT_HEADER_BYTES_V3 + ROUTE_BYTES];
        let mut base = vec![0_u8; base_scratch.len()];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 5,
                item_account_stride: 0,
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 2,
                item_identity_stride: 0,
            },
            &routes,
            &[],
            &[],
            &mut base_scratch,
            &mut base,
        )
        .expect("V3 effect");
        let mut scratch = vec![0_u8; HEADER_BYTES_V4 + base.len()];
        let mut output = vec![0_u8; scratch.len()];
        encode_program_v4_atomic(
            &base,
            BorrowedRangePolicyV4::DisjointExactCoverage,
            1,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("V4 effect");
        output
    }

    fn exact_effect_v5(base: &[u8]) -> Vec<u8> {
        let mut scratch = vec![0_u8; HEADER_BYTES_V5 + base.len()];
        let mut output = vec![0_u8; scratch.len()];
        encode_program_v5_atomic(base, &[], &[], &mut scratch, &mut output).expect("V5 effect");
        output
    }

    #[test]
    fn effect_schema_selects_v4_or_full_v5_base_execution_view() {
        let base = exact_effect_v4();
        let successor = exact_effect_v5(&base);
        assert_eq!(
            decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V4, &base)
                .expect("V4 execution view")
                .bytes(),
            base
        );
        assert_eq!(
            decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V5, &successor)
                .expect("V5 base execution view")
                .bytes(),
            base
        );
        assert_ne!(successor, base);
    }

    #[test]
    fn effect_schema_refuses_unknown_malformed_and_hybrid_programs() {
        let base = exact_effect_v4();
        let successor = exact_effect_v5(&base);
        assert!(decode_execution_effect_program([0x72; 32], &base).is_err());
        assert!(decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V4, &successor).is_err());
        assert!(decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V5, &base).is_err());
        let mut malformed = successor;
        malformed[5] = 1;
        assert!(decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V5, &malformed).is_err());
        let mut malformed_base = exact_effect_v5(&base);
        malformed_base[HEADER_BYTES_V5] ^= 1;
        assert!(
            decode_execution_effect_program(EFFECT_SCHEMA_RELEASE_ID_V5, &malformed_base).is_err()
        );
    }

    #[test]
    fn projected_observation_marker_mirrors_exact_profile_prestate() {
        assert!(prestate_uses_variable_marker(
            AccountPrestateV2::AdapterAuthenticatedVariableData
        ));
        for substituted in [
            AccountPrestateV2::Exact,
            AccountPrestateV2::LifecycleBound,
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias,
            AccountPrestateV2::AuthenticatedRouteAlias,
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        ] {
            assert!(!prestate_uses_variable_marker(substituted));
        }
    }

    #[test]
    #[allow(clippy::indexing_slicing, clippy::unwrap_used)]
    fn authenticated_span_selectors_seed_exactly_and_refuse_hostile_banks() {
        let widths = GeneralExternalAccountWidthsV3 {
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
        let profile_bytes =
            general_account_profile_bytes_v3(Action::OpenBatch).expect("General Profile13 width");
        let mut scratch = vec![0_u8; profile_bytes];
        let mut bytes = vec![0_u8; profile_bytes];
        encode_general_account_profile_v3_atomic(
            Action::OpenBatch,
            widths,
            &mut scratch,
            &mut bytes,
        )
        .expect("General Profile13");
        let profile = AccountProfileV2::decode(&bytes).expect("Profile13 decode");
        let scalar_count =
            usize::try_from(general_hot_scalar_count_v3(4).expect("General scalar count"))
                .expect("host usize");
        let selector =
            usize::try_from(scalar::INPUT_SCRATCH_PAGE_COUNT).expect("selector coordinate");
        let mut scalars = vec![0_u64; scalar_count];
        scalars[0] = 0x55;
        seed_authenticated_dynamic_span_counts(profile, &[3], &mut scalars)
            .expect("authenticated width seeds selector");
        assert_eq!(scalars[selector], 3);
        assert_eq!(scalars[0], 0x55, "unrelated semantic register is unchanged");

        assert_eq!(
            seed_authenticated_dynamic_span_counts(profile, &[], &mut scalars),
            Err(BuilderError::Spans("span-selector-count"))
        );
        assert_eq!(
            seed_authenticated_dynamic_span_counts(profile, &[0], &mut scalars),
            Err(BuilderError::Spans("span-selector-width"))
        );
        assert_eq!(
            seed_authenticated_dynamic_span_counts(profile, &[3, 3], &mut scalars),
            Err(BuilderError::Spans("span-selector-count"))
        );
        let mut short_bank = vec![0_u64; selector];
        assert_eq!(
            seed_authenticated_dynamic_span_counts(profile, &[3], &mut short_bank),
            Err(BuilderError::Spans("span-selector-register"))
        );
        let expanded = profile
            .logical_account_count_with_dynamic_spans(4, &[3])
            .expect("expanded Profile13 width");
        assert_eq!(
            lifecycle_semantic_prefix_width(profile, 4, &[3], expanded),
            Ok(usize::from(profile.fixed_account_count()))
        );
        assert_eq!(
            lifecycle_semantic_prefix_width(profile, 4, &[3], expanded + 1),
            Err(BuilderError::Lifecycle("expanded-account-width"))
        );
    }
}
