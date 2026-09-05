//! Candidate execution under the sealed strategy: the interpreted transition,
//! the admitted and shadow accelerators, the effect projection and the runtime
//! transcript the accelerators must reproduce.

use super::*;

pub(super) struct AdmittedCandidateViewV3<'a, 'data, 'accounts, 'info> {
    pub(super) program_id: &'a Pubkey,
    pub(super) frame: &'a HotFrameV3<'accounts, 'info>,
    pub(super) hot_fixed_accounts: &'a [AccountInfo<'info>],
    pub(super) caller_authorities: &'a [AccountInfo<'info>],
    pub(super) output_page: Option<&'a AccountInfo<'info>>,
    pub(super) strategy_extras: &'a [AccountInfo<'info>],
    pub(super) runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    pub(super) input_scratch_pages: &'a [&'accounts AccountInfo<'info>],
    pub(super) observations: &'a [AccountObservationV1<'data>],
    pub(super) envelope: HotExecutionEnvelopeV3,
    pub(super) context: &'a TradingFamilyContextV1,
    pub(super) descriptor: &'a CapabilityProgramV4,
    pub(super) strategy: &'a AuthenticatedExecutionStrategyV2,
    pub(super) product_runtime_v3: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    pub(super) family_request: &'a [u8],
    pub(super) root_prestate: [u8; 32],
    pub(super) selected_program: ContentId,
    pub(super) selected_action: u32,
    pub(super) tail_count: u32,
    pub(super) scalars: &'a [u64],
    pub(super) identities: &'a [[u8; 32]],
    /// Exact representative coordinate of every logical runtime coordinate.
    pub(super) representatives: &'a [usize],
}

pub(super) struct CandidateExecutionV3 {
    pub(super) scalars: Vec<u64>,
    pub(super) identities: Vec<[u8; 32]>,
    pub(super) transcript_digest: [u8; 32],
}

/// Fold the interpreted transition without allocating a register bank.
///
/// The fold needs three pairs: the input it reads, a scratch pair, and the
/// output pair it returns. All three already exist and none of them had to be
/// allocated here.
///
/// - the input is the preplan's output, borrowed;
/// - the *output* is the request-projection pair moved in. It was dead the
///   moment the preplan copied it, it is exactly the right width, and the
///   candidate's registers outlive this call -- so the pair that leaves as
///   `CandidateExecutionV3` is the pair that arrived, not a fresh `to_vec` of
///   the input;
/// - the *scratch* is rented from the preplan arena, which is idle between the
///   two `prepare_lifecycle_v4` passes this call sits between.
///
/// Renting the arena's working pair is sound rather than merely convenient:
/// `prepare_lifecycle_v4` copies `output_scalars`/`output_identities` over all
/// four arena working banks immediately before every use, so nothing it does
/// can observe what this fold left in them. Previously this function rented one
/// pair for scratch and then allocated a whole second pair for the output --
/// which, on an allocator whose `dealloc` is a no-op, charged the heap a full
/// pair while the rented one died here unrecoverably.
/// The three register-bank pairs the fold runs on, named by their ROLE, which
/// is the only thing distinguishing them: all three are the same width and two
/// of them are borrowed from phases that are done with them.
///
/// Shaped like `ProjectionRegistersV2`, and for the same reason -- six
/// same-typed banks passed positionally is six chances to transpose scratch and
/// output, and the compiler catches none of them.
pub(super) struct TransitionRegistersV3<'a> {
    pub(super) input_scalars: &'a [u64],
    pub(super) input_identities: &'a [[u8; 32]],
    /// The preplan arena's working pair, idle between the preplan and the replan.
    pub(super) scratch_scalars: &'a mut [u64],
    pub(super) scratch_identities: &'a mut [[u8; 32]],
    /// The request-projection output pair, dead since the preplan copied it.
    /// Returned as the candidate's registers rather than cloned from the input.
    pub(super) output_scalars: Vec<u64>,
    pub(super) output_identities: Vec<[u8; 32]>,
}

#[inline(never)]
pub(super) fn execute_interpreted_transition_v3(
    transition: TransitionProgramV3<'_>,
    tail_count: u32,
    registers: TransitionRegistersV3<'_>,
) -> Result<CandidateExecutionV3, ProgramError> {
    let TransitionRegistersV3 {
        input_scalars,
        input_identities,
        scratch_scalars,
        scratch_identities,
        mut output_scalars,
        mut output_identities,
    } = registers;
    if output_scalars.len() != input_scalars.len()
        || output_identities.len() != input_identities.len()
        || scratch_scalars.len() != input_scalars.len()
        || scratch_identities.len() != input_identities.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    scratch_scalars.copy_from_slice(input_scalars);
    scratch_identities.copy_from_slice(input_identities);
    output_scalars.copy_from_slice(input_scalars);
    output_identities.copy_from_slice(input_identities);
    execute_fold_atomic(
        transition,
        tail_count,
        RegisterInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        RegisterOutput {
            scalars: scratch_scalars,
            identities: scratch_identities,
        },
        RegisterOutput {
            scalars: &mut output_scalars,
            identities: &mut output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(CandidateExecutionV3 {
        scalars: output_scalars,
        identities: output_identities,
        transcript_digest: [0_u8; 32],
    })
}

pub(super) struct ProjectedEffectsV3 {
    pub(super) lamports: Vec<u64>,
    pub(super) requests: Vec<u8>,
    /// One flag per representative coordinate the local effects mutate, or
    /// `None` when the Effect declares no child route and the answer has no
    /// consumer. Folded out of the projection's own walk.
    pub(super) participation: Option<Vec<CoordinateParticipationV3>>,
}

/// One exactly-sized projection bank, refused rather than aborted when the heap
/// cannot cover it.
///
/// `vec![v; n]` and `collect` allocate infallibly: on an exhausted heap they
/// abort the whole invocation (`memory allocation failed` ->
/// `ProgramFailedToComplete`), which is fail-closed at the transaction but is
/// not a protocol refusal and leaves a caller nothing to read. Every bank this
/// projection needs has its exact width before it is filled, so the same
/// allocation can be asked for fallibly and answered with the refusal the rest
/// of this boundary speaks.
/// The two numbers per coordinate the effect projection reads out of the
/// observation bank.
///
/// Taken as its own bank so the observation bank -- forty-eight bytes per
/// coordinate plus a sixteen-byte borrow guard, both in the scratch region --
/// can be released before the projection runs. It is `Vec` rather than a
/// scratch bank because the projection mutates it in place through
/// [`apply_lifecycle_candidates_v3`] and reads it for the whole walk.
pub(super) fn account_inputs_v3(
    observations: &[AccountObservationV1<'_>],
) -> Result<Vec<AccountInput>, ProgramError> {
    let mut account_inputs: Vec<AccountInput> = Vec::new();
    account_inputs
        .try_reserve_exact(observations.len())
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    account_inputs.extend(observations.iter().map(|observation| AccountInput {
        lamports: observation.lamports(),
        data_len: observation.data().len(),
    }));
    Ok(account_inputs)
}

/// Whether a runtime coordinate's BYTES are loader state rather than prestate.
///
/// The runtime transcript exists so the accelerator and Trading can prove they
/// evaluated the same accounts. For a state account that means its bytes: the
/// revision, the balances, the widths a candidate is computed from. For an
/// account the LOADER owns it means nothing of the kind. Its bytes are an ELF
/// and a 45-byte deployment header, no program can write them inside this
/// instruction, and both sides already commit its `key`, `owner`, `lamports`
/// and `executable` -- so a substituted deployment changes the transcript
/// through its key, and the pair is independently authenticated by
/// `ProgramDataMetadataV3View::parse` and the Registry activation join, neither
/// of which reads a byte past the header.
///
/// It is not free. MEASURED on real ELFs 2026-09-02, inside the accepted
/// campaign's equity Add: 58 runtime accounts carrying 9,510,282 transcript
/// bytes, of which 9,509,994 are the three loader pairs (Trading, Claims, Core)
/// -- each bound TWICE, once at its top-level frame coordinate and once inside
/// the Claims fixed span -- and the single widest is Trading's own
/// 2,315,397-byte programdata. `sol_sha256` is ~0.5 CU/byte, so that transcript
/// is ~4.75M CU against a 1,399,700 ceiling: the honest equity Add died at
/// 1,399,692 with ZERO CPIs, and so did the hostile, at the same unit, because
/// the cost was never about the hostility. Under this rule the same transcript
/// is 9,865 bytes and 14,756 CU. The LP Open, which binds one executable and no
/// programdata, digested 1,709 bytes for 3,197 CU and has always executed.
///
/// The rule is the one canonical scratch pages already get four lines below,
/// for the same reason: bytes committed elsewhere are not committed here.
pub(super) fn transcript_omits_loader_bytes_v3(owner: &Pubkey, executable: bool) -> bool {
    executable || owner == &solana_sdk_ids::bpf_loader_upgradeable::ID
}

/// The accelerator transcript digest over one observation bank.
///
/// Both accelerator dispositions committed to exactly this value and each had
/// its own copy of the walk; the shadow one now takes it before the bank is
/// released, so the two agree by construction rather than by inspection.
pub(super) fn runtime_transcript_digest_v3(
    observations: &[AccountObservationV1<'_>],
    runtime_accounts: &[&AccountInfo<'_>],
    canonical_scratch_pages: &[&AccountInfo<'_>],
) -> Result<ContentId, ProgramError> {
    // NO BANK OF OBSERVATIONS. `ShadowRuntimeObservationV3` owns its key and
    // owner, so collecting one per coordinate copies sixty-four bytes that are
    // already addressable in the account frame -- and the copy is charged for
    // the rest of the invocation, because the upward end never frees. Measured
    // 2026-09-02 on the Dealer post-trade partial equity Remove: seventy-four
    // coordinates, 7,104 bytes, on a route whose peak stood 136 bytes over its
    // 65,536-byte grant. The two remaining buffers are exactly the widths the
    // digest declares, allocated FALLIBLY: the convenience form's `vec!` aborts
    // the invocation on an exhausted heap, which is fail-closed at the
    // transaction and is not a refusal any caller can read.
    let empty: &[u8] = &[];
    let mut scratch = try_projection_bank_v3(
        &0_u8,
        runtime_observations_scratch_bytes_v3(observations.len()),
    )?;
    let mut slices = try_projection_bank_v3(
        &empty,
        runtime_observations_scratch_slices_v3(observations.len()),
    )?;
    borrowed_runtime_observations_digest_in_v3(
        observations
            .iter()
            .zip(runtime_accounts)
            .map(|(observation, account)| {
                let canonical_scratch = canonical_scratch_pages
                    .iter()
                    .any(|page| page.key == account.key);
                BorrowedRuntimeObservationV3 {
                    key: observation.key_bytes(),
                    owner: observation.owner_bytes(),
                    lamports: observation.lamports(),
                    data: if canonical_scratch
                        || transcript_omits_loader_bytes_v3(account.owner, account.executable)
                    {
                        &[]
                    } else {
                        observation.data()
                    },
                    signer: false,
                    writable: false,
                    executable: account.executable,
                }
            }),
        &mut scratch,
        &mut slices,
    )
    .map_err(|_| TradingSbfError::Content.into())
}

pub(super) fn try_projection_bank_v3<T: Clone>(value: &T, len: usize) -> Result<Vec<T>, ProgramError> {
    let mut bank = Vec::new();
    bank.try_reserve_exact(len)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    bank.resize(len, value.clone());
    Ok(bank)
}

/// Account candidates and both Effect scratch banks are phase-local. Only the
/// exact lamport projection and child-request bank survive into preflight/CPI.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn project_hot_effects_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    // Taken by value from [`account_inputs_v3`], which the caller runs while
    // the observation bank is still live. This function never sees that bank:
    // it is released before this runs.
    mut account_inputs: Vec<AccountInput>,
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
    // The bank `project_account_and_request_registers_v3` filled out of the
    // same rules, at the same widths, in the same instruction. The
    // `AccountProfileV2` and the span counts this phase used to take are gone
    // with the second walk that read them: the only thing it decoded them for
    // was the permission bank.
    effect_permissions: &[AccountPermission],
    aliases: &[usize],
    runtime_account_count: usize,
    request_bytes: usize,
) -> Result<ProjectedEffectsV3, ProgramError> {
    if account_inputs.len() != runtime_account_count {
        return Err(TradingSbfError::Content.into());
    }
    hot_heap_mark!("effects-account-inputs");
    apply_lifecycle_candidates_v3(lifecycle_plans, aliases, &mut account_inputs)?;
    apply_funding_candidates_v5(effect.funding(), scalars, aliases, &mut account_inputs)?;
    // Four of this projection's six banks are read only by the kernel walk
    // below and are dead the instant it returns, and the two that are not are
    // the ones this function returns. Splitting them by END rather than by
    // name is what makes the split reclaimable: the phase-local four come off
    // the scratch end and go back in one store when `phase` drops, whatever
    // the two survivors were allocated between.
    let phase = HeapScratchRegionV1::open()?;
    let mut permissions = ScratchVecV1::filled(
        &phase,
        &AccountPermission::read_only(),
        runtime_account_count,
    )?;
    hot_heap_mark!("effects-permissions");
    // The projection's bank, at this phase's width. A caller that handed over a
    // bank of the wrong width refuses here rather than projecting against a
    // permission set that is not this frame's.
    if effect_permissions.len() != permissions.len() {
        return Err(TradingSbfError::Content.into());
    }
    permissions.copy_from_slice(effect_permissions);
    require_common_projection_permissions_v3(&permissions)?;
    hot_cu_checkpoint!("p7e-permissions");
    let effect_account_count = effect
        .successor
        .account_count(tail_count, scalars)
        .map_err(|_| TradingSbfError::Transition)?;
    if effect_account_count > runtime_account_count
        || permissions
            .get(effect_account_count..)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .any(|permission| *permission != AccountPermission::read_only())
    {
        return Err(TradingSbfError::Content.into());
    }
    hot_heap_mark!("effects-count-checked");
    let mut scratch_lamports = ScratchVecV1::filled(&phase, &0_u64, effect_account_count)?;
    // One lamport output bank, not two. The projection's own output bank was a
    // separate `effect_account_count`-wide allocation whose entire contents were
    // then copied into the prefix of the wider bank this function returns -- so
    // on an allocator that never frees, the heap carried a whole second copy of
    // the projected balances for the rest of the instruction to serve one
    // `copy_from_slice`.
    //
    // The returned bank is built first and the projection writes straight into
    // its prefix. Its incoming contents are not load-bearing in either
    // direction: on success the kernel overwrites every entry of the output
    // bank from the alias-resolved scratch bank, and on refusal it leaves the
    // bank untouched and this function returns `Err`, so nothing downstream can
    // observe the seed. Seeding it from `account_inputs` before the projection
    // rather than after is the same value -- the projection takes `accounts` as
    // a shared slice and cannot alter it.
    let mut output_lamports: Vec<u64> = Vec::new();
    output_lamports
        .try_reserve_exact(account_inputs.len())
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    output_lamports.extend(account_inputs.iter().map(|account| account.lamports));
    hot_heap_mark!("effects-lamport-banks");
    // One bank, not two. The projection's second request bank was written once,
    // at the end, as a verbatim copy of the first; on an allocator that never
    // frees that copy cost the full declared request width for the whole
    // instruction. The single bank carries the same bytes into preflight/CPI.
    let mut requests = try_projection_bank_v3(&0_u8, request_bytes)?;
    hot_heap_mark!("effects-request-bank");
    // The kernel allocates nothing, so the runtime-write overlap refusal's
    // scratch is one of this function's banks. It is what lets that refusal
    // resolve each local-effect ordinal once instead of once per PAIR of them,
    // and it is twelve bytes per ordinal that RECORDS a range -- not per
    // ordinal. A Direct walk resolves 131 and records two of them.
    let mut write_ranges = ScratchVecV1::filled(
        &phase,
        &ResolvedWriteRangeV4::vacant(),
        effect
            .successor
            .data_write_operation_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?,
    )?;
    hot_heap_mark!("effects-write-ranges");
    // The local-effect discipline rides this projection's walk instead of
    // making its own. Both see every operation of the same Effect at the same
    // registers and in the same order; the second walk measured 139,214 CU on
    // the canonical Direct bundle and resolved nothing the first had not.
    let mut binding_count = 0_usize;
    for prepared in lifecycle_plans {
        binding_count = binding_count
            .checked_add(prepared.immutable_identity_bindings.len())
            .ok_or(TradingSbfError::Transition)?;
    }
    let mut written = ScratchVecV1::filled(&phase, &false, binding_count)?;
    let mut participation = if effect.route_count() == 0 {
        None
    } else {
        Some(try_projection_bank_v3(
            &CoordinateParticipationV3::default(),
            aliases.len(),
        )?)
    };
    hot_heap_mark!("effects-discipline-banks");
    hot_cu_checkpoint!("p7e-banks");
    // A refusal from the visitor is the visitor's own refusal, carried out
    // through the kernel's single error channel and re-raised here so its exact
    // code survives.
    let mut refused: Option<ProgramError> = None;
    let outcome = project_effects_v4_atomic_visiting(
        effect.successor,
        tail_count,
        ProjectionV3 {
            scalars,
            identities,
            aliases: aliases
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            accounts: account_inputs
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            permissions: permissions
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: output_lamports
                .get_mut(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            requests: &mut requests,
        },
        &mut write_ranges,
        &mut |resolved| match require_no_funding_local_mutation_v5(effect.funding(), resolved)
            .and_then(|()| {
                inspect_local_effect_discipline_v5(
                    lifecycle_plans,
                    resolved,
                    aliases,
                    &mut written,
                    participation.as_deref_mut(),
                )
            }) {
            Ok(()) => Ok(()),
            Err(error) => {
                refused = Some(error);
                Err(EffectKernelErrorV4::BaseProgram)
            }
        },
    );
    if let Some(error) = refused {
        return Err(error);
    }
    outcome.map_err(|_| TradingSbfError::Transition)?;
    require_lifecycle_binding_coverage_v4(lifecycle_plans, &written)?;
    hot_heap_mark!("effects-projected");
    Ok(ProjectedEffectsV3 {
        lamports: output_lamports,
        requests,
        participation,
    })
}

fn require_no_funding_local_mutation_v5(
    funding: Option<EffectProgramV5<'_>>,
    resolved: ResolvedEffectV3,
) -> Result<(), ProgramError> {
    let Some(funding) = funding else {
        return Ok(());
    };
    match resolved {
        ResolvedEffectV3::TransferLamports {
            source,
            destination,
            ..
        } if funding_owns_coordinate_v5(funding, source)
            || funding_owns_coordinate_v5(funding, destination) =>
        {
            Err(TradingSbfError::Transition.into())
        }
        ResolvedEffectV3::WriteScalar { account, .. }
        | ResolvedEffectV3::WriteIdentity { account, .. }
        | ResolvedEffectV3::WriteU8 { account, .. }
        | ResolvedEffectV3::WriteU16 { account, .. }
        | ResolvedEffectV3::WriteU32 { account, .. }
            if funding_owns_coordinate_v5(funding, account)
                && !funding_allows_created_state_write_v5(funding, account) =>
        {
            Err(TradingSbfError::Transition.into())
        }
        _ => Ok(()),
    }
}

fn funding_allows_created_state_write_v5(funding: EffectProgramV5<'_>, coordinate: usize) -> bool {
    let mut index = 0_u16;
    while index < funding.funding_action_count() {
        let Ok(action) = funding.funding_action(index) else {
            return false;
        };
        if usize::from(action.state()) == coordinate {
            return action.operation() == FundingOperationV5::Create;
        }
        index = match index.checked_add(1) {
            Some(index) => index,
            None => return false,
        };
    }
    false
}

/// The disposition-dependent span between the strategy extras and the runtime.
pub(super) struct StrategyFrameSpanV3<'a, 'info> {
    pub(super) shadow_caller_authority: Option<&'a AccountInfo<'info>>,
    pub(super) admitted_caller_authorities: Option<&'a [AccountInfo<'info>]>,
    pub(super) admitted_output_page: Option<&'a AccountInfo<'info>>,
    pub(super) runtime_start: usize,
}

/// Carve the caller-authority span, and the output page when there is one.
///
/// The admitted arm refuses `AdmittedFrame` rather than `Content`. Every
/// coordinate it computes is an admitted-frame coordinate, `Content` has 2,126
/// sites in this program, and a route that dies here with `Content` is a route
/// whose reader has to bisect. The Interpreted and Shadow arms keep the code
/// they always raised.
///
/// `#[inline(never)]`, and that is a MEASURED constraint rather than a
/// preference. Inlined, this arithmetic is 64 bytes over
/// `execute_authenticated_hot_v3`'s 4,096-byte frame -- `cargo build-sbf`
/// reported "Estimated function frame size: 4160 bytes" on the commit that
/// added the page to the carving, and a frame diagnostic is a wall this tree
/// keeps at zero. Its own frame is where these locals belong anyway: nothing
/// below the return needs them.
#[inline(never)]
pub(super) fn carve_strategy_frame_span_v3<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    strategy: &AuthenticatedExecutionStrategyV2,
    strategy_extras_start: usize,
    strategy_extras_end: usize,
    provisional_scalar_count: usize,
    provisional_identity_count: usize,
) -> Result<StrategyFrameSpanV3<'a, 'info>, ProgramError> {
    let displacement = strategy_extras_start
        .checked_sub(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3)
        .ok_or(TradingSbfError::Content)?;
    match strategy.strategy().disposition() {
        StrategyDispositionV2::Interpreted => Ok(StrategyFrameSpanV3 {
            shadow_caller_authority: None,
            admitted_caller_authorities: None,
            admitted_output_page: None,
            runtime_start: strategy_extras_end,
        }),
        StrategyDispositionV2::ShadowAot => {
            let expected = HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3
                .checked_add(displacement)
                .ok_or(TradingSbfError::Content)?;
            if strategy_extras_end != expected {
                return Err(TradingSbfError::Content.into());
            }
            Ok(StrategyFrameSpanV3 {
                shadow_caller_authority: Some(
                    accounts
                        .get(strategy_extras_end)
                        .ok_or(TradingSbfError::Content)?,
                ),
                admitted_caller_authorities: None,
                admitted_output_page: None,
                runtime_start: strategy_extras_end
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?,
            })
        }
        StrategyDispositionV2::AdmittedAot => {
            let admitted_start = HOT_ADMITTED_CALLER_AUTHORITIES_START_V3
                .checked_add(displacement)
                .ok_or(TradingSbfError::AdmittedFrame)?;
            if strategy_extras_end != admitted_start {
                return Err(TradingSbfError::AdmittedFrame.into());
            }
            let profile = strategy
                .strategy()
                .transport_profile()
                .map_err(|_| TradingSbfError::AdmittedFrame)?;
            let callers_end = admitted_start
                .checked_add(admitted_caller_authority_count_v3(
                    profile,
                    u32::try_from(provisional_scalar_count)
                        .map_err(|_| TradingSbfError::AdmittedFrame)?,
                    u32::try_from(provisional_identity_count)
                        .map_err(|_| TradingSbfError::AdmittedFrame)?,
                )?)
                .ok_or(TradingSbfError::AdmittedFrame)?;
            // The page is carved in the same relative place the CPI frame puts
            // it: after the caller-authority span, before the runtime slice.
            // Under the chunked profile that span is zero accounts wide and
            // this line is the identity.
            let runtime_start = callers_end
                .checked_add(hot_admitted_output_page_accounts_v3(profile))
                .ok_or(TradingSbfError::AdmittedFrame)?;
            Ok(StrategyFrameSpanV3 {
                shadow_caller_authority: None,
                admitted_caller_authorities: Some(
                    accounts
                        .get(admitted_start..callers_end)
                        .ok_or(TradingSbfError::AdmittedFrame)?,
                ),
                admitted_output_page: match profile {
                    AcceleratorTransportProfileV2::OutputPageV3 => Some(
                        accounts
                            .get(callers_end)
                            .ok_or(TradingSbfError::AdmittedFrame)?,
                    ),
                    AcceleratorTransportProfileV2::ChunkedBankV2
                    | AcceleratorTransportProfileV2::ShadowTranscriptV3 => None,
                },
                runtime_start,
            })
        }
    }
}

#[inline(never)]
/// Refuse a bank whose width is not the width the account frame was carved for.
///
/// WIRED, AND THE 64 BYTES WERE NEVER ELSEWHERE IN THE FUNCTION. Every shape
/// that CARRIED the frame's pair to the comparison cost the same 64-byte step
/// and the same three `overwrites values in the frame` diagnostics -- as two
/// `usize` on the by-value `AdmittedCandidateViewV3`, as two scalar arguments,
/// as a tuple-pair call from the caller, as a call from an `#[inline(never)]`
/// helper, and (measured 2026-09-02) as a reuse of the carving's own
/// `provisional_scalar_count`/`provisional_identity_count` at a join placed two
/// hundred lines earlier. The cost was never the call. It was the two spill
/// slots the pair needs to survive the projection, the preplan and the fold,
/// and no rearrangement of the call can avoid paying for a value that has to
/// live across them.
///
/// The join is guarded on `admitted_caller_authorities.is_some()`, because a
/// frame that carved no caller authorities carved nothing for a bank to match
/// and an Interpreted or Shadow route must not acquire a conjunct that the
/// admitted one needs.
///
/// The admitted path derives two things from the register-bank geometry and
/// derives them in different places from different sources. The FRAME carving
/// (`hot_v3::execute_authenticated_hot_v3`) asks
/// `admitted_caller_authority_count_v3` for a caller-authority count using the
/// EFFECT's declared widths, because it runs before any bank exists and has to
/// know where the runtime slice starts. The TRANSPORT
/// (`admitted_composition_v3::execute_admitted_aot_v3`) asks
/// `classify_bank_transport_v2` for a chunk count using the bank it is handed.
///
/// One caller-authority account per chunk is the whole contract between them,
/// and if the two counts ever disagree the accelerator is invoked with a
/// caller-authority span carved for a different number of pages than the
/// transport writes. Nothing said so. Stated here, and separated from the walk
/// so both disagreements can be driven directly.
pub(super) fn require_admitted_bank_matches_frame_v3(
    bank: (usize, usize),
    frame: (usize, usize),
) -> Result<(), ProgramError> {
    if bank == frame {
        Ok(())
    } else {
        Err(TradingSbfError::AdmittedTransport.into())
    }
}

/// Execute the admitted accelerator as the sole candidate authority.
///
/// EVERY REFUSAL RAISED HERE NAMES ITS CAUSE. This function used to publish
/// `Content` from twenty-five sites, and `Content` has 2,126 sites in this
/// program: an honest equity Add and a hostile strategy substitution refused
/// with the same code, which is what makes a hostile assertion a universal
/// donor (ledger `M-38`) and an honest wall unlocalizable.
///
/// The three codes it needed already existed and it used none of them.
/// `AdmittedFrame` 0x4017, `AdmittedTransport` 0x4018 and `AdmittedContext`
/// 0x4019 were split out of `Content` for exactly this boundary, and their doc
/// comments already describe these lines -- "an identity that is not a valid
/// `ContentId`, or a strategy that names no certificate, admission, or artifact
/// release" is the invocation context built below, verbatim. So this is not a
/// new vocabulary, it is the existing one reaching the sites it was created
/// for: the accelerator deployment pair and the eight strategy-owned evidence
/// coordinates are `AdmittedFrame`, the context and its identities are
/// `AdmittedContext`, and the bank-width binding is `AdmittedTransport`.
pub(super) fn execute_admitted_candidate_v3(
    view: AdmittedCandidateViewV3<'_, '_, '_, '_>,
) -> Result<CandidateExecutionV3, ProgramError> {
    let accelerator_program = view
        .strategy_extras
        .get(6)
        .ok_or(TradingSbfError::AdmittedFrame)?;
    let accelerator_programdata = view
        .strategy_extras
        .get(7)
        .ok_or(TradingSbfError::AdmittedFrame)?;
    let family_request_digest = family_request_digest_v3(view.family_request)
        .map_err(|_| TradingSbfError::AdmittedContext)?;
    let runtime_observations_digest = runtime_transcript_digest_v3(
        view.observations,
        view.runtime_accounts,
        view.input_scratch_pages,
    )?;
    // The candidate phase is the widest unlit stretch on an admitted route: on
    // 2026-09-02 a hostile equity Add entered it with 715,210 units, made ZERO
    // CPIs, and died at the budget -- so every unit went somewhere between the
    // `request-lifecycle-preplan` checkpoint and the accelerator's first
    // invoke, with nothing to say which consumer. This splits that stretch in
    // two: everything up to here is the runtime transcript over every account,
    // and everything after is context assembly, frame validation, bank encode
    // and chunk classification.
    hot_cu_checkpoint!("candidate-transcript");
    let product_runtime = view.product_runtime_v3.runtime;
    let admitted_context = AdmittedInvocationContextV3 {
        release_set: ContentId::new(view.envelope.release_set())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        market: ContentId::new(view.envelope.market())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        root: ContentId::new(view.frame.root.key.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        registry_program: ContentId::new(view.frame.registry.key.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        trading_program: ContentId::new(view.program_id.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        accelerator_program: ContentId::new(accelerator_program.key.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        capability_program: view.selected_program,
        account_profile: view.descriptor.account_profile().program(),
        request_profile: view.descriptor.request_profile().program(),
        transition: view.strategy.strategy().transition_program(),
        effect: view.descriptor.effect().program(),
        lifecycle: view.descriptor.derivation_policy(),
        strategy: view.strategy.strategy_program_id(),
        certificate: view
            .strategy
            .certificate_program_id()
            .ok_or(TradingSbfError::AdmittedContext)?,
        admission: view
            .strategy
            .admission_program_id()
            .ok_or(TradingSbfError::AdmittedContext)?,
        artifact_release: view
            .strategy
            .artifact_release_id()
            .ok_or(TradingSbfError::AdmittedContext)?,
        config: view.context.selection().config(),
        product: ContentId::new(product_runtime.product_record.content_digest.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        portfolio: ContentId::new(product_runtime.portfolio_record.content_digest.to_bytes())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        linked_basis: ContentId::new(
            view.product_runtime_v3
                .linked_basis_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::AdmittedContext)?,
        family_request_digest,
        runtime_observations_digest,
        root_prestate_digest: ContentId::new(view.root_prestate)
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        selected_action: view.selected_action,
        tail_count: view.tail_count,
        account_count: u32::try_from(view.runtime_accounts.len())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        scalar_count: u32::try_from(view.scalars.len())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
        identity_count: u32::try_from(view.identities.len())
            .map_err(|_| TradingSbfError::AdmittedContext)?,
    };
    // THE PRELUDE'S OUTPUTS TRAVEL IN THE REQUEST. Everything this program has
    // just derived -- the complete invocation-context preimage and the
    // representative coordinates -- is a value the callee's
    // own prelude would otherwise re-derive from twelve accounts, and the
    // caller-authority PDA that authorizes the CPI proves the composer is this
    // Trading program executing this signed family request. So the request is a
    // channel that costs nothing to widen, and widening it is a MOVE rather
    // than a deletion: the callee still
    // re-derives every field it holds an independent source for and refuses on
    // the first disagreement. See `authenticate_accelerator_invocation_v4`.
    let mut witness = vec![0_u8; admitted_prelude_witness_bytes_v1(view.representatives.len())];
    AdmittedPreludeWitnessV1::encode_into(
        admitted_context,
        // The eight Product graph record bumps THIS program's prelude already
        // derived, relayed so the callee's independent walk over the same four
        // records reproduces each address instead of searching for it.
        view.product_runtime_v3.record_bumps.0,
        view.representatives,
        &mut witness,
    )
    .map_err(|_| TradingSbfError::AdmittedContext)?;
    hot_cu_checkpoint!("cx-witness-encoded");
    let execution = execute_admitted_aot_v3(
        view.program_id,
        AdmittedCpiFrameV3 {
            caller_authorities: view.caller_authorities,
            output_page: view.output_page,
            hot_fixed_accounts: view.hot_fixed_accounts,
            activation: view.frame.activation_cache,
            registry: view.frame.registry,
            rent: view.frame.rent,
            instructions: view.frame.instructions,
            trading_program: view.frame.trading_program,
            trading_programdata: view.frame.trading_programdata,
            capability_raw: view.frame.descriptor_raw,
            capability_staging: view.frame.descriptor_staging,
            strategy_raw: view.frame.strategy_raw,
            strategy_staging: view.frame.strategy_staging,
            certificate_raw: view
                .strategy_extras
                .first()
                .ok_or(TradingSbfError::AdmittedFrame)?,
            certificate_staging: view
                .strategy_extras
                .get(1)
                .ok_or(TradingSbfError::AdmittedFrame)?,
            admission_raw: view
                .strategy_extras
                .get(2)
                .ok_or(TradingSbfError::AdmittedFrame)?,
            admission_staging: view
                .strategy_extras
                .get(3)
                .ok_or(TradingSbfError::AdmittedFrame)?,
            artifact_raw: view
                .strategy_extras
                .get(4)
                .ok_or(TradingSbfError::AdmittedFrame)?,
            artifact_staging: view
                .strategy_extras
                .get(5)
                .ok_or(TradingSbfError::AdmittedFrame)?,
            accelerator_program,
            accelerator_programdata,
        },
        view.runtime_accounts,
        view.input_scratch_pages,
        &admitted_context,
        *view.strategy,
        view.scalars,
        view.identities,
        &witness,
    )?;
    Ok(CandidateExecutionV3 {
        scalars: execution.scalars,
        identities: execution.identities,
        transcript_digest: execution.transcript_digest,
    })
}

pub(super) struct ShadowCandidateViewV3<'a, 'accounts, 'info> {
    pub(super) program_id: &'a Pubkey,
    pub(super) frame: &'a HotFrameV3<'accounts, 'info>,
    pub(super) caller_authority: &'a AccountInfo<'info>,
    pub(super) strategy_extras: &'a [AccountInfo<'info>],
    pub(super) runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    /// The transcript digest, not the bank it is taken over. The observation
    /// bank lives in the scratch region and is released before this runs; the
    /// digest is the only thing this candidate ever read out of it, and
    /// [`runtime_transcript_digest_v3`] takes it while the bank is still live.
    pub(super) runtime_observations_digest: ContentId,
    pub(super) envelope: HotExecutionEnvelopeV3,
    pub(super) descriptor: &'a CapabilityProgramV4,
    pub(super) strategy: &'a AuthenticatedExecutionStrategyV2,
    pub(super) family_request: &'a [u8],
    pub(super) root_prestate: [u8; 32],
    pub(super) selected_program: ContentId,
    pub(super) selected_action: u32,
    pub(super) effect: SelectedEffectProgramV4<'a>,
    pub(super) tail_count: u32,
    pub(super) scalars: &'a [u64],
    pub(super) identities: &'a [[u8; 32]],
    pub(super) output_lamports: &'a [u64],
    pub(super) request_bank: &'a [u8],
}

#[inline(never)]
pub(super) fn execute_shadow_candidate_v3(
    view: ShadowCandidateViewV3<'_, '_, '_>,
) -> Result<[u8; 32], ProgramError> {
    let accelerator_program = view
        .strategy_extras
        .get(4)
        .ok_or(TradingSbfError::Content)?;
    let accelerator_programdata = view
        .strategy_extras
        .get(5)
        .ok_or(TradingSbfError::Content)?;
    let family_digest =
        family_request_digest_v3(view.family_request).map_err(|_| TradingSbfError::Content)?;
    let runtime_digest = view.runtime_observations_digest;
    let candidate_digest = candidate_digest_v3(view.tail_count, view.scalars, view.identities)
        .map_err(|_| TradingSbfError::Content)?;
    let routes = shadow_routes_v3(view.effect, view.tail_count, view.scalars, view.identities)?;
    let effect_digest = effect_digest_v3(ShadowEffectProjectionV3 {
        tail_count: view.tail_count,
        output_lamports: view.output_lamports,
        request_bank: view.request_bank,
        routes: &routes,
    })
    .map_err(|_| TradingSbfError::Content)?;
    let release_set =
        ContentId::new(view.envelope.release_set()).map_err(|_| TradingSbfError::Content)?;
    let market = ContentId::new(view.envelope.market()).map_err(|_| TradingSbfError::Content)?;
    let root =
        ContentId::new(view.frame.root.key.to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let root_prestate_digest =
        ContentId::new(view.root_prestate).map_err(|_| TradingSbfError::Content)?;
    let invocation_context = invocation_context_digest_v3(ShadowInvocationContextV3 {
        release_set,
        market,
        root,
        capability_program: view.selected_program,
        selected_action: view.selected_action,
        family_request_digest: family_digest,
        root_prestate_digest,
    })
    .map_err(|_| TradingSbfError::Content)?;
    execute_shadow_aot_v3(
        view.program_id,
        ShadowCpiFrameV3 {
            caller_authority: view.caller_authority,
            activation: view.frame.activation_cache,
            registry: view.frame.registry,
            trading_program: view.frame.trading_program,
            trading_programdata: view.frame.trading_programdata,
            accelerator_program,
            accelerator_programdata,
        },
        view.runtime_accounts,
        ShadowRequestV3 {
            release_set,
            market,
            root,
            registry_program: ContentId::new(view.frame.registry.key.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            trading_program: ContentId::new(view.program_id.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            accelerator_program: ContentId::new(accelerator_program.key.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            artifacts: ShadowArtifactTupleV3 {
                capability_program: view.selected_program,
                account_profile: view.descriptor.account_profile().program(),
                request_profile: view.descriptor.request_profile().program(),
                transition: view.strategy.strategy().transition_program(),
                effect: view.descriptor.effect().program(),
                strategy: view.strategy.strategy_program_id(),
                certificate: view
                    .strategy
                    .certificate_program_id()
                    .ok_or(TradingSbfError::Content)?,
            },
            invocation_context,
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: runtime_digest,
                family_request: family_digest,
                interpreted_candidate: candidate_digest,
                interpreted_effect: effect_digest,
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: view.tail_count,
                account_count: u32::try_from(view.runtime_accounts.len())
                    .map_err(|_| TradingSbfError::Content)?,
                scalar_count: u32::try_from(view.scalars.len())
                    .map_err(|_| TradingSbfError::Content)?,
                identity_count: u32::try_from(view.identities.len())
                    .map_err(|_| TradingSbfError::Content)?,
            },
            family_request: view.family_request,
        },
    )
}

/// The four authenticated record identities the common Hot frame substitutes
/// for a physical address when it observes a logical coordinate.
///
/// Borrowed, not copied, into each observation: a fixed topology aliases many
/// logical coordinates onto few physical accounts, and the SBF bump allocator
/// never frees, so a 90-entry bank pays for every by-value identity twice.
pub(super) struct LogicalProjectionKeysV3 {
    pub(super) selected_config: [u8; 32],
    pub(super) product_root: [u8; 32],
    pub(super) portfolio: [u8; 32],
    pub(super) linked_basis: [u8; 32],
}

pub(super) fn logical_projection_key_v3<'a>(
    coordinate: usize,
    physical_key: &'a Pubkey,
    projected: &'a LogicalProjectionKeysV3,
) -> &'a [u8; 32] {
    match coordinate {
        1 => &projected.selected_config,
        2 => &projected.product_root,
        3 => &projected.portfolio,
        4 => &projected.linked_basis,
        _ => physical_key.as_array(),
    }
}

/// The projected request registers, together with the two register-bank pairs
/// the rotation left holding stale values.
///
/// The SBF bump allocator's `dealloc` is a no-op, so a bank that goes out of
/// scope is still charged against total-ever-allocated for the rest of the
/// execution. Dropping the two spare pairs here and allocating two more in the
/// preplan arena therefore costs the heap two whole pairs to obtain buffers
/// that already exist and are already dead. They are handed back instead of
/// dropped, and the phases downstream rent them rather than allocate.
/// Every bank here is in the Hot execution's scratch region, and every one of
/// them is dead by the replan: the preplan copies the output pair into its own
/// working banks and the arena rents the two spares. Nothing that survives the
/// replan is in this struct -- the transition writes its outputs into a bank
/// the caller allocates at the upward end for exactly that reason.
pub(super) struct ProjectedRequestRegistersV3<'region> {
    pub(super) scalars: ScratchVecV1<'region, u64>,
    pub(super) identities: ScratchVecV1<'region, [u8; 32]>,
    pub(super) spare_scalars: [ScratchVecV1<'region, u64>; 2],
    pub(super) spare_identities: [ScratchVecV1<'region, [u8; 32]>; 2],
}

/// Keep the transient Account/Request projection banks in one noinline phase.
/// Only the final candidate registers cross the boundary; scratch banks never
/// remain live across child CPI or commit-last execution.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn project_account_and_request_registers_v3<'region, 'artifact, 'accounts, 'info>(
    region: &'region HeapScratchRegionV1,
    current_instruction: u16,
    native_message_offset_bias: u16,
    instruction_data: &'artifact [u8],
    frame: HotFrameV3<'accounts, 'info>,
    account_profile: AccountProfileV2<'artifact>,
    request_profile: RequestProfileKindV3<'artifact>,
    lifecycle: StateLifecyclePolicyV5<'artifact>,
    profile_join: ValidatedProfileJoinV3<'artifact>,
    action: u32,
    current_rent_quotes: &[AuthenticatedRentQuoteV5],
    span_counts: &[u32],
    tail_count: u32,
    observations: &[AccountObservationV1<'_>],
    // The effect permission bank the account walk fills as it decodes the rules
    // it already needs. `p7e-permissions` used to decode all of them again for
    // this one byte per coordinate, 14,800 CU on the partial equity Remove.
    effect_permissions: &mut [AccountPermission],
    family_request: &'artifact [u8],
    request_digest: [u8; 32],
    trusted_environment: TrustedEnvironmentObservationV3,
    authenticated_product_tail_count: u32,
    scalar_count: usize,
    identity_count: usize,
) -> Result<ProjectedRequestRegistersV3<'region>, ProgramError> {
    let mut input_scalars = ScratchVecV1::filled(region, &0_u64, scalar_count)?;
    let mut input_identities = ScratchVecV1::filled(region, &[0_u8; 32], identity_count)?;
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(TradingSbfError::Content)? = request_digest;
    seed_trusted_environment_v3(
        trusted_environment,
        &mut input_scalars,
        &mut input_identities,
    )?;
    if account_profile.uses_dynamic_fixed_spans() {
        if span_counts.len() != usize::from(account_profile.dynamic_fixed_span_count()) {
            return Err(TradingSbfError::Content.into());
        }
        let mut index = 0_u16;
        while index < account_profile.dynamic_fixed_span_count() {
            let span = account_profile
                .dynamic_fixed_span(index)
                .map_err(|_| TradingSbfError::Content)?;
            *input_scalars
                .get_mut(usize::from(span.count_scalar()))
                .ok_or(TradingSbfError::Content)? = u64::from(
                *span_counts
                    .get(usize::from(index))
                    .ok_or(TradingSbfError::Content)?,
            );
            index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    } else if !span_counts.is_empty() {
        return Err(TradingSbfError::Content.into());
    }
    // Five chained projections share three scalar banks and three identity
    // banks, rotated by `swap`, instead of cloning a fresh pair per step. The
    // SBF allocator never frees, so a clone per step charged seven live pairs
    // of total-ever-allocated for a chain that is never more than three deep.
    let mut current_scalars = input_scalars;
    let mut current_identities = input_identities;
    let mut scratch_scalars = ScratchVecV1::filled(region, &0_u64, scalar_count)?;
    let mut scratch_identities = ScratchVecV1::filled(region, &[0_u8; 32], identity_count)?;
    let mut next_scalars = ScratchVecV1::filled(region, &0_u64, scalar_count)?;
    let mut next_identities = ScratchVecV1::filled(region, &[0_u8; 32], identity_count)?;
    hot_heap_mark!("projection-three-pairs");
    // The widest unlit stretch of the ladder ran from `p5-geometry-rent` to
    // `p5r-account-projection`: 145,229 CU on 2026-09-02, the largest span in
    // the route that is not a child CPI, and it covered TWO unrelated things --
    // the six register banks above being allocated and zero-filled, and the
    // account projection walk below. A span with two subjects cannot answer for
    // either, so this splits it.
    hot_cu_checkpoint!("p5r-projection-banks");

    let account_registers = ProjectionRegistersV2 {
        input_scalars: &current_scalars,
        input_identities: &current_identities,
        scratch_scalars: &mut scratch_scalars,
        scratch_identities: &mut scratch_identities,
        output_scalars: &mut next_scalars,
        output_identities: &mut next_identities,
    };
    if account_profile.uses_dynamic_fixed_spans() {
        project_dynamic_fixed_spans_atomic(
            account_profile,
            tail_count,
            span_counts,
            observations,
            account_registers,
            Some(effect_permissions),
        )
    } else {
        project_accounts_atomic(
            account_profile,
            tail_count,
            observations,
            account_registers,
            Some(effect_permissions),
        )
    }
    .map_err(|error| {
        hot_cu_reason!("account-projection", error);
        hot_cu_data_length_disagreement!(account_profile, tail_count, &observations);
        TradingSbfError::Content
    })?;
    hot_cu_checkpoint!("p5r-account-projection");
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);
    require_projected_tail_count_agreement_v3(
        account_profile,
        authenticated_product_tail_count,
        &current_scalars,
    )?;
    require_trusted_environment_v3(trusted_environment, &current_scalars, &current_identities)?;

    lifecycle
        .project_authenticated_current_rent_quotes_atomic(
            account_profile,
            Some(profile_join),
            tail_count,
            action,
            &current_scalars,
            current_rent_quotes,
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch_scalars,
                output_scalars: &mut next_scalars,
            },
        )
        .map_err(|_| TradingSbfError::Content)?;
    hot_cu_checkpoint!("p5r-rent-quote-projection");
    core::mem::swap(&mut current_scalars, &mut next_scalars);

    if let RequestProfileKindV3::Signed(profile) = request_profile {
        next_identities.copy_from_slice(&current_identities);
        seed_native_signatures_at_authenticated_instruction(
            current_instruction,
            instruction_data,
            native_message_offset_bias,
            frame.instructions,
            profile,
            tail_count,
            NativeSignatureRegistersV1 {
                input_identities: &current_identities,
                scratch_identities: &mut scratch_identities,
                output_identities: &mut next_identities,
            },
        )?;
        core::mem::swap(&mut current_identities, &mut next_identities);
    }
    hot_cu_checkpoint!("p5r-native-signatures");

    request_profile.project_atomic(
        tail_count,
        family_request,
        ProjectionRegistersV1 {
            input_scalars: &current_scalars,
            input_identities: &current_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut next_scalars,
            output_identities: &mut next_identities,
        },
    )?;
    hot_cu_checkpoint!("p5r-request-projection");
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);
    if account_profile.uses_dynamic_fixed_spans() {
        let mut revalidated = ScratchVecV1::filled(region, &0_u32, span_counts.len())?;
        account_profile
            .dynamic_span_widths_from_scalars(&current_scalars, &mut revalidated)
            .map_err(|_| TradingSbfError::Content)?;
        if revalidated.as_slice() != span_counts {
            return Err(TradingSbfError::Content.into());
        }
    }
    require_trusted_environment_v3(trusted_environment, &current_scalars, &current_identities)?;
    lifecycle
        .validate_projected_current_rent_quotes(
            account_profile,
            Some(profile_join),
            tail_count,
            action,
            &current_scalars,
            current_rent_quotes,
        )
        .map_err(|_| TradingSbfError::Content)?;
    Ok(ProjectedRequestRegistersV3 {
        scalars: current_scalars,
        identities: current_identities,
        spare_scalars: [scratch_scalars, next_scalars],
        spare_identities: [scratch_identities, next_identities],
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TrustedEnvironmentObservationV3 {
    pub(super) current_slot: Option<(usize, u64)>,
    pub(super) current_executing_program: Option<(usize, [u8; 32])>,
    pub(super) system_program: Option<(usize, [u8; 32])>,
}

/// A `ShadowAot` strategy may not be paired with a slot-declaring AccountProfile.
///
/// # What this forbids, and what it does NOT claim
///
/// It forbids a pairing NOTHING HAS EVER EXECUTED. Series is the only family on
/// this disposition and declares `TrustedEnvironmentV2::None`, so the
/// combination has no fixture, no campaign and no on-chain instruction
/// anywhere in this tree -- and until `3a8ac205d` it was also unsound:
/// `shadow_composition_v3`'s caller-authority seed was `hash(ShadowRequestV3)`,
/// and that request carries `candidate_digest_v3` over the post-transition
/// register bank, which is where a `CurrentSlot` profile's `Clock::get().slot`
/// lands. The address moved every slot and a signed account list cannot name
/// it.
///
/// **That seed is fixed, so this is not the thing standing between the pairing
/// and the wall**, and the doc says so rather than implying otherwise. Both
/// accelerator dispositions and the Shadow callback authenticator in
/// `dclutch-shadow-accelerator-auth-v4` now derive from
/// `family_request_digest_v3`, which is a function of the signed instruction
/// alone. What this refusal buys is that the FIRST family to want the pairing
/// arrives here, by name, instead of arriving at whatever the untested
/// remainder of the Shadow route does with a bank that moves -- the runtime
/// account projection above all, whose slot-independence is proved for the
/// admitted route by `one_signed_account_list_opens_the_same_batch_at_two_execution_slots`
/// and is proved for this one by nothing.
///
/// Lifting it is therefore a measurement, not an argument: give the Shadow
/// route that two-slot proof and delete this.
///
/// It is a SELECTION conjunct and sits here rather than beside the disposition
/// gate in `authenticate_strategy_from_sealed_boxed_v3` for one reason: the
/// AccountProfile is not decoded there. This is the first point at which the
/// two authenticated artifacts are both in hand, and it is still ahead of the
/// register banks, the effect projection and every CPI.
pub(super) fn require_shadow_declares_no_trusted_slot_v1(
    disposition: StrategyDispositionV2,
    declared: TrustedEnvironmentV2,
) -> Result<(), ProgramError> {
    match (disposition, declared) {
        (StrategyDispositionV2::ShadowAot, TrustedEnvironmentV2::CurrentSlot { .. }) => {
            Err(TradingSbfError::ShadowTrustedEnvironment.into())
        }
        _ => Ok(()),
    }
}

pub(super) fn observe_trusted_environment_v3(
    profile: AccountProfileV2<'_>,
    program_id: &Pubkey,
) -> Result<TrustedEnvironmentObservationV3, ProgramError> {
    let current_slot = match profile.trusted_environment() {
        TrustedEnvironmentV2::None => None,
        TrustedEnvironmentV2::CurrentSlot { destination } => {
            let current_slot = Clock::get().map_err(|_| TradingSbfError::Content)?.slot;
            Some((usize::from(destination), current_slot))
        }
    };
    Ok(TrustedEnvironmentObservationV3 {
        current_slot,
        current_executing_program: profile
            .trusted_current_executing_program_identity()
            .map(|destination| (usize::from(destination), program_id.to_bytes())),
        system_program: profile
            .trusted_system_program_identity()
            .map(|destination| (usize::from(destination), system_program::ID.to_bytes())),
    })
}

pub(super) fn seed_trusted_environment_v3(
    observation: TrustedEnvironmentObservationV3,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), ProgramError> {
    if let Some((destination, current_slot)) = observation.current_slot {
        *scalars
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = current_slot;
    }
    if let Some((destination, current_program)) = observation.current_executing_program {
        *identities
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = current_program;
    }
    if let Some((destination, system_program)) = observation.system_program {
        *identities
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = system_program;
    }
    Ok(())
}

pub(super) fn require_trusted_environment_v3(
    observation: TrustedEnvironmentObservationV3,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    if observation
        .current_slot
        .is_some_and(|(destination, current_slot)| scalars.get(destination) != Some(&current_slot))
        || observation
            .current_executing_program
            .is_some_and(|(destination, current_program)| {
                identities.get(destination) != Some(&current_program)
            })
        || observation
            .system_program
            .is_some_and(|(destination, system_program)| {
                identities.get(destination) != Some(&system_program)
            })
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
fn shadow_routes_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<Vec<ShadowResolvedRouteV3>, ProgramError> {
    let mut output = Vec::new();
    let mut route = 0_u16;
    while route < effect.route_count() {
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation_index = 0_u32;
        while invocation_index < count {
            let invocation = effect
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            let borrowed_witness = match invocation.borrowed_witness {
                Some(witness) => Some((
                    u32::try_from(witness.source_offset()).map_err(|_| TradingSbfError::Content)?,
                    u32::try_from(witness.len()).map_err(|_| TradingSbfError::Content)?,
                )),
                None => None,
            };
            let mut shadow_dependencies = Vec::new();
            let mut dependency_index = 0_u16;
            while dependency_index < invocation.receipt_dependencies.len() {
                let dependency = effect
                    .resolved_receipt_dependency(invocation.receipt_dependencies, dependency_index)
                    .map_err(|_| TradingSbfError::Content)?;
                shadow_dependencies.push(ShadowReceiptDependencyV3 {
                    producer_role: match dependency.producer_role {
                        FixedRole::Core => ShadowRouteRoleV3::Core,
                        FixedRole::Claims => ShadowRouteRoleV3::Claims,
                        FixedRole::Resolution => ShadowRouteRoleV3::Resolution,
                        FixedRole::Custody => ShadowRouteRoleV3::Custody,
                    },
                    producer_route: dependency.producer_route,
                    producer_invocation: dependency.producer_invocation,
                    expected_receipt_bytes: dependency.expected_receipt_bytes,
                });
                dependency_index = dependency_index
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?;
            }
            let receipt_dependency = if shadow_dependencies.len() == 1 {
                shadow_dependencies.first().copied()
            } else {
                None
            };
            let receipt_dependency_count =
                u16::try_from(shadow_dependencies.len()).map_err(|_| TradingSbfError::Content)?;
            let receipt_dependencies_digest = if shadow_dependencies.is_empty() {
                [0; 32]
            } else {
                receipt_dependencies_digest_v4(&shadow_dependencies)
                    .map_err(|_| TradingSbfError::Content)?
            };
            output.push(ShadowResolvedRouteV3 {
                role: match invocation.role {
                    FixedRole::Core => ShadowRouteRoleV3::Core,
                    FixedRole::Claims => ShadowRouteRoleV3::Claims,
                    FixedRole::Resolution => ShadowRouteRoleV3::Resolution,
                    FixedRole::Custody => ShadowRouteRoleV3::Custody,
                },
                kind: match invocation.kind {
                    dclutch_effect_kernel::v3::RouteKindV3::Once => ShadowRouteKindV3::Once,
                    dclutch_effect_kernel::v3::RouteKindV3::AffineOnce => {
                        ShadowRouteKindV3::AffineOnce
                    }
                    dclutch_effect_kernel::v3::RouteKindV3::Each => ShadowRouteKindV3::Each,
                },
                item: invocation.item,
                fixed_account_start: invocation.fixed_account_start,
                fixed_account_count: invocation.fixed_account_count,
                item_account_start: u32::try_from(invocation.item_account_start)
                    .map_err(|_| TradingSbfError::Content)?,
                item_account_count: invocation.item_account_count,
                item_account_stride: invocation.item_account_stride,
                repeated_item_count: invocation.repeated_item_count,
                request_offset: u32::try_from(invocation.request_offset)
                    .map_err(|_| TradingSbfError::Content)?,
                request_len: u32::try_from(invocation.request_len)
                    .map_err(|_| TradingSbfError::Content)?,
                borrowed_witness,
                receipt_dependency,
                receipt_dependency_count,
                receipt_dependencies_digest,
            });
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(output)
}
