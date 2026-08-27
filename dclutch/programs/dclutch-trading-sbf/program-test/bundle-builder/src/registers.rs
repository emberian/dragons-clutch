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
//! 7. execute the transition fold,
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
    v4::{ProgramV4 as EffectProgramV4, ResolvedWriteRangeV4, project_atomic_visiting},
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

fn digest32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Run the full register pipeline. See the module documentation for phases.
#[allow(clippy::too_many_lines)]
pub fn run_engine(input: &EngineInputV1<'_>) -> Result<EngineOutputV1, BuilderError> {
    let profile = input.profile;
    let tail_count = input.tail_count;
    let lifecycle_digest = digest32(input.lifecycle_bytes);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_digest,
        lifecycle_digest,
        input.lifecycle_bytes,
    )
    .map_err(|_| BuilderError::Projection("lifecycle-decode"))?;
    let profile_join = lifecycle
        .validate_account_profile_join(profile)
        .map_err(|_| BuilderError::Projection("profile-join"))?;
    let transition = TransitionProgramV3::decode(input.transition_bytes)
        .map_err(|_| BuilderError::Projection("transition-decode"))?;
    let effect = EffectProgramV4::decode(input.effect_bytes)
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
    let selected_config_is_variable = profile
        .rule(
            false,
            u16::try_from(1_usize).map_err(|_| BuilderError::Arithmetic)?,
        )
        .map_err(|_| BuilderError::Profile(line!()))?
        .prestate()
        == AccountPrestateV2::AdapterAuthenticatedVariableData;
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
            if coordinate == 4 || (coordinate == 1 && selected_config_is_variable) {
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
    let quotes = current_rent_quotes(lifecycle, input.rent)?;
    lifecycle
        .project_authenticated_current_rent_quotes_atomic(
            profile,
            Some(profile_join),
            tail_count,
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

    // Phase 7: the transition fold.
    scratch_scalars.copy_from_slice(&current_scalars);
    scratch_identities.copy_from_slice(&current_identities);
    next_scalars.copy_from_slice(&current_scalars);
    next_identities.copy_from_slice(&current_identities);
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
    .map_err(|_| BuilderError::Projection("transition"))?;
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
    )?;

    Ok(EngineOutputV1 {
        scalars: transition_scalars,
        identities: transition_identities,
        request_bank,
        lifecycle_states: preplan.states,
        invocations,
        complete: true,
    })
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
    let profile = input.profile;
    let effect = EffectProgramV4::decode(input.effect_bytes)
        .map_err(|_| BuilderError::Spans("effect-decode"))?;
    let span_count = profile.dynamic_fixed_span_count();
    if !profile.uses_dynamic_fixed_spans() || span_count == 0 {
        if span_count != 0 || effect.span_count() != 0 {
            return Err(BuilderError::Spans("undeclared-spans"));
        }
        return Ok(Vec::new());
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
    Ok(widths)
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

fn current_rent_quotes(
    lifecycle: StateLifecyclePolicyV5<'_>,
    rent: &Rent,
) -> Result<Vec<AuthenticatedRentQuoteV5>, BuilderError> {
    let mut quotes = Vec::with_capacity(usize::from(lifecycle.current_rent_quote_count()));
    let mut ordinal = 0_u16;
    while ordinal < lifecycle.current_rent_quote_count() {
        let declaration = lifecycle
            .current_rent_quote(ordinal)
            .map_err(|_| BuilderError::Projection("rent-quote-declaration"))?;
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
        if count == 0 {
            // A disabled route projects no invocation, but its request bytes
            // still occupy the bank and the Hot executor still derives its
            // caller-authority coordinate from them. Synthesize the shadow
            // geometry so the authority can be derived.
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
